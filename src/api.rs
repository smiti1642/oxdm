//! Application-layer wrappers around `oxvif` for the Dioxus UI.
//!
//! Every wrapper here funnels through [`crate::sessions`], the
//! per-`(addr, creds)` cache of [`oxvif::OnvifSession`]. The session
//! caches `Capabilities` after `build()`, so once a device has been
//! touched in this process, every subsequent SOAP call is a single
//! round-trip (no `GetCapabilities` retry, no service-URL juggling
//! at the call site) and reuses the underlying `reqwest` connection
//! pool.
//!
//! Only two functions in this file *don't* use `OnvifSession`:
//! [`fetch_snapshot_data_uri`] (raw HTTP + custom Digest, because
//! Hikvision/Uniview snapshot endpoints reject anything that isn't
//! byte-identical to curl) and [`discover_one_round`] (WS-Discovery
//! is its own protocol).
//!
//! Error policy: every fallible function returns
//! [`ApiError`]`= String`. The UI displays it as a toast; richer
//! error types would be wasted because Dioxus event handlers
//! ultimately render strings anyway.

use oxvif::{
    DeviceInfo, DiscoveredDevice, DnsInformation, EventProperties, FocusMove, Hostname,
    IpStackConfig, ManualAddress, MediaProfile, NetworkGateway, NetworkInterface,
    NetworkInterfaceConfig, NetworkProtocol, NotificationMessage, NtpInfo, OnvifSession,
    OsdConfiguration, OsdOptions, PtzPreset, PullPointSubscription, RelayOutput, SnapshotUri,
    StreamUri, SystemDateTime, User, VideoEncoderConfiguration, VideoEncoderConfiguration2,
    VideoEncoderConfigurationOptions, VideoEncoding, VideoRateControl2,
};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, instrument, trace, warn};

use crate::sessions;
use crate::state::Credentials;

pub type ApiError = String;

/// Process-wide TLS strictness for snapshot HTTPS calls. `false` (default)
/// preserves the legacy `danger_accept_invalid_certs(true)` behaviour
/// most cameras need; flipping to `true` makes the snapshot fetcher
/// refuse self-signed and expired certs. Driven from the About dialog
/// toggle via `set_tls_strict` and persisted in config.toml.
///
/// Stored as a static atomic so the synchronous reqwest builder doesn't
/// need to thread a Dioxus signal through every call site.
static TLS_STRICT: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_tls_strict(strict: bool) {
    TLS_STRICT.store(strict, std::sync::atomic::Ordering::Relaxed);
}

fn tls_strict() -> bool {
    TLS_STRICT.load(std::sync::atomic::Ordering::Relaxed)
}

/// Fetch (or build-and-cache) the `OnvifSession` for `(addr, creds)`,
/// converting `OnvifError` to the string-flavoured [`ApiError`] the
/// UI consumes.
///
/// First call for an `(addr, creds)` pair pays one
/// `GetCapabilities` round-trip; every subsequent call returns a
/// cheap `Arc` clone of the already-built session.
async fn session_for(addr: &str, creds: &Credentials) -> Result<Arc<OnvifSession>, ApiError> {
    let (u, p) = creds.as_options();
    sessions::get(addr, u, p).await.map_err(|e| e.to_string())
}

/// Log the result of an API call and convert error to String.
fn trace_result<T>(
    method: &str,
    addr: &str,
    result: Result<T, impl std::fmt::Display>,
) -> Result<T, ApiError> {
    match result {
        Ok(v) => {
            debug!(method, addr, "OK");
            Ok(v)
        }
        Err(e) => {
            error!(method, addr, error = %e, "FAIL");
            Err(e.to_string())
        }
    }
}

// ── Discovery ───────────────────────────────────────────────────────────────

/// Run a single WS-Discovery round across all network interfaces.
///
/// Delegates to [`oxvif::discovery::probe`], which handles multi-NIC
/// enumeration and `IP_MULTICAST_IF` pinning (critical on Windows with
/// Hyper-V / WSL virtual adapters). Callers that want multi-round
/// resilience should loop and dedupe by [`DiscoveredDevice::endpoint`] —
/// see `device_list::do_scan` which drives 3 rounds with progressive UI
/// updates so the device list fills in as responses arrive instead of
/// blocking on a single 9 s `probe_rounds`.
#[instrument(skip_all, fields(timeout_secs = timeout.as_secs()))]
pub async fn discover_one_round(timeout: Duration) -> Result<Vec<DiscoveredDevice>, ApiError> {
    Ok(oxvif::discovery::probe(timeout).await)
}

// ── Device Info ─────────────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr))]
pub async fn get_device_info(addr: &str, creds: &Credentials) -> Result<DeviceInfo, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetDeviceInformation", addr, s.get_device_info().await)
}

/// Replace the device's configurable scopes (typically `name` and `location`).
///
/// `scopes` should be a complete list of `onvif://www.onvif.org/...` scope
/// URIs. The device retains any *fixed* scopes (hardware class, codec
/// support, etc.) regardless of what's sent — those can't be changed —
/// but every configurable scope it currently has is REPLACED by this list,
/// so callers must include any existing scopes they want to keep.
#[instrument(skip(creds), fields(addr, count = scopes.len()))]
pub async fn set_scopes(
    addr: &str,
    creds: &Credentials,
    scopes: &[String],
) -> Result<(), ApiError> {
    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
    let s = session_for(addr, creds).await?;
    trace_result("SetScopes", addr, s.set_scopes(&scope_refs).await)
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_scopes(addr: &str, creds: &Credentials) -> Result<Vec<String>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetScopes", addr, s.get_scopes().await)
}

// ── Imaging ─────────────────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr, source_token))]
pub async fn get_imaging_settings(
    addr: &str,
    creds: &Credentials,
    source_token: &str,
) -> Result<oxvif::ImagingSettings, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetImagingSettings",
        addr,
        s.get_imaging_settings(source_token).await,
    )
}

