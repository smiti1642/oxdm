use crate::api::{
    base_url_from_device_addr, focus_speed, is_action_unsupported, pick_channel,
    resolve_snapshot_url, ChannelKind, DeviceGate,
};
use oxvif::{Capabilities, FloatRange, MediaProfile, MediaServiceCapabilities};

#[test]
fn base_url_strips_onvif_path() {
    assert_eq!(
        base_url_from_device_addr("http://192.168.1.1/onvif/device_service"),
        "http://192.168.1.1"
    );
    assert_eq!(
        base_url_from_device_addr("http://192.168.1.1:8080/onvif/device_service"),
        "http://192.168.1.1:8080"
    );
}

#[test]
fn base_url_falls_back_when_no_path() {
    // No `/` after authority — return the input unchanged.
    assert_eq!(
        base_url_from_device_addr("http://192.168.1.1"),
        "http://192.168.1.1"
    );
}

#[test]
fn io_unsupported_covers_the_fault_texts_and_a_missing_deviceio_endpoint() {
    // What a camera with no IO board answers.
    assert!(is_action_unsupported(
        "SOAP-ENV:Receiver: Optional Action Not Implemented"
    ));
    assert!(is_action_unsupported("ter:ActionNotSupported"));

    // What oxvif 0.15 produces before sending, when the device advertises no
    // DeviceIO service URL. `GetDigitalInputs` is a DeviceIO operation as of
    // 0.15; under 0.14 it went to the device service and the camera answered.
    assert!(is_action_unsupported(
        "Missing required field: DeviceIO service URL"
    ));

    // A different missing field is a real parse failure and must stay one —
    // the whole field name is matched, not the "missing required field" prefix.
    assert!(!is_action_unsupported("Missing required field: Token"));
    assert!(!is_action_unsupported("HTTP status 401 Unauthorized"));
}

#[test]
fn resolve_snapshot_url_passes_absolute_through() {
    assert_eq!(
        resolve_snapshot_url("http://camera/onvif", "http://other/snap.jpg"),
        "http://other/snap.jpg"
    );
    assert_eq!(
        resolve_snapshot_url("http://camera/onvif", "https://other/snap.jpg"),
        "https://other/snap.jpg"
    );
}

#[test]
fn resolve_snapshot_url_prefixes_absolute_path() {
    assert_eq!(
        resolve_snapshot_url(
            "http://192.168.1.1/onvif/device_service",
            "/cgi-bin/snap.jpg"
        ),
        "http://192.168.1.1/cgi-bin/snap.jpg"
    );
}

#[test]
fn resolve_snapshot_url_prefixes_bare_path() {
    // Some cameras (e.g. older firmware) return URI fragments without
    // a leading slash. Treat as a relative path off the device base.
    assert_eq!(
        resolve_snapshot_url("http://192.168.1.1/onvif/device_service", "snap.jpg"),
        "http://192.168.1.1/snap.jpg"
    );
}

// ── Capability gate ─────────────────────────────────────────────────────────
//
// These exercise `DeviceGate::from_caps`, not `device_gate`. The fetch is two
// lines; the *policy* — which layer answers which question, and which way to
// fail when nobody answers — is the part that can be silently wrong. It also
// cannot be tested through oxvif's mock, which advertises every service: a
// mock-driven test can only ever prove the all-true case.

/// A device advertising exactly the named services and nothing else.
fn caps_with(services: &[&str]) -> Capabilities {
    let mut c = Capabilities::default();
    for s in services {
        let url = Some(format!("http://cam/onvif/{s}"));
        match *s {
            "media" => c.media.url = url,
            "media2" => c.media2.url = url,
            "imaging" => c.imaging.url = url,
            "ptz" => c.ptz.url = url,
            "events" => c.events.url = url,
            "search" => c.search.url = url,
            "recording" => c.recording.url = url,
            "device_io" => c.device_io.url = url,
            other => panic!("test names a service the gate does not read: {other}"),
        }
    }
    c
}

fn media_caps_osd(osd: Option<bool>) -> MediaServiceCapabilities {
    MediaServiceCapabilities {
        osd,
        ..Default::default()
    }
}

#[test]
fn a_fixed_camera_hides_ptz_and_io_and_keeps_what_it_advertises() {
    // The shape that motivated the whole gate: a fixed dome with no motor and
    // no IO board, which until now still rendered both entry points.
    let caps = caps_with(&["media", "imaging", "events", "search"]);
    let gate = DeviceGate::from_caps(&caps, Some(&media_caps_osd(Some(true))));

    assert!(!gate.ptz, "no PTZ service URL must close the PTZ button");
    assert!(!gate.io, "no DeviceIO service URL must close the IO tab");

    assert!(gate.media);
    assert!(gate.imaging);
    assert!(gate.events);
    assert!(gate.recordings);
    assert!(gate.osd);
}

