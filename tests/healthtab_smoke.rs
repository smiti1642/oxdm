//! End-to-end smoke test for oxdm's healthcheck wiring against the oxvif
//! mock ONVIF server.
//!
//! Proves four things at once:
//!  1. `oxvif::mock::MockServer` boots and answers SOAP requests.
//!  2. `oxvif::HealthCheck` runs against it without failures (Profile S/T
//!     must not be Unsupported — the mock advertises all the services).
//!  3. `HealthReport::to_json` / `to_json_pretty` round-trip cleanly back
//!     into a structurally identical `HealthReport` — the contract oxdm's
//!     baseline persistence relies on.
//!  4. `HealthReport::diff` is reflexive: a report compared to itself
//!     yields an empty `ReportDiff`.
//!
//! Running:
//! ```
//! cargo test --test healthtab_smoke
//! ```

use oxvif::health::{CheckStatus, HealthCheck, ProfileVerdict};
use oxvif::mock::MockServer;
use oxvif::HealthReport;

#[path = "../src/api.rs"]
#[allow(dead_code, unused_imports)]
mod api;
#[path = "../src/sessions.rs"]
#[allow(dead_code, unused_imports)]
mod sessions;
#[path = "../src/state.rs"]
#[allow(dead_code, unused_imports)]
mod state;

#[tokio::test(flavor = "multi_thread")]
async fn application_health_checks_fetch_snapshot_bytes() {
    let server = MockServer::start().await.unwrap();
    let report = api::run_health_check(
        server.device_url(),
        &state::Credentials::default(),
        false,
        false,
        false,
    )
    .await;
    let snapshot = report
        .checks
        .iter()
        .find(|c| c.id == "get_snapshot_uri")
        .unwrap();
    assert!(matches!(snapshot.status, CheckStatus::Pass), "{snapshot:?}");
    assert!(snapshot.detail.contains("image signature"));
    assert!(snapshot.detail.contains("full decoding not tested"));
}

