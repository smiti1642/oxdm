#![allow(non_snake_case)]
use crate::{
    api,
    components::{
        AddDeviceDialog, ContextMenu, CtxMenuItem, EditDeviceDialog, GlobalCredentialsDialog, Icon,
    },
    i18n,
    state::{AuthStatus, ConfirmDialog, Ctx, DeviceEntry, DeviceListTab, ToastLevel, View},
    util,
};
use dioxus::prelude::*;

/// Which subset of the device list to show. Pairs with the tab filter.
#[derive(Clone, Copy, PartialEq)]
enum StatusFilter {
    All,
    Ok,
    Failed,
    Unknown,
}

impl StatusFilter {
    fn matches(self, status: AuthStatus) -> bool {
        match self {
            Self::All => true,
            Self::Ok => status == AuthStatus::Ok,
            Self::Failed => status == AuthStatus::Failed,
            Self::Unknown => status == AuthStatus::Unknown,
        }
    }
    fn as_str(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Ok => "ok",
            Self::Failed => "failed",
            Self::Unknown => "unknown",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "ok" => Self::Ok,
            "failed" => Self::Failed,
            "unknown" => Self::Unknown,
            _ => Self::All,
        }
    }
}

/// How to order devices within the tab. `Default` keeps insertion order,
/// which for Discovered ≈ the order WS-Discovery responses came back in.
#[derive(Clone, Copy, PartialEq)]
enum SortBy {
    Default,
    Name,
    Ip,
}

impl SortBy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Name => "name",
            Self::Ip => "ip",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "name" => Self::Name,
            "ip" => Self::Ip,
            _ => Self::Default,
        }
    }
}

/// Parse a dotted-quad IPv4 into its packed u32 for numeric sort.
/// Non-IPv4 strings collide at `u32::MAX`, which is fine — they end up
/// sorted together at the bottom.
fn ip_to_u32(addr: &str) -> u32 {
    let octets: Vec<u8> = addr
        .split('.')
        .filter_map(|p| p.parse::<u8>().ok())
        .collect();
    if octets.len() == 4 {
        u32::from_be_bytes([octets[0], octets[1], octets[2], octets[3]])
    } else {
        u32::MAX
    }
}