#[test]
fn a_device_advertising_nothing_closes_everything_and_is_not_permissive() {
    let gate = DeviceGate::from_caps(&Capabilities::default(), None);

    assert!(!gate.media);
    assert!(!gate.imaging);
    assert!(!gate.ptz);
    assert!(!gate.events);
    assert!(!gate.recordings);
    assert!(!gate.io);
    assert!(!gate.osd);

    // The two directions must actually differ — otherwise every assertion in
    // this section would hold against a gate that had stopped reading `caps`.
    assert_ne!(gate, DeviceGate::permissive());
}

#[test]
fn silence_about_osd_is_not_a_denial_but_an_explicit_false_is() {
    let caps = caps_with(&["media"]);

    // The device said yes.
    assert!(DeviceGate::from_caps(&caps, Some(&media_caps_osd(Some(true)))).osd);

    // The device sent the element but omitted the attribute — the majority of
    // firmware predating Media 2.2. Not a denial.
    assert!(DeviceGate::from_caps(&caps, Some(&media_caps_osd(None))).osd);

    // The service faulted on GetServiceCapabilities entirely. Also not a denial.
    assert!(DeviceGate::from_caps(&caps, None).osd);

    // Only this hides it.
    assert!(!DeviceGate::from_caps(&caps, Some(&media_caps_osd(Some(false)))).osd);
}

#[test]
fn osd_follows_media1_because_that_is_where_get_osds_is_sent() {
    // `OnvifSession::get_osds` resolves `media_url()`, so a Media2-only device
    // cannot serve the OSD view however enthusiastic its Media2 capabilities.
    let media2_only = caps_with(&["media2"]);
    assert!(!DeviceGate::from_caps(&media2_only, Some(&media_caps_osd(Some(true)))).osd);
    assert!(!DeviceGate::from_caps(&media2_only, Some(&media_caps_osd(Some(true)))).media);
}

#[test]
fn recordings_follows_the_search_service_not_the_recording_service() {
    // The view lists via `search_recordings`; a device that records but offers
    // no Search service has nothing to show, and the two URLs are independent.
    let records_only = caps_with(&["recording"]);
    assert!(!DeviceGate::from_caps(&records_only, None).recordings);

    let searchable = caps_with(&["search"]);
    assert!(DeviceGate::from_caps(&searchable, None).recordings);
}

// ── Per-channel token resolution ────────────────────────────────────────────
//
// Every fixture below is **two-sensor, and the two lenses disagree on every
// token**. That is the whole point: a single-sensor fixture passes just as
// well against code that ignores the requested profile entirely, so it would
// report this logic as covered while proving nothing about it.

/// Lens 0 and lens 1, sharing nothing. `meta` is a metadata-only profile — it
/// exists, so a lookup for it *succeeds*, and then has no channel of any kind.
/// That combination is what separated the three original fallback copies.
fn two_lens_profiles() -> Vec<MediaProfile> {
    let lens = |n: u8| MediaProfile {
        token: format!("profile_{n}"),
        name: format!("Lens {n}"),
        fixed: true,
        video_source_config_token: Some(format!("vsc_{n}")),
        video_source_token: Some(format!("source_{n}")),
        video_encoder_token: Some(format!("enc_{n}")),
        audio_source_token: None,
        audio_encoder_token: None,
        ptz_config_token: None,
    };
    vec![
        lens(0),
        lens(1),
        MediaProfile {
            token: "meta".to_string(),
            name: "Metadata only".to_string(),
            fixed: false,
            video_source_config_token: None,
            video_source_token: None,
            video_encoder_token: None,
            audio_source_token: None,
            audio_encoder_token: None,
            ptz_config_token: None,
        },
    ]
}

#[test]
fn asking_for_lens_1_gets_lens_1_and_is_not_a_fallback() {
    let profiles = two_lens_profiles();
    for (kind, want) in [
        (ChannelKind::Source, "source_1"),
        (ChannelKind::SourceConfig, "vsc_1"),
        (ChannelKind::Encoder, "enc_1"),
    ] {
        let pick = pick_channel(&profiles, Some("profile_1"), kind).unwrap();
        assert_eq!(pick.token, want, "{kind:?} resolved the wrong channel");
        assert!(
            !pick.fell_back,
            "{kind:?} reported a fallback it did not make"
        );
    }
}

#[test]
fn a_profile_from_another_device_falls_back_to_lens_0_and_says_so() {
    // The exact shape a device switch used to produce before `selected_profile`
    // was cleared: a token that simply is not on this camera.
    let profiles = two_lens_profiles();
    let pick = pick_channel(
        &profiles,
        Some("someone_elses_profile"),
        ChannelKind::Source,
    )
    .unwrap();

    assert_eq!(pick.token, "source_0");
    assert!(
        pick.fell_back,
        "a silent fallback is the bug — lens 0's settings shown as if they were \
         the selected profile's"
    );
}

