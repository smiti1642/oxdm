use std::{
    path::PathBuf,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    process::{Child, Command},
    time::{sleep, timeout},
};

use crate::state::{Credentials, Locale, View};

use super::{
    grammar::{normalize_target, shell_command, Camera, DynamicValues},
    process::{load_grammar, Controller, OutputKind, Snapshot},
};

#[test]
fn presentation_preserves_user_expansion_across_modes_and_expires_old_entries() {
    let mut presentation = super::RunPresentation::default();
    assert!(presentation.is_expanded(1, true, false));
    assert!(!presentation.is_expanded(1, false, false));
    presentation.set_expanded(1, false, 1);
    assert!(presentation.is_expanded(1, true, true));
    assert!(!presentation.is_expanded(1, true, false));
    presentation.set_expanded(1, true, 1);
    assert!(presentation.is_expanded(1, false, false));
    presentation.set_expanded(2, false, 2);
    assert!(!presentation.is_expanded(1, false, false));
    assert!(!presentation.is_expanded(2, true, false));
}

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

#[test]
fn console_returns_to_its_source_page_when_available() {
    assert_eq!(super::console_destination(View::Events, true), View::Events);
    assert_eq!(
        super::console_destination(View::Events, false),
        View::Welcome
    );
    assert_eq!(
        super::console_destination(View::HealthOverview, false),
        View::HealthOverview
    );
}

fn executable() -> PathBuf {
    std::env::var_os("OXDM_OXVIF_TEST")
        .expect("set OXDM_OXVIF_TEST to the absolute path of the compatible CLI")
        .into()
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "requires the bundled CLI and Windows Credential Manager"]
async fn bundled_cli_round_trips_a_synthetic_native_credential() {
    use std::process::Stdio;

    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let id = format!("oxdm-qa-{}-{now}", std::process::id());
    let reference = format!("profile/{id}");
    let entry = keyring::Entry::new("oxvif", &reference).unwrap();
    assert!(matches!(entry.get_password(), Err(keyring::Error::NoEntry)));
    struct RemoveFixture(keyring::Entry);
    impl Drop for RemoveFixture {
        fn drop(&mut self) {
            let _ = self.0.delete_credential();
        }
    }
    let fixture = RemoveFixture(entry);
    let input = format!("credential profile set {id}");
    let request = grammar.prepare(executable.clone(), &input, None).unwrap();
    assert!(request.credential_input);
    let mut child = super::process::command(&executable)
        .args(&request.argv)
        .arg("--password-stdin")
        .env("OXVIF_CONFIG_DIR", directory.path())
        .env("OXVIF_USERNAME", "oxdm-qa-native-user")
        .stdin(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"oxdm-qa-native-password")
        .await
        .unwrap();
    let result = timeout(Duration::from_secs(10), child.wait_with_output())
        .await
        .unwrap()
        .unwrap();
    assert!(
        result.status.success(),
        "native credential set exited with {}",
        result.status
    );
    assert!(!String::from_utf8_lossy(&result.stdout).contains("oxdm-qa-native-password"));
    assert!(fixture
        .0
        .get_password()
        .is_ok_and(|value| value == "oxdm-qa-native-password"));

    let remove = grammar
        .prepare(
            executable.clone(),
            &format!("credential profile delete {id}"),
            None,
        )
        .unwrap();
    let removed = super::process::command(&executable)
        .args(remove.argv)
        .env("OXVIF_CONFIG_DIR", directory.path())
        .output()
        .await
        .unwrap();
    assert!(
        removed.status.success(),
        "native credential delete exited with {}",
        removed.status
    );
    assert!(matches!(
        fixture.0.get_password(),
        Err(keyring::Error::NoEntry)
    ));
}

