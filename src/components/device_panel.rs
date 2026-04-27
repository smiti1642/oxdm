#![allow(non_snake_case)]
use crate::api;
use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx, View};
use dioxus::prelude::*;
use tracing::{debug, warn};

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
    /// `true` when the camera marked the snapshot URI as single-use
    /// (`<tt:InvalidAfterConnect>true</tt:InvalidAfterConnect>`). Each tick
    /// must re-resolve a fresh URL via GetSnapshotUri instead of reusing the
    /// cached one — seen on GeoVision LPR cameras whose URLs embed a
    /// per-call timestamp pointing at a temp file the camera then deletes.
    invalid_after_connect: bool,
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
                return Ok::<(String, Vec<ProfileInfo>), String>((addr, Vec::new()));
            }
            let (u, p) = creds.as_options();

            let profiles = api::get_profiles(&addr, u, p).await?;
            debug!(addr = %addr, count = profiles.len(), "GetProfiles OK");

            let mut infos = Vec::new();
            for profile in &profiles {
                // ONVIF Profile S: only profiles bound to a video encoder
                // configuration support GetSnapshotUri. Metadata-only,
                // audio-only and analytics-only profiles will either return
                // a SOAP fault or hand back a URL the camera then 500s on
                // (seen on GeoVision LPR series). Skip them outright.
                if profile.video_encoder_token.is_none() {
                    debug!(
                        addr = %addr,
                        profile = %profile.token,
                        name = %profile.name,
                        "Skipping GetSnapshotUri (no video encoder configuration)"
                    );
                    infos.push(ProfileInfo {
                        profile_token: profile.token.clone(),
                        profile_name: profile.name.clone(),
                        snapshot_url: None,
                        invalid_after_connect: false,
                        creds: creds.clone(),
                    });
                    continue;
                }
                match api::get_snapshot_uri(&addr, u, p, &profile.token).await {
                    Ok(snap) => {
                        let snapshot_url = api::resolve_snapshot_url(&addr, &snap.uri);
                        debug!(
                            addr = %addr,
                            profile = %profile.token,
                            name = %profile.name,
                            raw_uri = %snap.uri,
                            resolved_url = %snapshot_url,
                            invalid_after_connect = snap.invalid_after_connect,
                            "GetSnapshotUri OK"
                        );
                        infos.push(ProfileInfo {
                            profile_token: profile.token.clone(),
                            profile_name: profile.name.clone(),
                            snapshot_url: Some(snapshot_url),
                            invalid_after_connect: snap.invalid_after_connect,
                            creds: creds.clone(),
                        });
                    }
                    Err(e) => {
                        warn!(
                            addr = %addr,
                            profile = %profile.token,
                            name = %profile.name,
                            error = %e,
                            "GetSnapshotUri FAILED — profile shown without thumbnail"
                        );
                        infos.push(ProfileInfo {
                            profile_token: profile.token.clone(),
                            profile_name: profile.name.clone(),
                            snapshot_url: None,
                            invalid_after_connect: false,
                            creds: creds.clone(),
                        });
                    }
                }
            }
            // Tag the result with the addr it was fetched for so the
            // render layer can detect a stale fetch (different addr) and
            // show Loading instead of mixing one camera's snapshot URLs
            // with another camera's UI.
            Ok((addr.clone(), infos))
        }
    });

    // Read the currently-selected device addr at render time. Used both as
    // the ProfileCard key prefix (forces remount on device switch) and as a
    // freshness check against `profiles_res` — see comment below.
    let addr_now = ctx
        .selected
        .read()
        .and_then(|i| ctx.devices.read().get(i).map(|d| d.addr.clone()))
        .unwrap_or_default();

    rsx! {
        match &*profiles_res.read_unchecked() {
            None => rsx! {
                div { class: "thumb-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(_)) => rsx! {
                div { class: "thumb-empty", {i18n::t(locale, "no_profiles")} }
            },
            // Stale: result was fetched for the previous device, but the
            // user has already switched. Render Loading until use_resource
            // re-runs with the new addr — otherwise we'd build ProfileCards
            // with the old device's snapshot URLs and briefly display
            // its thumbnails on the new device's panel.
            Some(Ok((res_addr, _))) if res_addr != &addr_now => rsx! {
                div { class: "thumb-loading", {i18n::t(locale, "loading")} }
            },
            Some(Ok((_, infos))) if infos.is_empty() => rsx! {
                div { class: "thumb-empty", {i18n::t(locale, "no_profiles")} }
            },
            Some(Ok((_, infos))) => rsx! {
                div { class: "thumb-grid",
                    for info in infos {
                        ProfileCard {
                            key: "{addr_now}::{info.profile_token}",
                            device_addr: addr_now.clone(),
                            profile_token: info.profile_token.clone(),
                            profile_name: info.profile_name.clone(),
                            snapshot_url: info.snapshot_url.clone(),
                            invalid_after_connect: info.invalid_after_connect,
                            creds: info.creds.clone(),
                        }
                    }
                }
            },
        }
    }
}

