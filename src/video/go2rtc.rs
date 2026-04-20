//! go2rtc subprocess backend.
//!
//! [`go2rtc`](https://github.com/AlexxIT/go2rtc) is a small Go binary that
//! re-broadcasts RTSP / ONVIF streams as WebRTC, HLS, or fragmented MP4 —
//! all of which a WebView2/WKWebView/WebKitGTK can consume natively. Pair
//! that with full FPS, audio, and (when the source is H.265) automatic
//! transcoding into something the WebView's media engine actually
//! understands, and you get a "real" Live Video experience without us
//! having to write a decoder or renderer.
//!
//! ## Status
//!
//! Skeleton only. Binary discovery and subprocess lifecycle are wired but
//! [`Go2rtcBackend::open`] currently returns an error explaining what's
//! missing — the stream-registration HTTP calls and ready-probe loop are
//! still TODO. The trait surface matches [`super::mjpeg::MjpegBackend`] so
//! a future commit can drop in the real implementation without touching
//! `LiveVideoView`.
//!
//! ## How a finished implementation will work
//!
//! 1. On first [`open`], lazily spawn `go2rtc` as a child process (port
//!    1984 by default), capture stdout/stderr to tracing.
//! 2. Poll `GET http://127.0.0.1:1984/api` until it responds 200 — proves
//!    the bridge is up.
//! 3. `POST /api/streams?name={id}&src=rtsp://user:pw@addr/...` to
//!    register the camera. The RTSP URL comes from oxvif's
//!    `GetStreamUri` plus the per-device credentials.
//! 4. Hand back `http://127.0.0.1:1984/stream.html?src={id}` as an
//!    [`super::EmbedKind::Iframe`] source — go2rtc ships a working HTML
//!    player.
//! 5. On [`close`], `DELETE /api/streams?src={id}`. On app shutdown,
//!    kill the child.

use crate::state::Credentials;
use crate::video::{VideoBackend, VideoSource};
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::warn;

/// Default HTTP API port go2rtc binds to. Configurable via go2rtc's own
/// `--api` flag if the default ever clashes; we follow upstream's default
/// here so an unsupplied binary works on first run.
#[allow(dead_code)]
const DEFAULT_API_PORT: u16 = 1984;

#[allow(dead_code)] // Skeleton — wired for future settings-UI selection
pub struct Go2rtcBackend {
    /// Resolved path to the go2rtc binary, if discovery succeeded at startup.
    /// `None` means we'll fail-fast on `open` with a helpful message.
    binary: Option<PathBuf>,
    /// Subprocess handle once spawned. Lazily populated on first successful
    /// `open`. Wrapped in a Mutex (not RwLock) because borrowing `Child`
    /// mutably is the common case.
    child: Mutex<Option<tokio::process::Child>>,
}

#[allow(dead_code)] // Skeleton — installed via future settings UI
impl Go2rtcBackend {
    /// Search the standard locations for the go2rtc binary. Doesn't spawn
    /// anything yet — the subprocess starts lazily on first stream open.
    ///
    /// Search order:
    /// 1. `OXDM_GO2RTC` environment variable (explicit override)
    /// 2. Alongside the running oxdm executable (e.g. installer bundles it)
    /// 3. `$PATH` lookup of `go2rtc` / `go2rtc.exe`
    pub fn new() -> Self {
        let binary = discover_binary();
        if binary.is_none() {
            warn!(
                "go2rtc binary not found — set OXDM_GO2RTC, drop go2rtc{ext} \
                 next to oxdm{ext}, or install go2rtc on PATH",
                ext = if cfg!(windows) { ".exe" } else { "" }
            );
        }
        Self {
            binary,
            child: Mutex::new(None),
        }
    }
}

#[async_trait::async_trait]
impl VideoBackend for Go2rtcBackend {
    fn id(&self) -> &'static str {
        "go2rtc"
    }
    fn display_name(&self) -> &'static str {
        "go2rtc bridge"
    }

    async fn is_available(&self) -> bool {
        self.binary.is_some()
    }

    async fn open(
        &self,
        _device_addr: &str,
        _profile_token: &str,
        _creds: &Credentials,
    ) -> Result<VideoSource, String> {
        let _binary = self.binary.as_ref().ok_or_else(|| {
            "go2rtc binary not configured (set OXDM_GO2RTC or install on PATH)".to_string()
        })?;

        // TODO:
        //   1. Lazy spawn child if `self.child` is None — store handle.
        //   2. Poll http://127.0.0.1:1984/api until ready (timeout ~5s).
        //   3. Resolve the RTSP URI via oxvif `GetStreamUri` and inject
        //      credentials inline (`rtsp://user:pw@host/...`).
        //   4. POST /api/streams?name=<id>&src=<rtsp_url>.
        //   5. Return VideoSource { url: ".../stream.html?src=<id>",
        //                            embed: EmbedKind::Iframe }.
        Err("go2rtc backend not yet implemented — use the mjpeg backend".to_string())
    }

    async fn close(&self, _source_id: &str) {
        // TODO: DELETE /api/streams?src={id}
    }
}

#[allow(dead_code)] // Used by Go2rtcBackend::new (also currently dead)
fn discover_binary() -> Option<PathBuf> {
    let bin_name = if cfg!(windows) {
        "go2rtc.exe"
    } else {
        "go2rtc"
    };

    // 1. Explicit override.
    if let Ok(path) = std::env::var("OXDM_GO2RTC") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    // 2. Alongside oxdm executable.
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    // 3. Walk PATH.
    if let Ok(path) = std::env::var("PATH") {
        for dir in std::env::split_paths(&path) {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    None
}
