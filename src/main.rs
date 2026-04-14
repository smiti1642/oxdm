#![allow(non_snake_case)]
use dioxus::prelude::*;

mod api;
mod components;
mod discovery;
mod i18n;
mod state;
#[cfg(test)]
mod tests;
mod views;

use components::{ConfirmDialogModal, DeviceList, DevicePanel, ToastContainer, Topbar};
use state::{Credentials, Ctx, Locale, SettingsTab, Theme, View};
use views::MainContent;

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
    // RUST_LOG=oxdm=debug for verbose output; default is info
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "oxdm=info".parse().unwrap()),
        )
        .with_target(false)
        .init();

    tracing::info!("OxDM starting");

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(
                    dioxus::desktop::WindowBuilder::new()
                        .with_title("OxDM")
                        .with_inner_size(dioxus::desktop::LogicalSize::new(1280.0, 800.0))
                        .with_min_inner_size(dioxus::desktop::LogicalSize::new(900.0, 500.0)),
                )
                .with_disable_context_menu(true),
        )
        .launch(App);
}

fn App() -> Element {
    let ctx = Ctx {
        devices: use_signal(Vec::new),
        selected: use_signal(|| None),
        view: use_signal(|| View::Welcome),
        settings_tab: use_signal(|| SettingsTab::Identification),
        scanning: use_signal(|| false),
        theme: use_signal(|| Theme::Dark),
        locale: use_signal(|| Locale::En),
        toasts: use_signal(Vec::new),
        next_toast_id: use_signal(|| 0),
        dialog: use_signal(|| None),
        global_credentials: use_signal(Credentials::default),
    };
    use_context_provider(|| ctx);

    let theme_class = ctx.theme.read().css_class();

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        div { class: theme_class,
            Topbar {}
            div { class: "shell-body",
                DeviceList {}
                DevicePanel {}
                MainContent {}
            }
            ToastContainer {}
            ConfirmDialogModal {}
        }
    }
}
