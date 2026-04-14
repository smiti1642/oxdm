#![allow(non_snake_case)]
use crate::components::{GlobalCredentialsDialog, Icon};
use crate::i18n;
use crate::state::{Ctx, Theme};
use dioxus::prelude::*;

#[component]
pub fn Topbar() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let theme = *ctx.theme.read();
    let mut theme_sig = ctx.theme;
    let mut locale_sig = ctx.locale;
    let mut creds_open = use_signal(|| false);

    let locale_label = locale.label();
    let theme_icon = match theme {
        Theme::Dark => "moon",
        Theme::Light => "sun",
        Theme::Classic => "monitor",
    };

    // Show a warning dot if no global credentials are set
    let creds = ctx.global_credentials.read();
    let creds_empty = creds.username.is_empty();
    drop(creds);

    rsx! {
        header { class: "topbar",
            div { class: "topbar-left",
                span { class: "topbar-logo",
                    Icon { name: "hexagon", size: 18 }
                    " OxDM"
                }
            }
            // Topbar search removed — sidebar filter handles device search
            div { class: "topbar-center" }
            div { class: "topbar-right",
                button {
                    class: if creds_empty { "icon-btn icon-btn--warn" } else { "icon-btn" },
                    title: i18n::t(locale, "tooltip_credentials"),
                    onclick: move |_| creds_open.set(true),
                    Icon { name: "key", size: 16 }
                }
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "tooltip_theme"),
                    onclick: move |_| theme_sig.set(theme.next()),
                    Icon { name: theme_icon, size: 16 }
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
                    Icon { name: "help-circle", size: 16 }
                }
            }
        }

        GlobalCredentialsDialog { open: creds_open }
    }
}
