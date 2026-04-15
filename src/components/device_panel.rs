#![allow(non_snake_case)]
use crate::api;
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx, View};
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
    drop(devices);

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
                NavLink { view: View::DeviceSettings, icon: "settings", label: i18n::t(locale, "nav_settings") }
                NavLink { view: View::Events,         icon: "bell",     label: i18n::t(locale, "nav_events") }
            }

            // ── NVT profile thumbnails ──────────────────────────────────────
            div { class: "panel-section panel-thumbnails",
                div { class: "panel-section-title", "NVT" }
                ProfileThumbnails {}
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

// ── NVT Profile Thumbnails ──────────────────────────────────────────────────

#[derive(Clone, Debug)]
struct ProfileInfo {
    profile_token: String,
    profile_name: String,
    /// None when GetSnapshotUri is not supported by the device for this profile.
    snapshot_url: Option<String>,
    creds: Credentials,
}

#[component]
fn ProfileThumbnails() -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let profiles_res = use_resource(move || {
        let devices = ctx.devices.read();
        let selected = *ctx.selected.read();
        let dev = selected.and_then(|i| devices.get(i)).cloned();
        let creds = dev
            .as_ref()
            .map(|d| ctx.credentials_for(d))
            .unwrap_or_else(|| ctx.global_credentials.read().clone());
        let addr = dev.map(|d| d.addr).unwrap_or_default();

        async move {
            if addr.is_empty() {
                return Ok::<_, String>(Vec::new());
            }
            let (u, p) = creds.as_options();

            let profiles = api::get_profiles(&addr, u, p).await?;

            let mut infos = Vec::new();
            for profile in &profiles {
                let snap_url = api::get_snapshot_uri(&addr, u, p, &profile.token)
                    .await
                    .ok()
                    .map(|s| s.uri);
                infos.push(ProfileInfo {
                    profile_token: profile.token.clone(),
                    profile_name: profile.name.clone(),
                    snapshot_url: snap_url,
                    creds: creds.clone(),
                });
            }
            Ok(infos)
        }
    });

    // Auto-refresh tick — drives periodic snapshot re-fetch
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
        match &*profiles_res.read_unchecked() {
            None => rsx! {
                div { class: "thumb-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(_)) => rsx! {
                div { class: "thumb-empty", {i18n::t(locale, "no_profiles")} }
            },
            Some(Ok(infos)) if infos.is_empty() => rsx! {
                div { class: "thumb-empty", {i18n::t(locale, "no_profiles")} }
            },
            Some(Ok(infos)) => rsx! {
                div { class: "thumb-grid",
                    for info in infos {
                        ProfileCard {
                            key: "{info.profile_token}",
                            profile_token: info.profile_token.clone(),
                            profile_name: info.profile_name.clone(),
                            snapshot_url: info.snapshot_url.clone(),
                            creds: info.creds.clone(),
                            tick: tick_val,
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn ProfileCard(
    profile_token: String,
    profile_name: String,
    snapshot_url: Option<String>,
    creds: Credentials,
    tick: u32,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut view = ctx.view;
    let mut profile_sig = ctx.selected_profile;

    // Fetch snapshot via Rust backend (handles Digest auth, self-signed certs)
    let data_uri = use_resource(move || {
        let url = snapshot_url.clone();
        let creds = creds.clone();
        let _tick = tick; // subscribe to tick changes for periodic refresh
        async move {
            let Some(url) = url else {
                return Err("No snapshot".to_string());
            };
            let (u, p) = creds.as_options();
            api::fetch_snapshot_data_uri(&url, u, p).await
        }
    });

    let selected = ctx
        .selected_profile
        .read()
        .as_ref()
        .map(|t| t == &profile_token)
        .unwrap_or(false);

    let card_class = if selected {
        "thumb-card thumb-card--selected"
    } else {
        "thumb-card"
    };

    let token_live = profile_token.clone();
    let token_img = profile_token.clone();
    let token_ptz = profile_token.clone();

    rsx! {
        div {
            class: card_class,
            onclick: move |_| {
                profile_sig.set(Some(profile_token.clone()));
            },
            match &*data_uri.read_unchecked() {
                Some(Ok(src)) => rsx! {
                    img { class: "thumb-img", src: "{src}", alt: "{profile_name}" }
                },
                _ => rsx! {
                    div { class: "thumb-img thumb-img--placeholder",
                        Icon { name: "camera", size: 24 }
                    }
                },
            }
            div { class: "thumb-footer",
                span { class: "thumb-label", "{profile_name}" }
                div { class: "thumb-actions",
                    button {
                        class: "thumb-action",
                        title: i18n::t(locale, "nav_live_video"),
                        onclick: move |e| {
                            e.stop_propagation();
                            profile_sig.set(Some(token_live.clone()));
                            view.set(View::LiveVideo);
                        },
                        Icon { name: "video", size: 12 }
                    }
                    button {
                        class: "thumb-action",
                        title: i18n::t(locale, "nav_imaging"),
                        onclick: move |e| {
                            e.stop_propagation();
                            profile_sig.set(Some(token_img.clone()));
                            view.set(View::ImagingSettings);
                        },
                        Icon { name: "sliders", size: 12 }
                    }
                    button {
                        class: "thumb-action",
                        title: i18n::t(locale, "nav_ptz"),
                        onclick: move |e| {
                            e.stop_propagation();
                            profile_sig.set(Some(token_ptz.clone()));
                            view.set(View::PtzControl);
                        },
                        Icon { name: "crosshair", size: 12 }
                    }
                }
            }
        }
    }
}
