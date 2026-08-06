//! End-to-end smoke test for OxDM's PTZ status / node / absolute-move wiring
//! against the oxvif mock.
//!
//! The mock is a **two-head** device and the heads deliberately disagree, which
//! is what makes this able to fail:
//!
//! ```text
//! Profile_1, Profile_2  →  PTZConfig_1  →  PTZNode_1   pan + tilt + zoom
//! Profile_3             →  PTZConfig_2  →  PTZNode_2   zoom only, no pan/tilt space
//! Profile_4             →  (unbound)                   no PTZ at all
//! ```
//!
//! A resolver that ignored the profile token, or that fell back to the first
//! node when a lookup missed, would answer `PTZNode_1` for all four and pass
//! every assertion a single-head fixture could make.

use oxvif::mock::MockServer;

#[path = "../src/api.rs"]
#[allow(dead_code, unused_imports)]
mod api;
#[path = "../src/sessions.rs"]
#[allow(dead_code, unused_imports)]
mod sessions;
#[path = "../src/state.rs"]
#[allow(dead_code, unused_imports)]
mod state;

use crate::state::Credentials;

#[tokio::test(flavor = "multi_thread")]
async fn each_profile_resolves_to_its_own_head() {
    let server = MockServer::start().await.expect("mock server boots");
    let addr = server.device_url().to_string();
    let creds = Credentials::default();

    let one = api::ptz_node_for_profile(&addr, &creds, "Profile_1")
        .await
        .expect("Profile_1 is bound to a PTZ configuration");
    assert_eq!(one.token, "PTZNode_1");

    let three = api::ptz_node_for_profile(&addr, &creds, "Profile_3")
        .await
        .expect("Profile_3 is bound to the second head");
    assert_eq!(
        three.token, "PTZNode_2",
        "Profile_3 must reach head 2 — answering head 1 here is the multi-head \
         bug this fixture exists to catch"
    );

    // The two heads disagree on more than their token, so a renderer that
    // ignored `NodeToken` could not get these right either.
    assert_ne!(one.max_presets, three.max_presets);
    assert!(one.home_supported);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_profile_with_no_ptz_configuration_resolves_to_nothing() {
    let server = MockServer::start().await.expect("mock server boots");
    let addr = server.device_url().to_string();
    let creds = Credentials::default();

    let err = api::ptz_node_for_profile(&addr, &creds, "Profile_4")
        .await
        .expect_err("Profile_4 is deliberately unbound");

    // Specifically "this profile has no head", not a transport or fault error —
    // and above all not a silent fallback to head 1.
    assert_eq!(err, "no_ptz_config");
}

#[tokio::test(flavor = "multi_thread")]
async fn the_zoom_only_head_offers_zoom_and_not_pan_or_tilt() {
    let server = MockServer::start().await.expect("mock server boots");
    let addr = server.device_url().to_string();
    let creds = Credentials::default();

    let full = api::ptz_node_for_profile(&addr, &creds, "Profile_1")
        .await
        .unwrap();
    let full_limits = api::ptz_absolute_limits(&full);
    assert!(full_limits.pan.is_some(), "head 1 pans");
    assert!(full_limits.tilt.is_some(), "head 1 tilts");
    assert!(full_limits.zoom.is_some(), "head 1 zooms");

    let zoom_only = api::ptz_node_for_profile(&addr, &creds, "Profile_3")
        .await
        .unwrap();
    let zoom_limits = api::ptz_absolute_limits(&zoom_only);
    assert_eq!(zoom_limits.pan, None, "head 2 declares no pan/tilt space");
    assert_eq!(zoom_limits.tilt, None);
    assert!(
        zoom_limits.zoom.is_some(),
        "a zoom-only head still zooms — the whole block must not vanish"
    );
    assert!(!zoom_limits.is_empty());
}

#[tokio::test(flavor = "multi_thread")]
async fn an_absolute_move_is_visible_in_the_next_status_read() {
    let server = MockServer::start().await.expect("mock server boots");
    let addr = server.device_url().to_string();
    let creds = Credentials::default();

    let before = api::ptz_get_status(&addr, &creds, "Profile_1")
        .await
        .expect("GetStatus on a bound profile");

    // Pick a target the head is not already at, so this cannot pass by the
    // device simply not moving.
    let target_pan = match before.pan {
        Some(p) if p > 0.0 => -0.5,
        _ => 0.5,
    };

    api::ptz_absolute_move(&addr, &creds, "Profile_1", target_pan, 0.25, 0.75)
        .await
        .expect("AbsoluteMove within the declared range");

    let after = api::ptz_get_status(&addr, &creds, "Profile_1")
        .await
        .expect("GetStatus after the move");

    assert_eq!(
        after.pan,
        Some(target_pan),
        "the head did not end up where it was sent"
    );
    assert_eq!(after.tilt, Some(0.25));
    assert_eq!(after.zoom, Some(0.75));

    // And the other head must not have followed it. This is the assertion that
    // makes the whole file a *multi-head* test rather than a round-trip one.
    let other = api::ptz_get_status(&addr, &creds, "Profile_3")
        .await
        .expect("GetStatus on the second head");
    assert_ne!(
        other.pan,
        Some(target_pan),
        "moving head 1 moved head 2 — the mock is not discriminating and \
         nothing above this line proves anything"
    );
}
