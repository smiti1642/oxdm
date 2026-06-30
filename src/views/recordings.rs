#![allow(non_snake_case)]
use crate::api;
use crate::components::{Icon, TabError};
use crate::i18n;
use crate::state::{Credentials, Ctx};
use crate::video::{self, EmbedKind};
use dioxus::prelude::*;
use oxvif::RecordingInformation;

/// Profile G recording playback. Lists the device's stored recordings
/// (via `api::search_recordings`) on the left; selecting one resolves its
/// RTSP replay URI (`api::get_replay_uri`) and plays it through the go2rtc
/// bridge on the right.
///
/// MVP scope: whole-recording playback. Timeline seeking needs ONVIF replay
/// RTSP headers (`Range` / `Require: onvif/replay`) that go2rtc doesn't drive,
/// so it's deferred — see ROADMAP.md #4 (risk R1).
#[component]
pub fn RecordingsView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let selected_rec = use_signal(|| None::<String>);

    let mut recordings_res = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move {
            if addr.is_empty() {
                return Ok::<Vec<RecordingInformation>, String>(Vec::new());
            }
            api::search_recordings(&addr, &creds).await
        }
    });

    rsx! {
        div { class: "recordings-view",
            div { class: "content-header",
                Icon { name: "clock", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_recordings")} }
            }

            div { class: "recordings-body",
                div { class: "recordings-list",
                    match &*recordings_res.read_unchecked() {
                        None => rsx! {
                            div { class: "recordings-placeholder", {i18n::t(locale, "loading")} }
                        },
                        Some(Err(e)) => rsx! {
                            TabError {
                                error: e.clone(),
                                on_retry: move |_| recordings_res.restart(),
                            }
                        },
                        Some(Ok(recs)) if recs.is_empty() => rsx! {
                            div { class: "recordings-placeholder", {i18n::t(locale, "recordings_empty")} }
                        },
                        Some(Ok(recs)) => rsx! {
                            for rec in recs.iter() {
                                RecordingRow {
                                    key: "{rec.recording_token}",
                                    token: rec.recording_token.clone(),
                                    source: rec.source_name.clone(),
                                    time_range: format_range(rec),
                                    status: rec.recording_status.clone(),
                                    selected: selected_rec,
                                }
                            }
                        },
                    }
                }

                div { class: "recordings-player",
                    match selected_rec.read().clone() {
                        None => rsx! {
                            div { class: "recordings-placeholder",
                                {i18n::t(locale, "recordings_select_hint")}
                            }
                        },
                        Some(token) => rsx! {
                            ReplayStage { key: "{token}", addr, creds, recording_token: token }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn RecordingRow(
    token: String,
    source: String,
    time_range: String,
    status: String,
    selected: Signal<Option<String>>,
) -> Element {
    let is_sel = selected.read().as_deref() == Some(token.as_str());
    let cls = if is_sel {
        "recording-row recording-row--active"
    } else {
        "recording-row"
    };
    let t = token.clone();
    rsx! {
        button {
            class: cls,
            onclick: move |_| {
                let mut selected = selected;
                selected.set(Some(t.clone()));
            },
            div { class: "recording-row-source", "{source}" }
            div { class: "recording-row-time", "{time_range}" }
            span { class: "recording-row-status", "{status}" }
        }
    }
}

/// Resolve the replay URI for `recording_token` and play it via go2rtc.
/// Remounted (keyed on the token) whenever the user picks another recording,
/// so the `use_resource` re-fetches cleanly.
#[component]
fn ReplayStage(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    recording_token: String,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let source = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        let token = recording_token.clone();
        async move {
            if addr.is_empty() {
                return Err("no_device".to_string());
            }
            // Replay is RTSP-only; the snapshot backend can't do it.
            let backend = video::go2rtc().ok_or_else(|| "no_backend".to_string())?;
            let uri = api::get_replay_uri(&addr, &creds, &token)
                .await
                .map_err(|e| format!("replay_uri:{e}"))?;
            backend
                .open_rtsp(&uri, &addr, &creds)
                .await
                .map_err(|e| format!("backend_error:{e}"))
        }
    });

    rsx! {
        div { class: "live-video-stage",
            match &*source.read_unchecked() {
                None => rsx! {
                    div { class: "live-video-placeholder", {i18n::t(locale, "loading")} }
                },
                Some(Err(reason)) => {
                    let key = match reason.as_str() {
                        "no_device"  => "live_video_no_device",
                        "no_backend" => "live_video_no_backend",
                        _            => "live_video_error",
                    };
                    let detail = reason
                        .strip_prefix("backend_error:")
                        .or_else(|| reason.strip_prefix("replay_uri:"))
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
                        img { class: "live-video-frame", src: "{src.url}", alt: "recording replay" }
                    },
                    EmbedKind::Video => rsx! {
                        video {
                            class: "live-video-frame",
                            src: "{src.url}",
                            autoplay: true,
                            controls: true,
                            muted: true,
                        }
                    },
                    EmbedKind::Iframe => rsx! {
                        iframe { class: "live-video-frame", src: "{src.url}" }
                    },
                },
            }
        }
    }
}

/// Render a recording's `earliest – latest` time span (ISO-8601, as the
/// device reports it), falling back to whichever bound is present.
fn format_range(rec: &RecordingInformation) -> String {
    match (&rec.earliest_recording, &rec.latest_recording) {
        (Some(a), Some(b)) => format!("{a} – {b}"),
        (Some(a), None) => a.clone(),
        (None, Some(b)) => b.clone(),
        (None, None) => "—".to_string(),
    }
}
