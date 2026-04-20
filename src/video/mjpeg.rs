//! MJPEG snapshot-loop backend.
//!
//! Runs a tiny HTTP/1.1 server on `127.0.0.1` (auto-picked port). For each
//! `GET /stream/{id}` it walks an indefinite loop:
//!
//! 1. Look up `(addr, profile_token, creds)` for the stream id.
//! 2. Resolve the camera's snapshot URI via ONVIF `GetSnapshotUri`.
//! 3. Fetch the JPEG with whatever auth the camera demands (delegated to
//!    [`crate::api::fetch_snapshot_data_uri`], which already handles Digest
//!    auth + the raw-TCP fallback for picky vendors).
//! 4. Write the JPEG as a `multipart/x-mixed-replace` part.
//! 5. Sleep `frame_interval`, repeat.
//!
//! Browsers (and WebView2 / WKWebView / WebKitGTK) all natively render this
//! into an `<img>` tag with no JS or polyfill needed — it's the same
//! technology mid-2000s IP-cam web UIs used.
//!
//! No new third-party dependency: the HTTP server is hand-rolled over a
//! `tokio::net::TcpListener`, mirroring the raw-TCP pattern already used
//! for the snapshot Digest fallback in [`crate::api`].

use crate::api;
use crate::state::Credentials;
use crate::video::{EmbedKind, VideoBackend, VideoSource};
use base64::Engine;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::sync::RwLock;
use tracing::{debug, error, info, warn};

const MULTIPART_BOUNDARY: &str = "oxdm-mjpeg-frame";
/// Default polling interval between snapshot pulls. Cameras vary in how
/// fast they can serve `GetSnapshotUri` → 200 ms (5 fps) is a safe ceiling
/// that doesn't trip Hanwha-style brute-force lockouts.
const DEFAULT_FRAME_INTERVAL: Duration = Duration::from_millis(200);

/// Per-stream state held by the backend: enough to pull snapshots on demand.
#[derive(Clone)]
struct StreamMeta {
    device_addr: String,
    profile_token: String,
    creds: Credentials,
}

#[derive(Default)]
struct Inner {
    streams: HashMap<String, StreamMeta>,
}

pub struct MjpegBackend {
    port: u16,
    inner: Arc<RwLock<Inner>>,
}

impl MjpegBackend {
    /// Bind a loopback listener and spawn the accept loop.
    ///
    /// Must be called from inside a tokio runtime (Dioxus sets one up
    /// before `App()` runs, so calling from a `use_hook` is fine).
    /// Returns `Err` only if binding the loopback port fails.
    pub fn start() -> Result<Self, String> {
        // Bind via std then convert — tokio::net::TcpListener::bind would
        // require us to be inside a runtime *and* use .await; doing it
        // synchronously lets `start()` be called from a sync `use_hook`.
        let listener = std::net::TcpListener::bind("127.0.0.1:0")
            .map_err(|e| format!("MJPEG: bind 127.0.0.1:0 failed: {e}"))?;
        listener
            .set_nonblocking(true)
            .map_err(|e| format!("MJPEG: set_nonblocking failed: {e}"))?;
        let port = listener
            .local_addr()
            .map_err(|e| format!("MJPEG: local_addr failed: {e}"))?
            .port();
        let listener = tokio::net::TcpListener::from_std(listener)
            .map_err(|e| format!("MJPEG: tokio::TcpListener::from_std failed: {e}"))?;

        let inner = Arc::new(RwLock::new(Inner::default()));
        let inner_for_loop = Arc::clone(&inner);
        let frame_interval = DEFAULT_FRAME_INTERVAL;

        tokio::spawn(async move {
            info!(port, "MJPEG backend listening on 127.0.0.1");
            loop {
                match listener.accept().await {
                    Ok((sock, peer)) => {
                        let inner = Arc::clone(&inner_for_loop);
                        tokio::spawn(async move {
                            if let Err(e) = handle_connection(sock, inner, frame_interval).await {
                                debug!(?peer, error = %e, "MJPEG connection ended");
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "MJPEG accept failed");
                        // Don't busy-loop on permanent failure
                        tokio::time::sleep(Duration::from_secs(1)).await;
                    }
                }
            }
        });

        Ok(Self { port, inner })
    }
}

#[async_trait::async_trait]
impl VideoBackend for MjpegBackend {
    fn id(&self) -> &'static str {
        "mjpeg"
    }
    fn display_name(&self) -> &'static str {
        "MJPEG snapshot stream"
    }

