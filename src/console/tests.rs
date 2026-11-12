use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::{sleep, timeout},
};

use crate::state::Credentials;

use super::{
    grammar::{normalize_target, shell_command, Camera, DynamicValues},
    process::{load_grammar, Controller, OutputKind, Snapshot},
};

#[test]
fn direct_targets_keep_https_ipv6_port_and_service_path() {
    assert_eq!(
        normalize_target("https://[::1]:8443/camera/service").as_deref(),
        Some("https://[::1]:8443/camera/service")
    );
    assert_eq!(
        normalize_target("2001:db8::1").as_deref(),
        Some("http://[2001:db8::1]/onvif/device_service")
    );
    assert!(normalize_target("https://user:secret@camera/service").is_none());
}

fn executable() -> PathBuf {
    std::env::var_os("OXDM_OXVIF_TEST")
        .expect("set OXDM_OXVIF_TEST to the absolute path of the compatible CLI")
        .into()
}

fn mock_executable() -> PathBuf {
    std::env::var_os("OXDM_MOCK_EVENTS_TEST")
        .expect("set OXDM_MOCK_EVENTS_TEST to the matching console_mock fixture")
        .into()
}

async fn mock_camera(arguments: &[&str]) -> (Child, Camera) {
    use std::process::Stdio;
    let mut child = Command::new(mock_executable())
        .args(arguments)
        .kill_on_drop(true)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut address = String::new();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    timeout(Duration::from_secs(5), stdout.read_line(&mut address))
        .await
        .unwrap()
        .unwrap();
    let address = address.trim().to_owned();
    assert!(address.starts_with("http://127.0.0.1:"));
    (
        child,
        Camera {
            name: "Test camera".into(),
            address,
            credentials: Credentials::default(),
        },
    )
}

async fn wait_for(controller: &Controller, predicate: impl Fn(&Snapshot) -> bool) -> Snapshot {
    timeout(Duration::from_secs(12), async {
        let mut revision = 0;
        loop {
            if let Some(state) = controller.snapshot_after(revision) {
                revision = state.revision;
                if predicate(&state) {
                    return state;
                }
                assert!(
                    state.finished.is_none(),
                    "CLI stopped before reaching expected state: {}",
                    state
                        .lines
                        .iter()
                        .map(|line| line.text.as_str())
                        .collect::<Vec<_>>()
                        .join("\n")
                );
            }
            sleep(Duration::from_millis(15)).await;
        }
    })
    .await
    .expect("expected CLI state did not arrive")
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn watch_waits_for_the_events_view_lease_and_can_stop_while_waiting() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_mock, camera) = mock_camera(&[]).await;
    let controller = Controller::default();
    let subscriptions = controller.event_subscriptions();
    let permit = subscriptions.acquire(&camera.address).await;
    let view = super::ViewSubscription::create(
        camera.address.clone(),
        camera.credentials.clone(),
        None,
        "PT60S",
        permit,
    )
    .await
    .unwrap();
    let old_url = view.url.clone();
    let pull_previous = || {
        crate::api::pull_event_messages(&camera.address, &camera.credentials, &old_url, "PT1S", 1)
    };
    pull_previous().await.unwrap();

    let input = "events watch --duration 30s";
    let prepare = || {
        grammar
            .prepare(executable.clone(), input, Some(&camera))
            .unwrap()
    };
    assert!(controller.start(prepare(), input.into(), None));
    let waiting = wait_for(&controller, |state| state.waiting_for_subscription).await;
    assert_eq!(waiting.events, 0);
    controller.stop();
    let stopped = wait_for(&controller, |state| state.finished.is_some()).await;
    assert!(!stopped.waiting_for_subscription);
    let finished = stopped.finished.unwrap();
    assert!(!finished.started);
    assert!(finished.error.is_none());
    pull_previous().await.unwrap();

    assert!(controller.start(prepare(), input.into(), None));
    assert!(timeout(
        Duration::from_millis(150),
        wait_for(&controller, |state| !state.waiting_for_subscription),
    )
    .await
    .is_err());
    drop(view);
    wait_for(&controller, |state| state.events > 0).await;
    assert!(
        pull_previous().await.is_err(),
        "the events view lease is still active"
    );
    assert!(timeout(
        Duration::from_millis(50),
        subscriptions.acquire(&camera.address)
    )
    .await
    .is_err());
    controller.stop();
    let finished = wait_for(&controller, |state| state.finished.is_some()).await;
    assert!(!finished.summary.unwrap().cleanup_failed);
    let _permit = timeout(
        Duration::from_secs(2),
        subscriptions.acquire(&camera.address),
    )
    .await
    .expect("the console did not release the subscription permit after stopping");
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "requires the bundled CLI, its matching console_mock fixture and Python"]
async fn bundled_watch_launch_has_no_visible_console_window() {
    use std::process::Stdio;

    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_mock, camera) = mock_camera(&[]).await;
    let invocation = grammar
        .prepare(executable, "events watch --duration 30s", Some(&camera))
        .unwrap();
    let mut child = super::process::command(&invocation.executable)
        .args(invocation.argv)
        .arg(invocation.control.unwrap().flag)
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    let pid = child.id().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap()).lines();
    let record = timeout(Duration::from_secs(5), stdout.next_line())
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let record: serde_json::Value = serde_json::from_str(&record).unwrap();
    assert_eq!(record["data"]["kind"], "event_notification");

    // Detach only the probe from its console. A console may have a window
    // handle without displaying a window, so check the window's visibility.
    let probe = r#"
