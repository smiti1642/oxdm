use dioxus::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Welcome,
    DeviceInfo,
    LiveVideo,
    NetworkSettings,
    TimeSettings,
    UserManagement,
    ImagingSettings,
    PtzControl,
    Events,
    Maintenance,
}

#[derive(Clone, Debug)]
pub struct DeviceEntry {
    pub name: String,
    /// Full ONVIF device service URL (e.g. http://192.168.1.10/onvif/device_service)
    pub addr: String,
    /// Human-readable IP address for display
    pub display_addr: String,
    pub firmware: String,
    pub online: bool,
}

/// Global app state passed via context to all components.
#[derive(Clone, Copy)]
pub struct Ctx {
    pub devices: Signal<Vec<DeviceEntry>>,
    pub selected: Signal<Option<usize>>,
    pub view: Signal<View>,
    pub scanning: Signal<bool>,
}
