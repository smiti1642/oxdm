//! Settings and device persistence.
//!
//! - `~/.oxdm/config.toml`: theme, locale (no credentials)
//! - `~/.oxdm/devices.toml`: manually added devices (per-device creds in keychain)
//! - System keychain: global credentials + per-device credential overrides

use crate::state::{Credentials, DeviceEntry, Locale, Theme};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

const KEYRING_SERVICE: &str = "com.oxdm";
const KEYRING_GLOBAL_USER: &str = "global";

// ── On-disk structures ──────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub locale: String,
    // Legacy fields — read for migration, not written
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
    pub has_credentials: bool,
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

// ── Keychain helpers ────────────────────────────────────────────────────────

fn keyring_save(key: &str, username: &str, password: &str) {
    let value = format!("{username}\n{password}");
    match keyring::Entry::new(KEYRING_SERVICE, key) {
        Ok(entry) => {
            if let Err(e) = entry.set_password(&value) {
                warn!(error = %e, key, "Keychain save failed, credentials not persisted");
            }
        }
        Err(e) => warn!(error = %e, key, "Keychain entry creation failed"),
    }
}

fn keyring_load(key: &str) -> Option<(String, String)> {
    let entry = keyring::Entry::new(KEYRING_SERVICE, key).ok()?;
    let value = entry.get_password().ok()?;
    let mut lines = value.splitn(2, '\n');
    let username = lines.next()?.to_string();
    let password = lines.next().unwrap_or("").to_string();
    if username.is_empty() {
        return None;
    }
    Some((username, password))
}

fn keyring_delete(key: &str) {
    if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, key) {
        let _ = entry.delete_credential();
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

/// Load global credentials from keychain, falling back to legacy config.toml.
pub fn load_global_credentials(cfg: &ConfigFile) -> Credentials {
    // Try keychain first
    if let Some((u, p)) = keyring_load(KEYRING_GLOBAL_USER) {
        debug!("Loaded global credentials from keychain");
        return Credentials {
            username: u,
            password: p,
        };
    }
    // Fallback: migrate from legacy config.toml
    if !cfg.username.is_empty() {
        info!("Migrating credentials from config.toml to keychain");
        keyring_save(KEYRING_GLOBAL_USER, &cfg.username, &cfg.password);
        return Credentials {
            username: cfg.username.clone(),
            password: cfg.password.clone(),
        };
    }
    Credentials::default()
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
                        let display_addr = crate::util::extract_ip(&r.addr);
                        let creds = if r.has_credentials {
                            keyring_load(&r.addr).map(|(u, p)| Credentials {
                                username: u,
                                password: p,
                            })
                        } else {
                            None
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

    // Save theme/locale to config.toml (no credentials)
    if let Some(path) = config_path() {
        #[derive(Serialize)]
        struct ConfigOut {
            theme: String,
            locale: String,
        }
        let cfg = ConfigOut {
            theme: theme_to_str(theme).to_string(),
            locale: locale_to_str(locale).to_string(),
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

    // Save credentials to keychain
    if creds.username.is_empty() {
        keyring_delete(KEYRING_GLOBAL_USER);
    } else {
        keyring_save(KEYRING_GLOBAL_USER, &creds.username, &creds.password);
    }
}

pub fn save_devices(devices: &[DeviceEntry]) {
    ensure_dir();
    let Some(path) = devices_path() else { return };

    let records: Vec<DeviceRecord> = devices
        .iter()
        .filter(|d| d.manual)
        .map(|d| {
            let has_creds = d.credentials.is_some();
            // Save per-device credentials to keychain
            if let Some(c) = &d.credentials {
                keyring_save(&d.addr, &c.username, &c.password);
            } else {
                keyring_delete(&d.addr);
            }
            DeviceRecord {
                name: d.name.clone(),
                addr: d.addr.clone(),
                has_credentials: has_creds,
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
