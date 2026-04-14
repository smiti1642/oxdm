#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx, ToastLevel, View};
use dioxus::prelude::*;

/// Modal for editing the global (default) credentials.
#[component]
pub fn GlobalCredentialsDialog(open: Signal<bool>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let is_open = *open.read();

    if !is_open {
        return rsx! {};
    }

    let creds = ctx.global_credentials.read();
    let mut username = use_signal(|| creds.username.clone());
    let mut password = use_signal(|| creds.password.clone());
    let mut show_pw = use_signal(|| false);
    drop(creds);

    let mut open_sig = open;
    let mut global_creds = ctx.global_credentials;

    rsx! {
        div {
            class: "dialog-overlay",
            onclick: move |_| open_sig.set(false),

            div {
                class: "dialog",
                onclick: |e| e.stop_propagation(),

                div { class: "dialog-header",
                    span { class: "dialog-title", {i18n::t(locale, "cred_global_title")} }
                }
                div { class: "dialog-body",
                    p { class: "dialog-hint", {i18n::t(locale, "cred_global_hint")} }
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "cred_username")} }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: i18n::t(locale, "cred_username"),
                            value: "{username}",
                            oninput: move |e| username.set(e.value()),
                        }
                    }
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "cred_password")} }
                        div { class: "form-input-row",
                            input {
                                class: "form-input form-input--flex",
                                r#type: if *show_pw.read() { "text" } else { "password" },
                                placeholder: i18n::t(locale, "cred_password"),
                                value: "{password}",
                                oninput: move |e| password.set(e.value()),
                            }
                            button {
                                class: "btn btn-ghost btn-sm",
                                onclick: move |_| show_pw.toggle(),
                                if *show_pw.read() {
                                    Icon { name: "eye-off", size: 14 }
                                } else {
                                    Icon { name: "eye", size: 14 }
                                }
                            }
                        }
                    }
                }
                div { class: "dialog-footer",
                    button {
                        class: "btn btn-md btn-ghost",
                        onclick: move |_| open_sig.set(false),
                        {i18n::t(locale, "btn_cancel")}
                    }
                    button {
                        class: "btn btn-md btn-primary",
                        onclick: move |_| {
                            global_creds.set(Credentials {
                                username: username.peek().clone(),
                                password: password.peek().clone(),
                            });
                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, "cred_saved"));
                            open_sig.set(false);
                        },
                        {i18n::t(locale, "btn_save")}
                    }
                }
            }
        }
    }
}

