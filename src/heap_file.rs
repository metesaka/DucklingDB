use std::sync::{Arc, Mutex};

use crate::buffer_manager::BufferPoolManager;
use crate::slotted_page::{SlotId, SlottedPage};

pub type PageId = u64;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct TupleId {
    pub page_id: PageId,
    pub slot_id: SlotId,
}

pub struct HeapFile {
    buffer_pool_manager: Arc<Mutex<BufferPoolManager>>,
    pages: Vec<PageId>,
}

impl HeapFile {
    pub fn new(buffer_pool_manager: Arc<Mutex<BufferPoolManager>>) -> Self {
        Self {
            buffer_pool_manager,
            pages: Vec::new(),
        }
    }

    pub fn insert_tuple(&mut self, data: &[u8]) -> Option<TupleId> {
        // For each page in the heap file, try to insert the tuple
        // let mut bpm: std::sync::MutexGuard<'_, BufferPoolManager> = self.buffer_pool_manager.lock().unwrap();

        for &page_id in self.pages.iter() {
            let frame = {
                let mut bpm = self.buffer_pool_manager.lock().unwrap();
                bpm.fetch_page(page_id)?
            };
            let slot_id_opt = {
                let mut frame_lock: std::sync::MutexGuard<'_, crate::buffer_manager::Frame> =
                    frame.lock().unwrap();
                let mut sp: SlottedPage = SlottedPage::from_buffer(&mut frame_lock.data);
                let slot_id = sp.insert(data);
                if slot_id.is_some() {
                    frame_lock.is_dirty = true;
                }
                slot_id
            };
            {
                let mut bpm = self.buffer_pool_manager.lock().unwrap();
                let _ = bpm.unpin_page(page_id, slot_id_opt.is_some());
            }
            if let Some(slot_id) = slot_id_opt {
                return Some(TupleId { page_id, slot_id });
            }
        }
        // If we're here, no existing page could accommodate the tuple
        let (new_page_id, frame) = {
            let mut bpm = self.buffer_pool_manager.lock().unwrap();
            // Ideally have bpm.new_page(); using allocate + fetch for now:
            let pid = bpm.disk_manager.lock().unwrap().allocate_page().ok()?;
            let f = bpm.fetch_page(pid)?;
            (pid, f)
        };
        let slot_id = {
            let mut frame_lock = frame.lock().unwrap();
            let mut sp = SlottedPage::init(&mut frame_lock.data); // <-- init for fresh page
            let sid = sp.insert(data)?; // must succeed on empty page
            frame_lock.is_dirty = true;
            sid
        };
        {
            let mut bpm = self.buffer_pool_manager.lock().unwrap();
            let _ = bpm.unpin_page(new_page_id, true);
        }
        self.pages.push(new_page_id);

        Some(TupleId {
            page_id: new_page_id,
            slot_id,
        })
    }

    // Read a tuple given its TupleId
    pub fn read_tuple(&mut self, tid: TupleId) -> Option<Vec<u8>> {
        let frame = {
            let mut bpm = self.buffer_pool_manager.lock().unwrap();
            bpm.fetch_page(tid.page_id)?
        };
        let data_opt: Option<Vec<u8>> = {
            let mut frame_lock: std::sync::MutexGuard<'_, crate::buffer_manager::Frame> =
                frame.lock().unwrap();
            let sp = SlottedPage::from_buffer(&mut frame_lock.data);
            sp.read(tid.slot_id).map(|data| data.to_vec())
        };
        {
            let mut bpm = self.buffer_pool_manager.lock().unwrap();
            let _ = bpm.unpin_page(tid.page_id, false);
        }
        data_opt
    }
}