#[instrument(skip(creds), fields(addr, source_token))]
pub async fn get_imaging_options(
    addr: &str,
    creds: &Credentials,
    source_token: &str,
) -> Result<oxvif::ImagingOptions, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetImagingOptions",
        addr,
        s.get_imaging_options(source_token).await,
    )
}

#[instrument(skip(creds, settings), fields(addr, source_token))]
pub async fn set_imaging_settings(
    addr: &str,
    creds: &Credentials,
    source_token: &str,
    settings: &oxvif::ImagingSettings,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetImagingSettings",
        addr,
        s.set_imaging_settings(source_token, settings).await,
    )
}

/// Resolve the `video_source_token` for `profile_token`.
///
/// Used by views that drive the Imaging service (image quality
/// settings, focus motor) — the Imaging service addresses cameras
/// by video source, not by profile. Prefers the requested profile;
/// falls back to the first profile that has any video source so
/// the UI doesn't bail when `selected_profile` is stale from
/// another device or points at a metadata-only profile. Pass
/// `None` for "no preference, just give me one that works".
///
/// Distinct from [`get_video_source_config_token`], which resolves
/// the *configuration* token used by OSD attach.
#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_video_source_token(
    addr: &str,
    creds: &Credentials,
    profile_token: Option<&str>,
) -> Result<String, ApiError> {
    let s = session_for(addr, creds).await?;
    let profiles = s.get_profiles().await.map_err(|e| e.to_string())?;
    profiles
        .iter()
        .find(|p| profile_token.is_some_and(|t| p.token == t))
        .or_else(|| profiles.first())
        .and_then(|p| p.video_source_token.clone())
        .ok_or_else(|| "No video source found".to_string())
}

// ── Media ───────────────────────────────────────────────────────────────────

/// Fetch all media profiles.
#[instrument(skip(creds), fields(addr))]
pub async fn get_profiles(addr: &str, creds: &Credentials) -> Result<Vec<MediaProfile>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetProfiles", addr, s.get_profiles().await)
}

/// Fetch snapshot URI for a specific profile.
#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_snapshot_uri(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<SnapshotUri, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetSnapshotUri",
        addr,
        s.get_snapshot_uri(profile_token).await,
    )
}

/// Extract the scheme + authority of an ONVIF device address — used as the
/// base for resolving relative snapshot URIs.
///
/// `"http://192.168.1.1/onvif/device_service"` → `"http://192.168.1.1"`.
/// Falls back to returning the address unchanged when no `/` follows the host.
pub fn base_url_from_device_addr(addr: &str) -> String {
    addr.find("://")
        .and_then(|i| addr[i + 3..].find('/').map(|j| &addr[..i + 3 + j]))
        .unwrap_or(addr)
        .to_string()
}

/// Resolve a `<tt:Uri>` from `GetSnapshotUriResponse` to a full HTTP URL.
///
/// Handles three shapes that turn up in the wild:
///   1. Already absolute (`http://…`/`https://…`) — passed through.
///   2. Absolute path (`/cgi-bin/snap.cgi`) — prefixed with the device's
///      base URL.
///   3. Bare path (`snap.cgi?...`) — prefixed with `<base>/`.
pub fn resolve_snapshot_url(device_addr: &str, raw_uri: &str) -> String {
    if raw_uri.starts_with("http://") || raw_uri.starts_with("https://") {
        return raw_uri.to_string();
    }
    let base = base_url_from_device_addr(device_addr);
    if raw_uri.starts_with('/') {
        format!("{base}{raw_uri}")
    } else {
        format!("{base}/{raw_uri}")
    }
}

