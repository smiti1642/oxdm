use oxvif::{
    Capabilities, DeviceInfo, DiscoveredDevice, DnsInformation, EventProperties, FocusMove,
    Hostname, MediaProfile, NetworkGateway, NetworkInterface, NetworkProtocol, NotificationMessage,
    NtpInfo, OnvifClient, OsdConfiguration, OsdOptions, PtzPreset, PullPointSubscription,
    SnapshotUri, StreamUri, SystemDateTime, User,
};
use std::time::Duration;
use tracing::{debug, error, info, instrument, trace, warn};

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

/// Build a client for a device, optionally with credentials.
fn build_client(addr: &str, username: Option<&str>, password: Option<&str>) -> OnvifClient {
    let mut client = OnvifClient::new(addr);
    if let (Some(u), Some(p)) = (username, password) {
        client = client.with_credentials(u, p);
    }
    client
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

#[instrument(skip(username, password), fields(addr))]
pub async fn get_device_info(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<DeviceInfo, ApiError> {
    trace_result(
        "GetDeviceInformation",
        addr,
        build_client(addr, username, password)
            .get_device_info()
            .await,
    )
}

/// Replace the device's configurable scopes (typically `name` and `location`).
///
/// `scopes` should be a complete list of `onvif://www.onvif.org/...` scope
/// URIs. The device retains any *fixed* scopes (hardware class, codec
/// support, etc.) regardless of what's sent — those can't be changed —
/// but every configurable scope it currently has is REPLACED by this list,
/// so callers must include any existing scopes they want to keep.
#[instrument(skip(username, password), fields(addr, count = scopes.len()))]
pub async fn set_scopes(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    scopes: &[String],
) -> Result<(), ApiError> {
    let scope_refs: Vec<&str> = scopes.iter().map(String::as_str).collect();
    trace_result(
        "SetScopes",
        addr,
        build_client(addr, username, password)
            .set_scopes(&scope_refs)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_scopes(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<String>, ApiError> {
    trace_result(
        "GetScopes",
        addr,
        build_client(addr, username, password).get_scopes().await,
    )
}

#[allow(dead_code)]
#[instrument(skip(username, password), fields(addr))]
pub async fn get_capabilities(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Capabilities, ApiError> {
    trace_result(
        "GetCapabilities",
        addr,
        build_client(addr, username, password)
            .get_capabilities()
            .await,
    )
}

// ── Imaging ─────────────────────────────────────────────────────────────────

#[instrument(skip(username, password), fields(addr, source_token))]
pub async fn get_imaging_settings(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    source_token: &str,
) -> Result<oxvif::ImagingSettings, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let url = caps.imaging.url.ok_or("No imaging service URL")?;
    trace_result(
        "GetImagingSettings",
        addr,
        client.get_imaging_settings(&url, source_token).await,
    )
}

#[instrument(skip(username, password), fields(addr, source_token))]
pub async fn get_imaging_options(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    source_token: &str,
) -> Result<oxvif::ImagingOptions, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let url = caps.imaging.url.ok_or("No imaging service URL")?;
    trace_result(
        "GetImagingOptions",
        addr,
        client.get_imaging_options(&url, source_token).await,
    )
}

#[instrument(skip(username, password, settings), fields(addr, source_token))]
pub async fn set_imaging_settings(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    source_token: &str,
    settings: &oxvif::ImagingSettings,
) -> Result<(), ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let url = caps.imaging.url.ok_or("No imaging service URL")?;
    trace_result(
        "SetImagingSettings",
        addr,
        client
            .set_imaging_settings(&url, source_token, settings)
            .await,
    )
}

// ── Media ───────────────────────────────────────────────────────────────────

/// Fetch all media profiles. Requires the media service URL from GetCapabilities.
#[instrument(skip(username, password), fields(addr))]
pub async fn get_profiles(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<MediaProfile>, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result("GetProfiles", addr, client.get_profiles(&media_url).await)
}

/// Fetch snapshot URI for a specific profile.
#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn get_snapshot_uri(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    profile_token: &str,
) -> Result<SnapshotUri, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result(
        "GetSnapshotUri",
        addr,
        client.get_snapshot_uri(&media_url, profile_token).await,
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
#[instrument(skip(username, password), fields(snapshot_url))]
pub async fn fetch_snapshot_data_uri(
    snapshot_url: &str,
    username: Option<&str>,
    password: Option<&str>,
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

    let has_auth = matches!((username, password), (Some(u), Some(_)) if !u.is_empty());

    // ── If we have credentials, try authenticated methods first ──────────

    if has_auth {
        let (u, p) = (username.unwrap(), password.unwrap());

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
#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn get_stream_uri(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    profile_token: &str,
) -> Result<StreamUri, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result(
        "GetStreamUri",
        addr,
        client.get_stream_uri(&media_url, profile_token).await,
    )
}

// ── Focus (Imaging service) ─────────────────────────────────────────────────
//
// Focus motor control lives on the Imaging service (not PTZ) and addresses
// the camera by **video_source_token** (not profile_token). Auto Focus
// mode (AUTO / MANUAL) is in `ImagingSettings.focus_mode`, edited via
// the Imaging Settings tab.

/// Fetch the device's Imaging service URL. Errors if not advertised.
#[instrument(skip(username, password), fields(addr))]
pub async fn get_imaging_url(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    caps.imaging
        .url
        .ok_or_else(|| "Imaging service not advertised by this device".to_string())
}

/// Start continuous focus movement. `speed > 0` focuses farther,
/// `speed < 0` focuses nearer. Call [`imaging_focus_stop`] to halt.
#[instrument(skip(username, password), fields(addr, source_token, speed))]
pub async fn imaging_focus_continuous(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    imaging_url: &str,
    source_token: &str,
    speed: f32,
) -> Result<(), ApiError> {
    trace_result(
        "Imaging Move (Focus)",
        addr,
        build_client(addr, username, password)
            .imaging_move(imaging_url, source_token, &FocusMove::Continuous { speed })
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, source_token))]
pub async fn imaging_focus_stop(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    imaging_url: &str,
    source_token: &str,
) -> Result<(), ApiError> {
    trace_result(
        "Imaging Stop (Focus)",
        addr,
        build_client(addr, username, password)
            .imaging_stop(imaging_url, source_token)
            .await,
    )
}

// ── PTZ ─────────────────────────────────────────────────────────────────────
//
// PTZ operations re-use the device's PTZ service URL. Callers should fetch
// it once (via [`get_ptz_url`]) and cache it — joystick UX needs sub-100ms
// response, and re-querying GetCapabilities on every mousedown would add
// 200–400 ms of round-trip latency. The other api wrappers in this file
// re-fetch capabilities each call because their callers are tab-rate, not
// joystick-rate.

/// Fetch the device's PTZ service URL. Errors if the device doesn't expose
/// a PTZ service in its capabilities.
#[instrument(skip(username, password), fields(addr))]
pub async fn get_ptz_url(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    caps.ptz
        .url
        .ok_or_else(|| "PTZ service not advertised by this device".to_string())
}

#[allow(clippy::too_many_arguments)] // Mirrors oxvif's signature 1:1
#[instrument(skip(username, password), fields(addr, profile_token, pan, tilt, zoom))]
pub async fn ptz_continuous_move(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
    pan: f32,
    tilt: f32,
    zoom: f32,
) -> Result<(), ApiError> {
    trace_result(
        "PTZ ContinuousMove",
        addr,
        build_client(addr, username, password)
            .ptz_continuous_move(ptz_url, profile_token, pan, tilt, zoom)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn ptz_stop(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
) -> Result<(), ApiError> {
    trace_result(
        "PTZ Stop",
        addr,
        build_client(addr, username, password)
            .ptz_stop(ptz_url, profile_token)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn ptz_get_presets(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
) -> Result<Vec<PtzPreset>, ApiError> {
    trace_result(
        "PTZ GetPresets",
        addr,
        build_client(addr, username, password)
            .ptz_get_presets(ptz_url, profile_token)
            .await,
    )
}

/// Save the camera's current position as a preset.
///
/// `preset_name` is the label shown in the UI; `preset_token` is optional —
/// pass `None` to create a new preset, or `Some(token)` to overwrite an
/// existing one. Returns the token of the saved preset (camera-assigned
/// for new ones, same as input for updates).
#[allow(clippy::too_many_arguments)] // Mirrors oxvif's signature 1:1
#[instrument(skip(username, password), fields(addr, profile_token, preset_name))]
pub async fn ptz_set_preset(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
    preset_name: Option<&str>,
    preset_token: Option<&str>,
) -> Result<String, ApiError> {
    trace_result(
        "PTZ SetPreset",
        addr,
        build_client(addr, username, password)
            .ptz_set_preset(ptz_url, profile_token, preset_name, preset_token)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, profile_token, preset_token))]
pub async fn ptz_remove_preset(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
    preset_token: &str,
) -> Result<(), ApiError> {
    trace_result(
        "PTZ RemovePreset",
        addr,
        build_client(addr, username, password)
            .ptz_remove_preset(ptz_url, profile_token, preset_token)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, profile_token, preset_token))]
pub async fn ptz_goto_preset(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
    preset_token: &str,
) -> Result<(), ApiError> {
    trace_result(
        "PTZ GotoPreset",
        addr,
        build_client(addr, username, password)
            .ptz_goto_preset(ptz_url, profile_token, preset_token)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn ptz_goto_home_position(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ptz_url: &str,
    profile_token: &str,
) -> Result<(), ApiError> {
    trace_result(
        "PTZ GotoHomePosition",
        addr,
        build_client(addr, username, password)
            .ptz_goto_home_position(ptz_url, profile_token, None)
            .await,
    )
}

// ── Date / Time ─────────────────────────────────────────────────────────────

/// Apply a new system date/time/timezone/DST configuration.
///
/// `datetime_type` is either `"Manual"` (use `utc_datetime`) or `"NTP"`
/// (device syncs from its configured NTP server — see `set_ntp`).
/// `timezone` is a POSIX TZ string, e.g. `"CST-8"` or `"EST5EDT"`.
#[instrument(skip(username, password), fields(addr, datetime_type))]
pub async fn set_system_date_and_time(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    req: &oxvif::SetDateTimeRequest,
) -> Result<(), ApiError> {
    trace_result(
        "SetSystemDateAndTime",
        addr,
        build_client(addr, username, password)
            .set_system_date_and_time(req)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_system_date_and_time(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<SystemDateTime, ApiError> {
    trace_result(
        "GetSystemDateAndTime",
        addr,
        build_client(addr, username, password)
            .get_system_date_and_time()
            .await,
    )
}

// ── Network ─────────────────────────────────────────────────────────────────

/// Set the device hostname. Most firmware requires a reboot to take effect.
#[instrument(skip(username, password), fields(addr, name))]
pub async fn set_hostname(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    name: &str,
) -> Result<(), ApiError> {
    trace_result(
        "SetHostname",
        addr,
        build_client(addr, username, password)
            .set_hostname(name)
            .await,
    )
}

/// Update the IPv4 configuration of a network interface. Returns `true`
/// if the device needs a reboot to apply the change — we surface this to
/// the caller so the UI can prompt the user.
#[allow(clippy::too_many_arguments)] // Mirrors oxvif's signature 1:1
#[instrument(skip(username, password), fields(addr, token, ipv4_address, from_dhcp))]
pub async fn set_network_interfaces(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    token: &str,
    enabled: bool,
    ipv4_address: &str,
    prefix_length: u32,
    from_dhcp: bool,
) -> Result<bool, ApiError> {
    trace_result(
        "SetNetworkInterfaces",
        addr,
        build_client(addr, username, password)
            .set_network_interfaces(token, enabled, ipv4_address, prefix_length, from_dhcp)
            .await,
    )
}

/// Set the DNS servers. If `from_dhcp` is true, `servers` is ignored.
#[instrument(skip(username, password), fields(addr, from_dhcp))]
pub async fn set_dns(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    from_dhcp: bool,
    servers: &[String],
) -> Result<(), ApiError> {
    let refs: Vec<&str> = servers.iter().map(String::as_str).collect();
    trace_result(
        "SetDNS",
        addr,
        build_client(addr, username, password)
            .set_dns(from_dhcp, &refs)
            .await,
    )
}

/// Set the NTP servers. If `from_dhcp` is true, `servers` is ignored.
#[instrument(skip(username, password), fields(addr, from_dhcp))]
pub async fn set_ntp(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    from_dhcp: bool,
    servers: &[String],
) -> Result<(), ApiError> {
    let refs: Vec<&str> = servers.iter().map(String::as_str).collect();
    trace_result(
        "SetNTP",
        addr,
        build_client(addr, username, password)
            .set_ntp(from_dhcp, &refs)
            .await,
    )
}

/// Replace the default IPv4 gateway list.
#[instrument(skip(username, password), fields(addr))]
pub async fn set_network_default_gateway(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    ipv4_addresses: &[String],
) -> Result<(), ApiError> {
    let refs: Vec<&str> = ipv4_addresses.iter().map(String::as_str).collect();
    trace_result(
        "SetNetworkDefaultGateway",
        addr,
        build_client(addr, username, password)
            .set_network_default_gateway(&refs)
            .await,
    )
}

/// Enable/disable network protocols (HTTP / HTTPS / RTSP) and their ports.
/// Each entry: `(name, enabled, ports)`. Names are ONVIF-standard strings.
#[instrument(skip(username, password), fields(addr))]
pub async fn set_network_protocols(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    protocols: &[(String, bool, Vec<u32>)],
) -> Result<(), ApiError> {
    // Build the `&[(&str, bool, &[u32])]` shape oxvif expects by
    // borrowing every name and ports slice from our owned inputs.
    let refs: Vec<(&str, bool, &[u32])> = protocols
        .iter()
        .map(|(n, e, p)| (n.as_str(), *e, p.as_slice()))
        .collect();
    trace_result(
        "SetNetworkProtocols",
        addr,
        build_client(addr, username, password)
            .set_network_protocols(&refs)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_hostname(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Hostname, ApiError> {
    trace_result(
        "GetHostname",
        addr,
        build_client(addr, username, password).get_hostname().await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_network_interfaces(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<NetworkInterface>, ApiError> {
    trace_result(
        "GetNetworkInterfaces",
        addr,
        build_client(addr, username, password)
            .get_network_interfaces()
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_dns(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<DnsInformation, ApiError> {
    trace_result(
        "GetDNS",
        addr,
        build_client(addr, username, password).get_dns().await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_ntp(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<NtpInfo, ApiError> {
    trace_result(
        "GetNTP",
        addr,
        build_client(addr, username, password).get_ntp().await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_network_default_gateway(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<NetworkGateway, ApiError> {
    trace_result(
        "GetNetworkDefaultGateway",
        addr,
        build_client(addr, username, password)
            .get_network_default_gateway()
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_network_protocols(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<NetworkProtocol>, ApiError> {
    trace_result(
        "GetNetworkProtocols",
        addr,
        build_client(addr, username, password)
            .get_network_protocols()
            .await,
    )
}

// ── Users ───────────────────────────────────────────────────────────────────

#[instrument(skip(username, password), fields(addr))]
pub async fn get_users(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<User>, ApiError> {
    trace_result(
        "GetUsers",
        addr,
        build_client(addr, username, password).get_users().await,
    )
}

#[instrument(skip(username, password, new_password), fields(addr, new_username))]
pub async fn create_user(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    new_username: &str,
    new_password: &str,
    user_level: &str,
) -> Result<(), ApiError> {
    trace_result(
        "CreateUsers",
        addr,
        build_client(addr, username, password)
            .create_users(&[(new_username, new_password, user_level)])
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, target_username))]
pub async fn delete_user(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    target_username: &str,
) -> Result<(), ApiError> {
    trace_result(
        "DeleteUsers",
        addr,
        build_client(addr, username, password)
            .delete_users(&[target_username])
            .await,
    )
}

/// Update an existing user's password and/or role.
///
/// Pass `new_password: None` to keep the existing password; `Some("")` is
/// treated as a password change to empty by most cameras (rare and
/// usually rejected), so we guard against that at the call site.
#[instrument(
    skip(username, password, new_password),
    fields(addr, target_username, user_level)
)]
pub async fn set_user(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    target_username: &str,
    new_password: Option<&str>,
    user_level: &str,
) -> Result<(), ApiError> {
    trace_result(
        "SetUser",
        addr,
        build_client(addr, username, password)
            .set_user(target_username, new_password, user_level)
            .await,
    )
}

// ── Maintenance ─────────────────────────────────────────────────────────────

#[instrument(skip(username, password), fields(addr))]
pub async fn system_reboot(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String, ApiError> {
    info!(addr, "Requesting system reboot");
    trace_result(
        "SystemReboot",
        addr,
        build_client(addr, username, password).system_reboot().await,
    )
}

#[instrument(skip(username, password), fields(addr, default_type))]
pub async fn set_system_factory_default(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    default_type: &str,
) -> Result<(), ApiError> {
    info!(addr, default_type, "Requesting factory reset");
    trace_result(
        "SetSystemFactoryDefault",
        addr,
        build_client(addr, username, password)
            .set_system_factory_default(default_type)
            .await,
    )
}

// ── Events ──────────────────────────────────────────────────────────────────

#[instrument(skip(username, password), fields(addr))]
pub async fn get_events_url(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    caps.events
        .url
        .ok_or_else(|| "Events service not advertised by this device".to_string())
}

#[instrument(skip(username, password), fields(addr))]
pub async fn get_event_properties(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    events_url: &str,
) -> Result<EventProperties, ApiError> {
    trace_result(
        "GetEventProperties",
        addr,
        build_client(addr, username, password)
            .get_event_properties(events_url)
            .await,
    )
}

#[allow(clippy::too_many_arguments)] // Mirrors oxvif's signature 1:1
#[instrument(skip(username, password), fields(addr))]
pub async fn create_pull_subscription(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    events_url: &str,
    filter: Option<&str>,
    initial_termination_time: Option<&str>,
) -> Result<PullPointSubscription, ApiError> {
    trace_result(
        "CreatePullPointSubscription",
        addr,
        build_client(addr, username, password)
            .create_pull_point_subscription(events_url, filter, initial_termination_time)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr, timeout))]
pub async fn pull_event_messages(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    subscription_url: &str,
    timeout: &str,
    max_messages: u32,
) -> Result<Vec<NotificationMessage>, ApiError> {
    trace_result(
        "PullMessages",
        addr,
        build_client(addr, username, password)
            .pull_messages(subscription_url, timeout, max_messages)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn renew_event_subscription(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    subscription_url: &str,
    termination_time: &str,
) -> Result<String, ApiError> {
    trace_result(
        "RenewSubscription",
        addr,
        build_client(addr, username, password)
            .renew_subscription(subscription_url, termination_time)
            .await,
    )
}

#[instrument(skip(username, password), fields(addr))]
pub async fn unsubscribe_events(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    subscription_url: &str,
) -> Result<(), ApiError> {
    trace_result(
        "Unsubscribe",
        addr,
        build_client(addr, username, password)
            .unsubscribe(subscription_url)
            .await,
    )
}

// ── OSD ─────────────────────────────────────────────────────────────────────
//
// Two-call pattern in every wrapper: GetCapabilities → media_url, then
// the OSD operation. The cost is one extra round-trip per call which
// matters less here than for hot paths (snapshot loop) — the OSD UI is
// click-driven, not auto-refreshed.

#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn get_osds(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    profile_token: &str,
) -> Result<Vec<OsdConfiguration>, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    // Resolve the video source config token. Prefer the requested profile,
    // fall back to the first profile that has one — handles the case where
    // selected_profile signal is stale from another device, or where the
    // current profile is metadata-only without a video source. Mirrors
    // the Imaging tab's "use first viable" behaviour.
    let profiles = client
        .get_profiles(&media_url)
        .await
        .map_err(|e| e.to_string())?;
    let vsc_token = profiles
        .iter()
        .find(|p| p.token == profile_token)
        .and_then(|p| p.video_source_config_token.clone())
        .or_else(|| {
            profiles
                .iter()
                .find_map(|p| p.video_source_config_token.clone())
        })
        .ok_or("No profile with a video source configuration")?;
    trace_result(
        "GetOSDs",
        addr,
        client.get_osds(&media_url, Some(&vsc_token)).await,
    )
}

#[instrument(skip(username, password, osd), fields(addr, token = %osd.token))]
pub async fn set_osd(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    osd: &OsdConfiguration,
) -> Result<(), ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result("SetOSD", addr, client.set_osd(&media_url, osd).await)
}

#[instrument(skip(username, password, osd), fields(addr))]
pub async fn create_osd(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    osd: &OsdConfiguration,
) -> Result<String, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result("CreateOSD", addr, client.create_osd(&media_url, osd).await)
}

#[instrument(skip(username, password), fields(addr, osd_token))]
pub async fn delete_osd(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    osd_token: &str,
) -> Result<(), ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result(
        "DeleteOSD",
        addr,
        client.delete_osd(&media_url, osd_token).await,
    )
}

/// Fetch the camera's allowed OSD configuration options for the
/// selected profile's video source. Same fallback logic as
/// `get_osds`: try the requested profile, otherwise pick any with a
/// video source. The returned `OsdOptions` lists supported text
/// types, position types, date/time formats, and font size range —
/// driving the OSD editor's dropdowns to values the camera will
/// actually accept.
#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn get_osd_options(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    profile_token: &str,
) -> Result<OsdOptions, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    let profiles = client
        .get_profiles(&media_url)
        .await
        .map_err(|e| e.to_string())?;
    let vsc_token = profiles
        .iter()
        .find(|p| p.token == profile_token)
        .and_then(|p| p.video_source_config_token.clone())
        .or_else(|| {
            profiles
                .iter()
                .find_map(|p| p.video_source_config_token.clone())
        })
        .ok_or("No profile with a video source configuration")?;
    trace_result(
        "GetOSDOptions",
        addr,
        client.get_osd_options(&media_url, &vsc_token).await,
    )
}

/// Resolve the video source configuration token for a profile.
/// Used by the OSD UI when CREATING a new OSD — the new entry needs
/// to know which video source it attaches to. Same fallback logic as
/// `get_osds`: try the requested profile, otherwise pick any profile
/// that actually has a video source.
#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn get_video_source_config_token(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    profile_token: &str,
) -> Result<String, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    let profiles = client
        .get_profiles(&media_url)
        .await
        .map_err(|e| e.to_string())?;
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

// ── Profile management ──────────────────────────────────────────────────────

#[instrument(skip(username, password), fields(addr, name))]
pub async fn create_profile(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    name: &str,
) -> Result<MediaProfile, ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result(
        "CreateProfile",
        addr,
        client.create_profile(&media_url, name, None).await,
    )
}

#[instrument(skip(username, password), fields(addr, profile_token))]
pub async fn delete_profile(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    profile_token: &str,
) -> Result<(), ApiError> {
    let client = build_client(addr, username, password);
    let caps = client.get_capabilities().await.map_err(|e| e.to_string())?;
    let media_url = caps.media.url.ok_or("No media service URL")?;
    trace_result(
        "DeleteProfile",
        addr,
        client.delete_profile(&media_url, profile_token).await,
    )
}
