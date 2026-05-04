#![allow(non_snake_case)]
use crate::components::{Icon, TabError};
use crate::state::{ConfirmDialog, Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;
use oxvif::{OsdColor, OsdConfiguration, OsdOptions, OsdPosition, OsdTextString};

/// Default font color sent on CreateOSD when the camera doesn't have
/// an existing one to copy from. White in the standard YCbCr 8-bit
/// limited range (luma 16–235, chroma 16–240): X=235 max luma,
/// Y/Z=128 neutral chroma. Matches what every observed camera ships
/// as the factory-default OSD color.
fn default_font_color() -> OsdColor {
    OsdColor {
        x: 235.0,
        y: 128.0,
        z: 128.0,
        colorspace: Some("http://www.onvif.org/ver10/colorspace/YCbCr".to_string()),
        transparent: None,
    }
}

/// Fallback values used when the camera doesn't advertise OSDOptions
/// (or advertises an empty list). The four standard corners + the
/// four canonical text types are universally supported by the spec
/// even if the camera doesn't itemise them. Date/time formats and
/// font size have NO sensible fallback — different cameras really do
/// require specific strings, so we leave those empty when unknown
/// and fall back to a free-text input.
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
            api::get_osds(&addr, &creds, &token).await
        }
    });

    // Fetch the camera's OSDOptions in parallel — used to populate
    // dropdowns in the editor with values the camera will accept.
    // Failures here are non-fatal (editor falls back to free-text
    // inputs) so don't block the OSD list rendering.
    let options = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let profile = profile_sig.read().clone();
        async move {
            let token = profile.ok_or_else(|| "no_profile".to_string())?;
            api::get_osd_options(&addr, &creds, &token).await
        }
    });

    // Edit drawer state — Some(token) edits an existing OSD by token,
    // None means hidden, Some("") means "create new".
    let editing: Signal<Option<String>> = use_signal(|| None);

    // Pre-count existing OSDs by text type. Used together with
    // options.max_per_text_type so the editor can mark a type
    // exhausted when the camera enforces per-type quotas (Genetec
    // observed: DateAndTime max 1).
    let existing_counts: std::collections::HashMap<String, u32> = osds
        .read_unchecked()
        .as_ref()
        .and_then(|r| r.as_ref().ok())
        .map(|list| {
            let mut m: std::collections::HashMap<String, u32> = Default::default();
            for o in list {
                if let Some(t) = o.text_string.as_ref() {
                    *m.entry(t.type_.clone()).or_insert(0) += 1;
                }
            }
            m
        })
        .unwrap_or_default();

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
                    options: options.read_unchecked().as_ref().and_then(|r| r.as_ref().ok()).cloned(),
                    existing_counts: existing_counts.clone(),
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
                                    match api::delete_osd(&addr_s, &creds_s, &token).await {
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
    /// Camera-advertised OSD options. `None` if GetOSDOptions failed
    /// or hasn't loaded yet — editor falls back to safe defaults
    /// (four-corner positions, free-text date/time formats).
    options: Option<OsdOptions>,
    /// Count of existing OSDs grouped by text type. Used together
    /// with `options.max_per_text_type` to grey out dropdown choices
    /// whose quota is exhausted (e.g. Genetec only allows one
    /// DateAndTime OSD).
    existing_counts: std::collections::HashMap<String, u32>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let is_create = initial.is_none();

    // Build the lists the form will offer. Prefer what the camera
    // told us; fall back to safe spec defaults if the camera didn't
    // advertise specifics.
    let text_types: Vec<String> = options
        .as_ref()
        .filter(|o| !o.text_types.is_empty())
        .map(|o| o.text_types.clone())
        .unwrap_or_else(|| TEXT_TYPES.iter().map(|s| s.to_string()).collect());
    let position_types: Vec<String> = options
        .as_ref()
        .filter(|o| !o.position_types.is_empty())
        .map(|o| o.position_types.clone())
        .unwrap_or_else(|| POSITION_TYPES.iter().map(|s| s.to_string()).collect());
    let date_formats: Vec<String> = options
        .as_ref()
        .map(|o| o.date_formats.clone())
        .unwrap_or_default();
    let time_formats: Vec<String> = options
        .as_ref()
        .map(|o| o.time_formats.clone())
        .unwrap_or_default();
    let font_size_range = options.as_ref().and_then(|o| o.font_size_range);
    let max_per_type = options
        .as_ref()
        .map(|o| o.max_per_text_type.clone())
        .unwrap_or_default();

    // Seed form state once from `initial` (or first allowed option
    // when creating) — subsequent re-renders keep whatever the user
    // typed.
    let first_text_type = text_types
        .first()
        .cloned()
        .unwrap_or_else(|| "Plain".to_string());
    let initial_for_seed = initial.clone();
    let mut text_type = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().map(|t| t.type_.clone()))
            .unwrap_or(first_text_type)
    });
    let initial_for_seed = initial.clone();
    let mut plain_text = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().and_then(|t| t.plain_text.clone()))
            .unwrap_or_default()
    });
    // For date/time format default to the camera's first allowed
    // value — picking yyyy-MM-dd by guess was the bug that caused
    // every CreateOSD to fail with `Argument Value`.
    let first_date = date_formats.first().cloned().unwrap_or_default();
    let initial_for_seed = initial.clone();
    let mut date_format = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().and_then(|t| t.date_format.clone()))
            .filter(|s| !s.is_empty())
            .unwrap_or(first_date)
    });
    let first_time = time_formats.first().cloned().unwrap_or_default();
    let initial_for_seed = initial.clone();
    let mut time_format = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().and_then(|t| t.time_format.clone()))
            .filter(|s| !s.is_empty())
            .unwrap_or(first_time)
    });
    let first_position = position_types
        .first()
        .cloned()
        .unwrap_or_else(|| "UpperLeft".to_string());
    let initial_for_seed = initial.clone();
    let mut position = use_signal(move || {
        initial_for_seed
            .as_ref()
            .map(|i| i.position.type_.clone())
            .unwrap_or(first_position)
    });
    let default_font = font_size_range.map(|(min, _)| min).unwrap_or(20);
    let initial_for_seed = initial.clone();
    let mut font_size = use_signal(move || {
        initial_for_seed
            .as_ref()
            .and_then(|i| i.text_string.as_ref().and_then(|t| t.font_size))
            .unwrap_or(default_font)
            .to_string()
    });

    let initial_for_save = initial.clone();
    let saving = use_signal(|| false);

    // Quota gate: when creating, the chosen text type can't exceed
    // its per-type max. Editing an existing OSD doesn't add to the
    // count, so the gate is skipped there.
    let current_type = text_type.read().clone();
    let used_for_type = existing_counts.get(&current_type).copied().unwrap_or(0);
    let max_for_type = max_per_type.get(&current_type).copied();
    let quota_full = is_create
        && max_for_type
            .map(|max| used_for_type >= max)
            .unwrap_or(false);
    let quota_label = max_for_type
        .map(|max| format!("{used_for_type}/{max}"))
        .unwrap_or_default();

    rsx! {
        div { class: "osd-editor",
            div { class: "osd-editor-title",
                {if is_create { i18n::t(locale, "osd_create") } else { i18n::t(locale, "osd_edit") }}
            }
            div { class: "osd-editor-grid",
                label { class: "osd-editor-label", {i18n::t(locale, "osd_field_text_type")} }
                div { class: "osd-editor-type",
                    select {
                        class: "form-input",
                        value: "{text_type}",
                        onchange: move |evt: Event<FormData>| text_type.set(evt.value()),
                        for t in text_types.iter().cloned() {
                            option { value: "{t}", "{t}" }
                        }
                    }
                    if !quota_label.is_empty() {
                        span {
                            class: if quota_full { "osd-editor-quota osd-editor-quota--full" } else { "osd-editor-quota" },
                            "{quota_label}"
                        }
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

                if matches!(text_type.read().as_str(), "Date" | "DateAndTime") {
                    label { class: "osd-editor-label", {i18n::t(locale, "osd_field_date_format")} }
                    if date_formats.is_empty() {
                        // Camera didn't advertise allowed formats —
                        // free-text input, user is on their own.
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "yyyy-MM-dd",
                            value: "{date_format}",
                            oninput: move |evt: Event<FormData>| date_format.set(evt.value()),
                        }
                    } else {
                        select {
                            class: "form-input",
                            value: "{date_format}",
                            onchange: move |evt: Event<FormData>| date_format.set(evt.value()),
                            for f in date_formats.iter().cloned() {
                                option { value: "{f}", "{f}" }
                            }
                        }
                    }
                }

                if matches!(text_type.read().as_str(), "Time" | "DateAndTime") {
                    label { class: "osd-editor-label", {i18n::t(locale, "osd_field_time_format")} }
                    if time_formats.is_empty() {
                        input {
                            class: "form-input",
                            r#type: "text",
                            placeholder: "HH:mm:ss",
                            value: "{time_format}",
                            oninput: move |evt: Event<FormData>| time_format.set(evt.value()),
                        }
                    } else {
                        select {
                            class: "form-input",
                            value: "{time_format}",
                            onchange: move |evt: Event<FormData>| time_format.set(evt.value()),
                            for f in time_formats.iter().cloned() {
                                option { value: "{f}", "{f}" }
                            }
                        }
                    }
                }

                label { class: "osd-editor-label", {i18n::t(locale, "osd_field_position")} }
                select {
                    class: "form-input",
                    value: "{position}",
                    onchange: move |evt: Event<FormData>| position.set(evt.value()),
                    for p in position_types.iter().cloned() {
                        option { value: "{p}", "{p}" }
                    }
                }

                label { class: "osd-editor-label", {i18n::t(locale, "osd_field_font_size")} }
                input {
                    class: "form-input",
                    r#type: "number",
                    min: font_size_range.map(|(min, _)| min.to_string()).unwrap_or_else(|| "8".to_string()),
                    max: font_size_range.map(|(_, max)| max.to_string()).unwrap_or_else(|| "72".to_string()),
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
                    disabled: *saving.read() || quota_full,
                    title: if quota_full { i18n::t(locale, "osd_quota_full") } else { "" },
                    onclick: move |_| {
                        let mut saving = saving;
                        if *saving.read() || quota_full { return; }
                        saving.set(true);
                        let initial = initial_for_save.clone();
                        let addr_s = addr.read().clone();
                        let creds_s = creds.read().clone();
                        let text_type_v = text_type.read().clone();
                        let plain_text_v = plain_text.read().clone();
                        let date_format_v = date_format.read().clone();
                        let time_format_v = time_format.read().clone();
                        let position_v = position.read().clone();
                        let font_size_v: Option<u32> = font_size.read().parse().ok();
                        let on_close = on_close;
                        let on_saved = on_saved;
                        let profile_sig = ctx.selected_profile;

                        spawn(async move {
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
                                    match api::get_video_source_config_token(&addr_s, &creds_s, &profile).await {
                                        Ok(t) => t,
                                        Err(e) => {
                                            ctx.push_toast(ToastLevel::Error, e);
                                            saving.set(false);
                                            return;
                                        }
                                    }
                                }
                            };

                            // Only set the field that matches the chosen
                            // text type. Cameras reject e.g. PlainText
                            // when type=Date with a schema validation
                            // fault — the spec only allows the matching
                            // field to be present.
                            let (plain, date_fmt, time_fmt) = match text_type_v.as_str() {
                                "Plain" => (Some(plain_text_v), None, None),
                                "Date" => (None, Some(date_format_v), None),
                                "Time" => (None, None, Some(time_format_v)),
                                "DateAndTime" => {
                                    (None, Some(date_format_v), Some(time_format_v))
                                }
                                _ => (Some(plain_text_v), None, None),
                            };
                            // Preserve the existing FontColor when
                            // editing; otherwise send a default white
                            // (X=235 luma, Y=Z=128 = neutral chroma in
                            // the standard YCbCr space). Some cameras
                            // (Genetec observed, Hikvision suspected)
                            // require FontColor on Create, returning a
                            // generic ter:InvalidArgs without it.
                            let font_color = initial
                                .as_ref()
                                .and_then(|i| i.text_string.as_ref().and_then(|t| t.font_color.clone()))
                                .or_else(|| Some(default_font_color()));
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
                                    plain_text: plain,
                                    date_format: date_fmt,
                                    time_format: time_fmt,
                                    font_size: font_size_v,
                                    font_color,
                                    ..Default::default()
                                }),
                                image_path: None,
                            };

                            let result = if osd.token.is_empty() {
                                api::create_osd(&addr_s, &creds_s, &osd).await.map(|_| ())
                            } else {
                                api::set_osd(&addr_s, &creds_s, &osd).await
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
