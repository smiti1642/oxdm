//! Process-wide pool of running clone **replay servers** (metamorph "container").
//!
//! A recorded camera clone ([`oxvif::metamorph::FixtureStore`]) is served from a
//! real bound-port [`MockServer`] so the rest of oxdm can drive it exactly like a
//! live device — the served URL goes into a virtual [`DeviceEntry`], and every
//! existing view works against it unchanged.
//!
//! [`MockServer`] shuts itself down on `Drop`, so a clone only stays reachable
//! while its handle is *held*. This module is that holder: one `OnceLock`
//! singleton keyed by a clone label, mirroring [`crate::sessions`]'s session
//! pool. [`serve`] starts (or replaces) a clone's server; [`stop`] drops it,
//! shutting the server down.
//!
//! # Two pools
//!
//! Serving a clone and *analysing* one are separate concerns. The analyses —
//! [`quirks`], [`details`], [`parse_report`] — need nothing but a
//! [`FixtureStore`], so they read a second pool keyed by **device address**:
//!
//! - [`serve`] puts the served clone's store in it (keyed by the served URL),
//!   which is what makes the Quirks view work for a replayed clone.
//! - [`set_analysis`] puts a *live* camera's freshly recorded store in it
//!   (keyed by the camera's own address) — see
//!   [`crate::api::run_live_quirk_test`]. No server is involved, so a quirk
//!   test can run straight against a camera without cloning-then-serving it.
//!
//! [`DeviceEntry`]: crate::state::DeviceEntry

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use oxvif::metamorph::{
    FixtureStore, OpOutcome, OperationDiff, ParseReport, QuirkReport, SurfaceOp,
};
use oxvif::mock::MockServer;

/// A served clone: its bound replay server. The material the Quirks view needs
/// lives in the [`ANALYSES`] pool, which [`serve`] populates in step.
struct Served {
    /// Held only to keep the bound server alive; dropped (→ shutdown) by `stop`.
    #[allow(dead_code)]
    server: MockServer,
    /// The clone's label, for [`active_labels`].
    label: String,
}

/// Everything needed to analyse one device's recorded read surface.
struct Analysis {
    /// The recorded exchanges the reports are derived from.
    store: FixtureStore,
    /// Per-operation sweep outcomes — empty for a served clone (which was not
    /// swept here), populated by a live test so the UI can tell "this camera
    /// has no PTZ" ([`OpOutcome::SkippedNoData`]) from "the command broke"
    /// ([`OpOutcome::Failed`]).
    sweep: HashMap<SurfaceOp, OpOutcome>,
}

/// Running clone servers, keyed by their served device-service URL — the same
/// value that goes into the virtual `DeviceEntry.addr`, so removal can stop the
/// server by `addr`.
static SERVERS: OnceLock<Mutex<HashMap<String, Served>>> = OnceLock::new();

/// Analysis material, keyed by device address (`DeviceEntry.addr`): a served
/// clone's URL, or a live camera's own device-service URL.
static ANALYSES: OnceLock<Mutex<HashMap<String, Analysis>>> = OnceLock::new();

fn servers() -> &'static Mutex<HashMap<String, Served>> {
    SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn analyses() -> &'static Mutex<HashMap<String, Analysis>> {
    ANALYSES.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Start a bound replay server for `store` and return its device-service URL
/// (`http://127.0.0.1:<port>/onvif/device`). The server is held alive in the
/// pool — keyed by that URL — until [`stop`], along with the clone's structural
/// [`quirks`]. Reads replay the clone's recorded responses; writes and
/// unrecorded operations fall to synthetic device state, so `Set → Get` still
/// round-trips.
pub async fn serve(store: FixtureStore) -> std::io::Result<String> {
    // Keep a clone for the analysis pool before the store moves into replay.
    let view = store.clone();
    let label = store.device().to_string();
    let server = MockServer::builder().replay(store).start().await?;
    let url = server.device_url().to_string();
    // A served clone was not swept here, so it carries no per-op outcomes.
    set_analysis(&url, view, HashMap::new());
    servers()
        .lock()
        .unwrap()
        .insert(url.clone(), Served { server, label });
    Ok(url)
}

/// Stop and drop the replay server serving `url`, if one is running (Drop shuts
/// the bound port down), and drop its analysis. Call this when a clone device is
/// removed.
pub fn stop(url: &str) {
    servers().lock().unwrap().remove(url);
    forget(url);
}

/// Labels (the `FixtureStore::device` of each served clone) of every clone
/// currently running — so the UI can hide saved clones that are already active.
pub fn active_labels() -> Vec<String> {
    servers()
        .lock()
        .unwrap()
        .values()
        .map(|s| s.label.clone())
        .collect()
}

/// Publish (or replace) the analysis material for the device at `addr`: the
/// recorded exchanges plus the sweep's per-operation outcomes. This is what
/// makes [`quirks`] / [`details`] / [`parse_report`] answer for `addr`.
pub fn set_analysis(addr: &str, store: FixtureStore, sweep: HashMap<SurfaceOp, OpOutcome>) {
    analyses()
        .lock()
        .unwrap()
        .insert(addr.to_string(), Analysis { store, sweep });
}