/// Download a snapshot image and return it as a `data:` URI (base64-encoded).
///
/// When credentials are available, tries authenticated methods first:
/// 1. Probe with GET (no auth) to discover the auth scheme
/// 2. If 401 + Digest challenge → manual Digest auth
/// 3. If still failing → Basic auth
/// 4. If no 401 (e.g. 500) → retry with Basic auth anyway (some cameras
///    return non-401 errors when auth is missing)
///
/// Without credentials, sends a single unauthenticated GET.
#[instrument(skip(creds), fields(snapshot_url))]
pub async fn fetch_snapshot_data_uri(
    snapshot_url: &str,
    creds: &Credentials,
) -> Result<String, ApiError> {
    // Match curl's wire shape (Uniview LAPI rejects requests that diverge
    // from curl's exact header set + Authorization field order):
    //   - no Accept-Encoding         (no_gzip / no_brotli / no_deflate / no_zstd)
    //   - User-Agent: curl/8.13.0
    //   - Accept: */*                (added per-request)
    //   - Authorization fields reordered to curl's order in try_digest_auth
    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(!tls_strict())
        .timeout(Duration::from_secs(5))
        .no_gzip()
        .no_brotli()
        .no_deflate()
        .no_zstd()
        .user_agent("curl/8.13.0")
        .build()
        .map_err(|e| e.to_string())?;

    let has_auth = !creds.username.is_empty();

    // ── If we have credentials, try authenticated methods first ──────────

    if has_auth {
        let (u, p) = (creds.username.as_str(), creds.password.as_str());

        // Probe to discover auth scheme
        let probe = http.get(snapshot_url).send().await.map_err(|e| {
            error!(snapshot_url, error = %e, "HTTP request failed");
            e.to_string()
        })?;

        if probe.status().is_success() {
            return snapshot_response_to_data_uri(probe, snapshot_url).await;
        }

        let probe_status = probe.status();
        let www_auth = probe
            .headers()
            .get("www-authenticate")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        // Try Digest auth if challenge is present.
        // Uses a raw TcpStream rather than reqwest because some cameras
        // (Uniview LAPI) reject reqwest/hyper-framed requests even with
        // a byte-identical Authorization header — likely a header-ordering
        // or framing quirk we couldn't pin down. Raw TCP gives byte-for-byte
        // control matching curl.
        //
        // The raw path is plain http only; for https URLs we'd need a TLS
        // handshake we don't carry, so log and skip — reqwest's Basic-auth
        // attempt below is the fallback. Visible in logs so HTTPS+Uniview
        // failures don't look like a silent dead end.
        let raw_eligible =
            www_auth.to_lowercase().contains("digest") && snapshot_url.starts_with("http://");
        if !raw_eligible && www_auth.to_lowercase().contains("digest") {
            debug!(
                snapshot_url,
                "Skipping raw TCP Digest path (https — Basic-auth fallback only)"
            );
        }
        if raw_eligible {
            trace!(snapshot_url, www_authenticate = %www_auth, "Attempting Digest auth (raw TCP)");
            match try_digest_auth_raw(snapshot_url, u, p, &www_auth).await {
                Ok((content_type, bytes)) => {
                    use base64::Engine;
                    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
                    // Snapshot success fires once per profile per ~3s
                    // tick; trace level keeps `oxdm=debug` quiet so
                    // SOAP/event traces stay readable.
                    trace!(
                        snapshot_url,
                        content_type = %content_type,
                        size_bytes = bytes.len(),
                        "Snapshot OK (raw TCP)"
                    );
                    return Ok(format!("data:{content_type};base64,{b64}"));
                }
                Err(e) => {
                    warn!(snapshot_url, error = %e, "Digest auth (raw TCP) failed");
                }
            }
        }

        // Try Basic auth (works for many cameras, also covers non-401 cases)
        trace!(snapshot_url, "Attempting Basic auth");
        let resp = http
            .get(snapshot_url)
            .basic_auth(u, Some(p))
            .send()
            .await
            .map_err(|e| e.to_string())?;

        if resp.status().is_success() {
            return snapshot_response_to_data_uri(resp, snapshot_url).await;
        }

        let status = resp.status();
        error!(
            snapshot_url,
            probe_status = %probe_status,
            final_status = %status,
            www_authenticate = %www_auth,
            "All auth methods failed"
        );
        return Err(format!("HTTP {status}"));
    }

    // ── No credentials — single unauthenticated attempt ─────────────────

    let resp = http.get(snapshot_url).send().await.map_err(|e| {
        error!(snapshot_url, error = %e, "HTTP request failed");
        e.to_string()
    })?;

    if resp.status().is_success() {
        return snapshot_response_to_data_uri(resp, snapshot_url).await;
    }

    let status = resp.status();
    error!(snapshot_url, status = %status, "Snapshot failed (no credentials)");
    Err(format!("HTTP {status}"))
}

/// Manually compute Digest Auth header and send via a raw `TcpStream`,
/// byte-for-byte matching curl's request shape. Returns `(content_type, body)`
/// on HTTP 200, otherwise an error.
///
/// Why raw TCP instead of reqwest: the Uniview LAPI endpoint
/// `/LAPI/V1.0/Media/Video/Streams/0/Snapshot` accepts curl's request but
/// rejects reqwest's request even when the Authorization header is
/// byte-identical and in the same field order. The discrepancy is somewhere
/// in hyper's request framing (probably a default header we couldn't strip).
/// Writing raw HTTP/1.1 to a `TcpStream` sidesteps the entire hyper layer.
async fn try_digest_auth_raw(
    url: &str,
    username: &str,
    password: &str,
    www_authenticate: &str,
) -> Result<(String, Vec<u8>), ApiError> {
    use tokio::io::AsyncWriteExt;
    use tokio::net::TcpStream;

    let (host, port, path) =
        parse_http_url(url).ok_or_else(|| format!("invalid HTTP URL: {url}"))?;

    // Credential diagnostic. Release builds only log the username + password
    // length to avoid leaking entropy when users share logs with support;
    // debug builds get the first/last char too which is occasionally
    // necessary to spot copy-paste invisible characters.
    #[cfg(debug_assertions)]
    {
        let pass_hint = if password.len() >= 2 {
            let chars: Vec<char> = password.chars().collect();
            format!(
                "{}...{} (len={})",
                chars[0],
                chars[chars.len() - 1],
                password.len()
            )
        } else {
            format!("(len={})", password.len())
        };
        trace!(username, pass_hint = %pass_hint, "Digest auth credentials");
    }
    #[cfg(not(debug_assertions))]
    debug!(
        username,
        pass_len = password.len(),
        "Digest auth credentials"
    );

    // Compute Digest Authorization header.
    let context = digest_auth::AuthContext::new(username, password, &path);
    let mut prompt = digest_auth::parse(www_authenticate).map_err(|e| {
        error!(error = %e, www_authenticate, "Failed to parse WWW-Authenticate");
        e.to_string()
    })?;
    let answer = prompt.respond(&context).map_err(|e| {
        error!(error = %e, "Failed to compute Digest response");
        e.to_string()
    })?;
    let raw = answer
        .to_header_string()
        .replace("qop=auth", r#"qop="auth""#)
        .replace(", ", ",");
    let auth_header = reorder_digest_fields(&raw);

    // Build the request line. Match curl exactly: Host (without :80), then
    // Authorization, User-Agent, Accept. No Connection, no Content-Length,
    // no Accept-Encoding.
    let host_header = if port == 80 {
        host.clone()
    } else {
        format!("{host}:{port}")
    };
    let req = format!(
        "GET {path} HTTP/1.1\r\n\
         Host: {host_header}\r\n\
         Authorization: {auth_header}\r\n\
         User-Agent: curl/8.13.0\r\n\
         Accept: */*\r\n\
         \r\n"
    );
    trace!(uri_path = %path, "Sending raw Digest request");

    let connect = TcpStream::connect((host.as_str(), port));
    let mut stream = tokio::time::timeout(Duration::from_secs(5), connect)
        .await
        .map_err(|_| "TCP connect timeout".to_string())?
        .map_err(|e| format!("TCP connect: {e}"))?;
    stream
        .write_all(req.as_bytes())
        .await
        .map_err(|e| format!("write: {e}"))?;

    let (status, headers, body) =
        tokio::time::timeout(Duration::from_secs(10), read_http_response(&mut stream))
            .await
            .map_err(|_| "HTTP read timeout".to_string())??;

    if status != 200 {
        return Err(format!("HTTP {status}"));
    }
    let content_type = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Content-Type"))
        .map(|(_, v)| v.clone())
        .unwrap_or_else(|| "image/jpeg".to_string());
    Ok((content_type, body))
}

/// Parse `http://host[:port]/path?...` → `(host, port, path_with_query)`.
fn parse_http_url(url: &str) -> Option<(String, u16, String)> {
    let rest = url.strip_prefix("http://")?;
    let (authority, path) = match rest.find('/') {
        Some(i) => (&rest[..i], rest[i..].to_string()),
        None => (rest, "/".to_string()),
    };
    let (host, port) = match authority.rsplit_once(':') {
        Some((h, p)) => (h.to_string(), p.parse().ok()?),
        None => (authority.to_string(), 80u16),
    };
    Some((host, port, path))
}

/// Read an HTTP/1.1 response from a `TcpStream`. Honours `Content-Length`;
/// does not handle chunked encoding (snapshot endpoints always send a fixed
/// `Content-Length`).
async fn read_http_response(
    stream: &mut tokio::net::TcpStream,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>), ApiError> {
    use tokio::io::AsyncReadExt;

    let mut buf: Vec<u8> = Vec::with_capacity(8192);
    let mut tmp = [0u8; 4096];
    let header_end = loop {
        let n = stream
            .read(&mut tmp)
            .await
            .map_err(|e| format!("read: {e}"))?;
        if n == 0 {
            return Err("connection closed before headers".into());
        }
        buf.extend_from_slice(&tmp[..n]);
        if let Some(pos) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos;
        }
    };

    let header_str = std::str::from_utf8(&buf[..header_end])
        .map_err(|_| "non-utf8 headers".to_string())?
        .to_string();
    let mut lines = header_str.lines();
    let status_line = lines.next().ok_or("no status line")?;
    let status: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .ok_or("bad status line")?;

    let mut headers: Vec<(String, String)> = Vec::new();
    for line in lines {
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }

    let content_length: usize = headers
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("Content-Length"))
        .and_then(|(_, v)| v.parse().ok())
        .unwrap_or(0);

    let body_start = header_end + 4;
    let already = buf.len() - body_start;
    let mut body: Vec<u8> = buf[body_start..].to_vec();
    if content_length > already {
        let needed = content_length - already;
        let mut rest = vec![0u8; needed];
        stream
            .read_exact(&mut rest)
            .await
            .map_err(|e| format!("read body: {e}"))?;
        body.extend_from_slice(&rest);
    } else if content_length > 0 {
        body.truncate(content_length);
    }
    Ok((status, headers, body))
}

