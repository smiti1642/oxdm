//! Settings and device persistence.
//!
//! - `~/.oxdm/config.toml`: theme, locale (no credentials)
//! - `~/.oxdm/devices.toml`: manually added devices (per-device creds in keychain)
//! - System keychain: global credentials + per-device credential overrides

use crate::state::{Credentials, DeviceEntry, HealthGroup, Locale, Theme};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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
    /// When `true`, snapshot HTTPS connections refuse self-signed and
    /// otherwise-invalid TLS certificates. Default is `false` because
    /// most IP cameras ship a self-signed cert; flipping this on without
    /// thinking would break HTTPS snapshots. Toggle in About.
    #[serde(default)]
    pub tls_strict: bool,
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

/// `healthcheck.toml` wrapper. `HealthGroup`'s credential fields are
/// `#[serde(skip)]`, so this file never contains secrets (they live in the
/// keychain blob) — same guarantee as `devices.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
struct HealthGroupsFile {
    #[serde(default, rename = "group")]
    groups: Vec<HealthGroup>,
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

fn healthcheck_path() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("healthcheck.toml"))
}

fn ensure_dir() {
    if let Some(dir) = oxdm_dir() {
        if !dir.exists() {
            let _ = std::fs::create_dir_all(&dir);
        }
    }
}

// ── Health report device-ref salt ───────────────────────────────────────────

fn ref_salt_path() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("health-ref-salt"))
}

/// A fresh 64-bit value seeded from OS entropy (`RandomState` seeds from the
/// platform CSPRNG). Used to bootstrap the persisted salt.
fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u8(0);
    h.finish()
}

/// A stable, secret, per-user salt for pseudonymising device references in
/// exported health reports. Generated once and persisted to
/// `~/.oxdm/health-ref-salt`; it never leaves the machine. This keeps a
/// device's `device_ref` stable across scans (so a reader can track "was this
/// fixed since last time?") while being unlinkable across users and not
/// brute-forceable back to a real address/serial the way a bare hash would be.
pub fn health_ref_salt() -> u64 {
    let Some(path) = ref_salt_path() else {
        // No home dir: fall back to a per-run salt — redaction still holds, the
        // ref just won't be stable across runs.
        return random_u64();
    };
    if let Ok(s) = std::fs::read_to_string(&path) {
        if let Ok(v) = u64::from_str_radix(s.trim(), 16) {
            return v;
        }
    }
    let salt = random_u64();
    ensure_dir();
    if let Err(e) = std::fs::write(&path, format!("{salt:016x}")) {
        warn!(error = %e, "could not persist health-ref salt; device_refs won't be stable across runs");
    }
    salt
}

// ── Health baselines (one JSON report per device) ───────────────────────────

/// Directory where per-device baseline `HealthReport`s live. One file per
/// `(scheme, host[:port], path)` triple, sanitized to a safe filename.
fn baseline_dir() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("baselines"))
}

/// Turn a device URL into something safe to use as a filename. We don't
/// hash because filenames stay human-recognizable on disk (helpful when
/// the user wants to back up / share a specific device's baseline).
fn sanitize_addr_for_file(addr: &str) -> String {
    let trimmed = addr
        .trim_start_matches("https://")
        .trim_start_matches("http://");
    trimmed
        .chars()
        .map(|c| match c {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '-' => c,
            _ => '_',
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string()
}

fn baseline_path(addr: &str) -> Option<PathBuf> {
    baseline_dir().map(|d| d.join(format!("{}.json", sanitize_addr_for_file(addr))))
}

/// Load a previously-saved baseline `HealthReport` for `addr`. Returns
/// `None` if the file doesn't exist or fails to parse (a stale or
/// corrupt baseline shouldn't break the Diagnostics tab — the UI just
/// proceeds without a baseline).
pub fn read_baseline(addr: &str) -> Option<oxvif::HealthReport> {
    let path = baseline_path(addr)?;
    let json = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<oxvif::HealthReport>(&json) {
        Ok(r) => Some(r),
        Err(e) => {
            warn!(error = %e, path = %path.display(), "stale baseline ignored");
            None
        }
    }
}

/// Persist `report` as the baseline for `addr`. Returns the path it was
/// written to so the UI can show "saved to …".
pub fn write_baseline(addr: &str, report: &oxvif::HealthReport) -> std::io::Result<PathBuf> {
    let path = baseline_path(addr)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir unavailable"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, report.to_json_pretty())?;
    Ok(path)
}

// ── Camera clones (one fixtures.json per cloned device) ──────────────────────
// One subdirectory per clone label, keyed by the sanitized label so a saved
// clone can be listed and re-served later.

/// Directory holding every recorded clone, one subdirectory per clone label.
fn clones_dir() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("clones"))
}

