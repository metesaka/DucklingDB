use crate::disk_manager::{DiskManager, Page, PAGE_SIZE};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// A Frame holds one page and its metadata.
pub struct Frame {
    page_id: u64,
    pub data: Page,
    pub is_dirty: bool,
    pin_count: u32,
}
impl Frame {
    pub fn copy(&self) -> Self {
        Self {
            page_id: self.page_id,
            data: self.data,
            is_dirty: self.is_dirty,
            pin_count: self.pin_count,
        }
    }
}

// The BufferPoolManager manages the buffer pool.
pub struct BufferPoolManager {
    buffer_pool: Vec<Arc<Mutex<Frame>>>,
    page_table: HashMap<u64, usize>, // page_id -> frame_id
    replacer: ClockReplacer,
    pub disk_manager: Arc<Mutex<DiskManager>>,
    free_list: Vec<usize>, // List of frame_ids that are free
}

impl BufferPoolManager {
    pub fn new(pool_size: usize, disk_manager: DiskManager) -> Self {
        let mut buffer_pool = Vec::with_capacity(pool_size);
        for _ in 0..pool_size {
            buffer_pool.push(Arc::new(Mutex::new(Frame {
                page_id: 0,
                data: [0; PAGE_SIZE],
                is_dirty: false,
                pin_count: 0,
            })));
        }
        BufferPoolManager {
            buffer_pool,
            page_table: HashMap::new(),
            replacer: ClockReplacer::new(pool_size),
            disk_manager: Arc::new(Mutex::new(disk_manager)),
            free_list: (0..pool_size).collect(),
        }
    }

    // Create and allocate a new page in the buffer pool.
    pub fn new_page(&mut self) -> Option<Arc<Mutex<Frame>>> {
        let frame_id = if let Some(free_frame_id) = self.free_list.pop() {
            free_frame_id
        } else if let Some(victim_frame_id) = self.replacer.victim() {
            // Evict the victim frame
            let victim_frame: Arc<Mutex<Frame>> = self.buffer_pool[victim_frame_id].clone();
            let victim_lock: std::sync::MutexGuard<'_, Frame> = victim_frame.lock().unwrap();
            if victim_lock.is_dirty {
                // Write back to disk if dirty
                self.disk_manager
                    .lock()
                    .unwrap()
                    .write_page(victim_lock.page_id, &victim_lock.data)
                    .unwrap();
            }
            self.page_table.remove(&victim_lock.page_id);
            victim_frame_id
        } else {
            // No free frame and no victim available
            return None;
        };
        // Allocate a new page id from disk manager
        let new_page_id = self.disk_manager.lock().unwrap().allocate_page().unwrap();
        // Initialize the frame
        let frame: Arc<Mutex<Frame>> = self.buffer_pool[frame_id].clone();
        {
            let mut frame_lock: std::sync::MutexGuard<'_, Frame> = frame.lock().unwrap();
            frame_lock.page_id = new_page_id;
            frame_lock.is_dirty = false;
            frame_lock.pin_count = 1;
            frame_lock.data = [0; PAGE_SIZE]; // New page is empty
        }
        self.page_table.insert(new_page_id, frame_id);
        self.replacer.pin(frame_id);
        Some(frame)
    }

