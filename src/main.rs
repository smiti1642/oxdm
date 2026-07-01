#![allow(non_snake_case)]
use dioxus::prelude::*;

mod api;
mod components;
mod device_ops;
mod i18n;
mod persist;
mod sessions;
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

/// App icon — same master PNG `build.rs` uses to mint the embedded ICO,
/// re-decoded here at startup so `WindowBuilder::with_window_icon` has
/// something to feed to the OS title bar / taskbar / Alt-Tab switcher.
/// `dx` doesn't wire this up for us; without it, Windows + Wry falls back
/// to the Dioxus default.
const ICON_PNG: &[u8] = include_bytes!("../assets/icons/icon.png");

/// Decode the embedded PNG into a `tao::window::Icon`. Returns `None` if
/// the PNG is in an unexpected colour format — the rest of the app keeps
/// working with the default icon.
fn load_window_icon() -> Option<dioxus::desktop::tao::window::Icon> {
    let decoder = png::Decoder::new(std::io::Cursor::new(ICON_PNG));
    let mut reader = decoder.read_info().ok()?;
    let mut buf = vec![0; reader.output_buffer_size()?];
    let info = reader.next_frame(&mut buf).ok()?;
    let rgba = match info.color_type {
        png::ColorType::Rgba => buf,
        png::ColorType::Rgb => {
            let mut out = Vec::with_capacity(buf.len() / 3 * 4);
            for chunk in buf.chunks_exact(3) {
                out.extend_from_slice(chunk);
                out.push(0xFF);
            }
            out
        }
        _ => return None,
    };
    dioxus::desktop::tao::window::Icon::from_rgba(rgba, info.width, info.height).ok()
}

/// Set up tracing with a stderr layer (env-filter respected). When
/// `log_to_file` is true, also adds a daily-rolling file appender at
/// `~/.oxdm/logs/oxdm.log.*` and returns its `WorkerGuard` — the caller
/// MUST keep it alive (bind it for the duration of `main`) so the
/// background flush thread isn't dropped before the program exits.
///
/// Defaults to off because most users never look at logs but they do
/// notice ~/.oxdm growing on disk. The About dialog has the toggle and
/// it persists to config.toml; takes effect on next launch.
fn init_logging(log_to_file: bool) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};

    let env_filter =
        || EnvFilter::try_from_default_env().unwrap_or_else(|_| "oxdm=info".parse().unwrap());
    let stderr_layer = fmt::layer().with_target(false).with_filter(env_filter());

    let (file_layer, guard) = if log_to_file {
        let log_dir = dirs::home_dir().map(|h| h.join(".oxdm").join("logs"));
        match log_dir {
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
        }
    } else {
        (None, None)
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
    // Read just the log preference up-front so init_logging knows whether
    // to spin up the file appender. The full config is re-loaded inside
    // App() — this duplicate read is one tiny TOML parse, not worth a
    // hand-off mechanism.
    let log_to_file = persist::load_config().log_to_file;
    let _log_guard = init_logging(log_to_file);

    tracing::info!(log_to_file, "OxDM starting");

    dioxus::LaunchBuilder::desktop()
        .with_cfg(
            dioxus::desktop::Config::new()
                .with_window(
                    dioxus::desktop::WindowBuilder::new()
                        .with_title("OxDM")
                        .with_window_icon(load_window_icon())
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
    let saved_groups = use_hook(|| persist::load_health_groups(&creds_map));

    // Install both video backends. MJPEG is the always-on default;
    // go2rtc is optional and lazy — its `new()` only locates the binary,
    // the subprocess spawns on first stream. Both run inside the dioxus
    // tokio runtime so spawning is safe here. Failure to bind MJPEG only
    // logs; the rest of the app keeps working without live video.
    use_hook(|| match video::mjpeg::MjpegBackend::start() {
        Ok(b) => video::install_mjpeg(std::sync::Arc::new(b)),
        Err(e) => tracing::error!(error = %e, "failed to start MJPEG backend"),
    });
    use_hook(|| {
        video::install_go2rtc(std::sync::Arc::new(video::go2rtc::Go2rtcBackend::new()));
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
        health_groups: use_signal(|| saved_groups),
        selected_profile: use_signal(|| None),
        keyboard_action: use_signal(|| None),
        log_to_file: use_signal(|| cfg.log_to_file),
        tls_strict: use_signal(|| cfg.tls_strict),
    };
    // Seed the TLS-strict atomic from config so the first snapshot fetch
    // after launch already honours the saved preference.
    api::set_tls_strict(cfg.tls_strict);
    use_context_provider(|| ctx);

    // Auto-save when theme / locale / log / tls preference change.
    // Also pushes tls_strict into the api atomic so a toggle takes effect
    // on the next snapshot without a restart (unlike log_to_file).
    use_effect(move || {
        let theme = *ctx.theme.read();
        let locale = *ctx.locale.read();
        let log_to_file = *ctx.log_to_file.read();
        let tls_strict = *ctx.tls_strict.read();
        api::set_tls_strict(tls_strict);
        persist::save_config(theme, locale, log_to_file, tls_strict);
    });

    // Re-verify auth when credentials change
    use_effect(move || {
        let _creds = ctx.global_credentials.read();
        device_ops::reverify_auth(ctx, ctx.devices);
    });

    // Auto-save credentials + devices when either changes (single keychain
    // write). `groups` is `.peek()`d (included in the blob, not subscribed) so a
    // device/cred change re-emits group creds too and can't clobber them.
    use_effect(move || {
        let creds = ctx.global_credentials.read().clone();
        let devices = ctx.devices.read().clone();
        let groups = ctx.health_groups.peek().clone();
        persist::save_credentials_and_devices(&creds, &devices, &groups);
    });

    // Auto-save health groups when they change. Symmetrically, `creds`/`devices`
    // are `.peek()`d so a group change re-emits the full keychain blob (device +
    // global creds included) — neither effect erases the other's keys.
    use_effect(move || {
        let groups = ctx.health_groups.read().clone();
        let creds = ctx.global_credentials.peek().clone();
        let devices = ctx.devices.peek().clone();
        persist::save_health_groups(&creds, &devices, &groups);
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
