use dioxus::prelude::*;

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

    #[allow(dead_code)]
    pub fn icon(self) -> &'static str {
        match self {
            Self::Dark => "moon",
            Self::Light => "sun",
            Self::Classic => "monitor",
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTab {
    Identification,
    Network,
    Time,
    Users,
    Maintenance,
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

    /// Icon name for the Icon component.
    #[allow(dead_code)]
    pub fn icon_name(self) -> &'static str {
        match self {
            Self::Success => "check",
            Self::Info => "info",
            Self::Warning => "alert-triangle",
            Self::Error => "x",
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

#[derive(Clone, Debug, Default)]
pub struct Credentials {
    pub username: String,
    pub password: String,
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
    /// Whether this device was manually added (can have its own credentials)
    pub manual: bool,
    /// Per-device credentials override (only for manually added devices).
    /// Used by api layer when connecting to this device.
    #[allow(dead_code)]
    pub credentials: Option<Credentials>,
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
    #[allow(dead_code)]
    pub fn credentials_for(&self, device: &DeviceEntry) -> Credentials {
        device
            .credentials
            .clone()
            .unwrap_or_else(|| self.global_credentials.peek().clone())
    }
}
