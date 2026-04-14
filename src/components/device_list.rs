#![allow(non_snake_case)]
use crate::{
    api,
    components::{AddDeviceDialog, Icon},
    i18n,
    state::{Ctx, DeviceEntry, ToastLevel, View},
};
use dioxus::prelude::*;

#[component]
pub fn DeviceList() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut filter = use_signal(String::new);
    let mut add_dialog_open = use_signal(|| false);

    let mut scanning = ctx.scanning;
    let mut selected = ctx.selected;
    let mut view = ctx.view;
    let mut devices = ctx.devices;

    let do_scan = move |_| async move {
        scanning.set(true);

        // Remember current selection to try to preserve it
        let prev_addr = selected
            .peek()
            .and_then(|i| devices.peek().get(i).map(|d| d.addr.clone()));

        match api::discover_devices().await {
            Ok(found) => {
                let count = found.len();
                let entries: Vec<DeviceEntry> = found
                    .into_iter()
                    .map(|d| {
                        let addr = d.xaddrs.first().cloned().unwrap_or_default();
                        let display_addr = extract_ip(&addr);
                        let name = d
                            .scopes
                            .iter()
                            .find_map(|s| s.strip_prefix("onvif://www.onvif.org/name/"))
                            .map(str::to_string)
                            .unwrap_or_else(|| display_addr.clone());
                        DeviceEntry {
                            name,
                            addr,
                            display_addr,
                            firmware: String::new(),
                            online: true,
                            manual: false,
                            credentials: None,
                        }
                    })
                    .collect();

                // Preserve manually added devices
                let manual: Vec<DeviceEntry> = devices
                    .peek()
                    .iter()
                    .filter(|d| d.manual)
                    .cloned()
                    .collect();

                let mut all = entries;
                all.extend(manual);

                // Try to restore previous selection
                let new_sel = prev_addr
                    .as_ref()
                    .and_then(|addr| all.iter().position(|d| &d.addr == addr));

                devices.set(all);
                selected.set(new_sel);

                if new_sel.is_none() {
                    view.set(View::Welcome);
                }

                let locale = *ctx.locale.read();
                ctx.push_toast(
                    ToastLevel::Success,
                    i18n::t(locale, "scan_found").replace("{n}", &count.to_string()),
                );
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
            filter_str.is_empty()
                || d.name.to_lowercase().contains(&filter_str)
                || d.display_addr.contains(&filter_str)
        })
        .collect();

    rsx! {
        aside { class: "sidebar",

            div { class: "sidebar-header",
                span { class: "sidebar-title", {i18n::t(locale, "sidebar_title")} }
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
                        if devs.is_empty() {
                            {i18n::t(locale, "no_devices")}
                        } else {
                            {i18n::t(locale, "no_matches")}
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
                        online: dev.online,
                        manual: dev.manual,
                        selected: sel == Some(i),
                    }
                }
            }

            div { class: "sidebar-footer",
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
                button {
                    class: "btn btn-ghost btn-sm",
                    onclick: move |_| add_dialog_open.set(true),
                    span { class: "btn-icon", Icon { name: "plus", size: 13 } }
                    {i18n::t(locale, "btn_add_label")}
                }
            }
        }

        AddDeviceDialog { open: add_dialog_open }
    }
}

#[component]
fn DeviceCard(
    index: usize,
    name: String,
    display_addr: String,
    firmware: String,
    online: bool,
    manual: bool,
    selected: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let mut sel = ctx.selected;
    let mut view = ctx.view;

    let card_class = if selected {
        "device-card device-card--selected"
    } else {
        "device-card"
    };
    let dot_class = if manual {
        "status-dot status-dot--manual"
    } else if online {
        "status-dot status-dot--online"
    } else {
        "status-dot status-dot--offline"
    };

    rsx! {
        div {
            class: card_class,
            onclick: move |_| {
                sel.set(Some(index));
                // Auto-navigate to Settings when selecting a device
                view.set(View::DeviceSettings);
            },
            div { class: "device-card-header",
                span { class: dot_class }
                span { class: "device-name", "{name}" }
            }
            div { class: "device-addr", "{display_addr}" }
            if !firmware.is_empty() {
                div { class: "device-firmware", "{firmware}" }
            }
        }
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
