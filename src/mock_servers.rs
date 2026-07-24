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

use oxvif::metamorph::FixtureStore;
use oxvif::mock::MockServer;

/// Running clone servers, keyed by clone label (e.g. `"hikvision-ds2cd"`).
static SERVERS: OnceLock<Mutex<HashMap<String, MockServer>>> = OnceLock::new();

fn servers() -> &'static Mutex<HashMap<String, MockServer>> {
    SERVERS.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Start (or restart) a bound replay server for `label`, serving `store`, and
/// return its device-service URL (`http://127.0.0.1:<port>/onvif/device`).
///
/// The server is held alive in the pool until [`stop`]; a second call for the
/// same `label` replaces (and shuts down) the previous one. Reads replay the
/// clone's recorded responses; writes and unrecorded operations fall to
/// synthetic device state, so `Set → Get` still round-trips.
pub async fn serve(label: &str, store: FixtureStore) -> std::io::Result<String> {
    let server = MockServer::builder().replay(store).start().await?;
    let url = server.device_url().to_string();
    // Replacing an existing entry drops the old server (Drop → shutdown).
    servers().lock().unwrap().insert(label.to_string(), server);
    Ok(url)
}

/// Stop and drop the replay server for `label`, if one is running.
pub fn stop(label: &str) {
    servers().lock().unwrap().remove(label);
}

/// The served URL for `label`, if its server is currently running.
pub fn url_for(label: &str) -> Option<String> {
    servers()
        .lock()
        .unwrap()
        .get(label)
        .map(|s| s.device_url().to_string())
}

/// Labels of every clone server currently running.
pub fn running() -> Vec<String> {
    servers().lock().unwrap().keys().cloned().collect()
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

        // Structural quirk diff runs (clone == synthetic here, so no drift, but
        // the call path is exercised).
        let report = api::quirk_diff(&store);
        assert!(report.compared >= 2, "diff should compare the recorded ops");

        // Serve the clone from the pool and drive it over real HTTP.
        let url = serve("smoke-clone", store).await.expect("serve clone");
        assert_eq!(url_for("smoke-clone").as_deref(), Some(url.as_str()));
        assert!(running().contains(&"smoke-clone".to_string()));

        let client = oxvif::OnvifClient::new(&url);
        let host = client.get_hostname().await.expect("get_hostname on clone");
        assert_eq!(
            host.name.as_deref(),
            Some("smoke-real-host"),
            "the clone must replay the recorded hostname"
        );

        // Stopping drops the server; the label is gone from the pool.
        stop("smoke-clone");
        assert!(url_for("smoke-clone").is_none());
    }
}
