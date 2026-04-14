#![allow(non_snake_case)]
use crate::state::Credentials;
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;

#[component]
pub fn UsersTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let users = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move {
            let (user, pass) = if creds.username.is_empty() {
                (None, None)
            } else {
                (Some(creds.username.as_str()), Some(creds.password.as_str()))
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
                    tr { class: "prop-table-header",
                        th { class: "prop-label", {i18n::t(locale, "user_name")} }
                        th { class: "prop-label", {i18n::t(locale, "user_level")} }
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