/// Run a quirk/parse test against the **live** camera at `addr` and publish the
/// result here, so [`quirks`] / [`details`] / [`parse_report`] /
/// [`sweep_outcomes`] answer for the camera itself. No replay server is started.
///
/// The UI's single entry point for the live path — [`crate::api::run_live_quirk_test`]
/// does the work and reports `progress`, this adds the pool insert (`api.rs`
/// cannot reference this module; see that function's docs).
#[allow(dead_code)] // Driven by the Quirks view's "test this camera" action.
pub async fn run_live_test(
    addr: &str,
    creds: &crate::state::Credentials,
    label: &str,
    selection: &oxvif::metamorph::SurfaceSelection,
    progress: impl Fn(crate::api::LiveTestProgress) + Send + Sync,
) -> Result<(), crate::api::ApiError> {
    let (store, report) =
        crate::api::run_live_quirk_test(addr, creds, label, selection, progress).await?;
    // The store is published only after the awaits above have all completed —
    // the pool lock is taken and released inside `set_analysis`, never held
    // across one.
    set_analysis(addr, store, report.entries().into_iter().collect());
    Ok(())
}

/// Drop the analysis material for `addr`. Call this when a device is removed or
/// before re-testing it; [`stop`] already does it for a served clone.
pub fn forget(addr: &str) {
    analyses().lock().unwrap().remove(addr);
}

/// The sweep's per-operation outcomes for `addr`, sorted by operation — empty
/// for a device that was never swept (a served clone, or an unknown address).
///
/// The UI renders [`OpOutcome::SkippedNoData`] ("this camera has no such path")
/// differently from [`OpOutcome::Failed`] ("the command broke").
#[allow(dead_code)] // Read by the Quirks view's live-test result table.
pub fn sweep_outcomes(addr: &str) -> Vec<(SurfaceOp, OpOutcome)> {
    let mut v: Vec<_> = analyses()
        .lock()
        .unwrap()
        .get(addr)
        .map(|a| a.sweep.iter().map(|(&op, &o)| (op, o)).collect())
        .unwrap_or_default();
    v.sort_by_key(|(op, _)| *op);
    v
}

/// The structural quirk report for the device at `addr`, if it has been
/// recorded (served clone or live test).
pub fn quirks(addr: &str) -> Option<QuirkReport> {
    analyses()
        .lock()
        .unwrap()
        .get(addr)
        .map(|a| a.store.diff_against_synthetic())
}

/// Per-operation side-by-side diff material (baseline vs recorded XML) for the
/// device at `addr`, if it has been recorded.
pub fn details(addr: &str) -> Option<Vec<OperationDiff>> {
    analyses()
        .lock()
        .unwrap()
        .get(addr)
        .map(|a| a.store.diff_details())
}

