use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

pub fn fast_hash(path: &str, size: u64) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(path);
    
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut hasher = Sha256::new();

    let header_size = 64 * 1024;
    let mut header_buf = vec![0u8; header_size];
    let bytes_read = reader.read(&mut header_buf)?;
    hasher.update(&header_buf[..bytes_read]);

    // Read footer from the same file handle — no second open needed.
    let file_size = path.metadata()?.len();
    if file_size > header_size as u64 * 2 {
        if reader.seek(SeekFrom::End(-(64 * 1024) as i64)).is_ok() {
            let mut footer_buf = vec![0u8; 64 * 1024];
            let bytes_read = reader.read(&mut footer_buf)?;
            hasher.update(&footer_buf[..bytes_read]);
        }
    }
    
    hasher.update(size.to_le_bytes());
    
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

pub fn full_hash(path: &str) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    full_hash_with_progress(path, None)
}

/// SHA-256 of a whole file, adding each chunk's length to `progress` as it goes.
///
/// Media files run to gigabytes, so hashing is slow enough that the UI needs to
/// show movement — without a counter the interface looks hung during verify.
pub fn full_hash_with_progress(
    path: &str,
    progress: Option<&AtomicU64>,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(path);
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);

    let mut hasher = Sha256::new();
    // 1 MiB: the old 8 KiB buffer meant ~125k syscalls per gigabyte.
    let mut buffer = vec![0u8; 1024 * 1024];

    loop {
        let bytes_read = reader.read(&mut buffer)?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
        if let Some(p) = progress {
            p.fetch_add(bytes_read as u64, Ordering::Relaxed);
        }
    }

    let result = hasher.finalize();
    Ok(hex::encode(result))
}
