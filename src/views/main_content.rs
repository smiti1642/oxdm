#![allow(non_snake_case)]
use crate::state::{Ctx, View};
use dioxus::prelude::*;

#[component]
pub fn MainContent() -> Element {
    let ctx = use_context::<Ctx>();
    let view = *ctx.view.read();
    let devices = ctx.devices.read();
    let selected = *ctx.selected.read();

    let addr = selected
        .and_then(|i| devices.get(i))
        .map(|d| d.addr.clone())
        .unwrap_or_default();

    rsx! {
        main { class: "main-content",
            match view {
                View::Welcome          => rsx! { WelcomeView {} },
                View::DeviceInfo       => rsx! { PlaceholderView { title: "Identification",    addr, icon: "ℹ" } },
                View::LiveVideo        => rsx! { PlaceholderView { title: "Live Video",         addr, icon: "📹" } },
                View::NetworkSettings  => rsx! { PlaceholderView { title: "Network Settings",  addr, icon: "🌐" } },
                View::TimeSettings     => rsx! { PlaceholderView { title: "Time Settings",     addr, icon: "🕐" } },
                View::UserManagement   => rsx! { PlaceholderView { title: "User Management",   addr, icon: "👤" } },
                View::ImagingSettings  => rsx! { PlaceholderView { title: "Imaging Settings",  addr, icon: "🎨" } },
                View::PtzControl       => rsx! { PlaceholderView { title: "PTZ Control",       addr, icon: "🎯" } },
                View::Events           => rsx! { PlaceholderView { title: "Events",            addr, icon: "🔔" } },
                View::Maintenance      => rsx! { PlaceholderView { title: "Maintenance",       addr, icon: "🔧" } },
            }
        }
    }
}

#[component]
fn WelcomeView() -> Element {
    rsx! {
        div { class: "welcome",
            div { class: "welcome-icon", "⬡" }
            h1  { class: "welcome-title", "OxDM" }
            p   { class: "welcome-sub",  "ONVIF Device Manager" }
            p   { class: "welcome-hint",
                "Select a device from the left panel,\nor click  ⟳ Scan  to discover devices on the network."
            }
        }
    }
}

#[component]
fn PlaceholderView(title: String, addr: String, icon: &'static str) -> Element {
    rsx! {
        div { class: "placeholder-view",
            div { class: "content-header",
                span { style: "font-size:20px", "{icon}" }
                span { class: "content-title", "{title}" }
            }
            if !addr.is_empty() {
                span { class: "placeholder-addr", "{addr}" }
            }
            p { class: "placeholder-hint", "🚧  Coming soon" }
        }
    }
}
