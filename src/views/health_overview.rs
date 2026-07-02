#![allow(non_snake_case)]
//! Batch health / conformance check across a device list.
//!
//! This is a global (device-independent) `View`: the community-facing tool for
//! testing a fleet of mixed-brand IP cameras against ONVIF and exporting the
//! results, so wildly non-conformant firmwares can be reported back and folded
//! into oxvif's per-vendor compatibility fixtures.
//!
//! Two kinds of list coexist in the left column: the dynamic **All devices**
//! list (filterable, ephemeral selection) and saved **groups** (persisted,
//! optionally carrying their own credentials).
//!
//! Beyond oxvif's read-only `HealthCheck` (which only *presence-checks* Profile
//! G), this actively probes Profile G — it really calls `FindRecordings` and
//! `GetReplayUri` and captures the verbatim SOAP fault, since that replay/search
//! path is where brand quirks (e.g. Hanwha's occurrence-constraint fault) hide.
use crate::components::DialogOverlay;
use crate::components::{GroupCredentialsDialog, GroupDeviceCredentialsDialog, Icon};
use crate::state::{
    AuthStatus, CredSource, Credentials, Ctx, DeviceEntry, DragPending, HealthDeviceRef,
    HealthGroup, ToastLevel,
};
use crate::views::settings::health::{
    group_by_category, row_message, status_class, status_icon, verdict_class, verdict_label,
};
use crate::{api, i18n};
use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use oxvif::health::{CheckStatus, ErrorClass, HealthReport, ProfileVerdict};
use serde::Serialize;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

/// Hard cap so a single non-responsive device can't stall its row forever.
const DEVICE_TIMEOUT: Duration = Duration::from_secs(120);

const BUNDLE_SCHEMA: &str = "oxdm-health-batch/v3";

/// Clock skew (seconds, absolute) beyond which WS-Security timestamp validation
/// is likely to reject requests — the usual cause of spurious auth failures.
const SKEW_BREAKS_WSSEC: i64 = 300;

// ── List selection + filters ────────────────────────────────────────────────

// The selected list now lives in `Ctx.health_list` (set by the sidebar Groups
// tab / topbar); alias the shared enum so the rest of this module reads cleanly.
use crate::state::HealthListSel as ListSel;

#[derive(Clone, Copy, PartialEq)]
enum SourceFilter {
    All,
    Discovered,
    Manual,
}

#[derive(Clone, Copy, PartialEq)]
enum AuthFilter {
    All,
    LoggedIn,
    Failed,
    Unverified,
}

fn source_ok(d: &DeviceEntry, f: SourceFilter) -> bool {
    match f {
        SourceFilter::All => true,
        SourceFilter::Discovered => !d.manual,
        SourceFilter::Manual => d.manual,
    }
}

fn auth_ok(d: &DeviceEntry, f: AuthFilter) -> bool {
    match f {
        AuthFilter::All => true,
        AuthFilter::LoggedIn => d.auth_status == AuthStatus::Ok,
        AuthFilter::Failed => d.auth_status == AuthStatus::Failed,
        AuthFilter::Unverified => d.auth_status == AuthStatus::Unknown,
    }
}

/// Resolve a persisted group member to a live device: match by endpoint (stable
/// across IP changes) when present, else by addr.
fn resolve<'a>(devices: &'a [DeviceEntry], r: &HealthDeviceRef) -> Option<&'a DeviceEntry> {
    devices
        .iter()
        .find(|d| (!r.endpoint.is_empty() && d.endpoint == r.endpoint) || d.addr == r.addr)
}

fn cred_badge_key(s: CredSource) -> &'static str {
    match s {
        CredSource::Device => "hgroups_cred_device",
        CredSource::Group => "hgroups_cred_group",
        CredSource::App => "hgroups_cred_app",
    }
}

/// A precomputed group-member row (resolved to a live device or marked offline),
/// so the rsx `for` loop renders a keyed `DeviceRow` directly — dioxus requires
/// the key on the immediate loop child, not nested inside a `match`.
struct GroupRow {
    name: String,
    display_addr: String,
    offline: bool,
    cred_badge: &'static str,
    /// Live device addr — results/creds lookup (empty when offline).
    addr: String,
    /// Persisted ref identity, for removal from the group.
    ref_endpoint: String,
    ref_addr: String,
}

// ── Per-device run state ────────────────────────────────────────────────────

#[derive(Clone, PartialEq)]
enum RunState {
    Pending,
    Running,
    TimedOut,
    Done(Box<DeviceResult>),
}

// ── Serializable export shapes ──────────────────────────────────────────────

#[derive(Clone, PartialEq, Serialize)]
struct Fingerprint {
    manufacturer: String,
    model: String,
    firmware_version: String,
    serial_number: String,
    hardware_id: String,
}

/// The maintainer-facing interpretation of a failure — lets a reader triage
/// "this is my fault (auth/clock)" vs "this is a real brand quirk" at a glance.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
enum LikelyCause {
    /// Credentials rejected (not a device bug).
    Auth,
    /// Auth fault with a large clock skew — WS-Security timestamp likely rejected.
    ClockSkew,
    /// Service/feature not advertised — precondition unmet, call never sent.
    Unsupported,
    /// Network / TLS / HTTP transport failure.
    Transport,
    /// A device-reported fault that isn't auth — the interesting cross-brand case.
    BrandQuirk,
}

/// A structured, classified error for export. Health `checks[]` carry oxvif's
/// own `CheckError`; this is the oxdm-side equivalent for the Profile G probe
/// and fingerprint calls, whose errors only cross the api boundary as strings.
#[derive(Clone, PartialEq, Serialize)]
struct ExportError {
    class: ErrorClass,
    #[serde(skip_serializing_if = "Option::is_none")]
    fault_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    subcode: Option<String>,
    reason: String,
    likely_cause: LikelyCause,
    raw: String,
}

impl std::fmt::Display for ExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.reason)
    }
}

/// Per-device reachability / auth summary, so a reader can separate credential
/// and clock problems from genuine conformance findings without reading checks.
#[derive(Clone, PartialEq, Serialize)]
struct Connectivity {
    /// The initial `GetCapabilities` succeeded.
    reachable: bool,
    /// No auth-class fault was seen (only meaningful when `reachable`).
    authenticated: bool,
    /// Device clock skew vs local (seconds), when measured.
    #[serde(skip_serializing_if = "Option::is_none")]
    clock_skew_s: Option<i64>,
    /// An auth failure coincides with a skew past `SKEW_BREAKS_WSSEC` — the auth
    /// failures are probably spurious (fix the clock, not the credentials).
    auth_blocked_by_skew: bool,
}

/// Active Profile G probe — real `FindRecordings` + `GetReplayUri` calls, with
/// the classified fault preserved on failure.
#[derive(Clone, PartialEq, Serialize, Default)]
struct ProfileGProbe {
    /// Number of recordings returned by the search (on success).
    search_recordings: Option<usize>,
    /// Classified error from `FindRecordings` (on failure).
    search_error: Option<ExportError>,
    /// Replay URI resolved for the first recording (on success).
    replay_uri: Option<String>,
    /// Classified error from `GetReplayUri` (on failure).
    replay_error: Option<ExportError>,
}

#[derive(Clone, PartialEq, Serialize)]
struct DeviceResult {
    target: String,
    display_addr: String,
    name: String,
    fingerprint: Option<Fingerprint>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fingerprint_error: Option<ExportError>,
    /// Reachability / auth digest derived from the report.
    connectivity: Connectivity,
    /// The device didn't finish within the per-device timeout.
    timed_out: bool,
    report: Option<HealthReport>,
    profile_g_probe: ProfileGProbe,
}

