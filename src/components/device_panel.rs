#![allow(non_snake_case)]
use crate::state::{Ctx, View};
use dioxus::prelude::*;

#[component]
pub fn DevicePanel() -> Element {
    let ctx = use_context::<Ctx>();
    let devices = ctx.devices.read();
    let selected = *ctx.selected.read();

    let Some(idx) = selected else {
        return rsx! {
            div { class: "device-panel device-panel--empty",
                span { class: "panel-empty-hint", "← Select a device" }
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
                div { class: "panel-device-icon", "📷" }
                div { class: "panel-device-name", "{dev_name}" }
            }

            div { class: "panel-section",
                div { class: "panel-section-title", "General" }
                NavLink { view: View::DeviceInfo,      icon: "ℹ",  label: "Identification" }
                NavLink { view: View::TimeSettings,    icon: "🕐", label: "Time settings" }
                NavLink { view: View::NetworkSettings, icon: "🌐", label: "Network settings" }
                NavLink { view: View::UserManagement,  icon: "👤", label: "User management" }
                NavLink { view: View::Maintenance,     icon: "🔧", label: "Maintenance" }
                NavLink { view: View::Events,          icon: "🔔", label: "Events" }
            }

            div { class: "panel-section",
                div { class: "panel-section-title", "NVT" }
                div { class: "panel-thumbnail", "▶  Live preview" }
                NavLink { view: View::LiveVideo,       icon: "📹", label: "Live video" }
                NavLink { view: View::ImagingSettings, icon: "🎨", label: "Imaging settings" }
                NavLink { view: View::PtzControl,      icon: "🎯", label: "PTZ control" }
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
