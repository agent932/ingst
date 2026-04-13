use std::path::Path;

/// Resolve a bundled sidecar binary by name (no extension, no triple).
/// Checks next to the running executable first (production Tauri sidecar
/// naming: `{name}-{target_triple}[.exe]`), then falls back to PATH.
fn sidecar_path(name: &str) -> std::path::PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidates: &[&str] = if cfg!(target_os = "windows") {
                &[
                    concat!("-x86_64-pc-windows-msvc.exe"),
                    concat!("-aarch64-pc-windows-msvc.exe"),
                    ".exe",
                ]
            } else if cfg!(target_os = "macos") {
                &["-x86_64-apple-darwin", "-aarch64-apple-darwin", ""]
            } else {
                &["-x86_64-unknown-linux-gnu", ""]
            };
            for suffix in candidates {
                let p = dir.join(format!("{}{}", name, suffix));
                if p.exists() {
                    return p;
                }
            }
        }
    }
    std::path::PathBuf::from(if cfg!(windows) {
        format!("{}.exe", name)
    } else {
        name.to_string()
    })
}

pub fn ffprobe_path() -> std::path::PathBuf {
    sidecar_path("ffprobe")
}

pub fn ffmpeg_path() -> std::path::PathBuf {
    sidecar_path("ffmpeg")
}

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
