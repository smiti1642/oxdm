#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{ConfirmDialog, Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;

/// Standard ONVIF user levels. Cameras may also accept `"Anonymous"` and
/// `"Extended"` but these three cover every real-world config. Order of
/// decreasing privilege.
const USER_LEVELS: &[&str] = &["Administrator", "Operator", "User"];

#[component]
pub fn UsersTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let mut users = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move { api::get_users(&addr, &creds).await }
    });

    // Add-user form buffers.
    let new_username = use_signal(String::new);
    let new_password = use_signal(String::new);
    let new_level = use_signal(|| "User".to_string());

    // Inline-edit: which username is being edited (None = no row is in
    // edit mode). When set, that row replaces its static cells with
    // password + level inputs + Save/Cancel actions.
    let editing = use_signal(|| None::<String>);
    let edit_password = use_signal(String::new);
    let edit_level = use_signal(String::new);

    // ── Callbacks ──────────────────────────────────────────────────────────
    let create_cb = use_callback(move |_: ()| {
        let uname = new_username.peek().trim().to_string();
        let pass = new_password.peek().clone();
        let level = new_level.peek().clone();
        if uname.is_empty() || pass.is_empty() {
            ctx.push_toast(
                ToastLevel::Warning,
                i18n::t(locale, "user_create_needs_fields"),
            );
            return;
        }
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            match api::create_user(&addr_s, &creds_s, &uname, &pass, &level).await {
                Ok(()) => {
                    new_username.clone().set(String::new());
                    new_password.clone().set(String::new());
                    users.restart();
                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "user_created"));
                }
                Err(e) => ctx.push_toast(ToastLevel::Error, e),
            }
        });
    });

    let save_cb = use_callback(move |_: ()| {
        let Some(target) = editing.peek().clone() else {
            return;
        };
        let level = edit_level.peek().clone();
        // Empty password = no change. Some firmware rejects SetUser with
        // an empty Password element, so we omit it entirely in that case.
        let pass = edit_password.peek().clone();
        let pass_opt = if pass.is_empty() { None } else { Some(pass) };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            match api::set_user(&addr_s, &creds_s, &target, pass_opt.as_deref(), &level).await {
                Ok(()) => {
                    editing.clone().set(None);
                    edit_password.clone().set(String::new());
                    users.restart();
                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "user_updated"));
                }
                Err(e) => ctx.push_toast(ToastLevel::Error, e),
            }
        });
    });

    let delete_cb = use_callback(move |target: String| {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let target_for_closure = target.clone();
        let confirm_msg = i18n::t(locale, "user_delete_confirm").replace("{name}", &target);
        ctx.dialog.clone().set(Some(ConfirmDialog {
            title: i18n::t(locale, "user_delete_title").to_string(),
            message: confirm_msg,
            confirm_label: i18n::t(locale, "btn_confirm").to_string(),
            cancel_label: i18n::t(locale, "btn_cancel").to_string(),
            dangerous: true,
            on_confirm: EventHandler::new(move |_| {
                let addr_s = addr_s.clone();
                let creds_s = creds_s.clone();
                let target = target_for_closure.clone();
                spawn(async move {
                    match api::delete_user(&addr_s, &creds_s, &target).await {
                        Ok(()) => {
                            users.restart();
                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, "user_deleted"));
                        }
                        Err(e) => ctx.push_toast(ToastLevel::Error, e),
                    }
                });
            }),
        }));
    });

    // ── Render ─────────────────────────────────────────────────────────────
    rsx! {
        match &*users.read_unchecked() {
            None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
            Some(Err(e)) => rsx! {
                crate::components::TabError {
                    error: e.clone(),
                    on_retry: move |_| users.restart(),
                }
            },
            Some(Ok(user_list)) => rsx! {
                table { class: "prop-table",
                    tr { class: "prop-table-header",
                        th { class: "prop-label", {i18n::t(locale, "user_name")} }
                        th { class: "prop-label", {i18n::t(locale, "user_level")} }
                        th { class: "prop-label", "" }
                    }
                    for user in user_list {
                        {
                            let is_editing = editing
                                .read()
                                .as_ref()
                                .map(|u| u == &user.username)
                                .unwrap_or(false);
                            let uname_for_delete = user.username.clone();
                            let uname_for_edit = user.username.clone();
                            let current_level = user.user_level.clone();
                            rsx! {
                                tr {
                                    td { class: "prop-value", "{user.username}" }
                                    if is_editing {
                                        td { class: "prop-value",
                                            UserLevelSelect { value: edit_level }
                                        }
                                        td { class: "user-row-actions",
                                            input {
                                                class: "user-edit-password",
                                                r#type: "password",
                                                placeholder: i18n::t(locale, "user_edit_pw_placeholder"),
                                                value: "{*edit_password.read()}",
                                                oninput: move |e| edit_password.clone().set(e.value()),
                                            }
                                            button {
                                                class: "btn btn-sm btn-primary",
                                                onclick: move |_| save_cb.call(()),
                                                Icon { name: "check", size: 12 }
                                            }
                                            button {
                                                class: "btn btn-sm",
                                                onclick: move |_| {
                                                    editing.clone().set(None);
                                                    edit_password.clone().set(String::new());
                                                },
                                                Icon { name: "x", size: 12 }
                                            }
                                        }
                                    } else {
                                        td { class: "prop-value", "{user.user_level}" }
                                        td { class: "user-row-actions",
                                            button {
                                                class: "user-row-btn",
                                                title: i18n::t(locale, "btn_edit"),
                                                onclick: move |_| {
                                                    editing.clone().set(Some(uname_for_edit.clone()));
                                                    edit_level.clone().set(current_level.clone());
                                                    edit_password.clone().set(String::new());
                                                },
                                                Icon { name: "pencil", size: 12 }
                                            }
                                            button {
                                                class: "user-row-btn user-row-btn--danger",
                                                title: i18n::t(locale, "btn_delete"),
                                                onclick: move |_| delete_cb.call(uname_for_delete.clone()),
                                                Icon { name: "trash-2", size: 12 }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if user_list.is_empty() {
                    div { class: "tab-empty", {i18n::t(locale, "user_none")} }
                }

                // ── Add user form ──
                div { class: "prop-section-header", {i18n::t(locale, "user_add_section")} }
                div { class: "id-edit-form",
                    div { class: "id-edit-row",
                        label { class: "id-edit-label", {i18n::t(locale, "user_name")} }
                        input {
                            class: "id-edit-input",
                            r#type: "text",
                            value: "{*new_username.read()}",
                            oninput: move |e| new_username.clone().set(e.value()),
                        }
                    }
                    div { class: "id-edit-row",
                        label { class: "id-edit-label", {i18n::t(locale, "cred_password")} }
                        input {
                            class: "id-edit-input",
                            r#type: "password",
                            value: "{*new_password.read()}",
                            oninput: move |e| new_password.clone().set(e.value()),
                        }
                    }
                    div { class: "id-edit-row",
                        label { class: "id-edit-label", {i18n::t(locale, "user_level")} }
                        UserLevelSelect { value: new_level }
                    }
                    div { class: "id-edit-actions",
                        button {
                            class: "btn btn-md btn-primary",
                            onclick: move |_| create_cb.call(()),
                            Icon { name: "plus", size: 14 }
                            " "
                            {i18n::t(locale, "user_add")}
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn UserLevelSelect(value: Signal<String>) -> Element {
    let current = value.read().clone();
    rsx! {
        select {
            class: "id-edit-input",
            value: "{current}",
            onchange: move |e| value.clone().set(e.value()),
            for level in USER_LEVELS {
                option {
                    value: "{level}",
                    selected: *level == current,
                    "{level}"
                }
            }
        }
    }
}
