#![allow(non_snake_case)]
use crate::components::{Icon, TabError};
use crate::state::{ConfirmDialog, Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;
use oxvif::{OsdConfiguration, OsdPosition, OsdTextString};

const POSITION_TYPES: &[&str] = &["UpperLeft", "UpperRight", "LowerLeft", "LowerRight"];
const TEXT_TYPES: &[&str] = &["Plain", "Date", "Time", "DateAndTime"];

/// OSD ("on-screen display") configuration tab. Lists OSDs attached to
/// the selected profile's video source, with create / edit / delete.
///
/// Scope deliberately narrow for the first cut: text OSDs only, four
/// fixed corner positions, font size. Image OSDs, custom xy positions,
/// font/background colors, and persistence flag are all valid ONVIF
/// settings but skipped for MVP — most cameras only get configured for
/// "show date/time in the corner" and the simpler form covers that.
#[component]
pub fn OsdView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let profile_sig = ctx.selected_profile;

    let mut osds = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let profile = profile_sig.read().clone();
        async move {
            let token = profile.ok_or_else(|| "no_profile".to_string())?;
            let (u, p) = creds.as_options();
            api::get_osds(&addr, u, p, &token).await
        }
    });

    // Edit drawer state — Some(token) edits an existing OSD by token,
    // None means hidden, Some("") means "create new".
    let editing: Signal<Option<String>> = use_signal(|| None);

    rsx! {
        div { class: "osd-view",
            div { class: "content-header",
                Icon { name: "info", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_osd")} }
                button {
                    class: "btn btn-sm btn-primary",
                    style: "margin-left: auto",
                    onclick: {
                        let mut editing = editing;
                        move |_| editing.set(Some(String::new()))
                    },
                    Icon { name: "plus", size: 12 }
                    span { style: "margin-left: 4px", {i18n::t(locale, "osd_add")} }
                }
            }
            div { class: "osd-body",
                match &*osds.read_unchecked() {
                    None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
                    Some(Err(e)) if e == "no_profile" => rsx! {
                        div { class: "tab-empty", {i18n::t(locale, "live_video_no_profile")} }
                    },
                    Some(Err(e)) => rsx! {
                        TabError { error: e.clone(), on_retry: move |_| osds.restart() }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "tab-empty", {i18n::t(locale, "osd_empty")} }
                    },
                    Some(Ok(list)) => rsx! {
                        table { class: "prop-table osd-table",
                            tr { class: "prop-table-header",
                                th { {i18n::t(locale, "osd_col_type")} }
                                th { {i18n::t(locale, "osd_col_position")} }
                                th { {i18n::t(locale, "osd_col_content")} }
                                th { "" }
                            }
                            for osd in list.iter().cloned() {
                                OsdRow {
                                    key: "{osd.token}",
                                    osd,
                                    addr,
                                    creds,
                                    editing,
                                    refresh: move |_| osds.restart(),
                                }
                            }
                        }
                    },
                }
            }
            // Conditional drawer below the table — Some("") = create
            // new (with empty initial form), Some(token) = edit existing.
            if let Some(token) = editing.read().as_ref().cloned() {
                OsdEditor {
                    addr,
                    creds,
                    initial: editing_initial(&token, &osds.read_unchecked()),
                    on_close: {
                        let mut editing = editing;
                        move |_| editing.set(None)
                    },
                    on_saved: move |_| {
                        osds.restart();
                    },
                }
            }
        }
    }
}

fn editing_initial(
    token: &str,
    osds: &Option<Result<Vec<OsdConfiguration>, String>>,
) -> Option<OsdConfiguration> {
    if token.is_empty() {
        return None;
    }
    osds.as_ref()
        .and_then(|r| r.as_ref().ok())
        .and_then(|list| list.iter().find(|o| o.token == token).cloned())
}

