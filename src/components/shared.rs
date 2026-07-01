#![allow(non_snake_case)]
//! Reusable UI components used across multiple views.

use crate::components::Icon;
use crate::i18n;
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

/// A username text input + password field pair. Shared by the global,
/// add-device, and HealthCheck-group credential dialogs so the markup isn't
/// duplicated per call site.
#[component]
pub fn CredentialsFields(
    username: Signal<String>,
    password: Signal<String>,
    locale: crate::state::Locale,
) -> Element {
    rsx! {
        div { class: "form-field",
            label { class: "form-label", {i18n::t(locale, "cred_username")} }
            input {
                class: "form-input",
                r#type: "text",
                placeholder: i18n::t(locale, "cred_username"),
                value: "{username}",
                oninput: move |e| username.clone().set(e.value()),
            }
        }
        div { class: "form-field",
            label { class: "form-label", {i18n::t(locale, "cred_password")} }
            PasswordField {
                value: password,
                placeholder: i18n::t(locale, "cred_password"),
            }
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