    // Fetch a page from the buffer pool, loading it from disk if necessary.
    // Returns None if no frame is available.
    pub fn fetch_page(&mut self, page_id: u64) -> Option<Arc<Mutex<Frame>>> {
        // Check if the page is already in the buffer pool
        match self.page_table.get(&page_id) {
            Some(&frame_id) => {
                // Found the page
                let frame = self.buffer_pool[frame_id].clone();
                {
                    let mut frame_lock = frame.lock().unwrap();
                    frame_lock.pin_count += 1;
                }
                self.replacer.pin(frame_id);
                Some(frame)
            }
            None => {
                // Not found
                let frame_id = if let Some(free_frame_id) = self.free_list.pop() {
                    free_frame_id
                } else if let Some(victim_frame_id) = self.replacer.victim() {
                    // Evict the victim frame
                    let victim_frame: Arc<Mutex<Frame>> = self.buffer_pool[victim_frame_id].clone();
                    let victim_lock: std::sync::MutexGuard<'_, Frame> =
                        victim_frame.lock().unwrap();
                    if victim_lock.is_dirty {
                        // Write back to disk if dirty
                        self.disk_manager
                            .lock()
                            .unwrap()
                            .write_page(victim_lock.page_id, &victim_lock.data)
                            .unwrap();
                    }
                    self.page_table.remove(&victim_lock.page_id);
                    victim_frame_id
                } else {
                    // No free frame and no victim available
                    return None;
                };
                // Load the new page from disk
                let frame: Arc<Mutex<Frame>> = self.buffer_pool[frame_id].clone();
                {
                    let mut frame_lock: std::sync::MutexGuard<'_, Frame> = frame.lock().unwrap();
                    frame_lock.page_id = page_id;
                    frame_lock.is_dirty = false;
                    frame_lock.pin_count = 1;
                    self.disk_manager
                        .lock()
                        .unwrap()
                        .read_page(page_id, &mut frame_lock.data)
                        .unwrap();
                }
                self.page_table.insert(page_id, frame_id);
                self.replacer.pin(frame_id);
                Some(frame)
            }
        }
    }

    // Unpin a page in the buffer pool.
    // Unpin means that the page is no longer needed by the caller.
    pub fn unpin_page(&mut self, page_id: u64, is_dirty: bool) -> bool {
        match self.page_table.get(&page_id) {
            Some(&frame_id) => {
                let frame = self.buffer_pool[frame_id].clone();
                let mut frame_lock = frame.lock().unwrap();
                if frame_lock.pin_count > 0 {
                    frame_lock.pin_count -= 1;
                    if is_dirty {
                        frame_lock.is_dirty = true;
                    }
                    if frame_lock.pin_count == 0 {
                        self.replacer.unpin(frame_id);
                    }
                    true
                } else {
                    false
                }
            }
            None => false,
        }
    }
}

pub struct ClockReplacer {
    frames: Vec<Option<usize>>, // Holds the frame_ids of frames in the buffer pool
    clock_hand: usize,
}

impl ClockReplacer {
    pub fn new(pool_size: usize) -> Self {
        Self {
            frames: vec![None; pool_size],
            clock_hand: 0,
        }
    }

    // Finds a frame to evict.
    pub fn victim(&mut self) -> Option<usize> {
        for _ in 0..(2 * self.frames.len()) {
            // Loop at most twice to find a victim
            let frame_id = self.clock_hand;
            self.clock_hand = (self.clock_hand + 1) % self.frames.len();

            if let Some(id) = self.frames[frame_id] {
                // In a real implementation, you would check a ref bit or pin count.
                // For now, we'll just return the first frame we find.
                return Some(id);
            }
        }
        None // No frames to evict
    }

    // Add a frame to the replacer's tracking.
    pub fn pin(&mut self, frame_id: usize) {
        self.frames[frame_id] = None;
    }

    // Remove a frame from the replacer's tracking.
    pub fn unpin(&mut self, frame_id: usize) {
        self.frames[frame_id] = Some(frame_id);
    }
}

#[test]
fn clock_replacer_test() {
    let mut clock_replacer = ClockReplacer::new(3);
    clock_replacer.unpin(0);
    clock_replacer.unpin(1);
    clock_replacer.unpin(2);
    assert_eq!(clock_replacer.victim(), Some(0));
    clock_replacer.pin(0);
    assert_eq!(clock_replacer.victim(), Some(1));
    clock_replacer.pin(1);
    assert_eq!(clock_replacer.victim(), Some(2));
    clock_replacer.pin(2);
    assert_eq!(clock_replacer.victim(), None);
}
