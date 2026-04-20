//! Live video backends.
//!
//! `LiveVideoView` calls into an opaque [`VideoBackend`] trait that knows how
//! to take a device's RTSP/snapshot source and produce a URL the WebView can
//! embed. Two implementations ship today — pick at app startup, swap freely:
//!
//! * [`mjpeg`] — pure-Rust HTTP server on `127.0.0.1` that polls
//!   `GetSnapshotUri` for each registered stream and pushes JPEG frames out
//!   as `multipart/x-mixed-replace`. ~5–10 fps, no codec issues, zero new
//!   runtime deps. The default backend.
//! * [`go2rtc`] — spawns the [go2rtc](https://github.com/AlexxIT/go2rtc)
//!   helper binary and returns its built-in player URL. Real RTSP-grade fps,
//!   H.265 transcoded to whatever the WebView supports. Currently a skeleton
//!   awaiting full subprocess wiring.
//!
//! UI code never sees the implementation — it just calls
//! [`current().open()`] and renders the returned [`VideoSource`] according to
//! its [`EmbedKind`].

use crate::state::Credentials;
use std::sync::{Arc, OnceLock};

pub mod go2rtc;
pub mod mjpeg;

/// How a frontend should embed a [`VideoSource::url`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EmbedKind {
    /// `<img src="…">` — multipart MJPEG, animated GIF, single image.
    Img,
    /// `<video src="…" autoplay>` — HLS, fragmented MP4, MPEG-DASH.
    #[allow(dead_code)]
    Video,
    /// `<iframe src="…">` — full HTML player page (e.g. go2rtc's UI).
    #[allow(dead_code)]
    Iframe,
}

/// A handle to a currently-streaming video source.
#[derive(Clone, Debug)]
pub struct VideoSource {
    /// Opaque identifier — pass back to [`VideoBackend::close`] to stop the
    /// stream. Currently unused on the UI side because `LiveVideoView`
    /// holds the source for the lifetime of the view.
    #[allow(dead_code)]
    pub id: String,
    /// URL the frontend embeds. Always served from `127.0.0.1` (or the
    /// equivalent loopback interface). Backends are responsible for picking
    /// a port and avoiding collisions.
    pub url: String,
    /// Which HTML element to render the URL with.
    pub embed: EmbedKind,
}

/// Pluggable video transport.
///
/// Implementations bridge an ONVIF device into something a WebView can render.
/// They run for the lifetime of the app — `open` is cheap and idempotent
/// (re-opening the same `(addr, profile_token)` returns the same handle).
///
/// `id`, `is_available` and `close` aren't called yet by user-facing code
/// but exist for the planned settings UI (toggle backend, surface status,
/// release streams when leaving Live Video).
#[async_trait::async_trait]
pub trait VideoBackend: Send + Sync {
    /// Stable short identifier used in logs and persisted settings.
    #[allow(dead_code)] // Reserved for future settings UI
    fn id(&self) -> &'static str;

    /// Human-readable name for UI display.
    fn display_name(&self) -> &'static str;

    /// Whether this backend can actually serve streams in the current
    /// environment (e.g. go2rtc binary present, port free, etc.).
    #[allow(dead_code)] // Reserved for future settings UI
    async fn is_available(&self) -> bool;

    /// Begin streaming `profile_token` from `device_addr`. Returns an
    /// embeddable URL.
    async fn open(
        &self,
        device_addr: &str,
        profile_token: &str,
        creds: &Credentials,
    ) -> Result<VideoSource, String>;

    /// Stop streaming the source previously returned by `open`. Idempotent.
    #[allow(dead_code)] // Reserved for view teardown / settings UI
    async fn close(&self, source_id: &str);
}

// ── Global registry ──────────────────────────────────────────────────────────

static BACKEND: OnceLock<Arc<dyn VideoBackend>> = OnceLock::new();

/// Install the process-wide video backend. Call once at startup.
/// Subsequent calls are silently ignored (so tests that re-init the app
/// don't panic).
pub fn install(backend: Arc<dyn VideoBackend>) {
    let _ = BACKEND.set(backend);
}

/// Currently installed backend, if any. `None` before [`install`] is called
/// or in code paths (e.g. unit tests) that never wired one up.
pub fn current() -> Option<Arc<dyn VideoBackend>> {
    BACKEND.get().cloned()
}
