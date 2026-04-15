use oxvif::{
    Capabilities, DeviceInfo, DiscoveredDevice, DnsInformation, Hostname, MediaProfile,
    NetworkGateway, NetworkInterface, NetworkProtocol, NtpInfo, OnvifClient, SnapshotUri,
    StreamUri, SystemDateTime, User,
};
use std::time::Duration;
use tracing::{debug, error, info, instrument, warn};

pub type ApiError = String;

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

const PROBE_ROUNDS: usize = 3;
const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const PROBE_INTERVAL: Duration = Duration::from_millis(800);

/// Run WS-Discovery probes on all network interfaces, multiple rounds.
///
/// Uses our own multi-NIC implementation instead of oxvif's single-interface
/// `probe()`, ensuring cameras on all subnets are discovered.
#[instrument(skip_all)]
pub async fn discover_devices() -> Result<Vec<DiscoveredDevice>, ApiError> {
    Ok(crate::discovery::probe_all_interfaces(PROBE_ROUNDS, PROBE_TIMEOUT, PROBE_INTERVAL).await)
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
    let http = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .timeout(Duration::from_secs(5))
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

        // Try Digest auth if challenge is present
        if www_auth.to_lowercase().contains("digest") {
            info!(snapshot_url, www_authenticate = %www_auth, "Attempting Digest auth");
            match try_digest_auth(&http, snapshot_url, u, p, &www_auth).await {
                Ok(resp) if resp.status().is_success() => {
                    return snapshot_response_to_data_uri(resp, snapshot_url).await;
                }
                Ok(resp) => {
                    warn!(snapshot_url, status = %resp.status(), "Digest auth rejected");
                }
                Err(e) => {
                    debug!(snapshot_url, error = %e, "Digest auth error");
                }
            }
        }

        // Try Basic auth (works for many cameras, also covers non-401 cases)
        debug!(snapshot_url, "Attempting Basic auth");
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

/// Extract URI path from a full URL for Digest auth computation.
fn url_to_path(url: &str) -> &str {
    url.find("://")
        .and_then(|i| url[i + 3..].find('/').map(|j| &url[i + 3 + j..]))
        .unwrap_or("/")
}

/// Manually compute Digest Auth header and retry.
async fn try_digest_auth(
    http: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
    www_authenticate: &str,
) -> Result<reqwest::Response, ApiError> {
    let uri_path = url_to_path(url);

    // Log credential hint for diagnostics (password length + first/last char)
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
    info!(username, pass_hint = %pass_hint, "Digest auth credentials");

    let context = digest_auth::AuthContext::new(username, password, uri_path);
    let mut prompt = digest_auth::parse(www_authenticate).map_err(|e| {
        error!(error = %e, www_authenticate, "Failed to parse WWW-Authenticate");
        e.to_string()
    })?;
    let answer = prompt.respond(&context).map_err(|e| {
        error!(error = %e, "Failed to compute Digest response");
        e.to_string()
    })?;
    // digest_auth crate emits `qop=auth` (unquoted per RFC 2617), but some
    // cameras (e.g. Hikvision) require `qop="auth"` (quoted). Patch the
    // header to quote the qop value for maximum compatibility.
    let header_val = answer
        .to_header_string()
        .replace("qop=auth", r#"qop="auth""#);
    info!(
        uri_path,
        authorization = %header_val,
        "Sending Digest auth request"
    );
    http.get(url)
        .header("Authorization", header_val)
        .send()
        .await
        .map_err(|e| e.to_string())
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
    debug!(
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

// ── Date / Time ─────────────────────────────────────────────────────────────

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

#[allow(dead_code)]
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

#[allow(dead_code)]
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