/// Re-emit a `Digest k=v,k=v,...` Authorization header with fields in curl's
/// canonical order. Unknown fields (anything not in the order list) are
/// appended at the end in their original sequence.
fn reorder_digest_fields(raw: &str) -> String {
    const ORDER: &[&str] = &[
        "username",
        "realm",
        "nonce",
        "uri",
        "cnonce",
        "nc",
        "algorithm",
        "response",
        "qop",
        "opaque",
    ];

    let body = raw.strip_prefix("Digest ").unwrap_or(raw);

    // (key, full "k=v" segment)
    let mut pairs: Vec<(&str, &str)> = Vec::new();
    for seg in body.split(',') {
        let key = seg.split('=').next().unwrap_or("").trim();
        pairs.push((key, seg));
    }

    let mut out = String::from("Digest ");
    let mut first = true;
    let mut emitted = vec![false; pairs.len()];

    // Emit known fields in canonical order.
    for &want in ORDER {
        if let Some(idx) = pairs.iter().position(|(k, _)| *k == want) {
            if !first {
                out.push(',');
            }
            out.push_str(pairs[idx].1);
            emitted[idx] = true;
            first = false;
        }
    }
    // Append any fields not in ORDER (forward-compat for userhash, etc.).
    for (i, (_, seg)) in pairs.iter().enumerate() {
        if !emitted[i] {
            if !first {
                out.push(',');
            }
            out.push_str(seg);
            first = false;
        }
    }
    out
}

/// Extract image bytes from a successful response and encode as data URI.
async fn snapshot_response_to_data_uri(
    resp: reqwest::Response,
    snapshot_url: &str,
) -> Result<String, ApiError> {
    use base64::Engine;

    let content_type = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/jpeg")
        .to_string();

    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    trace!(
        snapshot_url,
        content_type = %content_type,
        size_bytes = bytes.len(),
        "Snapshot OK"
    );
    let b64 = base64::engine::general_purpose::STANDARD.encode(&bytes);
    Ok(format!("data:{content_type};base64,{b64}"))
}

/// Fetch RTSP stream URI for a specific profile.
#[allow(dead_code)]
#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_stream_uri(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<StreamUri, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetStreamUri", addr, s.get_stream_uri(profile_token).await)
}

// ── Focus (Imaging service) ─────────────────────────────────────────────────
//
// Focus motor control lives on the Imaging service (not PTZ) and addresses
// the camera by **video_source_token** (not profile_token). Auto Focus
// mode (AUTO / MANUAL) is in `ImagingSettings.focus_mode`, edited via
// the Imaging Settings tab.

/// Start continuous focus movement. `speed > 0` focuses farther,
/// `speed < 0` focuses nearer. Call [`imaging_focus_stop`] to halt.
#[instrument(skip(creds), fields(addr, source_token, speed))]
pub async fn imaging_focus_continuous(
    addr: &str,
    creds: &Credentials,
    source_token: &str,
    speed: f32,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "Imaging Move (Focus)",
        addr,
        s.imaging_move(source_token, &FocusMove::Continuous { speed })
            .await,
    )
}

