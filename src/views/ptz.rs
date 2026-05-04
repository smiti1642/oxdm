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
            let (u, p) = creds_s.as_options();
            match api::has_ptz_service(&addr_s, u, p).await {
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
            let (u, p) = creds_s.as_options();
            api::ptz_get_presets(&addr_s, u, p, &token).await
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
            let (u, p) = creds_s.as_options();
            let profiles = api::get_profiles(&addr_s, u, p).await?;
            let source_token = token_opt
                .as_ref()
                .and_then(|pt| profiles.iter().find(|p| p.token == *pt))
                .or_else(|| profiles.first())
                .and_then(|p| p.video_source_token.clone())
                .ok_or_else(|| "no_video_source".to_string())?;
            Ok::<_, String>(source_token)
        }
    });

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
            let (u, p) = creds_s.as_options();
            if let Err(e) = api::ptz_continuous_move(&addr_s, u, p, &token, pan, tilt, zoom).await {
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
            let (u, p) = creds_s.as_options();
            if let Err(e) = api::ptz_stop(&addr_s, u, p, &token).await {
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
            let (u, p) = creds_s.as_options();
            match api::ptz_goto_home_position(&addr_s, u, p, &token).await {
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
            let (u, p) = creds_s.as_options();
            let profiles = api::get_profiles(&addr_s, u, p).await?;
            let source_token = token_opt
                .as_ref()
                .and_then(|pt| profiles.iter().find(|p| p.token == *pt))
                .or_else(|| profiles.first())
                .and_then(|p| p.video_source_token.clone())
                .ok_or_else(|| "no_video_source".to_string())?;
            let settings = api::get_imaging_settings(&addr_s, u, p, &source_token).await?;
            Ok(settings.focus_mode.unwrap_or_else(|| "AUTO".to_string()))
        }
    });

    // Toggle Focus.AutoFocusMode by GET-modify-SET to avoid clobbering
    // unrelated ImagingSettings fields. Restarts focus_mode_state so the
    // segmented control reflects the new value within one round-trip.
    let set_focus_mode_cb = use_callback(move |auto: bool| {
        let source_token = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => t.clone(),
            _ => return,
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            let (u, p) = creds_s.as_options();
            let mut settings = match api::get_imaging_settings(&addr_s, u, p, &source_token).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!(error = %e, "get_imaging_settings failed");
                    return;
                }
            };
            settings.focus_mode = Some(if auto { "AUTO".into() } else { "MANUAL".into() });
            match api::set_imaging_settings(&addr_s, u, p, &source_token, &settings).await {
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
            Some(Ok(t)) => t.clone(),
            _ => return,
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            let (u, p) = creds_s.as_options();
            if let Err(e) = api::imaging_focus_continuous(&addr_s, u, p, &source_token, speed).await
            {
                tracing::warn!(error = %e, "focus continuous failed");
            }
        });
    });

    let focus_stop = use_callback(move |_: ()| {
        let source_token = match &*focus_state.read_unchecked() {
            Some(Ok(t)) => t.clone(),
            _ => return,
        };
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        spawn(async move {
            let (u, p) = creds_s.as_options();
            if let Err(e) = api::imaging_focus_stop(&addr_s, u, p, &source_token).await {
                tracing::warn!(error = %e, "focus stop failed");
            }
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
            let (u, p) = creds_s.as_options();
            match api::ptz_set_preset(&addr_s, u, p, &token, Some(name.trim()), None).await {
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
            let (u, p) = creds_s.as_options();
            match api::ptz_remove_preset(&addr_s, u, p, &token, &preset_token).await {
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
            let (u, p) = creds_s.as_options();
            if let Err(e) = api::ptz_goto_preset(&addr_s, u, p, &token, &preset_token).await {
                ctx.push_toast(ToastLevel::Error, e);
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
                                dir:  1.0,
                                icon: "arrow-up",
                                label: i18n::t(locale, "ptz_focus_far"),
                                focus_move: focus_move,
                                focus_stop: focus_stop,
                                speed,
                            }
                            FocusButton {
                                dir: -1.0,
                                icon: "arrow-down",
                                label: i18n::t(locale, "ptz_focus_near"),
                                focus_move: focus_move,
                                focus_stop: focus_stop,
                                speed,
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

#[component]
fn FocusButton(
    dir: f32,
    icon: &'static str,
    label: &'static str,
    focus_move: Callback<f32>,
    focus_stop: Callback<()>,
    speed: Signal<f32>,
) -> Element {
    rsx! {
        button {
            class: "ptz-zoom-btn",
            title: "{label}",
            onmousedown: move |_| {
                let s = *speed.read();
                focus_move.call(dir * s);
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
