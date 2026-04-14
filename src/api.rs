use oxvif::{
    discovery, Capabilities, DeviceInfo, DiscoveredDevice, DnsInformation, Hostname,
    NetworkGateway, NetworkInterface, NetworkProtocol, NtpInfo, OnvifClient, SystemDateTime, User,
};
use std::time::Duration;

pub type ApiError = String;

/// Build a client for a device, optionally with credentials.
fn build_client(addr: &str, username: Option<&str>, password: Option<&str>) -> OnvifClient {
    let mut client = OnvifClient::new(addr);
    if let (Some(u), Some(p)) = (username, password) {
        client = client.with_credentials(u, p);
    }
    client
}

// ── Discovery ───────────────────────────────────────────────────────────────

/// Run WS-Discovery and return found devices.
pub async fn discover_devices() -> Result<Vec<DiscoveredDevice>, ApiError> {
    Ok(discovery::probe(Duration::from_secs(3)).await)
}

// ── Device Info ─────────────────────────────────────────────────────────────

/// Connect to a camera and fetch basic device info.
pub async fn get_device_info(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<DeviceInfo, ApiError> {
    build_client(addr, username, password)
        .get_device_info()
        .await
        .map_err(|e| e.to_string())
}

/// Fetch device scopes (name, location, etc.).
pub async fn get_scopes(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<String>, ApiError> {
    build_client(addr, username, password)
        .get_scopes()
        .await
        .map_err(|e| e.to_string())
}

/// Fetch device capabilities.
#[allow(dead_code)]
pub async fn get_capabilities(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Capabilities, ApiError> {
    build_client(addr, username, password)
        .get_capabilities()
        .await
        .map_err(|e| e.to_string())
}

// ── Date / Time ─────────────────────────────────────────────────────────────

pub async fn get_system_date_and_time(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<SystemDateTime, ApiError> {
    build_client(addr, username, password)
        .get_system_date_and_time()
        .await
        .map_err(|e| e.to_string())
}

// ── Network ─────────────────────────────────────────────────────────────────

pub async fn get_hostname(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Hostname, ApiError> {
    build_client(addr, username, password)
        .get_hostname()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_network_interfaces(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<NetworkInterface>, ApiError> {
    build_client(addr, username, password)
        .get_network_interfaces()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_dns(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<DnsInformation, ApiError> {
    build_client(addr, username, password)
        .get_dns()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_ntp(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<NtpInfo, ApiError> {
    build_client(addr, username, password)
        .get_ntp()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_network_default_gateway(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<NetworkGateway, ApiError> {
    build_client(addr, username, password)
        .get_network_default_gateway()
        .await
        .map_err(|e| e.to_string())
}

pub async fn get_network_protocols(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<NetworkProtocol>, ApiError> {
    build_client(addr, username, password)
        .get_network_protocols()
        .await
        .map_err(|e| e.to_string())
}

// ── Users ───────────────────────────────────────────────────────────────────

pub async fn get_users(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<Vec<User>, ApiError> {
    build_client(addr, username, password)
        .get_users()
        .await
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn create_user(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    new_username: &str,
    new_password: &str,
    user_level: &str,
) -> Result<(), ApiError> {
    build_client(addr, username, password)
        .create_users(&[(new_username, new_password, user_level)])
        .await
        .map_err(|e| e.to_string())
}

#[allow(dead_code)]
pub async fn delete_user(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    target_username: &str,
) -> Result<(), ApiError> {
    build_client(addr, username, password)
        .delete_users(&[target_username])
        .await
        .map_err(|e| e.to_string())
}

// ── Maintenance ─────────────────────────────────────────────────────────────

pub async fn system_reboot(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<String, ApiError> {
    build_client(addr, username, password)
        .system_reboot()
        .await
        .map_err(|e| e.to_string())
}

pub async fn set_system_factory_default(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
    default_type: &str,
) -> Result<(), ApiError> {
    build_client(addr, username, password)
        .set_system_factory_default(default_type)
        .await
        .map_err(|e| e.to_string())
}
