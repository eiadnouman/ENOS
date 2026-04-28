use alloc::string::String;
use alloc::vec::Vec;
use lazy_static::lazy_static;
use spin::Mutex;

const MAX_FILES: usize = 64;
const MAX_FILE_NAME_LEN: usize = 32;
const MAX_FILE_SIZE: usize = 2048;

#[derive(Clone)]
struct FileEntry {
    name: String,
    contents: String,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct FsStats {
    pub file_count: usize,
    pub total_bytes: usize,
    pub max_files: usize,
    pub max_file_size: usize,
}

lazy_static! {
    static ref FILES: Mutex<Vec<FileEntry>> = Mutex::new(Vec::new());
}

fn is_valid_name(name: &str) -> bool {
    if name.is_empty() || name.len() > MAX_FILE_NAME_LEN {
        return false;
    }

    name.bytes()
        .all(|b| matches!(b, b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b'_' | b'-' | b'.'))
}

pub fn create_file(name: &str) -> Result<(), &'static str> {
    if !is_valid_name(name) {
        return Err("invalid file name");
    }

    let mut files = FILES.lock();
    if files.iter().any(|f| f.name == name) {
        return Err("file already exists");
    }

    if files.len() >= MAX_FILES {
        return Err("file table is full");
    }

    files.push(FileEntry {
        name: String::from(name),
        contents: String::new(),
    });
    Ok(())
}

pub fn delete_file(name: &str) -> Result<(), &'static str> {
    let mut files = FILES.lock();
    let Some(idx) = files.iter().position(|f| f.name == name) else {
        return Err("file not found");
    };
    files.remove(idx);
    Ok(())
}

pub fn write_file(name: &str, content: &str) -> Result<(), &'static str> {
    if content.len() > MAX_FILE_SIZE {
        return Err("content too large");
    }

    let mut files = FILES.lock();
    let Some(file) = files.iter_mut().find(|f| f.name == name) else {
        return Err("file not found");
    };

    file.contents.clear();
    file.contents.push_str(content);
    Ok(())
}

pub fn read_file(name: &str) -> Result<String, &'static str> {
    let files = FILES.lock();
    let Some(file) = files.iter().find(|f| f.name == name) else {
        return Err("file not found");
    };
    Ok(file.contents.clone())
}

pub fn list_files() -> Vec<FileInfo> {
    let files = FILES.lock();
    files
        .iter()
        .map(|file| FileInfo {
            name: file.name.clone(),
            size: file.contents.len(),
        })
        .collect()
}

pub fn stats() -> FsStats {
    let files = FILES.lock();
    let mut total_bytes = 0usize;
    for file in files.iter() {
        total_bytes = total_bytes.saturating_add(file.contents.len());
    }

    FsStats {
        file_count: files.len(),
        total_bytes,
        max_files: MAX_FILES,
        max_file_size: MAX_FILE_SIZE,
    }
}
