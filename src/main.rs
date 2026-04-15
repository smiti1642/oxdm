#![allow(non_snake_case)]
use dioxus::prelude::*;

mod api;
mod components;
mod device_ops;
mod discovery;
mod i18n;
mod persist;
mod state;
#[cfg(test)]
mod tests;
pub(crate) mod util;
mod views;

use components::{ConfirmDialogModal, DeviceList, DevicePanel, ToastContainer, Topbar};
use state::{Ctx, SettingsTab, View};
use views::MainContent;

const MAIN_CSS: Asset = asset!("/assets/main.css");

fn main() {
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
                .with_disable_context_menu(true)
                .with_menu(None),
        )
        .launch(App);
}

fn App() -> Element {
    // Load persisted settings
    let cfg = use_hook(persist::load_config);
    let saved_devices = use_hook(persist::load_devices);

    let ctx = Ctx {
        devices: use_signal(|| saved_devices),
        selected: use_signal(|| None),
        view: use_signal(|| View::Welcome),
        settings_tab: use_signal(|| SettingsTab::Identification),
        scanning: use_signal(|| false),
        theme: use_signal(|| persist::theme_from_str(&cfg.theme)),
        locale: use_signal(|| persist::locale_from_str(&cfg.locale)),
        toasts: use_signal(Vec::new),
        next_toast_id: use_signal(|| 0),
        dialog: use_signal(|| None),
        global_credentials: use_signal(|| persist::load_global_credentials(&cfg)),
        selected_profile: use_signal(|| None),
    };
    use_context_provider(|| ctx);

    // Auto-save when theme, locale, or credentials change
    use_effect(move || {
        let theme = *ctx.theme.read();
        let locale = *ctx.locale.read();
        let creds = ctx.global_credentials.read().clone();
        persist::save_config(theme, locale, &creds);
    });

    // Re-verify auth when credentials change
    use_effect(move || {
        let _creds = ctx.global_credentials.read();
        device_ops::reverify_auth(ctx, ctx.devices);
    });

    // Auto-save when manual devices change
    use_effect(move || {
        let devices = ctx.devices.read();
        persist::save_devices(&devices);
    });

    let theme_class = ctx.theme.read().css_class();

    rsx! {
        document::Stylesheet { href: MAIN_CSS }
        ErrorBoundary {
            handle_error: |errors: ErrorContext| {
                rsx! {
                    div { class: "error-boundary",
                        h2 { "Something went wrong" }
                        for error in errors.error() {
                            p { class: "error-boundary-detail", "{error}" }
                        }
                        p { "Please restart the application." }
                    }
                }
            },
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
}
