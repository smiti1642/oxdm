//! End-to-end smoke test for oxdm's IO Control wiring against the oxvif
//! mock ONVIF server.
//!
//! Proves three things:
//!  1. `api::get_relay_outputs` / `api::get_digital_inputs` parse the mock's
//!     stateful response (oxvif 0.9.9 surface).
//!  2. `api::set_relay_output_state` / `api::set_relay_output_settings` flow
//!     through the session cache and the mock's state-mutating handlers
//!     reflect the change on the next get.
//!  3. The `/mock/digital-input/:token/pulse` REST hook is reachable from
//!     a process that already holds an `OnvifSession` to the same mock —
//!     i.e. callers can flip inputs without leaving the test harness.

use oxvif::mock::MockServer;

#[path = "../src/sessions.rs"]
#[allow(dead_code, unused_imports)]
mod sessions;
#[path = "../src/state.rs"]
#[allow(dead_code, unused_imports)]
mod state;

// Bring in the api wrappers we want to exercise via path inclusion. The
// rest of oxdm (Dioxus components, persist, etc.) doesn't load — only
// api.rs and its hard deps below.
#[path = "../src/api.rs"]
#[allow(dead_code, unused_imports)]
mod api;

use crate::state::Credentials;

#[tokio::test(flavor = "multi_thread")]
async fn io_control_round_trip_against_mock() {
    let server = MockServer::start().await.expect("mock server boots");
    let addr = server.device_url().to_string();
    let creds = Credentials::default();

    // Defaults — two of each.
    let relays = api::get_relay_outputs(&addr, &creds).await.unwrap();
    assert_eq!(relays.len(), 2);
    assert!(relays.iter().any(|r| r.token == "RelayOutput_1"));

    let inputs = api::get_digital_inputs(&addr, &creds).await.unwrap();
    assert_eq!(inputs.len(), 2);
    assert!(inputs.iter().any(|d| d.token == "DigitalInput_1"));

    // Flip relay logical state; oxdm's view depends on the SetState path
    // returning OK without error.
    api::set_relay_output_state(&addr, &creds, "RelayOutput_1", "active")
        .await
        .unwrap();
    assert_eq!(
        server
            .device()
            .read()
            .relay_outputs
            .iter()
            .find(|r| r.token == "RelayOutput_1")
            .unwrap()
            .logical_state,
        "active"
    );

    // Edit settings; next GetRelayOutputs must reflect the change.
    api::set_relay_output_settings(&addr, &creds, "RelayOutput_1", "Monostable", "PT3S", "open")
        .await
        .unwrap();
    let after = api::get_relay_outputs(&addr, &creds).await.unwrap();
    let r1 = after.iter().find(|r| r.token == "RelayOutput_1").unwrap();
    assert_eq!(r1.mode, "Monostable");
    assert_eq!(r1.delay_time, "PT3S");
    assert_eq!(r1.idle_state, "open");

    // REST simulator hook reachable from the same test process.
    let resp = reqwest::Client::new()
        .post(format!(
            "{}/mock/digital-input/DigitalInput_1/pulse",
            server.base_url()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    // Two events should be queued.
    assert!(server.device().read().pending_io_events.len() >= 2);
}
