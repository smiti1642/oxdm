//! Background device operations: firmware fetching and auth verification.
//!
//! Extracted from `device_list.rs` to keep UI components lean.

use crate::api;
use crate::state::{AuthStatus, Ctx, DeviceEntry};
use dioxus::prelude::*;

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
            let status = match api::get_device_info(&addr, u, p).await {
                Ok(info) => {
                    if let Some(d) = devices.write().iter_mut().find(|d| d.addr == addr) {
                        d.firmware = info.firmware_version;
                    }
                    AuthStatus::Ok
                }
                Err(_) => AuthStatus::Failed,
            };
            if let Some(d) = devices.write().iter_mut().find(|d| d.addr == addr) {
                d.auth_status = status;
            }
        });
    }
}

/// Re-verify auth status for all devices (called when credentials change).
pub fn reverify_auth(ctx: Ctx, mut devices: Signal<Vec<DeviceEntry>>) {
    let snapshot: Vec<(String, bool, crate::state::Credentials)> = devices
        .peek()
        .iter()
        .map(|d| (d.addr.clone(), d.manual, ctx.credentials_for(d)))
        .collect();

    for (addr, is_manual, creds) in snapshot {
        spawn(async move {
            let (u, p) = creds.as_options();
            let status = match api::get_device_info(&addr, u, p).await {
                Ok(info) => {
                    if !is_manual {
                        if let Some(d) = devices.write().iter_mut().find(|d| d.addr == addr) {
                            d.firmware = info.firmware_version;
                        }
                    }
                    AuthStatus::Ok
                }
                Err(_) => AuthStatus::Failed,
            };
            if let Some(d) = devices.write().iter_mut().find(|d| d.addr == addr) {
                d.auth_status = status;
            }
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
    spawn(async move {
        let (u, p) = creds.as_options();
        let status = match api::get_device_info(&addr, u, p).await {
            Ok(_) => AuthStatus::Ok,
            Err(_) => AuthStatus::Failed,
        };
        if let Some(d) = devices.write().iter_mut().find(|d| d.addr == addr) {
            d.auth_status = status;
        }
    });
}
