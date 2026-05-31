#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx};
use crate::video::{self, EmbedKind};
use dioxus::prelude::*;

/// Which video backend a stage should use for the current view.
/// Defaults to Snapshot — light, no extra runtime, works everywhere.
/// User flips to Rtsp from the tab strip; preference is per-session
/// (intentionally not persisted yet — most users will pick once and
/// stay).
///
/// Reused by Imaging and PTZ so they can offer the same Snapshot/RTSP
/// choice as the dedicated Live Video view.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LiveVideoMode {
    Snapshot,
    Rtsp,
}

impl LiveVideoMode {
    pub fn backend_id(self) -> &'static str {
        match self {
            Self::Snapshot => "mjpeg",
            Self::Rtsp => "go2rtc",
        }
    }
}

/// Reusable `<Snapshot | RTSP>` tab strip plus the H.265-needs-ffmpeg
/// tip. The caller owns the `mode` signal and decides where to place
/// this in its header. Designed to drop into Live Video, Imaging, and
/// PTZ uniformly so users encounter the same affordance everywhere.
#[component]
pub fn LiveModeTabs(mode: Signal<LiveVideoMode>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        div { class: "live-video-modes",
            ModeTab {
                active: *mode.read() == LiveVideoMode::Snapshot,
                label: i18n::t(locale, "live_mode_snapshot"),
                title: i18n::t(locale, "live_mode_snapshot_hint"),
                onclick: {
                    let mut mode = mode;
                    move |_| mode.set(LiveVideoMode::Snapshot)
                },
            }
            ModeTab {
                active: *mode.read() == LiveVideoMode::Rtsp,
                label: i18n::t(locale, "live_mode_rtsp"),
                title: i18n::t(locale, "live_mode_rtsp_hint"),
                onclick: {
                    let mut mode = mode;
                    move |_| mode.set(LiveVideoMode::Rtsp)
                },
            }
        }
    }
}

/// Conditional banner explaining the H.265 / ffmpeg situation. Renders
/// nothing unless the user is on RTSP mode without ffmpeg in PATH.
#[component]
pub fn LiveH265Tip(mode: Signal<LiveVideoMode>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let show = matches!(*mode.read(), LiveVideoMode::Rtsp) && !video::go2rtc::ffmpeg_available();
    if !show {
        return rsx! {};
    }
    rsx! {
        div { class: "live-video-tip",
            Icon { name: "info", size: 14 }
            span { class: "live-video-tip-body",
                {i18n::t(locale, "live_h265_tip")}
            }
        }
    }
}

/// Live video panel — full view with header.
///
/// Tab strip lets the user choose between Snapshot mode (the
/// always-available MJPEG polling backend, ~5–10 fps) and RTSP mode
/// (go2rtc bridge → WebRTC, real frame rates including H.265). RTSP
/// requires the bundled `go2rtc(.exe)` to be locatable — the Stage
/// shows an explanatory error if not.
///
/// `LiveVideoStage` is reusable elsewhere (Imaging preview), but
/// embedded uses pin to Snapshot — the tab strip lives here only.
#[component]
pub fn LiveVideoView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mode = use_signal(|| LiveVideoMode::Snapshot);
    let profile_sig = ctx.selected_profile;

    // Memo so LiveVideoStage's use_resource sees the backend choice
    // as a reactive dep — Dioxus only re-runs a resource when signals
    // it reads change, so passing a plain value doesn't trigger a
    // re-fetch on tab switch.
    let backend_id = use_memo(move || mode.read().backend_id());
    let backend_display = match *mode.read() {
        LiveVideoMode::Snapshot => video::mjpeg(),
        LiveVideoMode::Rtsp => video::go2rtc(),
    }
    .map(|b| b.display_name());

    // Saving the live frame directly isn't possible — the stream lives in
    // the webview (an <img>/<video> URL), not in Rust memory. So we grab a
    // fresh JPEG via GetSnapshotUri on click, which works in both Snapshot
    // and RTSP modes. Disabled until a device + profile are selected.
    let can_save = !addr.read().is_empty() && profile_sig.read().is_some();

    rsx! {
        div { class: "live-video-view",
            div { class: "content-header",
                Icon { name: "video", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_live_video")} }
                LiveModeTabs { mode }
                if let Some(name) = backend_display {
                    span { class: "live-video-backend",
                        " · {name}"
                    }
                }
                button {
                    class: "icon-btn live-video-save",
                    disabled: !can_save,
                    title: if can_save { i18n::t(locale, "snapshot_save") } else { i18n::t(locale, "snapshot_save_no_image") },
                    onclick: move |_| {
                        let Some(token) = profile_sig.read().clone() else { return };
                        let addr = addr.read().clone();
                        let creds = creds.read().clone();
                        let toast_ctx = ctx;
                        let default_name = format!("{}.jpg", crate::util::sanitize_filename(&token));
                        let saved_label = i18n::t(locale, "snapshot_saved").to_string();
                        let failed_label = i18n::t(locale, "snapshot_save_failed").to_string();
                        spawn(async move {
                            let snap = match crate::api::get_snapshot_uri(&addr, &creds, &token).await {
                                Ok(s) => {
                                    let url = crate::api::resolve_snapshot_url(&addr, &s.uri);
                                    match crate::api::fetch_snapshot_data_uri(&url, &creds).await {
                                        Ok(uri) => uri,
                                        Err(e) => {
                                            toast_ctx.push_toast(crate::state::ToastLevel::Error, format!("{failed_label}: {e}"));
                                            return;
                                        }
                                    }
                                }
                                Err(e) => {
                                    toast_ctx.push_toast(crate::state::ToastLevel::Error, format!("{failed_label}: {e}"));
                                    return;
                                }
                            };
                            let Some(handle) = rfd::AsyncFileDialog::new()
                                .set_file_name(&default_name)
                                .add_filter("JPEG", &["jpg", "jpeg"])
                                .save_file()
                                .await
                            else {
                                return;
                            };
                            let path = handle.path().to_path_buf();
                            match crate::util::decode_jpeg_data_uri(&snap) {
                                Some(bytes) => match std::fs::write(&path, &bytes) {
                                    Ok(()) => {
                                        tracing::info!(path = %path.display(), bytes = bytes.len(), "live snapshot saved");
                                        toast_ctx.push_toast(crate::state::ToastLevel::Success, format!("{}: {}", saved_label, path.display()));
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, path = %path.display(), "live snapshot save failed");
                                        toast_ctx.push_toast(crate::state::ToastLevel::Error, format!("{failed_label}: {e}"));
                                    }
                                },
                                None => toast_ctx.push_toast(crate::state::ToastLevel::Error, failed_label),
                            }
                        });
                    },
                    Icon { name: "download", size: 16 }
                }
            }

            LiveH265Tip { mode }

            LiveVideoStage {
                addr,
                creds,
                backend_id: Some(backend_id.into()),
            }
        }
    }
}

