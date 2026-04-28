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
//! ## Lifecycle
//!
//! 1. `discover_binary` runs at startup and locates `go2rtc(.exe)` via
//!    `OXDM_GO2RTC` / sibling-of-oxdm / `$PATH`.
//! 2. First [`open`] writes a minimal `go2rtc.yaml` (loopback-only API
//!    listener) and spawns the child. We poll `/api` for ~5 s until the
//!    server answers.
//! 3. Each [`open`] resolves the device's RTSP URI via oxvif's
//!    `GetStreamUri`, splices credentials in, and `PUT /api/streams` to
//!    register it. The returned URL is the go2rtc shipped player page in
//!    WebRTC mode — codec negotiation and reconnect logic are go2rtc's
//!    problem, not ours.
//! 4. [`close`] sends `DELETE /api/streams`. Process is killed when the
//!    `Go2rtcBackend` value is dropped (or, in practice, on app exit).

use crate::state::Credentials;
use crate::video::{EmbedKind, VideoBackend, VideoSource};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;
use tokio::process::Command;
use tracing::{debug, info, warn};

/// Pinned go2rtc release we ship + test against. Bump when you've
/// validated a newer build end-to-end. Read from `AboutDialog` so
/// users can see exactly which build they have.
pub const BUNDLED_VERSION: &str = "1.9.14";

/// HTTP API host:port go2rtc binds to. Loopback-only — we never want
/// the camera streams reachable from the LAN.
const API_HOST: &str = "127.0.0.1";
const API_PORT: u16 = 1984;

/// Cap for the post-spawn readiness probe. go2rtc starts in well under
/// a second on every platform we've tested; a 5 s ceiling is generous
/// and still bounded enough to surface a hard failure to the user.
const READY_PROBE_TIMEOUT: Duration = Duration::from_secs(5);
const READY_PROBE_INTERVAL: Duration = Duration::from_millis(100);

pub struct Go2rtcBackend {
    /// Resolved path to the go2rtc binary, if discovery succeeded at startup.
    /// `None` means [`open`] fails fast with a helpful message — keeps
    /// errors contained to the RTSP tab instead of breaking app start.
    binary: Option<PathBuf>,
    /// Subprocess handle once spawned. Lazily populated on first successful
    /// [`open`]. Mutex (not RwLock) because `Child` is mutated to wait on,
    /// and there's only ever one writer at a time anyway.
    child: Mutex<Option<tokio::process::Child>>,
    /// HTTP client reused across stream registrations. reqwest pools
    /// connections, so this avoids one TCP handshake per add/remove.
    http: reqwest::Client,
}

impl Go2rtcBackend {
    /// Search standard locations for the go2rtc binary. Doesn't spawn
    /// anything — the subprocess starts lazily on first stream open so
    /// users who never touch the RTSP tab never pay any cost.
    ///
    /// Search order:
    /// 1. `OXDM_GO2RTC` environment variable (explicit override / dev)
    /// 2. Alongside the running oxdm executable (release-zip layout)
    /// 3. `$PATH` lookup of `go2rtc` / `go2rtc.exe`
    pub fn new() -> Self {
        let binary = discover_binary();
        if binary.is_none() {
            warn!(
                "go2rtc binary not found — RTSP backend will fail-fast on use. \
                 Set OXDM_GO2RTC, drop go2rtc{ext} next to oxdm{ext}, \
                 or install go2rtc on PATH.",
                ext = if cfg!(windows) { ".exe" } else { "" }
            );
        }
        Self {
            binary,
            child: Mutex::new(None),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .expect("reqwest client"),
        }
    }

    /// Make sure the child process is running and the API responds.
    /// Idempotent — subsequent calls after a successful spawn are a
    /// trivial liveness check on the in-memory handle.
    async fn ensure_running(&self, binary: &Path) -> Result<(), String> {
        // Fast path: child still tracked, assume alive. If it crashed
        // silently the next API call surfaces a network error and the
        // user retries, which respawns. Cheaper than try_wait on every
        // open().
        if self.child.lock().unwrap().is_some() {
            return Ok(());
        }

        let config_path = write_config_file()?;
        info!(binary = %binary.display(), config = %config_path.display(), "spawning go2rtc");

        let mut cmd = Command::new(binary);
        cmd.arg("-config")
            .arg(&config_path)
            // Detach stdio — go2rtc's own logging is verbose and noisy
            // for end users. If diagnostics are needed, set OXDM_GO2RTC
            // to a wrapper script that tees output.
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .kill_on_drop(true);

        // Hide the console window on Windows. Without this, spawning a
        // console exe (go2rtc is built without /SUBSYSTEM:WINDOWS)
        // pops a cmd window even when stdio is redirected, which looks
        // like a "black flash" to the user every time they open the
        // RTSP tab.
        // tokio::process::Command exposes `creation_flags` directly on
        // Windows — no trait import needed (unlike std::process::Command).
        #[cfg(windows)]
        {
            const CREATE_NO_WINDOW: u32 = 0x0800_0000;
            cmd.creation_flags(CREATE_NO_WINDOW);
        }

        let child = cmd.spawn().map_err(|e| format!("spawn go2rtc: {e}"))?;

        *self.child.lock().unwrap() = Some(child);

        // Poll until the API answers. Without this race, the first
        // PUT /api/streams can land before the listener is up.
        self.wait_until_ready().await
    }

