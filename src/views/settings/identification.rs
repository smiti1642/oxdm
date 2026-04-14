#![allow(non_snake_case)]
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;

#[component]
pub fn IdentificationTab(addr: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let creds = ctx.global_credentials.read().clone();

    let info = use_resource(move || {
        let addr = addr.clone();
        let u = creds.username.clone();
        let p = creds.password.clone();
        async move {
            let (user, pass) = if u.is_empty() {
                (None, None)
            } else {
                (Some(u.as_str()), Some(p.as_str()))
            };
            let info = api::get_device_info(&addr, user, pass).await;
            let scopes = api::get_scopes(&addr, user, pass).await.unwrap_or_default();
            info.map(|i| (i, scopes))
        }
    });

    rsx! {
        match &*info.read_unchecked() {
            None => rsx! {
                div { class: "tab-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(e)) => rsx! {
                div { class: "tab-error", "{e}" }
            },
            Some(Ok((dev, scopes))) => rsx! {
                table { class: "prop-table",
                    PropRow { label: i18n::t(locale, "prop_manufacturer"), value: dev.manufacturer.clone() }
                    PropRow { label: i18n::t(locale, "prop_model"),        value: dev.model.clone() }
                    PropRow { label: i18n::t(locale, "prop_firmware"),     value: dev.firmware_version.clone() }
                    PropRow { label: i18n::t(locale, "prop_serial"),       value: dev.serial_number.clone() }
                    PropRow { label: i18n::t(locale, "prop_hardware_id"),  value: dev.hardware_id.clone() }
                }

                if !scopes.is_empty() {
                    div { class: "prop-section-header", {i18n::t(locale, "prop_scopes")} }
                    table { class: "prop-table",
                        for scope in scopes {
                            PropRow { label: scope_key(scope), value: scope_value(scope) }
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn PropRow(label: String, value: String) -> Element {
    rsx! {
        tr {
            td { class: "prop-label", "{label}" }
            td { class: "prop-value", "{value}" }
        }
    }
}

fn scope_key(scope: &str) -> String {
    // onvif://www.onvif.org/type/video_encoder → "type"
    scope
        .strip_prefix("onvif://www.onvif.org/")
        .and_then(|s| s.split('/').next())
        .unwrap_or("scope")
        .to_string()
}

fn scope_value(scope: &str) -> String {
    // onvif://www.onvif.org/type/video_encoder → "video_encoder"
    scope
        .strip_prefix("onvif://www.onvif.org/")
        .and_then(|s| s.split('/').nth(1))
        .unwrap_or(scope)
        .to_string()
}
