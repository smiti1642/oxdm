#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::Ctx;
use dioxus::prelude::*;

const APP_VERSION: &str = env!("CARGO_PKG_VERSION");
/// oxvif version. Bumped manually when the Cargo.toml dependency moves —
/// `cargo metadata` lookup at runtime would mean shipping cargo, which
/// the single-binary release explicitly avoids.
const OXVIF_VERSION: &str = "0.9.3";
const REPO_URL: &str = "https://github.com/smiti1642/oxdm";

#[component]
pub fn AboutDialog(open: Signal<bool>) -> Element {
    if !*open.read() {
        return rsx! {};
    }
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let log_path = crate::log_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_default();

    rsx! {
        div {
            class: "dialog-overlay",
            tabindex: "-1",
            onmousedown: {
                let mut open = open;
                move |_| open.set(false)
            },
            onkeydown: {
                let mut open = open;
                move |evt: KeyboardEvent| {
                    if evt.key() == Key::Escape {
                        open.set(false);
                    }
                }
            },

            div {
                class: "dialog about-dialog",
                onmousedown: |e| e.stop_propagation(),

                div { class: "dialog-header",
                    span { class: "dialog-title", "About OxDM" }
                }
                div { class: "dialog-body",
                    div { class: "about-icon", Icon { name: "hexagon", size: 48 } }
                    div { class: "about-name", "OxDM" }
                    div { class: "about-sub", {i18n::t(locale, "app_subtitle")} }
                    div { class: "about-versions",
                        div { "OxDM v{APP_VERSION}" }
                        div { "oxvif v{OXVIF_VERSION}" }
                    }
                    if !log_path.is_empty() {
                        div { class: "about-logpath",
                            span { class: "about-logpath-label", {i18n::t(locale, "about_log_dir")} }
                            code { "{log_path}" }
                        }
                    }
                    label { class: "about-log-toggle",
                        input {
                            r#type: "checkbox",
                            checked: *ctx.log_to_file.read(),
                            onchange: {
                                let mut log_sig = ctx.log_to_file;
                                move |evt: Event<FormData>| log_sig.set(evt.checked())
                            },
                        }
                        span { class: "about-log-toggle-text",
                            {i18n::t(locale, "about_log_to_file")}
                        }
                        span { class: "about-log-toggle-hint",
                            {i18n::t(locale, "about_log_takes_effect")}
                        }
                    }

                    div { class: "about-shortcuts",
                        div { class: "about-shortcuts-title", {i18n::t(locale, "about_shortcuts")} }
                        div { class: "about-shortcut",
                            span { class: "about-shortcut-keys",
                                kbd { "Ctrl" } "+" kbd { "F" }
                            }
                            span { class: "about-shortcut-desc", {i18n::t(locale, "shortcut_focus_search")} }
                        }
                        div { class: "about-shortcut",
                            span { class: "about-shortcut-keys", kbd { "F5" } }
                            span { class: "about-shortcut-desc", {i18n::t(locale, "shortcut_scan")} }
                        }
                        div { class: "about-shortcut",
                            span { class: "about-shortcut-keys",
                                kbd { "↑" } " / " kbd { "↓" }
                            }
                            span { class: "about-shortcut-desc", {i18n::t(locale, "shortcut_nav_devices")} }
                        }
                        div { class: "about-shortcut",
                            span { class: "about-shortcut-keys", kbd { "Esc" } }
                            span { class: "about-shortcut-desc", {i18n::t(locale, "shortcut_close_modal")} }
                        }
                    }
                }
                div { class: "dialog-footer about-footer",
                    button {
                        class: "btn btn-md btn-ghost",
                        onclick: move |_| {
                            if let Some(dir) = crate::log_dir() {
                                if let Err(e) = opener::open(&dir) {
                                    tracing::warn!(error = %e, "open log dir failed");
                                }
                            }
                        },
                        {i18n::t(locale, "about_open_logs")}
                    }
                    button {
                        class: "btn btn-md btn-ghost",
                        onclick: move |_| {
                            if let Err(e) = opener::open_browser(REPO_URL) {
                                tracing::warn!(error = %e, "open repo failed");
                            }
                        },
                        {i18n::t(locale, "about_github")}
                    }
                    button {
                        class: "btn btn-md btn-primary",
                        onclick: {
                            let mut open = open;
                            move |_| open.set(false)
                        },
                        {i18n::t(locale, "btn_close")}
                    }
                }
            }
        }
    }
}
