#![allow(non_snake_case)]
use dioxus::prelude::*;

/// Standard modal-overlay scaffolding. Wraps `children` in a
/// `<div.dialog-overlay>` with:
///
/// - click-outside to close (overlay catches mousedown; the inner
///   `dialog` div stops propagation so clicks on the body don't close)
/// - Escape-to-close via onkeydown on the overlay (tabindex=-1 keeps
///   the div focusable so keydown events from focused inputs bubble
///   here without us having to install a global listener)
///
/// `inner_class` is the inner `dialog` div's class — most callers want
/// `"dialog"`, the wide variants use `"dialog dialog--wide"`.
///
/// `on_close` is called from both routes (click-outside and Escape).
/// Components that need to clear extra local state (input drafts etc.)
/// should do that in the callback.
#[component]
pub fn DialogOverlay(on_close: Callback<()>, inner_class: String, children: Element) -> Element {
    rsx! {
        div {
            class: "dialog-overlay",
            tabindex: "-1",
            onmousedown: move |_| on_close.call(()),
            onkeydown: move |evt: KeyboardEvent| {
                if evt.key() == Key::Escape {
                    on_close.call(());
                }
            },
            div {
                class: "{inner_class}",
                onmousedown: |e| e.stop_propagation(),
                {children}
            }
        }
    }
}
