#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{Credentials, Ctx, ToastLevel};
use crate::views::live_video::{LiveH265Tip, LiveModeTabs, LiveVideoMode, LiveVideoStage};
use crate::{api, i18n};
use dioxus::prelude::*;

/// PTZ control panel.
///
/// Layout mirrors `ImagingView`: live preview on top, controls below.
/// The controls split into a directional pad + zoom column on the left and
/// a preset list on the right. Preset list scrolls when overflowing.
#[component]
pub fn PtzControlView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let profile_sig = ctx.selected_profile;

    let speed = use_signal(|| 0.5_f32);
    let preset_search = use_signal(String::new);

    // Per-view backend choice — same Snapshot/RTSP toggle as Live Video.
    // Independent state from the other views so each tab remembers its
    // own preference for the current session.
    let preview_mode = use_signal(|| LiveVideoMode::Snapshot);
    let preview_backend_id = use_memo(move || preview_mode.read().backend_id());

    // Feature-detect PTZ on this camera. Just a capabilities probe —
    // the underlying `OnvifSession` caches the GetCapabilities response,
    // so this is one round-trip per (addr, creds) for the lifetime of
    // the process and free after that. Drives the "PTZ unavailable"
    // empty-state below.
    let ptz_state = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        async move {
            if addr_s.is_empty() {
                return Err("no_device".to_string());
            }
            match api::has_ptz_service(&addr_s, &creds_s).await {
                Ok(true) => Ok(()),
                Ok(false) => Err("ptz_unavailable".to_string()),
                Err(e) => Err(e),
            }
        }
    });

    // Re-fetch presets when profile changes. Joystick latency used to
    // require pre-caching the PTZ URL; the session pool makes that
    // unnecessary — every `api::ptz_*` call hits a cached session and
    // costs exactly one SOAP round-trip.
    let mut presets_state = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let token_opt = profile_sig.read().clone();
        async move {
            let token = token_opt.ok_or_else(|| "no_profile".to_string())?;
            api::ptz_get_presets(&addr_s, &creds_s, &token).await
        }
    });

    // Resolve the selected profile's video_source_token. Focus motor
    // control lives on the Imaging service (separate from PTZ) and
    // addresses the camera by source token, not profile token.
    let focus_state = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let token_opt = profile_sig.read().clone();
        async move {
            if addr_s.is_empty() {
                return Err("no_device".to_string());
            }
            api::get_video_source_token(&addr_s, &creds_s, token_opt.as_deref()).await
        }
    });

    // What the lens says it will accept, before anything is sent to the motor.
    // Keyed off the resolved source token so a device or profile change
    // re-asks — the two lenses of a dual-sensor camera need not agree.
    let move_opts = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let source = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => Some(t.token.clone()),
            _ => None,
        };
        async move {
            let source = source.ok_or_else(|| "no_source".to_string())?;
            api::imaging_get_move_options(&addr_s, &creds_s, &source).await
        }
    });

    // Focus position readout. Refreshed after every stop rather than polled:
    // a moving lens is already visible in the preview, and a 3s poll against
    // every camera in the list is a cost the answer does not justify.
    let mut focus_status = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let source = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => Some(t.token.clone()),
            _ => None,
        };
        async move {
            let source = source.ok_or_else(|| "no_source".to_string())?;
            api::imaging_get_status(&addr_s, &creds_s, &source).await
        }
    });

    // The head behind the *selected profile*. Resolved profile → PTZ
    // configuration → node, with no fallback: on a multi-head device answering
    // with another head's limits would drive head 1's range into head 2.
    let node_state = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let token_opt = profile_sig.read().clone();
        async move {
            let token = token_opt.ok_or_else(|| "no_profile".to_string())?;
            api::ptz_node_for_profile(&addr_s, &creds_s, &token).await
        }
    });

    // Where the head is now. Read on profile change and after every absolute
    // move; the joystick paths do not restart it, because a status round-trip
    // per mousedown is exactly the latency the session cache exists to avoid.
    let mut ptz_status = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let token_opt = profile_sig.read().clone();
        async move {
            let token = token_opt.ok_or_else(|| "no_profile".to_string())?;
            api::ptz_get_status(&addr_s, &creds_s, &token).await
        }
    });

    // Commanded absolute position. Seeded once from the head's real position so
    // "Go" does not fling a camera to 0,0 the first time it is pressed.
    let mut abs_pan = use_signal(|| 0.0_f32);
    let mut abs_tilt = use_signal(|| 0.0_f32);
    let mut abs_zoom = use_signal(|| 0.0_f32);
    let mut abs_seeded = use_signal(|| false);

    // ── Action callbacks ───────────────────────────────────────────────────
    // Wrapped with `use_callback` so they're `Copy` and can be passed as
    // props to child components (DirButton, ZoomButton, PresetRow). Each
    // re-reads `profile_sig` at invocation — stale cached values would
    // silently fire moves at the wrong device when the user switches
    // profile while holding a button.
    let do_move = use_callback(move |args: (f32, f32, f32)| {
        let (pan, tilt, zoom) = args;
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            if let Err(e) =
                api::ptz_continuous_move(&addr_s, &creds_s, &token, pan, tilt, zoom).await
            {
                tracing::warn!(error = %e, "PTZ continuous_move failed");
            }
        });
    });

    let do_stop = use_callback(move |_: ()| {
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            if let Err(e) = api::ptz_stop(&addr_s, &creds_s, &token).await {
                tracing::warn!(error = %e, "PTZ stop failed");
            }
        });
    });

    let goto_home = use_callback(move |_: ()| {
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            match api::ptz_goto_home_position(&addr_s, &creds_s, &token).await {
                Ok(()) => ctx.push_toast(ToastLevel::Info, i18n::t(locale, "ptz_home_ok")),
                Err(e) => ctx.push_toast(ToastLevel::Error, e),
            }
        });
    });

    // Current focus mode (AUTO / MANUAL) — read from ImagingSettings so the
    // toggle highlights match the camera's actual state. Refreshed on
    // device/profile change and after every user toggle (via .restart()).
    let mut focus_mode_state = use_resource(move || {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let token_opt = profile_sig.read().clone();
        async move {
            if addr_s.is_empty() {
                return Err::<String, String>("no_device".to_string());
            }
            let source_token = api::get_video_source_token(&addr_s, &creds_s, token_opt.as_deref())
                .await?
                .token;
            let settings = api::get_imaging_settings(&addr_s, &creds_s, &source_token).await?;
            Ok(settings.focus_mode.unwrap_or_else(|| "AUTO".to_string()))
        }
    });

    // Toggle Focus.AutoFocusMode by GET-modify-SET to avoid clobbering
    // unrelated ImagingSettings fields. Restarts focus_mode_state so the
    // segmented control reflects the new value within one round-trip.
    let set_focus_mode_cb = use_callback(move |auto: bool| {
        let source_token = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => t.token.clone(),
            _ => return,
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            let mut settings =
                match api::get_imaging_settings(&addr_s, &creds_s, &source_token).await {
                    Ok(s) => s,
                    Err(e) => {
                        tracing::warn!(error = %e, "get_imaging_settings failed");
                        return;
                    }
                };
            settings.focus_mode = Some(if auto { "AUTO".into() } else { "MANUAL".into() });
            match api::set_imaging_settings(&addr_s, &creds_s, &source_token, &settings).await {
                Ok(()) => focus_mode_state.restart(),
                Err(e) => {
                    ctx.push_toast(ToastLevel::Error, e);
                }
            }
        });
    });

    // Focus motor — speed sign carries direction (+ far / − near), so
    // FocusButton encodes the dir at construction time the same way ZoomButton does.
    let focus_move = use_callback(move |speed: f32| {
        let source_token = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => t.token.clone(),
            _ => return,
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            if let Err(e) =
                api::imaging_focus_continuous(&addr_s, &creds_s, &source_token, speed).await
            {
                tracing::warn!(error = %e, "focus continuous failed");
            }
        });
    });

    let focus_stop = use_callback(move |_: ()| {
        let source_token = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => t.token.clone(),
            _ => return,
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            if let Err(e) = api::imaging_focus_stop(&addr_s, &creds_s, &source_token).await {
                tracing::warn!(error = %e, "focus stop failed");
            }
            // The motor has settled — re-read where it ended up.
            focus_status.restart();
        });
    });

    // Save current camera position as a new preset with the given name.
    // On success, clears the input and re-fetches the preset list so the
    // new entry appears.
    let mut new_preset_name = use_signal(String::new);
    let save_preset = use_callback(move |name: String| {
        if name.trim().is_empty() {
            return;
        }
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            match api::ptz_set_preset(&addr_s, &creds_s, &token, Some(name.trim()), None).await {
                Ok(_) => {
                    new_preset_name.set(String::new());
                    presets_state.restart();
                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "ptz_preset_saved"));
                }
                Err(e) => ctx.push_toast(ToastLevel::Error, e),
            }
        });
    });

    let remove_preset = use_callback(move |preset_token: String| {
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            match api::ptz_remove_preset(&addr_s, &creds_s, &token, &preset_token).await {
                Ok(()) => {
                    presets_state.restart();
                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "ptz_preset_removed"));
                }
                Err(e) => ctx.push_toast(ToastLevel::Error, e),
            }
        });
    });

    let goto_preset = use_callback(move |preset_token: String| {
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            if let Err(e) = api::ptz_goto_preset(&addr_s, &creds_s, &token, &preset_token).await {
                ctx.push_toast(ToastLevel::Error, e);
            }
        });
    });

    // Resolve each focus direction against what the lens declared.
    //
    // The slider beside these buttons is a 0.1–1.0 fraction shared with
    // pan/tilt/zoom; it has never had any relationship to the focus speed range
    // the device publishes in `GetMoveOptions`. Each direction is resolved
    // separately because the sign of the speed *is* the direction on the wire,
    // so a range like `0.0..=1.0` offers "farther" and no way to say "nearer".
    let (focus_far, focus_near) = {
        let slider = *speed.read();
        match &*move_opts.read_unchecked() {
            // The device answered. `continuous_speed_range` is `None` only when
            // it omitted the Continuous family entirely — `Speed` is that
            // family's sole required member — so this is a real denial, and
            // both buttons go dead rather than sending a speed it never offered.
            Some(Ok(o)) => match o.continuous_speed_range {
                Some(r) => (
                    api::focus_speed(r, slider, 1.0),
                    api::focus_speed(r, slider, -1.0),
                ),
                None => (None, None),
            },
            // Could not ask, or still asking. Silence is not a denial: drive
            // the motor the way OxDM always has rather than disabling a control
            // on no evidence.
            _ => (Some(slider), Some(-slider)),
        }
    };

    let focus_position = match &*focus_status.read_unchecked() {
        Some(Ok(s)) => s.focus_position,
        _ => None,
    };

    // What this head declares it can be driven to, absolutely. `None` while the
    // node is still being read or could not be read — the controls are simply
    // not offered yet, rather than offered against a guessed range.
    let abs_limits = match &*node_state.read_unchecked() {
        Some(Ok(node)) => Some(api::ptz_absolute_limits(node)),
        _ => None,
    }
    .filter(|l: &api::PtzAbsoluteLimits| !l.is_empty());

    // Live readout, and the one-shot seed for the sliders.
    let (status_now, status_moving) = match &*ptz_status.read_unchecked() {
        Some(Ok(s)) => (
            Some((s.pan, s.tilt, s.zoom)),
            s.pan_tilt_status == "MOVING" || s.zoom_status == "MOVING",
        ),
        _ => (None, false),
    };
    let seeded = *abs_seeded.peek();
    if let (false, Some((p, t, z))) = (seeded, status_now) {
        if let Some(v) = p {
            abs_pan.set(v);
        }
        if let Some(v) = t {
            abs_tilt.set(v);
        }
        if let Some(v) = z {
            abs_zoom.set(v);
        }
        abs_seeded.set(true);
    }

    let go_absolute = use_callback(move |_: ()| {
        let Some(token) = profile_sig.read().clone() else {
            return;
        };
        let (pan, tilt, zoom) = (*abs_pan.read(), *abs_tilt.read(), *abs_zoom.read());
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            match api::ptz_absolute_move(&addr_s, &creds_s, &token, pan, tilt, zoom).await {
                // Re-read rather than trust the command: a device is free to
                // clamp, refuse an axis, or stop short, and the readout is the
                // only thing that shows it.
                Ok(()) => ptz_status.restart(),
                Err(e) => ctx.push_toast(ToastLevel::Error, e),
            }
        });
    });

    // ── Render ─────────────────────────────────────────────────────────────
    rsx! {
        div { class: "ptz-view",
            div { class: "content-header",
                Icon { name: "crosshair", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_ptz")} }
                LiveModeTabs { mode: preview_mode }
                if let Some(Err(e)) = &*ptz_state.read_unchecked() {
                    span { class: "ptz-status-error", " · {e}" }
                }
            }
            LiveH265Tip { mode: preview_mode }
            div { class: "imaging-preview",
                LiveVideoStage {
                    addr,
                    creds,
                    backend_id: Some(preview_backend_id.into()),
                }
            }

            div { class: "ptz-body",
                // ── Left: joystick + zoom + speed + home/stop ──
                div { class: "ptz-controls",
                    // Directional pad (3×3). Centre is "stop" for users
                    // whose mouseup somehow misfires.
                    div { class: "ptz-pad",
                        DirButton { pan: -1.0, tilt:  1.0, icon: "arrow-up-left",    do_move: do_move, do_stop: do_stop, speed }
                        DirButton { pan:  0.0, tilt:  1.0, icon: "arrow-up",          do_move: do_move, do_stop: do_stop, speed }
                        DirButton { pan:  1.0, tilt:  1.0, icon: "arrow-up-right",   do_move: do_move, do_stop: do_stop, speed }
                        DirButton { pan: -1.0, tilt:  0.0, icon: "arrow-left",        do_move: do_move, do_stop: do_stop, speed }
                        button {
                            class: "ptz-dir ptz-dir--center",
                            onclick: move |_| do_stop.call(()),
                            title: i18n::t(locale, "ptz_stop"),
                            Icon { name: "square", size: 16 }
                        }
                        DirButton { pan:  1.0, tilt:  0.0, icon: "arrow-right",       do_move: do_move, do_stop: do_stop, speed }
                        DirButton { pan: -1.0, tilt: -1.0, icon: "arrow-down-left",  do_move: do_move, do_stop: do_stop, speed }
                        DirButton { pan:  0.0, tilt: -1.0, icon: "arrow-down",        do_move: do_move, do_stop: do_stop, speed }
                        DirButton { pan:  1.0, tilt: -1.0, icon: "arrow-down-right", do_move: do_move, do_stop: do_stop, speed }
                    }

                    div { class: "ptz-side",
                        div { class: "ptz-zoom",
                            span { class: "ptz-side-label", {i18n::t(locale, "ptz_zoom")} }
                            ZoomButton { dir:  1.0, icon: "plus",  do_move: do_move, do_stop: do_stop, speed }
                            ZoomButton { dir: -1.0, icon: "minus", do_move: do_move, do_stop: do_stop, speed }
                        }
                        div { class: "ptz-focus",
                            span { class: "ptz-side-label", {i18n::t(locale, "ptz_focus")} }
                            // AUTO/MANUAL segmented toggle. Mirrors the
                            // Focus mode select in Imaging Settings tab so
                            // users can switch without leaving PTZ.
                            // Near/Far below have no effect while in AUTO.
                            {
                                let mode = match &*focus_mode_state.read_unchecked() {
                                    Some(Ok(m)) => m.clone(),
                                    _ => String::new(),
                                };
                                let auto_active = mode == "AUTO";
                                let manual_active = mode == "MANUAL";
                                let auto_class = if auto_active {
                                    "ptz-focus-mode-btn ptz-focus-mode-btn--active"
                                } else { "ptz-focus-mode-btn" };
                                let manual_class = if manual_active {
                                    "ptz-focus-mode-btn ptz-focus-mode-btn--active"
                                } else { "ptz-focus-mode-btn" };
                                rsx! {
                                    div { class: "ptz-focus-mode",
                                        button {
                                            class: "{auto_class}",
                                            onclick: move |_| set_focus_mode_cb.call(true),
                                            {i18n::t(locale, "ptz_focus_auto")}
                                        }
                                        button {
                                            class: "{manual_class}",
                                            onclick: move |_| set_focus_mode_cb.call(false),
                                            {i18n::t(locale, "ptz_focus_manual")}
                                        }
                                    }
                                }
                            }
                            FocusButton {
                                icon: "arrow-up",
                                label: i18n::t(locale, "ptz_focus_far"),
                                unsupported_label: i18n::t(locale, "ptz_focus_unsupported"),
                                focus_move: focus_move,
                                focus_stop: focus_stop,
                                speed: focus_far,
                            }
                            FocusButton {
                                icon: "arrow-down",
                                label: i18n::t(locale, "ptz_focus_near"),
                                unsupported_label: i18n::t(locale, "ptz_focus_unsupported"),
                                focus_move: focus_move,
                                focus_stop: focus_stop,
                                speed: focus_near,
                            }
                            if let Some(pos) = focus_position {
                                div { class: "ptz-focus-pos",
                                    {i18n::t(locale, "ptz_focus_position").replace("{pos}", &format!("{pos:.2}"))}
                                }
                            }
                        }
                        div { class: "ptz-speed",
                            span { class: "ptz-side-label", {i18n::t(locale, "ptz_speed")} }
                            input {
                                r#type: "range",
                                min: "0.1", max: "1.0", step: "0.05",
                                value: "{*speed.read()}",
                                oninput: move |e| {
                                    if let Ok(v) = e.value().parse::<f32>() {
                                        speed.clone().set(v);
                                    }
                                },
                            }
                            span { class: "ptz-speed-value", "{(*speed.read() * 100.0) as u32}%" }
                        }
                        // ── Where the head is, and where to send it ──
                        //
                        // Both halves are gated on the device having said
                        // something. The readout appears only once GetStatus
                        // answered; the sliders only once the node published an
                        // absolute position space for that axis. A head that
                        // cannot pan accepts an AbsoluteMove carrying a pan and
                        // does nothing observable with it, so offering the
                        // control blind is worse than not offering it.
                        if let Some((pan, tilt, zoom)) = status_now {
                            div { class: "ptz-status",
                                div { class: "ptz-side-label",
                                    {i18n::t(locale, "ptz_status")}
                                    if status_moving {
                                        span { class: "ptz-status-moving",
                                            {i18n::t(locale, "ptz_status_moving")}
                                        }
                                    }
                                }
                                div { class: "ptz-status-axes",
                                    PtzAxisReadout { label: "P", value: pan }
                                    PtzAxisReadout { label: "T", value: tilt }
                                    PtzAxisReadout { label: "Z", value: zoom }
                                }
                            }
                        }
                        if let Some(limits) = abs_limits {
                            div { class: "ptz-absolute",
                                span { class: "ptz-side-label", {i18n::t(locale, "ptz_absolute")} }
                                if let Some((min, max)) = limits.pan {
                                    AbsAxisSlider { label: "P", min, max, value: abs_pan }
                                }
                                if let Some((min, max)) = limits.tilt {
                                    AbsAxisSlider { label: "T", min, max, value: abs_tilt }
                                }
                                if let Some((min, max)) = limits.zoom {
                                    AbsAxisSlider { label: "Z", min, max, value: abs_zoom }
                                }
                                button {
                                    class: "btn btn-sm btn-primary",
                                    onclick: move |_| go_absolute.call(()),
                                    {i18n::t(locale, "ptz_absolute_go")}
                                }
                            }
                        }
                        div { class: "ptz-misc",
                            button {
                                class: "btn btn-md",
                                onclick: move |_| goto_home.call(()),
                                Icon { name: "home", size: 14 }
                                " "
                                {i18n::t(locale, "ptz_home")}
                            }
                        }
                    }
                }

                // ── Right: presets list + new-preset input ──
                div { class: "ptz-presets",
                    div { class: "ptz-presets-header",
                        Icon { name: "bookmark", size: 14 }
                        span { {i18n::t(locale, "ptz_presets")} }
                    }
                    input {
                        class: "ptz-preset-search",
                        r#type: "text",
                        placeholder: i18n::t(locale, "ptz_preset_search_placeholder"),
                        value: "{*preset_search.read()}",
                        oninput: {
                            let mut preset_search = preset_search;
                            move |evt: Event<FormData>| preset_search.set(evt.value())
                        },
                    }
                    match &*presets_state.read_unchecked() {
                        None => rsx! { div { class: "ptz-presets-empty", {i18n::t(locale, "loading")} } },
                        Some(Err(e)) if e == "no_profile" => rsx! {
                            div { class: "ptz-presets-empty", {i18n::t(locale, "live_video_no_profile")} }
                        },
                        Some(Err(e)) if e == "ptz_unavailable" => rsx! {
                            div { class: "ptz-presets-empty", {i18n::t(locale, "ptz_unavailable")} }
                        },
                        Some(Err(e)) => rsx! {
                            div { class: "ptz-presets-empty",
                                span { "{e}" }
                                button {
                                    class: "btn btn-sm btn-ghost tab-error-retry",
                                    onclick: move |_| presets_state.restart(),
                                    {i18n::t(locale, "btn_retry")}
                                }
                            }
                        },
                        Some(Ok(list)) if list.is_empty() => rsx! {
                            div { class: "ptz-presets-empty", {i18n::t(locale, "ptz_no_presets")} }
                        },
                        Some(Ok(list)) => {
                            let needle = preset_search.read().to_lowercase();
                            let filtered: Vec<_> = list
                                .iter()
                                .filter(|p| {
                                    needle.is_empty()
                                        || p.name.to_lowercase().contains(&needle)
                                        || p.token.to_lowercase().contains(&needle)
                                })
                                .cloned()
                                .collect();
                            if filtered.is_empty() {
                                rsx! {
                                    div { class: "ptz-presets-empty",
                                        {i18n::t(locale, "ptz_presets_no_match")}
                                    }
                                }
                            } else {
                                rsx! {
                                    ul { class: "ptz-presets-list",
                                        for preset in filtered {
                                            PresetRow {
                                                key: "{preset.token}",
                                                token: preset.token.clone(),
                                                name: preset.name.clone(),
                                                goto_preset: goto_preset,
                                                remove_preset: remove_preset,
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    // Save the camera's current position as a new preset.
                    // The camera captures whatever it's pointing at when
                    // SetPreset fires; we only supply the label here.
                    div { class: "ptz-preset-new",
                        input {
                            class: "ptz-preset-input",
                            r#type: "text",
                            placeholder: i18n::t(locale, "ptz_preset_new_placeholder"),
                            value: "{*new_preset_name.read()}",
                            oninput: move |e| new_preset_name.set(e.value()),
                            onkeydown: move |e| {
                                if e.key() == Key::Enter {
                                    save_preset.call(new_preset_name.peek().clone());
                                }
                            },
                        }
                        button {
                            class: "btn btn-sm btn-primary",
                            title: i18n::t(locale, "ptz_preset_save_hint"),
                            onclick: move |_| save_preset.call(new_preset_name.peek().clone()),
                            Icon { name: "plus", size: 12 }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DirButton(
    pan: f32,
    tilt: f32,
    icon: &'static str,
    do_move: Callback<(f32, f32, f32)>,
    do_stop: Callback<()>,
    speed: Signal<f32>,
) -> Element {
    rsx! {
        button {
            class: "ptz-dir",
            onmousedown: move |_| {
                let s = *speed.read();
                do_move.call((pan * s, tilt * s, 0.0));
            },
            onmouseup: move |_| do_stop.call(()),
            // Mouse drag-off should also stop — ONVIF ContinuousMove keeps
            // running on the camera until something tells it otherwise.
            onmouseleave: move |_| do_stop.call(()),
            Icon { name: icon, size: 18 }
        }
    }
}

#[component]
fn ZoomButton(
    dir: f32,
    icon: &'static str,
    do_move: Callback<(f32, f32, f32)>,
    do_stop: Callback<()>,
    speed: Signal<f32>,
) -> Element {
    rsx! {
        button {
            class: "ptz-zoom-btn",
            onmousedown: move |_| {
                let s = *speed.read();
                do_move.call((0.0, 0.0, dir * s));
            },
            onmouseup: move |_| do_stop.call(()),
            onmouseleave: move |_| do_stop.call(()),
            Icon { name: icon, size: 16 }
        }
    }
}

/// One axis of the live PTZ readout.
///
/// `None` renders as `—`, not as `0.00`. Every axis of `PtzStatus` is
/// `Option<f32>` because a device is free to report a move state and no
/// position, and printing a zero there would be a coordinate the head is
/// probably not at.
#[component]
fn PtzAxisReadout(label: &'static str, value: Option<f32>) -> Element {
    rsx! {
        span { class: "ptz-status-axis",
            span { class: "ptz-status-axis-label", "{label}" }
            match value {
                Some(v) => rsx! { span { {format!("{v:.2}")} } },
                None => rsx! { span { class: "ptz-status-axis-unknown", "—" } },
            }
        }
    }
}

/// One absolute-position slider, bounded by what the node declared for that
/// axis rather than by a normalised guess.
#[component]
fn AbsAxisSlider(label: &'static str, min: f32, max: f32, value: Signal<f32>) -> Element {
    // Devices publish anything from -1..1 to 0..3600; a fixed step would be
    // either uselessly coarse or absurdly fine. 200 stops across whatever the
    // head offers.
    let step = ((max - min) / 200.0).max(f32::EPSILON);
    rsx! {
        label { class: "ptz-abs-axis",
            span { class: "ptz-abs-axis-label", "{label}" }
            input {
                r#type: "range",
                min: "{min}", max: "{max}", step: "{step}",
                value: "{*value.read()}",
                oninput: move |e| {
                    if let Ok(v) = e.value().parse::<f32>() {
                        value.clone().set(v);
                    }
                },
            }
            span { class: "ptz-abs-axis-value", {format!("{:.2}", *value.read())} }
        }
    }
}

/// One focus direction.
///
/// `speed` arrives already resolved against the device's declared range and
/// already carrying its sign, so this component never computes a speed of its
/// own. `None` means the lens declared no speed in this direction — the button
/// is disabled rather than sending a value that would be accepted and ignored.
#[component]
fn FocusButton(
    icon: &'static str,
    label: &'static str,
    unsupported_label: &'static str,
    focus_move: Callback<f32>,
    focus_stop: Callback<()>,
    speed: Option<f32>,
) -> Element {
    rsx! {
        button {
            class: "ptz-zoom-btn",
            disabled: speed.is_none(),
            title: if speed.is_some() { label } else { unsupported_label },
            onmousedown: move |_| {
                if let Some(s) = speed {
                    focus_move.call(s);
                }
            },
            onmouseup: move |_| focus_stop.call(()),
            onmouseleave: move |_| focus_stop.call(()),
            Icon { name: icon, size: 16 }
        }
    }
}

#[component]
fn PresetRow(
    token: String,
    name: String,
    goto_preset: Callback<String>,
    remove_preset: Callback<String>,
) -> Element {
    let display = if name.is_empty() {
        format!("[{token}]")
    } else {
        name.clone()
    };
    let token_for_click = token.clone();
    let token_for_delete = token;
    rsx! {
        li {
            class: "ptz-preset-item",
            onclick: move |_| goto_preset.call(token_for_click.clone()),
            Icon { name: "navigation-2", size: 12 }
            span { class: "ptz-preset-name", "{display}" }
            button {
                class: "ptz-preset-delete",
                title: "Delete",
                onclick: move |e| {
                    // Don't bubble up into the row's Goto click.
                    e.stop_propagation();
                    remove_preset.call(token_for_delete.clone());
                },
                Icon { name: "x", size: 12 }
            }
        }
    }
}