#[tokio::test]
#[ignore = "requires the bundled CLI"]
async fn inspect_displays_a_direct_read_after_using_the_private_credential_form() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let server = oxvif::mock::MockServer::builder()
        .enforce_auth(true)
        .start()
        .await
        .unwrap();
    let input = format!("inspect {} --read profiles", server.device_url());
    let stream = format!("inspect {} --read stream-uri", server.device_url());
    assert_eq!(
        grammar.prepare(executable.clone(), &stream, None).err(),
        Some("console_required_argument")
    );
    let stream = format!("{stream} --profile Profile_1");
    assert!(grammar.prepare(executable.clone(), &stream, None).is_ok());
    for inline in ["--password synthetic-secret", "--password=synthetic-secret"] {
        assert!(grammar
            .prepare(executable.clone(), &format!("{input} {inline}"), None)
            .is_err());
    }
    let mut request = grammar.prepare(executable, &input, None).unwrap();
    assert!(request.credential_input);
    assert!(request.workspace_change.is_none());
    assert!(request.credentials.is_none());
    request.credential = Some(super::grammar::CredentialInput {
        username: zeroize::Zeroizing::new("admin".into()),
        password: zeroize::Zeroizing::new("admin".into()),
    });
    let controller = Controller::default();
    assert!(controller.start(request, input, None));
    let snapshot = wait_for(&controller, |snapshot| snapshot.finished.is_some()).await;
    assert_eq!(snapshot.finished.unwrap().exit_code, Some(0));
    assert_eq!(snapshot.malformed, 0);
    assert!(snapshot
        .lines
        .iter()
        .any(|line| line.kind == OutputKind::Json));
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn credential_commands_use_a_private_pipe_and_keep_the_transcript_clean() {
    let cli = executable();
    let grammar = load_grammar(&cli).await.unwrap();
    let controller = Controller::default();
    for input in [
        "auth fixture",
        "device credential set fixture",
        "credential profile set fixture",
        "setup localhost --id fixture",
    ] {
        let mut invocation = grammar.prepare(cli.clone(), input, None).unwrap();
        assert!(invocation.credential_input && invocation.workspace_change.is_some());
        assert!(invocation.credential.is_none() && invocation.credentials.is_none());
        assert!(!invocation.argv.iter().any(|arg| arg == "--password-stdin"));
        invocation.executable = mock_executable();
        invocation
            .argv
            .insert(0, "--credential-pipe-fixture".into());
        invocation.credential = Some(super::grammar::CredentialInput {
            username: zeroize::Zeroizing::new("synthetic-account".into()),
            password: zeroize::Zeroizing::new("synthetic-password".into()),
        });
        assert!(controller.start(invocation, input.into(), None));
        let result = wait_for(&controller, |snapshot| snapshot.finished.is_some()).await;
        assert_eq!(result.finished.unwrap().exit_code, Some(0), "{input}");
        assert!(result.lines.is_empty());
        assert_eq!(result.command, input);
    }
    for input in [
        "auth fixture --username wrong-channel",
        "auth fixture --password-stdin",
        "setup",
        "setup http://user:password@localhost --id fixture",
    ] {
        assert!(
            grammar.prepare(cli.clone(), input, None).is_err(),
            "{input}"
        );
    }
    assert!(!controller.start(
        grammar.prepare(cli, "auth fixture", None).unwrap(),
        "auth fixture".into(),
        None
    ));
}

