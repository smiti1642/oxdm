#![allow(non_snake_case)]
use crate::i18n;
use crate::state::Ctx;
use dioxus::prelude::*;

/// Inline error block with a Retry button. Used by every settings tab,
/// PTZ presets, imaging, etc. — anywhere a `use_resource` failure
/// should be surfaced with a one-click recovery path.
///
/// `error` is the raw error string from the resource. `on_retry` is
/// the close-over of `resource.restart()` from the calling component.
#[component]
pub fn TabError(error: String, on_retry: Callback<()>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        div { class: "tab-error",
            span { "{error}" }
            button {
                class: "btn btn-sm btn-ghost tab-error-retry",
                onclick: move |_| on_retry.call(()),
                {i18n::t(locale, "btn_retry")}
            }
        }
    }
}