/// The on-disk subdirectory *name* for a clone label (sanitized). Matches the
/// entries [`list_clones`] returns, so a running clone (whose label is
/// `FixtureStore::device`) can be correlated with what's saved on disk.
pub fn clone_dir_name(label: &str) -> String {
    sanitize_addr_for_file(label)
}

/// Directory for a single clone's `fixtures.json`, keyed by a sanitized label
/// (`~/.oxdm/clones/<label>/`). Pass it to
/// [`oxvif::metamorph::FixtureStore::save`] / `load`.
pub fn clone_dir(label: &str) -> Option<PathBuf> {
    clones_dir().map(|d| d.join(sanitize_addr_for_file(label)))
}

/// Delete a recorded clone from disk, by the directory name [`list_clones`]
/// returns. Removes the whole `~/.oxdm/clones/<dir>/` subtree.
///
/// Takes the on-disk directory name rather than the clone label, so a caller
/// holding a row from [`list_clones`] cannot re-sanitize an already-sanitized
/// name and miss.
///
/// `dir_name` must be exactly one normal path component. This is a
/// `remove_dir_all`, so the check is deliberately a whitelist rather than a
/// scan for `..`: comparing `root.join(name).parent()` against `root` is *not*
/// enough, because `Path::parent` is purely lexical — `clones/..` has parent
/// `clones` and would pass, then delete `~/.oxdm` itself.
pub fn delete_clone(dir_name: &str) -> std::io::Result<()> {
    let name = clone_component(dir_name)?;
    let root = clones_dir()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir unavailable"))?;
    std::fs::remove_dir_all(root.join(name))
}

/// Accept `dir_name` only if it is exactly one normal path component, and
/// return it. Split out from [`delete_clone`] so the rule can be tested
/// without a filesystem anywhere near it.
fn clone_component(dir_name: &str) -> std::io::Result<&std::ffi::OsStr> {
    use std::path::Component;

    let bad = || std::io::Error::new(std::io::ErrorKind::InvalidInput, "not a clone directory");
    let mut components = Path::new(dir_name).components();
    let Some(Component::Normal(name)) = components.next() else {
        return Err(bad());
    };
    if components.next().is_some() {
        return Err(bad());
    }
    Ok(name)
}

/// Names of every recorded clone on disk (subdirectories that actually contain
/// a `fixtures.json`).
pub fn list_clones() -> Vec<String> {
    let Some(dir) = clones_dir() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(&dir) else {
        return Vec::new();
    };
    entries
        .filter_map(|e| e.ok())
        .filter(|e| e.path().join("fixtures.json").exists())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect()
}

/// File modification time of the saved baseline, formatted for the UI
/// ("yyyy-mm-dd hh:mm"). Returns `None` if there is no baseline or the
/// mtime is unavailable / unreadable.
pub fn baseline_saved_at(addr: &str) -> Option<String> {
    mtime_label(&baseline_path(addr)?)
}

