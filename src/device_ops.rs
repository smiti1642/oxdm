//! Background device operations: firmware fetching and auth verification.
//!
//! Extracted from `device_list.rs` to keep UI components lean.
//!
//! Auth status is gated on `GetProfiles` succeeding, not just
//! `GetDeviceInformation`. Some cameras (Hikvision OEMs, certain GeoVision
//! models) expose `GetDeviceInformation` anonymously but require credentials
//! for the media service — under the looser check those would light up green
//! and then fail the moment the user clicked into them.

use crate::api;
use crate::state::{AuthStatus, Credentials, Ctx, DeviceEntry};
use dioxus::prelude::*;

/// Probe a device: pull firmware via `GetDeviceInformation` and verify auth
/// via `GetProfiles` (which requires both `GetCapabilities` and a working
/// media service). Returned `firmware` is `Some` only when the device-info
/// call succeeded. Returned `auth_status` reflects the profiles call alone.
async fn probe_device(
    addr: &str,
    u: Option<&str>,
    p: Option<&str>,
) -> (AuthStatus, Option<String>) {
    let (info_res, profiles_res) = tokio::join!(
        api::get_device_info(addr, u, p),
        api::get_profiles(addr, u, p),
    );
    let firmware = info_res.ok().map(|i| i.firmware_version);
    let auth_status = if profiles_res.is_ok() {
        AuthStatus::Ok
    } else {
        AuthStatus::Failed
    };
    (auth_status, firmware)
}

/// Apply a probe result to the device entry matching `addr`. `firmware` is
/// only written for non-manual devices (manual entries keep their
/// user-supplied display name and don't pull firmware via background ops).
fn apply_probe(
    devices: &mut Signal<Vec<DeviceEntry>>,
    addr: &str,
    auth_status: AuthStatus,
    firmware: Option<String>,
    is_manual: bool,
) {
    let mut guard = devices.write();
    if let Some(d) = guard.iter_mut().find(|d| d.addr == addr) {
        d.auth_status = auth_status;
        if !is_manual {
            if let Some(fw) = firmware {
                d.firmware = fw;
            }
        }
    }
}

/// After scan, fetch firmware version and verify auth for each discovered device.
/// Uses addr-based matching to avoid index invalidation races.
pub fn fetch_firmware_for_all(ctx: Ctx, mut devices: Signal<Vec<DeviceEntry>>) {
    let creds = ctx.global_credentials.peek().clone();
    let addrs: Vec<String> = devices
        .peek()
        .iter()
        .filter(|d| !d.manual)
        .map(|d| d.addr.clone())
        .collect();

    for addr in addrs {
        let creds = creds.clone();
        spawn(async move {
            let (u, p) = creds.as_options();
            let (auth_status, firmware) = probe_device(&addr, u, p).await;
            apply_probe(&mut devices, &addr, auth_status, firmware, false);
        });
    }
}

/// Re-verify auth status for all devices (called when credentials change).
pub fn reverify_auth(ctx: Ctx, mut devices: Signal<Vec<DeviceEntry>>) {
    let snapshot: Vec<(String, bool, Credentials)> = devices
        .peek()
        .iter()
        .map(|d| (d.addr.clone(), d.manual, ctx.credentials_for(d)))
        .collect();

    for (addr, is_manual, creds) in snapshot {
        spawn(async move {
            let (u, p) = creds.as_options();
            let (auth_status, firmware) = probe_device(&addr, u, p).await;
            apply_probe(&mut devices, &addr, auth_status, firmware, is_manual);
        });
    }
}

/// Re-verify auth for a single device by index.
pub fn reverify_device(ctx: Ctx, mut devices: Signal<Vec<DeviceEntry>>, idx: usize) {
    let Some(dev) = devices.peek().get(idx).cloned() else {
        return;
    };
    let creds = ctx.credentials_for(&dev);
    let addr = dev.addr.clone();
    let is_manual = dev.manual;
    spawn(async move {
        let (u, p) = creds.as_options();
        let (auth_status, firmware) = probe_device(&addr, u, p).await;
        apply_probe(&mut devices, &addr, auth_status, firmware, is_manual);
    });
}