#[instrument(skip(creds), fields(addr, source_token))]
pub async fn imaging_focus_stop(
    addr: &str,
    creds: &Credentials,
    source_token: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "Imaging Stop (Focus)",
        addr,
        s.imaging_stop(source_token).await,
    )
}

// ── PTZ ─────────────────────────────────────────────────────────────────────
//
// PTZ operations were historically gated by an externally-cached
// PTZ service URL because re-fetching `GetCapabilities` on every
// joystick mousedown added 200–400 ms of latency. With the per-
// `(addr, creds)` `OnvifSession` cache in `crate::sessions`, the
// PTZ URL is already in `session.capabilities()` and the session
// itself is cheap to fetch — joystick paths now just call these
// wrappers directly with no pre-fetch dance.

/// Whether the device advertises a PTZ service.
#[instrument(skip(creds), fields(addr))]
pub async fn has_ptz_service(addr: &str, creds: &Credentials) -> Result<bool, ApiError> {
    let s = session_for(addr, creds).await?;
    Ok(s.capabilities().ptz.url.is_some())
}

#[instrument(skip(creds), fields(addr, profile_token, pan, tilt, zoom))]
pub async fn ptz_continuous_move(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
    pan: f32,
    tilt: f32,
    zoom: f32,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PTZ ContinuousMove",
        addr,
        s.ptz_continuous_move(profile_token, pan, tilt, zoom).await,
    )
}

#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn ptz_stop(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("PTZ Stop", addr, s.ptz_stop(profile_token).await)
}

#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn ptz_get_presets(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<Vec<PtzPreset>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PTZ GetPresets",
        addr,
        s.ptz_get_presets(profile_token).await,
    )
}

/// Save the camera's current position as a preset.
///
/// `preset_name` is the label shown in the UI; `preset_token` is optional —
/// pass `None` to create a new preset, or `Some(token)` to overwrite an
/// existing one. Returns the token of the saved preset (camera-assigned
/// for new ones, same as input for updates).
#[instrument(skip(creds), fields(addr, profile_token, preset_name))]
pub async fn ptz_set_preset(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
    preset_name: Option<&str>,
    preset_token: Option<&str>,
) -> Result<String, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PTZ SetPreset",
        addr,
        s.ptz_set_preset(profile_token, preset_name, preset_token)
            .await,
    )
}

#[instrument(skip(creds), fields(addr, profile_token, preset_token))]
pub async fn ptz_remove_preset(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
    preset_token: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PTZ RemovePreset",
        addr,
        s.ptz_remove_preset(profile_token, preset_token).await,
    )
}

#[instrument(skip(creds), fields(addr, profile_token, preset_token))]
pub async fn ptz_goto_preset(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
    preset_token: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PTZ GotoPreset",
        addr,
        s.ptz_goto_preset(profile_token, preset_token).await,
    )
}

#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn ptz_goto_home_position(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PTZ GotoHomePosition",
        addr,
        s.ptz_goto_home_position(profile_token, None).await,
    )
}

// ── Date / Time ─────────────────────────────────────────────────────────────

/// Apply a new system date/time/timezone/DST configuration.
///
/// `datetime_type` is either `"Manual"` (use `utc_datetime`) or `"NTP"`
/// (device syncs from its configured NTP server — see `set_ntp`).
/// `timezone` is a POSIX TZ string, e.g. `"CST-8"` or `"EST5EDT"`.
#[instrument(skip(creds), fields(addr, datetime_type))]
pub async fn set_system_date_and_time(
    addr: &str,
    creds: &Credentials,
    req: &oxvif::SetDateTimeRequest,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetSystemDateAndTime",
        addr,
        s.set_system_date_and_time(req).await,
    )
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_system_date_and_time(
    addr: &str,
    creds: &Credentials,
) -> Result<SystemDateTime, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetSystemDateAndTime",
        addr,
        s.get_system_date_and_time().await,
    )
}

// ── Network ─────────────────────────────────────────────────────────────────

/// Set the device hostname. Most firmware requires a reboot to take effect.
#[instrument(skip(creds), fields(addr, name))]
pub async fn set_hostname(addr: &str, creds: &Credentials, name: &str) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("SetHostname", addr, s.set_hostname(name).await)
}

/// Update the IPv4 configuration of a network interface. Returns `true`
/// if the device needs a reboot to apply the change — we surface this to
/// the caller so the UI can prompt the user.
///
/// This is the IPv4-only convenience entry retained for existing UI
/// callers; new code that needs IPv6 or MTU should call
/// [`set_network_interfaces_full`] directly with a
/// [`NetworkInterfaceConfig`].
#[allow(clippy::too_many_arguments)] // Mirrors the historical 0.9.7 signature
#[instrument(skip(creds), fields(addr, token, ipv4_address, from_dhcp))]
pub async fn set_network_interfaces(
    addr: &str,
    creds: &Credentials,
    token: &str,
    enabled: bool,
    ipv4_address: &str,
    prefix_length: u32,
    from_dhcp: bool,
) -> Result<bool, ApiError> {
    let cfg = NetworkInterfaceConfig {
        enabled,
        mtu: None,
        ipv4: Some(IpStackConfig {
            enabled: true,
            from_dhcp,
            manual: vec![ManualAddress {
                address: ipv4_address.to_string(),
                prefix_length,
            }],
        }),
        ipv6: None,
    };
    set_network_interfaces_full(addr, creds, token, &cfg).await
}

/// Full-shape `SetNetworkInterfaces` for the new IPv6 / MTU panel.
/// Mirrors oxvif 0.9.8's struct API directly.
#[instrument(skip(creds, cfg), fields(addr, token))]
pub async fn set_network_interfaces_full(
    addr: &str,
    creds: &Credentials,
    token: &str,
    cfg: &NetworkInterfaceConfig,
) -> Result<bool, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetNetworkInterfaces",
        addr,
        s.set_network_interfaces(token, cfg).await,
    )
}

