use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
pub const PAGE_SIZE: usize = 4096;

// A Page is just an array of bytes.
pub type Page = [u8; PAGE_SIZE];
pub struct DiskManager {
    db_file: File,
    num_pages: u64,
}

impl DiskManager {
    // Create a new DiskManager with the given file path.
    pub fn new(file_path: &str) -> Self {
        let db_file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(file_path)
            .expect("Failed to open database file");
        DiskManager { db_file, num_pages: 0 }
    }

    // Read a page from the database file.
    pub fn read_page(&mut self, page_id: u64, page: &mut Page) -> std::io::Result<()> {
        let offset = page_id * PAGE_SIZE as u64;
        self.db_file
            .seek(SeekFrom::Start(offset))
            .expect("Failed to seek to page");
        self.db_file
            .read_exact(page)
            .expect("Failed to read page");
    Ok(())
    }

    // Write a page to the database file.
    pub fn write_page(&mut self, page_id: u64, page: &Page) -> std::io::Result<()> {
        let offset = page_id * PAGE_SIZE as u64;
        self.db_file
            .seek(SeekFrom::Start(offset))
            .expect("Failed to seek to page");
        self.db_file
            .write_all(page)
            .expect("Failed to write page");
        self.db_file.flush()?;
        self.num_pages = self.num_pages.max(page_id + 1);
        Ok(())
    }

    pub fn allocate_page(&mut self) -> std::io::Result<u64> {
        let new_page_id = self.num_pages as u64 + 1 as u64;
        self.num_pages += 1;
        let new_page: Page = [0; PAGE_SIZE];
        self.write_page(new_page_id, &new_page).unwrap();
        Ok(new_page_id)
    }
}