/// A file's mtime as "yyyy-mm-dd hh:mm" in local time, for the small "saved
/// at" note under a header. `None` if the file is missing or its mtime is
/// unreadable.
fn mtime_label(path: &std::path::Path) -> Option<String> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    let dt = time::OffsetDateTime::from(modified);
    let local =
        dt.to_offset(time::UtcOffset::current_local_offset().unwrap_or(time::UtcOffset::UTC));
    // YYYY-MM-DD HH:MM — fits in the small note slot without seconds.
    Some(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}",
        local.year(),
        u8::from(local.month()),
        local.day(),
        local.hour(),
        local.minute(),
    ))
}

// ── Quirk baselines (one JSON QuirkReport per device) ───────────────────────
// Per device, not per app: "did this camera's quirks change?" is a question
// about one camera over time. Kept in a directory of its own so a quirk
// baseline and a health baseline for the same address cannot collide.

fn quirk_baseline_dir() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("quirk-baselines"))
}

fn quirk_baseline_path(addr: &str) -> Option<PathBuf> {
    quirk_baseline_dir().map(|d| d.join(format!("{}.json", sanitize_addr_for_file(addr))))
}

/// Load a previously-saved baseline `QuirkReport` for `addr`. `None` when
/// there is none or it no longer parses — a stale baseline must not break the
/// Quirks tab, it just leaves the diff section out.
pub fn read_quirk_baseline(addr: &str) -> Option<oxvif::metamorph::QuirkReport> {
    let path = quirk_baseline_path(addr)?;
    let json = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<oxvif::metamorph::QuirkReport>(&json) {
        Ok(r) => Some(r),
        Err(e) => {
            warn!(error = %e, path = %path.display(), "stale quirk baseline ignored");
            None
        }
    }
}

/// Persist `report` as the quirk baseline for `addr`. Returns the path so the
/// UI can report where it landed.
pub fn write_quirk_baseline(
    addr: &str,
    report: &oxvif::metamorph::QuirkReport,
) -> std::io::Result<PathBuf> {
    let path = quirk_baseline_path(addr)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "home dir unavailable"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, report.to_json_pretty())?;
    Ok(path)
}

/// When the quirk baseline for `addr` was saved, formatted for the UI.
pub fn quirk_baseline_saved_at(addr: &str) -> Option<String> {
    mtime_label(&quirk_baseline_path(addr)?)
}

// ── Quirk sweep surface selection ───────────────────────────────────────────
// Which ONVIF read operations the Quirks tab drives when testing a live camera.
// One file for the whole app, not one per device: the subset is a tester's
// working preference (a hunt for one model-specific quirk is normally re-run
// across several cameras), not a fact about any one device.

fn surface_selection_path() -> Option<PathBuf> {
    oxdm_dir().map(|d| d.join("quirk-surface.json"))
}

/// The saved quirk-sweep operation selection, or `None` when nothing has been
/// saved yet (or the file is stale / unreadable — the caller falls back to the
/// default surface rather than failing).
pub fn read_surface_selection() -> Option<oxvif::metamorph::SurfaceSelection> {
    let path = surface_selection_path()?;
    let json = std::fs::read_to_string(&path).ok()?;
    match serde_json::from_str(&json) {
        Ok(s) => Some(s),
        Err(e) => {
            warn!(error = %e, path = %path.display(), "stale quirk surface selection ignored");
            None
        }
    }
}