/// Set the DNS servers. If `from_dhcp` is true, `servers` is ignored.
#[instrument(skip(creds), fields(addr, from_dhcp))]
pub async fn set_dns(
    addr: &str,
    creds: &Credentials,
    from_dhcp: bool,
    servers: &[String],
) -> Result<(), ApiError> {
    let refs: Vec<&str> = servers.iter().map(String::as_str).collect();
    let s = session_for(addr, creds).await?;
    trace_result("SetDNS", addr, s.set_dns(from_dhcp, &refs).await)
}

/// Set the NTP servers. If `from_dhcp` is true, `servers` is ignored.
#[instrument(skip(creds), fields(addr, from_dhcp))]
pub async fn set_ntp(
    addr: &str,
    creds: &Credentials,
    from_dhcp: bool,
    servers: &[String],
) -> Result<(), ApiError> {
    let refs: Vec<&str> = servers.iter().map(String::as_str).collect();
    let s = session_for(addr, creds).await?;
    trace_result("SetNTP", addr, s.set_ntp(from_dhcp, &refs).await)
}

/// Replace the default IPv4 gateway list.
#[instrument(skip(creds), fields(addr))]
pub async fn set_network_default_gateway(
    addr: &str,
    creds: &Credentials,
    ipv4_addresses: &[String],
) -> Result<(), ApiError> {
    let refs: Vec<&str> = ipv4_addresses.iter().map(String::as_str).collect();
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetNetworkDefaultGateway",
        addr,
        s.set_network_default_gateway(&refs).await,
    )
}

/// Enable/disable network protocols (HTTP / HTTPS / RTSP) and their ports.
/// Each entry: `(name, enabled, ports)`. Names are ONVIF-standard strings.
#[instrument(skip(creds), fields(addr))]
pub async fn set_network_protocols(
    addr: &str,
    creds: &Credentials,
    protocols: &[(String, bool, Vec<u32>)],
) -> Result<(), ApiError> {
    // Build the `&[(&str, bool, &[u32])]` shape oxvif expects by
    // borrowing every name and ports slice from our owned inputs.
    let refs: Vec<(&str, bool, &[u32])> = protocols
        .iter()
        .map(|(n, e, p)| (n.as_str(), *e, p.as_slice()))
        .collect();
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetNetworkProtocols",
        addr,
        s.set_network_protocols(&refs).await,
    )
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_hostname(addr: &str, creds: &Credentials) -> Result<Hostname, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetHostname", addr, s.get_hostname().await)
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_network_interfaces(
    addr: &str,
    creds: &Credentials,
) -> Result<Vec<NetworkInterface>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetNetworkInterfaces",
        addr,
        s.get_network_interfaces().await,
    )
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_dns(addr: &str, creds: &Credentials) -> Result<DnsInformation, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetDNS", addr, s.get_dns().await)
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_ntp(addr: &str, creds: &Credentials) -> Result<NtpInfo, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetNTP", addr, s.get_ntp().await)
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_network_default_gateway(
    addr: &str,
    creds: &Credentials,
) -> Result<NetworkGateway, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetNetworkDefaultGateway",
        addr,
        s.get_network_default_gateway().await,
    )
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_network_protocols(
    addr: &str,
    creds: &Credentials,
) -> Result<Vec<NetworkProtocol>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetNetworkProtocols", addr, s.get_network_protocols().await)
}

// ── IO control ──────────────────────────────────────────────────────────────
//
// `GetRelayOutputs` is ONVIF-optional. Cameras without an IO board typically
// return `SOAP-ENV:Receiver: Optional Action Not Implemented` (or
// `ter:ActionNotSupported`). Map those to the `"unsupported"` sentinel so the
// IO Control view shows a soft empty state instead of a red error banner —
// the device simply doesn't have any IO hardware, which isn't a failure.
const IO_UNSUPPORTED_SENTINEL: &str = "unsupported";

fn is_action_unsupported(err: &str) -> bool {
    let lower = err.to_ascii_lowercase();
    lower.contains("not implemented")
        || lower.contains("actionnotsupported")
        || lower.contains("not supported")
}

fn map_io_unsupported<T>(err: ApiError) -> Result<T, ApiError> {
    if is_action_unsupported(&err) {
        Err(IO_UNSUPPORTED_SENTINEL.to_string())
    } else {
        Err(err)
    }
}

#[instrument(skip(creds), fields(addr))]
pub async fn get_relay_outputs(
    addr: &str,
    creds: &Credentials,
) -> Result<Vec<RelayOutput>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetRelayOutputs", addr, s.get_relay_outputs().await).or_else(map_io_unsupported)
}

/// `state` must be `"active"` or `"inactive"`.
#[instrument(skip(creds), fields(addr, relay_token))]
pub async fn set_relay_output_state(
    addr: &str,
    creds: &Credentials,
    relay_token: &str,
    state: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetRelayOutputState",
        addr,
        s.set_relay_output_state(relay_token, state).await,
    )
}

#[instrument(skip(creds), fields(addr, relay_token, mode))]
pub async fn set_relay_output_settings(
    addr: &str,
    creds: &Credentials,
    relay_token: &str,
    mode: &str,
    delay_time: &str,
    idle_state: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetRelayOutputSettings",
        addr,
        s.set_relay_output_settings(relay_token, mode, delay_time, idle_state)
            .await,
    )
}

// ── Users ───────────────────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr))]
pub async fn get_users(addr: &str, creds: &Credentials) -> Result<Vec<User>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetUsers", addr, s.get_users().await)
}

#[instrument(skip(creds, new_password), fields(addr, new_username))]
pub async fn create_user(
    addr: &str,
    creds: &Credentials,
    new_username: &str,
    new_password: &str,
    user_level: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "CreateUsers",
        addr,
        s.create_users(&[(new_username, new_password, user_level)])
            .await,
    )
}

