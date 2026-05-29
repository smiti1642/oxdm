#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{Credentials, Ctx, ToastLevel};
use crate::views::live_video::{LiveH265Tip, LiveModeTabs, LiveVideoMode, LiveVideoStage};
use crate::views::video_encoder::VideoEncoderSection;
use crate::{api, i18n};
use dioxus::prelude::*;

#[component]
pub fn ImagingView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let profile_token = ctx.selected_profile.read().clone();

    // Fetch settings + options together
    let mut data = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let profile = profile_token.clone();
        async move {
            let source_token =
                api::get_video_source_token(&addr, &creds, profile.as_deref()).await?;
            let settings = api::get_imaging_settings(&addr, &creds, &source_token).await?;
            let options = api::get_imaging_options(&addr, &creds, &source_token).await?;
            Ok::<_, String>((source_token, settings, options))
        }
    });

    // Local state for controls — initialized from fetched data
    let brightness = use_signal(|| 50.0f32);
    let contrast = use_signal(|| 50.0f32);
    let saturation = use_signal(|| 50.0f32);
    let sharpness = use_signal(|| 50.0f32);
    let wdr_level = use_signal(|| 50.0f32);
    let exposure_mode = use_signal(|| "AUTO".to_string());
    let wb_mode = use_signal(|| "AUTO".to_string());
    let blc_mode = use_signal(|| "OFF".to_string());
    let wdr_mode = use_signal(|| "OFF".to_string());
    let ir_mode = use_signal(|| "AUTO".to_string());
    let focus_mode = use_signal(|| "AUTO".to_string());
    let initialized = use_signal(|| false);

    // Per-view backend choice — same Snapshot/RTSP toggle as Live Video,
    // independent state so a user who's running RTSP in Imaging can
    // still keep PTZ on Snapshot, etc.
    let preview_mode = use_signal(|| LiveVideoMode::Snapshot);
    let preview_backend_id = use_memo(move || preview_mode.read().backend_id());

    rsx! {
        div { class: "imaging-view",
            div { class: "content-header",
                Icon { name: "sliders", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_imaging")} }
                LiveModeTabs { mode: preview_mode }
            }
            LiveH265Tip { mode: preview_mode }
            // Live preview at the top so adjustments are visible without
            // jumping back to the LiveVideo view. Reuses the same backend
            // pipeline; the snapshot loop refreshes ~5 fps, so the user sees
            // the camera's response within a second of pressing Apply.
            div { class: "imaging-preview",
                LiveVideoStage {
                    addr,
                    creds,
                    backend_id: Some(preview_backend_id.into()),
                }
            }
            div { class: "imaging-body",
                match &*data.read_unchecked() {
                    None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
                    Some(Err(e)) => rsx! {
                        crate::components::TabError {
                            error: e.clone(),
                            on_retry: move |_| data.restart(),
                        }
                    },
                    Some(Ok((source_token, settings, options))) => {
                        // Init local signals from fetched data (once)
                        if !*initialized.peek() {
                            brightness.clone().set(settings.brightness.unwrap_or(50.0));
                            contrast.clone().set(settings.contrast.unwrap_or(50.0));
                            saturation.clone().set(settings.color_saturation.unwrap_or(50.0));
                            sharpness.clone().set(settings.sharpness.unwrap_or(50.0));
                            wdr_level.clone().set(settings.wide_dynamic_range_level.unwrap_or(50.0));
                            exposure_mode.clone().set(settings.exposure_mode.clone().unwrap_or("AUTO".into()));
                            wb_mode.clone().set(settings.white_balance_mode.clone().unwrap_or("AUTO".into()));
                            blc_mode.clone().set(settings.backlight_compensation.clone().unwrap_or("OFF".into()));
                            wdr_mode.clone().set(settings.wide_dynamic_range_mode.clone().unwrap_or("OFF".into()));
                            ir_mode.clone().set(settings.ir_cut_filter.clone().unwrap_or("AUTO".into()));
                            focus_mode.clone().set(settings.focus_mode.clone().unwrap_or("AUTO".into()));
                            initialized.clone().set(true);
                        }

                        let br = options.brightness.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let ct = options.contrast.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let sa = options.color_saturation.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let sh = options.sharpness.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let wr = options.wdr_level_range.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });

                        let token = source_token.clone();

                        rsx! {
                            div { class: "prop-section-header", {i18n::t(locale, "img_basic")} }
                            SliderRow { label: i18n::t(locale, "img_brightness"), value: brightness, min: br.min, max: br.max }
                            SliderRow { label: i18n::t(locale, "img_contrast"),    value: contrast,   min: ct.min, max: ct.max }
                            SliderRow { label: i18n::t(locale, "img_saturation"),  value: saturation, min: sa.min, max: sa.max }
                            SliderRow { label: i18n::t(locale, "img_sharpness"),   value: sharpness,  min: sh.min, max: sh.max }

                            div { class: "prop-section-header", {i18n::t(locale, "img_exposure")} }
                            SelectRow { label: i18n::t(locale, "img_mode"), value: exposure_mode,
                                options_list: nonempty(&options.exposure_modes, &["AUTO", "MANUAL"]) }

                            div { class: "prop-section-header", {i18n::t(locale, "img_white_balance")} }
                            SelectRow { label: i18n::t(locale, "img_mode"), value: wb_mode,
                                options_list: nonempty(&options.white_balance_modes, &["AUTO", "MANUAL"]) }

                            div { class: "prop-section-header", {i18n::t(locale, "img_backlight")} }
                            SelectRow { label: i18n::t(locale, "img_mode"), value: blc_mode,
                                options_list: nonempty(&options.backlight_compensation_modes, &["OFF", "ON"]) }

                            div { class: "prop-section-header", {i18n::t(locale, "img_wdr")} }
                            SelectRow { label: i18n::t(locale, "img_mode"), value: wdr_mode,
                                options_list: nonempty(&options.wdr_modes, &["OFF", "ON"]) }
                            SliderRow { label: i18n::t(locale, "img_level"), value: wdr_level, min: wr.min, max: wr.max }

                            div { class: "prop-section-header", {i18n::t(locale, "img_ir_cut")} }
                            SelectRow { label: i18n::t(locale, "img_mode"), value: ir_mode,
                                options_list: nonempty(&options.ir_cut_filter_modes, &["ON", "OFF", "AUTO"]) }

                            div { class: "prop-section-header", {i18n::t(locale, "img_focus")} }
                            SelectRow { label: i18n::t(locale, "img_mode"), value: focus_mode,
                                options_list: nonempty(&options.focus_af_modes, &["AUTO", "MANUAL"]) }

                            div { class: "imaging-footer",
                                button {
                                    class: "btn btn-md btn-primary",
                                    onclick: move |_| {
                                        let addr = addr.read().clone();
                                        let creds = creds.read().clone();
                                        let tk = token.clone();
                                        let new_settings = oxvif::ImagingSettings {
                                            brightness: Some(*brightness.peek()),
                                            color_saturation: Some(*saturation.peek()),
                                            contrast: Some(*contrast.peek()),
                                            sharpness: Some(*sharpness.peek()),
                                            ir_cut_filter: Some(ir_mode.peek().clone()),
                                            white_balance_mode: Some(wb_mode.peek().clone()),
                                            exposure_mode: Some(exposure_mode.peek().clone()),
                                            backlight_compensation: Some(blc_mode.peek().clone()),
                                            focus_mode: Some(focus_mode.peek().clone()),
                                            wide_dynamic_range_mode: Some(wdr_mode.peek().clone()),
                                            wide_dynamic_range_level: Some(*wdr_level.peek()),
                                            ..Default::default()
                                        };
                                        spawn(async move {
                                            match api::set_imaging_settings(&addr, &creds, &tk, &new_settings).await {
                                                Ok(()) => ctx.push_toast(ToastLevel::Success, i18n::t(locale, "img_saved")),
                                                Err(e) => ctx.push_toast(ToastLevel::Error, e),
                                            }
                                        });
                                    },
                                    Icon { name: "check", size: 14 }
                                    " "
                                    {i18n::t(locale, "btn_apply")}
                                }
                            }
                        }
                    },
                }
                VideoEncoderSection { addr, creds }
            }
        }
    }
}

