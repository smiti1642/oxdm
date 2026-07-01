use dioxus::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ── Theme ───────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Theme {
    Dark,
    Light,
    Classic,
}

impl Theme {
    pub fn next(self) -> Self {
        match self {
            Self::Dark => Self::Light,
            Self::Light => Self::Classic,
            Self::Classic => Self::Dark,
        }
    }

    pub fn css_class(self) -> &'static str {
        match self {
            Self::Dark => "shell theme-dark",
            Self::Light => "shell theme-light",
            Self::Classic => "shell theme-classic",
        }
    }
}

// ── Locale ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Locale {
    En,
    ZhTw,
    Ru,
}

impl Locale {
    pub fn next(self) -> Self {
        match self {
            Self::En => Self::ZhTw,
            Self::ZhTw => Self::Ru,
            Self::Ru => Self::En,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::En => "EN",
            Self::ZhTw => "\u{4E2D}",
            Self::Ru => "RU",
        }
    }
}

// ── Device list tab ─────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum DeviceListTab {
    Discovered,
    Manual,
}

// ── View ────────────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum View {
    Welcome,
    DeviceSettings,
    LiveVideo,
    ImagingSettings,
    PtzControl,
    Events,
    Osd,
    IoControl,
    Recordings,
    /// Global (device-independent) batch health check across selected devices,
    /// with an exportable cross-brand conformance report.
    HealthOverview,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTab {
    Identification,
    Network,
    Time,
    Users,
    Maintenance,
    Health,
}

// ── Auth status ─────────────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AuthStatus {
    #[default]
    Unknown,
    Ok,
    Failed,
}

// ── Toast notifications ─────────────────────────────────────────────────────

#[derive(Clone, Copy, Debug, PartialEq)]
#[allow(dead_code)]
pub enum ToastLevel {
    Success,
    Info,
    Warning,
    Error,
}

impl ToastLevel {
    pub fn css_class(self) -> &'static str {
        match self {
            Self::Success => "toast toast--success",
            Self::Info => "toast toast--info",
            Self::Warning => "toast toast--warning",
            Self::Error => "toast toast--error",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Toast {
    pub id: u32,
    pub level: ToastLevel,
    pub message: String,
}

// ── Confirm dialog ──────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct ConfirmDialog {
    pub title: String,
    pub message: String,
    pub confirm_label: String,
    pub cancel_label: String,
    pub dangerous: bool,
    pub on_confirm: EventHandler<()>,
}

// ── Credentials ─────────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Credentials {
    pub username: String,
    pub password: String,
}

impl Credentials {
    /// Convert to `(Option<&str>, Option<&str>)` for API calls.
    /// Returns `(None, None)` if username is empty.
    pub fn as_options(&self) -> (Option<&str>, Option<&str>) {
        if self.username.is_empty() {
            (None, None)
        } else {
            (Some(&self.username), Some(&self.password))
        }
    }
}

// ── Device entry ────────────────────────────────────────────────────────────

#[derive(Clone, Debug)]
pub struct DeviceEntry {
    pub name: String,
    /// Full ONVIF device service URL (e.g. http://192.168.1.10/onvif/device_service)
    pub addr: String,
    /// Human-readable IP address for display
    pub display_addr: String,
    pub firmware: String,
    pub location: String,
    pub online: bool,
    pub auth_status: AuthStatus,
    /// Whether this device was manually added (can have its own credentials)
    pub manual: bool,
    /// Per-device credentials override (only for manually added devices).
    pub credentials: Option<Credentials>,
    /// WS-Discovery `EndpointReference/Address` (typically `uuid:...`).
    /// Stable across IP changes — primary key for cross-scan merge of
    /// discovered devices. Empty for manually-added entries that have
    /// not yet been correlated with a discovery response.
    pub endpoint: String,
}

// ── HealthCheck groups ──────────────────────────────────────────────────────

/// A persisted pointer to a device inside a HealthCheck group. Not a live
/// `DeviceEntry` — the group survives scans / restarts / the device being
/// offline. Resolved back to a live device by `endpoint` (when non-empty)
/// else `addr`.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HealthDeviceRef {
    /// WS-Discovery endpoint (`uuid:…`); primary match key, stable across IP
    /// changes. May be empty for manually-added devices.
    #[serde(default)]
    pub endpoint: String,
    /// Full ONVIF device service URL; fallback match key + per-device cred key.
    pub addr: String,
    /// Cached display name, so offline members still render meaningfully.
    #[serde(default)]
    pub name: String,
}

/// A named, persisted collection of devices to batch-health-check together,
/// optionally carrying its own credentials (group-level + per-device override).
///
/// Credentials live in the keychain, never in `healthcheck.toml`, so the two
/// cred fields are `#[serde(skip)]` and hydrated at load time.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HealthGroup {
    /// Stable, immutable id (creds are keyed by it, so rename must not change it).
    pub id: String,
    pub name: String,
    #[serde(default, rename = "device")]
    pub devices: Vec<HealthDeviceRef>,
    /// Group-level credentials (keychain-only, never serialised to TOML).
    #[serde(skip)]
    pub credentials: Option<Credentials>,
    /// Per-device-in-group overrides, keyed by device `addr` (keychain-only).
    #[serde(skip)]
    pub device_credentials: HashMap<String, Credentials>,
}