#[instrument(skip(creds), fields(addr, target_username))]
pub async fn delete_user(
    addr: &str,
    creds: &Credentials,
    target_username: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "DeleteUsers",
        addr,
        s.delete_users(&[target_username]).await,
    )
}

/// Update an existing user's password and/or role.
///
/// Pass `new_password: None` to keep the existing password; `Some("")` is
/// treated as a password change to empty by most cameras (rare and
/// usually rejected), so we guard against that at the call site.
#[instrument(skip(creds, new_password), fields(addr, target_username, user_level))]
pub async fn set_user(
    addr: &str,
    creds: &Credentials,
    target_username: &str,
    new_password: Option<&str>,
    user_level: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetUser",
        addr,
        s.set_user(target_username, new_password, user_level).await,
    )
}

// ── Maintenance ─────────────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr))]
pub async fn system_reboot(addr: &str, creds: &Credentials) -> Result<String, ApiError> {
    info!(addr, "Requesting system reboot");
    let s = session_for(addr, creds).await?;
    trace_result("SystemReboot", addr, s.system_reboot().await)
}

#[instrument(skip(creds), fields(addr, default_type))]
pub async fn set_system_factory_default(
    addr: &str,
    creds: &Credentials,
    default_type: &str,
) -> Result<(), ApiError> {
    info!(addr, default_type, "Requesting factory reset");
    let s = session_for(addr, creds).await?;
    trace_result(
        "SetSystemFactoryDefault",
        addr,
        s.set_system_factory_default(default_type).await,
    )
}

// ── Events ──────────────────────────────────────────────────────────────────
//
// Pull-point subscriptions need two distinct URLs:
//   * the **events service URL** (`events_url`) — where to POST
//     `CreatePullPointSubscription`. The session reads this from
//     `capabilities()`, so callers don't pass it.
//   * the **subscription reference URL** — returned by the camera
//     in `CreatePullPointSubscriptionResponse.SubscriptionReference`,
//     used as the endpoint for subsequent `PullMessages` /
//     `Renew` / `Unsubscribe`. This *must* be passed by the caller
//     because it's per-subscription, not per-device.

#[instrument(skip(creds), fields(addr))]
pub async fn get_event_properties(
    addr: &str,
    creds: &Credentials,
) -> Result<EventProperties, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("GetEventProperties", addr, s.get_event_properties().await)
}

#[instrument(skip(creds), fields(addr))]
pub async fn create_pull_subscription(
    addr: &str,
    creds: &Credentials,
    filter: Option<&str>,
    initial_termination_time: Option<&str>,
) -> Result<PullPointSubscription, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "CreatePullPointSubscription",
        addr,
        s.create_pull_point_subscription(filter, initial_termination_time)
            .await,
    )
}

#[instrument(skip(creds), fields(addr, timeout))]
pub async fn pull_event_messages(
    addr: &str,
    creds: &Credentials,
    subscription_url: &str,
    timeout: &str,
    max_messages: u32,
) -> Result<Vec<NotificationMessage>, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "PullMessages",
        addr,
        s.pull_messages(subscription_url, timeout, max_messages)
            .await,
    )
}

#[instrument(skip(creds), fields(addr))]
pub async fn renew_event_subscription(
    addr: &str,
    creds: &Credentials,
    subscription_url: &str,
    termination_time: &str,
) -> Result<String, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "RenewSubscription",
        addr,
        s.renew_subscription(subscription_url, termination_time)
            .await,
    )
}

#[instrument(skip(creds), fields(addr))]
pub async fn unsubscribe_events(
    addr: &str,
    creds: &Credentials,
    subscription_url: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("Unsubscribe", addr, s.unsubscribe(subscription_url).await)
}

// ── OSD ─────────────────────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_osds(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<Vec<OsdConfiguration>, ApiError> {
    let s = session_for(addr, creds).await?;
    let vsc_token = resolve_vsc_token(&s, profile_token).await?;
    trace_result("GetOSDs", addr, s.get_osds(Some(&vsc_token)).await)
}

#[instrument(skip(creds, osd), fields(addr, token = %osd.token))]
pub async fn set_osd(
    addr: &str,
    creds: &Credentials,
    osd: &OsdConfiguration,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("SetOSD", addr, s.set_osd(osd).await)
}

#[instrument(skip(creds, osd), fields(addr))]
pub async fn create_osd(
    addr: &str,
    creds: &Credentials,
    osd: &OsdConfiguration,
) -> Result<String, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("CreateOSD", addr, s.create_osd(osd).await)
}

#[instrument(skip(creds), fields(addr, osd_token))]
pub async fn delete_osd(addr: &str, creds: &Credentials, osd_token: &str) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("DeleteOSD", addr, s.delete_osd(osd_token).await)
}

/// Fetch the camera's allowed OSD configuration options.
///
/// The returned `OsdOptions` lists supported text types, position
/// types, date/time formats, font size range — and the per-text-type
/// quotas some cameras stash in non-spec XML attributes (Genetec,
/// late Hikvision). The quotas are only populated by
/// `OnvifSession::get_osd_options`, not `OnvifClient::get_osd_options`
/// — that's the spec-strict-vs-vendor-tolerant split discussed in
/// oxvif's CHANGELOG. The OSD editor uses them to disable the type
/// dropdown when the camera is full.
#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_osd_options(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<OsdOptions, ApiError> {
    let s = session_for(addr, creds).await?;
    let vsc_token = resolve_vsc_token(&s, profile_token).await?;
    trace_result("GetOSDOptions", addr, s.get_osd_options(&vsc_token).await)
}

/// Resolve the video source configuration token for a profile.
/// Used by the OSD UI when CREATING a new OSD — the new entry needs
/// to know which video source it attaches to.
#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn get_video_source_config_token(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<String, ApiError> {
    let s = session_for(addr, creds).await?;
    resolve_vsc_token(&s, profile_token).await
}

