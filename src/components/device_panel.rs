#![allow(non_snake_case)]
use crate::components::Icon;
use crate::i18n;
use crate::state::{Ctx, View};
use crate::{api, state::Credentials};
use dioxus::prelude::*;

#[component]
pub fn DevicePanel() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let devices = ctx.devices.read();
    let selected = *ctx.selected.read();

    let Some(idx) = selected else {
        return rsx! {
            div { class: "device-panel device-panel--empty",
                span { class: "panel-empty-hint", {i18n::t(locale, "select_device")} }
            }
        };
    };

    let Some(dev) = devices.get(idx) else {
        return rsx! { div { class: "device-panel" } };
    };

    let dev_name = dev.name.clone();
    let dev_addr = dev.addr.clone();
    let effective_creds = ctx.credentials_for(dev);

    rsx! {
        div { class: "device-panel",

            div { class: "panel-header",
                div { class: "panel-device-icon",
                    Icon { name: "camera", size: 26 }
                }
                div { class: "panel-device-name", "{dev_name}" }
            }

            div { class: "panel-section",
                div { class: "panel-section-title", {i18n::t(locale, "section_general")} }
                NavLink { view: View::DeviceSettings, icon: "settings",    label: i18n::t(locale, "nav_settings") }
                NavLink { view: View::Events,         icon: "bell",        label: i18n::t(locale, "nav_events") }
            }

            div { class: "panel-section",
                div { class: "panel-section-title", {i18n::t(locale, "section_nvt")} }
                NavLink { view: View::LiveVideo,       icon: "video",      label: i18n::t(locale, "nav_live_video") }
                NavLink { view: View::ImagingSettings, icon: "sliders",    label: i18n::t(locale, "nav_imaging") }
                NavLink { view: View::PtzControl,      icon: "crosshair",  label: i18n::t(locale, "nav_ptz") }
            }

            // ── Stream thumbnails at bottom ─────────────────────────────────
            div { class: "panel-section panel-thumbnails",
                div { class: "panel-section-title", {i18n::t(locale, "section_streams")} }
                StreamThumbnails { addr: dev_addr, creds: effective_creds }
            }
        }
    }
}

#[component]
fn NavLink(view: View, icon: &'static str, label: &'static str) -> Element {
    let ctx = use_context::<Ctx>();
    let mut view_sig = ctx.view;
    let is_active = *ctx.view.read() == view;
    let cls = if is_active {
        "nav-link nav-link--active"
    } else {
        "nav-link"
    };

    rsx! {
        button {
            class: cls,
            onclick: move |_| view_sig.set(view),
            span { class: "nav-link-icon",
                Icon { name: icon, size: 16 }
            }
            "{label}"
        }
    }
}

// ── Stream Thumbnails ───────────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct StreamInfo {
    profile_name: String,
    snapshot_url: String,
}

#[component]
fn StreamThumbnails(addr: String, creds: Credentials) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    // Fetch profiles + snapshot URIs
    let streams = use_resource(move || {
        let addr = addr.clone();
        let creds = creds.clone();
        async move {
            let (u, p) = if creds.username.is_empty() {
                (None, None)
            } else {
                (Some(creds.username.as_str()), Some(creds.password.as_str()))
            };

            let profiles = api::get_profiles(&addr, u, p).await?;

            let mut infos = Vec::new();
            for profile in &profiles {
                if let Ok(snap) = api::get_snapshot_uri(&addr, u, p, &profile.token).await {
                    infos.push(StreamInfo {
                        profile_name: profile.name.clone(),
                        snapshot_url: inject_credentials(&snap.uri, u, p),
                    });
                }
            }
            Ok::<_, String>(infos)
        }
    });

    // Auto-refresh counter to force image reload
    let mut tick = use_signal(|| 0u32);
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let next = tick.peek().wrapping_add(1);
            tick.set(next);
        }
    });
    let tick_val = *tick.read();

    rsx! {
        match &*streams.read_unchecked() {
            None => rsx! {
                div { class: "thumb-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(_)) => rsx! {
                div { class: "thumb-empty", {i18n::t(locale, "no_streams")} }
            },
            Some(Ok(infos)) if infos.is_empty() => rsx! {
                div { class: "thumb-empty", {i18n::t(locale, "no_streams")} }
            },
            Some(Ok(infos)) => rsx! {
                div { class: "thumb-grid",
                    for info in infos {
                        div { class: "thumb-card",
                            img {
                                class: "thumb-img",
                                src: "{info.snapshot_url}&_t={tick_val}",
                                alt: "{info.profile_name}",
                            }
                            div { class: "thumb-label", "{info.profile_name}" }
                        }
                    }
                }
            },
        }
    }
}

/// Inject credentials into a snapshot HTTP URL for basic auth.
/// e.g., http://192.168.1.10/snap.jpg → http://admin:pass@192.168.1.10/snap.jpg
fn inject_credentials(url: &str, username: Option<&str>, password: Option<&str>) -> String {
    let (Some(u), Some(p)) = (username, password) else {
        return url.to_string();
    };
    if u.is_empty() {
        return url.to_string();
    }
    if let Some(rest) = url.strip_prefix("http://") {
        format!("http://{u}:{p}@{rest}")
    } else if let Some(rest) = url.strip_prefix("https://") {
        format!("https://{u}:{p}@{rest}")
    } else {
        url.to_string()
    }
}