#[component]
pub fn DeviceList() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut filter = use_signal(String::new);
    let mut add_dialog_open = use_signal(|| false);
    let mut creds_open = use_signal(|| false);
    let edit_dialog_open = use_signal(|| false);
    let edit_device_idx: Signal<Option<usize>> = use_signal(|| None);
    let mut list_tab = use_signal(|| DeviceListTab::Discovered);
    let mut status_filter = use_signal(|| StatusFilter::All);
    let mut sort_by = use_signal(|| SortBy::Default);

    let creds = ctx.global_credentials.read();
    let creds_empty = creds.username.is_empty();
    let creds_username = creds.username.clone();
    drop(creds);

    let mut scanning = ctx.scanning;
    let mut selected = ctx.selected;
    let mut view = ctx.view;
    let mut devices = ctx.devices;

    let active_tab = *list_tab.read();

    // Progressive scan: drive 3 single-round probes back-to-back so the UI
    // fills in roughly every 2 s instead of blocking on one ~9 s call.
    // Per-device auth/firmware probes fire the moment a device first appears
    // (tracked in `auth_started` to avoid re-probing on subsequent rounds).
    // Callback wrapper so the keyboard shortcut effect can fire the same
    // scan as the toolbar button without duplicating the body.
    let do_scan = use_callback(move |_: ()| {
        spawn(async move {
            use std::collections::HashSet;
            const ROUNDS: u32 = 3;
            const PROBE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(2);
            const PROBE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(800);

            scanning.set(true);

            // Remember current selection to try to preserve it.
            let prev_addr = selected
                .peek()
                .and_then(|i| devices.peek().get(i).map(|d| d.addr.clone()));

            // Mark all non-manual entries offline up front. Each round flips
            // re-discovered ones back online; ones that never reappear stay
            // greyed so the UI can show them as stale rather than vanishing.
            {
                let mut snap = devices.peek().clone();
                for entry in snap.iter_mut() {
                    if !entry.manual {
                        entry.online = false;
                    }
                }
                devices.set(snap);
            }

            let mut auth_started: HashSet<String> = HashSet::new();
            let mut total_seen: HashSet<String> = HashSet::new();
            let mut had_error = false;

            for round in 0..ROUNDS {
                let found = match api::discover_one_round(PROBE_TIMEOUT).await {
                    Ok(v) => v,
                    Err(e) => {
                        ctx.push_toast(ToastLevel::Error, e);
                        had_error = true;
                        break;
                    }
                };

                let mut next: Vec<DeviceEntry> = devices.peek().clone();
                // Devices first appearing this round → start their auth probe.
                let mut new_addrs: Vec<String> = Vec::new();

                for d in found {
                    let addr = d.xaddrs.first().cloned().unwrap_or_default();
                    let display_addr = util::extract_ip(&addr);
                    let name = d
                        .scopes
                        .iter()
                        .find_map(|s| s.strip_prefix("onvif://www.onvif.org/name/"))
                        .map(str::to_string)
                        .unwrap_or_else(|| display_addr.clone());
                    let location = d
                        .scopes
                        .iter()
                        .find_map(|s| s.strip_prefix("onvif://www.onvif.org/location/"))
                        .map(util::urldecode)
                        .unwrap_or_default();
                    let endpoint = d.endpoint.clone();

                    total_seen.insert(endpoint.clone());

                    // URI conflict: any other entry still claiming one of this
                    // device's xaddrs is stale (its IP got reassigned).
                    if !addr.is_empty() {
                        for other in next.iter_mut() {
                            if other.endpoint != endpoint && !other.manual && other.addr == addr {
                                other.online = false;
                            }
                        }
                    }

                    let existing_idx = if !endpoint.is_empty() {
                        next.iter()
                            .position(|e| !e.manual && e.endpoint == endpoint)
                    } else {
                        next.iter().position(|e| !e.manual && e.addr == addr)
                    };

                    match existing_idx {
                        Some(i) => {
                            // Refresh discovery-derived fields; preserve
                            // auth_status / firmware (background tasks own those).
                            let e = &mut next[i];
                            e.name = name;
                            e.addr = addr.clone();
                            e.display_addr = display_addr;
                            e.location = location;
                            e.online = true;
                            e.endpoint = endpoint.clone();
                        }
                        None => {
                            next.push(DeviceEntry {
                                name,
                                addr: addr.clone(),
                                display_addr,
                                firmware: String::new(),
                                location,
                                online: true,
                                auth_status: Default::default(),
                                manual: false,
                                credentials: None,
                                endpoint: endpoint.clone(),
                            });
                        }
                    }

                    // Kick off auth/firmware probe exactly once per endpoint
                    // per scan, however many rounds it gets re-discovered in.
                    let key = if endpoint.is_empty() {
                        addr.clone()
                    } else {
                        endpoint.clone()
                    };
                    if !key.is_empty() && auth_started.insert(key) {
                        new_addrs.push(addr);
                    }
                }

                devices.set(next);

                // Fire per-device probes. spawn'd inside fetch_firmware_for_addr,
                // so the UI keeps painting while these run in the background.
                for addr in new_addrs {
                    crate::device_ops::fetch_firmware_for_addr(ctx, devices, addr);
                }

                if round + 1 < ROUNDS {
                    tokio::time::sleep(PROBE_INTERVAL).await;
                }
            }

            // Restore selection (or fall back to Welcome if device gone).
            let new_sel = prev_addr
                .as_ref()
                .and_then(|addr| devices.peek().iter().position(|d| &d.addr == addr));
            selected.set(new_sel);
            if new_sel.is_none() {
                view.set(View::Welcome);
            }

            if !had_error {
                let locale = *ctx.locale.read();
                let total = total_seen.len();
                if total > 0 {
                    ctx.push_toast(
                        ToastLevel::Success,
                        i18n::t(locale, "scan_found").replace("{n}", &total.to_string()),
                    );
                } else {
                    ctx.push_toast(ToastLevel::Warning, i18n::t(locale, "scan_none"));
                }
            }

            scanning.set(false);
        });
    });

    // Keyboard shortcut dispatcher: react to global key presses set by the
    // App-level onkeydown. Each branch self-clears the slot so the same
    // press doesn't fire twice if some other signal causes a re-render.
    let mut keyboard_action_sig = ctx.keyboard_action;
    use_effect(move || {
        let action = match *keyboard_action_sig.read() {
            Some(a) => a,
            None => return,
        };
        match action {
            crate::state::GlobalKey::FocusSearch => {
                let _ = document::eval(
                    "const el = document.getElementById('device-list-filter'); if (el) el.focus();",
                );
            }
            crate::state::GlobalKey::Scan => {
                if !*ctx.scanning.peek() {
                    do_scan.call(());
                }
            }
            crate::state::GlobalKey::NavUp | crate::state::GlobalKey::NavDown => {
                let len = ctx.devices.peek().len();
                if len > 0 {
                    let cur = ctx.selected.peek().unwrap_or(0);
                    let next = if matches!(action, crate::state::GlobalKey::NavUp) {
                        if cur == 0 {
                            len - 1
                        } else {
                            cur - 1
                        }
                    } else {
                        (cur + 1) % len
                    };
                    ctx.selected.clone().set(Some(next));
                }
            }
        }
        keyboard_action_sig.set(None);
    });

    let is_scanning = *ctx.scanning.read();
    let filter_str = filter.read().to_lowercase();
    let devs = ctx.devices.read();
    let sel = *ctx.selected.read();

    let active_status = *status_filter.read();
    let active_sort = *sort_by.read();

    let mut filtered: Vec<(usize, &DeviceEntry)> = devs
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            // Filter by active tab
            let tab_match = match active_tab {
                DeviceListTab::Discovered => !d.manual,
                DeviceListTab::Manual => d.manual,
            };
            tab_match
                && active_status.matches(d.auth_status)
                && (filter_str.is_empty()
                    || d.name.to_lowercase().contains(&filter_str)
                    || d.display_addr.contains(&filter_str))
        })
        .collect();

    // Sort last so the tab/filter work is done up front. Name is
    // case-insensitive so mixed-case device names don't scatter; IP uses
    // numeric octets to avoid the string-sort "1.2" < "1.10" wrong-order.
    match active_sort {
        SortBy::Default => {}
        SortBy::Name => filtered.sort_by_key(|(_, d)| d.name.to_lowercase()),
        SortBy::Ip => filtered.sort_by_key(|(_, d)| ip_to_u32(&d.display_addr)),
    }

    // Tab badges reflect the current filter + search, not the raw totals
    // — otherwise "Discovered (20)" next to a list showing only 3 matches
    // is confusing. Each badge answers "how many entries in this tab
    // match my current filters?".
    let matches_filters = |d: &DeviceEntry| {
        active_status.matches(d.auth_status)
            && (filter_str.is_empty()
                || d.name.to_lowercase().contains(&filter_str)
                || d.display_addr.contains(&filter_str))
    };
    let discovered_count = devs
        .iter()
        .filter(|d| !d.manual && matches_filters(d))
        .count();
    let manual_count = devs
        .iter()
        .filter(|d| d.manual && matches_filters(d))
        .count();

    rsx! {
        aside { class: "sidebar",

            div { class: "sidebar-header",
                span { class: "sidebar-title", {i18n::t(locale, "sidebar_title")} }
                button {
                    class: if creds_empty { "cred-indicator cred-indicator--empty" } else { "cred-indicator" },
                    onclick: move |_| creds_open.set(true),
                    if creds_empty {
                        Icon { name: "key", size: 12 }
                        span { class: "cred-indicator-text", {i18n::t(locale, "not_logged_in")} }
                    } else {
                        span { class: "cred-indicator-text", "{creds_username}" }
                        Icon { name: "key", size: 12 }
                    }
                }
            }

            // ── Tab bar ─────────────────────────────────────────────────────
            div { class: "sidebar-tabs",
                button {
                    class: if active_tab == DeviceListTab::Discovered { "sidebar-tab sidebar-tab--active" } else { "sidebar-tab" },
                    onclick: move |_| list_tab.set(DeviceListTab::Discovered),
                    {i18n::t(locale, "devtab_discovered")}
                    if discovered_count > 0 {
                        span { class: "sidebar-tab-badge", "{discovered_count}" }
                    }
                }
                button {
                    class: if active_tab == DeviceListTab::Manual { "sidebar-tab sidebar-tab--active" } else { "sidebar-tab" },
                    onclick: move |_| list_tab.set(DeviceListTab::Manual),
                    {i18n::t(locale, "devtab_manual")}
                    if manual_count > 0 {
                        span { class: "sidebar-tab-badge", "{manual_count}" }
                    }
                }
            }

            div { class: "sidebar-search",
                input {
                    id: "device-list-filter",
                    class: "search-input",
                    placeholder: i18n::t(locale, "filter_placeholder"),
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }

            // Filter + sort controls. Narrow sidebar → two compact
            // side-by-side selects. Resets and defaults are local state,
            // not persisted — users who want a specific view re-pick each
            // session. Keeps the UI discoverable without a hidden popup.
            div { class: "sidebar-filters",
                select {
                    class: "sidebar-filter-select",
                    title: i18n::t(locale, "filter_status_tooltip"),
                    value: "{active_status.as_str()}",
                    onchange: move |e| status_filter.set(StatusFilter::from_str(&e.value())),
                    option { value: "all",     {i18n::t(locale, "filter_status_all")} }
                    option { value: "ok",      {i18n::t(locale, "filter_status_ok")} }
                    option { value: "failed",  {i18n::t(locale, "filter_status_failed")} }
                    option { value: "unknown", {i18n::t(locale, "filter_status_unknown")} }
                }
                select {
                    class: "sidebar-filter-select",
                    title: i18n::t(locale, "filter_sort_tooltip"),
                    value: "{active_sort.as_str()}",
                    onchange: move |e| sort_by.set(SortBy::from_str(&e.value())),
                    option { value: "default", {i18n::t(locale, "filter_sort_default")} }
                    option { value: "name",    {i18n::t(locale, "filter_sort_name")} }
                    option { value: "ip",      {i18n::t(locale, "filter_sort_ip")} }
                }
            }

            div { class: "device-list",
                if filtered.is_empty() {
                    div { class: "device-empty",
                        match active_tab {
                            DeviceListTab::Discovered => {
                                if devs.iter().any(|d| !d.manual) {
                                    rsx! { {i18n::t(locale, "no_matches")} }
                                } else {
                                    rsx! { {i18n::t(locale, "no_devices")} }
                                }
                            }
                            DeviceListTab::Manual => {
                                if devs.iter().any(|d| d.manual) {
                                    rsx! { {i18n::t(locale, "no_matches")} }
                                } else {
                                    rsx! { {i18n::t(locale, "no_manual_devices")} }
                                }
                            }
                        }
                    }
                }
                for (i, dev) in filtered {
                    DeviceCard {
                        key: "{i}",
                        index: i,
                        name: dev.name.clone(),
                        display_addr: dev.display_addr.clone(),
                        firmware: dev.firmware.clone(),
                        location: dev.location.clone(),
                        online: dev.online,
                        manual: dev.manual,
                        selected: sel == Some(i),
                        auth_status: dev.auth_status,
                        edit_dialog_open,
                        edit_device_idx,
                    }
                }
            }

            // ── Footer: context-dependent buttons ───────────────────────────
            div { class: "sidebar-footer",
                match active_tab {
                    DeviceListTab::Discovered => rsx! {
                        button {
                            class: "btn btn-primary btn-sm btn-scan",
                            disabled: is_scanning,
                            onclick: move |_| do_scan.call(()),
                            if is_scanning {
                                {i18n::t(locale, "btn_scanning")}
                            } else {
                                span { class: "btn-icon", Icon { name: "refresh-cw", size: 13 } }
                                {i18n::t(locale, "btn_scan_label")}
                            }
                        }
                    },
                    DeviceListTab::Manual => rsx! {
                        button {
                            class: "btn btn-primary btn-sm btn-scan",
                            onclick: move |_| add_dialog_open.set(true),
                            span { class: "btn-icon", Icon { name: "plus", size: 13 } }
                            {i18n::t(locale, "btn_add_label")}
                        }
                    },
                }
            }
        }

        AddDeviceDialog { open: add_dialog_open }
        GlobalCredentialsDialog { open: creds_open }
        EditDeviceDialog { open: edit_dialog_open, device_index: edit_device_idx }
    }
}