/// Pick the most-appropriate video source configuration token for
/// `profile_token`. Tries the requested profile first; falls back to
/// the first profile that has any video source — covers the case
/// where `selected_profile` is stale from another device, or where
/// the current profile is metadata-only.
async fn resolve_vsc_token(
    session: &OnvifSession,
    profile_token: &str,
) -> Result<String, ApiError> {
    let profiles = session.get_profiles().await.map_err(|e| e.to_string())?;
    profiles
        .iter()
        .find(|p| p.token == profile_token)
        .and_then(|p| p.video_source_config_token.clone())
        .or_else(|| {
            profiles
                .iter()
                .find_map(|p| p.video_source_config_token.clone())
        })
        .ok_or_else(|| "No profile with a video source configuration".to_string())
}

// ── Video encoder ─────────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr, token))]
pub async fn get_video_encoder_configuration(
    addr: &str,
    creds: &Credentials,
    token: &str,
) -> Result<VideoEncoderConfiguration, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetVideoEncoderConfiguration",
        addr,
        s.get_video_encoder_configuration(token).await,
    )
}

#[instrument(skip(creds), fields(addr, config_token))]
pub async fn get_video_encoder_configuration_options(
    addr: &str,
    creds: &Credentials,
    config_token: Option<&str>,
) -> Result<VideoEncoderConfigurationOptions, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result(
        "GetVideoEncoderConfigurationOptions",
        addr,
        s.get_video_encoder_configuration_options(config_token)
            .await,
    )
}

#[instrument(skip(creds, cfg), fields(addr, token = %cfg.token, encoding = ?cfg.encoding))]
pub async fn set_video_encoder_configuration(
    addr: &str,
    creds: &Credentials,
    cfg: &VideoEncoderConfiguration,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    // oxvif 0.9.8 rejects H265 on the Media1 path (schema-correct: Media1
    // schema is JPEG/MPEG4/H264 only). Reroute H265 through Media2 so the
    // UI keeps working uniformly. Cameras that advertise Media2 — which
    // is almost all H265-capable devices — accept this cleanly.
    if cfg.encoding == VideoEncoding::H265 {
        if s.capabilities().media2.url.is_none() {
            warn!(addr, "device advertises H265 but no Media2 service URL");
            return Err("This camera reports H265 but doesn't advertise Media2; \
                        H265 encoder edits aren't supported via Media1."
                .into());
        }
        let cfg2 = to_video_encoder_configuration2(cfg);
        return trace_result(
            "SetVideoEncoderConfiguration2",
            addr,
            s.set_video_encoder_configuration_media2(&cfg2).await,
        );
    }
    trace_result(
        "SetVideoEncoderConfiguration",
        addr,
        s.set_video_encoder_configuration(cfg).await,
    )
}

/// Convert a Media1 `VideoEncoderConfiguration` to the Media2 shape. The
/// Media2 type folds H264/H265 fields up to the top level (no codec
/// sub-struct) and drops `encoding_interval` from rate control —
/// those drop on the wire. Multicast / session-timeout /
/// guaranteed-frame-rate aren't carried in Media2's struct here either.
fn to_video_encoder_configuration2(cfg: &VideoEncoderConfiguration) -> VideoEncoderConfiguration2 {
    let (gov_length, profile) = match cfg.encoding {
        VideoEncoding::H264 => cfg
            .h264
            .as_ref()
            .map(|h| (Some(h.gov_length), Some(h.profile.clone())))
            .unwrap_or((None, None)),
        VideoEncoding::H265 => cfg
            .h265
            .as_ref()
            .map(|h| (Some(h.gov_length), Some(h.profile.clone())))
            .unwrap_or((None, None)),
        _ => (None, None),
    };
    VideoEncoderConfiguration2 {
        token: cfg.token.clone(),
        name: cfg.name.clone(),
        use_count: cfg.use_count,
        encoding: cfg.encoding.clone(),
        resolution: cfg.resolution,
        quality: cfg.quality,
        rate_control: cfg.rate_control.as_ref().map(|rc| VideoRateControl2 {
            frame_rate_limit: rc.frame_rate_limit,
            bitrate_limit: rc.bitrate_limit,
        }),
        gov_length,
        profile,
    }
}

// ── Profile management ──────────────────────────────────────────────────────

#[instrument(skip(creds), fields(addr, name))]
pub async fn create_profile(
    addr: &str,
    creds: &Credentials,
    name: &str,
) -> Result<MediaProfile, ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("CreateProfile", addr, s.create_profile(name, None).await)
}

#[instrument(skip(creds), fields(addr, profile_token))]
pub async fn delete_profile(
    addr: &str,
    creds: &Credentials,
    profile_token: &str,
) -> Result<(), ApiError> {
    let s = session_for(addr, creds).await?;
    trace_result("DeleteProfile", addr, s.delete_profile(profile_token).await)
}

/// Run oxvif's read-only ONVIF health / conformance check against a device.
///
/// Unlike the other wrappers this does *not* go through the `sessions`
/// cache — `HealthCheck` builds its own short-lived session internally (it
/// deliberately tests connectivity from scratch). It never mutates the
/// device: write probes are left disabled. Infallible — connection/auth
/// problems surface as failed checks inside the returned
/// [`oxvif::health::HealthReport`], not as an `Err`.
#[instrument(skip(creds), fields(addr))]
pub async fn run_health_check(addr: &str, creds: &Credentials) -> oxvif::health::HealthReport {
    let mut hc = oxvif::health::HealthCheck::new(addr);
    if !creds.username.is_empty() {
        hc = hc.with_credentials(creds.username.clone(), creds.password.clone());
    }
    let report = hc.run().await;
    debug!(
        addr,
        ok = report.ok(),
        elapsed_ms = report.total_elapsed.as_millis() as u64,
        "health check done"
    );
    report
}
