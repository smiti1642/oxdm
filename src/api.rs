use oxvif::{discovery, DeviceInfo, DiscoveredDevice, OnvifClient};
use std::time::Duration;

pub type ApiError = String;

/// Run WS-Discovery and return found devices.
pub async fn discover_devices() -> Result<Vec<DiscoveredDevice>, ApiError> {
    Ok(discovery::probe(Duration::from_secs(3)).await)
}

/// Connect to a camera and fetch basic device info.
#[allow(dead_code)]
pub async fn get_device_info(
    addr: &str,
    username: Option<&str>,
    password: Option<&str>,
) -> Result<DeviceInfo, ApiError> {
    let mut client = OnvifClient::new(addr);
    if let (Some(u), Some(p)) = (username, password) {
        client = client.with_credentials(u, p);
    }
    client.get_device_info().await.map_err(|e| e.to_string())
}
