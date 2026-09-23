#![allow(non_snake_case)]
use crate::components::TabError;
use crate::state::{Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;
use oxvif::VideoEncoding;

/// Encoder (stream) settings for the selected profile, embedded inside
/// the Imaging view so image quality and stream parameters live in one
/// place keyed off the same selected stream.
///
/// Scope: edits the profile-bound encoder config only; does NOT switch
/// the encoding type. Only fields the camera actually returned are
/// editable; everything else degrades gracefully.
#[component]
pub fn VideoEncoderSection(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let profile_sig = ctx.selected_profile;

    let mut data = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let profile = profile_sig.read().clone();
        async move { api::get_encoder_settings(&addr, &creds, profile.as_deref()).await }
    });

    // Read once, up here: the note cannot live inside the match arm below,
    // because `VideoEncoderForm` carries a content-based `key` and Dioxus
    // allows a key only on the first node of a block. That key is load-bearing
    // (see the comment where it is built), so the note moves instead.
    let fell_back_to = match &*data.read_unchecked() {
        Some(Ok((pick, _))) if pick.fell_back => Some(pick.token.clone()),
        _ => None,
    };

    rsx! {
        div { class: "prop-section-header", {i18n::t(locale, "nav_video_encoder")} }
        if let Some(tok) = fell_back_to {
            div { class: "channel-fallback-note",
                {i18n::t(locale, "channel_fell_back").replace("{token}", &tok)}
            }
        }
        match &*data.read_unchecked() {
            None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
            Some(Err(e)) if e == "no_encoder" => rsx! {
                div { class: "tab-empty", {i18n::t(locale, "ve_no_encoder")} }
            },
            Some(Err(e)) => rsx! {
                TabError { error: e.clone(), on_retry: move |_| data.restart() }
            },
            // `pick` is consumed above, where the fallback note is built.
            Some(Ok((_pick, settings))) => {
                let cfg = &settings.config;
                // Content-based key: a successful Apply refetches, and if
                // the fetched values differ the key changes and the form
                // remounts/reseeds. So when a camera silently ignores a
                // change, the form reverts to the camera's real value and
                // the user can see the edit didn't take.
                let fps = cfg.rate_control.as_ref().map(|r| r.frame_rate_limit).unwrap_or(0.0);
                let kbps = cfg.rate_control.as_ref().map(|r| r.bitrate_limit).unwrap_or(0);
                let key = format!(
                    "{}|{}|{}x{}|{}|{}|{}|{:?}|{:?}|{:?}",
                    settings.is_media2(), cfg.token, cfg.resolution.width, cfg.resolution.height, fps, kbps, cfg.quality, cfg.gov_length, cfg.profile, settings.options
                );
                rsx! {
                    VideoEncoderForm {
                        key: "{key}",
                        addr,
                        creds,
                        settings: settings.clone(),
                        on_saved: move |_| data.restart(),
                    }
                }
            }
        }
    }
}

fn parse_resolution(s: &str) -> Option<(u32, u32)> {
    let (w, h) = s.split_once('x')?;
    Some((w.trim().parse().ok()?, h.trim().parse().ok()?))
}

fn encoding_label(enc: &VideoEncoding) -> String {
    match enc {
        VideoEncoding::Jpeg => "JPEG".to_string(),
        VideoEncoding::H264 => "H.264".to_string(),
        VideoEncoding::H265 => "H.265".to_string(),
        VideoEncoding::Other(s) => s.clone(),
    }
}

