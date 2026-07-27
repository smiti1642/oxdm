#![allow(non_snake_case)]
use crate::components::{DialogOverlay, PasswordField};
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
    // Which device the three fields above were last filled from. They are
    // component-level signals that outlive an open/close cycle, so without
    // this the dialog can show — and then save — the *previous* device's
    // credentials. Cleared on close, so re-opening always re-reads and a
    // cancelled edit does not survive.
    let mut loaded_for = use_signal(|| None::<usize>);

    let is_open = *open.read();
    let idx = *device_index.read();

    if !is_open || idx.is_none() {
        return rsx! {};
    }
    let idx = idx.unwrap();

    // Fill the fields from the device this dialog was opened for.
    //
    // Keyed on the index rather than on "the name field is still blank": the
    // save below trims, so a name of only spaces becomes empty, and the old
    // `!dev.name.is_empty()` guard then skipped the load entirely — leaving
    // whatever the last device put in the username/password fields, which the
    // next save wrote onto this device. An empty pair saves as `None`, and a
    // manual device with no credentials is not persisted at all
    // (`persist::build_creds_map` only stores devices whose `credentials` is
    // `Some`), so the loss only became visible on the next launch.
    if *loaded_for.peek() != Some(idx) {
        let devices = ctx.devices.read();
        if let Some(dev) = devices.get(idx) {
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
            loaded_for.set(Some(idx));
        }
    }

    let mut open_sig = open;
    let mut devices = ctx.devices;

    rsx! {
        DialogOverlay {
            on_close: move |_| {
                clear(name, username, password, loaded_for);
                open_sig.set(false);
            },
            inner_class: "dialog dialog--wide".to_string(),
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
                            clear(name, username, password, loaded_for);
                            open_sig.set(false);
                        },
                        {i18n::t(locale, "btn_cancel")}
                    }
                    button {
                        class: "btn btn-md btn-primary",
                        onclick: move |_| {
                            let mut addr_to_invalidate: Option<String> = None;
                            if let Some(d) = devices.write().get_mut(idx) {
                                d.name = name.peek().trim().to_string();
                                let u = username.peek().clone();
                                let p = password.peek().clone();
                                d.credentials = if u.is_empty() && p.is_empty() {
                                    None
                                } else {
                                    Some(Credentials { username: u, password: p })
                                };
                                // Per-device creds may have changed —
                                // capture the addr so we can drop any
                                // sessions cached under the old creds
                                // after we release the devices lock.
                                addr_to_invalidate = Some(d.addr.clone());
                            }
                            if let Some(addr) = addr_to_invalidate {
                                crate::sessions::invalidate(&addr);
                            }
                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, "edit_device_saved"));
                            crate::device_ops::reverify_device(ctx, devices, idx);
                            clear(name, username, password, loaded_for);
                            open_sig.set(false);
                        },
                        {i18n::t(locale, "btn_save")}
                    }
                }
        }
    }
}

/// Blank every field the dialog carries between opens.
///
/// `loaded_for` is what makes the next open re-read the device, so clearing it
/// is the load-bearing part; the three strings are cleared with it so a closed
/// dialog is not sitting on a password.
fn clear(
    mut name: Signal<String>,
    mut username: Signal<String>,
    mut password: Signal<String>,
    mut loaded_for: Signal<Option<usize>>,
) {
    name.set(String::new());
    username.set(String::new());
    password.set(String::new());
    loaded_for.set(None);
}
