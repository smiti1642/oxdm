#![allow(non_snake_case)]
use crate::components::PasswordField;
use crate::i18n;
use crate::state::{Credentials, Ctx, ToastLevel};
use dioxus::prelude::*;

/// Modal for editing a manual device's name and credentials.
#[component]
pub fn EditDeviceDialog(open: Signal<bool>, device_index: Signal<Option<usize>>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    // Hooks called unconditionally
    let mut name = use_signal(String::new);
    let mut username = use_signal(String::new);
    let mut password = use_signal(String::new);

    let is_open = *open.read();
    let idx = *device_index.read();

    if !is_open || idx.is_none() {
        return rsx! {};
    }
    let idx = idx.unwrap();

    // Sync signals with current device state when dialog opens
    {
        let devices = ctx.devices.read();
        if let Some(dev) = devices.get(idx) {
            if name.peek().is_empty() && !dev.name.is_empty() {
                name.set(dev.name.clone());
                username.set(
                    dev.credentials
                        .as_ref()
                        .map(|c| c.username.clone())
                        .unwrap_or_default(),
                );
                password.set(
                    dev.credentials
                        .as_ref()
                        .map(|c| c.password.clone())
                        .unwrap_or_default(),
                );
            }
        }
    }

    let mut open_sig = open;
    let mut devices = ctx.devices;

    rsx! {
        div {
            class: "dialog-overlay",
            onmousedown: move |_| {
                name.set(String::new());
                open_sig.set(false);
            },
            div {
                class: "dialog dialog--wide",
                onmousedown: |e| e.stop_propagation(),
                div { class: "dialog-header",
                    span { class: "dialog-title", {i18n::t(locale, "edit_device_title")} }
                }
                div { class: "dialog-body",
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "add_device_name")} }
                        input {
                            class: "form-input",
                            r#type: "text",
                            value: "{name}",
                            oninput: move |e| name.set(e.value()),
                        }
                    }
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "cred_username")} }
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: i18n::t(locale, "edit_device_cred_hint"),
                            value: "{username}",
                            oninput: move |e| username.set(e.value()),
                        }
                    }
                    div { class: "form-field",
                        label { class: "form-label", {i18n::t(locale, "cred_password")} }
                        PasswordField {
                            value: password,
                            placeholder: i18n::t(locale, "edit_device_cred_hint"),
                        }
                        p { class: "form-hint", {i18n::t(locale, "edit_device_cred_fallback")} }
                    }
                }
                div { class: "dialog-footer",
                    button {
                        class: "btn btn-md btn-ghost",
                        onclick: move |_| {
                            name.set(String::new());
                            open_sig.set(false);
                        },
                        {i18n::t(locale, "btn_cancel")}
                    }
                    button {
                        class: "btn btn-md btn-primary",
                        onclick: move |_| {
                            if let Some(d) = devices.write().get_mut(idx) {
                                d.name = name.peek().trim().to_string();
                                let u = username.peek().clone();
                                let p = password.peek().clone();
                                d.credentials = if u.is_empty() && p.is_empty() {
                                    None
                                } else {
                                    Some(Credentials { username: u, password: p })
                                };
                            }
                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, "edit_device_saved"));
                            crate::components::device_list::reverify_device(ctx, devices, idx);
                            name.set(String::new());
                            open_sig.set(false);
                        },
                        {i18n::t(locale, "btn_save")}
                    }
                }
            }
        }
    }
}