#[tokio::test]
#[ignore = "requires the bundled CLI"]
async fn completion_script_retains_exact_output_for_copying() {
    let cli = executable();
    let grammar = load_grammar(&cli).await.unwrap();
    let input = "completion bash";
    let request = grammar.prepare(cli.clone(), input, None).unwrap();
    let reference = super::process::command(&cli)
        .args(&request.argv)
        .output()
        .await
        .unwrap();
    assert!(reference.status.success());
    for invalid in [
        "completion bash --output json",
        "--output json completion bash",
        "--json completion bash",
    ] {
        assert!(
            grammar.prepare(cli.clone(), invalid, None).is_err(),
            "{invalid}"
        );
    }
    let controller = Controller::default();
    assert!(controller.start(request, input.into(), None));
    let snapshot = wait_for(&controller, |snapshot| snapshot.finished.is_some()).await;
    assert_eq!(snapshot.finished.unwrap().exit_code, Some(0));
    assert_eq!(
        snapshot.completion_script.as_deref().unwrap().as_bytes(),
        reference.stdout
    );
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn import_preview_generates_an_apply_draft_and_rejects_stale_fingerprints() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    let seeded = super::process::command(&mock_executable())
        .arg("--seed-import")
        .env("OXVIF_CONFIG_DIR", directory.path())
        .status()
        .await
        .unwrap();
    assert!(seeded.success());

    let preview = "device import --from console-fixture --plan";
    let request = grammar.prepare(executable.clone(), preview, None).unwrap();
    assert!(request.import_plan && request.workspace_change.is_none());
    let command = shell_command(&request.argv);
    let result = super::process::command(&executable)
        .args(request.argv)
        .env("OXVIF_CONFIG_DIR", directory.path())
        .output()
        .await
        .unwrap();
    assert!(result.status.success());
    let json = serde_json::from_slice(&result.stdout).unwrap();
    let fingerprint = super::process::import_plan_fingerprint(&json).unwrap();
    let draft = grammar.apply_import_draft(&command, fingerprint).unwrap();
    let request = grammar.prepare(executable.clone(), &draft, None).unwrap();
    assert!(!request.import_plan && request.workspace_change.is_some());
    let options = grammar.available_arguments(preview, "overrides", Locale::En);
    let override_path = r"C:\QA files\site's overrides.json";
    let token = options[0].insertion(override_path).unwrap();
    let with_file = grammar
        .prepare(executable.clone(), &format!("{preview} {token}"), None)
        .unwrap();
    let editor_draft = with_file
        .argv
        .iter()
        .map(|token| super::grammar::quote(token))
        .collect::<Vec<_>>()
        .join(" ");
    let edited = grammar
        .apply_import_draft(&editor_draft, fingerprint)
        .unwrap();
    let with_file = grammar.prepare(executable.clone(), &edited, None).unwrap();
    assert!(with_file
        .argv
        .contains(&format!("--overrides={override_path}")));
    let stale = draft.replace(fingerprint, &format!("sha256:{}", "0".repeat(64)));
    for (input, expected) in [(stale.as_str(), false), (draft.as_str(), true)] {
        let invocation = grammar.prepare(executable.clone(), input, None).unwrap();
        let result = super::process::command(&executable)
            .args(invocation.argv)
            .env("OXVIF_CONFIG_DIR", directory.path())
            .output()
            .await
            .unwrap();
        assert_eq!(result.status.success(), expected);
        let json: serde_json::Value = serde_json::from_slice(&result.stdout).unwrap();
        if expected {
            assert_eq!(json["data"]["applied"], true);
        } else {
            assert_eq!(json["error"]["code"], "IMPORT_PLAN_MISMATCH");
        }
    }
}

