#![allow(non_snake_case)]
use crate::components::{AboutDialog, Icon, LogViewer};
use crate::i18n;
use crate::state::{Ctx, Theme, View};
use dioxus::prelude::*;

#[component]
pub fn Topbar() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let theme = *ctx.theme.read();
    let mut theme_sig = ctx.theme;
    let mut locale_sig = ctx.locale;
    let about_open = use_signal(|| false);
    let logs_open = use_signal(|| false);

    let locale_label = locale.label();
    let theme_icon = match theme {
        Theme::Dark => "moon",
        Theme::Light => "sun",
        Theme::Classic => "monitor",
    };

    rsx! {
        header { class: "topbar",
            div { class: "topbar-left",
                span { class: "topbar-logo",
                    Icon { name: "lens", size: 18 }
                    " OxDM"
                }
            }
            div { class: "topbar-center" }
            div { class: "topbar-right",
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "hbatch_title"),
                    onclick: {
                        let mut view_sig = ctx.view;
                        let mut health_list = ctx.health_list;
                        move |_| {
                            health_list.set(crate::state::HealthListSel::AllDevices);
                            view_sig.set(View::HealthOverview);
                        }
                    },
                    Icon { name: "activity", size: 16 }
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
                    title: i18n::t(locale, "logs_title"),
                    onclick: {
                        let mut logs_open = logs_open;
                        move |_| logs_open.set(true)
                    },
                    Icon { name: "file-text", size: 16 }
                }
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "tooltip_help"),
                    onclick: {
                        let mut about_open = about_open;
                        move |_| about_open.set(true)
                    },
                    Icon { name: "help-circle", size: 16 }
                }
            }
        }
        AboutDialog { open: about_open }
        LogViewer { open: logs_open }
    }
}
