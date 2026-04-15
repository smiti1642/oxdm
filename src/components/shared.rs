#![allow(non_snake_case)]
//! Reusable UI components used across multiple views.

use crate::components::Icon;
use dioxus::prelude::*;

/// A key-value row for property tables.
#[component]
pub fn PropRow(label: String, value: String) -> Element {
    rsx! {
        tr {
            td { class: "prop-label", "{label}" }
            td { class: "prop-value", "{value}" }
        }
    }
}

/// A password input field with show/hide toggle.
#[component]
pub fn PasswordField(value: Signal<String>, placeholder: &'static str) -> Element {
    let mut show = use_signal(|| false);

    rsx! {
        div { class: "form-input-row",
            input {
                class: "form-input form-input--flex",
                r#type: if *show.read() { "text" } else { "password" },
                placeholder,
                value: "{value}",
                oninput: move |e| value.clone().set(e.value()),
            }
            button {
                class: "btn btn-ghost btn-sm",
                onclick: move |_| show.toggle(),
                if *show.read() {
                    Icon { name: "eye-off", size: 14 }
                } else {
                    Icon { name: "eye", size: 14 }
                }
            }
        }
    }
}
