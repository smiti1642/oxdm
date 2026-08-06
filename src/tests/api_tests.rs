use crate::api::{
    base_url_from_device_addr, is_action_unsupported, resolve_snapshot_url, DeviceGate,
};
use oxvif::{Capabilities, MediaServiceCapabilities};

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
