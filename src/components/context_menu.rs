#![allow(non_snake_case)]
use crate::components::Icon;
use dioxus::prelude::*;

/// A floating context menu rendered at a specific (x, y) position.
#[component]
pub fn ContextMenu(x: f64, y: f64, on_close: EventHandler<()>, children: Element) -> Element {
    rsx! {
        div {
            class: "ctx-menu-overlay",
            onmousedown: move |_| on_close.call(()),

            div {
                class: "ctx-menu",
                style: "left: {x}px; top: {y}px;",
                onmousedown: |e| e.stop_propagation(),
                {children}
            }
        }
    }
}

#[component]
pub fn CtxMenuItem(
    icon: &'static str,
    label: &'static str,
    danger: Option<bool>,
    on_click: EventHandler<()>,
) -> Element {
    let cls = if danger.unwrap_or(false) {
        "ctx-menu-item ctx-menu-item--danger"
    } else {
        "ctx-menu-item"
    };

    rsx! {
        button {
            class: cls,
            onclick: move |_| on_click.call(()),
            span { class: "ctx-menu-item-icon", Icon { name: icon, size: 14 } }
            "{label}"
        }
    }
}
