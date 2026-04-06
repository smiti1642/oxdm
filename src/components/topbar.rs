#![allow(non_snake_case)]
use dioxus::prelude::*;

#[component]
pub fn Topbar() -> Element {
    rsx! {
        header { class: "topbar",
            div { class: "topbar-left",
                span { class: "topbar-logo", "⬡ OxDM" }
            }
            div { class: "topbar-center",
                div { class: "topbar-search-wrap",
                    span { class: "topbar-search-icon", "⌕" }
                    input {
                        class: "topbar-search",
                        r#type: "text",
                        placeholder: "Search devices…",
                    }
                }
            }
            div { class: "topbar-right",
                button { class: "icon-btn", title: "Settings",   "⚙" }
                button { class: "icon-btn", title: "Theme",      "◑" }
                button { class: "icon-btn", title: "Help",       "?" }
            }
        }
    }
}
