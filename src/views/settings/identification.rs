#![allow(non_snake_case)]
use crate::components::{Icon, PropRow};
use crate::state::{Credentials, ToastLevel};
use crate::util::urldecode;
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;

const NAME_PREFIX: &str = "onvif://www.onvif.org/name/";
const LOCATION_PREFIX: &str = "onvif://www.onvif.org/location/";

#[component]
pub fn IdentificationTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut devices = ctx.devices;

    let mut info = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move {
            let (u, p) = creds.as_options();
            let info = api::get_device_info(&addr, u, p).await;
            let scopes = api::get_scopes(&addr, u, p).await.unwrap_or_default();
            info.map(|i| (i, scopes))
        }
    });

    // Editable buffers seeded from the fetched scopes on first read.
    let name_in = use_signal(String::new);
    let location_in = use_signal(String::new);
    let initialized = use_signal(|| false);

    rsx! {
        match &*info.read_unchecked() {
            None => rsx! {
                div { class: "tab-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(e)) => rsx! {
                div { class: "tab-error", "{e}" }
            },
            Some(Ok((dev, scopes))) => {
                if !*initialized.peek() {
                    let cur_name = scopes
                        .iter()
                        .find_map(|s| s.strip_prefix(NAME_PREFIX))
                        .map(urldecode)
                        .unwrap_or_default();
                    let cur_loc = scopes
                        .iter()
                        .find_map(|s| s.strip_prefix(LOCATION_PREFIX))
                        .map(urldecode)
                        .unwrap_or_default();
                    name_in.clone().set(cur_name);
                    location_in.clone().set(cur_loc);
                    initialized.clone().set(true);
                }

                // Note: SetScopes only accepts *configurable* scopes;
                // sending a fixed one back triggers
                // "overwrites fixed device scope setting". GetScopes
                // doesn't tag fixed vs configurable, so we play it safe
                // and ship only `name` and `location` — the two scopes
                // every spec-compliant camera marks configurable. Other
                // scopes still render below as read-only context.

                rsx! {
                    table { class: "prop-table",
                        PropRow { label: i18n::t(locale, "prop_manufacturer"), value: dev.manufacturer.clone() }
                        PropRow { label: i18n::t(locale, "prop_model"),        value: dev.model.clone() }
                        PropRow { label: i18n::t(locale, "prop_firmware"),     value: dev.firmware_version.clone() }
                        PropRow { label: i18n::t(locale, "prop_serial"),       value: dev.serial_number.clone() }
                        PropRow { label: i18n::t(locale, "prop_hardware_id"),  value: dev.hardware_id.clone() }
                    }

                    div { class: "prop-section-header", {i18n::t(locale, "id_editable_scopes")} }
                    div { class: "id-edit-form",
                        div { class: "id-edit-row",
                            label { class: "id-edit-label", {i18n::t(locale, "id_name")} }
                            input {
                                class: "id-edit-input",
                                r#type: "text",
                                value: "{*name_in.read()}",
                                oninput: move |e| name_in.clone().set(e.value()),
                            }
                        }
                        div { class: "id-edit-row",
                            label { class: "id-edit-label", {i18n::t(locale, "id_location")} }
                            input {
                                class: "id-edit-input",
                                r#type: "text",
                                value: "{*location_in.read()}",
                                oninput: move |e| location_in.clone().set(e.value()),
                            }
                        }
                        div { class: "id-edit-actions",
                            button {
                                class: "btn btn-md btn-primary",
                                onclick: move |_| {
                                    let addr_s = addr.read().clone();
                                    let creds_s = creds.read().clone();
                                    let new_name = name_in.peek().clone();
                                    let new_loc = location_in.peek().clone();
                                    let mut new_scopes: Vec<String> = Vec::new();
                                    if !new_name.is_empty() {
                                        new_scopes.push(format!("{NAME_PREFIX}{new_name}"));
                                    }
                                    if !new_loc.is_empty() {
                                        new_scopes.push(format!("{LOCATION_PREFIX}{new_loc}"));
                                    }
                                    spawn(async move {
                                        let (u, p) = creds_s.as_options();
                                        match api::set_scopes(&addr_s, u, p, &new_scopes).await {
                                            Ok(()) => {
                                                // Reflect new name in the
                                                // device list immediately
                                                // so the sidebar updates
                                                // before the next scan.
                                                if !new_name.is_empty() {
                                                    let mut all = devices.write();
                                                    if let Some(d) = all.iter_mut().find(|d| d.addr == addr_s) {
                                                        d.name = new_name.clone();
                                                    }
                                                }
                                                ctx.push_toast(
                                                    ToastLevel::Success,
                                                    i18n::t(locale, "id_scopes_saved"),
                                                );
                                                info.restart();
                                            }
                                            Err(e) => {
                                                ctx.push_toast(ToastLevel::Error, e);
                                            }
                                        }
                                    });
                                },
                                Icon { name: "check", size: 14 }
                                " "
                                {i18n::t(locale, "btn_save")}
                            }
                        }
                    }

                    if scopes.iter().any(|s| s.starts_with("onvif://www.onvif.org/")
                        && !s.starts_with(NAME_PREFIX) && !s.starts_with(LOCATION_PREFIX)) {
                        div { class: "prop-section-header", {i18n::t(locale, "prop_scopes")} }
                        table { class: "prop-table",
                            for scope in scopes.iter().filter(|s| {
                                !s.starts_with(NAME_PREFIX) && !s.starts_with(LOCATION_PREFIX)
                            }) {
                                PropRow { label: scope_key(scope), value: scope_value(scope) }
                            }
                        }
                    }
                }
            },
        }
    }
}

pub fn scope_key(scope: &str) -> String {
    scope
        .strip_prefix("onvif://www.onvif.org/")
        .and_then(|s| s.split('/').next())
        .unwrap_or("scope")
        .to_string()
}

pub fn scope_value(scope: &str) -> String {
    scope
        .strip_prefix("onvif://www.onvif.org/")
        .and_then(|s| s.split('/').nth(1))
        .unwrap_or(scope)
        .to_string()
}
