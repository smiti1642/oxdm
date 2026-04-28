#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx};
use crate::video::{self, EmbedKind};
use dioxus::prelude::*;

/// Which video backend Live Video should use for the current view.
/// Defaults to Snapshot — light, no extra runtime, works everywhere.
/// User flips to Rtsp from the tab strip; preference is per-session
/// (intentionally not persisted yet — most users will pick once and
/// stay).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LiveVideoMode {
    Snapshot,
    Rtsp,
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
    let mut mode = use_signal(|| LiveVideoMode::Snapshot);

    let backend_id = match *mode.read() {
        LiveVideoMode::Snapshot => "mjpeg",
        LiveVideoMode::Rtsp => "go2rtc",
    };
    let backend_display = match *mode.read() {
        LiveVideoMode::Snapshot => video::mjpeg(),
        LiveVideoMode::Rtsp => video::go2rtc(),
    }
    .map(|b| b.display_name());

    rsx! {
        div { class: "live-video-view",
            div { class: "content-header",
                Icon { name: "video", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_live_video")} }
                div { class: "live-video-modes",
                    ModeTab {
                        active: *mode.read() == LiveVideoMode::Snapshot,
                        label: i18n::t(locale, "live_mode_snapshot"),
                        title: i18n::t(locale, "live_mode_snapshot_hint"),
                        onclick: move |_| mode.set(LiveVideoMode::Snapshot),
                    }
                    ModeTab {
                        active: *mode.read() == LiveVideoMode::Rtsp,
                        label: i18n::t(locale, "live_mode_rtsp"),
                        title: i18n::t(locale, "live_mode_rtsp_hint"),
                        onclick: move |_| mode.set(LiveVideoMode::Rtsp),
                    }
                }
                if let Some(name) = backend_display {
                    span { class: "live-video-backend",
                        " · {name}"
                    }
                }
            }

            LiveVideoStage { addr, creds, backend_id: Some(backend_id) }
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
    /// Which backend to use ("mjpeg" or "go2rtc"). `None` falls back
    /// to the implicit default (currently MJPEG via `video::current()`).
    /// String prop instead of `Arc<dyn VideoBackend>` because Dioxus
    /// props need PartialEq and trait objects don't implement it.
    #[props(optional)]
    backend_id: Option<&'static str>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let profile_sig = ctx.selected_profile;

    let source = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let profile = profile_sig.read().clone();
        async move {
            if addr.is_empty() {
                return Err("no_device".to_string());
            }
            let token = profile.ok_or_else(|| "no_profile".to_string())?;
            let backend = match backend_id {
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