    async fn wait_until_ready(&self) -> Result<(), String> {
        let deadline = std::time::Instant::now() + READY_PROBE_TIMEOUT;
        let probe_url = format!("http://{API_HOST}:{API_PORT}/api/streams");
        loop {
            if let Ok(resp) = self.http.get(&probe_url).send().await {
                if resp.status().is_success() {
                    debug!("go2rtc ready");
                    return Ok(());
                }
            }
            if std::time::Instant::now() >= deadline {
                return Err(format!(
                    "go2rtc API didn't respond within {:?}",
                    READY_PROBE_TIMEOUT
                ));
            }
            tokio::time::sleep(READY_PROBE_INTERVAL).await;
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
        device_addr: &str,
        profile_token: &str,
        creds: &Credentials,
    ) -> Result<VideoSource, String> {
        let binary = self.binary.as_ref().ok_or_else(|| {
            "go2rtc binary not found. Drop go2rtc(.exe) next to oxdm(.exe), \
             set OXDM_GO2RTC=/path/to/go2rtc, or install on PATH."
                .to_string()
        })?;

        self.ensure_running(binary).await?;

        // Resolve the RTSP URI via ONVIF GetStreamUri. The device is
        // free to return a URL with or without an embedded credential
        // pair; either way we splice in our own to make sure go2rtc's
        // RTSP client authenticates correctly. Some devices return
        // localhost-style URIs (`rtsp://0.0.0.0/stream`) which go2rtc
        // won't connect to — `inject_credentials` also rewrites the
        // host to the device address we already know works.
        let (u, p) = creds.as_options();
        let stream = crate::api::get_stream_uri(device_addr, u, p, profile_token)
            .await
            .map_err(|e| format!("GetStreamUri: {e}"))?;
        let rtsp_url =
            inject_credentials(&stream.uri, device_addr, &creds.username, &creds.password);

        let stream_name = stream_name_for(device_addr, profile_token);

        // Always re-PUT in case the camera URL or credentials changed
        // since the last open. go2rtc treats PUT as upsert, so this is
        // safe (and idempotent for repeated tab switches).
        let put = format!(
            "http://{API_HOST}:{API_PORT}/api/streams?name={}&src={}",
            urlencode(&stream_name),
            urlencode(&rtsp_url)
        );
        let resp = self
            .http
            .put(&put)
            .send()
            .await
            .map_err(|e| format!("register stream: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("register stream HTTP {}", resp.status()));
        }

        // mode=webrtc preferred — sub-second latency, codec negotiation
        // handled by browser. go2rtc transparently transcodes H.265 →
        // H.264 when the WebView refuses HEVC, at the cost of CPU.
        let url = format!(
            "http://{API_HOST}:{API_PORT}/stream.html?src={}&mode=webrtc",
            urlencode(&stream_name)
        );

        Ok(VideoSource {
            id: stream_name,
            url,
            embed: EmbedKind::Iframe,
        })
    }

    async fn close(&self, source_id: &str) {
        let url = format!(
            "http://{API_HOST}:{API_PORT}/api/streams?src={}",
            urlencode(source_id)
        );
        if let Err(e) = self.http.delete(&url).send().await {
            debug!(error = %e, source_id, "best-effort DELETE /api/streams failed");
        }
    }
}

// ── helpers ──────────────────────────────────────────────────────────────────

fn discover_binary() -> Option<PathBuf> {
    let bin_name = if cfg!(windows) {
        "go2rtc.exe"
    } else {
        "go2rtc"
    };

    if let Ok(path) = std::env::var("OXDM_GO2RTC") {
        let p = PathBuf::from(path);
        if p.is_file() {
            return Some(p);
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let candidate = dir.join(bin_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

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

/// Write a minimal config file to `~/.oxdm/go2rtc.yaml` (or a fallback
/// temp dir if home isn't available). We pin the API to loopback
/// because the default `:1984` binds to all interfaces, which would
/// expose the camera streams to the LAN — a real surprise for users
/// who think they're running a "local-only" tool.
fn write_config_file() -> Result<PathBuf, String> {
    let dir = dirs::home_dir()
        .map(|h| h.join(".oxdm"))
        .unwrap_or_else(std::env::temp_dir);
    std::fs::create_dir_all(&dir).map_err(|e| format!("mkdir {}: {e}", dir.display()))?;
    let path = dir.join("go2rtc.yaml");
    let yaml = format!("api:\n  listen: \"{API_HOST}:{API_PORT}\"\nlog:\n  level: warn\n");
    std::fs::write(&path, yaml).map_err(|e| format!("write config: {e}"))?;
    Ok(path)
}

/// Build a stream name for go2rtc. Must be URL-safe and stable across
/// re-opens (so PUT idempotency works) but unique across (device,
/// profile) pairs. Hash collision risk is irrelevant at this scale.
fn stream_name_for(device_addr: &str, profile_token: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    device_addr.hash(&mut h);
    profile_token.hash(&mut h);
    format!("oxdm-{:016x}", h.finish())
}

/// Splice credentials into an RTSP URL. Also rewrites the host to
/// `device_addr`'s host:port pair so cameras that return their own
/// internal hostname (or `0.0.0.0`) still work — we already know which
/// IP responded to ONVIF, so trust that over whatever the camera says.
///
/// `device_addr` may be the full ONVIF service URL (`http://1.2.3.4/onvif/...`)
/// or a bare host. We extract just the host:port piece.
fn inject_credentials(rtsp_uri: &str, device_addr: &str, user: &str, pass: &str) -> String {
    // Strip scheme.
    let after_scheme = rtsp_uri.strip_prefix("rtsp://").unwrap_or(rtsp_uri);

    // If the camera embedded its own creds, drop them — ours win.
    let after_creds = match after_scheme.find('@') {
        Some(i) => &after_scheme[i + 1..],
        None => after_scheme,
    };

    // Split host[:port] from path.
    let (orig_host_port, path_with_query) = match after_creds.find('/') {
        Some(i) => (&after_creds[..i], &after_creds[i..]),
        None => (after_creds, ""),
    };

    // Pull the host (and any port) from device_addr.
    let device_host_port =
        host_port_from_addr(device_addr).unwrap_or_else(|| orig_host_port.to_string());

    // We keep the camera's port if it set one explicitly (rtsp port may
    // differ from the ONVIF http port — usually 554 vs 80), but only if
    // device_addr didn't already specify one.
    let final_host_port = if device_host_port.contains(':') {
        device_host_port
    } else if let Some((_h, port)) = orig_host_port.rsplit_once(':') {
        format!("{device_host_port}:{port}")
    } else {
        device_host_port
    };

    let cred_prefix = if user.is_empty() && pass.is_empty() {
        String::new()
    } else {
        format!("{}:{}@", urlencode(user), urlencode(pass))
    };

    format!("rtsp://{cred_prefix}{final_host_port}{path_with_query}")
}

/// Extract `host` or `host:port` from various address shapes — bare
/// host, `host:port`, `http://host/path`, etc.
fn host_port_from_addr(addr: &str) -> Option<String> {
    let stripped = addr
        .strip_prefix("http://")
        .or_else(|| addr.strip_prefix("https://"))
        .unwrap_or(addr);
    let host_port = stripped.split('/').next()?;
    if host_port.is_empty() {
        None
    } else {
        Some(host_port.to_string())
    }
}

/// Minimal percent-encoder for URL query / path components. We can't
/// pull in a heavy URL crate just for this, and the inputs are
/// constrained (usernames, passwords, RTSP URLs) so the
/// allow-set-then-encode-everything-else approach is fine.
fn urlencode(s: &str) -> String {
    const KEEP: &[u8] = b"-_.~";
    let mut out = String::with_capacity(s.len());
    for b in s.as_bytes() {
        let c = *b;
        if c.is_ascii_alphanumeric() || KEEP.contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{c:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn injects_creds_and_keeps_camera_path() {
        let url = inject_credentials(
            "rtsp://192.168.1.10/Streaming/Channels/101",
            "192.168.1.10",
            "admin",
            "secret",
        );
        assert_eq!(
            url,
            "rtsp://admin:secret@192.168.1.10/Streaming/Channels/101"
        );
    }

    #[test]
    fn drops_camera_supplied_creds() {
        let url = inject_credentials(
            "rtsp://other:wrong@cam.local/s1",
            "192.168.1.10",
            "admin",
            "secret",
        );
        assert_eq!(url, "rtsp://admin:secret@192.168.1.10/s1");
    }

    #[test]
    fn keeps_rtsp_port_when_device_addr_has_none() {
        let url = inject_credentials(
            "rtsp://192.168.1.10:554/s",
            "http://192.168.1.10/onvif/device",
            "u",
            "p",
        );
        assert_eq!(url, "rtsp://u:p@192.168.1.10:554/s");
    }

    #[test]
    fn url_encodes_special_chars_in_password() {
        let url = inject_credentials("rtsp://192.168.1.10/s", "192.168.1.10", "user", "p@ss/word");
        assert!(url.contains("p%40ss%2Fword@"));
    }

    #[test]
    fn stream_name_stable() {
        let a = stream_name_for("192.168.1.10", "Profile_1");
        let b = stream_name_for("192.168.1.10", "Profile_1");
        assert_eq!(a, b);
        assert!(a.starts_with("oxdm-"));
    }

    #[test]
    fn stream_name_differs_per_profile() {
        let a = stream_name_for("192.168.1.10", "Profile_1");
        let b = stream_name_for("192.168.1.10", "Profile_2");
        assert_ne!(a, b);
    }
}
