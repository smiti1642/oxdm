#![allow(non_snake_case)]
use crate::i18n;
use crate::state::{Ctx, SettingsTab, View};
use dioxus::prelude::*;

#[component]
pub fn MainContent() -> Element {
    let ctx = use_context::<Ctx>();
    let view = *ctx.view.read();
    let locale = *ctx.locale.read();
    let devices = ctx.devices.read();
    let selected = *ctx.selected.read();

    let addr = selected
        .and_then(|i| devices.get(i))
        .map(|d| d.addr.clone())
        .unwrap_or_default();

    rsx! {
        main { class: "main-content",
            match view {
                View::Welcome         => rsx! { WelcomeView {} },
                View::DeviceSettings  => rsx! { DeviceSettingsView { addr } },
                View::LiveVideo       => rsx! { PlaceholderView { title: i18n::t(locale, "nav_live_video"),  addr, icon: "\u{1F4F9}" } },
                View::ImagingSettings => rsx! { PlaceholderView { title: i18n::t(locale, "nav_imaging"),     addr, icon: "\u{1F3A8}" } },
                View::PtzControl      => rsx! { PlaceholderView { title: i18n::t(locale, "nav_ptz"),        addr, icon: "\u{1F3AF}" } },
                View::Events          => rsx! { PlaceholderView { title: i18n::t(locale, "nav_events"),     addr, icon: "\u{1F514}" } },
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
            div { class: "welcome-icon", "\u{2B21}" }
            h1  { class: "welcome-title", {i18n::t(locale, "app_name")} }
            p   { class: "welcome-sub",   {i18n::t(locale, "app_subtitle")} }
            p   { class: "welcome-hint",  {i18n::t(locale, "welcome_hint")} }
        }
    }
}

// ── Device Settings (tabbed view) ───────────────────────────────────────────

const SETTINGS_TABS: &[(SettingsTab, &str, &str)] = &[
    (
        SettingsTab::Identification,
        "\u{2139}",
        "tab_identification",
    ),
    (SettingsTab::Network, "\u{1F310}", "tab_network"),
    (SettingsTab::Time, "\u{1F550}", "tab_time"),
    (SettingsTab::Users, "\u{1F464}", "tab_users"),
    (SettingsTab::Maintenance, "\u{1F527}", "tab_maintenance"),
];

#[component]
fn DeviceSettingsView(addr: String) -> Element {
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
                        span { class: "tab-icon", "{icon}" }
                        {i18n::t(locale, key)}
                    }
                }
            }

            div { class: "tab-content",
                match active {
                    SettingsTab::Identification => rsx! { SettingsPlaceholder { title: i18n::t(locale, "tab_identification"), addr } },
                    SettingsTab::Network        => rsx! { SettingsPlaceholder { title: i18n::t(locale, "tab_network"),        addr } },
                    SettingsTab::Time           => rsx! { SettingsPlaceholder { title: i18n::t(locale, "tab_time"),           addr } },
                    SettingsTab::Users          => rsx! { SettingsPlaceholder { title: i18n::t(locale, "tab_users"),          addr } },
                    SettingsTab::Maintenance    => rsx! { SettingsPlaceholder { title: i18n::t(locale, "tab_maintenance"),    addr } },
                }
            }
        }
    }
}

#[component]
fn SettingsPlaceholder(title: &'static str, addr: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    rsx! {
        div { class: "settings-placeholder",
            if !addr.is_empty() {
                span { class: "placeholder-addr", "{addr}" }
            }
            p { class: "placeholder-hint", {i18n::t(locale, "coming_soon")} }
        }
    }
}

// ── Generic placeholder for non-settings views ─────────────────────────────

#[component]
fn PlaceholderView(title: &'static str, addr: String, icon: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    rsx! {
        div { class: "placeholder-view",
            div { class: "content-header",
                span { style: "font-size:20px", "{icon}" }
                span { class: "content-title", "{title}" }
            }
            if !addr.is_empty() {
                span { class: "placeholder-addr", "{addr}" }
            }
            p { class: "placeholder-hint", {i18n::t(locale, "coming_soon")} }
        }
    }
}