impl DeviceResult {
    /// Blank the obvious PII the maintainer doesn't need for compatibility work
    /// (serial / hardware id). Addresses are scrubbed separately by the
    /// IPv4-wide pass over the serialized JSON.
    fn redact(&mut self) {
        if let Some(fp) = &mut self.fingerprint {
            if !fp.serial_number.is_empty() {
                fp.serial_number = "[redacted]".into();
            }
            if !fp.hardware_id.is_empty() {
                fp.hardware_id = "[redacted]".into();
            }
        }
    }
}

/// Pass/Warn/Fail/Skip tally for one check id across the fleet.
#[derive(Serialize, Default)]
struct StatusTally {
    pass: usize,
    warn: usize,
    fail: usize,
    skip: usize,
}

/// Profile verdict tally across the fleet.
#[derive(Serialize, Default)]
struct VerdictTally {
    conformant: usize,
    partial: usize,
    unsupported: usize,
}

#[derive(Serialize, Default)]
struct ProfileTallies {
    s: VerdictTally,
    t: VerdictTally,
    g: VerdictTally,
}

/// How many failures fell into each likely cause across the fleet.
#[derive(Serialize, Default)]
struct CauseTotals {
    auth: usize,
    clock_skew: usize,
    unsupported: usize,
    transport: usize,
    brand_quirk: usize,
}

/// Reconciliation of one ONVIF profile: what the device *declares* (via scopes)
/// vs what oxvif *assessed* by probing. The `Broken` case — declared but not
/// conformant — is the headline diagnostic ("claims Profile G, replay fails").
#[derive(Clone, Copy, Debug, PartialEq)]
enum ProfileStatus {
    /// Declared and assessed Conformant.
    Ok,
    /// Declared but assessed Partial/Unsupported.
    Broken,
    /// Declared but oxvif doesn't assess this profile (M/A/C/D/K).
    Unverified,
    /// Assessed Conformant but not declared (works, vendor didn't claim it).
    Extra,
}

struct ProfileState {
    profile: String,
    assessed: Option<ProfileVerdict>,
    status: ProfileStatus,
}

/// A declared-but-broken profile rolled up across the fleet.
#[derive(Serialize)]
struct ProfileGap {
    /// Canonical profile letter (`S` / `T` / `G`).
    profile: String,
    /// Worst assessed verdict seen (`partial` / `unsupported`).
    verdict: &'static str,
    count: usize,
    sample_models: Vec<String>,
}

/// The same fault seen across devices, collapsed by its grouping key (subcode →
/// fault code → reason) — the cross-brand normalization payoff.
#[derive(Serialize)]
struct FaultGroup {
    key: String,
    likely_cause: LikelyCause,
    count: usize,
    /// A few distinct device models exhibiting it (capped).
    sample_models: Vec<String>,
}

/// Fleet-wide rollup so a reader can triage before opening any single device.
#[derive(Serialize)]
struct Summary {
    device_count: usize,
    timed_out: usize,
    profiles: ProfileTallies,
    /// Per-check-id status tally, keyed by check id (sorted).
    checks: std::collections::BTreeMap<String, StatusTally>,
    /// Distinct faults, most frequent first.
    fault_groups: Vec<FaultGroup>,
    likely_cause_totals: CauseTotals,
    /// Profiles a device declares (via scopes) but that failed assessment —
    /// the "self-certified but broken" cases, most frequent first.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    declared_but_broken: Vec<ProfileGap>,
    /// Profiles declared but not assessed by oxvif (M/A/C/D/K), keyed by letter
    /// → number of devices declaring it. Informational, not a failure.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    declared_unverified: BTreeMap<String, usize>,
}

#[derive(Serialize)]
struct ReportBundle {
    schema: &'static str,
    oxdm_version: &'static str,
    oxvif_version: &'static str,
    generated_at: String,
    redacted: bool,
    summary: Summary,
    devices: Vec<DeviceResult>,
}

// ── Component ───────────────────────────────────────────────────────────────

