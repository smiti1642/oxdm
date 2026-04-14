#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{Ctx, ToastLevel};
use dioxus::prelude::*;

const TOAST_DURATION_MS: u64 = 4000;

#[component]
pub fn ToastContainer() -> Element {
    let ctx = use_context::<Ctx>();
    let toasts = ctx.toasts.read();

    if toasts.is_empty() {
        return rsx! {};
    }

    rsx! {
        div { class: "toast-container",
            for toast in toasts.iter() {
                ToastItem {
                    key: "{toast.id}",
                    id: toast.id,
                    level: toast.level,
                    message: toast.message.clone(),
                }
            }
        }
    }
}

#[component]
fn ToastItem(id: u32, level: ToastLevel, message: String) -> Element {
    let ctx = use_context::<Ctx>();

    let icon_name = match level {
        ToastLevel::Success => "check",
        ToastLevel::Info => "info",
        ToastLevel::Warning => "alert-triangle",
        ToastLevel::Error => "x",
    };

    // Auto-dismiss after TOAST_DURATION_MS
    use_future(move || {
        let ctx = ctx;
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(TOAST_DURATION_MS)).await;
            ctx.dismiss_toast(id);
        }
    });

    rsx! {
        div { class: level.css_class(),
            span { class: "toast-icon", Icon { name: icon_name, size: 16 } }
            span { class: "toast-message", "{message}" }
            button {
                class: "toast-close",
                onclick: move |_| ctx.dismiss_toast(id),
                Icon { name: "x", size: 14 }
            }
        }
    }
}