#[component]
fn OsdRow(
    osd: OsdConfiguration,
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    editing: Signal<Option<String>>,
    refresh: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let content = osd
        .text_string
        .as_ref()
        .map(|t| match t.type_.as_str() {
            "Plain" => t.plain_text.clone().unwrap_or_default(),
            other => format!("[{other}]"),
        })
        .unwrap_or_else(|| osd.image_path.clone().unwrap_or_default());

    let token_for_edit = osd.token.clone();
    let token_for_delete = osd.token.clone();

    rsx! {
        tr {
            td { "{osd.type_}" }
            td { "{osd.position.type_}" }
            td { class: "osd-content-cell", "{content}" }
            td { class: "osd-actions",
                button {
                    class: "btn btn-sm btn-ghost",
                    onclick: {
                        let mut editing = editing;
                        let token = token_for_edit;
                        move |_| editing.set(Some(token.clone()))
                    },
                    {i18n::t(locale, "btn_edit")}
                }
                button {
                    class: "btn btn-sm btn-danger",
                    onclick: move |evt| {
                        evt.stop_propagation();
                        let token = token_for_delete.clone();
                        let addr_s = addr.read().clone();
                        let creds_s = creds.read().clone();
                        let confirm_label = i18n::t(locale, "btn_confirm").to_string();
                        let cancel_label = i18n::t(locale, "btn_cancel").to_string();
                        let title = i18n::t(locale, "osd_delete_title").to_string();
                        let message = i18n::t(locale, "osd_delete_confirm").to_string();
                        ctx.dialog.clone().set(Some(ConfirmDialog {
                            title,
                            message,
                            confirm_label,
                            cancel_label,
                            dangerous: true,
                            on_confirm: Callback::new(move |_| {
                                let token = token.clone();
                                let addr_s = addr_s.clone();
                                let creds_s = creds_s.clone();
                                let refresh = refresh;
                                spawn(async move {
                                    let (u, p) = creds_s.as_options();
                                    match api::delete_osd(&addr_s, u, p, &token).await {
                                        Ok(()) => {
                                            ctx.push_toast(
                                                ToastLevel::Success,
                                                i18n::t(locale, "osd_deleted"),
                                            );
                                            refresh.call(());
                                        }
                                        Err(e) => ctx.push_toast(ToastLevel::Error, e),
                                    }
                                });
                            }),
                        }));
                    },
                    {i18n::t(locale, "btn_delete")}
                }
            }
        }
    }
}