#[component]
fn VideoEncoderForm(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    settings: ReadSignal<api::EncoderSettings>,
    on_saved: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let settings = settings.read();
    let config = &settings.config;
    let options = &settings.options;
    let res_options: Vec<String> = options
        .resolutions
        .iter()
        .map(ToString::to_string)
        .collect();
    let frame_rate_range = options.frame_rate_range.map(|r| (r.min, r.max));
    let bitrate_range = options.bitrate_range.map(|r| (r.min, r.max));
    let gov_range = options.gov_length_range.map(|r| (r.min, r.max));
    let quality_range = options.quality_range.map(|r| (r.min, r.max));
    let profiles = options.profiles.clone();
    let is_video_codec = matches!(config.encoding, VideoEncoding::H264 | VideoEncoding::H265);
    let has_rate_control = config.rate_control.is_some();
    let has_gov = is_video_codec && config.gov_length.is_some();
    let codec_profile = config.profile.clone();
    let has_profile = codec_profile.is_some() && !profiles.is_empty();
    let fractional_fps = settings.is_media2();
    let mut frame_rate_choices: Vec<String> = options
        .frame_rates
        .iter()
        .map(ToString::to_string)
        .collect();
    if let Some(rc) = config
        .rate_control
        .as_ref()
        .filter(|_| !frame_rate_choices.is_empty())
    {
        let current = rc.frame_rate_limit.to_string();
        if !frame_rate_choices.contains(&current) {
            frame_rate_choices.insert(0, current);
        }
    }

    // Current resolution must always be selectable even if the camera
    // didn't itemise it in the options list.
    let current_res = format!("{}x{}", config.resolution.width, config.resolution.height);
    let mut resolution_choices = res_options.clone();
    if !resolution_choices.contains(&current_res) {
        resolution_choices.insert(0, current_res.clone());
    }

    // Seed editable state once (this component remounts via its content
    // key in the parent, so seeds re-run when the camera's values change).
    let mut resolution = use_signal(|| current_res.clone());
    let mut frame_rate = use_signal(|| {
        config
            .rate_control
            .as_ref()
            .map(|rc| rc.frame_rate_limit.to_string())
            .unwrap_or_default()
    });
    let mut bitrate = use_signal(|| {
        config
            .rate_control
            .as_ref()
            .map(|rc| rc.bitrate_limit.to_string())
            .unwrap_or_default()
    });
    let mut gov = use_signal(|| config.gov_length.map(|g| g.to_string()).unwrap_or_default());
    let mut quality = use_signal(|| config.quality.to_string());
    let mut profile = use_signal(|| codec_profile.clone().unwrap_or_default());
    let saving = use_signal(|| false);

    let res_cur = resolution.read().clone();
    let prof_cur = profile.read().clone();
    let fps_valid = !has_rate_control || settings.parse_frame_rate(&frame_rate.read()).is_some();
    let settings_for_save = settings.clone();

    rsx! {
        div { class: "imaging-row",
            span { class: "imaging-label", {i18n::t(locale, "ve_encoding")} }
            span { class: "imaging-value", {encoding_label(&config.encoding)} }
        }

        div { class: "imaging-row",
            span { class: "imaging-label", {i18n::t(locale, "ve_resolution")} }
            select {
                class: "imaging-select",
                value: "{res_cur}",
                onchange: move |e: Event<FormData>| resolution.set(e.value()),
                for r in resolution_choices.iter().cloned() {
                    option { value: "{r}", selected: r == res_cur, "{r}" }
                }
            }
        }

        if has_rate_control {
            div { class: "imaging-row",
                span { class: "imaging-label", {i18n::t(locale, "ve_frame_rate")} }
                if frame_rate_choices.is_empty() {
                    input {
                        class: "imaging-select",
                        r#type: "number",
                        step: if fractional_fps { "any" } else { "1" },
                        min: frame_rate_range.map(|(min, _)| min.to_string()).unwrap_or_else(|| "0".into()),
                        max: frame_rate_range.map(|(_, max)| max.to_string()),
                        value: "{frame_rate}",
                        oninput: move |e: Event<FormData>| frame_rate.set(e.value()),
                    }
                } else {
                    select {
                        class: "imaging-select",
                        value: "{frame_rate}",
                        onchange: move |e: Event<FormData>| frame_rate.set(e.value()),
                        for fps in frame_rate_choices {
                            option { value: "{fps}", "{fps}" }
                        }
                    }
                }
                if !fps_valid {
                    span { class: "tab-error", {i18n::t(locale, "ve_invalid_fps")} }
                }
            }
        }

        if has_rate_control && is_video_codec {
            div { class: "imaging-row",
                span { class: "imaging-label", {i18n::t(locale, "ve_bitrate")} }
                input {
                    class: "imaging-select",
                    r#type: "number",
                    min: bitrate_range.map(|(min, _)| min.to_string()),
                    max: bitrate_range.map(|(_, max)| max.to_string()),
                    value: "{bitrate}",
                    oninput: move |e: Event<FormData>| bitrate.set(e.value()),
                }
            }
        }

        if has_gov {
            div { class: "imaging-row",
                span { class: "imaging-label", {i18n::t(locale, "ve_gov_length")} }
                input {
                    class: "imaging-select",
                    r#type: "number",
                    min: gov_range.map(|(min, _)| min.to_string()),
                    max: gov_range.map(|(_, max)| max.to_string()),
                    value: "{gov}",
                    oninput: move |e: Event<FormData>| gov.set(e.value()),
                }
            }
        }

        div { class: "imaging-row",
            span { class: "imaging-label", {i18n::t(locale, "ve_quality")} }
            input {
                class: "imaging-select",
                r#type: "number",
                step: "0.1",
                min: quality_range.map(|(min, _)| min.to_string()),
                max: quality_range.map(|(_, max)| max.to_string()),
                value: "{quality}",
                oninput: move |e: Event<FormData>| quality.set(e.value()),
            }
        }

        if has_profile {
            div { class: "imaging-row",
                span { class: "imaging-label", {i18n::t(locale, "ve_profile")} }
                select {
                    class: "imaging-select",
                    value: "{prof_cur}",
                    onchange: move |e: Event<FormData>| profile.set(e.value()),
                    for p in profiles.iter().cloned() {
                        option { value: "{p}", selected: p == prof_cur, "{p}" }
                    }
                }
            }
        }

        div { class: "imaging-footer",
            button {
                class: "btn btn-md btn-primary",
                disabled: *saving.read() || !fps_valid,
                onclick: move |_| {
                    let mut saving = saving;
                    if *saving.read() || !fps_valid { return; }
                    saving.set(true);
                    let addr_s = addr.read().clone();
                    let creds_s = creds.read().clone();
                    let mut settings = settings_for_save.clone();
                    let resolution_v = resolution.read().clone();
                    let frame_rate_v = frame_rate.read().clone();
                    let bitrate_v = bitrate.read().clone();
                    let gov_v = gov.read().clone();
                    let quality_v = quality.read().clone();
                    let profile_v = profile.read().clone();
                    let on_saved = on_saved;

                    spawn(async move {
                        let fps = settings.parse_frame_rate(&frame_rate_v);
                        if settings.config.rate_control.is_some() && fps.is_none() {
                            ctx.push_toast(ToastLevel::Error, i18n::t(locale, "ve_invalid_fps"));
                            saving.set(false);
                            return;
                        }
                        let cfg = &mut settings.config;
                        if let Some((w, h)) = parse_resolution(&resolution_v) {
                            cfg.resolution.width = w;
                            cfg.resolution.height = h;
                        }
                        if let Ok(q) = quality_v.parse::<f32>() {
                            cfg.quality = q;
                        }
                        if let Some(rc) = cfg.rate_control.as_mut() {
                            if let Some(f) = fps {
                                rc.frame_rate_limit = f;
                            }
                            if let Ok(b) = bitrate_v.parse::<u32>() {
                                rc.bitrate_limit = b;
                            }
                        }
                        if cfg.gov_length.is_some() {
                            if let Ok(g) = gov_v.parse::<u32>() { cfg.gov_length = Some(g); }
                        }
                        if cfg.profile.is_some() && !profile_v.is_empty() { cfg.profile = Some(profile_v); }

                        match api::save_encoder_settings(&addr_s, &creds_s, &settings).await {
                            Ok(()) => {
                                ctx.push_toast(ToastLevel::Success, i18n::t(locale, "ve_saved"));
                                on_saved.call(());
                            }
                            Err(e) => ctx.push_toast(ToastLevel::Error, e),
                        }
                        saving.set(false);
                    });
                },
                {i18n::t(locale, "ve_save")}
            }
        }
    }
}
