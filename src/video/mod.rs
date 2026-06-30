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

    /// Play an already-resolved RTSP URI (e.g. an ONVIF replay URI from
    /// `GetReplayUri`) rather than resolving a live profile stream.
    /// `device_addr` is used only as a host fallback when the URI reports
    /// an unreachable host (`0.0.0.0` / loopback). Returns an embeddable URL.
    ///
    /// Default: unsupported — only the go2rtc bridge can do RTSP. The MJPEG
    /// snapshot backend has no way to play arbitrary RTSP, so it inherits
    /// this `Err`.
    async fn open_rtsp(
        &self,
        _rtsp_url: &str,
        _device_addr: &str,
        _creds: &Credentials,
    ) -> Result<VideoSource, String> {
        Err("this backend does not support RTSP playback".to_string())
    }

    /// Stop streaming the source previously returned by `open`. Idempotent.
    #[allow(dead_code)] // Reserved for view teardown / settings UI
    async fn close(&self, source_id: &str);
}

// ── Global registry ──────────────────────────────────────────────────────────
//
// Two named slots — Live Video can flip between them per-session via the
// view's tab strip, and the embedded preview in Imaging can pin to MJPEG
// (the small thumbnail doesn't need RTSP machinery). Reusing a single
// `current()` getter would force every consumer through one backend; the
// pair lets callers express intent at the call site instead.

static MJPEG: OnceLock<Arc<dyn VideoBackend>> = OnceLock::new();
static GO2RTC: OnceLock<Arc<dyn VideoBackend>> = OnceLock::new();

/// Install the lightweight snapshot-loop backend. Always available; this
/// is what `current()` returns by default and what the embedded preview
/// in Imaging pins to.
pub fn install_mjpeg(backend: Arc<dyn VideoBackend>) {
    let _ = MJPEG.set(backend);
}

/// Install the go2rtc bridge backend. Optional — depends on whether
/// `Go2rtcBackend::new` found the bundled binary. Live Video's RTSP tab
/// surfaces an inline error if this slot is empty at use time.
pub fn install_go2rtc(backend: Arc<dyn VideoBackend>) {
    let _ = GO2RTC.set(backend);
}

/// The default backend (MJPEG). Used by callers that don't care which
/// backend they get — `LiveVideoStage` defaults to this when its caller
/// doesn't pass an override.
pub fn current() -> Option<Arc<dyn VideoBackend>> {
    MJPEG.get().cloned()
}

pub fn mjpeg() -> Option<Arc<dyn VideoBackend>> {
    MJPEG.get().cloned()
}

pub fn go2rtc() -> Option<Arc<dyn VideoBackend>> {
    GO2RTC.get().cloned()
}
