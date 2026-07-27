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

#[cfg(test)]
mod tests {
    use super::*;

    const CHUNK: usize = 64 * 1024; // the head/tail window fast_hash samples

    fn tmpdir(name: &str) -> std::path::PathBuf {
        let p = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn write(dir: &Path, name: &str, bytes: &[u8]) -> String {
        let p = dir.join(name);
        std::fs::write(&p, bytes).unwrap();
        p.to_string_lossy().to_string()
    }

    fn filler(len: usize) -> Vec<u8> {
        (0..len).map(|i| (i % 251) as u8).collect()
    }

    /// DOCUMENTS A REAL LIMITATION, not a guarantee.
    ///
    /// `fast_hash` samples only the first 64 KiB, the last 64 KiB and the byte
    /// count, so two clips of the same length that differ anywhere in between
    /// hash identically. With `skip_duplicates` on, the second one is reported
    /// as a duplicate and never copied — a real risk for camera formats that
    /// share a header and a trailer (same camera, same settings, same length).
    /// `full_hash`, which the executor uses to verify a copy, does tell them
    /// apart; only the *dedup* decision is blind.
    ///
    /// If this test fails, fast_hash's coverage changed. That is an improvement,
    /// but the skip-duplicates behaviour it documents must be re-checked.
    #[test]
    fn fast_hash_cannot_see_changes_in_the_middle_of_a_file() {
        let dir = tmpdir("ingst_fast_hash_middle");

        let size = 300 * 1024;
        let a = filler(size);
        // Untouched head and tail; flip a byte deep in the unsampled middle.
        let mut b = a.clone();
        b[size / 2] ^= 0xFF;

        assert_eq!(a[..CHUNK], b[..CHUNK], "test setup: heads must match");
        assert_eq!(a[size - CHUNK..], b[size - CHUNK..], "test setup: tails must match");

        let pa = write(&dir, "take_a.mp4", &a);
        let pb = write(&dir, "take_b.mp4", &b);

        assert_eq!(
            fast_hash(&pa, size as u64).unwrap(),
            fast_hash(&pb, size as u64).unwrap(),
            "known limitation: middle-of-file differences are invisible to fast_hash"
        );
        assert_ne!(
            full_hash(&pa).unwrap(),
            full_hash(&pb).unwrap(),
            "full_hash must still distinguish them — it is the only check that can"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// DOCUMENTS A REAL LIMITATION.
    ///
    /// The tail is only sampled when the file is larger than 128 KiB, so for a
    /// file between 64 KiB and 128 KiB everything after the first 64 KiB is
    /// unhashed. That covers plenty of real stills, LUTs and subtitle files.
    #[test]
    fn fast_hash_ignores_everything_after_64k_in_a_100k_file() {
        let dir = tmpdir("ingst_fast_hash_small");

        let size = 100 * 1024; // > CHUNK, but not > 2 * CHUNK
        let a = filler(size);
        let mut b = a.clone();
        b[size - 1] ^= 0xFF; // last byte of the file

        let pa = write(&dir, "still_a.jpg", &a);
        let pb = write(&dir, "still_b.jpg", &b);

        assert_eq!(
            fast_hash(&pa, size as u64).unwrap(),
            fast_hash(&pb, size as u64).unwrap(),
            "known limitation: no tail sample below 128 KiB, so the end is unhashed"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// What fast_hash *does* catch. A change in the head, in the tail, or in
    /// the length must change the digest — if any of these stopped registering,
    /// distinct takes would be silently skipped as duplicates and left on a
    /// card that is about to be formatted.
    #[test]
    fn fast_hash_detects_head_tail_and_size_changes() {
        let dir = tmpdir("ingst_fast_hash_detects");

        let size = 300 * 1024;
        let base = filler(size);
        let baseline = fast_hash(&write(&dir, "base.mp4", &base), size as u64).unwrap();

        let mut head = base.clone();
        head[0] ^= 0xFF;
        assert_ne!(
            fast_hash(&write(&dir, "head.mp4", &head), size as u64).unwrap(),
            baseline,
            "a difference in the first bytes must be detected"
        );

        let mut tail = base.clone();
        tail[size - 1] ^= 0xFF;
        assert_ne!(
            fast_hash(&write(&dir, "tail.mp4", &tail), size as u64).unwrap(),
            baseline,
            "a difference in the last bytes must be detected"
        );

        let mut longer = base.clone();
        longer.extend_from_slice(&[0u8; 1024]);
        assert_ne!(
            fast_hash(&write(&dir, "longer.mp4", &longer), longer.len() as u64).unwrap(),
            baseline,
            "a difference in length must be detected"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// The same file must hash the same way on the plan pass and on any later
    /// run, including the seek-to-tail path taken by files over 128 KiB. An
    /// unstable digest would make the dedup index useless — every re-ingest
    /// would look like new footage.
    #[test]
    fn fast_hash_is_stable_across_calls_on_a_file_large_enough_to_seek() {
        let dir = tmpdir("ingst_fast_hash_stable");
        let size = 5 * 1024 * 1024;
        let path = write(&dir, "clip.mp4", &filler(size));

        let first = fast_hash(&path, size as u64).unwrap();
        for _ in 0..5 {
            assert_eq!(fast_hash(&path, size as u64).unwrap(), first);
        }

        std::fs::remove_dir_all(&dir).ok();
    }
}
