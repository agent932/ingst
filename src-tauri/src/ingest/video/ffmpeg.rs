//! Video backend backed by the bundled `ffprobe` and `ffmpeg` sidecars.
//!
//! Portable, and the only option on Linux, but it is also the source of this
//! project's packaging problems — see the module docs on the parent. Prefer a
//! native backend where one exists.

use super::{VideoBackend, VideoInfo};
use std::path::Path;
use std::process::Command;

pub struct Ffmpeg;

impl VideoBackend for Ffmpeg {
    fn name(&self) -> &'static str {
        "ffmpeg-sidecar"
    }

    fn is_available(&self) -> bool {
        // Both are needed: ffprobe for metadata, ffmpeg for frames. Reporting
        // available when only one works would hide half the functionality.
        crate::utils::paths::sidecar_works("ffprobe")
            && crate::utils::paths::sidecar_works("ffmpeg")
    }

    fn info(&self, path: &Path) -> Option<VideoInfo> {
        let output = Command::new(crate::utils::paths::ffprobe_path())
            .arg("-v")
            .arg("quiet")
            .arg("-print_format")
            .arg("json")
            .arg("-show_format")
            .arg(path)
            .output()
            .ok()?;

        let json: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
        let tags = json.get("format").and_then(|f| f.get("tags"));

        let capture_date = tags
            .and_then(|t| t.get("creation_time"))
            .and_then(|v| v.as_str())
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(crate::ingest::metadata::normalize_to_local);

        // QuickTime metadata keys are `com.apple.quicktime.*`. Cameras that
        // write plain MP4 tags (DJI, GoPro, Android) use the bare keys.
        let device_name = tags
            .and_then(|t| {
                t.get("com.apple.quicktime.model")
                    .or_else(|| t.get("com.apple.quicktime.make"))
                    .or_else(|| t.get("model"))
                    .or_else(|| t.get("make"))
                    .or_else(|| t.get("device_model"))
            })
            .and_then(|v| v.as_str())
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty());

        let info = VideoInfo {
            capture_date,
            device_name,
        };

        if info.is_empty() {
            None
        } else {
            Some(info)
        }
    }

    fn thumbnail(&self, path: &Path, max_dim: u32) -> Option<Vec<u8>> {
        // Seek a second in before opening, so ffmpeg jumps to a keyframe rather
        // than decoding from the start; a clip shorter than that produces no
        // output and falls back to an icon. `-2` keeps the height even, which
        // the JPEG encoder requires.
        let scale = format!("scale={}:-2", max_dim);

        let output = Command::new(crate::utils::paths::ffmpeg_path())
            .args(["-ss", "1", "-i"])
            .arg(path)
            .args([
                "-vframes",
                "1",
                "-vf",
                &scale,
                "-f",
                "mjpeg",
                "-loglevel",
                "error",
                "pipe:1",
            ])
            .output()
            .ok()?;

        if output.stdout.is_empty() {
            None
        } else {
            Some(output.stdout)
        }
    }
}