#[component]
fn ProfileCard(
    device_addr: String,
    profile_token: String,
    profile_name: String,
    snapshot_url: Option<String>,
    /// `true` when GetSnapshotUri reported `<InvalidAfterConnect>true</…>`.
    /// Card re-resolves a fresh URL each tick instead of caching the first
    /// one — see `ProfileInfo::invalid_after_connect`.
    invalid_after_connect: bool,
    creds: Credentials,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut view = ctx.view;
    let mut profile_sig = ctx.selected_profile;

    // Auto-refresh tick signal — triggers use_resource re-run every 3s
    let mut tick = use_signal(|| 0u32);
    use_future(move || async move {
        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            let next = tick.peek().wrapping_add(1);
            tick.set(next);
        }
    });

    // Sticky failure flag — once a snapshot URL has failed to fetch, stop
    // re-trying it on every tick. Hammering a known-broken URL every 3s
    // wastes bandwidth, spams logs, and on cameras with brute-force
    // protection (Hanwha/Samsung Wisenet) can lock the admin account.
    // The flag is reset implicitly when the user re-selects the device,
    // because that creates a fresh ProfileCard instance with a fresh signal.
    let mut broken = use_signal(|| false);

    // Fetch snapshot via Rust backend (handles Digest auth, self-signed certs).
    // For most cameras the cached URL is reused every tick. For cameras that
    // marked the URL single-use (`invalid_after_connect`), GetSnapshotUri is
    // called again per tick to mint a fresh URL — typically these embed a
    // per-call timestamp pointing at a temp file the camera then deletes.
    let token_for_fetch = profile_token.clone();
    let data_uri = use_resource(move || {
        let url = snapshot_url.clone();
        let creds = creds.clone();
        let device_addr = device_addr.clone();
        let profile_token = token_for_fetch.clone();
        let _tick = *tick.read(); // subscribe to tick signal for periodic refresh
        let already_broken = *broken.read();
        let needs_refresh = invalid_after_connect;
        async move {
            if already_broken {
                return Err("Snapshot endpoint marked broken — skipping retry".to_string());
            }
            let (u, p) = creds.as_options();

            // Resolve the URL — either reuse the cached one or re-fetch.
            // For invalid_after_connect cameras (LPR-style temp-file URLs
            // with per-call timestamps) we don't mark broken on errors;
            // many of those 500s are timing races where the camera serves
            // some frames but not others, so each tick re-tries.
            let url = if needs_refresh {
                match api::get_snapshot_uri(&device_addr, u, p, &profile_token).await {
                    Ok(snap) => api::resolve_snapshot_url(&device_addr, &snap.uri),
                    Err(e) => return Err(e),
                }
            } else {
                match url {
                    Some(u) => u,
                    None => return Err("No snapshot".to_string()),
                }
            };

            let result = api::fetch_snapshot_data_uri(&url, u, p).await;
            if result.is_err() && !needs_refresh {
                broken.set(true);
            }
            result
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
    let name_for_save = profile_name.clone();
    let data_uri_for_save = data_uri;

    rsx! {
        div {
            class: card_class,
            onclick: move |_| {
                profile_sig.set(Some(profile_token.clone()));
            },
            // Snapshot wrapper so the download button can be absolutely
            // positioned in the top-right of the image without joining
            // the page-switch action row in the footer.
            div { class: "thumb-img-wrap",
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
                button {
                    class: "thumb-snapshot-save",
                    title: i18n::t(locale, "snapshot_save"),
                    onclick: move |e| {
                        e.stop_propagation();
                        // Snapshot only the *current* data URI value;
                        // fire-and-forget the save so the file dialog
                        // doesn't block the auto-refresh tick.
                        let snap = match &*data_uri_for_save.read_unchecked() {
                            Some(Ok(uri)) => uri.clone(),
                            _ => {
                                ctx.push_toast(
                                    crate::state::ToastLevel::Error,
                                    i18n::t(locale, "snapshot_save_no_image"),
                                );
                                return;
                            }
                        };
                        let default_name = format!("{}.jpg", sanitize_filename(&name_for_save));
                        let toast_ctx = ctx;
                        let saved_label = i18n::t(locale, "snapshot_saved").to_string();
                        let failed_label = i18n::t(locale, "snapshot_save_failed").to_string();
                        spawn(async move {
                            let Some(handle) = rfd::AsyncFileDialog::new()
                                .set_file_name(&default_name)
                                .add_filter("JPEG", &["jpg", "jpeg"])
                                .save_file()
                                .await
                            else {
                                return;
                            };
                            let path = handle.path().to_path_buf();
                            match decode_jpeg_data_uri(&snap) {
                                Some(bytes) => match std::fs::write(&path, &bytes) {
                                    Ok(()) => {
                                        tracing::info!(path = %path.display(), bytes = bytes.len(), "snapshot saved");
                                        toast_ctx.push_toast(
                                            crate::state::ToastLevel::Success,
                                            format!("{}: {}", saved_label, path.display()),
                                        );
                                    }
                                    Err(e) => {
                                        tracing::warn!(error = %e, path = %path.display(), "snapshot save failed");
                                        toast_ctx.push_toast(
                                            crate::state::ToastLevel::Error,
                                            format!("{failed_label}: {e}"),
                                        );
                                    }
                                },
                                None => {
                                    toast_ctx.push_toast(
                                        crate::state::ToastLevel::Error,
                                        failed_label,
                                    );
                                }
                            }
                        });
                    },
                    Icon { name: "download", size: 12 }
                }
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

/// Strip non-filesystem-safe characters and collapse whitespace so the
/// suggested filename in the Save dialog doesn't get rejected on
/// Windows (which forbids `<>:"/\|?*`).
fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "snapshot".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Decode a `data:image/jpeg;base64,...` URI into raw JPEG bytes.
/// Returns `None` if the URI doesn't have the expected prefix or the
/// base64 payload doesn't decode.
fn decode_jpeg_data_uri(uri: &str) -> Option<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    // We accept both "image/jpeg" and "image/jpg" since some cameras
    // (and our own snapshot fetcher) have shipped both at various points.
    let payload = uri
        .strip_prefix("data:image/jpeg;base64,")
        .or_else(|| uri.strip_prefix("data:image/jpg;base64,"))?;
    STANDARD.decode(payload).ok()
}