    async fn is_available(&self) -> bool {
        true
    }

    async fn open(
        &self,
        device_addr: &str,
        profile_token: &str,
        creds: &Credentials,
    ) -> Result<VideoSource, String> {
        // Stream id is `{addr}::{profile_token}`. Re-opening the same pair
        // is a cheap dedupe; the connection handler always reads the most
        // recent meta from `inner` so credential changes propagate next frame.
        let id = format!("{device_addr}::{profile_token}");
        let meta = StreamMeta {
            device_addr: device_addr.to_string(),
            profile_token: profile_token.to_string(),
            creds: creds.clone(),
        };
        self.inner.write().await.streams.insert(id.clone(), meta);

        let url = format!(
            "http://127.0.0.1:{port}/stream/{id_enc}",
            port = self.port,
            id_enc = url_encode(&id),
        );
        debug!(stream_id = %id, %url, "MJPEG stream opened");
        Ok(VideoSource {
            id,
            url,
            embed: EmbedKind::Img,
        })
    }

    async fn close(&self, source_id: &str) {
        self.inner.write().await.streams.remove(source_id);
        debug!(stream_id = %source_id, "MJPEG stream closed");
    }
}

// ── HTTP/multipart machinery ─────────────────────────────────────────────────

async fn handle_connection(
    mut sock: TcpStream,
    inner: Arc<RwLock<Inner>>,
    frame_interval: Duration,
) -> Result<(), String> {
    // Read just enough to capture the request line + headers.
    let mut buf = Vec::with_capacity(2048);
    let mut tmp = [0u8; 1024];
    let header_end = loop {
        let n = sock
            .read(&mut tmp)
            .await
            .map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("client closed before request".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
        if buf.len() > 16 * 1024 {
            return Err("request headers too large".into());
        }
    };

    let head = std::str::from_utf8(&buf[..header_end]).map_err(|_| "non-utf8 headers")?;
    let request_line = head.lines().next().ok_or("empty request")?;
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let path = parts.next().unwrap_or("");

    if method != "GET" {
        write_text(
            &mut sock,
            405,
            "Method Not Allowed",
            "MJPEG only accepts GET",
        )
        .await?;
        return Ok(());
    }
    let Some(stream_id_enc) = path.strip_prefix("/stream/") else {
        write_text(&mut sock, 404, "Not Found", "Unknown route").await?;
        return Ok(());
    };
    let stream_id = url_decode(stream_id_enc);

    if !inner.read().await.streams.contains_key(&stream_id) {
        write_text(&mut sock, 404, "Not Found", "Unknown stream id").await?;
        return Ok(());
    }

    // Multipart response — never closes until peer disconnects or we error.
    let head = format!(
        "HTTP/1.1 200 OK\r\n\
         Content-Type: multipart/x-mixed-replace; boundary={MULTIPART_BOUNDARY}\r\n\
         Cache-Control: no-cache, no-store, must-revalidate\r\n\
         Pragma: no-cache\r\n\
         Connection: close\r\n\
         Access-Control-Allow-Origin: *\r\n\
         \r\n"
    );
    sock.write_all(head.as_bytes())
        .await
        .map_err(|e| format!("write head: {e}"))?;

    info!(stream_id = %stream_id, "MJPEG stream started");

    loop {
        // Re-read the meta on every frame so credential / token swaps take
        // effect mid-stream without disconnecting.
        let meta = match inner.read().await.streams.get(&stream_id).cloned() {
            Some(m) => m,
            None => {
                debug!(stream_id = %stream_id, "stream removed; closing");
                break;
            }
        };

        match fetch_jpeg(&meta).await {
            Ok(bytes) => {
                let part = format!(
                    "--{MULTIPART_BOUNDARY}\r\n\
                     Content-Type: image/jpeg\r\n\
                     Content-Length: {}\r\n\
                     \r\n",
                    bytes.len()
                );
                sock.write_all(part.as_bytes())
                    .await
                    .map_err(|e| format!("write part header: {e}"))?;
                sock.write_all(&bytes)
                    .await
                    .map_err(|e| format!("write jpeg: {e}"))?;
                sock.write_all(b"\r\n")
                    .await
                    .map_err(|e| format!("write trailer: {e}"))?;
            }
            Err(e) => {
                warn!(stream_id = %stream_id, error = %e, "snapshot fetch failed; pausing");
                // Slower retry on failure so we don't spam a flaky camera.
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        }
        tokio::time::sleep(frame_interval).await;
    }
    Ok(())
}

/// Pull a single JPEG for the meta. Re-resolves the snapshot URI each call —
/// some cameras (GeoVision) embed a per-call timestamp and TTL into the URL,
/// so caching across frames isn't safe.
async fn fetch_jpeg(meta: &StreamMeta) -> Result<Vec<u8>, String> {
    let (u, p) = meta.creds.as_options();
    let snap = api::get_snapshot_uri(&meta.device_addr, u, p, &meta.profile_token).await?;
    let snapshot_url = api::resolve_snapshot_url(&meta.device_addr, &snap.uri);
    let data_uri = api::fetch_snapshot_data_uri(&snapshot_url, u, p).await?;
    data_uri_to_bytes(&data_uri).ok_or_else(|| "malformed data URI from snapshot".to_string())
}

/// Decode the base64 payload of a `data:image/...;base64,…` URI.
fn data_uri_to_bytes(uri: &str) -> Option<Vec<u8>> {
    let comma = uri.find(',')?;
    let b64 = &uri[comma + 1..];
    base64::engine::general_purpose::STANDARD.decode(b64).ok()
}

async fn write_text(
    sock: &mut TcpStream,
    code: u16,
    reason: &str,
    body: &str,
) -> Result<(), String> {
    let resp = format!(
        "HTTP/1.1 {code} {reason}\r\n\
         Content-Type: text/plain; charset=utf-8\r\n\
         Content-Length: {len}\r\n\
         Connection: close\r\n\
         \r\n\
         {body}",
        len = body.len(),
    );
    sock.write_all(resp.as_bytes())
        .await
        .map_err(|e| format!("write_text: {e}"))
}

/// Minimal percent-encode for stream ids in the URL path. We only need to
/// escape characters that would break `GET /stream/...`: anything outside
/// unreserved ASCII gets `%XX`.
fn url_encode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for &b in s.as_bytes() {
        let safe = b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'~');
        if safe {
            out.push(b as char);
        } else {
            out.push('%');
            out.push_str(&format!("{b:02X}"));
        }
    }
    out
}