/// Which tier of the group credential cascade is in effect for a device —
/// drives the per-row source badge in the Health view.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CredSource {
    /// Per-device-in-group override.
    Device,
    /// Group-level credentials.
    Group,
    /// Falls through to app-level `credentials_for` (device override → global).
    App,
}

/// Generate a stable, unique group id without a uuid crate: a unix-timestamp
/// base plus a uniqueness guard against same-second collisions. `time`'s
/// `now_utc` is always available (unlike `now_local`, which can error).
pub fn new_group_id(existing: &[HealthGroup]) -> String {
    let base = time::OffsetDateTime::now_utc().unix_timestamp();
    let mut id = format!("g-{base}");
    let mut n = 1;
    while existing.iter().any(|g| g.id == id) {
        id = format!("g-{base}-{n}");
        n += 1;
    }
    id
}

// ── Global context ──────────────────────────────────────────────────────────

/// Global app state passed via context to all components.
#[derive(Clone, Copy)]
pub struct Ctx {
    pub devices: Signal<Vec<DeviceEntry>>,
    pub selected: Signal<Option<usize>>,
    pub view: Signal<View>,
    pub settings_tab: Signal<SettingsTab>,
    pub scanning: Signal<bool>,
    pub theme: Signal<Theme>,
    pub locale: Signal<Locale>,
    pub toasts: Signal<Vec<Toast>>,
    pub next_toast_id: Signal<u32>,
    pub dialog: Signal<Option<ConfirmDialog>>,
    pub global_credentials: Signal<Credentials>,
    /// Persisted HealthCheck groups (device references + per-group credentials).
    pub health_groups: Signal<Vec<HealthGroup>>,
    /// Currently selected media profile token (for NVT operations).
    pub selected_profile: Signal<Option<String>>,
    /// Persist tracing output to disk. Toggled in the About dialog,
    /// saved to config.toml, applied on next launch.
    pub log_to_file: Signal<bool>,
    /// Refuse self-signed / invalid TLS certs on snapshot HTTPS calls.
    /// Toggled in the About dialog, saved to config.toml, applies
    /// immediately (next snapshot fetch reads the global atomic).
    pub tls_strict: Signal<bool>,
    /// Pending global keyboard shortcut. Producers (root onkeydown) write
    /// here; consumers (DeviceList, etc.) react via use_effect and clear
    /// the slot back to None. `Esc` is handled by individual modals via
    /// their own onkeydown — they have richer close semantics than a
    /// global signal can express cleanly.
    pub keyboard_action: Signal<Option<GlobalKey>>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GlobalKey {
    /// Ctrl+F / Cmd+F — focus the device list filter input.
    FocusSearch,
    /// F5 — kick off a WS-Discovery scan.
    Scan,
    /// Up arrow — move device list selection up by one.
    NavUp,
    /// Down arrow — move device list selection down by one.
    NavDown,
}

impl Ctx {
    /// Push a toast notification. It will be auto-dismissed by the ToastContainer.
    pub fn push_toast(&self, level: ToastLevel, message: impl Into<String>) {
        let id = *self.next_toast_id.peek();
        self.next_toast_id.clone().set(id + 1);
        self.toasts.clone().write().push(Toast {
            id,
            level,
            message: message.into(),
        });
    }

    pub fn dismiss_toast(&self, id: u32) {
        self.toasts.clone().write().retain(|t| t.id != id);
    }

    /// Get the effective credentials for a device (device override > global).
    pub fn credentials_for(&self, device: &DeviceEntry) -> Credentials {
        device
            .credentials
            .clone()
            .unwrap_or_else(|| self.global_credentials.peek().clone())
    }

    /// Credentials to use when health-checking `device` as part of `group`:
    /// per-device-in-group override → group-level creds → app default
    /// (`credentials_for`). An override with an empty username is treated as
    /// unset so it transparently falls through to the next tier.
    pub fn group_credentials_for(&self, group: &HealthGroup, device: &DeviceEntry) -> Credentials {
        if let Some(c) = group.device_credentials.get(&device.addr) {
            if !c.username.is_empty() {
                return c.clone();
            }
        }
        if let Some(c) = &group.credentials {
            if !c.username.is_empty() {
                return c.clone();
            }
        }
        self.credentials_for(device)
    }

    /// Which credential tier `group_credentials_for` will resolve to — for the
    /// per-row source badge.
    pub fn group_cred_source(&self, group: &HealthGroup, device: &DeviceEntry) -> CredSource {
        if group
            .device_credentials
            .get(&device.addr)
            .is_some_and(|c| !c.username.is_empty())
        {
            return CredSource::Device;
        }
        if group
            .credentials
            .as_ref()
            .is_some_and(|c| !c.username.is_empty())
        {
            return CredSource::Group;
        }
        CredSource::App
    }
}
