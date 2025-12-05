mod disk_manager;
mod buffer_manager;
mod slotted_page;
mod heap_file;
use crate::disk_manager::{DiskManager, Page, PAGE_SIZE};
use crate::buffer_manager::{BufferPoolManager, ClockReplacer};
use crate::slotted_page::{SlottedPage, SlotId}; 
use crate::heap_file::HeapFile;

// The DiskManager is responsible for reading and writing pages to the database file.

pub fn clock_replacer_test() {
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
    println!("ClockReplacer tests passed.");
}


fn main() {
    let mut disk_manager = DiskManager::new("test.db");
    let mut page: Page = [2; PAGE_SIZE];
    let page2: Page = [1; PAGE_SIZE];
    disk_manager.write_page(0, &page).unwrap();
    disk_manager.write_page(1, &page2).unwrap();
    disk_manager.read_page(0, &mut page).unwrap();
    println!("Read page: {:?}", &page[..16]); // Print first 16 bytes for brevity

    clock_replacer_test();
    // BufferPoolManager test
    let mut buffer_pool_manager = BufferPoolManager::new(2, disk_manager);
    let frame1 = buffer_pool_manager.fetch_page(0).unwrap();
    {
        let frame1_lock = frame1.lock().unwrap();
        println!("Fetched page 0: {:?}", &frame1_lock.data[..16]); // Print first 16 bytes for brevity  
    }
    let frame2 = buffer_pool_manager.fetch_page(1).unwrap();
    {
        let frame2_lock = frame2.lock().unwrap();
        println!("Fetched page 1: {:?}", &frame2_lock.data[..16]); // Print first 16 bytes for brevity  
    }

    let mut page: Page = [0u8; PAGE_SIZE];
    let mut sp = SlottedPage::init(&mut page);

    let t1: &[u8; 11] = b"hello world";
    let t2 = b"database systems are fun";

    let id1 = sp.insert(t1).unwrap();
    let id2 = sp.insert(t2).unwrap();

    println!("Inserted tuples {:?} and {:?}", id1, id2);

    let read1 = sp.read(id1).unwrap();
    let read2 = sp.read(id2).unwrap();

    println!("Read 1: {:?}", std::str::from_utf8(read1).unwrap());
    println!("Read 2: {:?}", std::str::from_utf8(read2).unwrap());

    let t3 = b"another tuple";
    let id3: SlotId = sp.insert(t3).unwrap();

    sp.delete(id2)  ;

    

    let a = sp.insert(b"short").unwrap();
    let updated = sp.update(a, b"this is much longer than short");
    // force a hole, then compact
    sp.compact();
    if updated {
        println!("Updated slot {:?} successfully.", a);
        // assert_eq!(sp.read(a).unwrap(), b"this is much longer than short");
    } else {
        println!("Failed to update slot {:?}.", a);
    }
    for (slot,tuple) in sp.iter() {
        println!("Slot ID:{:?}- tuple: {:?}",slot, std::str::from_utf8(tuple).unwrap());
    }


        let dm = DiskManager::new("test.db");
    let bpm = BufferPoolManager::new(8, dm);
    let bpm = std::sync::Arc::new(std::sync::Mutex::new(bpm));

    let mut hf = HeapFile::new(bpm.clone());

    println!("Inserting tuples into HeapFile...");
    let r1 = hf.insert_tuple(b"alice").unwrap();
    let r2 = hf.insert_tuple(b"bob").unwrap();
    let r3 = hf.insert_tuple(b"carol").unwrap();

    println!("Inserted RIDs: {:?} {:?} {:?}", r1, r2, r3);

    let v1 = hf.read_tuple(r1).unwrap();
    println!("get(r1) = {}", std::str::from_utf8(&v1).unwrap());


    
}
