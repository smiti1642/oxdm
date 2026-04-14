#![allow(non_snake_case)]
use crate::state::Ctx;
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
                    level_class: toast.level.css_class(),
                    icon: toast.level.icon(),
                    message: toast.message.clone(),
                }
            }
        }
    }
}

#[component]
fn ToastItem(id: u32, level_class: &'static str, icon: &'static str, message: String) -> Element {
    let ctx = use_context::<Ctx>();

    // Auto-dismiss after TOAST_DURATION_MS
    use_future(move || {
        let ctx = ctx;
        async move {
            tokio::time::sleep(std::time::Duration::from_millis(TOAST_DURATION_MS)).await;
            ctx.dismiss_toast(id);
        }
    });

    rsx! {
        div { class: level_class,
            span { class: "toast-icon", "{icon}" }
            span { class: "toast-message", "{message}" }
            button {
                class: "toast-close",
                onclick: move |_| ctx.dismiss_toast(id),
                "\u{00D7}"
            }
        }
    }
}
