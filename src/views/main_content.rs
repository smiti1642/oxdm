#![allow(non_snake_case)]
use crate::components::{Icon, LensBrand};
use crate::i18n;
use crate::state::{Credentials, Ctx, SettingsTab, View};
use crate::views::events::EventsView;
use crate::views::imaging::ImagingView;
use crate::views::io_control::IoControlView;
use crate::views::live_video::LiveVideoView;
use crate::views::osd::OsdView;
use crate::views::ptz::PtzControlView;
use crate::views::settings::{
    HealthTab, IdentificationTab, MaintenanceTab, NetworkTab, TimeTab, UsersTab,
};
use dioxus::prelude::*;

#[component]
pub fn MainContent() -> Element {
    let ctx = use_context::<Ctx>();
    let view = *ctx.view.read();

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

    // Use addr as a render key so views with internal `use_resource`
    // (LiveVideoView, ImagingView, PtzControlView) remount cleanly on
    // device switch — otherwise their previous-device fetch result stays
    // visible until the new fetch lands, which feels like a stale UI bug.
    let addr_key = addr.read().clone();

    rsx! {
        main { class: "main-content",
            match view {
                View::Welcome         => rsx! { WelcomeView {} },
                View::DeviceSettings  => rsx! { DeviceSettingsView { key: "{addr_key}", addr, creds } },
                View::LiveVideo       => rsx! { LiveVideoView      { key: "{addr_key}", addr, creds } },
                View::ImagingSettings => rsx! { ImagingView        { key: "{addr_key}", addr, creds } },
                View::PtzControl      => rsx! { PtzControlView     { key: "{addr_key}", addr, creds } },
                View::Events          => rsx! { EventsView         { key: "{addr_key}", addr, creds } },
                View::Osd             => rsx! { OsdView            { key: "{addr_key}", addr, creds } },
                View::IoControl       => rsx! { IoControlView      { key: "{addr_key}", addr, creds } },
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
                LensBrand { size: 128 }
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
    (SettingsTab::Health, "activity", "tab_health"),
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
                    SettingsTab::Health         => rsx! { HealthTab { addr, creds } },
                }
            }
        }
    }
}
