#![allow(non_snake_case)]
use crate::api;
use dioxus::prelude::*;
use oxvif::DiscoveredDevice;

#[component]
pub fn Home() -> Element {
    let mut devices: Signal<Vec<DiscoveredDevice>> = use_signal(Vec::new);
    let mut scanning = use_signal(|| false);
    let mut error: Signal<Option<String>> = use_signal(|| None);

    let do_scan = move |_| async move {
        scanning.set(true);
        error.set(None);
        match api::discover_devices().await {
            Ok(found) => devices.set(found),
            Err(e) => error.set(Some(e)),
        }
        scanning.set(false);
    };

    rsx! {
        div { class: "p-6 max-w-3xl mx-auto",
            h1 { class: "text-2xl font-bold mb-4 dark:text-white", "OxDM — Device Manager" }
            div { class: "flex items-center gap-4 mb-6",
                button {
                    class: "px-4 py-2 bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50",
                    disabled: *scanning.read(),
                    onclick: do_scan,
                    if *scanning.read() { "Scanning…" } else { "Scan Network" }
                }
                span { class: "text-sm text-gray-500 dark:text-gray-400",
                    "WS-Discovery · 3 s timeout"
                }
            }
            if let Some(err) = error.read().as_deref() {
                p { class: "text-red-500 mb-4", "{err}" }
            }
            if devices.read().is_empty() {
                p { class: "text-gray-400", "No devices found yet. Click Scan Network to start." }
            } else {
                ul { class: "divide-y divide-gray-200 dark:divide-gray-700",
                    for (i, dev) in devices.read().iter().enumerate() {
                        DeviceRow { key: "{i}", index: i, xaddrs: dev.xaddrs.clone(), scopes: dev.scopes.clone() }
                    }
                }
            }
        }
    }
}

#[component]
fn DeviceRow(index: usize, xaddrs: Vec<String>, scopes: Vec<String>) -> Element {
    let nav = use_navigator();
    let addr = xaddrs.first().cloned().unwrap_or_default();
    let display_addr = addr.clone();

    // Try to extract a friendly name from scopes like onvif://www.onvif.org/name/Camera1
    let name = scopes
        .iter()
        .find_map(|s| s.strip_prefix("onvif://www.onvif.org/name/"))
        .map(urlencoding_decode)
        .unwrap_or_else(|| format!("Device {}", index + 1));

    rsx! {
        li {
            class: "py-3 flex items-center justify-between hover:bg-gray-50 dark:hover:bg-gray-800 px-2 rounded cursor-pointer",
            onclick: move |_| {
                let encoded = urlencoding_encode(&addr);
                nav.push(crate::Route::CameraDetail { addr: encoded });
            },
            div {
                p { class: "font-medium dark:text-white", "{name}" }
                p { class: "text-sm text-gray-500", "{display_addr}" }
            }
            span { class: "text-blue-500 text-sm", "Open →" }
        }
    }
}

fn urlencoding_encode(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9'
            | b'-' | b'_' | b'.' | b'~' | b':' | b'/' | b'?' | b'#'
            | b'[' | b']' | b'@' | b'!' | b'$' | b'&' | b'\'' | b'('
            | b')' | b'*' | b'+' | b',' | b';' | b'=' => out.push(b as char),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}

fn urlencoding_decode(s: &str) -> String {
    let mut out = String::new();
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '%' {
            let h: String = chars.by_ref().take(2).collect();
            if let Ok(b) = u8::from_str_radix(&h, 16) {
                out.push(b as char);
            }
        } else if c == '+' {
            out.push(' ');
        } else {
            out.push(c);
        }
    }
    out
}