#[tokio::test]
#[ignore = "requires the bundled CLI"]
async fn bundled_command_help_and_completion_cover_all_supported_locales() {
    let grammar = load_grammar(&executable()).await.unwrap();
    for locale in [Locale::En, Locale::ZhTw, Locale::Ru] {
        let missing = grammar.missing_translations(locale);
        assert!(missing.is_empty(), "{locale:?}: {missing:?}");
    }

    let input = "events watch --to";
    let completion = grammar.analyze(input, input.len(), &DynamicValues::default(), Locale::Ru);
    assert!(completion.candidates.iter().any(|candidate| {
        candidate.label == "--topic" && candidate.detail.contains("темы ONVIF")
    }));
    assert!(grammar
        .available_arguments("events watch", "темы", Locale::Ru)
        .iter()
        .any(|option| option.label == "--topic" && option.detail.contains("темы ONVIF")));
    let input = "events watch dur";
    let completion = grammar.analyze(input, input.len(), &DynamicValues::default(), Locale::Ru);
    assert!(completion.candidates.iter().any(|candidate| {
        candidate.label == "--duration"
            && candidate.replacement == "--duration "
            && &input[candidate.range.clone()] == "dur"
    }));
    let input = "events watch --topic 'VideoSource/MotionAlarm' --quiet";
    let chips = grammar.argument_chips(input, &DynamicValues::default());
    let (edited, _) = chips[0].edited(input, "VideoSource/測試").unwrap();
    assert_eq!(edited, "events watch --topic=VideoSource/測試 --quiet");
    assert_eq!(chips[1].value.as_deref(), Some("true"));
    let options = grammar.available_arguments("events watch", "duration", Locale::En);
    assert_eq!(options[0].insertion("30").as_deref(), Some("--duration=30"));
    let draft = "events watch --duration 30s";
    let chip = &grammar.argument_chips(draft, &DynamicValues::default())[0];
    assert!(options[0].insertion("60m").is_some());
    assert!(chip.edited(draft, "60m").is_some());
    for invalid in ["0s", "+30s", "61m", "18446744073709551615m"] {
        assert!(options[0].insertion(invalid).is_none(), "{invalid}");
        assert!(chip.edited(draft, invalid).is_none(), "{invalid}");
        let draft = format!("events watch --target localhost --duration {invalid}");
        assert!(
            grammar.prepare(executable(), &draft, None).is_err(),
            "{draft}"
        );
    }

    let options = grammar.available_arguments("group member add lab", "", Locale::Ru);
    let positional = options
        .iter()
        .find(|option| option.label == "device_id")
        .unwrap();
    assert_eq!(
        positional.insertion("fixture-camera").as_deref(),
        Some("fixture-camera")
    );
    assert!(options.iter().all(|option| option.label != "group_id"));

    let options = grammar.available_arguments("config diff", "against", Locale::En);
    assert_eq!(options[0].value_kind, super::grammar::ValueKind::InputFile);
    let token = options[0].insertion(r"C:\test data\盤點.json").unwrap();
    let request = grammar
        .prepare(
            executable(),
            &format!("config diff --target localhost {token}"),
            None,
        )
        .unwrap();
    assert!(request
        .argv
        .contains(&r"--against=C:\test data\盤點.json".to_owned()));

    let draft = "device info -vv --verbose --target localhost";
    let chips = grammar.argument_chips(draft, &DynamicValues::default());
    assert_eq!(chips[0].value.as_deref(), Some("3"));
    let (edited, _) = chips[0].edited(draft, "4").unwrap();
    let request = grammar.prepare(executable(), &edited, None).unwrap();
    assert!(request.argv.contains(&"-vvvv".into()));
    assert!(!request.argv.contains(&"--verbose".into()));

    let draft = "media stream-uri --profile Pro";
    let dynamic = DynamicValues {
        media_profiles: vec!["Profile 1".into()],
        ..Default::default()
    };
    assert!(grammar.needs_media_profiles(draft, "localhost"));
    assert!(!grammar.needs_media_profiles("media stream-uri --device other", "localhost"));
    assert!(!grammar.needs_media_profiles("media stream-uri --target elsewhere", "localhost"));
    assert!(grammar
        .analyze(draft, draft.len(), &dynamic, Locale::Ru)
        .candidates
        .iter()
        .any(|candidate| candidate.label == "Profile 1"));
}

#[tokio::test]
#[ignore = "requires the bundled CLI"]
async fn coverage_document_matches_every_public_command_route() {
    let grammar = load_grammar(&executable()).await.unwrap();
    let document = include_str!("../../docs/oxvif-console-coverage.md");
    let listed = document
        .lines()
        .filter_map(|line| {
            let (command, rest) = line.strip_prefix("| `")?.split_once("` | ")?;
            let (route, detail) = rest.split_once(" | ")?;
            assert!(!detail.trim_matches([' ', '|']).is_empty(), "{command}");
            Some((command.to_owned(), route))
        })
        .collect::<Vec<_>>();
    assert_eq!(grammar.command_coverage(), listed);
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn expanded_routes_handle_profile_options_local_ids_and_workspace_selectors() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_mock, camera) = mock_camera(&[]).await;
    let input = "media stream-uri --profile Profile_1 --media-service=media1";
    let request = grammar
        .prepare(executable.clone(), input, Some(&camera))
        .unwrap();
    assert_eq!(
        &request.argv[..3],
        ["--media-service=media1", "media", "stream-uri"]
    );
    assert_eq!(request.target, camera.address);
    assert_eq!(
        grammar
            .prepare(executable.clone(), "media stream-uri", Some(&camera))
            .err(),
        Some("console_required_argument")
    );

    let controller = Controller::default();
    let files = tempfile::tempdir().unwrap();
    let inventory = super::grammar::quote(files.path().join("inventory.json").to_str().unwrap());
    let image = super::grammar::quote(files.path().join("snapshot.jpg").to_str().unwrap());
    for input in [
        "device test".to_owned(),
        "device capabilities".to_owned(),
        "device services".to_owned(),
        "media stream-uri --profile Profile_1 --media-service=media1".to_owned(),
        "media snapshot-uri --profile Profile_1".to_owned(),
        "ptz status --profile Profile_1".to_owned(),
        "ptz presets --profile Profile_1".to_owned(),
        "health check".to_owned(),
        "device list".to_owned(),
        "config path".to_owned(),
        format!("config export --save {inventory}"),
        format!("config diff --against {inventory}"),
        format!("media snapshot-save --profile Profile_1 --save {image}"),
    ] {
        let request = grammar
            .prepare(executable.clone(), &input, Some(&camera))
            .unwrap();
        assert!(controller.start(request, input.clone(), None));
        let completed = wait_for(&controller, |snapshot| snapshot.finished.is_some()).await;
        assert_eq!(completed.finished.unwrap().exit_code, Some(0), "{input}");
        assert_eq!(completed.malformed, 0, "{input}");
        assert!(
            completed
                .lines
                .iter()
                .any(|line| line.kind == OutputKind::Json),
            "{input}"
        );
    }
    assert!(files.path().join("inventory.json").is_file());
    assert!(files.path().join("snapshot.jpg").is_file());
    let request = grammar
        .prepare(
            executable.clone(),
            "device show -- front-door",
            Some(&camera),
        )
        .unwrap();
    assert_eq!(
        request.argv,
        [
            "device",
            "show",
            "--non-interactive",
            "--output",
            "json",
            "--",
            "front-door"
        ]
    );
    assert!(request.target.is_empty() && request.credentials.is_none());
    let request = grammar
        .prepare(
            executable.clone(),
            "device test --group north --jobs 2",
            Some(&camera),
        )
        .unwrap();
    assert_eq!(
        &request.argv[..6],
        ["--group", "north", "--jobs", "2", "device", "test"]
    );
    assert!(request.target.is_empty() && request.credentials.is_none());
    let request = grammar
        .prepare(executable, "discover scan --save fixture", None)
        .unwrap();
    assert!(request.workspace_change.is_some());
}

