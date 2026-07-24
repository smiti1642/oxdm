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
//! [`DeviceEntry`]: crate::state::DeviceEntry

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use oxvif::metamorph::{FixtureStore, OperationDiff, QuirkReport};
use oxvif::mock::MockServer;

/// A served clone: its bound replay server plus a clone of the fixture store, so
/// the Quirks view can derive both the structural report and the per-operation
/// side-by-side diff on demand.
struct Served {
    /// Held only to keep the bound server alive; dropped (→ shutdown) by `stop`.
    #[allow(dead_code)]
    server: MockServer,
    store: FixtureStore,
}

/// Running clone servers, keyed by their served device-service URL — the same
/// value that goes into the virtual `DeviceEntry.addr`, so removal can stop the
/// server by `addr`.
static SERVERS: OnceLock<Mutex<HashMap<String, Served>>> = OnceLock::new();

fn servers() -> &'static Mutex<HashMap<String, Served>> {
    SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Start a bound replay server for `store` and return its device-service URL
/// (`http://127.0.0.1:<port>/onvif/device`). The server is held alive in the
/// pool — keyed by that URL — until [`stop`], along with the clone's structural
/// [`quirks`]. Reads replay the clone's recorded responses; writes and
/// unrecorded operations fall to synthetic device state, so `Set → Get` still
/// round-trips.
pub async fn serve(store: FixtureStore) -> std::io::Result<String> {
    // Keep a clone for the Quirks view before the store moves into replay.
    let view = store.clone();
    let server = MockServer::builder().replay(store).start().await?;
    let url = server.device_url().to_string();
    servers().lock().unwrap().insert(
        url.clone(),
        Served {
            server,
            store: view,
        },
    );
    Ok(url)
}

/// Stop and drop the replay server serving `url`, if one is running (Drop shuts
/// the bound port down). Call this when a clone device is removed.
pub fn stop(url: &str) {
    servers().lock().unwrap().remove(url);
}

/// The structural quirk report for the clone served at `url`, if running.
pub fn quirks(url: &str) -> Option<QuirkReport> {
    servers()
        .lock()
        .unwrap()
        .get(url)
        .map(|s| s.store.diff_against_synthetic())
}

/// Per-operation side-by-side diff material (baseline vs clone XML) for the
/// clone served at `url`, if running.
pub fn details(url: &str) -> Option<Vec<OperationDiff>> {
    servers()
        .lock()
        .unwrap()
        .get(url)
        .map(|s| s.store.diff_details())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api;
    use crate::state::Credentials;
    use oxvif::mock::MockServer;

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
}
