use crate::disk_manager::PAGE_SIZE;
use crate::disk_manager::Page;
pub const INVALID_SLOT: u16 = 0xFFFF;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotId(pub u16);

/// SlottedPage: manages variable-length tuples in one page.
pub struct SlottedPage<'a> {
    buf: &'a mut Page,
}

/// Header layout
/// [0..2): free_start (u16)
/// [2..4): free_end (u16)
/// [4..6): num_slots (u16)
const HDR_FREE_START: usize = 0;
const HDR_FREE_END: usize = 2;
const HDR_NUM_SLOTS: usize = 4;
const SLOT_ENTRY_SIZE: usize = 4; // offset(2) + len(2)

impl<'a> SlottedPage<'a> {
    /// Initialize an empty page
    pub fn init(buf: &'a mut [u8; PAGE_SIZE]) -> Self {
        let total: u16 = PAGE_SIZE as u16;
        buf[HDR_FREE_START..HDR_FREE_START + 2].copy_from_slice(&6u16.to_le_bytes()); // store the place where free bytes start in bytes 0-1 (initially 6 (header size))
        buf[HDR_FREE_END..HDR_FREE_END + 2].copy_from_slice(&total.to_le_bytes()); // store the total page size in bytes 2-3 (initially 4096)
        buf[HDR_NUM_SLOTS..HDR_NUM_SLOTS + 2].copy_from_slice(&0u16.to_le_bytes()); // store number of slots (initially 0) in bytes 4-5 
        Self { buf }
    }

    pub fn from_buffer(buf: &'a mut [u8; PAGE_SIZE]) -> Self {
        Self { buf }
    }

    fn free_start(&self) -> u16 {
        // Read starting place size from bytes 0-1
        u16::from_le_bytes(self.buf[HDR_FREE_START..HDR_FREE_START + 2].try_into().unwrap())
    }
    fn free_end(&self) -> u16 {
        // Read total page size from bytes 2-3
        u16::from_le_bytes(self.buf[HDR_FREE_END..HDR_FREE_END + 2].try_into().unwrap())
    }
    fn num_slots(&self) -> u16 {
        // Read number of slots from bytes 4-5
        u16::from_le_bytes(self.buf[HDR_NUM_SLOTS..HDR_NUM_SLOTS + 2].try_into().unwrap())
    }

    // these functions are to modify the header fields with new integer values (u16), makes life easier not to deal with byte slices directly
    fn set_free_start(&mut self, val: u16) {
        self.buf[HDR_FREE_START..HDR_FREE_START + 2].copy_from_slice(&val.to_le_bytes());
    }
    fn set_free_end(&mut self, val: u16) {
        self.buf[HDR_FREE_END..HDR_FREE_END + 2].copy_from_slice(&val.to_le_bytes());
    }
    fn set_num_slots(&mut self, val: u16) {
        self.buf[HDR_NUM_SLOTS..HDR_NUM_SLOTS + 2].copy_from_slice(&val.to_le_bytes());
    }

    // Tuple metadata (slot entries) management
    // First two bytes: offset (u16)
    // Next two bytes: length (u16)
    // This metadata is stored at the end of the page and grows backwards
    // Slot 0 -> 4092-4095, Slot 1 -> 4088-4091, etc.
    fn slot_offset(&self, slot_id: u16) -> usize {
        PAGE_SIZE - ((slot_id as usize + 1) * SLOT_ENTRY_SIZE)
    }


    // Read Slot, finds metadata for the given slot_id
    // First two bytes: offset (u16)
    // Next two bytes: length (u16)
    // This will be used by other functions: page[offset..offset+length] -> actual tuple data
    fn read_slot(&self, slot_id: u16) -> (u16, u16) {
        let off: usize = self.slot_offset(slot_id);
        let offset = u16::from_le_bytes(self.buf[off..off + 2].try_into().unwrap());
        let len = u16::from_le_bytes(self.buf[off + 2..off + 4].try_into().unwrap());
        (offset, len)
    }

    // Write slot write metadata for the given slot_id 
    /// First two bytes: offset (u16)
    /// Next two bytes: length (u16)
    fn write_slot(&mut self, slot_id: u16, offset: u16, len: u16) {
        let off = self.slot_offset(slot_id);
        self.buf[off..off + 2].copy_from_slice(&offset.to_le_bytes());
        self.buf[off + 2..off + 4].copy_from_slice(&len.to_le_bytes());
    }

    /// Insert a tuple (variable length)
    pub fn insert(&mut self, tuple: &[u8]) -> Option<SlotId> {
        let num_slots = self.num_slots();
        let free_start = self.free_start();
        let free_end = self.free_end();
        let need_space = tuple.len() as u16 + SLOT_ENTRY_SIZE as u16;

        if free_start + need_space > free_end {
            return None; // no space
        }

        // Copy tuple into free space
        let offset: u16 = free_start;
        self.buf[offset as usize..offset as usize + tuple.len()].copy_from_slice(tuple);

        // Update header
        self.set_free_start(offset + tuple.len() as u16);
        self.set_num_slots(num_slots + 1);
        self.set_free_end(free_end - SLOT_ENTRY_SIZE as u16);

        // Write slot entry
        self.write_slot(num_slots, offset, tuple.len() as u16);
        Some(SlotId(num_slots))
    }

