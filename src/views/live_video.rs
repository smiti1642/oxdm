#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx};
use crate::video::{self, EmbedKind};
use dioxus::prelude::*;

/// Live video panel.
///
/// Reads the currently selected device + profile from [`Ctx`], asks the
/// installed [`crate::video::VideoBackend`] for a [`VideoSource`], then
/// embeds it according to its [`EmbedKind`]. The DOM tag (`<img>`,
/// `<video>`, `<iframe>`) is the only thing that varies — every backend
/// looks the same to this view.
#[component]
pub fn LiveVideoView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
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
            let backend = video::current().ok_or_else(|| "no_backend".to_string())?;
            backend
                .open(&addr, &token, &creds)
                .await
                .map_err(|e| format!("backend_error:{e}"))
        }
    });

    rsx! {
        div { class: "live-video-view",
            div { class: "content-header",
                Icon { name: "video", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_live_video")} }
                if let Some(b) = video::current() {
                    span { class: "live-video-backend",
                        " · {b.display_name()}"
                    }
                }
            }

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
}
