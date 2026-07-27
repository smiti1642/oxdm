#![allow(non_snake_case)]
use crate::components::Icon;
use dioxus::prelude::*;

/// A floating context menu rendered at a specific (x, y) position.
///
/// The position is **clamped to the viewport**. Right-clicking a device low in
/// the sidebar used to put the menu's top edge near the bottom of the window,
/// leaving most of it below the fold and unclickable — there is no scrolling
/// out to it, because the menu is `position: fixed`.
///
/// The clamp is `min(click, viewport - reserve)` in CSS, so the browser does
/// the viewport arithmetic and no JS measurement is needed (this app
/// deliberately avoids `eval`). The reserve is an **upper bound** on the
/// menu's size, not its measured size — the tallest menu here is six items —
/// and `.ctx-menu` enforces the vertical bound with `max-height` +
/// `overflow-y: auto`, so it cannot be escaped.
///
/// A click with room to spare is positioned exactly where it always was; only
/// a click inside the reserve band near an edge moves.
const RESERVE_V: &str = "var(--ctx-menu-max-h)";
const RESERVE_H: &str = "var(--ctx-menu-reserve-w)";

#[component]
pub fn ContextMenu(x: f64, y: f64, on_close: EventHandler<()>, children: Element) -> Element {
    // `max(8px, …)` keeps the menu off the very edge, and keeps the value sane
    // if the window is ever shorter/narrower than the reserve.
    let style = format!(
        "left: max(8px, min({x}px, calc(100vw - {RESERVE_H} - 8px))); \
         top: max(8px, min({y}px, calc(100vh - {RESERVE_V} - 8px)));"
    );
    rsx! {
        div {
            class: "ctx-menu-overlay",
            onmousedown: move |_| on_close.call(()),

            div {
                class: "ctx-menu",
                style: "{style}",
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
