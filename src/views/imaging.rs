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
    // oxvif 0.9.8 — manual exposure / WB gains / focus limits. Each is
    // Option<f32> in the wire model; we expose them as f32 with a
    // present-bit so the caller can opt out.
    let exposure_priority = use_signal(|| "FrameRate".to_string());
    let exposure_time = use_signal(|| 0.0f32);
    let exposure_gain = use_signal(|| 0.0f32);
    let exposure_iris = use_signal(|| 0.0f32);
    let wb_cr_gain = use_signal(|| 0.0f32);
    let wb_cb_gain = use_signal(|| 0.0f32);
    let focus_near_limit = use_signal(|| 0.0f32);
    let focus_far_limit = use_signal(|| 0.0f32);
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
                            // oxvif 0.9.8 manual fields — default to 0.0 if unread.
                            exposure_priority.clone().set(settings.exposure_priority.clone().unwrap_or("FrameRate".into()));
                            exposure_time.clone().set(settings.exposure_time.unwrap_or(0.0));
                            exposure_gain.clone().set(settings.exposure_gain.unwrap_or(0.0));
                            exposure_iris.clone().set(settings.exposure_iris.unwrap_or(0.0));
                            wb_cr_gain.clone().set(settings.wb_cr_gain.unwrap_or(0.0));
                            wb_cb_gain.clone().set(settings.wb_cb_gain.unwrap_or(0.0));
                            focus_near_limit.clone().set(settings.focus_near_limit.unwrap_or(0.0));
                            focus_far_limit.clone().set(settings.focus_far_limit.unwrap_or(0.0));
                            initialized.clone().set(true);
                        }

                        let br = options.brightness.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let ct = options.contrast.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let sa = options.color_saturation.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let sh = options.sharpness.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let wr = options.wdr_level_range.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        // Manual-exposure ranges from oxvif 0.9.8 GetOptions.
                        let exp_t = options.exposure_time_range.unwrap_or(oxvif::FloatRange { min: 0.0, max: 1.0 });
                        let exp_g = options.gain_range.unwrap_or(oxvif::FloatRange { min: 0.0, max: 100.0 });
                        let exp_i = options.iris_range.unwrap_or(oxvif::FloatRange { min: 0.0, max: 22.0 });

                        let exposure_is_manual = exposure_mode.read().eq_ignore_ascii_case("MANUAL");
                        let wb_is_manual = wb_mode.read().eq_ignore_ascii_case("MANUAL");

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
                            FloatInputRow { label: i18n::t(locale, "img_focus_near"), value: focus_near_limit, step: 1.0 }
                            FloatInputRow { label: i18n::t(locale, "img_focus_far"), value: focus_far_limit, step: 1.0 }

                            // Manual exposure / WB gains — visible always so the
                            // user can prepare values before switching mode.
                            div { class: "prop-section-header", {i18n::t(locale, "img_manual_group")} }
                            SelectRow { label: i18n::t(locale, "img_exposure_priority"),
                                value: exposure_priority,
                                options_list: vec!["FrameRate".into(), "LowNoise".into()] }
                            if exposure_is_manual {
                                FloatInputRow { label: i18n::t(locale, "img_exposure_time"),
                                    value: exposure_time, step: 0.0001 }
                                FloatInputRow { label: i18n::t(locale, "img_exposure_gain"),
                                    value: exposure_gain, step: 0.1 }
                                FloatInputRow { label: i18n::t(locale, "img_exposure_iris"),
                                    value: exposure_iris, step: 0.1 }
                                ManualRangeNote { min_label: "exposure_time", lo: exp_t.min, hi: exp_t.max }
                                ManualRangeNote { min_label: "gain", lo: exp_g.min, hi: exp_g.max }
                                ManualRangeNote { min_label: "iris", lo: exp_i.min, hi: exp_i.max }
                            }
                            if wb_is_manual {
                                FloatInputRow { label: i18n::t(locale, "img_wb_cr_gain"),
                                    value: wb_cr_gain, step: 0.01 }
                                FloatInputRow { label: i18n::t(locale, "img_wb_cb_gain"),
                                    value: wb_cb_gain, step: 0.01 }
                            }

                            div { class: "imaging-footer",
                                button {
                                    class: "btn btn-md btn-primary",
                                    onclick: move |_| {
                                        let addr = addr.read().clone();
                                        let creds = creds.read().clone();
                                        let tk = token.clone();
                                        // Only send manual-exposure / manual-WB sub-fields when
                                        // the parent mode is MANUAL — sending them in AUTO mode
                                        // is at best ignored, at worst rejected by strict devices.
                                        let exp_manual = exposure_mode.peek().eq_ignore_ascii_case("MANUAL");
                                        let wb_manual = wb_mode.peek().eq_ignore_ascii_case("MANUAL");
                                        let near = *focus_near_limit.peek();
                                        let far = *focus_far_limit.peek();
                                        let new_settings = oxvif::ImagingSettings {
                                            brightness: Some(*brightness.peek()),
                                            color_saturation: Some(*saturation.peek()),
                                            contrast: Some(*contrast.peek()),
                                            sharpness: Some(*sharpness.peek()),
                                            ir_cut_filter: Some(ir_mode.peek().clone()),
                                            white_balance_mode: Some(wb_mode.peek().clone()),
                                            wb_cr_gain: if wb_manual { Some(*wb_cr_gain.peek()) } else { None },
                                            wb_cb_gain: if wb_manual { Some(*wb_cb_gain.peek()) } else { None },
                                            exposure_mode: Some(exposure_mode.peek().clone()),
                                            exposure_priority: Some(exposure_priority.peek().clone()),
                                            exposure_time: if exp_manual { Some(*exposure_time.peek()) } else { None },
                                            exposure_gain: if exp_manual { Some(*exposure_gain.peek()) } else { None },
                                            exposure_iris: if exp_manual { Some(*exposure_iris.peek()) } else { None },
                                            backlight_compensation: Some(blc_mode.peek().clone()),
                                            focus_mode: Some(focus_mode.peek().clone()),
                                            focus_near_limit: if near > 0.0 { Some(near) } else { None },
                                            focus_far_limit: if far > 0.0 { Some(far) } else { None },
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

/// Free-form numeric input for fields where a slider's coarse step isn't
/// adequate (exposure time, WB gain, focus limits). Stores into a
/// `Signal<f32>` so the Apply path reads it uniformly.
#[component]
fn FloatInputRow(label: &'static str, value: Signal<f32>, step: f32) -> Element {
    let display = format!("{:.4}", *value.read());
    rsx! {
        div { class: "imaging-row",
            span { class: "imaging-label", "{label}" }
            input {
                class: "imaging-input",
                r#type: "number",
                step: "{step}",
                value: "{display}",
                oninput: move |e| {
                    if let Ok(v) = e.value().parse::<f32>() {
                        value.clone().set(v);
                    }
                },
            }
        }
    }
}

/// Small inline annotation showing the device-advertised valid range for
/// a manual-exposure field. Helps the user pick a value the camera will
/// actually accept (and saves a SOAP Fault round-trip).
#[component]
fn ManualRangeNote(min_label: &'static str, lo: f32, hi: f32) -> Element {
    rsx! {
        div { class: "imaging-row imaging-range-note",
            span { class: "imaging-label", "{min_label}" }
            span { class: "imaging-value", "{lo}..{hi}" }
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