#[component]
pub fn HealthOverviewView() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let selected_list = ctx.health_list;
    let mut source_filter = use_signal(|| SourceFilter::All);
    let mut auth_filter = use_signal(|| AuthFilter::LoggedIn);
    let mut selected = use_signal(HashSet::<String>::new);
    let mut results = use_signal(HashMap::<String, RunState>::new);
    let mut running = use_signal(|| false);
    let mut pending = use_signal(|| 0usize);
    // Default ON: these exports are meant to be pasted into public issues.
    let mut redact = use_signal(|| true);
    let gcreds_open = use_signal(|| false);
    let dev_creds_open = use_signal(|| false);
    let dev_creds_addr = use_signal(String::new);
    // Per-device drill-down: addr of the device whose full report modal is open.
    let mut detail_open = use_signal(|| None::<String>);

    let busy = *running.read();
    let devices = ctx.devices.read().clone();
    let groups = ctx.health_groups.read().clone();

    // Resolve the active group; if the selected group was deleted, fall back.
    let active_group: Option<HealthGroup> = match &*selected_list.read() {
        ListSel::Group(id) => groups.iter().find(|g| &g.id == id).cloned(),
        ListSel::AllDevices => None,
    };
    let list = if matches!(&*selected_list.read(), ListSel::Group(_)) && active_group.is_none() {
        ListSel::AllDevices
    } else {
        selected_list.read().clone()
    };

    let src = *source_filter.read();
    let auth = *auth_filter.read();

    // Group-level creds override every member's own login — surface it so the
    // user knows the run tests that one account, not each device's known-good.
    let group_creds_active = active_group.as_ref().is_some_and(|g| {
        g.credentials
            .as_ref()
            .is_some_and(|c| !c.username.is_empty())
    });

    // Count of devices the Run button will act on, for its label.
    let run_count = match &list {
        ListSel::AllDevices => {
            let sel = selected.read();
            devices
                .iter()
                .filter(|d| source_ok(d, src) && auth_ok(d, auth) && sel.contains(&d.addr))
                .count()
        }
        ListSel::Group(_) => active_group
            .as_ref()
            .map(|g| {
                g.devices
                    .iter()
                    .filter(|r| resolve(&devices, r).is_some())
                    .count()
            })
            .unwrap_or(0),
    };

    let all_visible_selected = {
        let sel = selected.read();
        let vis: Vec<&DeviceEntry> = devices
            .iter()
            .filter(|d| source_ok(d, src) && auth_ok(d, auth))
            .collect();
        !vis.is_empty() && vis.iter().all(|d| sel.contains(&d.addr))
    };

    let toggle_all = move |_| {
        let devices = ctx.devices.peek().clone();
        let src = *source_filter.peek();
        let auth = *auth_filter.peek();
        let visible: Vec<String> = devices
            .iter()
            .filter(|d| source_ok(d, src) && auth_ok(d, auth))
            .map(|d| d.addr.clone())
            .collect();
        let mut set = selected.write();
        if visible.iter().all(|a| set.contains(a)) {
            for a in &visible {
                set.remove(a);
            }
        } else {
            for a in visible {
                set.insert(a);
            }
        }
    };

    // Pointer-down on an All-devices row: capture a pending drag (promoted to an
    // active drag by the root once it moves past the threshold). If the row is
    // part of the checked selection, drag the whole selection; else just it.
    let drag_start = use_callback(move |(clicked, x, y): (HealthDeviceRef, f64, f64)| {
        let sel = selected.peek();
        let payload: Vec<HealthDeviceRef> = if sel.contains(&clicked.addr) {
            ctx.devices
                .peek()
                .iter()
                .filter(|d| sel.contains(&d.addr))
                .map(|d| HealthDeviceRef {
                    endpoint: d.endpoint.clone(),
                    addr: d.addr.clone(),
                    name: d.name.clone(),
                })
                .collect()
        } else {
            vec![clicked]
        };
        drop(sel);
        ctx.drag_pending.clone().set(Some(DragPending {
            start_x: x,
            start_y: y,
            payload,
        }));
    });

    let run = move |_| {
        if *running.read() {
            return;
        }
        let list = selected_list.peek().clone();
        let devices = ctx.devices.peek().clone();
        let targets: Vec<(DeviceEntry, Credentials)> = match list {
            ListSel::AllDevices => {
                let src = *source_filter.peek();
                let auth = *auth_filter.peek();
                let sel = selected.peek().clone();
                devices
                    .iter()
                    .filter(|d| source_ok(d, src) && auth_ok(d, auth) && sel.contains(&d.addr))
                    .map(|d| (d.clone(), ctx.credentials_for(d)))
                    .collect()
            }
            ListSel::Group(id) => {
                let groups = ctx.health_groups.peek();
                match groups.iter().find(|g| g.id == id) {
                    Some(g) => g
                        .devices
                        .iter()
                        .filter_map(|r| {
                            resolve(&devices, r)
                                .map(|d| (d.clone(), ctx.group_credentials_for(g, d)))
                        })
                        .collect(),
                    None => Vec::new(),
                }
            }
        };
        if targets.is_empty() {
            return;
        }

        let mut init = HashMap::new();
        for (d, _) in &targets {
            init.insert(d.addr.clone(), RunState::Pending);
        }
        results.set(init);
        pending.set(targets.len());
        running.set(true);

        for (d, creds) in targets {
            spawn(async move {
                results.write().insert(d.addr.clone(), RunState::Running);
                let outcome = run_one(&d, &creds).await;
                results.write().insert(d.addr.clone(), outcome);
                let remaining = {
                    let mut p = pending.write();
                    *p = p.saturating_sub(1);
                    *p
                };
                if remaining == 0 {
                    running.set(false);
                }
            });
        }
    };

    let export = move |_| {
        let res = results.peek();
        let mut done: Vec<DeviceResult> = Vec::new();
        for (addr, state) in res.iter() {
            match state {
                RunState::Done(r) => done.push((**r).clone()),
                // Record timed-out devices too (a hanging brand is itself a
                // finding) — build a minimal result from the live device.
                RunState::TimedOut => {
                    let (name, display_addr) = ctx
                        .devices
                        .peek()
                        .iter()
                        .find(|d| &d.addr == addr)
                        .map(|d| (d.name.clone(), d.display_addr.clone()))
                        .unwrap_or_else(|| (addr.clone(), addr.clone()));
                    done.push(DeviceResult {
                        target: addr.clone(),
                        display_addr,
                        name,
                        fingerprint: None,
                        fingerprint_error: None,
                        connectivity: connectivity(None),
                        timed_out: true,
                        report: None,
                        profile_g_probe: ProfileGProbe::default(),
                    });
                }
                _ => {}
            }
        }
        drop(res);
        if done.is_empty() {
            ctx.push_toast(ToastLevel::Info, i18n::t(locale, "hbatch_export_nothing"));
            return;
        }
        let do_redact = *redact.peek();
        if do_redact {
            for d in &mut done {
                d.redact();
            }
        }
        let summary = build_summary(&done);
        let bundle = ReportBundle {
            schema: BUNDLE_SCHEMA,
            oxdm_version: env!("CARGO_PKG_VERSION"),
            oxvif_version: crate::components::OXVIF_VERSION,
            generated_at: now_iso(),
            redacted: do_redact,
            summary,
            devices: done,
        };
        let mut json = serde_json::to_string_pretty(&bundle).unwrap_or_default();
        if do_redact {
            json = redact_ipv4(&json);
        }
        let file_name = format!("oxdm-health-report-{}.json", now_file_stamp());
        spawn(async move {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name(&file_name)
                .add_filter("JSON", &["json"])
                .save_file()
                .await
            else {
                return;
            };
            let path = handle.path().to_path_buf();
            match std::fs::write(&path, json.as_bytes()) {
                Ok(()) => ctx.push_toast(
                    ToastLevel::Success,
                    format!("{}: {}", i18n::t(locale, "hbatch_exported"), path.display()),
                ),
                Err(e) => ctx.push_toast(
                    ToastLevel::Error,
                    format!("{}: {e}", i18n::t(locale, "hbatch_export_failed")),
                ),
            }
        });
    };

    let has_results = results
        .read()
        .values()
        .any(|s| matches!(s, RunState::Done(_)));

    let is_group = matches!(list, ListSel::Group(_));
    let title = match &list {
        ListSel::AllDevices => i18n::t(locale, "hbatch_title").to_string(),
        ListSel::Group(_) => active_group
            .as_ref()
            .map(|g| g.name.clone())
            .unwrap_or_default(),
    };
    let active_group_id = active_group
        .as_ref()
        .map(|g| g.id.clone())
        .unwrap_or_default();

    // Precompute the row lists so each rsx `for` renders a keyed `DeviceRow`
    // directly (no `if`/`match` wrapper around the keyed node).
    let visible_all: Vec<DeviceEntry> = devices
        .iter()
        .filter(|d| source_ok(d, src) && auth_ok(d, auth))
        .cloned()
        .collect();
    let group_rows: Vec<GroupRow> = active_group
        .as_ref()
        .map(|g| {
            g.devices
                .iter()
                .map(|r| match resolve(&devices, r) {
                    Some(d) => GroupRow {
                        name: d.name.clone(),
                        display_addr: d.display_addr.clone(),
                        offline: false,
                        cred_badge: cred_badge_key(ctx.group_cred_source(g, d)),
                        addr: d.addr.clone(),
                        ref_endpoint: r.endpoint.clone(),
                        ref_addr: r.addr.clone(),
                    },
                    None => GroupRow {
                        name: r.name.clone(),
                        display_addr: r.addr.clone(),
                        offline: true,
                        cred_badge: "",
                        addr: String::new(),
                        ref_endpoint: r.endpoint.clone(),
                        ref_addr: r.addr.clone(),
                    },
                })
                .collect()
        })
        .unwrap_or_default();

    rsx! {
        div { class: "hbatch-view",
            div { class: "hbatch-panel",
                div { class: "hbatch-header",
                    div { class: "hbatch-header-text",
                        h2 { "{title}" }
                        if !is_group {
                            p { class: "hbatch-subtitle", {i18n::t(locale, "hbatch_subtitle")} }
                        }
                    }
                    div { class: "hbatch-actions",
                        if is_group {
                            button {
                                class: "btn btn-md btn-secondary",
                                onclick: {
                                    let mut o = gcreds_open;
                                    move |_| o.set(true)
                                },
                                Icon { name: "key", size: 14 }
                                {i18n::t(locale, "hgroups_group_creds")}
                            }
                        }
                        label { class: "hbatch-redact",
                            input {
                                r#type: "checkbox",
                                checked: *redact.read(),
                                onchange: move |e| redact.set(e.checked()),
                            }
                            {i18n::t(locale, "hbatch_redact")}
                        }
                        button {
                            class: "btn btn-md btn-secondary",
                            disabled: busy || !has_results,
                            onclick: export,
                            Icon { name: "download", size: 14 }
                            {i18n::t(locale, "hbatch_export")}
                        }
                        button {
                            class: "btn btn-md btn-primary",
                            disabled: busy || run_count == 0,
                            onclick: run,
                            Icon { name: "activity", size: 16 }
                            if busy {
                                {i18n::t(locale, "hbatch_running")}
                            } else {
                                {format!("{} ({run_count})", i18n::t(locale, "hbatch_run"))}
                            }
                        }
                    }
                }

                // Group creds override each member's snapshotted own login.
                if is_group && group_creds_active {
                    div { class: "hbatch-cred-notice",
                        Icon { name: "key", size: 12 }
                        {i18n::t(locale, "hgroups_group_creds_override")}
                    }
                }

                // Filters (All-devices list only).
                if !is_group {
                    div { class: "hbatch-filters",
                        span { class: "hbatch-filter-label", {i18n::t(locale, "hgroups_source_label")} }
                        select {
                            class: "sidebar-filter-select",
                            onchange: move |e| source_filter.set(match e.value().as_str() {
                                "discovered" => SourceFilter::Discovered,
                                "manual" => SourceFilter::Manual,
                                _ => SourceFilter::All,
                            }),
                            option { value: "all", {i18n::t(locale, "filter_status_all")} }
                            option { value: "discovered", {i18n::t(locale, "devtab_discovered")} }
                            option { value: "manual", {i18n::t(locale, "devtab_manual")} }
                        }
                        span { class: "hbatch-filter-label", {i18n::t(locale, "hgroups_auth_label")} }
                        select {
                            class: "sidebar-filter-select",
                            onchange: move |e| auth_filter.set(match e.value().as_str() {
                                "ok" => AuthFilter::LoggedIn,
                                "failed" => AuthFilter::Failed,
                                "unknown" => AuthFilter::Unverified,
                                _ => AuthFilter::All,
                            }),
                            option { value: "all", {i18n::t(locale, "filter_status_all")} }
                            option { value: "ok", {i18n::t(locale, "filter_status_ok")} }
                            option { value: "failed", {i18n::t(locale, "filter_status_failed")} }
                            option { value: "unknown", {i18n::t(locale, "filter_status_unknown")} }
                        }
                    }
                }

                // Rows.
                if !is_group && devices.is_empty() {
                    div { class: "hbatch-empty",
                        Icon { name: "activity", size: 28 }
                        p { {i18n::t(locale, "hbatch_no_devices")} }
                    }
                } else if is_group && group_rows.is_empty() {
                    div { class: "hbatch-empty",
                        Icon { name: "folder", size: 28 }
                        p { {i18n::t(locale, "hgroups_empty")} }
                    }
                } else if is_group {
                    div { class: "hbatch-list",
                        for (i , row) in group_rows.iter().enumerate() {
                            DeviceRow {
                                key: "{i}",
                                locale,
                                name: row.name.clone(),
                                display_addr: row.display_addr.clone(),
                                checkbox: false,
                                checked: false,
                                offline: row.offline,
                                cred_badge: row.cred_badge.to_string(),
                                show_key: !row.offline,
                                state: if row.offline { None } else { results.read().get(&row.addr).cloned() },
                                on_toggle: move |_| {},
                                on_key: {
                                    let addr = row.addr.clone();
                                    let mut open = dev_creds_open;
                                    let mut a = dev_creds_addr;
                                    move |_| {
                                        a.set(addr.clone());
                                        open.set(true);
                                    }
                                },
                                show_remove: true,
                                draggable: false,
                                on_remove: {
                                    let ep = row.ref_endpoint.clone();
                                    let ra = row.ref_addr.clone();
                                    let gid = active_group_id.clone();
                                    let mut hg = ctx.health_groups;
                                    move |_| {
                                        let mut groups = hg.write();
                                        if let Some(g) = groups.iter_mut().find(|g| g.id == gid) {
                                            g.devices.retain(|r| {
                                                !((!ep.is_empty() && r.endpoint == ep) || r.addr == ra)
                                            });
                                        }
                                    }
                                },
                                on_pointerdown: move |_| {},
                                on_details: {
                                    let addr = row.addr.clone();
                                    move |_| detail_open.set(Some(addr.clone()))
                                },
                            }
                        }
                    }
                } else {
                    div { class: "hbatch-list",
                        div { class: "hbatch-row hbatch-row--head",
                            label { class: "hbatch-check",
                                input {
                                    r#type: "checkbox",
                                    checked: all_visible_selected,
                                    onchange: toggle_all,
                                }
                                {i18n::t(locale, "hbatch_select_all")}
                            }
                        }
                        for (i , d) in visible_all.iter().enumerate() {
                            DeviceRow {
                                key: "{i}",
                                locale,
                                name: d.name.clone(),
                                display_addr: d.display_addr.clone(),
                                checkbox: true,
                                checked: selected.read().contains(&d.addr),
                                offline: false,
                                cred_badge: if d.credentials.is_some() { "hgroups_cred_device".to_string() } else { "hgroups_cred_app".to_string() },
                                show_key: false,
                                state: results.read().get(&d.addr).cloned(),
                                on_toggle: {
                                    let addr = d.addr.clone();
                                    move |_| {
                                        let mut set = selected.write();
                                        if !set.remove(&addr) {
                                            set.insert(addr.clone());
                                        }
                                    }
                                },
                                on_key: move |_| {},
                                show_remove: false,
                                draggable: true,
                                on_remove: move |_| {},
                                on_pointerdown: {
                                    let r = HealthDeviceRef {
                                        endpoint: d.endpoint.clone(),
                                        addr: d.addr.clone(),
                                        name: d.name.clone(),
                                    };
                                    move |(x, y)| drag_start.call((r.clone(), x, y))
                                },
                                on_details: {
                                    let addr = d.addr.clone();
                                    move |_| detail_open.set(Some(addr.clone()))
                                },
                            }
                        }
                    }
                }
            }

            // Group credential dialogs — gated on `open` so they mount fresh
            // (re-seeding their fields from the current group/device) each time.
            if is_group && *gcreds_open.read() {
                GroupCredentialsDialog { open: gcreds_open, group_id: active_group_id.clone() }
            }
            if is_group && *dev_creds_open.read() {
                GroupDeviceCredentialsDialog {
                    open: dev_creds_open,
                    group_id: active_group_id.clone(),
                    addr: dev_creds_addr.read().clone(),
                }
            }

            // Per-device drill-down: full check list for the clicked device.
            {
                let open = detail_open.read().clone();
                open.and_then(|addr| match results.read().get(&addr) {
                    Some(RunState::Done(r)) => Some((**r).clone()),
                    _ => None,
                })
                .map(|res| rsx! {
                    HealthDetailModal {
                        locale,
                        result: Box::new(res),
                        on_close: move |_| detail_open.set(None),
                    }
                })
            }
        }
    }
}

