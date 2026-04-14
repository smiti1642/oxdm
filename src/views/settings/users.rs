#![allow(non_snake_case)]
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;

#[component]
pub fn UsersTab(addr: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let creds = ctx.global_credentials.read().clone();

    let users = use_resource(move || {
        let addr = addr.clone();
        let u = creds.username.clone();
        let p = creds.password.clone();
        async move {
            let (user, pass) = if u.is_empty() {
                (None, None)
            } else {
                (Some(u.as_str()), Some(p.as_str()))
            };
            api::get_users(&addr, user, pass).await
        }
    });

    rsx! {
        match &*users.read_unchecked() {
            None => rsx! {
                div { class: "tab-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(e)) => rsx! {
                div { class: "tab-error", "{e}" }
            },
            Some(Ok(user_list)) => rsx! {
                table { class: "prop-table",
                    tr {
                        td { class: "prop-label", {i18n::t(locale, "user_name")} }
                        td { class: "prop-label", {i18n::t(locale, "user_level")} }
                    }
                    for user in user_list {
                        tr {
                            td { class: "prop-value", "{user.username}" }
                            td { class: "prop-value", "{user.user_level}" }
                        }
                    }
                }
                if user_list.is_empty() {
                    div { class: "tab-empty", {i18n::t(locale, "user_none")} }
                }
            },
        }
    }
}