fn url_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let hi = hex_digit(bytes[i + 1]);
            let lo = hex_digit(bytes[i + 2]);
            if let (Some(h), Some(l)) = (hi, lo) {
                out.push((h << 4) | l);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8(out).unwrap_or_default()
}

fn hex_digit(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(10 + b - b'a'),
        b'A'..=b'F' => Some(10 + b - b'A'),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_encode_round_trips_simple() {
        assert_eq!(url_decode(&url_encode("simple-id_123")), "simple-id_123");
    }

    #[test]
    fn url_encode_handles_special_chars() {
        let s = "http://192.168.1.1/onvif/device_service::profile_1";
        let enc = url_encode(s);
        // `:`, `/` get percent-escaped.
        assert!(enc.contains("%3A"), "expected colon escape: {enc}");
        assert!(enc.contains("%2F"), "expected slash escape: {enc}");
        assert_eq!(url_decode(&enc), s);
    }

    #[test]
    fn data_uri_extracts_jpeg_bytes() {
        let bytes = data_uri_to_bytes("data:image/jpeg;base64,SGVsbG8=").unwrap();
        assert_eq!(bytes, b"Hello");
    }

    #[test]
    fn data_uri_handles_missing_comma() {
        assert!(data_uri_to_bytes("not a data uri").is_none());
    }

    // base_url_from_device_addr / resolve_snapshot_url moved to api.rs;
    // their unit tests live there now.
}
