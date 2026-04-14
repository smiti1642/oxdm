#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx, SettingsTab, View};
use crate::views::settings::{IdentificationTab, MaintenanceTab, NetworkTab, TimeTab, UsersTab};
use dioxus::prelude::*;

#[component]
pub fn MainContent() -> Element {
    let ctx = use_context::<Ctx>();
    let view = *ctx.view.read();
    let locale = *ctx.locale.read();

    // Derive addr and effective credentials as reactive memos
    let addr = use_memo(move || {
        let devices = ctx.devices.read();
        let selected = *ctx.selected.read();
        selected
            .and_then(|i| devices.get(i))
            .map(|d| d.addr.clone())
            .unwrap_or_default()
    });

    let creds = use_memo(move || {
        let devices = ctx.devices.read();
        let selected = *ctx.selected.read();
        selected
            .and_then(|i| devices.get(i))
            .map(|d| ctx.credentials_for(d))
            .unwrap_or_else(|| ctx.global_credentials.read().clone())
    });

    rsx! {
        main { class: "main-content",
            match view {
                View::Welcome         => rsx! { WelcomeView {} },
                View::DeviceSettings  => rsx! { DeviceSettingsView { addr, creds } },
                View::LiveVideo       => rsx! { PlaceholderView { title: i18n::t(locale, "nav_live_video"),  icon: "video" } },
                View::ImagingSettings => rsx! { PlaceholderView { title: i18n::t(locale, "nav_imaging"),     icon: "sliders" } },
                View::PtzControl      => rsx! { PlaceholderView { title: i18n::t(locale, "nav_ptz"),        icon: "crosshair" } },
                View::Events          => rsx! { PlaceholderView { title: i18n::t(locale, "nav_events"),     icon: "bell" } },
            }
        }
    }
}

#[component]
fn WelcomeView() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    rsx! {
        div { class: "welcome",
            div { class: "welcome-icon",
                Icon { name: "hexagon", size: 52 }
            }
            h1  { class: "welcome-title", {i18n::t(locale, "app_name")} }
            p   { class: "welcome-sub",   {i18n::t(locale, "app_subtitle")} }
            p   { class: "welcome-hint",  {i18n::t(locale, "welcome_hint")} }
        }
    }
}

// ── Device Settings (tabbed view) ───────────────────────────────────────────

const SETTINGS_TABS: &[(SettingsTab, &str, &str)] = &[
    (SettingsTab::Identification, "info", "tab_identification"),
    (SettingsTab::Network, "globe", "tab_network"),
    (SettingsTab::Time, "clock", "tab_time"),
    (SettingsTab::Users, "users", "tab_users"),
    (SettingsTab::Maintenance, "wrench", "tab_maintenance"),
];

#[component]
fn DeviceSettingsView(addr: Memo<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let active = *ctx.settings_tab.read();
    let mut tab_sig = ctx.settings_tab;

    rsx! {
        div { class: "settings-view",
            div { class: "tab-bar",
                for &(tab, icon, key) in SETTINGS_TABS {
                    button {
                        class: if active == tab { "tab tab--active" } else { "tab" },
                        onclick: move |_| tab_sig.set(tab),
                        span { class: "tab-icon", Icon { name: icon, size: 14 } }
                        {i18n::t(locale, key)}
                    }
                }
            }

            div { class: "tab-content",
                match active {
                    SettingsTab::Identification => rsx! { IdentificationTab { addr, creds } },
                    SettingsTab::Network        => rsx! { NetworkTab { addr, creds } },
                    SettingsTab::Time           => rsx! { TimeTab { addr, creds } },
                    SettingsTab::Users          => rsx! { UsersTab { addr, creds } },
                    SettingsTab::Maintenance    => rsx! { MaintenanceTab { addr, creds } },
                }
            }
        }
    }
}

// ── Generic placeholder for non-settings views ─────────────────────────────

#[component]
fn PlaceholderView(title: &'static str, icon: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    rsx! {
        div { class: "placeholder-view",
            div { class: "content-header",
                Icon { name: icon, size: 20 }
                span { class: "content-title", "{title}" }
            }
            p { class: "placeholder-hint", {i18n::t(locale, "coming_soon")} }
        }
    }
}