#[tokio::test]
#[ignore = "requires the bundled CLI"]
async fn external_manage_accepts_only_its_supported_selectors() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    assert_eq!(
        grammar.external_terminal_args("manage").unwrap().unwrap(),
        ["manage"]
    );
    assert_eq!(
        grammar
            .external_terminal_args("manage front-door")
            .unwrap()
            .unwrap(),
        ["manage", "--", "front-door"]
    );
    assert_eq!(
        grammar
            .external_terminal_args("manage --target 127.0.0.1")
            .unwrap()
            .unwrap(),
        [
            "manage",
            "--target",
            "http://127.0.0.1/onvif/device_service"
        ]
    );
    for input in [
        "manage --output json",
        "manage a --target localhost",
        "manage --target http://user:password@localhost",
        "manage --username admin",
    ] {
        assert!(
            grammar.external_terminal_args(input).unwrap().is_err(),
            "{input}"
        );
    }
    assert!(grammar.prepare(executable, "manage", None).is_err());
}

#[cfg(windows)]
#[tokio::test]
#[ignore = "requires the matching console_mock fixture and Windows console host"]
async fn external_terminal_provides_native_terminal_handles() {
    let directory = tempfile::tempdir().unwrap();
    let result_path = directory.path().join("terminal status.json");
    let argv = vec![
        "--terminal-fixture".to_owned(),
        result_path.to_str().unwrap().to_owned(),
        "台北 \"A\" \\".to_owned(),
        String::new(),
    ];
    let child = super::terminal::spawn(&mock_executable(), &argv).unwrap();
    use std::os::windows::io::AsRawHandle;
    use windows_sys::Win32::System::Threading::WaitForSingleObject;
    let status = tokio::task::spawn_blocking(move || {
        // The handle belongs to this test child and remains open during the wait.
        unsafe { WaitForSingleObject(child.as_raw_handle(), 10_000) }
    })
    .await
    .unwrap();
    assert_eq!(status, windows_sys::Win32::Foundation::WAIT_OBJECT_0);
    let result: serde_json::Value =
        serde_json::from_slice(&std::fs::read(result_path).unwrap()).unwrap();
    assert_eq!(
        result,
        serde_json::json!({"stdin": true, "stdout": true, "stderr": true, "isolated": true, "args": &argv[2..]})
    );
}