import ctypes, sys
from ctypes import wintypes
kernel = ctypes.WinDLL('kernel32', use_last_error=True)
kernel.GetConsoleWindow.restype = wintypes.HWND
visible = ctypes.WinDLL('user32').IsWindowVisible
visible.argtypes = [wintypes.HWND]
kernel.FreeConsole()
if kernel.AttachConsole(int(sys.argv[1])):
    window = kernel.GetConsoleWindow()
    shown = window and visible(window)
    kernel.FreeConsole()
    if shown:
        sys.exit('CLI child has a visible console window')
else:
    error = ctypes.get_last_error()
    if error != 6:
        sys.exit(f'Unexpected AttachConsole error {error}')
"#;
    let result = timeout(
        Duration::from_secs(5),
        super::process::command(std::path::Path::new("python"))
            .args(["-c", probe, &pid.to_string()])
            .stderr(Stdio::piped())
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );

    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"stop\n")
        .await
        .unwrap();
    let summary = timeout(Duration::from_secs(5), async {
        while let Some(line) = stdout.next_line().await.unwrap() {
            let record: serde_json::Value = serde_json::from_str(&line).unwrap();
            if record["data"]["kind"] == "event_watch_summary" {
                return record["data"]["summary"].clone();
            }
        }
        panic!("CLI did not emit a watch summary");
    })
    .await
    .unwrap();
    assert_eq!(summary["reason"], "interrupted");
    assert_eq!(summary["cleanup_failed"], false);
    assert!(timeout(Duration::from_secs(5), child.wait())
        .await
        .unwrap()
        .unwrap()
        .success());
}

