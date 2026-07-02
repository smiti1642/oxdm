#![allow(non_snake_case)]
//! Dialogs for curating HealthCheck groups and their credentials.
//!
//! - [`AddToGroupDialog`] — the right-click "Add to HealthCheck list" picker.
//! - [`GroupCredentialsDialog`] — group-level credentials.
//! - [`GroupDeviceCredentialsDialog`] — per-device-in-group override.
//!
//! Group credentials are stored in the keychain (never in `healthcheck.toml`);
//! writing them here just mutates `ctx.health_groups`, which the save effect in
//! `main.rs` persists.

use crate::components::{CredentialsFields, DialogOverlay};
use crate::i18n;
use crate::state::{new_group_id, Credentials, Ctx, HealthDeviceRef, HealthGroup, ToastLevel};
use dioxus::prelude::*;

/// Picker shown from the device-list context menu: add the target device to an
/// existing group or a newly-created one.
#[component]
pub fn AddToGroupDialog(open: Signal<bool>, device_index: Signal<Option<usize>>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut new_name = use_signal(String::new);
    let mut open_sig = open;

    // Add the target device (snapshotted at call time by index) to `gid`, or to
    // a freshly-created group when `gid` is None. Deduped by endpoint‖addr.
    let commit = use_callback(move |gid: Option<String>| {
        let Some(idx) = *device_index.peek() else {
            return;
        };
        let Some(d) = ctx.devices.peek().get(idx).cloned() else {
            return;
        };
        let dref = HealthDeviceRef {
            endpoint: d.endpoint.clone(),
            addr: d.addr.clone(),
            name: d.name.clone(),
        };
        // Snapshot the device's effective creds so it joins with its own
        // known-good credentials pinned (see Ctx::group_credentials_for).
        let snap = {
            let c = ctx.credentials_for(&d);
            (!c.username.is_empty()).then(|| (d.addr.clone(), c))
        };
        let present = |g: &HealthGroup| {
            g.devices
                .iter()
                .any(|r| (!d.endpoint.is_empty() && r.endpoint == d.endpoint) || r.addr == d.addr)
        };
        let mut already = false;
        {
            let mut hg = ctx.health_groups;
            let mut groups = hg.write();
            match gid {
                Some(id) => {
                    if let Some(g) = groups.iter_mut().find(|g| g.id == id) {
                        if present(g) {
                            already = true;
                        } else {
                            g.devices.push(dref);
                            if let Some((addr, creds)) = snap {
                                g.device_credentials.entry(addr).or_insert(creds);
                            }
                        }
                    }
                }
                None => {
                    let name = new_name.peek().trim().to_string();
                    if name.is_empty() {
                        return;
                    }
                    let id = new_group_id(&groups);
                    groups.push(HealthGroup {
                        id,
                        name,
                        devices: vec![dref],
                        device_credentials: snap.into_iter().collect(),
                        ..Default::default()
                    });
                }
            }
        }
        new_name.set(String::new());
        ctx.push_toast(
            if already {
                ToastLevel::Info
            } else {
                ToastLevel::Success
            },
            i18n::t(
                locale,
                if already {
                    "hgroups_already"
                } else {
                    "hgroups_added"
                },
            ),
        );
        open_sig.set(false);
    });

    if !*open.read() {
        return rsx! {};
    }

    let groups: Vec<(String, String)> = ctx
        .health_groups
        .read()
        .iter()
        .map(|g| (g.id.clone(), g.name.clone()))
        .collect();

    rsx! {
        DialogOverlay {
            on_close: move |_| {
                new_name.set(String::new());
                open_sig.set(false);
            },
            inner_class: "dialog".to_string(),
            div { class: "dialog-header",
                span { class: "dialog-title", {i18n::t(locale, "ctx_add_to_group")} }
            }
            div { class: "dialog-body",
                if groups.is_empty() {
                    p { class: "dialog-hint", {i18n::t(locale, "hgroups_empty")} }
                } else {
                    div { class: "hgroups-picker-list",
                        for (id , name) in groups {
                            button {
                                class: "btn btn-md btn-secondary hgroups-picker-item",
                                onclick: move |_| commit.call(Some(id.clone())),
                                "{name}"
                            }
                        }
                    }
                }
                div { class: "hgroups-new-row",
                    input {
                        class: "form-input form-input--flex",
                        r#type: "text",
                        placeholder: i18n::t(locale, "hgroups_new_group_placeholder"),
                        value: "{new_name}",
                        oninput: move |e| new_name.set(e.value()),
                    }
                    button {
                        class: "btn btn-md btn-primary",
                        onclick: move |_| commit.call(None),
                        {i18n::t(locale, "hgroups_create")}
                    }
                }
            }
            div { class: "dialog-footer",
                button {
                    class: "btn btn-md btn-ghost",
                    onclick: move |_| {
                        new_name.set(String::new());
                        open_sig.set(false);
                    },
                    {i18n::t(locale, "btn_cancel")}
                }
            }
        }
    }
}

