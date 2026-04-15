//! Settings and device persistence to `~/.oxdm/`.
//!
//! - `config.toml`: theme, locale, global credentials
//! - `devices.toml`: manually added devices

use crate::state::{Credentials, DeviceEntry, Locale, Theme};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, error, info};

// ── On-disk structures ──────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub locale: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct DevicesFile {
    #[serde(default)]
    pub devices: Vec<DeviceRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRecord {
    pub name: String,
    pub addr: String,
    #[serde(default)]
    pub username: String,
    #[serde(default)]
    pub password: String,
}

// ── Paths ───────────────────────────────────────────────────────────────────

fn oxdm_dir() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxdm"))
}

fn config_path() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("config.toml"))
}

fn devices_path() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("devices.toml"))
}

fn ensure_dir() {
    if let Some(dir) = oxdm_dir() {
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
    }
}

// ── Load ────────────────────────────────────────────────────────────────────

pub fn load_config() -> ConfigFile {
    let Some(path) = config_path() else {
        return ConfigFile::default();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str(&content) {
            Ok(cfg) => {
                info!("Loaded config from {}", path.display());
                cfg
            }
            Err(e) => {
                error!(error = %e, "Failed to parse config.toml");
                ConfigFile::default()
            }
        },
        Err(_) => {
            debug!("No config.toml found, using defaults");
            ConfigFile::default()
        }
    }
}

pub fn load_devices() -> Vec<DeviceEntry> {
    let Some(path) = devices_path() else {
        return Vec::new();
    };
    match std::fs::read_to_string(&path) {
        Ok(content) => match toml::from_str::<DevicesFile>(&content) {
            Ok(file) => {
                info!(
                    count = file.devices.len(),
                    "Loaded devices from {}",
                    path.display()
                );
                file.devices
                    .into_iter()
                    .map(|r| {
                        let display_addr = extract_ip(&r.addr);
                        let creds = if r.username.is_empty() && r.password.is_empty() {
                            None
                        } else {
                            Some(Credentials {
                                username: r.username,
                                password: r.password,
                            })
                        };
                        DeviceEntry {
                            name: r.name,
                            display_addr,
                            addr: r.addr,
                            firmware: String::new(),
                            location: String::new(),
                            online: false,
                            auth_status: Default::default(),
                            manual: true,
                            credentials: creds,
                        }
                    })
                    .collect()
            }
            Err(e) => {
                error!(error = %e, "Failed to parse devices.toml");
                Vec::new()
            }
        },
        Err(_) => {
            debug!("No devices.toml found");
            Vec::new()
        }
    }
}

// ── Save ────────────────────────────────────────────────────────────────────

pub fn save_config(theme: Theme, locale: Locale, creds: &Credentials) {
    ensure_dir();
    let Some(path) = config_path() else { return };

    let cfg = ConfigFile {
        theme: theme_to_str(theme).to_string(),
        locale: locale_to_str(locale).to_string(),
        username: creds.username.clone(),
        password: creds.password.clone(),
    };

    match toml::to_string_pretty(&cfg) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                error!(error = %e, "Failed to write config.toml");
            } else {
                debug!("Saved config to {}", path.display());
            }
        }
        Err(e) => error!(error = %e, "Failed to serialize config"),
    }
}

pub fn save_devices(devices: &[DeviceEntry]) {
    ensure_dir();
    let Some(path) = devices_path() else { return };

    let records: Vec<DeviceRecord> = devices
        .iter()
        .filter(|d| d.manual)
        .map(|d| {
            let (u, p) = d
                .credentials
                .as_ref()
                .map(|c| (c.username.clone(), c.password.clone()))
                .unwrap_or_default();
            DeviceRecord {
                name: d.name.clone(),
                addr: d.addr.clone(),
                username: u,
                password: p,
            }
        })
        .collect();

    let file = DevicesFile { devices: records };
    match toml::to_string_pretty(&file) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                error!(error = %e, "Failed to write devices.toml");
            } else {
                debug!("Saved {} manual device(s)", file.devices.len());
            }
        }
        Err(e) => error!(error = %e, "Failed to serialize devices"),
    }
}

// ── Conversions ─────────────────────────────────────────────────────────────

pub fn theme_from_str(s: &str) -> Theme {
    match s {
        "light" => Theme::Light,
        "classic" => Theme::Classic,
        _ => Theme::Dark,
    }
}

fn theme_to_str(t: Theme) -> &'static str {
    match t {
        Theme::Dark => "dark",
        Theme::Light => "light",
        Theme::Classic => "classic",
    }
}

pub fn locale_from_str(s: &str) -> Locale {
    match s {
        "zh_tw" => Locale::ZhTw,
        "ru" => Locale::Ru,
        _ => Locale::En,
    }
}

fn locale_to_str(l: Locale) -> &'static str {
    match l {
        Locale::En => "en",
        Locale::ZhTw => "zh_tw",
        Locale::Ru => "ru",
    }
}

fn extract_ip(addr: &str) -> String {
    let stripped = addr
        .strip_prefix("http://")
        .or_else(|| addr.strip_prefix("https://"))
        .unwrap_or(addr);
    stripped
        .split('/')
        .next()
        .and_then(|h| h.split(':').next())
        .unwrap_or(addr)
        .to_string()
}