    /// Read a tuple
    pub fn read(&self, slot: SlotId) -> Option<&[u8]> {
        if slot.0 >= self.num_slots() {
            return None;
        }
        let (offset, len) = self.read_slot(slot.0);
        if len == INVALID_SLOT {
            return None;
        }
        Some(&self.buf[offset as usize..offset as usize + len as usize])
    }

    // Tuple Iterator
    pub fn iter(&self) -> SlottedPageIterator<'_> {
        SlottedPageIterator {
            sp: self,
            current_slot: 0,
        }
    }

    // Compact the page to remove fragmentation
    pub fn compact(&mut self) {
        let num_slots = self.num_slots();
        let mut tuples: Vec<(u16,u16, u16)> = Vec::new(); // (slot_id, offset, len)

        // Collect valid tuples
        for slot_id in 0..num_slots {
            let (offset, len) = self.read_slot(slot_id);
            if len != INVALID_SLOT {
                tuples.push((slot_id, offset, len));
            }
        }

        // Sort tuples by offset
        tuples.sort_by_key(|&(_,offset, _)| offset);

        // Rebuild the page with keeping slot ids the same
        let mut new_free_start: u16 = 6; // header size
        for (i, &(slot_id,old_offset, len)) in tuples.iter().enumerate() {
            // Move tuple to new location
            let slice:Vec<u8>  = self.buf[old_offset as usize..old_offset as usize + len as usize].to_vec();

            self.buf[new_free_start as usize..new_free_start as usize + len as usize]
                .copy_from_slice(&slice);
            // Update slot entry
            self.write_slot(slot_id as u16, new_free_start, len);
            new_free_start += len;
        }

        // Update header
        self.set_free_start(new_free_start);
        self.set_free_end(PAGE_SIZE as u16 - (num_slots - tuples.len() as u16) * SLOT_ENTRY_SIZE as u16);
    }

    pub fn largest_contiguous_free(&self) -> usize {
        let free_start = self.free_start() as usize;
        let free_end = self.free_end() as usize;
        if free_end >= free_start {
            free_end - free_start
        } else {
            0 
        }
    }

    // Update
    // If new tuple size is less than or equal to old size, do in-place update
    // If new tuple size is greater, call delete + insert
    pub fn update(&mut self, slot: SlotId, new_tuple: &[u8]) -> bool {
        if slot.0 >= self.num_slots() {
            return false;
        }
        let (offset, len) = self.read_slot(slot.0);
        if len == INVALID_SLOT {
            return false;
        }
        if new_tuple.len() as u16 <= len {
            // In-place update
            self.buf[offset as usize..offset as usize + new_tuple.len()]
                .copy_from_slice(new_tuple);
            // If new tuple is smaller, we can optionally update the length in slot metadata
            self.write_slot(slot.0, offset, new_tuple.len() as u16);
            return true ;
        } 

        // Case 2: needs more space — try to make a large contiguous chunk
        if self.largest_contiguous_free() < new_tuple.len() {
            self.compact();
            if self.largest_contiguous_free() < new_tuple.len() {
                return false; // still no room on this page
            }
        }

        // Place the new bytes at free_start, then repoint the SAME slot
        let new_off = self.free_start();
        let new_len = new_tuple.len() as u16;
        let dst = new_off as usize;
        self.buf[dst .. dst + new_tuple.len()].copy_from_slice(new_tuple);
        self.set_free_start(new_off + new_len);

        // Repoint slot -> new location
        self.write_slot(slot.0, new_off, new_len);

        // Old region [off..off+len] becomes a hole; compact() will reclaim later.
        return true;
    }

    // Delete a tuple
    pub fn delete(&mut self, slot: SlotId) -> bool {
        if slot.0 >= self.num_slots() { // Slot does not exist
            return false;
        }
        // get slot metadata
        let (offset, len) = self.read_slot(slot.0);
        if len == INVALID_SLOT { // Already deleted
            return false;
        }
        // Mark slot as deleted
        self.write_slot(slot.0, offset, INVALID_SLOT);
        true
    }
        
}

pub struct SlottedPageIterator<'a> {
    sp: &'a SlottedPage<'a>,
    current_slot: u16,
}

impl <'a> Iterator for SlottedPageIterator<'a> {
    type Item = (SlotId, &'a [u8]);

    fn next(&mut self) -> Option<Self::Item> {
        while self.current_slot < self.sp.num_slots() {
            let slot_id = self.current_slot;
            self.current_slot += 1;
            let (offset, len) = self.sp.read_slot(slot_id);
            if len != INVALID_SLOT {
                let data = &self.sp.buf[offset as usize..offset as usize + len as usize];
                return Some((SlotId(slot_id), data));
            }else{
                continue;
            }
        }
        None
    }
}
