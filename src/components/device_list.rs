#![allow(non_snake_case)]
use crate::{
    api,
    state::{Ctx, DeviceEntry, View},
};
use dioxus::prelude::*;

#[component]
pub fn DeviceList() -> Element {
    let ctx = use_context::<Ctx>();
    let mut filter = use_signal(String::new);

    let mut scanning = ctx.scanning;
    let mut selected = ctx.selected;
    let mut view = ctx.view;
    let mut devices = ctx.devices;

    let do_scan = move |_| async move {
        scanning.set(true);
        selected.set(None);
        view.set(View::Welcome);

        if let Ok(found) = api::discover_devices().await {
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
                    }
                })
                .collect();
            devices.set(entries);
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
                span { class: "sidebar-title", "Devices" }
            }

            div { class: "sidebar-search",
                input {
                    class: "search-input",
                    placeholder: "Name or address…",
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
            }

            div { class: "device-list",
                if filtered.is_empty() {
                    div { class: "device-empty",
                        if devs.is_empty() {
                            "No devices found.\nClick Scan to discover."
                        } else {
                            "No matches."
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
                        selected: sel == Some(i),
                    }
                }
            }

            div { class: "sidebar-footer",
                button {
                    class: "btn btn-primary btn-sm btn-scan",
                    disabled: is_scanning,
                    onclick: do_scan,
                    if is_scanning { "Scanning…" } else { "⟳  Scan" }
                }
                button { class: "btn btn-ghost btn-sm", "＋ Add" }
            }
        }
    }
}

#[component]
fn DeviceCard(
    index: usize,
    name: String,
    display_addr: String,
    firmware: String,
    online: bool,
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
    let dot_class = if online {
        "status-dot status-dot--online"
    } else {
        "status-dot status-dot--offline"
    };

    rsx! {
        div {
            class: card_class,
            onclick: move |_| {
                sel.set(Some(index));
                view.set(View::Welcome);
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