#[tokio::test]
#[ignore = "requires the bundled CLI"]
async fn workspace_writes_use_cli_registry_and_keep_passwords_out_of_arguments() {
    use super::grammar::WorkspaceChange;

    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let directory = tempfile::tempdir().unwrap();
    for (input, expected) in [
        (
            "device add fixture --target localhost --name 'Fixture camera'",
            Some(WorkspaceChange::Write),
        ),
        ("group create test-group", Some(WorkspaceChange::Write)),
        (
            "group member add test-group fixture --alias cam",
            Some(WorkspaceChange::Write),
        ),
        ("group show test-group", None),
        (
            "group member remove test-group cam",
            Some(WorkspaceChange::Remove),
        ),
    ] {
        let request = grammar.prepare(executable.clone(), input, None).unwrap();
        assert!(request.workspace_change == expected, "{input}");
        assert!(request.target.is_empty() && request.credentials.is_none());
        let output = super::process::command(&executable)
            .args(request.argv)
            .env("OXVIF_CONFIG_DIR", directory.path())
            .output()
            .await
            .unwrap();
        assert!(
            output.status.success(),
            "{input}: {}",
            String::from_utf8_lossy(&output.stdout)
        );
    }
    assert!(grammar
        .prepare(
            executable,
            "device add fixture --target http://user:password@localhost",
            None
        )
        .is_err());
}

