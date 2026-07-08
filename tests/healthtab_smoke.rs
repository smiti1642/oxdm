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
