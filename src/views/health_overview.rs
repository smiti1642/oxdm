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
use crate::components::{GroupCredentialsDialog, GroupDeviceCredentialsDialog, Icon};
use crate::state::{
    AuthStatus, CredSource, Credentials, Ctx, DeviceEntry, HealthDeviceRef, HealthGroup, ToastLevel,
};
use crate::views::settings::health::{verdict_class, verdict_label};
use crate::{api, i18n};
use dioxus::prelude::*;
use oxvif::health::{CheckStatus, HealthReport};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::time::Duration;

/// Hard cap so a single non-responsive device can't stall its row forever.
const DEVICE_TIMEOUT: Duration = Duration::from_secs(120);

const BUNDLE_SCHEMA: &str = "oxdm-health-batch/v1";

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

/// Active Profile G probe — real `FindRecordings` + `GetReplayUri` calls, with
/// the verbatim fault text preserved on failure.
#[derive(Clone, PartialEq, Serialize, Default)]
struct ProfileGProbe {
    /// Number of recordings returned by the search (on success).
    search_recordings: Option<usize>,
    /// Verbatim error/fault text from `FindRecordings` (on failure).
    search_error: Option<String>,
    /// Replay URI resolved for the first recording (on success).
    replay_uri: Option<String>,
    /// Verbatim error/fault text from `GetReplayUri` (on failure).
    replay_error: Option<String>,
}

#[derive(Clone, PartialEq, Serialize)]
struct DeviceResult {
    target: String,
    display_addr: String,
    name: String,
    fingerprint: Option<Fingerprint>,
    fingerprint_error: Option<String>,
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

#[derive(Serialize)]
struct ReportBundle {
    schema: &'static str,
    oxdm_version: &'static str,
    oxvif_version: &'static str,
    generated_at: String,
    redacted: bool,
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
    let mut redact = use_signal(|| false);
    let gcreds_open = use_signal(|| false);
    let dev_creds_open = use_signal(|| false);
    let dev_creds_addr = use_signal(String::new);

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

    // Drag start for an All-devices row: if the row is part of the checked
    // selection, drag the whole selection; otherwise just that device.
    let drag_start = use_callback(move |clicked: HealthDeviceRef| {
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
        ctx.dragging.clone().set(payload);
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
        let mut done: Vec<DeviceResult> = res
            .values()
            .filter_map(|s| match s {
                RunState::Done(r) => Some((**r).clone()),
                _ => None,
            })
            .collect();
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
        let bundle = ReportBundle {
            schema: BUNDLE_SCHEMA,
            oxdm_version: env!("CARGO_PKG_VERSION"),
            oxvif_version: crate::components::OXVIF_VERSION,
            generated_at: now_iso(),
            redacted: do_redact,
            devices: done,
        };
        let mut json = serde_json::to_string_pretty(&bundle).unwrap_or_default();
        if do_redact {
            json = redact_ipv4(&json);
        }
        spawn(async move {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name("oxdm-health-report.json")
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
                                on_dragstart: move |_| {},
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
                                on_dragstart: {
                                    let r = HealthDeviceRef {
                                        endpoint: d.endpoint.clone(),
                                        addr: d.addr.clone(),
                                        name: d.name.clone(),
                                    };
                                    move |_| drag_start.call(r.clone())
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
    on_dragstart: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    rsx! {
        div {
            class: if offline { "hbatch-row hbatch-row--offline" } else { "hbatch-row" },
            draggable,
            ondragstart: move |_| on_dragstart.call(()),
            ondragend: move |_| ctx.dragging.clone().set(Vec::new()),
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
            Err(e) => (None, Some(e)),
        };

        // oxvif's HealthCheck is read-only and infallible (errors become Fail
        // rows inside the report).
        let report = api::run_health_check(&d.addr, creds).await;

        // Active Profile G probe — the part oxvif's health check does not do.
        let mut probe = ProfileGProbe::default();
        match api::search_recordings(&d.addr, creds).await {
            Ok(recs) => {
                probe.search_recordings = Some(recs.len());
                if let Some(first) = recs.first() {
                    match api::get_replay_uri(&d.addr, creds, &first.recording_token).await {
                        Ok(uri) => probe.replay_uri = Some(uri),
                        Err(e) => probe.replay_error = Some(e),
                    }
                }
            }
            Err(e) => probe.search_error = Some(e),
        }

        DeviceResult {
            target: d.addr.clone(),
            display_addr: d.display_addr.clone(),
            name: d.name.clone(),
            fingerprint,
            fingerprint_error,
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

/// Local wall-clock stamp as `YYYY-MM-DDTHH:MM:SS`. Built from `OffsetDateTime`
/// components so we don't need the `time` crate's `formatting` feature.
fn now_iso() -> String {
    use time::OffsetDateTime;
    let t = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
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