/// The parse-verification report for the device at `addr`, if it has been
/// recorded — runs oxvif's own typed parser over each recorded response (the
/// value/type half of the quirk diff, joined to [`quirks`]/[`details`] by
/// `key_canon`).
///
/// Async: the store is cloned out from under the pool lock, then verified with
/// no lock held.
pub async fn parse_report(addr: &str) -> Option<ParseReport> {
    let store = analyses()
        .lock()
        .unwrap()
        .get(addr)
        .map(|a| a.store.clone())?;
    Some(store.verify_parsing().await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api;
    use crate::state::Credentials;
    use oxvif::metamorph::SurfaceSelection;
    use oxvif::mock::MockServer;
    use std::sync::Arc;

    /// The whole oxdm clone loop end-to-end, through the app-layer wrappers:
    /// record a (bound-mock) "camera" via `api::record_clone`, serve the clone
    /// from the pool, drive it over real HTTP, diff it, then stop it.
    #[tokio::test]
    async fn record_serve_drive_diff_and_stop() {
        // A bound mock standing in for a real camera, with a distinctive
        // hostname so a replayed GetHostname is unmistakable.
        let real = MockServer::start().await.expect("start mock camera");
        real.device()
            .modify(|s| s.hostname = "smoke-real-host".into());

        // Record its standard read surface through the oxdm api wrapper.
        let store = api::record_clone(real.device_url(), &Credentials::default(), "smoke-clone")
            .await
            .expect("record clone");
        assert!(store.len() >= 2, "expected several recorded reads");

        // Serve the clone from the pool and drive it over real HTTP.
        let url = serve(store).await.expect("serve clone");

        // The served clone exposes both the structural report and the
        // per-operation side-by-side diff material.
        let report = quirks(&url).expect("served clone exposes a quirk report");
        assert!(report.compared >= 2, "diff should compare the recorded ops");
        let details = details(&url).expect("served clone exposes diff details");
        assert_eq!(details.len(), report.compared, "one detail per recorded op");
        assert!(
            details.iter().all(|d| d.baseline_xml.contains('\n')),
            "each detail carries multi-line pretty XML"
        );

        // Parse verification runs oxvif's own parser over the recorded
        // responses; a clone of oxvif's own mock must parse cleanly.
        let parse = parse_report(&url)
            .await
            .expect("served clone exposes a parse report");
        assert!(!parse.verdicts.is_empty(), "parse report covers the reads");
        assert!(
            parse.all_parsed(),
            "oxvif should parse its own mock's responses: {:?}",
            parse.failures().collect::<Vec<_>>()
        );

        let client = oxvif::OnvifClient::new(&url);
        let host = client.get_hostname().await.expect("get_hostname on clone");
        assert_eq!(
            host.name.as_deref(),
            Some("smoke-real-host"),
            "the clone must replay the recorded hostname"
        );

        // Stopping drops the server; the URL is gone from the pool.
        stop(&url);
        assert!(
            quirks(&url).is_none(),
            "a stopped clone should be gone from the pool"
        );
    }

    /// The live path: run a quirk test straight against a (bound-mock) camera
    /// and assert every analysis answers for the camera's own address — with
    /// `serve` never called. That is the whole point of the feature.
    #[tokio::test]
    async fn live_test_analyses_a_camera_without_serving_it() {
        let real = MockServer::start().await.expect("start mock camera");
        real.device()
            .modify(|s| s.hostname = "live-real-host".into());
        let addr = real.device_url().to_string();

        // A partial surface: two picks, one of which drags in a prerequisite.
        let selection = SurfaceSelection::none()
            .with(SurfaceOp::GetHostname)
            .with(SurfaceOp::GetStreamUri);
        assert_eq!(selection.len(), 2, "the user ticked two operations");

        let seen: Arc<Mutex<Vec<api::LiveTestProgress>>> = Arc::new(Mutex::new(Vec::new()));
        let sink = Arc::clone(&seen);
        run_live_test(
            &addr,
            &Credentials::default(),
            "live-test",
            &selection,
            move |p| sink.lock().unwrap().push(p),
        )
        .await
        .expect("live quirk test");

        // ── the point: analyses answer for the live address, unserved ────────
        assert!(
            !active_labels().iter().any(|l| l == "live-test"),
            "the live path must not start a replay server"
        );
        let report = quirks(&addr).expect("live device exposes a quirk report");
        assert!(report.compared >= 2, "the recorded reads were compared");
        let details = details(&addr).expect("live device exposes diff details");
        assert_eq!(details.len(), report.compared, "one detail per recorded op");
        let parse = parse_report(&addr)
            .await
            .expect("live device exposes a parse report");
        assert!(
            parse.all_parsed(),
            "oxvif should parse its own mock's responses: {:?}",
            parse.failures().collect::<Vec<_>>()
        );

        // Per-op outcomes: the picks plus the auto-added prerequisite.
        let outcomes = sweep_outcomes(&addr);
        assert_eq!(outcomes.len(), 3, "picks + GetProfiles prerequisite");
        for (op, outcome) in &outcomes {
            assert_eq!(*outcome, OpOutcome::Recorded, "{op:?} should be recorded");
        }
        assert!(outcomes.iter().any(|(op, _)| *op == SurfaceOp::GetProfiles));

        // ── progress ────────────────────────────────────────────────────────
        let seen = Arc::try_unwrap(seen)
            .expect("sole owner")
            .into_inner()
            .unwrap();
        assert_eq!(
            seen.first().map(|p| (p.phase, p.total)),
            Some((api::LiveTestPhase::Connecting, None)),
            "the run opens indeterminate, before any total is knowable"
        );
        // Each counted phase: total is fixed, `done` is monotonic and lands on it.
        for phase in [
            api::LiveTestPhase::Sweep,
            api::LiveTestPhase::Verifying,
            api::LiveTestPhase::Diffing,
        ] {
            let events: Vec<_> = seen.iter().filter(|p| p.phase == phase).collect();
            assert!(!events.is_empty(), "{phase:?} reported no progress");
            let total = events[0].total.expect("a counted phase knows its total");
            let expected = match phase {
                // Selected operations after prerequisite expansion.
                api::LiveTestPhase::Sweep => 3,
                // One event per recorded fixture.
                _ => report.compared,
            };
            assert_eq!(total, expected, "{phase:?} total");
            assert!(
                events.iter().all(|p| p.total == Some(total)),
                "{phase:?} total must not move mid-phase"
            );
            assert!(
                events.windows(2).all(|w| w[0].done <= w[1].done),
                "{phase:?} done must be non-decreasing: {events:?}"
            );
            assert_eq!(
                events.last().map(|p| p.done),
                Some(total),
                "{phase:?} must reach its total"
            );
            assert!(
                events.iter().all(|p| !p.label.is_empty()),
                "{phase:?} events carry a display label"
            );
        }

        // Removing the device drops its analysis, no server involved.
        forget(&addr);
        assert!(quirks(&addr).is_none(), "forgetting drops the analysis");
        assert!(sweep_outcomes(&addr).is_empty());
    }
}
