//! Reading capture metadata and preview frames out of video files.
//!
//! Everything a video format needs from outside the app sits behind
//! [`VideoBackend`], so the implementation can differ per platform without the
//! scanner or the thumbnail command knowing.
//!
//! Today the only implementation shells out to bundled `ffprobe` and `ffmpeg`
//! sidecars. Those are the messiest part of the project: roughly 100 MB of
//! binaries, a licence that sits awkwardly with this app's, a supply-chain
//! decision about whose builds to trust, and a packaging bug that silently
//! shipped broken releases. macOS (AVFoundation) and Windows (Media Foundation)
//! can answer both questions natively, and this boundary is what lets those
//! land without touching callers.

use std::path::Path;

pub mod ffmpeg;

/// What a backend can tell us about a clip.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct VideoInfo {
    /// Capture time as local wall-clock, formatted `%Y-%m-%dT%H:%M:%S`.
    ///
    /// Local, not UTC: a shoot belongs to the day the person shooting it would
    /// name, and stills carry no timezone, so anything else splits one shoot
    /// across two folders at a month boundary.
    pub capture_date: Option<String>,
    /// Camera make and/or model, as written by the device.
    pub device_name: Option<String>,
}

impl VideoInfo {
    pub fn is_empty(&self) -> bool {
        self.capture_date.is_none() && self.device_name.is_none()
    }
}

pub trait VideoBackend: Send + Sync {
    /// Short identifier, used in logs so a support question can be answered
    /// with "which backend were you on".
    fn name(&self) -> &'static str;

    /// Whether this backend can run here. A bundled sidecar that is missing or
    /// cannot execute is unavailable exactly like an unsupported platform API.
    fn is_available(&self) -> bool;

    /// Capture metadata, or `None` if the file yielded nothing usable.
    fn info(&self, path: &Path) -> Option<VideoInfo>;

    /// JPEG bytes of a representative frame, scaled so its longest side is at
    /// most `max_dim`. `None` when no frame could be produced — a clip shorter
    /// than the seek point, a codec the backend cannot decode, or a missing
    /// sidecar. Callers fall back to a file-type icon.
    fn thumbnail(&self, path: &Path, max_dim: u32) -> Option<Vec<u8>>;
}

/// Backends in preference order for this platform.
///
/// Native APIs come first once they exist; the sidecar stays last as the
/// portable fallback, and remains the only option on Linux.
fn candidates() -> Vec<&'static dyn VideoBackend> {
    vec![&ffmpeg::Ffmpeg]
}

/// The backend in use, resolved once.
///
/// Availability is probed rather than assumed: a bundled binary can be present
/// but unrunnable, which is exactly the failure that shipped in past releases.
pub fn backend() -> &'static dyn VideoBackend {
    use std::sync::OnceLock;
    static SELECTED: OnceLock<&'static dyn VideoBackend> = OnceLock::new();

    *SELECTED.get_or_init(|| {
        let all = candidates();
        for candidate in &all {
            if candidate.is_available() {
                log::info!("video backend: {}", candidate.name());
                return *candidate;
            }
        }

        // Nothing usable. Return the last candidate anyway so callers keep a
        // uniform interface; every call will simply yield None, and video
        // capture dates, device names and thumbnails will be unavailable.
        log::warn!(
            "no video backend available (tried: {}) — video capture dates, \
             device names and thumbnails will be unavailable; photos are unaffected",
            all.iter().map(|c| c.name()).collect::<Vec<_>>().join(", ")
        );
        *all.last().expect("at least one backend is compiled in")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A backend must never panic or hang on a file that is not a video. The
    /// scanner hands it whatever the card contained, including truncated files
    /// and things mislabelled with a video extension.
    #[test]
    fn backend_returns_none_for_junk_input() {
        let dir = std::env::temp_dir().join("ingst_video_backend_junk");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let not_a_video = dir.join("truncated.mp4");
        std::fs::write(&not_a_video, b"this is not a video file").unwrap();

        let b = backend();
        assert!(b.info(&not_a_video).is_none() || b.info(&not_a_video).unwrap().is_empty());
        assert!(b.thumbnail(&not_a_video, 160).is_none());

        // A path that does not exist at all must be handled the same way.
        let missing = dir.join("nope.mp4");
        assert!(b.info(&missing).is_none() || b.info(&missing).unwrap().is_empty());
        assert!(b.thumbnail(&missing, 160).is_none());

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn selected_backend_is_stable() {
        assert_eq!(backend().name(), backend().name());
    }
}
