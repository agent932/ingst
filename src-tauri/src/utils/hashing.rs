use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// Bytes sampled from each end of a file by [`fast_hash`].
const SAMPLE_WINDOW: u64 = 64 * 1024;

/// Cheap content fingerprint: head window, tail window, and exact length.
///
/// This is a *candidate lookup key*, not proof of equality — any two files
/// sharing both windows and a length collide, because the middle is never
/// read. A caller must confirm a hit with [`full_hash`] before treating a file
/// as already ingested; acting on the fingerprint alone means real footage is
/// silently never copied. See `plan::is_confirmed_duplicate`.
pub fn fast_hash(path: &str, size: u64) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let path = Path::new(path);

    let mut file = File::open(path)?;
    let file_size = file.metadata()?.len();

    let mut hasher = Sha256::new();

    // `Read::read` may return fewer bytes than requested, which previously made
    // the fingerprint depend on how the filesystem happened to chunk the read.
    // `read_exact` removes that.
    let head_len = file_size.min(SAMPLE_WINDOW);
    let mut head = vec![0u8; head_len as usize];
    file.read_exact(&mut head)?;
    hasher.update(&head);

    // Sample the tail whenever the file extends past the head window. The old
    // guard required `file_size > SAMPLE_WINDOW * 2`, so files between 64 KiB
    // and 128 KiB were fingerprinted from their first 64 KiB and length alone —
    // every byte after that, including the last, was invisible. The windows may
    // overlap for such files; hashing the overlap is harmless.
    if file_size > SAMPLE_WINDOW {
        let tail_len = file_size.min(SAMPLE_WINDOW);
        let mut tail = vec![0u8; tail_len as usize];
        file.seek(SeekFrom::End(-(tail_len as i64)))?;
        file.read_exact(&mut tail)?;
        hasher.update(&tail);
    }

    hasher.update(size.to_le_bytes());

    Ok(hex::encode(hasher.finalize()))
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
