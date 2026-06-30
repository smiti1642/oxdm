//! End-to-end smoke test for oxdm's Profile G (recording playback) wiring
//! against the oxvif mock ONVIF server.
//!
//! Proves the Phase 1 api layer:
//!  1. `api::search_recordings` runs the find → poll → end search session and
//!     parses the mock's `RecordingInformation` (token, source, time bounds,
//!     status).
//!  2. `api::get_replay_uri` resolves an RTSP replay URI for a recording token
//!     through the session cache.

use oxvif::mock::MockServer;

#[path = "../src/sessions.rs"]
#[allow(dead_code, unused_imports)]
mod sessions;
#[path = "../src/state.rs"]
#[allow(dead_code, unused_imports)]
mod state;

#[path = "../src/api.rs"]
#[allow(dead_code, unused_imports)]
mod api;

use crate::state::Credentials;

#[tokio::test(flavor = "multi_thread")]
async fn recordings_search_and_replay_against_mock() {
    let server = MockServer::start().await.expect("mock server boots");
    let addr = server.device_url().to_string();
    let creds = Credentials::default();

    // Search returns the mock's single Rec_001 with parsed time bounds.
    let recordings = api::search_recordings(&addr, &creds).await.unwrap();
    assert!(!recordings.is_empty());
    let rec = recordings
        .iter()
        .find(|r| r.recording_token == "Rec_001")
        .expect("Rec_001 present");
    assert_eq!(
        rec.earliest_recording.as_deref(),
        Some("2026-01-01T00:00:00Z")
    );
    assert_eq!(rec.recording_status, "Stopped");

    // Replay URI resolves for that token.
    let uri = api::get_replay_uri(&addr, &creds, "Rec_001").await.unwrap();
    assert_eq!(uri, "rtsp://127.0.0.1:554/mock/replay/Rec_001");
}
