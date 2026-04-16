#![allow(non_snake_case)]
use crate::{
    api,
    components::{
        AddDeviceDialog, ContextMenu, CtxMenuItem, EditDeviceDialog, GlobalCredentialsDialog, Icon,
    },
    i18n,
    state::{ConfirmDialog, Ctx, DeviceEntry, DeviceListTab, ToastLevel, View},
    util,
};
use dioxus::prelude::*;

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

    let creds = ctx.global_credentials.read();
    let creds_empty = creds.username.is_empty();
    let creds_username = creds.username.clone();
    drop(creds);

    let mut scanning = ctx.scanning;
    let mut selected = ctx.selected;
    let mut view = ctx.view;
    let mut devices = ctx.devices;

    let active_tab = *list_tab.read();

    let do_scan = move |_| async move {
        scanning.set(true);

        // Remember current selection to try to preserve it
        let prev_addr = selected
            .peek()
            .and_then(|i| devices.peek().get(i).map(|d| d.addr.clone()));

        match api::discover_devices().await {
            Ok(found) => {
                let count = found.len();

                // Build the merged list:
                //   1. Start from the existing devices.
                //   2. For each discovered device, either update an existing
                //      entry (matched by endpoint UUID — stable across IP
                //      changes) or append it as new.
                //   3. Discovered entries not seen this round get online=false
                //      so the UI can show them as stale rather than vanishing
                //      mid-session (manual entries are untouched).
                //   4. URI conflict: if the discovered device's xaddrs collide
                //      with another existing entry's xaddrs, mark that other
                //      entry offline — its IP got reassigned.
                let mut next: Vec<DeviceEntry> = devices.peek().clone();

                // Pass 1: discovered entries that didn't come back this round
                // are stale until proven otherwise.
                for entry in next.iter_mut() {
                    if !entry.manual {
                        entry.online = false;
                    }
                }

                // Pass 2: merge each discovery hit.
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

                    // URI conflict: any *other* entry that still claims one of
                    // this device's xaddrs is now stale (IP reassigned).
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
                        // No endpoint UUID — fall back to addr match.
                        next.iter().position(|e| !e.manual && e.addr == addr)
                    };

                    match existing_idx {
                        Some(i) => {
                            // Refresh fields, preserve auth_status and
                            // firmware (those are populated by separate
                            // background tasks and must survive a re-scan).
                            let e = &mut next[i];
                            e.name = name;
                            e.addr = addr;
                            e.display_addr = display_addr;
                            e.location = location;
                            e.online = true;
                            e.endpoint = endpoint;
                        }
                        None => {
                            next.push(DeviceEntry {
                                name,
                                addr,
                                display_addr,
                                firmware: String::new(),
                                location,
                                online: true,
                                auth_status: Default::default(),
                                manual: false,
                                credentials: None,
                                endpoint,
                            });
                        }
                    }
                }

                // Try to restore previous selection
                let new_sel = prev_addr
                    .as_ref()
                    .and_then(|addr| next.iter().position(|d| &d.addr == addr));

                devices.set(next);
                selected.set(new_sel);

                if new_sel.is_none() {
                    view.set(View::Welcome);
                }

                let locale = *ctx.locale.read();
                if count > 0 {
                    ctx.push_toast(
                        ToastLevel::Success,
                        i18n::t(locale, "scan_found").replace("{n}", &count.to_string()),
                    );
                    // Background: fetch firmware version for each discovered device
                    crate::device_ops::fetch_firmware_for_all(ctx, devices);
                } else {
                    ctx.push_toast(ToastLevel::Warning, i18n::t(locale, "scan_none"));
                }
            }
            Err(e) => {
                ctx.push_toast(ToastLevel::Error, e);
            }
        }

        scanning.set(false);
    };

    let is_scanning = *ctx.scanning.read();
    let filter_str = filter.read().to_lowercase();
    let devs = ctx.devices.read();
    let sel = *ctx.selected.read();

    let filtered: Vec<(usize, &DeviceEntry)> = devs
        .iter()
        .enumerate()
        .filter(|(_, d)| {
            // Filter by active tab
            let tab_match = match active_tab {
                DeviceListTab::Discovered => !d.manual,
                DeviceListTab::Manual => d.manual,
            };
            tab_match
                && (filter_str.is_empty()
                    || d.name.to_lowercase().contains(&filter_str)
                    || d.display_addr.contains(&filter_str))
        })
        .collect();

    let discovered_count = devs.iter().filter(|d| !d.manual).count();
    let manual_count = devs.iter().filter(|d| d.manual).count();

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
                    class: "search-input",
                    placeholder: i18n::t(locale, "filter_placeholder"),
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
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
                            onclick: do_scan,
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
