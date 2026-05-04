#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{ConfirmDialog, Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;

#[component]
pub fn MaintenanceTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    rsx! {
        div { class: "maintenance-actions",
            // ── Reboot ──────────────────────────────────────────────────────
            div { class: "maintenance-card",
                div { class: "maintenance-card-header",
                    span { class: "maintenance-icon", Icon { name: "rotate-cw", size: 18 } }
                    span { class: "maintenance-title", {i18n::t(locale, "maint_reboot")} }
                }
                p { class: "maintenance-desc", {i18n::t(locale, "maint_reboot_desc")} }
                button {
                    class: "btn btn-md btn-ghost",
                    onclick: move |_| {
                        let addr = addr.read().clone();
                        let creds = creds.peek().clone();
                        ctx.dialog.clone().set(Some(ConfirmDialog {
                            title: i18n::t(locale, "maint_reboot").to_string(),
                            message: i18n::t(locale, "maint_reboot_confirm").to_string(),
                            confirm_label: i18n::t(locale, "btn_confirm").to_string(),
                            cancel_label: i18n::t(locale, "btn_cancel").to_string(),
                            dangerous: true,
                            on_confirm: EventHandler::new(move |_| {
                                let addr = addr.clone();
                                let creds = creds.clone();
                                spawn(async move {
                                    match api::system_reboot(&addr, &creds).await {
                                        Ok(msg) => ctx.push_toast(ToastLevel::Success, msg),
                                        Err(e) => ctx.push_toast(ToastLevel::Error, e),
                                    }
                                });
                            }),
                        }));
                    },
                    {i18n::t(locale, "maint_reboot")}
                }
            }

            // ── Factory Reset ───────────────────────────────────────────────
            div { class: "maintenance-card maintenance-card--danger",
                div { class: "maintenance-card-header",
                    span { class: "maintenance-icon", Icon { name: "alert-triangle", size: 18 } }
                    span { class: "maintenance-title", {i18n::t(locale, "maint_factory_reset")} }
                }
                p { class: "maintenance-desc", {i18n::t(locale, "maint_factory_reset_desc")} }
                button {
                    class: "btn btn-md btn-danger",
                    onclick: move |_| {
                        let addr = addr.read().clone();
                        let creds = creds.peek().clone();
                        ctx.dialog.clone().set(Some(ConfirmDialog {
                            title: i18n::t(locale, "maint_factory_reset").to_string(),
                            message: i18n::t(locale, "maint_factory_reset_confirm").to_string(),
                            confirm_label: i18n::t(locale, "btn_confirm").to_string(),
                            cancel_label: i18n::t(locale, "btn_cancel").to_string(),
                            dangerous: true,
                            on_confirm: EventHandler::new(move |_| {
                                let addr = addr.clone();
                                let creds = creds.clone();
                                spawn(async move {
                                    match api::set_system_factory_default(&addr, &creds, "Hard").await {
                                        Ok(()) => ctx.push_toast(
                                            ToastLevel::Success,
                                            i18n::t(locale, "maint_factory_reset_ok"),
                                        ),
                                        Err(e) => ctx.push_toast(ToastLevel::Error, e),
                                    }
                                });
                            }),
                        }));
                    },
                    {i18n::t(locale, "maint_factory_reset")}
                }
            }
        }
    }
}