#[tokio::test]
#[ignore = "requires OXDM_OXVIF_TEST with the version-matched CLI"]
async fn real_cli_completion_execution_and_graceful_watch() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_mock, camera) = mock_camera(&[]).await;
    let candidates = grammar
        .analyze("ev", 2, &DynamicValues::default())
        .candidates;
    assert!(candidates
        .iter()
        .any(|candidate| candidate.label == "events"));
    let partial = "events watch --output=j --duration 30s";
    let caret = partial.find(" --duration").unwrap();
    let candidates = grammar
        .analyze(partial, caret, &DynamicValues::default())
        .candidates;
    let jsonl = candidates
        .iter()
        .find(|candidate| candidate.label == "jsonl")
        .unwrap();
    assert_eq!(&partial[jsonl.range.clone()], "j");
    assert_eq!(jsonl.replacement, "jsonl");

    let invocation = grammar
        .prepare(executable.clone(), "device info", Some(&camera))
        .unwrap();
    assert_eq!(invocation.target, camera.address);
    let controller = Controller::default();
    assert!(controller.start(invocation, "device info".into(), None));
    let finished = wait_for(&controller, |state| state.finished.is_some()).await;
    assert_eq!(finished.finished.unwrap().exit_code, Some(0));
    assert_eq!(finished.malformed, 0);
    assert!(finished
        .lines
        .iter()
        .any(|line| line.kind == OutputKind::Json));

    let mut other = camera.clone();
    other.credentials = Credentials {
        username: "user".into(),
        password: "not-for-other-target".into(),
    };
    let explicit = grammar
        .prepare(
            executable.clone(),
            "device info --target http://[::1]:8443/custom",
            Some(&other),
        )
        .unwrap();
    assert!(explicit.credentials.is_none());
    assert!(grammar
        .prepare(
            executable.clone(),
            "device info\nmedia profiles",
            Some(&other)
        )
        .is_err());
    assert!(grammar
        .prepare(
            executable.clone(),
            "--device another device info",
            Some(&other)
        )
        .is_err());

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let file =
        std::env::temp_dir().join(format!("oxdm-console-{}-{now}.jsonl", std::process::id()));
    let invocation = grammar
        .prepare(
            executable,
            "events watch --duration 30s --topic VideoSource/MotionAlarm",
            Some(&camera),
        )
        .unwrap();
    assert!(controller.start(invocation, "events watch".into(), Some(file.clone())));
    wait_for(&controller, |state| state.events > 0).await;
    controller.stop();
    let finished = wait_for(&controller, |state| state.finished.is_some()).await;
    assert_eq!(finished.finished.unwrap().exit_code, Some(0));
    assert!(finished.export_error.is_none());
    let summary = finished.summary.unwrap();
    assert_eq!(summary.reason, "interrupted");
    assert!(summary.complete);
    assert!(!summary.cleanup_failed);
    let bytes = std::fs::read(&file).unwrap();
    std::fs::remove_file(file).unwrap();
    let records: Vec<serde_json::Value> = std::str::from_utf8(&bytes)
        .unwrap()
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let (last, events) = records.split_last().unwrap();
    assert!(!events.is_empty());
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["data"]["kind"], "event_notification");
        assert_eq!(event["data"]["sequence"], index + 1);
        assert_eq!(
            event["data"]["event"]["topic"],
            "tns1:VideoSource/MotionAlarm"
        );
    }
    assert_eq!(last["data"]["kind"], "event_watch_summary");
    assert_eq!(
        last["data"]["summary"]["counts"]["delivered"],
        summary.counts.delivered
    );
    assert_eq!(events.len() as u64, summary.counts.delivered);
}

#[tokio::test]
#[ignore = "requires OXDM_OXVIF_TEST and OXDM_MOCK_EVENTS_TEST"]
async fn burst_evicts_display_rows_and_a_failed_export_keeps_the_watch_live() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (mut mock, camera) = mock_camera(&[]).await;
    let invocation = grammar
        .prepare(executable, "events watch --duration 30s", Some(&camera))
        .unwrap();
    let controller = Controller::default();
    assert!(controller.start(
        invocation,
        "events watch".into(),
        Some(std::env::temp_dir())
    ));
    let mut burst = wait_for(&controller, |state| state.events > 0).await;
    for _ in 0..5 {
        let target = burst.events + 96;
        mock.stdin
            .as_mut()
            .unwrap()
            .write_all(b"burst\n")
            .await
            .unwrap();
        burst = wait_for(&controller, |state| state.events >= target).await;
    }
    assert!(burst.evicted > 0);
    assert!(burst.lines.len() <= 250);
    assert!(burst.export_error.is_some());
    mock.stdin
        .as_mut()
        .unwrap()
        .write_all(b"fail_pull\n")
        .await
        .unwrap();
    let finished = wait_for(&controller, |state| state.finished.is_some()).await;
    assert!(finished.gaps > 0);
    assert_eq!(
        finished.gaps,
        finished.summary.as_ref().unwrap().counts.gaps
    );
    assert!(!finished.summary.as_ref().unwrap().cleanup_failed);
    assert!(finished.export_error.is_some());
}

