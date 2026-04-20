use crate::api::{base_url_from_device_addr, resolve_snapshot_url};

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
