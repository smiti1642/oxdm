#![allow(non_snake_case)]
use dioxus::prelude::*;

mod api;
mod components;
mod device_ops;
mod i18n;
mod persist;
mod state;
#[cfg(test)]
mod tests;
pub(crate) mod util;
mod video;
mod views;

use components::{ConfirmDialogModal, DeviceList, DevicePanel, ToastContainer, Topbar};
use state::{Ctx, GlobalKey, SettingsTab, View};
use views::MainContent;

/// CSS is embedded directly in the binary so the release ships as a
/// single executable — no sibling `assets/` directory needed at runtime.
const MAIN_CSS: &str = include_str!("../assets/main.css");

/// Set up tracing with a stderr layer (env-filter respected) plus a
/// daily-rolling file appender at `~/.oxdm/logs/oxdm.log`. Returns the
/// file appender's `WorkerGuard` — the caller MUST keep it alive (bind
/// it for the duration of `main`) so the background flush thread isn't
/// dropped before the program exits.
fn init_logging() -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let env_filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| "oxdm=info".parse().unwrap());
    let stderr_layer = fmt::layer().with_target(false).with_filter(env_filter());

    // File appender: daily rolling under ~/.oxdm/logs/. Failure here
    // (no home dir, no write perms) is non-fatal — we still get stderr
    // output and the user sees the warning on first launch.
    let log_dir = dirs::home_dir().map(|h| h.join(".oxdm").join("logs"));
    let (file_layer, guard) = match log_dir {
        Some(ref dir) => match std::fs::create_dir_all(dir) {
            Ok(()) => {
                let file_appender = tracing_appender::rolling::daily(dir, "oxdm.log");
                let (nb, guard) = tracing_appender::non_blocking(file_appender);
                let layer = fmt::layer()
                    .with_writer(nb)
                    .with_ansi(false)
                    .with_target(false)
                    .with_filter(env_filter());
                (Some(layer), Some(guard))
            }
            Err(e) => {
                eprintln!("Could not create log dir {}: {e}", dir.display());
                (None, None)
            }
        },
        None => (None, None),
    };

    tracing_subscriber::registry()
        .with(stderr_layer)
        .with(file_layer)
        .init();

    guard
}

/// Path to the directory holding rotated log files.
pub fn log_dir() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".oxdm").join("logs"))
}

fn main() {
    let _log_guard = init_logging();

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
    // Load persisted settings (single keychain read for all credentials)
    let cfg = use_hook(persist::load_config);
    let (global_creds, creds_map) = use_hook(|| persist::load_all_credentials(&cfg));
    let saved_devices = use_hook(|| persist::load_devices(&creds_map));

    // Install the default video backend (MJPEG snapshot loop). Runs inside
    // the dioxus tokio runtime so spawning the listener task is safe here.
    // Failure to bind only logs — the rest of the app keeps working without
    // live video.
    use_hook(|| match video::mjpeg::MjpegBackend::start() {
        Ok(b) => video::install(std::sync::Arc::new(b)),
        Err(e) => tracing::error!(error = %e, "failed to start MJPEG backend"),
    });

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
        global_credentials: use_signal(|| global_creds),
        selected_profile: use_signal(|| None),
        keyboard_action: use_signal(|| None),
    };
    use_context_provider(|| ctx);

    // Auto-save when theme or locale change
    use_effect(move || {
        let theme = *ctx.theme.read();
        let locale = *ctx.locale.read();
        persist::save_config(theme, locale);
    });

    // Re-verify auth when credentials change
    use_effect(move || {
        let _creds = ctx.global_credentials.read();
        device_ops::reverify_auth(ctx, ctx.devices);
    });

    // Auto-save credentials + devices when either changes (single keychain write)
    use_effect(move || {
        let creds = ctx.global_credentials.read().clone();
        let devices = ctx.devices.read();
        persist::save_credentials_and_devices(&creds, &devices);
    });

    let theme_class = ctx.theme.read().css_class();

    rsx! {
        document::Style { {MAIN_CSS} }
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
            div {
                class: theme_class,
                tabindex: "-1",
                autofocus: true,
                // App-level shortcuts. Esc is handled per-modal because each
                // dialog has its own close semantics; the keys here are the
                // ones that should always work no matter what's focused.
                onkeydown: move |evt| {
                    let key = evt.key();
                    let mods = evt.modifiers();
                    use dioxus::html::input_data::keyboard_types::{Key, Modifiers};
                    let ctrl_or_meta =
                        mods.contains(Modifiers::CONTROL) || mods.contains(Modifiers::META);
                    let mut action = ctx.keyboard_action;
                    match key {
                        Key::Character(ref s) if ctrl_or_meta && s.eq_ignore_ascii_case("f") => {
                            action.set(Some(GlobalKey::FocusSearch));
                            evt.prevent_default();
                        }
                        Key::F5 => {
                            action.set(Some(GlobalKey::Scan));
                            evt.prevent_default();
                        }
                        Key::ArrowUp => {
                            action.set(Some(GlobalKey::NavUp));
                            evt.prevent_default();
                        }
                        Key::ArrowDown => {
                            action.set(Some(GlobalKey::NavDown));
                            evt.prevent_default();
                        }
                        _ => {}
                    }
                },
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