#[tokio::test]
#[ignore = "requires OXDM_OXVIF_TEST and OXDM_MOCK_EVENTS_TEST"]
async fn selected_camera_credentials_reach_only_the_selected_target() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_selected_mock, mut camera) = mock_camera(&["--auth"]).await;
    let (_other_mock, other) = mock_camera(&["--auth"]).await;
    camera.credentials = Credentials {
        username: "admin".into(),
        password: "admin".into(),
    };
    let controller = Controller::default();
    let target_override = format!("media profiles --target {}", other.address);
    for (input, succeeds) in [("media profiles", true), (target_override.as_str(), false)] {
        let request = grammar
            .prepare(executable.clone(), input, Some(&camera))
            .unwrap();
        assert!(controller.start(request, input.into(), None));
        let snapshot = wait_for(&controller, |state| state.finished.is_some()).await;
        assert_eq!(snapshot.finished.unwrap().exit_code == Some(0), succeeds);
    }
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn running_watch_keeps_its_camera_when_the_selection_changes() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_first_mock, mut selected) = mock_camera(&["--auth"]).await;
    selected.name = "Original camera".into();
    selected.credentials = Credentials {
        username: "admin".into(),
        password: "admin".into(),
    };
    let original_address = selected.address.clone();
    let controller = Controller::default();
    let watch = grammar
        .prepare(
            executable.clone(),
            "events watch --duration 30s",
            Some(&selected),
        )
        .unwrap();
    let command = shell_command(&watch.argv);
    assert!(controller.start(watch, command, None));
    let first = wait_for(&controller, |state| state.events > 0).await;
    assert_eq!(first.camera_name.as_deref(), Some("Original camera"));

    let (_second_mock, replacement) = mock_camera(&["--auth-empty"]).await;
    selected = replacement;
    selected.name = "New camera".into();
    selected.credentials = Credentials {
        username: "admin".into(),
        password: String::new(),
    };
    let query = || {
        grammar
            .prepare(executable.clone(), "device info", Some(&selected))
            .unwrap()
    };
    let command = shell_command(&query().argv);
    assert!(!controller.start(query(), command.clone(), None));
    let active = wait_for(&controller, |state| state.events > first.events).await;
    assert_eq!(active.run_id, first.run_id);
    assert_eq!(active.camera_name, first.camera_name);
    assert_eq!(active.target, original_address);
    controller.stop();
    let stopped = wait_for(&controller, |state| state.finished.is_some()).await;
    assert_eq!(stopped.run_id, first.run_id);
    assert_eq!(stopped.target, original_address);
    assert!(!stopped.summary.unwrap().cleanup_failed);

    assert!(controller.start(query(), command, None));
    let next = wait_for(&controller, |state| state.finished.is_some()).await;
    assert!(next.run_id > first.run_id);
    assert_eq!(next.camera_name.as_deref(), Some("New camera"));
    assert_eq!(next.target, selected.address);
    assert_eq!(next.finished.unwrap().exit_code, Some(0));
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn quoted_unicode_target_reaches_the_child_as_one_argument() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let input = r#"oxvif device info --target="https://[::1]:8443/自訂 路徑?label=\"鏡頭\"""#;
    let mut request = grammar.prepare(executable, input, None).unwrap();
    let command = shell_command(&request.argv);
    request.executable = mock_executable();
    request.argv.insert(0, "--echo-argv".into());
    let controller = Controller::default();
    assert!(controller.start(request, command, None));
    let snapshot = wait_for(&controller, |state| state.finished.is_some()).await;
    assert_eq!(snapshot.finished.unwrap().exit_code, Some(0));
    assert_eq!(snapshot.lines.len(), 1);
    let argv: Vec<String> = serde_json::from_str(&snapshot.lines[0].text).unwrap();
    assert_eq!(
        argv,
        [
            "device",
            "info",
            "--target=https://[::1]:8443/自訂 路徑?label=\"鏡頭\"",
            "--non-interactive",
            "--output",
            "json"
        ],
    );
}

#[tokio::test]
#[ignore = "requires OXDM_OXVIF_TEST and OXDM_MOCK_EVENTS_TEST"]
async fn missing_summary_and_forced_stop_are_distinct() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let controller = Controller::default();
    for (mode, forced) in [("--silent-exit", false), ("--ignore-stop", true)] {
        let mut request = grammar
            .prepare(executable.clone(), "events watch --target 127.0.0.1", None)
            .unwrap();
        request.executable = mock_executable();
        request.argv = vec![mode.into()];
        request.control.as_mut().unwrap().stop_grace_ms = 1_000;
        assert!(controller.start(request, "events watch".into(), None));
        if forced {
            wait_for(&controller, |state| state.events == 1).await;
            controller.stop();
        }
        let snapshot = wait_for(&controller, |state| state.finished.is_some()).await;
        assert_eq!(snapshot.events, 1);
        assert!(snapshot.summary.is_none());
        let finished = snapshot.finished.unwrap();
        assert_eq!(finished.forced, forced);
        if !forced {
            assert_eq!(finished.exit_code, Some(0));
            assert_eq!(
                finished.error.as_deref(),
                Some("Event watch exited without a final summary")
            );
        }
    }
}