#[tokio::test]
#[ignore = "requires the bundled CLI and its matching console_mock fixture"]
async fn shareable_diagnosis_keeps_only_the_validated_report() {
    let executable = executable();
    let grammar = load_grammar(&executable).await.unwrap();
    let (_mock, camera) = mock_camera(&[]).await;
    let controller = Controller::default();
    let config = grammar
        .prepare(executable.clone(), "config validate --share", Some(&camera))
        .unwrap();
    assert!(config.target.is_empty() && config.credentials.is_none());
    assert!(config.share_report);
    let invocation = grammar
        .prepare(executable, "diagnose --share", Some(&camera))
        .unwrap();
    assert_eq!(invocation.target, camera.address);
    assert!(invocation.share_report);
    assert!(controller.start(invocation, "diagnose --share".into(), None));
    let completed = wait_for(&controller, |snapshot| snapshot.finished.is_some()).await;
    let report = completed.share_report.expect("valid share report");
    let json = report.json().unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let destination = temporary.path().join("shareable.json");
    std::fs::write(&destination, "old report").unwrap();
    report.save(destination.clone()).unwrap();
    assert_eq!(std::fs::read_to_string(destination).unwrap(), json);
    assert_eq!(std::fs::read_dir(temporary.path()).unwrap().count(), 1);
    assert!(!json.contains(&camera.address));
    assert!(!json.contains("MockCam-1080p"));
    assert!(!json.contains("Profile_1"));
    assert!(super::share_report::ShareReport::parse(json.as_bytes()).is_some());
    let truncated = json.trim_end().strip_suffix('}').unwrap();
    assert!(super::share_report::ShareReport::parse(truncated.as_bytes()).is_none());
    let mut oversized = json.as_bytes().to_vec();
    oversized.resize(super::share_report::MAX_BYTES + 1, b' ');
    assert!(super::share_report::ShareReport::parse(&oversized).is_none());
    let mut poisoned: serde_json::Value = serde_json::from_str(&json).unwrap();
    poisoned["checks"][0]["target"] = serde_json::json!("secret-address");
    assert!(
        super::share_report::ShareReport::parse(&serde_json::to_vec(&poisoned).unwrap()).is_none()
    );

    let (_auth_mock, mut camera) = mock_camera(&["--auth"]).await;
    camera.credentials = Credentials {
        username: "fixture-private-username".into(),
        password: "fixture-private-password".into(),
    };
    let request = grammar
        .prepare(self::executable(), "diagnose --share", Some(&camera))
        .unwrap();
    assert!(controller.start(request, "diagnose --share".into(), None));
    let failed = wait_for(&controller, |snapshot| snapshot.finished.is_some()).await;
    assert_ne!(failed.finished.as_ref().unwrap().exit_code, Some(0));
    let failed_report = failed.share_report.unwrap();
    let (stage, reason, action) = failed_report.issue(Locale::Ru).unwrap();
    for translated in [stage, reason, action] {
        assert!(translated
            .chars()
            .any(|c| ('\u{0400}'..='\u{04ff}').contains(&c)));
    }
    let encoded = failed_report.json().unwrap();
    for secret in [
        "fixture-private-username",
        "fixture-private-password",
        &camera.address,
    ] {
        assert!(!encoded.contains(secret));
    }
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
    let mut view = super::ViewSubscription::create(
        camera.address.clone(),
        camera.credentials.clone(),
        None,
        permit,
    )
    .await
    .unwrap();
    let next = view.subscription().next();
    assert!(matches!(
        timeout(Duration::from_secs(6), next).await.unwrap(),
        Some(Ok(oxvif::events::ManagedEvent::Notification(_)))
    ));

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
    assert_eq!(
        view.subscription().state(),
        oxvif::events::ManagedEventState::Active
    );

    assert!(controller.start(prepare(), input.into(), None));
    assert!(timeout(
        Duration::from_millis(150),
        wait_for(&controller, |state| !state.waiting_for_subscription),
    )
    .await
    .is_err());
    drop(view);
    wait_for(&controller, |state| state.events > 0).await;
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
    let empty = grammar.analyze("", 0, &DynamicValues::default(), Locale::ZhTw);
    assert!(empty
        .candidates
        .iter()
        .all(|item| !item.label.starts_with('-')));
    let flags = grammar.analyze("--", 2, &DynamicValues::default(), Locale::ZhTw);
    for option in [
        "--clock-sync",
        "--soap-auth",
        "--media-service",
        "--rtsp-probe",
    ] {
        assert!(
            flags.candidates.iter().any(|item| {
                item.label == option
                    && item
                        .detail
                        .chars()
                        .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch))
            }),
            "missing translated option {option}"
        );
    }
    let incomplete = "device info --target https://camera.example/onvif --";
    let analysis = grammar.analyze(
        incomplete,
        incomplete.len(),
        &DynamicValues::default(),
        Locale::ZhTw,
    );
    assert_eq!(
        analysis.explicit_target.as_deref(),
        Some("https://camera.example/onvif")
    );
    let candidates = grammar
        .analyze("ev", 2, &DynamicValues::default(), Locale::ZhTw)
        .candidates;
    assert!(candidates
        .iter()
        .any(|candidate| candidate.label == "events" && candidate.detail.contains("攝影機")));
    let partial = "events watch --output=j --duration 30s";
    let caret = partial.find(" --duration").unwrap();
    let candidates = grammar
        .analyze(partial, caret, &DynamicValues::default(), Locale::En)
        .candidates;
    let jsonl = candidates
        .iter()
        .find(|candidate| candidate.label == "jsonl")
        .unwrap();
    assert_eq!(&partial[jsonl.range.clone()], "j");
    assert_eq!(jsonl.replacement, "jsonl");

    let group = grammar.command_help("events ", Locale::ZhTw);
    assert_eq!(group.path, "events");
    assert!(group.subcommands.iter().any(|node| node.name == "watch"));
    let help = grammar.command_help("oxvif events watch --duration", Locale::ZhTw);
    assert_eq!(help.path, "events watch");
    assert!(help.watch);
    assert!(help.summary.unwrap().contains("篩選"));
    assert!(help.arguments.iter().any(|arg| {
        arg.label() == "--topic"
            && arg
                .description(Locale::ZhTw)
                .is_some_and(|description| description.contains("主題"))
    }));
    assert!(!help.arguments.iter().any(|arg| arg.label() == "--device"));
    assert!(!help.arguments.iter().any(|arg| arg.label() == "--output"));
    assert!(grammar
        .prepare(executable.clone(), &help.template.unwrap(), Some(&camera))
        .is_ok());

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
    let workspace = grammar
        .prepare(
            executable.clone(),
            "device info --device another",
            Some(&other),
        )
        .unwrap();
    assert!(workspace.target.is_empty() && workspace.credentials.is_none());
    assert_eq!(
        &workspace.argv[..4],
        ["--device", "another", "device", "info"]
    );
    for conflict in [
        "device info another --target localhost",
        "device info --group lab --device another",
        "diagnose --share --group lab",
        "config validate --soap-auth combined",
        "events watch another",
        "events watch --device another",
    ] {
        assert!(
            grammar
                .prepare(executable.clone(), conflict, Some(&other))
                .is_err(),
            "{conflict}"
        );
    }

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