/// Real HTTP snapshot endpoints, reached through OxDM's liveness-enabled wrapper.
/// The replay changes only GetSnapshotUri; all other SOAP reads use the mock.
#[tokio::test(flavor = "multi_thread")]
async fn application_health_reports_bad_images_and_does_not_downgrade_digest() {
    use oxvif::metamorph::FixtureStore;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let seed = MockServer::start().await.unwrap();
    let session = oxvif::OnvifSession::builder(seed.device_url())
        .build()
        .await
        .unwrap();
    let token = session.get_profiles().await.unwrap()[0].token.clone();
    for authentication_failure in [false, true] {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/image", listener.local_addr().unwrap());
        let (stop, mut stopped) = tokio::sync::oneshot::channel();
        let endpoint = tokio::spawn(async move {
            let mut requests = Vec::new();
            loop {
                let (mut socket, _) = tokio::select! {
                    _ = &mut stopped => break,
                    accepted = listener.accept() => accepted.unwrap(),
                };
                let mut request = Vec::new();
                loop {
                    let mut buffer = [0; 1024];
                    let count = socket.read(&mut buffer).await.unwrap();
                    if count == 0 {
                        break;
                    }
                    request.extend_from_slice(&buffer[..count]);
                    if request.windows(4).any(|w| w == b"\r\n\r\n") {
                        break;
                    }
                    assert!(request.len() < 16384);
                }
                requests.push(String::from_utf8(request).unwrap());
                let response = if authentication_failure {
                    "HTTP/1.1 401 Unauthorized\r\nWWW-Authenticate: Digest realm=\"camera\", nonce=\"fixed\", algorithm=MD5, qop=\"auth\"\r\nWWW-Authenticate: Basic realm=\"camera\"\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                } else {
                    "HTTP/1.1 200 OK\r\nContent-Type: image/jpeg\r\nContent-Length: 4\r\nConnection: close\r\n\r\nHTML"
                };
                socket.write_all(response.as_bytes()).await.unwrap();
            }
            requests
        });
        let mut store = FixtureStore::new("snapshot-fixture");
        let req = oxvif::soap::SoapEnvelope::new(format!(
            "<trt:GetSnapshotUri><trt:ProfileToken>{token}</trt:ProfileToken></trt:GetSnapshotUri>"
        ))
        .with_wsa_to("http://recorded.invalid/onvif/media")
        .with_security(oxvif::soap::WsSecurityToken::from_parts(
            "test",
            "digest",
            "nonce",
            "2000-01-01T00:00:00Z",
        ))
        .build();
        let resp = oxvif::soap::SoapEnvelope::new(format!("<trt:GetSnapshotUriResponse xmlns:trt='http://www.onvif.org/ver10/media/wsdl' xmlns:tt='http://www.onvif.org/ver10/schema'><trt:MediaUri><tt:Uri>{url}</tt:Uri><tt:InvalidAfterConnect>false</tt:InvalidAfterConnect><tt:InvalidAfterReboot>false</tt:InvalidAfterReboot><tt:Timeout>PT0S</tt:Timeout></trt:MediaUri></trt:GetSnapshotUriResponse>")).build();
        store.record(
            "http://www.onvif.org/ver10/media/wsdl/GetSnapshotUri",
            &req,
            &resp,
        );
        let server = MockServer::builder().replay(store).start().await.unwrap();
        let credentials = state::Credentials {
            username: "test".into(),
            password: "test".into(),
        };
        let report =
            api::run_health_check(server.device_url(), &credentials, false, false, false).await;
        stop.send(()).unwrap();
        let requests = endpoint.await.unwrap();
        assert_eq!(requests.len(), if authentication_failure { 2 } else { 1 });
        let snapshot = report
            .checks
            .iter()
            .find(|c| c.id == "get_snapshot_uri")
            .unwrap();
        assert!(
            matches!(snapshot.status, CheckStatus::Warn(_)),
            "{snapshot:?}"
        );
        if authentication_failure {
            assert!(requests[1].contains("Authorization: Digest "));
            assert!(requests[1].contains("qop=auth"));
            assert!(!requests[1].contains("qop=\"auth\""));
            assert!(requests
                .iter()
                .all(|r| !r.contains("Authorization: Basic ")));
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn healthcheck_against_mock_round_trips_json_and_diffs_clean() {
    let server = MockServer::start().await.expect("mock server boots");

    let report = HealthCheck::new(server.device_url()).run().await;

    // Mock is a known-good target — no check should fail outright.
    let fails: Vec<&str> = report
        .checks
        .iter()
        .filter(|c| matches!(c.status, CheckStatus::Fail(_)))
        .map(|c| c.id.as_str())
        .collect();
    assert!(
        fails.is_empty(),
        "mock health check had failures: {fails:?}\n{report}"
    );

    // The mock advertises Media/Imaging/PTZ/Events — neither Profile S
    // nor Profile T should come back Unsupported.
    assert_ne!(
        report.profiles.profile_s.verdict,
        ProfileVerdict::Unsupported
    );
    assert_ne!(
        report.profiles.profile_t.verdict,
        ProfileVerdict::Unsupported
    );

    // JSON round-trip (the contract oxdm's persist::write_baseline /
    // persist::read_baseline depends on).
    let json = report.to_json();
    let rehydrated: HealthReport =
        serde_json::from_str(&json).expect("HealthReport JSON round-trips");
    assert_eq!(rehydrated.target, report.target);
    assert_eq!(rehydrated.checks.len(), report.checks.len());
    assert_eq!(
        rehydrated.profiles.profile_s.verdict,
        report.profiles.profile_s.verdict
    );

    // Pretty form parses to the same value.
    let pretty = report.to_json_pretty();
    let reparsed_pretty: HealthReport =
        serde_json::from_str(&pretty).expect("pretty JSON also parses");
    assert_eq!(reparsed_pretty.checks.len(), report.checks.len());

    // diff is reflexive — comparing a report against itself yields an
    // empty diff. (Mock state is deterministic, so this is also the
    // "no regression detected" baseline behaviour oxdm shows.)
    let diff = report.diff(&report);
    assert!(diff.is_empty(), "self-diff should be empty, got {diff:?}");
}
