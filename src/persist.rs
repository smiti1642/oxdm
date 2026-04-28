//! Settings and device persistence.
//!
//! - `~/.oxdm/config.toml`: theme, locale (no credentials)
//! - `~/.oxdm/devices.toml`: manually added devices (per-device creds in keychain)
//! - System keychain: global credentials + per-device credential overrides

use crate::state::{Credentials, DeviceEntry, Locale, Theme};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, error, info, warn};

const KEYRING_SERVICE: &str = "com.oxdm";
const KEYRING_USER: &str = "credentials";
const CREDS_KEY_GLOBAL: &str = "__global__";

// ── On-disk structures ──────────────────────────────────────────────────────

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ConfigFile {
    #[serde(default)]
    pub theme: String,
    #[serde(default)]
    pub locale: String,
    /// Persist tracing output to `~/.oxdm/logs/oxdm.log.*`. Defaults to
    /// `false` so a fresh install doesn't quietly start writing log files
    /// most users won't read; the About dialog has the toggle.
    #[serde(default)]
    pub log_to_file: bool,
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

// ── Keychain helpers (single entry for all credentials) ─────────────────────
//
// All credentials are stored as a single JSON blob in one keychain entry
// to avoid multiple macOS Keychain permission prompts.
//
// Format: `{ "__global__": [user, pass], "addr1": [user, pass], ... }`

type CredsMap = HashMap<String, (String, String)>;

fn keyring_load_all() -> CredsMap {
    let entry = match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(e) => e,
        Err(e) => {
            warn!(error = %e, "Keychain entry creation failed");
            return HashMap::new();
        }
    };
    let json = match entry.get_password() {
        Ok(v) => v,
        Err(_) => return HashMap::new(),
    };
    match serde_json::from_str::<HashMap<String, (String, String)>>(&json) {
        Ok(map) => map,
        Err(e) => {
            warn!(error = %e, "Failed to parse keychain credentials JSON");
            HashMap::new()
        }
    }
}

fn keyring_save_all(map: &CredsMap) {
    let json = match serde_json::to_string(map) {
        Ok(j) => j,
        Err(e) => {
            error!(error = %e, "Failed to serialize credentials");
            return;
        }
    };
    match keyring::Entry::new(KEYRING_SERVICE, KEYRING_USER) {
        Ok(entry) => {
            if let Err(e) = entry.set_password(&json) {
                warn!(error = %e, "Keychain save failed, credentials not persisted");
            }
        }
        Err(e) => warn!(error = %e, "Keychain entry creation failed"),
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
/// Also returns the loaded keychain map so `load_devices` can reuse it
/// without a second keychain access.
pub fn load_all_credentials(cfg: &ConfigFile) -> (Credentials, CredsMap) {
    let mut map = keyring_load_all();

    // Try keychain first
    if let Some((u, p)) = map.get(CREDS_KEY_GLOBAL) {
        debug!("Loaded global credentials from keychain");
        let creds = Credentials {
            username: u.clone(),
            password: p.clone(),
        };
        return (creds, map);
    }

    // Fallback: migrate from legacy config.toml
    if !cfg.username.is_empty() {
        info!("Migrating credentials from config.toml to keychain");
        map.insert(
            CREDS_KEY_GLOBAL.to_string(),
            (cfg.username.clone(), cfg.password.clone()),
        );
        keyring_save_all(&map);
        let creds = Credentials {
            username: cfg.username.clone(),
            password: cfg.password.clone(),
        };
        return (creds, map);
    }

    (Credentials::default(), map)
}

pub fn load_devices(creds_map: &CredsMap) -> Vec<DeviceEntry> {
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
                            creds_map.get(&r.addr).map(|(u, p)| Credentials {
                                username: u.clone(),
                                password: p.clone(),
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
                            endpoint: String::new(),
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

pub fn save_config(theme: Theme, locale: Locale, log_to_file: bool) {
    ensure_dir();

    // Save theme/locale/log preference to config.toml (no credentials)
    if let Some(path) = config_path() {
        #[derive(Serialize)]
        struct ConfigOut {
            theme: String,
            locale: String,
            log_to_file: bool,
        }
        let cfg = ConfigOut {
            theme: theme_to_str(theme).to_string(),
            locale: locale_to_str(locale).to_string(),
            log_to_file,
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
}

/// Save all credentials (global + per-device) to a single keychain entry,
/// and save manual device records to devices.toml.
pub fn save_credentials_and_devices(global_creds: &Credentials, devices: &[DeviceEntry]) {
    ensure_dir();

    // Build a single credentials map for the keychain
    let mut map: CredsMap = HashMap::new();

    if !global_creds.username.is_empty() {
        map.insert(
            CREDS_KEY_GLOBAL.to_string(),
            (global_creds.username.clone(), global_creds.password.clone()),
        );
    }

    let records: Vec<DeviceRecord> = devices
        .iter()
        .filter(|d| d.manual)
        .map(|d| {
            let has_creds = d.credentials.is_some();
            if let Some(c) = &d.credentials {
                map.insert(d.addr.clone(), (c.username.clone(), c.password.clone()));
            }
            DeviceRecord {
                name: d.name.clone(),
                addr: d.addr.clone(),
                has_credentials: has_creds,
            }
        })
        .collect();

    // Single keychain write for all credentials
    keyring_save_all(&map);

    // Save devices.toml
    let Some(path) = devices_path() else { return };
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
