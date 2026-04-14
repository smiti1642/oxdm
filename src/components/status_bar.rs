#![allow(non_snake_case)]
use crate::i18n;
use crate::state::Ctx;
use dioxus::prelude::*;

#[component]
pub fn StatusBar() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let devices = ctx.devices.read();
    let is_scanning = *ctx.scanning.read();

    let device_count = devices.len();
    let online_count = devices.iter().filter(|d| d.online).count();
    let dev_label = if device_count == 1 {
        i18n::t(locale, "status_device")
    } else {
        i18n::t(locale, "status_devices")
    };

    let online_label = i18n::t(locale, "status_online");

    rsx! {
        footer { class: "status-bar",
            div { class: "status-bar-left",
                if is_scanning {
                    span { class: "status-bar-scanning",
                        span { class: "status-bar-spinner" }
                        {i18n::t(locale, "status_scanning")}
                    }
                } else {
                    span { class: "status-bar-text",
                        "{device_count} {dev_label} \u{00B7} {online_count} {online_label}"
                    }
                }
            }
            div { class: "status-bar-right",
                span { class: "status-bar-text", {i18n::t(locale, "status_ws_discovery")} }
            }
        }
    }
}