#[test]
fn a_metadata_only_profile_falls_back_instead_of_erroring() {
    // `get_video_source_token` used to error here while its doc comment
    // promised this fallback; the other two copies fell back. The profile is
    // *found* — this is not a missing-token case — and simply has no channel.
    let profiles = two_lens_profiles();
    for (kind, want) in [
        (ChannelKind::Source, "source_0"),
        (ChannelKind::SourceConfig, "vsc_0"),
        (ChannelKind::Encoder, "enc_0"),
    ] {
        let pick = pick_channel(&profiles, Some("meta"), kind)
            .unwrap_or_else(|| panic!("{kind:?} gave up instead of falling back"));
        assert_eq!(pick.token, want);
        assert!(pick.fell_back);
    }
}

#[test]
fn expressing_no_preference_is_not_a_fallback() {
    // `None` means "give me any working channel". Nothing was asked for, so
    // nothing was overridden, and the views must not accuse the user of having
    // picked something else.
    let profiles = two_lens_profiles();
    let pick = pick_channel(&profiles, None, ChannelKind::Source).unwrap();

    assert_eq!(pick.token, "source_0");
    assert!(!pick.fell_back);
}

#[test]
fn a_device_with_no_channel_of_that_kind_resolves_to_nothing() {
    let audio_only = vec![MediaProfile {
        token: "a".to_string(),
        name: "Audio".to_string(),
        fixed: false,
        video_source_config_token: None,
        video_source_token: None,
        video_encoder_token: None,
        audio_source_token: Some("as_0".to_string()),
        audio_encoder_token: Some("ae_0".to_string()),
        ptz_config_token: None,
    }];

    assert!(pick_channel(&audio_only, Some("a"), ChannelKind::Source).is_none());
    assert!(pick_channel(&audio_only, None, ChannelKind::Encoder).is_none());
    assert!(pick_channel(&[], None, ChannelKind::SourceConfig).is_none());
}

#[test]
fn the_three_kinds_read_three_different_fields() {
    // Guards the `ChannelKind::token_of` match: a copy-paste arm reading the
    // wrong field would satisfy every test above that checks only one kind.
    let profiles = two_lens_profiles();
    let of = |kind| {
        pick_channel(&profiles, Some("profile_1"), kind)
            .unwrap()
            .token
    };

    let source = of(ChannelKind::Source);
    let config = of(ChannelKind::SourceConfig);
    let encoder = of(ChannelKind::Encoder);

    assert_ne!(source, config);
    assert_ne!(config, encoder);
    assert_ne!(source, encoder);
}

// ── Focus speed ─────────────────────────────────────────────────────────────

fn range(min: f32, max: f32) -> FloatRange {
    FloatRange { min, max }
}

#[test]
fn a_symmetric_lens_maps_the_slider_onto_both_directions() {
    // The ordinary case, and the one OxDM's old hardcoded 0.1–1.0 slider
    // happened to match: far and near are the same magnitude, opposite signs.
    let r = range(-1.0, 1.0);
    assert_eq!(focus_speed(r, 0.5, 1.0), Some(0.5));
    assert_eq!(focus_speed(r, 0.5, -1.0), Some(-0.5));
    assert_eq!(focus_speed(r, 1.0, 1.0), Some(1.0));
}

#[test]
fn a_wider_range_than_the_slider_is_used_in_full() {
    // A lens declaring 0..7 was being driven at 0.5 — a legal value, and about
    // 7% of the speed the device offered. The slider is a fraction, not a
    // speed, and this is the difference the fix makes.
    let r = range(-7.0, 7.0);
    assert_eq!(focus_speed(r, 1.0, 1.0), Some(7.0));
    assert_eq!(focus_speed(r, 0.5, 1.0), Some(3.5));
    assert_eq!(focus_speed(r, 0.5, -1.0), Some(-3.5));
}

#[test]
fn a_lens_that_declares_no_negative_speed_cannot_focus_nearer() {
    // The case that makes this a `None` and not a clamp. Sending 0.0 gets an
    // `OK` back and never moves the motor, which reads to the user as a broken
    // camera rather than an unsupported direction.
    let r = range(0.0, 1.0);
    assert_eq!(focus_speed(r, 0.5, 1.0), Some(0.5));
    assert_eq!(
        focus_speed(r, 0.5, -1.0),
        None,
        "near must be reported unsupported, not sent as zero"
    );
}

#[test]
fn a_degenerate_range_disables_both_directions() {
    let r = range(0.0, 0.0);
    assert_eq!(focus_speed(r, 1.0, 1.0), None);
    assert_eq!(focus_speed(r, 1.0, -1.0), None);
}

#[test]
fn the_result_never_leaves_the_declared_range() {
    // A slider above 1.0 should not be able to push a speed past what the
    // device accepts, whatever the UI does upstream.
    let r = range(-2.0, 3.0);
    let far = focus_speed(r, 4.0, 1.0).unwrap();
    let near = focus_speed(r, 4.0, -1.0).unwrap();

    assert!(far <= r.max, "{far} exceeds the declared maximum");
    assert!(near >= r.min, "{near} is below the declared minimum");
}