fn nonempty(vec: &[String], defaults: &[&str]) -> Vec<String> {
    if vec.is_empty() {
        defaults.iter().map(|s| s.to_string()).collect()
    } else {
        vec.to_vec()
    }
}

#[component]
fn SliderRow(label: &'static str, value: Signal<f32>, min: f32, max: f32) -> Element {
    let display = format!("{:.0}", *value.read());
    rsx! {
        div { class: "imaging-row",
            span { class: "imaging-label", "{label}" }
            input {
                class: "imaging-slider",
                r#type: "range",
                min: "{min}",
                max: "{max}",
                step: "1",
                value: "{display}",
                oninput: move |e| {
                    if let Ok(v) = e.value().parse::<f32>() {
                        value.clone().set(v);
                    }
                },
            }
            span { class: "imaging-value", "{display}" }
        }
    }
}

#[component]
fn SelectRow(label: &'static str, value: Signal<String>, options_list: Vec<String>) -> Element {
    let current = value.read().clone();
    rsx! {
        div { class: "imaging-row",
            span { class: "imaging-label", "{label}" }
            select {
                class: "imaging-select",
                value: "{current}",
                onchange: move |e| value.clone().set(e.value()),
                for opt in options_list {
                    option {
                        value: "{opt}",
                        selected: opt == current,
                        "{opt}"
                    }
                }
            }
        }
    }
}