#[component]
fn ModeTab(
    active: bool,
    label: &'static str,
    title: &'static str,
    onclick: EventHandler<MouseEvent>,
) -> Element {
    let class = if active {
        "live-video-mode-tab live-video-mode-tab--active"
    } else {
        "live-video-mode-tab"
    };
    rsx! {
        button {
            class,
            title,
            onclick: move |evt| onclick.call(evt),
            "{label}"
        }
    }
}

/// Embeddable video stage — header-less, fills its parent.
///
/// `backend` is optional. `None` is the implicit default (current
/// installed backend = MJPEG); embedded users that want a specific
/// backend pass `Some(...)`. Re-using this from Imaging keeps the
/// preview consistent with Live Video.
#[component]
pub fn LiveVideoStage(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    /// Which backend to use as a reactive signal — reading it inside
    /// `use_resource` makes Dioxus re-run the fetch on tab switch.
    /// `None` falls back to the implicit default (MJPEG via
    /// `video::current()`). Signal-of-string instead of trait object
    /// because Dioxus props need PartialEq and trait objects don't
    /// implement it.
    #[props(optional)]
    backend_id: Option<ReadSignal<&'static str>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let profile_sig = ctx.selected_profile;

    let source = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let profile = profile_sig.read().clone();
        // Read INSIDE the closure so changes re-trigger the resource.
        let backend_name = backend_id.map(|sig| *sig.read());
        async move {
            if addr.is_empty() {
                return Err("no_device".to_string());
            }
            let token = profile.ok_or_else(|| "no_profile".to_string())?;
            let backend = match backend_name {
                Some("mjpeg") => video::mjpeg(),
                Some("go2rtc") => video::go2rtc(),
                _ => video::current(),
            }
            .ok_or_else(|| "no_backend".to_string())?;
            backend
                .open(&addr, &token, &creds)
                .await
                .map_err(|e| format!("backend_error:{e}"))
        }
    });

    rsx! {
        div { class: "live-video-stage",
            match &*source.read_unchecked() {
                None => rsx! {
                    div { class: "live-video-placeholder",
                        {i18n::t(locale, "loading")}
                    }
                },
                Some(Err(reason)) => {
                    let key = match reason.as_str() {
                        "no_device"  => "live_video_no_device",
                        "no_profile" => "live_video_no_profile",
                        "no_backend" => "live_video_no_backend",
                        _            => "live_video_error",
                    };
                    let detail = reason
                        .strip_prefix("backend_error:")
                        .map(str::to_string);
                    rsx! {
                        div { class: "live-video-placeholder",
                            Icon { name: "alert-triangle", size: 28 }
                            p { {i18n::t(locale, key)} }
                            if let Some(msg) = detail {
                                p { class: "live-video-detail", "{msg}" }
                            }
                        }
                    }
                }
                Some(Ok(src)) => match src.embed {
                    EmbedKind::Img => rsx! {
                        img {
                            class: "live-video-frame",
                            src: "{src.url}",
                            alt: "live video stream"
                        }
                    },
                    EmbedKind::Video => rsx! {
                        video {
                            class: "live-video-frame",
                            src: "{src.url}",
                            autoplay: true,
                            controls: true,
                            muted: true
                        }
                    },
                    EmbedKind::Iframe => rsx! {
                        iframe {
                            class: "live-video-frame",
                            src: "{src.url}"
                        }
                    },
                },
            }
        }
    }
}
