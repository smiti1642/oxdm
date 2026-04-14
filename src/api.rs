use oxvif::{
    discovery, Capabilities, DeviceInfo, DiscoveredDevice, DnsInformation, Hostname,
    NetworkGateway, NetworkInterface, NetworkProtocol, NtpInfo, OnvifClient, SystemDateTime, User,
};
use std::time::Duration;
use tracing::{debug, error, info, instrument};

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

#[instrument(skip_all)]
pub async fn discover_devices() -> Result<Vec<DiscoveredDevice>, ApiError> {
    info!("WS-Discovery probe starting (3s timeout)");
    let devices = discovery::probe(Duration::from_secs(3)).await;
    info!(count = devices.len(), "WS-Discovery probe complete");
    Ok(devices)
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