/// Edit a group's group-level credentials. Render with `key: "{group_id}"` so it
/// remounts (and re-seeds its fields) when the selected group changes.
#[component]
pub fn GroupCredentialsDialog(open: Signal<bool>, group_id: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let current = ctx
        .health_groups
        .peek()
        .iter()
        .find(|g| g.id == group_id)
        .and_then(|g| g.credentials.clone())
        .unwrap_or_default();
    let username = use_signal(|| current.username.clone());
    let password = use_signal(|| current.password.clone());

    if !*open.read() {
        return rsx! {};
    }

    let mut open_sig = open;
    let gid = group_id.clone();

    rsx! {
        DialogOverlay {
            on_close: move |_| open_sig.set(false),
            inner_class: "dialog".to_string(),
            div { class: "dialog-header",
                span { class: "dialog-title", {i18n::t(locale, "hgroups_group_creds_title")} }
            }
            div { class: "dialog-body",
                p { class: "dialog-hint", {i18n::t(locale, "hgroups_creds_hint")} }
                CredentialsFields { username, password, locale }
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
                        let u = username.peek().clone();
                        let p = password.peek().clone();
                        let mut hg = ctx.health_groups;
                        if let Some(g) = hg.write().iter_mut().find(|g| g.id == gid) {
                            g.credentials = if u.is_empty() {
                                None
                            } else {
                                Some(Credentials { username: u, password: p })
                            };
                        }
                        ctx.push_toast(ToastLevel::Success, i18n::t(locale, "cred_saved"));
                        open_sig.set(false);
                    },
                    {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}

/// Edit a per-device-in-group credential override. Clearing the username removes
/// the override so the row falls back to the group/app tier. Render with
/// `key: "{group_id}:{addr}"` so it re-seeds per target.
#[component]
pub fn GroupDeviceCredentialsDialog(open: Signal<bool>, group_id: String, addr: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let current = ctx
        .health_groups
        .peek()
        .iter()
        .find(|g| g.id == group_id)
        .and_then(|g| g.device_credentials.get(&addr).cloned())
        .unwrap_or_default();
    let username = use_signal(|| current.username.clone());
    let password = use_signal(|| current.password.clone());

    if !*open.read() {
        return rsx! {};
    }

    let mut open_sig = open;
    let gid = group_id.clone();
    let daddr = addr.clone();

    rsx! {
        DialogOverlay {
            on_close: move |_| open_sig.set(false),
            inner_class: "dialog".to_string(),
            div { class: "dialog-header",
                span { class: "dialog-title", {i18n::t(locale, "hgroups_device_creds_title")} }
            }
            div { class: "dialog-body",
                p { class: "dialog-hint", {i18n::t(locale, "hgroups_creds_hint")} }
                CredentialsFields { username, password, locale }
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
                        let u = username.peek().clone();
                        let p = password.peek().clone();
                        let mut hg = ctx.health_groups;
                        if let Some(g) = hg.write().iter_mut().find(|g| g.id == gid) {
                            if u.is_empty() {
                                g.device_credentials.remove(&daddr);
                            } else {
                                g.device_credentials
                                    .insert(daddr.clone(), Credentials { username: u, password: p });
                            }
                        }
                        ctx.push_toast(ToastLevel::Success, i18n::t(locale, "cred_saved"));
                        open_sig.set(false);
                    },
                    {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}

/// Rename a group. Render with `key: "{group_id}"` so it re-seeds per target.
#[component]
pub fn RenameGroupDialog(open: Signal<bool>, group_id: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let current = ctx
        .health_groups
        .peek()
        .iter()
        .find(|g| g.id == group_id)
        .map(|g| g.name.clone())
        .unwrap_or_default();
    let mut name = use_signal(|| current.clone());

    if !*open.read() {
        return rsx! {};
    }

    let mut open_sig = open;
    let gid = group_id.clone();

    rsx! {
        DialogOverlay {
            on_close: move |_| open_sig.set(false),
            inner_class: "dialog".to_string(),
            div { class: "dialog-header",
                span { class: "dialog-title", {i18n::t(locale, "hgroups_rename_title")} }
            }
            div { class: "dialog-body",
                input {
                    class: "form-input",
                    r#type: "text",
                    placeholder: i18n::t(locale, "hgroups_new_group_placeholder"),
                    value: "{name}",
                    oninput: move |e| name.set(e.value()),
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
                        let n = name.peek().trim().to_string();
                        if !n.is_empty() {
                            let mut hg = ctx.health_groups;
                            let mut groups = hg.write();
                            if let Some(g) = groups.iter_mut().find(|g| g.id == gid) {
                                g.name = n;
                            }
                        }
                        open_sig.set(false);
                    },
                    {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}
