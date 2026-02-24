use std::path::Path;

pub fn normalize_path(path: &str) -> String {
    let path = Path::new(path);
    path.to_string_lossy().to_string()
}

pub fn get_file_stem(path: &str) -> String {
    Path::new(path)
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn get_extension(path: &str) -> String {
    Path::new(path)
        .extension()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default()
}

pub fn ensure_dir(path: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    std::fs::create_dir_all(Path::new(path))?;
    Ok(())
}
