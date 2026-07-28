pub mod commands;
pub mod ingest;
pub mod utils;

pub use commands::*;
pub use ingest::*;
pub use utils::*;

#[cfg(test)]
mod tests {
    use crate::ingest::plan::{parse_date_for_path, parse_year_month, resolve_collision};
    use std::collections::HashSet;
    use std::path::PathBuf;

    /// A scratch directory that is emptied on the way in, so a run that panicked
    /// before its own cleanup cannot change what the next run sees.
    fn fresh_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(name);
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
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

        // ffprobe and EXIF do not promise zero padding, so the month is padded
        // here — every value that comes back is the same shape.
        assert_eq!(parse_date_for_path("2024-1-15T10:30:00"), Some("2024/01".to_string()));
        assert_eq!(parse_date_for_path("2024-1"), Some("2024/01".to_string()));

        // A date that isn't a real year and month is rejected outright, so the
        // caller files it under an explicit unknown folder instead of under a
        // folder named from a slice of the garbage.
        assert_eq!(parse_date_for_path("2024-13-01T00:00:00"), None);
        assert_eq!(parse_date_for_path("2024-00-01T00:00:00"), None);
        assert_eq!(parse_date_for_path("24-01-15T10:30:00"), None);
        assert_eq!(parse_date_for_path("notayear-01-15"), None);
        assert_eq!(parse_date_for_path("2024-ab-15"), None);
    }

    /// `build_plan_sync` joins these two halves straight in as folder names, so
    /// each must be usable on its own — the month already padded, and neither
    /// one carved out of a longer string by byte offset.
    #[test]
    fn year_month_pair_is_folder_ready() {
        assert_eq!(
            parse_year_month("2024-1-15T10:30:00"),
            Some(("2024".to_string(), "01".to_string()))
        );
        assert_eq!(
            parse_year_month("2023-12-25T00:00:00"),
            Some(("2023".to_string(), "12".to_string()))
        );
        assert_eq!(parse_year_month("2024-13-01"), None);
        assert_eq!(parse_year_month("UnknownDate"), None);
    }
}