#[component]
fn DeviceCard(
    index: usize,
    name: String,
    display_addr: String,
    firmware: String,
    location: String,
    online: bool,
    manual: bool,
    selected: bool,
    auth_status: crate::state::AuthStatus,
    edit_dialog_open: Signal<bool>,
    edit_device_idx: Signal<Option<usize>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut sel = ctx.selected;
    let mut view = ctx.view;
    let mut devices = ctx.devices;

    let mut ctx_menu: Signal<Option<(f64, f64)>> = use_signal(|| None);

    let auth_class = match auth_status {
        crate::state::AuthStatus::Ok => " device-card--auth-ok",
        crate::state::AuthStatus::Failed => " device-card--auth-fail",
        crate::state::AuthStatus::Unknown => "",
    };
    let card_class = if selected {
        format!("device-card device-card--selected{auth_class}")
    } else {
        format!("device-card{auth_class}")
    };
    let dot_class = if manual {
        "status-dot status-dot--manual"
    } else if online {
        "status-dot status-dot--online"
    } else {
        "status-dot status-dot--offline"
    };

    let card_name = name.clone();
    let card_addr = display_addr.clone();

    rsx! {
        div {
            class: card_class,
            onclick: move |_| {
                sel.set(Some(index));
                view.set(View::DeviceSettings);
            },
            oncontextmenu: move |e| {
                e.prevent_default();
                let coords = e.data().client_coordinates();
                ctx_menu.set(Some((coords.x, coords.y)));
            },
            div { class: "device-card-header",
                span { class: dot_class }
                span { class: "device-name", "{name}" }
            }
            div { class: "device-addr", "{display_addr}" }
            if !firmware.is_empty() {
                div { class: "device-firmware", "FW {firmware}" }
            }
            if !location.is_empty() {
                div { class: "device-location", "{location}" }
            }
        }

        if let Some((mx, my)) = *ctx_menu.read() {
            ContextMenu {
                x: mx,
                y: my,
                on_close: move |_| ctx_menu.set(None),

                // ── Shared: Copy address (safe clipboard, no eval) ──────
                CtxMenuItem {
                    icon: "clipboard-copy",
                    label: i18n::t(locale, "ctx_copy_addr"),
                    on_click: move |_| {
                        if let Err(e) = util::copy_to_clipboard(&card_addr) {
                            ctx.push_toast(ToastLevel::Error, e);
                        } else {
                            ctx.push_toast(ToastLevel::Info, i18n::t(locale, "ctx_copied"));
                        }
                        ctx_menu.set(None);
                    },
                }

                // ── Manual-only actions ─────────────────────────────────
                if manual {
                    CtxMenuItem {
                        icon: "settings",
                        label: i18n::t(locale, "ctx_edit"),
                        on_click: move |_| {
                            ctx_menu.set(None);
                            edit_device_idx.clone().set(Some(index));
                            edit_dialog_open.clone().set(true);
                        },
                    }
                    CtxMenuItem {
                        icon: "trash-2",
                        label: i18n::t(locale, "ctx_delete"),
                        danger: true,
                        on_click: move |_| {
                            let dev_name = card_name.clone();
                            ctx_menu.set(None);
                            ctx.dialog.clone().set(Some(ConfirmDialog {
                                title: i18n::t(locale, "ctx_delete").to_string(),
                                message: i18n::t(locale, "ctx_delete_confirm")
                                    .replace("{name}", &dev_name),
                                confirm_label: i18n::t(locale, "btn_confirm").to_string(),
                                cancel_label: i18n::t(locale, "btn_cancel").to_string(),
                                dangerous: true,
                                on_confirm: EventHandler::new(move |_| {
                                    devices.write().remove(index);
                                    let current_sel = *ctx.selected.peek();
                                    if current_sel == Some(index) {
                                        ctx.selected.clone().set(None);
                                        ctx.view.clone().set(View::Welcome);
                                    } else if let Some(s) = current_sel {
                                        if s > index {
                                            ctx.selected.clone().set(Some(s - 1));
                                        }
                                    }
                                }),
                            }));
                        },
                    }
                }

                // ── Discovered-only actions ─────────────────────────────
                if !manual {
                    CtxMenuItem {
                        icon: "plus",
                        label: i18n::t(locale, "ctx_add_manual"),
                        on_click: move |_| {
                            ctx_menu.set(None);
                            let snapshot = devices.peek().get(index).cloned();
                            if let Some(dev) = snapshot {
                                let creds = ctx.global_credentials.peek().clone();
                                let cred = if creds.username.is_empty() { None } else { Some(creds) };
                                devices.write().push(DeviceEntry {
                                    name: dev.name,
                                    addr: dev.addr,
                                    display_addr: dev.display_addr,
                                    firmware: dev.firmware,
                                    location: dev.location,
                                    online: false,
                                    auth_status: dev.auth_status,
                                    manual: true,
                                    credentials: cred,
                                    endpoint: dev.endpoint,
                                });
                            }
                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, "ctx_added_manual"));
                        },
                    }
                }
            }
        }
    }
}
