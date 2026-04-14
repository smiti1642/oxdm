#![allow(non_snake_case)]
use crate::i18n;
use crate::state::{Ctx, View};
use dioxus::prelude::*;

#[component]
pub fn DevicePanel() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let devices = ctx.devices.read();
    let selected = *ctx.selected.read();

    let Some(idx) = selected else {
        return rsx! {
            div { class: "device-panel device-panel--empty",
                span { class: "panel-empty-hint", {i18n::t(locale, "select_device")} }
            }
        };
    };

    let Some(dev) = devices.get(idx) else {
        return rsx! { div { class: "device-panel" } };
    };

    let dev_name = dev.name.clone();

    rsx! {
        div { class: "device-panel",

            div { class: "panel-header",
                div { class: "panel-device-icon", "\u{1F4F7}" }
                div { class: "panel-device-name", "{dev_name}" }
            }

            div { class: "panel-section",
                div { class: "panel-section-title", {i18n::t(locale, "section_general")} }
                NavLink { view: View::DeviceSettings, icon: "\u{2699}", label: i18n::t(locale, "nav_settings") }
                NavLink { view: View::Events,         icon: "\u{1F514}", label: i18n::t(locale, "nav_events") }
            }

            div { class: "panel-section",
                div { class: "panel-section-title", {i18n::t(locale, "section_nvt")} }
                div { class: "panel-thumbnail", {i18n::t(locale, "live_preview")} }
                NavLink { view: View::LiveVideo,       icon: "\u{1F4F9}", label: i18n::t(locale, "nav_live_video") }
                NavLink { view: View::ImagingSettings, icon: "\u{1F3A8}", label: i18n::t(locale, "nav_imaging") }
                NavLink { view: View::PtzControl,      icon: "\u{1F3AF}", label: i18n::t(locale, "nav_ptz") }
            }
        }
    }
}

#[component]
fn NavLink(view: View, icon: &'static str, label: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let mut view_sig = ctx.view;
    let is_active = *ctx.view.read() == view;
    let cls = if is_active {
        "nav-link nav-link--active"
    } else {
        "nav-link"
    };

    rsx! {
        button {
            class: cls,
            onclick: move |_| view_sig.set(view),
            span { class: "nav-link-icon", "{icon}" }
            "{label}"
        }
    }
}