/// Modal for manually adding a device with optional credentials.
#[component]
pub fn AddDeviceDialog(open: Signal<bool>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let is_open = *open.read();

    if !is_open {
        return rsx! {};
    }

    let mut addr = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut show_pw = use_signal(|| false);
    let mut show_creds = use_signal(|| false);

    let mut open_sig = open;
    let mut devices = ctx.devices;
    let mut selected = ctx.selected;
    let mut view = ctx.view;

    rsx! {
        div {
            class: "dialog-overlay",
            onclick: move |_| open_sig.set(false),

            div {
                class: "dialog dialog--wide",
                onclick: |e| e.stop_propagation(),

                div { class: "dialog-header",
                    span { class: "dialog-title", {i18n::t(locale, "add_device_title")} }
                }
                div { class: "dialog-body",
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "add_device_addr")} }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: i18n::t(locale, "add_device_addr_hint"),
                            value: "{addr}",
                            oninput: move |e| addr.set(e.value()),
                        }
                        p { class: "form-hint", {i18n::t(locale, "add_device_addr_auto")} }
                    }
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "add_device_name")} }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: i18n::t(locale, "add_device_name_hint"),
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                    }

                    button {
                        class: "btn btn-ghost btn-sm form-toggle",
                        onclick: move |_| show_creds.toggle(),
                        if *show_creds.read() {
                            Icon { name: "chevron-down", size: 12 }
                        } else {
                            Icon { name: "chevron-right", size: 12 }
                        }
                        " "
                        {i18n::t(locale, "add_device_custom_creds")}
                    }

                    if *show_creds.read() {
                        div { class: "form-field",
                            label { class: "form-label", {i18n::t(locale, "cred_username")} }
                            input {
                                class: "form-input",
                                r#type: "text",
                                placeholder: i18n::t(locale, "cred_username"),
                                value: "{username}",
                                oninput: move |e| username.set(e.value()),
                            }
                        }
                        div { class: "form-field",
                            label { class: "form-label", {i18n::t(locale, "cred_password")} }
                            div { class: "form-input-row",
                                input {
                                    class: "form-input form-input--flex",
                                    r#type: if *show_pw.read() { "text" } else { "password" },
                                    placeholder: i18n::t(locale, "cred_password"),
                                    value: "{password}",
                                    oninput: move |e| password.set(e.value()),
                                }
                                button {
                                    class: "btn btn-ghost btn-sm",
                                    onclick: move |_| show_pw.toggle(),
                                    if *show_pw.read() {
                                        Icon { name: "eye-off", size: 14 }
                                    } else {
                                        Icon { name: "eye", size: 14 }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "dialog-footer",
                    button {
                        class: "btn btn-md btn-ghost",
                        onclick: move |_| open_sig.set(false),
                        {i18n::t(locale, "btn_cancel")}
                    }
                    button {
                        class: "btn btn-md btn-primary",
                        disabled: addr.read().trim().is_empty(),
                        onclick: move |_| {
                            let raw = addr.peek().trim().to_string();
                            let addr_val = normalize_onvif_addr(&raw);
                            let name_val = name.peek().trim().to_string();
                            let display = extract_ip(&addr_val);
                            let dev_name = if name_val.is_empty() { display.clone() } else { name_val };

                            let creds = if *show_creds.peek() {
                                let u = username.peek().clone();
                                let p = password.peek().clone();
                                if u.is_empty() && p.is_empty() {
                                    None
                                } else {
                                    Some(Credentials { username: u, password: p })
                                }
                            } else {
                                None
                            };

                            let mut devs = devices.write();
                            devs.push(crate::state::DeviceEntry {
                                name: dev_name,
                                addr: addr_val,
                                display_addr: display,
                                firmware: String::new(),
                                online: false,
                                manual: true,
                                credentials: creds,
                            });
                            let new_idx = devs.len() - 1;
                            drop(devs);

                            // Auto-select the newly added device
                            selected.set(Some(new_idx));
                            view.set(View::DeviceSettings);

                            ctx.push_toast(ToastLevel::Info, i18n::t(locale, "add_device_ok"));
                            open_sig.set(false);
                        },
                        {i18n::t(locale, "btn_add_short")}
                    }
                }
            }
        }
    }
}

/// Normalize user input to a full ONVIF device service URL.
/// - "192.168.1.10"          → "http://192.168.1.10/onvif/device_service"
/// - "192.168.1.10:8080"     → "http://192.168.1.10:8080/onvif/device_service"
/// - "http://192.168.1.10"   → "http://192.168.1.10/onvif/device_service"
/// - Full URL                → kept as-is
pub fn normalize_onvif_addr(input: &str) -> String {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return String::new();
    }

    // If it already has a path component, keep as-is
    if trimmed.starts_with("http://") || trimmed.starts_with("https://") {
        let after_scheme = trimmed
            .strip_prefix("http://")
            .or_else(|| trimmed.strip_prefix("https://"))
            .unwrap_or(trimmed);
        // Check if there's a path beyond the host
        if after_scheme.contains('/') {
            return trimmed.to_string();
        }
        // Has scheme but no path — append default path
        return format!("{trimmed}/onvif/device_service");
    }

    // Bare IP or IP:port — prepend http:// and append default path
    format!("http://{trimmed}/onvif/device_service")
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
