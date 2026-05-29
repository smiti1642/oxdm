#![allow(non_snake_case)]
use crate::components::TabError;
use crate::state::{Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;
use oxvif::{VideoEncoderConfiguration, VideoEncoderConfigurationOptions, VideoEncoding};

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
        async move {
            let profiles = api::get_profiles(&addr, &creds).await?;
            // Prefer the selected profile's encoder; fall back to the
            // first profile that has one (mirrors get_video_source_token)
            // so the section isn't blank when selected_profile is stale.
            let token = profile
                .and_then(|p| {
                    profiles
                        .iter()
                        .find(|x| x.token == p)
                        .and_then(|x| x.video_encoder_token.clone())
                })
                .or_else(|| profiles.iter().find_map(|x| x.video_encoder_token.clone()))
                .ok_or_else(|| "no_encoder".to_string())?;
            let cfg = api::get_video_encoder_configuration(&addr, &creds, &token).await?;
            let opts = api::get_video_encoder_configuration_options(&addr, &creds, Some(&token))
                .await
                .ok();
            Ok::<
                (
                    VideoEncoderConfiguration,
                    Option<VideoEncoderConfigurationOptions>,
                ),
                String,
            >((cfg, opts))
        }
    });

    rsx! {
        div { class: "prop-section-header", {i18n::t(locale, "nav_video_encoder")} }
        match &*data.read_unchecked() {
            None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
            Some(Err(e)) if e == "no_encoder" => rsx! {
                div { class: "tab-empty", {i18n::t(locale, "ve_no_encoder")} }
            },
            Some(Err(e)) => rsx! {
                TabError { error: e.clone(), on_retry: move |_| data.restart() }
            },
            Some(Ok((cfg, opts))) => {
                // Content-based key: a successful Apply refetches, and if
                // the fetched values differ the key changes and the form
                // remounts/reseeds. So when a camera silently ignores a
                // change, the form reverts to the camera's real value and
                // the user can see the edit didn't take.
                let fps = cfg.rate_control.as_ref().map(|r| r.frame_rate_limit).unwrap_or(0);
                let kbps = cfg.rate_control.as_ref().map(|r| r.bitrate_limit).unwrap_or(0);
                let key = format!(
                    "{}|{}x{}|{}|{}|{}",
                    cfg.token, cfg.resolution.width, cfg.resolution.height, fps, kbps, cfg.quality
                );
                rsx! {
                    VideoEncoderForm {
                        key: "{key}",
                        addr,
                        creds,
                        config: cfg.clone(),
                        options: opts.clone(),
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
    config: VideoEncoderConfiguration,
    options: Option<VideoEncoderConfigurationOptions>,
    on_saved: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    // Per-encoding option subset, flattened into plain locals so the rsx
    // below doesn't branch on the differently-typed h264/h265/jpeg structs.
    let res_options: Vec<String> = match (&config.encoding, options.as_ref()) {
        (VideoEncoding::H264, Some(o)) => o
            .h264
            .as_ref()
            .map(|h| h.resolutions.iter().map(|r| r.to_string()).collect())
            .unwrap_or_default(),
        (VideoEncoding::H265, Some(o)) => o
            .h265
            .as_ref()
            .map(|h| h.resolutions.iter().map(|r| r.to_string()).collect())
            .unwrap_or_default(),
        (VideoEncoding::Jpeg, Some(o)) => o
            .jpeg
            .as_ref()
            .map(|j| j.resolutions.iter().map(|r| r.to_string()).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };
    let frame_rate_range: Option<(i32, i32)> = match (&config.encoding, options.as_ref()) {
        (VideoEncoding::H264, Some(o)) => o.h264.as_ref().and_then(|h| h.frame_rate_range),
        (VideoEncoding::H265, Some(o)) => o.h265.as_ref().and_then(|h| h.frame_rate_range),
        (VideoEncoding::Jpeg, Some(o)) => o.jpeg.as_ref().and_then(|j| j.frame_rate_range),
        _ => None,
    }
    .map(|r| (r.min, r.max));
    let bitrate_range: Option<(i32, i32)> = match (&config.encoding, options.as_ref()) {
        (VideoEncoding::H264, Some(o)) => o.h264.as_ref().and_then(|h| h.bitrate_range),
        (VideoEncoding::H265, Some(o)) => o.h265.as_ref().and_then(|h| h.bitrate_range),
        _ => None,
    }
    .map(|r| (r.min, r.max));
    let gov_range: Option<(i32, i32)> = match (&config.encoding, options.as_ref()) {
        (VideoEncoding::H264, Some(o)) => o.h264.as_ref().and_then(|h| h.gov_length_range),
        (VideoEncoding::H265, Some(o)) => o.h265.as_ref().and_then(|h| h.gov_length_range),
        _ => None,
    }
    .map(|r| (r.min, r.max));
    let profiles: Vec<String> = match (&config.encoding, options.as_ref()) {
        (VideoEncoding::H264, Some(o)) => o.h264.as_ref().map(|h| h.profiles.clone()),
        (VideoEncoding::H265, Some(o)) => o.h265.as_ref().map(|h| h.profiles.clone()),
        _ => None,
    }
    .unwrap_or_default();
    let quality_range: Option<(f32, f32)> = options
        .as_ref()
        .and_then(|o| o.quality_range)
        .map(|r| (r.min, r.max));

    let is_video_codec = matches!(config.encoding, VideoEncoding::H264 | VideoEncoding::H265);
    let has_rate_control = config.rate_control.is_some();
    let has_gov = match config.encoding {
        VideoEncoding::H264 => config.h264.is_some(),
        VideoEncoding::H265 => config.h265.is_some(),
        _ => false,
    };
    let codec_profile = match config.encoding {
        VideoEncoding::H264 => config.h264.as_ref().map(|h| h.profile.clone()),
        VideoEncoding::H265 => config.h265.as_ref().map(|h| h.profile.clone()),
        _ => None,
    };
    let has_profile = codec_profile.is_some() && !profiles.is_empty();

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
    let mut gov = use_signal(|| match config.encoding {
        VideoEncoding::H264 => config
            .h264
            .as_ref()
            .map(|h| h.gov_length.to_string())
            .unwrap_or_default(),
        VideoEncoding::H265 => config
            .h265
            .as_ref()
            .map(|h| h.gov_length.to_string())
            .unwrap_or_default(),
        _ => String::new(),
    });
    let mut quality = use_signal(|| config.quality.to_string());
    let mut profile = use_signal(|| codec_profile.clone().unwrap_or_default());
    let saving = use_signal(|| false);

    let res_cur = resolution.read().clone();
    let prof_cur = profile.read().clone();
    let config_for_save = config.clone();

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
                input {
                    class: "imaging-select",
                    r#type: "number",
                    min: frame_rate_range.map(|(min, _)| min.to_string()),
                    max: frame_rate_range.map(|(_, max)| max.to_string()),
                    value: "{frame_rate}",
                    oninput: move |e: Event<FormData>| frame_rate.set(e.value()),
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
                disabled: *saving.read(),
                onclick: move |_| {
                    let mut saving = saving;
                    if *saving.read() { return; }
                    saving.set(true);
                    let addr_s = addr.read().clone();
                    let creds_s = creds.read().clone();
                    let mut cfg = config_for_save.clone();
                    let resolution_v = resolution.read().clone();
                    let frame_rate_v = frame_rate.read().clone();
                    let bitrate_v = bitrate.read().clone();
                    let gov_v = gov.read().clone();
                    let quality_v = quality.read().clone();
                    let profile_v = profile.read().clone();
                    let on_saved = on_saved;

                    spawn(async move {
                        if let Some((w, h)) = parse_resolution(&resolution_v) {
                            cfg.resolution.width = w;
                            cfg.resolution.height = h;
                        }
                        if let Ok(q) = quality_v.parse::<f32>() {
                            cfg.quality = q;
                        }
                        if let Some(rc) = cfg.rate_control.as_mut() {
                            if let Ok(f) = frame_rate_v.parse::<u32>() {
                                rc.frame_rate_limit = f;
                            }
                            if let Ok(b) = bitrate_v.parse::<u32>() {
                                rc.bitrate_limit = b;
                            }
                        }
                        match cfg.encoding {
                            VideoEncoding::H264 => {
                                if let Some(h) = cfg.h264.as_mut() {
                                    if let Ok(g) = gov_v.parse::<u32>() {
                                        h.gov_length = g;
                                    }
                                    if !profile_v.is_empty() {
                                        h.profile = profile_v;
                                    }
                                }
                            }
                            VideoEncoding::H265 => {
                                if let Some(h) = cfg.h265.as_mut() {
                                    if let Ok(g) = gov_v.parse::<u32>() {
                                        h.gov_length = g;
                                    }
                                    if !profile_v.is_empty() {
                                        h.profile = profile_v;
                                    }
                                }
                            }
                            _ => {}
                        }

                        match api::set_video_encoder_configuration(&addr_s, &creds_s, &cfg).await {
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
