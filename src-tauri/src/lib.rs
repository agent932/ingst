pub mod commands;
pub mod ingest;
pub mod utils;

pub use commands::*;
pub use ingest::*;
pub use utils::*;

#[cfg(test)]
mod tests {
    use std::path::Path;
    use std::env;

    #[test]
    fn test_hash_consistency() {
        use crate::utils::hashing::fast_hash;
        
        let temp_dir = env::temp_dir();
        let test_file = temp_dir.join("ingst_test.txt");
        std::fs::write(&test_file, "Hello, Ingst!").ok();
        
        let hash1 = fast_hash(test_file.to_str().unwrap(), 13).unwrap();
        let hash2 = fast_hash(test_file.to_str().unwrap(), 13).unwrap();
        
        assert_eq!(hash1, hash2);
        
        std::fs::remove_file(&test_file).ok();
    }

    #[test]
    fn test_collision_naming() {
        let temp_dir = env::temp_dir().join("ingst_test_collisions");
        std::fs::create_dir_all(&temp_dir).ok();
        
        std::fs::write(temp_dir.join("test.mp4"), "test").ok();
        
        use crate::ingest::plan::resolve_collision;
        let result = resolve_collision(&temp_dir, "test.mp4");
        
        assert!(result.to_string_lossy().contains("test_1.mp4"));
        
        std::fs::remove_dir_all(&temp_dir).ok();
    }

    #[test]
    fn test_path_date_parsing() {
        use crate::ingest::plan::parse_date_for_path;
        
        assert_eq!(parse_date_for_path("2024-01-15T10:30:00"), Some("2024/01".to_string()));
        assert_eq!(parse_date_for_path("2023-12-25T00:00:00"), Some("2023/12".to_string()));
        assert_eq!(parse_date_for_path("invalid"), None);
    }
}