/// Persist the quirk-sweep operation selection. Best-effort: a failure to write
/// costs the user their selection on next launch, nothing more, so it warns
/// rather than surfacing an error into the UI.
pub fn write_surface_selection(selection: &oxvif::metamorph::SurfaceSelection) {
    let Some(path) = surface_selection_path() else {
        return;
    };
    ensure_dir();
    match serde_json::to_string(selection) {
        Ok(json) => {
            if let Err(e) = std::fs::write(&path, json) {
                warn!(error = %e, path = %path.display(), "could not persist quirk surface selection");
            }
        }
        Err(e) => warn!(error = %e, "could not serialise quirk surface selection"),
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

// Group credentials share the single keychain blob via reserved key prefixes.
// Keys are only ever constructed for lookup, never split — and manual-device
// addrs are http(s) URLs, never `group:`-prefixed, so there's no collision.
fn group_cred_key(id: &str) -> String {
    format!("group:{id}")
}

fn group_device_cred_key(id: &str, addr: &str) -> String {
    format!("group:{id}:{addr}")
}

/// The single source of truth for the keychain blob: global creds + manual
/// per-device creds + per-group and per-device-in-group creds. Every keychain
/// write goes through here so no save path can clobber another's keys — a
/// deleted group simply stops contributing its `group:<id>*` keys.
fn build_creds_map(
    global: &Credentials,
    devices: &[DeviceEntry],
    groups: &[HealthGroup],
) -> CredsMap {
    let mut map = CredsMap::new();
    if !global.username.is_empty() {
        map.insert(
            CREDS_KEY_GLOBAL.to_string(),
            (global.username.clone(), global.password.clone()),
        );
    }
    for d in devices.iter().filter(|d| d.manual) {
        if let Some(c) = &d.credentials {
            map.insert(d.addr.clone(), (c.username.clone(), c.password.clone()));
        }
    }
    for g in groups {
        if let Some(c) = &g.credentials {
            if !c.username.is_empty() {
                map.insert(
                    group_cred_key(&g.id),
                    (c.username.clone(), c.password.clone()),
                );
            }
        }
        for (addr, c) in &g.device_credentials {
            if !c.username.is_empty() {
                map.insert(
                    group_device_cred_key(&g.id, addr),
                    (c.username.clone(), c.password.clone()),
                );
            }
        }
    }
    map
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
                            clone_of: None,
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

pub fn save_config(theme: Theme, locale: Locale, log_to_file: bool, tls_strict: bool) {
    ensure_dir();

    // Save theme/locale/log/tls preference to config.toml (no credentials)
    if let Some(path) = config_path() {
        #[derive(Serialize)]
        struct ConfigOut {
            theme: String,
            locale: String,
            log_to_file: bool,
            tls_strict: bool,
        }
        let cfg = ConfigOut {
            theme: theme_to_str(theme).to_string(),
            locale: locale_to_str(locale).to_string(),
            log_to_file,
            tls_strict,
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

/// Write the whole keychain blob from the complete app state. This is the ONLY
/// keychain write path (besides the legacy migration in `load_all_credentials`)
/// — both `save_credentials_and_devices` and `save_health_groups` funnel
/// through it with the full `(global, devices, groups)` triple, so neither can
/// erase the other's keys.
fn save_keychain_blob(global: &Credentials, devices: &[DeviceEntry], groups: &[HealthGroup]) {
    keyring_save_all(&build_creds_map(global, devices, groups));
}

fn write_devices_file(devices: &[DeviceEntry]) {
    let Some(path) = devices_path() else { return };
    let records: Vec<DeviceRecord> = devices
        .iter()
        // Served clones are ephemeral loopback entries — never persist them.
        .filter(|d| d.manual && d.clone_of.is_none())
        .map(|d| DeviceRecord {
            name: d.name.clone(),
            addr: d.addr.clone(),
            has_credentials: d.credentials.is_some(),
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

fn write_health_groups_file(groups: &[HealthGroup]) {
    let Some(path) = healthcheck_path() else {
        return;
    };
    let file = HealthGroupsFile {
        groups: groups.to_vec(),
    };
    match toml::to_string_pretty(&file) {
        Ok(content) => {
            if let Err(e) = std::fs::write(&path, content) {
                error!(error = %e, "Failed to write healthcheck.toml");
            } else {
                debug!("Saved {} health group(s)", file.groups.len());
            }
        }
        Err(e) => error!(error = %e, "Failed to serialize health groups"),
    }
}

/// Save all credentials (global + per-device + group) to the single keychain
/// entry, and save manual device records to devices.toml. Takes `groups` so the
/// rebuilt keychain blob keeps their creds (anti-clobber).
pub fn save_credentials_and_devices(
    global_creds: &Credentials,
    devices: &[DeviceEntry],
    groups: &[HealthGroup],
) {
    ensure_dir();
    save_keychain_blob(global_creds, devices, groups);
    write_devices_file(devices);
}

/// Save HealthCheck groups to healthcheck.toml, and re-emit the full keychain
/// blob (so group creds land alongside global/device creds without clobbering).
pub fn save_health_groups(
    global_creds: &Credentials,
    devices: &[DeviceEntry],
    groups: &[HealthGroup],
) {
    ensure_dir();
    save_keychain_blob(global_creds, devices, groups);
    write_health_groups_file(groups);
}

/// Load persisted HealthCheck groups from healthcheck.toml, hydrating each
/// group's credentials from the already-loaded keychain map (no extra keychain
/// access). Returns an empty vec on missing / unparseable file.
pub fn load_health_groups(creds_map: &CredsMap) -> Vec<HealthGroup> {
    let Some(path) = healthcheck_path() else {
        return Vec::new();
    };
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => {
            debug!("No healthcheck.toml found");
            return Vec::new();
        }
    };
    let file: HealthGroupsFile = match toml::from_str(&content) {
        Ok(f) => f,
        Err(e) => {
            error!(error = %e, "Failed to parse healthcheck.toml");
            return Vec::new();
        }
    };
    let mut groups = file.groups;
    for g in &mut groups {
        if let Some((u, p)) = creds_map.get(&group_cred_key(&g.id)) {
            g.credentials = Some(Credentials {
                username: u.clone(),
                password: p.clone(),
            });
        }
        for r in &g.devices {
            if let Some((u, p)) = creds_map.get(&group_device_cred_key(&g.id, &r.addr)) {
                g.device_credentials.insert(
                    r.addr.clone(),
                    Credentials {
                        username: u.clone(),
                        password: p.clone(),
                    },
                );
            }
        }
    }
    info!(
        count = groups.len(),
        "Loaded health groups from {}",
        path.display()
    );
    groups
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

#[cfg(test)]
mod tests {
    use super::*;

    /// `delete_clone` ends in `remove_dir_all`, so a name that resolves outside
    /// `~/.oxdm/clones/` would delete the wrong tree.
    ///
    /// `".."` is the case that matters, and it is why the check is a whitelist
    /// of one `Component::Normal` rather than a scan for `..`: the obvious
    /// guard, `root.join(name).parent() == Some(root)`, **accepts** it.
    /// `Path::parent` is purely lexical, so `clones/..` has parent `clones`,
    /// the guard passes, and `remove_dir_all` then erases `~/.oxdm` — every
    /// device, health group, baseline and saved clone in it. That version was
    /// written first, and this case is what caught it.
    ///
    /// Asserted against `clone_component`, never `delete_clone`: the rule is
    /// what needs testing, and calling the deleting function from a unit test
    /// leaves the developer's own `~/.oxdm` one logic error away from erasure.
    #[test]
    fn clone_component_rejects_every_name_that_is_not_one_plain_component() {
        for bad in [
            "..",
            "../..",
            "../baselines",
            "sub/dir",
            "sub\\dir",
            ".",
            "",
            "/etc",
            "C:\\Windows",
        ] {
            let err = clone_component(bad).expect_err(&format!("{bad:?} must be rejected"));
            assert_eq!(err.kind(), std::io::ErrorKind::InvalidInput, "{bad:?}");
        }
    }

    /// The other half: an ordinary sanitized clone name — what [`list_clones`]
    /// actually yields, and what the delete button passes back — must survive
    /// the check, or every real row would be rejected.
    #[test]
    fn clone_component_accepts_a_sanitized_clone_name() {
        let name = clone_dir_name("GV-SD4825-IR @ 192.168.5.206");
        assert_eq!(
            clone_component(&name).expect("a sanitized label is one component"),
            std::ffi::OsStr::new(&name)
        );
    }
}
