#![allow(non_snake_case)]
use crate::components::GlobalCredentialsDialog;
use crate::i18n;
use crate::state::Ctx;
use dioxus::prelude::*;

#[component]
pub fn Topbar() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let theme = *ctx.theme.read();
    let mut theme_sig = ctx.theme;
    let mut locale_sig = ctx.locale;
    let mut creds_open = use_signal(|| false);

    let theme_icon = theme.icon();
    let locale_label = locale.label();

    rsx! {
        header { class: "topbar",
            div { class: "topbar-left",
                span { class: "topbar-logo", "\u{2B21} OxDM" }
            }
            div { class: "topbar-center",
                div { class: "topbar-search-wrap",
                    span { class: "topbar-search-icon", "\u{2315}" }
                    input {
                        class: "topbar-search",
                        r#type: "text",
                        placeholder: i18n::t(locale, "search_placeholder"),
                    }
                }
            }
            div { class: "topbar-right",
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "tooltip_settings"),
                    onclick: move |_| creds_open.set(true),
                    "\u{2699}"
                }
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "tooltip_theme"),
                    onclick: move |_| theme_sig.set(theme.next()),
                    "{theme_icon}"
                }
                button {
                    class: "icon-btn icon-btn--label",
                    title: i18n::t(locale, "tooltip_language"),
                    onclick: move |_| locale_sig.set(locale.next()),
                    "{locale_label}"
                }
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "tooltip_help"),
                    "?"
                }
            }
        }

        GlobalCredentialsDialog { open: creds_open }
    }
}