/// Full per-check report for one device, opened from a batch row. Reuses the
/// single-device tab's row rendering, and adds the declared-vs-assessed profile
/// reconciliation that the batch export surfaces.
#[component]
fn HealthDetailModal(
    locale: crate::state::Locale,
    result: Box<DeviceResult>,
    on_close: EventHandler<()>,
) -> Element {
    let r = &*result;
    rsx! {
        DialogOverlay {
            on_close: move |_| on_close.call(()),
            inner_class: "dialog dialog--detail".to_string(),
            div { class: "hdetail",
                div { class: "hdetail-head",
                    div { class: "hdetail-ident",
                        h3 { class: "hdetail-title", "{r.name}" }
                        span { class: "hbatch-addr", "{r.display_addr}" }
                    }
                    button {
                        class: "btn btn-ghost btn-sm",
                        title: i18n::t(locale, "btn_close"),
                        onclick: move |_| on_close.call(()),
                        Icon { name: "x", size: 14 }
                    }
                }

                if let Some(fp) = r.fingerprint.as_ref() {
                    div { class: "hdetail-fp", {format!("{} {} · fw {}", fp.manufacturer, fp.model, fp.firmware_version)} }
                }

                if let Some(rep) = r.report.as_ref() {
                    // Declared profiles + reconciliation vs assessed.
                    {
                        let declared: Vec<ProfileState> = reconcile_profiles(rep)
                            .into_iter()
                            .filter(|ps| ps.status != ProfileStatus::Extra)
                            .collect();
                        (!declared.is_empty()).then(|| rsx! {
                            div { class: "health-group",
                                div { class: "health-group-title", {i18n::t(locale, "health_declared_profiles")} }
                                div { class: "hbatch-declared",
                                    for ps in declared {
                                        span {
                                            class: match ps.status {
                                                ProfileStatus::Ok => "hbatch-badge health-pass",
                                                ProfileStatus::Broken => "hbatch-badge health-fail",
                                                _ => "hbatch-badge hbatch-muted",
                                            },
                                            if matches!(ps.status, ProfileStatus::Broken) {
                                                {format!("{} {}", ps.profile, i18n::t(locale, "hbatch_declared_broken"))}
                                            } else {
                                                {ps.profile.clone()}
                                            }
                                        }
                                    }
                                }
                            }
                        })
                    }

                    // Per-category check list (same rendering as the single-device tab).
                    for (category , checks) in group_by_category(rep) {
                        div { class: "health-group",
                            div { class: "health-group-title", "{category}" }
                            for c in checks {
                                div { class: "health-row {status_class(&c.status)}",
                                    span { class: "health-row-status",
                                        Icon { name: status_icon(&c.status), size: 14 }
                                    }
                                    span { class: "health-row-name", "{c.id}" }
                                    span { class: "health-row-time", "{c.elapsed.as_millis()} ms" }
                                    span { class: "health-row-detail", "{row_message(c)}" }
                                }
                            }
                        }
                    }

                    // Assessed profile verdicts.
                    div { class: "health-group",
                        div { class: "health-group-title", {i18n::t(locale, "health_profiles")} }
                        div { class: "hbatch-profiles",
                            span { class: "hbatch-badge {verdict_class(&rep.profiles.profile_s.0)}", {format!("S {}", verdict_label(locale, &rep.profiles.profile_s.0))} }
                            span { class: "hbatch-badge {verdict_class(&rep.profiles.profile_t.0)}", {format!("T {}", verdict_label(locale, &rep.profiles.profile_t.0))} }
                            span { class: "hbatch-badge {verdict_class(&rep.profiles.profile_g.0)}", {format!("G {}", verdict_label(locale, &rep.profiles.profile_g.0))} }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DeviceRow(
    locale: crate::state::Locale,
    name: String,
    display_addr: String,
    checkbox: bool,
    checked: bool,
    offline: bool,
    cred_badge: String,
    show_key: bool,
    show_remove: bool,
    draggable: bool,
    state: Option<RunState>,
    on_toggle: EventHandler<()>,
    on_key: EventHandler<()>,
    on_remove: EventHandler<()>,
    on_pointerdown: EventHandler<(f64, f64)>,
    on_details: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let has_result = matches!(&state, Some(RunState::Done(_)));
    rsx! {
        div {
            class: if offline { "hbatch-row hbatch-row--offline" } else { "hbatch-row" },
            onpointerdown: move |e: Event<PointerData>| {
                if !draggable || e.data().trigger_button() != Some(MouseButton::Primary) {
                    return;
                }
                ctx.drag_just_finished.clone().set(false);
                let c = e.data().client_coordinates();
                on_pointerdown.call((c.x, c.y));
            },
            if checkbox {
                label { class: "hbatch-check",
                    input {
                        r#type: "checkbox",
                        checked,
                        onchange: move |_| on_toggle.call(()),
                    }
                }
            }
            div { class: "hbatch-ident",
                span { class: "hbatch-name", "{name}" }
                span { class: "hbatch-addr", "{display_addr}" }
            }
            if !cred_badge.is_empty() {
                span { class: "hbatch-cred-badge", {i18n::t(locale, &cred_badge)} }
            }
            if show_key {
                button {
                    class: "btn btn-ghost btn-sm hbatch-key-btn",
                    onclick: move |_| on_key.call(()),
                    Icon { name: "key", size: 12 }
                }
            }
            if show_remove {
                button {
                    class: "btn btn-ghost btn-sm hbatch-key-btn",
                    title: i18n::t(locale, "hgroups_remove"),
                    onclick: move |_| on_remove.call(()),
                    Icon { name: "x", size: 12 }
                }
            }
            if has_result {
                button {
                    class: "btn btn-ghost btn-sm hbatch-key-btn",
                    title: i18n::t(locale, "hbatch_details"),
                    // Stop the pointer event reaching the row's drag handler.
                    onpointerdown: move |e: Event<PointerData>| e.stop_propagation(),
                    onclick: move |_| on_details.call(()),
                    Icon { name: "list", size: 12 }
                }
            }
            div { class: "hbatch-outcome",
                if offline {
                    span { class: "health-warn hbatch-badge", {i18n::t(locale, "hgroups_offline")} }
                } else {
                    match state {
                        None => rsx! { span { class: "hbatch-muted", {i18n::t(locale, "hbatch_idle")} } },
                        Some(RunState::Pending) => rsx! { span { class: "hbatch-muted", {i18n::t(locale, "hbatch_pending")} } },
                        Some(RunState::Running) => rsx! { span { class: "hbatch-muted", {i18n::t(locale, "hbatch_state_running")} } },
                        Some(RunState::TimedOut) => rsx! {
                            span { class: "health-warn hbatch-badge",
                                Icon { name: "clock", size: 12 }
                                {i18n::t(locale, "hbatch_timeout")}
                            }
                        },
                        Some(RunState::Done(r)) => rsx! { DoneOutcome { locale, result: r } },
                    }
                }
            }
        }
    }
}

#[component]
fn DoneOutcome(locale: crate::state::Locale, result: Box<DeviceResult>) -> Element {
    let fp = result.fingerprint.as_ref();
    let (pass, warn, fail, skip) = result.report.as_ref().map(counts).unwrap_or((0, 0, 0, 0));
    let probe = &result.profile_g_probe;

    rsx! {
        div { class: "hbatch-done",
            // Fingerprint — make/model/fw, or the failure reason.
            if let Some(fp) = fp {
                span { class: "hbatch-fp", {format!("{} {} · fw {}", fp.manufacturer, fp.model, fp.firmware_version)} }
            } else if let Some(err) = result.fingerprint_error.as_ref() {
                span { class: "health-fail hbatch-badge", {format!("{}: {err}", i18n::t(locale, "hbatch_fp_failed"))} }
            }

            // Health summary counts.
            if result.report.is_some() {
                span { class: "hbatch-counts",
                    span { class: "health-pass", {format!("{pass} {}", i18n::t(locale, "health_pass"))} }
                    " · "
                    span { class: "health-warn", {format!("{warn} {}", i18n::t(locale, "health_warn"))} }
                    " · "
                    span { class: "health-fail", {format!("{fail} {}", i18n::t(locale, "health_fail"))} }
                    " · "
                    span { class: "health-skip", {format!("{skip} {}", i18n::t(locale, "health_skip"))} }
                }
            }

            // Profile S/T/G verdict badges.
            if let Some(rep) = result.report.as_ref() {
                span { class: "hbatch-profiles",
                    span { class: "hbatch-badge {verdict_class(&rep.profiles.profile_s.0)}", {format!("S {}", verdict_label(locale, &rep.profiles.profile_s.0))} }
                    span { class: "hbatch-badge {verdict_class(&rep.profiles.profile_t.0)}", {format!("T {}", verdict_label(locale, &rep.profiles.profile_t.0))} }
                    span { class: "hbatch-badge {verdict_class(&rep.profiles.profile_g.0)}", {format!("G {}", verdict_label(locale, &rep.profiles.profile_g.0))} }
                }

                // Declared-vs-assessed: surface "declares X but broken" (the
                // headline diagnostic) + declared-but-unverified profiles.
                {
                    let notable: Vec<ProfileState> = reconcile_profiles(rep)
                        .into_iter()
                        .filter(|ps| matches!(ps.status, ProfileStatus::Broken | ProfileStatus::Unverified))
                        .collect();
                    (!notable.is_empty()).then(|| rsx! {
                        span { class: "hbatch-declared",
                            span { class: "hbatch-declared-label", {i18n::t(locale, "hbatch_declared")} }
                            for ps in notable {
                                if matches!(ps.status, ProfileStatus::Broken) {
                                    span { class: "hbatch-badge health-fail",
                                        {format!("{} {}", ps.profile, i18n::t(locale, "hbatch_declared_broken"))}
                                    }
                                } else {
                                    span { class: "hbatch-badge hbatch-muted", {ps.profile.clone()} }
                                }
                            }
                        }
                    })
                }
            }

            // Active Profile G probe result.
            div { class: "hbatch-gprobe",
                span { class: "hbatch-gprobe-label", {i18n::t(locale, "hbatch_gprobe")} }
                if let Some(err) = probe.search_error.as_ref() {
                    span { class: "health-fail hbatch-badge", {format!("{}: {err}", i18n::t(locale, "hbatch_gprobe_search"))} }
                } else if let Some(n) = probe.search_recordings {
                    span { class: "health-pass hbatch-badge", {format!("{n} {}", i18n::t(locale, "hbatch_gprobe_recs"))} }
                    if let Some(err) = probe.replay_error.as_ref() {
                        span { class: "health-fail hbatch-badge", {format!("{}: {err}", i18n::t(locale, "hbatch_gprobe_replay"))} }
                    } else if probe.replay_uri.is_some() {
                        span { class: "health-pass hbatch-badge", {i18n::t(locale, "hbatch_gprobe_replay_ok")} }
                    }
                }
            }
        }
    }
}

// ── Per-device run ──────────────────────────────────────────────────────────

async fn run_one(d: &DeviceEntry, creds: &Credentials) -> RunState {
    let fut = async {
        // oxvif's HealthCheck is read-only and infallible (errors become Fail
        // rows inside the report). Run it first so its measured clock skew can
        // inform how we classify the probe / fingerprint auth failures.
        let report = api::run_health_check(&d.addr, creds).await;
        let skew = report.clock_skew_s;

        let (fingerprint, fingerprint_error) = match api::get_device_info(&d.addr, creds).await {
            Ok(i) => (
                Some(Fingerprint {
                    manufacturer: i.manufacturer,
                    model: i.model,
                    firmware_version: i.firmware_version,
                    serial_number: i.serial_number,
                    hardware_id: i.hardware_id,
                }),
                None,
            ),
            Err(e) => (None, Some(classify(&e, skew))),
        };

        // Active Profile G probe — the part oxvif's health check does not do.
        let mut probe = ProfileGProbe::default();
        match api::search_recordings(&d.addr, creds).await {
            Ok(recs) => {
                probe.search_recordings = Some(recs.len());
                if let Some(first) = recs.first() {
                    match api::get_replay_uri(&d.addr, creds, &first.recording_token).await {
                        Ok(uri) => probe.replay_uri = Some(uri),
                        Err(e) => probe.replay_error = Some(classify(&e, skew)),
                    }
                }
            }
            Err(e) => probe.search_error = Some(classify(&e, skew)),
        }

        DeviceResult {
            target: d.addr.clone(),
            display_addr: d.display_addr.clone(),
            name: d.name.clone(),
            fingerprint,
            fingerprint_error,
            connectivity: connectivity(Some(&report)),
            timed_out: false,
            report: Some(report),
            profile_g_probe: probe,
        }
    };

    match tokio::time::timeout(DEVICE_TIMEOUT, fut).await {
        Ok(r) => RunState::Done(Box::new(r)),
        Err(_) => RunState::TimedOut,
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn counts(rep: &HealthReport) -> (usize, usize, usize, usize) {
    (
        rep.count(|s| matches!(s, CheckStatus::Pass)),
        rep.count(|s| matches!(s, CheckStatus::Warn(_))),
        rep.count(|s| matches!(s, CheckStatus::Fail(_))),
        rep.count(|s| matches!(s, CheckStatus::Skip(_))),
    )
}

/// Interpret a classified error into a maintainer-facing likely cause. Shared by
/// oxvif's `CheckError`s (health checks) and oxdm's [`ExportError`]s (probe).
fn likely_cause(
    class: ErrorClass,
    subcode: Option<&str>,
    reason: &str,
    skew: Option<i64>,
) -> LikelyCause {
    match class {
        ErrorClass::Precondition => LikelyCause::Unsupported,
        ErrorClass::Http => LikelyCause::Transport,
        // Malformed responses / schema rejections are brand-specific oddities.
        ErrorClass::InvalidArgument | ErrorClass::Parse => LikelyCause::BrandQuirk,
        ErrorClass::SoapFault => {
            let hay = format!("{} {}", subcode.unwrap_or(""), reason).to_ascii_lowercase();
            let auth = hay.contains("notauthorized")
                || hay.contains("not authorized")
                || hay.contains("unauthorized")
                || hay.contains("security token")
                || hay.contains("authenticat");
            match (auth, skew.is_some_and(|s| s.abs() > SKEW_BREAKS_WSSEC)) {
                (true, true) => LikelyCause::ClockSkew,
                (true, false) => LikelyCause::Auth,
                (false, _) => LikelyCause::BrandQuirk,
            }
        }
    }
}

/// Parse an oxvif error string (it crosses the `ApiError = String` boundary as
/// the error's `Display`) back into a classified [`ExportError`]. Health
/// `checks[]` don't need this — they arrive structured from oxvif — but the
/// Profile G probe and fingerprint calls do.
fn classify(raw: &str, skew: Option<i64>) -> ExportError {
    let (class, fault_code, reason) = if let Some(rest) = raw.strip_prefix("SOAP fault [") {
        match rest.split_once("]: ") {
            Some((code, reason)) => (
                ErrorClass::SoapFault,
                Some(code.to_string()),
                reason.to_string(),
            ),
            None => (ErrorClass::SoapFault, None, rest.to_string()),
        }
    } else if raw.starts_with("Missing required field") {
        (ErrorClass::Precondition, None, raw.to_string())
    } else if raw.contains("HTTP request failed")
        || raw.contains("error sending request")
        || raw.starts_with("HTTP ")
    {
        (ErrorClass::Http, None, raw.to_string())
    } else {
        (ErrorClass::Parse, None, raw.to_string())
    };
    ExportError {
        class,
        fault_code,
        subcode: None,
        likely_cause: likely_cause(class, None, &reason, skew),
        reason,
        raw: raw.to_string(),
    }
}

/// Derive the per-device connectivity digest from its health report (`None` when
/// the device never produced one — e.g. it timed out).
fn connectivity(report: Option<&HealthReport>) -> Connectivity {
    let Some(rep) = report else {
        return Connectivity {
            reachable: false,
            authenticated: false,
            clock_skew_s: None,
            auth_blocked_by_skew: false,
        };
    };
    let reachable = rep
        .checks
        .iter()
        .any(|c| c.id == "connect" && matches!(c.status, CheckStatus::Pass));
    let skew = rep.clock_skew_s;
    let has_auth_fail = rep.checks.iter().any(|c| {
        c.error.as_ref().is_some_and(|e| {
            matches!(
                likely_cause(e.class, e.subcode.as_deref(), &e.reason, skew),
                LikelyCause::Auth | LikelyCause::ClockSkew
            )
        })
    });
    Connectivity {
        reachable,
        authenticated: reachable && !has_auth_fail,
        clock_skew_s: skew,
        auth_blocked_by_skew: has_auth_fail && skew.is_some_and(|s| s.abs() > SKEW_BREAKS_WSSEC),
    }
}

/// Grouping key for a fault: subcode → fault code → truncated reason.
fn fault_key(subcode: Option<&str>, fault_code: Option<&str>, reason: &str) -> String {
    if let Some(s) = subcode.filter(|s| !s.is_empty()) {
        return s.to_string();
    }
    if let Some(c) = fault_code.filter(|c| !c.is_empty()) {
        return c.to_string();
    }
    reason.chars().take(80).collect()
}

/// Fold one fault into the running group map + cause totals.
fn record_fault(
    groups: &mut HashMap<String, (LikelyCause, usize, Vec<String>)>,
    totals: &mut CauseTotals,
    key: String,
    lc: LikelyCause,
    model: &str,
) {
    match lc {
        LikelyCause::Auth => totals.auth += 1,
        LikelyCause::ClockSkew => totals.clock_skew += 1,
        LikelyCause::Unsupported => totals.unsupported += 1,
        LikelyCause::Transport => totals.transport += 1,
        LikelyCause::BrandQuirk => totals.brand_quirk += 1,
    }
    let entry = groups.entry(key).or_insert((lc, 0, Vec::new()));
    entry.1 += 1;
    if !model.is_empty() && entry.2.len() < 5 && !entry.2.iter().any(|m| m == model) {
        entry.2.push(model.to_string());
    }
}

/// Raw (untranslated) verdict token for the export JSON.
fn verdict_str(v: ProfileVerdict) -> &'static str {
    match v {
        ProfileVerdict::Conformant => "conformant",
        ProfileVerdict::Partial => "partial",
        ProfileVerdict::Unsupported => "unsupported",
    }
}

/// Reconcile a report's *declared* profiles (from scopes) against oxvif's
/// *assessed* verdicts. Only the S/T/G assessed set is compared; declared
/// profiles oxvif can't assess come back `Unverified`. Rows with nothing to say
/// (not declared, not conformant) are omitted.
fn reconcile_profiles(rep: &HealthReport) -> Vec<ProfileState> {
    let declared: HashSet<&str> = rep.declared_profiles.iter().map(String::as_str).collect();
    let assessed = [
        ("S", rep.profiles.profile_s.0),
        ("T", rep.profiles.profile_t.0),
        ("G", rep.profiles.profile_g.0),
    ];
    let mut out = Vec::new();
    for (p, v) in assessed {
        let is_declared = declared.contains(p);
        let status = match (is_declared, v) {
            (true, ProfileVerdict::Conformant) => ProfileStatus::Ok,
            (true, _) => ProfileStatus::Broken,
            (false, ProfileVerdict::Conformant) => ProfileStatus::Extra,
            (false, _) => continue,
        };
        out.push(ProfileState {
            profile: p.to_string(),
            assessed: Some(v),
            status,
        });
    }
    for p in &rep.declared_profiles {
        if !matches!(p.as_str(), "S" | "T" | "G") {
            out.push(ProfileState {
                profile: p.clone(),
                assessed: None,
                status: ProfileStatus::Unverified,
            });
        }
    }
    out
}

/// Build the fleet-wide [`Summary`] from the per-device results.
fn build_summary(devices: &[DeviceResult]) -> Summary {
    let mut checks: BTreeMap<String, StatusTally> = BTreeMap::new();
    let mut profiles = ProfileTallies::default();
    let mut cause_totals = CauseTotals::default();
    let mut groups: HashMap<String, (LikelyCause, usize, Vec<String>)> = HashMap::new();
    // Declared-vs-assessed reconciliation rollups.
    let mut broken: HashMap<String, (ProfileVerdict, usize, Vec<String>)> = HashMap::new();
    let mut unverified: BTreeMap<String, usize> = BTreeMap::new();
    let timed_out = devices.iter().filter(|d| d.timed_out).count();

    fn bump(t: &mut VerdictTally, v: &ProfileVerdict) {
        match v {
            ProfileVerdict::Conformant => t.conformant += 1,
            ProfileVerdict::Partial => t.partial += 1,
            ProfileVerdict::Unsupported => t.unsupported += 1,
        }
    }

    for d in devices {
        let model = d
            .fingerprint
            .as_ref()
            .map(|f| f.model.clone())
            .filter(|m| !m.is_empty())
            .unwrap_or_else(|| d.name.clone());

        if let Some(rep) = &d.report {
            bump(&mut profiles.s, &rep.profiles.profile_s.0);
            bump(&mut profiles.t, &rep.profiles.profile_t.0);
            bump(&mut profiles.g, &rep.profiles.profile_g.0);
            let skew = rep.clock_skew_s;
            for c in &rep.checks {
                let tally = checks.entry(c.id.clone()).or_default();
                match &c.status {
                    CheckStatus::Pass => tally.pass += 1,
                    CheckStatus::Warn(_) => tally.warn += 1,
                    CheckStatus::Fail(_) => tally.fail += 1,
                    CheckStatus::Skip(_) => tally.skip += 1,
                }
                if let Some(e) = &c.error {
                    let lc = likely_cause(e.class, e.subcode.as_deref(), &e.reason, skew);
                    let key = fault_key(e.subcode.as_deref(), e.fault_code.as_deref(), &e.reason);
                    record_fault(&mut groups, &mut cause_totals, key, lc, &model);
                }
            }

            for ps in reconcile_profiles(rep) {
                match ps.status {
                    ProfileStatus::Broken => {
                        let entry = broken.entry(ps.profile).or_insert((
                            ps.assessed.unwrap_or(ProfileVerdict::Unsupported),
                            0,
                            Vec::new(),
                        ));
                        // Unsupported is "more broken" than Partial.
                        if matches!(ps.assessed, Some(ProfileVerdict::Unsupported)) {
                            entry.0 = ProfileVerdict::Unsupported;
                        }
                        entry.1 += 1;
                        if !model.is_empty()
                            && entry.2.len() < 5
                            && !entry.2.iter().any(|m| m == &model)
                        {
                            entry.2.push(model.clone());
                        }
                    }
                    ProfileStatus::Unverified => *unverified.entry(ps.profile).or_insert(0) += 1,
                    ProfileStatus::Ok | ProfileStatus::Extra => {}
                }
            }
        }

        // Probe / fingerprint errors are already classified in oxdm.
        for e in [
            d.fingerprint_error.as_ref(),
            d.profile_g_probe.search_error.as_ref(),
            d.profile_g_probe.replay_error.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            let key = fault_key(e.subcode.as_deref(), e.fault_code.as_deref(), &e.reason);
            record_fault(&mut groups, &mut cause_totals, key, e.likely_cause, &model);
        }
    }

    let mut fault_groups: Vec<FaultGroup> = groups
        .into_iter()
        .map(|(key, (likely_cause, count, sample_models))| FaultGroup {
            key,
            likely_cause,
            count,
            sample_models,
        })
        .collect();
    fault_groups.sort_by(|a, b| b.count.cmp(&a.count).then_with(|| a.key.cmp(&b.key)));

    let mut declared_but_broken: Vec<ProfileGap> = broken
        .into_iter()
        .map(|(profile, (verdict, count, sample_models))| ProfileGap {
            profile,
            verdict: verdict_str(verdict),
            count,
            sample_models,
        })
        .collect();
    declared_but_broken.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| a.profile.cmp(&b.profile))
    });

    Summary {
        device_count: devices.len(),
        timed_out,
        profiles,
        checks,
        fault_groups,
        likely_cause_totals: cause_totals,
        declared_but_broken,
        declared_unverified: unverified,
    }
}

/// Local timestamp as ISO-8601 with an explicit UTC offset
/// (`YYYY-MM-DDTHH:MM:SS±HH:MM`), so the stamp is unambiguous when the offset
/// can't be determined we fall back to UTC (`+00:00`). Built from
/// `OffsetDateTime` components to avoid the `time` crate's `formatting` feature.
fn now_iso() -> String {
    use time::OffsetDateTime;
    let t = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    let off = t.offset().whole_seconds();
    let sign = if off < 0 { '-' } else { '+' };
    let (oh, om) = (off.abs() / 3600, (off.abs() % 3600) / 60);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{sign}{oh:02}:{om:02}",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
    )
}

/// Filesystem-safe local timestamp (`YYYYMMDD-HHMMSS`) for export file names —
/// no colons, so it is valid on Windows/macOS/Linux and sorts chronologically.
fn now_file_stamp() -> String {
    use time::OffsetDateTime;
    let t = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
    )
}

/// Replace dotted-quad IPv4 literals with `x.x.x.x` across a text blob. Runs of
/// digits and dots are collected and rewritten only when they parse as A.B.C.D,
/// so version strings (`0.10.0`) and millisecond fields survive untouched.
fn redact_ipv4(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut run = String::new();
    let flush = |run: &mut String, out: &mut String| {
        if is_ipv4(run) {
            out.push_str("x.x.x.x");
        } else {
            out.push_str(run);
        }
        run.clear();
    };
    for ch in s.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            run.push(ch);
        } else {
            flush(&mut run, &mut out);
            out.push(ch);
        }
    }
    flush(&mut run, &mut out);
    out
}

fn is_ipv4(t: &str) -> bool {
    let parts: Vec<&str> = t.split('.').collect();
    parts.len() == 4 && parts.iter().all(|p| p.parse::<u8>().is_ok())
}

// These helpers are private to this module, so their unit tests live inline
// rather than in `src/tests/` (which can only reach `pub` items).
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_normalizes_auth_and_separates_causes() {
        // The three brand spellings of "not authorized" all resolve to Auth.
        for raw in [
            "SOAP fault [SOAP-ENV:Sender]: ter:NotAuthorized",
            "SOAP fault [SOAP-ENV:Sender]: Sender not Authorized",
            "SOAP fault [SOAP-ENV:Sender]: The security token could not be authenticated or authorized",
        ] {
            let e = classify(raw, None);
            assert_eq!(e.class, ErrorClass::SoapFault, "raw: {raw}");
            assert_eq!(e.likely_cause, LikelyCause::Auth, "raw: {raw}");
        }

        // Same auth fault under a large clock skew → ClockSkew (spurious auth).
        let e = classify(
            "SOAP fault [SOAP-ENV:Sender]: ter:NotAuthorized",
            Some(-100_000),
        );
        assert_eq!(e.likely_cause, LikelyCause::ClockSkew);

        // Unadvertised service precondition → Unsupported, not a device fault.
        let e = classify("Missing required field: Search service URL", None);
        assert_eq!(e.class, ErrorClass::Precondition);
        assert_eq!(e.likely_cause, LikelyCause::Unsupported);

        // A non-auth device fault is the interesting cross-brand case.
        let e = classify("SOAP fault [SOAP-ENV:Receiver]: Action Failed", None);
        assert_eq!(e.fault_code.as_deref(), Some("SOAP-ENV:Receiver"));
        assert_eq!(e.likely_cause, LikelyCause::BrandQuirk);

        // Transport failure.
        let e = classify(
            "HTTP request failed: error sending request for url (http://x/onvif)",
            None,
        );
        assert_eq!(e.likely_cause, LikelyCause::Transport);
    }

    fn rep_with(
        declared: &[&str],
        s: ProfileVerdict,
        t: ProfileVerdict,
        g: ProfileVerdict,
    ) -> HealthReport {
        HealthReport {
            target: "http://x/onvif".into(),
            total_elapsed: Duration::from_millis(1),
            checks: vec![],
            profiles: oxvif::health::ProfileAssessment {
                profile_s: (s, vec![]),
                profile_t: (t, vec![]),
                profile_g: (g, vec![]),
            },
            clock_skew_s: None,
            declared_profiles: declared.iter().map(|p| p.to_string()).collect(),
        }
    }

    fn status_of<'a>(states: &'a [ProfileState], p: &str) -> Option<&'a ProfileState> {
        states.iter().find(|s| s.profile == p)
    }

    #[test]
    fn reconcile_separates_broken_extra_and_unverified() {
        use ProfileVerdict::*;
        // Declared G but assessed Unsupported → Broken (the headline case).
        // Declared M (not assessable) → Unverified.
        // S conformant but not declared → Extra.
        let rep = rep_with(&["G", "M"], Conformant, Unsupported, Unsupported);
        let states = reconcile_profiles(&rep);

        assert_eq!(
            status_of(&states, "S").unwrap().status,
            ProfileStatus::Extra
        );
        assert_eq!(
            status_of(&states, "G").unwrap().status,
            ProfileStatus::Broken
        );
        assert_eq!(
            status_of(&states, "M").unwrap().status,
            ProfileStatus::Unverified
        );
        // T: not declared and not conformant → nothing to say, omitted.
        assert!(status_of(&states, "T").is_none());
    }

    #[test]
    fn summary_rolls_up_declared_but_broken() {
        use ProfileVerdict::*;
        let dev = DeviceResult {
            target: "t".into(),
            display_addr: "d".into(),
            name: "n".into(),
            fingerprint: Some(Fingerprint {
                manufacturer: "GeoVision".into(),
                model: "GV-X".into(),
                firmware_version: "1".into(),
                serial_number: String::new(),
                hardware_id: String::new(),
            }),
            fingerprint_error: None,
            connectivity: connectivity(None),
            timed_out: false,
            report: Some(rep_with(&["G", "M"], Conformant, Conformant, Unsupported)),
            profile_g_probe: ProfileGProbe::default(),
        };
        let sum = build_summary(std::slice::from_ref(&dev));

        assert_eq!(sum.declared_but_broken.len(), 1);
        let gap = &sum.declared_but_broken[0];
        assert_eq!(gap.profile, "G");
        assert_eq!(gap.verdict, "unsupported");
        assert_eq!(gap.count, 1);
        assert_eq!(gap.sample_models, ["GV-X"]);
        assert_eq!(sum.declared_unverified.get("M"), Some(&1));
    }
}