#[component]
fn OsdEditor(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    initial: Option<OsdConfiguration>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let is_create = initial.is_none();

    // Seed form state once from `initial` — subsequent re-renders keep
    // whatever the user typed.
    let initial_for_seed = initial.clone();
    let mut text_type = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().map(|t| t.type_.clone()))
            .unwrap_or_else(|| "Plain".to_string())
    });
    let initial_for_seed = initial.clone();
    let mut plain_text = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().and_then(|t| t.plain_text.clone()))
            .unwrap_or_default()
    });
    let initial_for_seed = initial.clone();
    let mut position = use_signal(move || {
        initial_for_seed
            .as_ref()
            .map(|i| i.position.type_.clone())
            .filter(|t| POSITION_TYPES.contains(&t.as_str()))
            .unwrap_or_else(|| "UpperLeft".to_string())
    });
    let initial_for_seed = initial.clone();
    let mut font_size = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().and_then(|t| t.font_size))
            .unwrap_or(20)
            .to_string()
    });

    let initial_for_save = initial.clone();
    let saving = use_signal(|| false);

    rsx! {
        div { class: "osd-editor",
            div { class: "osd-editor-title",
                {if is_create { i18n::t(locale, "osd_create") } else { i18n::t(locale, "osd_edit") }}
            }
            div { class: "osd-editor-grid",
                label { class: "osd-editor-label", {i18n::t(locale, "osd_field_text_type")} }
                select {
                    class: "form-input",
                    value: "{text_type}",
                    onchange: move |evt: Event<FormData>| text_type.set(evt.value()),
                    for t in TEXT_TYPES {
                        option { value: "{t}", "{t}" }
                    }
                }

                if *text_type.read() == "Plain" {
                    label { class: "osd-editor-label", {i18n::t(locale, "osd_field_text")} }
                    input {
                        class: "form-input",
                        r#type: "text",
                        value: "{plain_text}",
                        oninput: move |evt: Event<FormData>| plain_text.set(evt.value()),
                    }
                }

                label { class: "osd-editor-label", {i18n::t(locale, "osd_field_position")} }
                select {
                    class: "form-input",
                    value: "{position}",
                    onchange: move |evt: Event<FormData>| position.set(evt.value()),
                    for p in POSITION_TYPES {
                        option { value: "{p}", "{p}" }
                    }
                }

                label { class: "osd-editor-label", {i18n::t(locale, "osd_field_font_size")} }
                input {
                    class: "form-input",
                    r#type: "number",
                    min: "8", max: "72",
                    value: "{font_size}",
                    oninput: move |evt: Event<FormData>| font_size.set(evt.value()),
                }
            }
            div { class: "osd-editor-actions",
                button {
                    class: "btn btn-md btn-ghost",
                    onclick: move |_| on_close.call(()),
                    {i18n::t(locale, "btn_cancel")}
                }
                button {
                    class: "btn btn-md btn-primary",
                    disabled: *saving.read(),
                    onclick: move |_| {
                        let mut saving = saving;
                        if *saving.read() { return; }
                        saving.set(true);
                        let initial = initial_for_save.clone();
                        let addr_s = addr.read().clone();
                        let creds_s = creds.read().clone();
                        let text_type_v = text_type.read().clone();
                        let plain_text_v = plain_text.read().clone();
                        let position_v = position.read().clone();
                        let font_size_v: Option<u32> = font_size.read().parse().ok();
                        let on_close = on_close;
                        let on_saved = on_saved;
                        let profile_sig = ctx.selected_profile;

                        spawn(async move {
                            let (u, p) = creds_s.as_options();
                            // For CREATE we need to know the video source
                            // configuration token to attach the OSD to.
                            // For EDIT we reuse the existing one.
                            let vsc_token = match initial.as_ref() {
                                Some(i) => i.video_source_config_token.clone(),
                                None => {
                                    let profile = match profile_sig.read().clone() {
                                        Some(p) => p,
                                        None => {
                                            ctx.push_toast(ToastLevel::Error,
                                                i18n::t(locale, "live_video_no_profile"));
                                            saving.set(false);
                                            return;
                                        }
                                    };
                                    match api::get_video_source_config_token(&addr_s, u, p, &profile).await {
                                        Ok(t) => t,
                                        Err(e) => {
                                            ctx.push_toast(ToastLevel::Error, e);
                                            saving.set(false);
                                            return;
                                        }
                                    }
                                }
                            };

                            let osd = OsdConfiguration {
                                token: initial.as_ref().map(|i| i.token.clone()).unwrap_or_default(),
                                video_source_config_token: vsc_token,
                                type_: "Text".to_string(),
                                position: OsdPosition {
                                    type_: position_v,
                                    x: None,
                                    y: None,
                                },
                                text_string: Some(OsdTextString {
                                    type_: text_type_v,
                                    plain_text: Some(plain_text_v),
                                    font_size: font_size_v,
                                    ..Default::default()
                                }),
                                image_path: None,
                            };

                            let result = if osd.token.is_empty() {
                                api::create_osd(&addr_s, u, p, &osd).await.map(|_| ())
                            } else {
                                api::set_osd(&addr_s, u, p, &osd).await
                            };
                            match result {
                                Ok(()) => {
                                    ctx.push_toast(ToastLevel::Success,
                                        i18n::t(locale, "osd_saved"));
                                    on_saved.call(());
                                    on_close.call(());
                                }
                                Err(e) => ctx.push_toast(ToastLevel::Error, e),
                            }
                            saving.set(false);
                        });
                    },
                    {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}
