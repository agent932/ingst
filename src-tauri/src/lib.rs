pub mod commands;
pub mod ingest;
pub mod utils;

pub use commands::*;
pub use ingest::*;
pub use utils::*;

#[cfg(test)]
mod tests {
    use crate::ingest::plan::{parse_date_for_path, resolve_collision};
    use crate::utils::hashing::{fast_hash, full_hash};
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};

    /// A scratch directory that is emptied on the way in, so a run that panicked
    /// before its own cleanup cannot change what the next run sees.
    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Write `len` bytes of a varying pattern, with `edits` applied by offset.
    fn write_pattern(path: &Path, len: usize, edits: &[(usize, u8)]) {
        let mut data: Vec<u8> = (0..len).map(|i| (i % 251) as u8).collect();
        for (offset, byte) in edits {
            data[*offset] = *byte;
        }
        std::fs::write(path, &data).unwrap();
    }

    fn fast(path: &Path, size: u64) -> String {
        fast_hash(path.to_str().unwrap(), size).unwrap()
    }

    fn full(path: &Path) -> String {
        full_hash(path.to_str().unwrap()).unwrap()
    }

    /// `fast_hash` decides whether a file is a duplicate of one already ingested,
    /// and a duplicate is never copied. So the property that protects footage is
    /// that *different* files hash differently — that the same file hashes the
    /// same way twice is something SHA-256 gives for free.
    #[test]
    fn fast_hash_separates_files_that_differ() {
        let dir = fresh_dir("ingst_fast_hash");
        // Over 128 KiB, so `fast_hash` reads its tail window as well as its head.
        const LEN: usize = 300 * 1024;

        let base = dir.join("base.bin");
        let head = dir.join("head.bin");
        let tail = dir.join("tail.bin");
        write_pattern(&base, LEN, &[]);
        write_pattern(&head, LEN, &[(0, 0xAA)]);
        write_pattern(&tail, LEN, &[(LEN - 1, 0xAA)]);

        let size = LEN as u64;

        assert_ne!(
            fast(&base, size),
            fast(&head, size),
            "a difference inside the first 64 KiB must change the hash"
        );
        assert_ne!(
            fast(&base, size),
            fast(&tail, size),
            "a difference inside the last 64 KiB must change the hash; drop the \
             tail read and a truncated or re-recorded clip hashes as a duplicate"
        );
        assert_ne!(
            fast(&base, size),
            fast(&base, size + 1),
            "length is part of the identity: files that sample alike but are \
             different lengths are not duplicates"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// `fast_hash` samples the first 64 KiB, the last 64 KiB and the length, so
    /// it is blind to any difference that falls between those windows. This test
    /// pins that blind spot: the files below genuinely differ (their full hashes
    /// differ) and `fast_hash` still calls them identical — which, with
    /// `skip_duplicates` on, is how a real take gets skipped and never copied.
    ///
    /// If this ever fails because the sampling got stronger, that is an
    /// improvement: delete the assertion rather than restoring the old behaviour.
    #[test]
    fn fast_hash_cannot_see_between_its_sample_windows() {
        let dir = fresh_dir("ingst_fast_hash_blind_spot");

        // Over 128 KiB: head and tail are both read, the middle never is.
        const BIG: usize = 300 * 1024;
        let a = dir.join("a.bin");
        let b = dir.join("b.bin");
        write_pattern(&a, BIG, &[]);
        write_pattern(&b, BIG, &[(BIG / 2, 0xAA)]);

        assert_ne!(full(&a), full(&b), "the two files must really differ");
        assert_eq!(
            fast(&a, BIG as u64),
            fast(&b, BIG as u64),
            "inherent to sampling: bytes between the head and tail windows are \
             never read. This is why a fingerprint match is only a candidate — \
             plan::is_confirmed_duplicate compares whole files before skipping, \
             so a collision here costs a redundant read, not lost footage."
        );

        // Files of 64 KiB..=128 KiB used to read no tail window at all, because
        // the guard demanded `size > window * 2`. Everything past the first
        // 64 KiB — including the final byte — was invisible, so two such files
        // collided and one was silently skipped as a duplicate.
        const SMALL: usize = 100 * 1024;
        let c = dir.join("c.bin");
        let d = dir.join("d.bin");
        write_pattern(&c, SMALL, &[]);
        write_pattern(&d, SMALL, &[(SMALL - 1, 0xAA)]);

        assert_ne!(full(&c), full(&d), "the two files must really differ");
        assert_ne!(
            fast(&c, SMALL as u64),
            fast(&d, SMALL as u64),
            "a file in the 64..=128 KiB range differing only in its last byte \
             must be distinguished: the tail window is now read whenever the \
             file extends past the head window"
        );

        std::fs::remove_dir_all(&dir).ok();
    }

    /// A name already on disk must never be handed back as a destination: the
    /// executor renames onto whatever path it is given, so returning an occupied
    /// path would overwrite footage that is already ingested.
    #[test]
    fn test_collision_naming() {
        let dir = fresh_dir("ingst_test_collisions");
        let existing = dir.join("test.mp4");
        std::fs::write(&existing, "test").unwrap();

        let mut claimed = HashSet::new();
        let result = resolve_collision(&dir, "test.mp4", &mut claimed);

        assert_ne!(result, existing, "must not hand back an existing file's path");
        assert!(!result.exists(), "the chosen destination must be free");
        assert!(result.to_string_lossy().contains("test_1.mp4"));

        std::fs::remove_dir_all(&dir).ok();
    }

    /// Two source files with the same name must never be assigned the same
    /// destination, even though neither exists on disk when the plan is built.
    #[test]
    fn test_collision_within_plan() {
        let dir = fresh_dir("ingst_test_plan_collisions");
        let mut claimed = HashSet::new();

        let first  = resolve_collision(&dir, "IMG_0001.JPG", &mut claimed);
        let second = resolve_collision(&dir, "IMG_0001.JPG", &mut claimed);
        let third  = resolve_collision(&dir, "IMG_0001.JPG", &mut claimed);

        assert_ne!(first, second);
        assert_ne!(second, third);
        assert_ne!(first, third);
        assert!(second.to_string_lossy().contains("IMG_0001_1.JPG"));
        assert!(third.to_string_lossy().contains("IMG_0001_2.JPG"));

        // Case-insensitive filesystems: a name differing only in case is the
        // same file, so it must not be given a path any of the three occupy.
        let lower = resolve_collision(&dir, "img_0001.jpg", &mut claimed);
        let lower_key = lower.to_string_lossy().to_lowercase();
        for taken in [&first, &second, &third] {
            assert_ne!(
                lower_key,
                taken.to_string_lossy().to_lowercase(),
                "case-only difference must still count as claimed"
            );
        }

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn test_path_date_parsing() {
        assert_eq!(parse_date_for_path("2024-01-15T10:30:00"), Some("2024/01".to_string()));
        assert_eq!(parse_date_for_path("2023-12-25T00:00:00"), Some("2023/12".to_string()));
        assert_eq!(parse_date_for_path("invalid"), None);

        // Characterisation, not endorsement: an unpadded month yields a 6-char
        // string, and `build_plan` slices the result at [0..4] and [5..7] — that
        // second slice panics on anything shorter than 7 bytes. If this
        // assertion starts failing because the month is padded, the hazard is
        // gone and the assertion should be updated.
        assert_eq!(parse_date_for_path("2024-1-15T10:30:00"), Some("2024/1".to_string()));
    }
}
