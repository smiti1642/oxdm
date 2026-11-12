use std::{
    collections::VecDeque,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    process::Stdio,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde::Deserialize;
use tokio::{
    io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    process::Command,
    sync::{mpsc, Notify},
    task::spawn_blocking,
    time::{sleep, timeout},
};

use super::grammar::{Description, Envelope, Grammar, Invocation};
use super::EventSubscriptions;

const MAX_RECORD_BYTES: usize = 1024 * 1024;
const MAX_DISPLAY_BYTES: usize = 512 * 1024;
const MAX_DISPLAY_LINES: usize = 250;
const EXPORT_QUEUE_RECORDS: usize = 32;
const IO_FINISH_TIMEOUT: Duration = Duration::from_secs(3);
const STARTUP_AND_CLEANUP_BUDGET: Duration = Duration::from_secs(45);
const MAX_WATCH_LIFETIME: Duration = Duration::from_secs(3600 + 45);

#[derive(Clone, PartialEq)]
pub struct OutputLine {
    pub index: u64,
    pub text: String,
    pub kind: OutputKind,
    pub event: Option<EventHeader>,
}

#[derive(Clone, PartialEq)]
pub struct EventHeader {
    pub sequence: u64,
    pub time: String,
    pub topic: String,
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum OutputKind {
    Text,
    Json,
    Event,
    Gap,
    Summary,
    Error,
}

#[derive(Clone, PartialEq, Deserialize)]
pub struct WatchSummary {
    pub reason: String,
    pub complete: bool,
    pub known_undelivered: u64,
    pub recoveries: u32,
    pub cleanup_failed: bool,
    pub counts: WatchCounts,
}

#[derive(Clone, PartialEq, Deserialize)]
pub struct WatchCounts {
    pub delivered: u64,
    pub gaps: u64,
}

#[derive(Clone, Default, PartialEq)]
pub struct Snapshot {
    pub run_id: u64,
    pub revision: u64,
    pub target: String,
    pub camera_name: Option<String>,
    pub command: String,
    pub watch: bool,
    pub lines: VecDeque<OutputLine>,
    pub evicted: u64,
    pub events: u64,
    pub gaps: u64,
    pub last_gap: Option<String>,
    pub malformed: u64,
    pub summary: Option<WatchSummary>,
    pub export: Option<PathBuf>,
    pub export_error: Option<String>,
    pub waiting_for_subscription: bool,
    pub stopping: bool,
    pub finished: Option<Finished>,
    next_line: u64,
    display_bytes: usize,
}

#[derive(Clone, PartialEq)]
pub struct Finished {
    pub exit_code: Option<i32>,
    pub started: bool,
    pub forced: bool,
    pub error: Option<String>,
}

impl Snapshot {
    fn append(&mut self, kind: OutputKind, text: String, event: Option<EventHeader>) {
        let mut display = String::with_capacity(text.len());
        for ch in text.trim_end_matches(['\r', '\n']).chars() {
            if ch.is_control() && ch != '\t' {
                display.extend(ch.escape_default());
            } else {
                display.push(ch);
            }
        }
        let text = display;
        if text.len() > MAX_DISPLAY_BYTES {
            self.evicted += 1;
            return;
        }
        while self.lines.len() >= MAX_DISPLAY_LINES
            || self.display_bytes + text.len() > MAX_DISPLAY_BYTES
        {
            if let Some(old) = self.lines.pop_front() {
                self.display_bytes -= old.text.len();
                self.evicted += 1;
            }
        }
        self.next_line += 1;
        self.display_bytes += text.len();
        self.lines.push_back(OutputLine {
            index: self.next_line,
            text,
            kind,
            event,
        });
    }
}

#[derive(Default, Clone)]
pub struct Controller {
    current: Arc<Mutex<Option<Arc<Run>>>>,
    subscriptions: EventSubscriptions,
}

struct Run {
    data: Mutex<Snapshot>,
    stop: Notify,
}

impl Run {
    fn update(&self, edit: impl FnOnce(&mut Snapshot)) {
        let mut data = self.data.lock().unwrap();
        edit(&mut data);
        data.revision += 1;
    }
}

impl Controller {
    pub(super) fn event_subscriptions(&self) -> EventSubscriptions {
        self.subscriptions.clone()
    }

    pub fn start(&self, request: Invocation, command: String, export: Option<PathBuf>) -> bool {
        let mut current = self.current.lock().unwrap();
        let revision = current.as_ref().map_or(1, |run| {
            let state = run.data.lock().unwrap();
            if state.finished.is_none() {
                0
            } else {
                state.revision + 1
            }
        });
        if revision == 0 {
            return false;
        }
        let run = Arc::new(Run {
            data: Mutex::new(Snapshot {
                run_id: revision,
                revision,
                target: request.target.clone(),
                camera_name: request.camera_name.clone(),
                command,
                watch: request.control.is_some(),
                waiting_for_subscription: request.control.is_some(),
                export: export.clone(),
                ..Snapshot::default()
            }),
            stop: Notify::new(),
        });
        *current = Some(run.clone());
        tokio::spawn(supervise(run, request, export, self.subscriptions.clone()));
        true
    }

    pub fn stop(&self) {
        if let Some(run) = self.current.lock().unwrap().as_ref() {
            let mut state = run.data.lock().unwrap();
            if state.finished.is_none() && !state.stopping {
                state.stopping = true;
                state.revision += 1;
                run.stop.notify_one();
            }
        }
    }

    pub fn is_active(&self) -> bool {
        self.current
            .lock()
            .unwrap()
            .as_ref()
            .is_some_and(|run| run.data.lock().unwrap().finished.is_none())
    }

    pub fn snapshot_after(&self, revision: u64) -> Option<Snapshot> {
        let current = self.current.lock().unwrap();
        let state = current.as_ref()?.data.lock().unwrap();
        (state.revision > revision).then(|| state.clone())
    }
}

pub(super) fn command(executable: &Path) -> Command {
    let mut cmd = Command::new(executable);
    cmd.env_remove("OXVIF_USERNAME")
        .env_remove("OXVIF_PASSWORD")
        .env_remove("OXVIF_DEVICE")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true);
    #[cfg(windows)]
    cmd.creation_flags(0x0800_0000); // CREATE_NO_WINDOW
    cmd
}

pub fn default_executable() -> Option<PathBuf> {
    if let Some(path) = std::env::var_os("OXDM_OXVIF")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
    {
        return Some(path);
    }
    let current = std::env::current_exe().ok()?;
    let dir = current.parent()?;
    let binary = dir.join(format!("oxdm-oxvif{}", std::env::consts::EXE_SUFFIX));
    binary.is_file().then_some(binary)
}

pub async fn load_grammar(executable: &Path) -> Result<Grammar, String> {
    let bytes = capture(
        executable,
        &[
            "describe",
            "--grammar",
            "--output",
            "json",
            "--non-interactive",
        ],
        2 * MAX_RECORD_BYTES,
    )
    .await?;
    Envelope::parse(&bytes)
}

pub struct LegacyHelp {
    pub version: Option<String>,
    pub commands: Vec<Description>,
}

pub async fn load_legacy_help(executable: &Path) -> LegacyHelp {
    #[derive(Deserialize)]
    struct Response {
        data: Catalogue,
    }
    #[derive(Deserialize)]
    struct Catalogue {
        commands: Vec<Description>,
    }
    let (version, descriptions) = tokio::join!(
        capture(executable, &["--version"], 512),
        capture(
            executable,
            &["describe", "--output", "json", "--non-interactive"],
            MAX_RECORD_BYTES
        ),
    );
    let commands = descriptions
        .ok()
        .and_then(|data| serde_json::from_slice::<Response>(&data).ok())
        .map(|response| {
            response
                .data
                .commands
                .into_iter()
                .filter(|command| {
                    matches!(
                        command.name.as_str(),
                        "device.info" | "media.profiles" | "events.watch"
                    )
                })
                .collect()
        })
        .unwrap_or_default();
    LegacyHelp {
        version: version
            .ok()
            .and_then(|data| String::from_utf8(data).ok())
            .map(|version| version.trim().to_owned()),
        commands,
    }
}

async fn capture(executable: &Path, arguments: &[&str], limit: usize) -> Result<Vec<u8>, String> {
    if !executable.is_absolute() || !executable.is_file() {
        return Err("Select the absolute path to the oxvif executable".into());
    }
    let mut child = command(executable)
        .args(arguments)
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdout = child
        .stdout
        .take()
        .expect("piped stdout")
        .take(limit as u64 + 1);
    timeout(Duration::from_secs(5), async {
        let mut data = Vec::new();
        stdout.read_to_end(&mut data).await?;
        if data.len() > limit {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "CLI metadata exceeds the size limit",
            ));
        }
        if !child.wait().await?.success() {
            return Err(io::Error::other("The CLI metadata command failed"));
        }
        Ok(data)
    })
    .await
    .map_err(|_| "Reading CLI metadata timed out".to_owned())?
    .map_err(|e| e.to_string())
}

async fn supervise(
    run: Arc<Run>,
    request: Invocation,
    export_path: Option<PathBuf>,
    subscriptions: EventSubscriptions,
) {
    let _subscription = if request.control.is_some() {
        tokio::select! {
            biased;
            () = run.stop.notified() => {
                run.update(|state| {
                    state.waiting_for_subscription = false;
                    state.export = None;
                    state.finished = Some(Finished {
                        exit_code: None,
                        started: false,
                        forced: false,
                        error: None,
                    });
                });
                return;
            }
            permit = subscriptions.acquire(&request.target) => {
                run.update(|state| state.waiting_for_subscription = false);
                Some(permit)
            }
        }
    } else {
        None
    };
    let mut command = command(&request.executable);
    command.args(&request.argv).stderr(Stdio::piped());
    if let Some(control) = &request.control {
        command.arg(&control.flag).stdin(Stdio::piped());
    }
    if let Some((user, password)) = request.credentials.as_ref().and_then(|creds| {
        let (user, password) = creds.as_options();
        user.zip(password)
    }) {
        command
            .env("OXVIF_USERNAME", user)
            .env("OXVIF_PASSWORD", password);
    }
    let child = command.spawn();
    drop(command);
    let Invocation {
        control, output, ..
    } = request;
    let mut child = match child {
        Ok(child) => child,
        Err(e) => {
            run.update(|state| {
                state.export = None;
                state.finished = Some(Finished {
                    exit_code: None,
                    started: false,
                    forced: false,
                    error: Some(e.to_string()),
                });
            });
            return;
        }
    };
    let watch = control.is_some();
    let (export, mut export_task) = match export_path {
        Some(path) => {
            let (sender, receiver) = mpsc::channel(EXPORT_QUEUE_RECORDS);
            let task = tokio::spawn(write_export(run.clone(), path, receiver));
            (Some(sender), Some(task))
        }
        None => (None, None),
    };
    let stdout = BufReader::new(child.stdout.take().expect("piped stdout"));
    let mut stdout = if output == "json" {
        tokio::spawn(read_json(run.clone(), stdout))
    } else {
        tokio::spawn(read_output(
            run.clone(),
            stdout,
            false,
            watch,
            output == "jsonl",
            export,
        ))
    };
    let mut stderr = tokio::spawn(read_output(
        run.clone(),
        BufReader::new(child.stderr.take().expect("piped stderr")),
        true,
        false,
        false,
        None,
    ));
    let mut stdin = child.stdin.take();
    let mut forced = false;
    let status = tokio::select! {
        result = child.wait() => result,
        () = run.stop.notified() => {
            if let Some(mut pipe) = stdin.take() {
                let _ = timeout(Duration::from_millis(250), pipe.write_all(b"stop\n")).await;
            }
            let grace = control.map_or(Duration::ZERO, |c| Duration::from_millis(c.stop_grace_ms));
            match timeout(grace, child.wait()).await {
                Ok(result) => result,
                Err(_) => {
                    forced = true;
                    let _ = child.start_kill();
                    child.wait().await
                }
            }
        }
        () = sleep(if watch { MAX_WATCH_LIFETIME } else { STARTUP_AND_CLEANUP_BUDGET }) => {
            forced = true;
            let _ = child.start_kill();
            child.wait().await
        }
    };
    drop(stdin);
    let output_error = match timeout(IO_FINISH_TIMEOUT, async {
        tokio::join!(&mut stdout, &mut stderr)
    })
    .await
    {
        Ok((Ok(Ok(())), Ok(Ok(())))) => None,
        Ok(_) => Some("Reading child output failed".to_owned()),
        Err(_) => {
            stdout.abort();
            stderr.abort();
            Some("Reading child output did not finish".to_owned())
        }
    };
    if let Some(task) = export_task.as_mut() {
        match timeout(IO_FINISH_TIMEOUT, &mut *task).await {
            Ok(Ok(())) => {}
            result => {
                if result.is_err() {
                    task.abort();
                }
                run.update(|state| {
                    state
                        .export_error
                        .get_or_insert_with(|| "Writing JSONL did not finish".into());
                });
            }
        }
    }
    run.update(|state| {
        if state.export.is_some() && (forced || state.summary.is_none() || output_error.is_some()) {
            state.export_error.get_or_insert_with(|| {
                "The event stream ended without a complete output record".into()
            });
        }
        let missing_summary = (watch
            && !forced
            && state.summary.is_none()
            && status.as_ref().is_ok_and(std::process::ExitStatus::success))
        .then(|| "Event watch exited without a final summary".to_owned());
        state.finished = Some(Finished {
            exit_code: status
                .as_ref()
                .ok()
                .and_then(std::process::ExitStatus::code),
            started: true,
            forced,
            error: status
                .err()
                .map(|e| e.to_string())
                .or(output_error)
                .or(missing_summary),
        });
    });
}

async fn read_json(run: Arc<Run>, mut reader: impl AsyncReadExt + Unpin) -> io::Result<()> {
    let mut document = Vec::new();
    let mut chunk = [0; 8192];
    let mut oversized = false;
    loop {
        let size = reader.read(&mut chunk).await?;
        if size == 0 {
            break;
        }
        if document.len() + size <= MAX_RECORD_BYTES && !oversized {
            document.extend_from_slice(&chunk[..size]);
        } else {
            oversized = true;
        }
    }
    let parsed = !oversized && serde_json::from_slice::<serde_json::Value>(&document).is_ok();
    run.update(|state| {
        if !parsed {
            state.malformed += 1;
        }
        if oversized {
            state.append(
                OutputKind::Error,
                "JSON output exceeds the size limit".into(),
                None,
            );
        }
        let text = String::from_utf8_lossy(&document);
        for line in text.lines() {
            let kind = if parsed {
                OutputKind::Json
            } else {
                OutputKind::Error
            };
            state.append(kind, line.into(), None);
        }
    });
    Ok(())
}

async fn read_output(
    run: Arc<Run>,
    mut reader: impl AsyncBufRead + Unpin,
    stderr: bool,
    watch: bool,
    structured: bool,
    mut export: Option<mpsc::Sender<Vec<u8>>>,
) -> io::Result<()> {
    let mut line = Vec::new();
    loop {
        let accepted = read_record(&mut reader, &mut line, MAX_RECORD_BYTES).await?;
        if accepted && line.is_empty() {
            return Ok(());
        }
        let text = if accepted {
            String::from_utf8_lossy(&line).into_owned()
        } else {
            "Output record exceeds the size limit".to_owned()
        };
        let json = (accepted && structured)
            .then(|| serde_json::from_slice::<serde_json::Value>(&line).ok())
            .flatten();
        let watch_record = json.as_ref().filter(|value| value["schema_version"] == "3");
        let mut kind = if stderr {
            OutputKind::Error
        } else if json.is_some() {
            OutputKind::Json
        } else {
            OutputKind::Text
        };
        run.update(|state| {
            let mut event = None;
            if !accepted || structured && json.is_none() || watch && watch_record.is_none() {
                state.malformed += 1;
                kind = OutputKind::Error;
            }
            if let Some(record) = watch_record.filter(|_| watch) {
                match record["data"]["kind"].as_str() {
                    Some("event_notification") => {
                        state.events += 1;
                        kind = OutputKind::Event;
                        let data = &record["data"];
                        let header = &data["event"];
                        event = Some(EventHeader {
                            sequence: data["sequence"].as_u64().unwrap_or_default(),
                            time: header["utc_time"]
                                .as_str()
                                .unwrap_or_default()
                                .chars()
                                .take(80)
                                .collect(),
                            topic: header["topic"]
                                .as_str()
                                .unwrap_or_default()
                                .chars()
                                .take(300)
                                .collect(),
                        });
                    }
                    Some("event_gap") => {
                        state.gaps += 1;
                        state.last_gap = record["data"]["reason"].as_str().map(str::to_owned);
                        kind = OutputKind::Gap;
                    }
                    Some("event_watch_summary") => {
                        state.summary =
                            serde_json::from_value(record["data"]["summary"].clone()).ok();
                        if state.summary.is_none() {
                            state.malformed += 1;
                        }
                        kind = OutputKind::Summary;
                    }
                    _ if record.get("error").is_some() => kind = OutputKind::Error,
                    _ => {
                        state.malformed += 1;
                        kind = OutputKind::Error;
                    }
                }
            }
            state.append(kind, text, event);
        });
        if let Some(sender) = &export {
            if watch_record.is_none() {
                run.update(|state| {
                    state.export_error.get_or_insert_with(|| {
                        "An invalid JSONL record could not be exported".into()
                    });
                });
            } else if sender.try_send(line.clone()).is_err() {
                run.update(|state| {
                    state
                        .export_error
                        .get_or_insert_with(|| "JSONL writer stopped accepting records".into());
                });
                export = None;
            }
        }
    }
}

// An empty accepted buffer means EOF. Blank lines are represented by their newline.
async fn read_record(
    reader: &mut (impl AsyncBufRead + Unpin),
    line: &mut Vec<u8>,
    max: usize,
) -> io::Result<bool> {
    line.clear();
    let mut oversize = false;
    loop {
        let chunk = reader.fill_buf().await?;
        if chunk.is_empty() {
            return Ok(!oversize);
        }
        let newline = chunk.iter().position(|byte| *byte == b'\n');
        let consumed = newline.map_or(chunk.len(), |index| index + 1);
        if !oversize && line.len() + consumed <= max {
            line.extend_from_slice(&chunk[..consumed]);
        } else {
            oversize = true;
        }
        reader.consume(consumed);
        if newline.is_some() {
            return Ok(!oversize);
        }
    }
}

async fn write_export(run: Arc<Run>, path: PathBuf, mut lines: mpsc::Receiver<Vec<u8>>) {
    let result: io::Result<()> = async {
        let mut file = spawn_blocking(move || File::create_new(path)).await??;
        let mut records = Vec::with_capacity(EXPORT_QUEUE_RECORDS);
        let mut written = 0;
        while lines.recv_many(&mut records, EXPORT_QUEUE_RECORDS).await > 0 {
            // Keep the write and rollback in the same blocking job, so a
            // supervisor timeout cannot interrupt recovery from a partial write.
            (file, records, written) = spawn_blocking(move || {
                write_export_batch(&mut file, &mut written, &mut records, File::write_all)?;
                records.clear();
                Ok::<_, io::Error>((file, records, written))
            })
            .await??;
        }
        Ok(())
    }
    .await;
    if let Err(error) = result {
        run.update(|state| state.export_error = Some(error.to_string()));
    }
}

fn write_export_batch(
    file: &mut File,
    written: &mut u64,
    records: &mut [Vec<u8>],
    mut write: impl FnMut(&mut File, &[u8]) -> io::Result<()>,
) -> io::Result<()> {
    for record in records {
        if !record.ends_with(b"\n") {
            record.push(b'\n');
        }
        if let Err(error) = write(file, record) {
            return match file.set_len(*written) {
                Ok(()) => Err(error),
                Err(truncate) => Err(io::Error::other(format!(
                    "{error}. Could not remove an incomplete JSONL record: {truncate}"
                ))),
            };
        }
        *written += record.len() as u64;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires OXDM_OXVIF_LEGACY_TEST with a CLI predating --grammar"]
    async fn legacy_cli_retains_version_and_command_help() {
        let executable = PathBuf::from(
            std::env::var_os("OXDM_OXVIF_LEGACY_TEST").expect("set OXDM_OXVIF_LEGACY_TEST"),
        );
        assert!(load_grammar(&executable).await.is_err());
        let help = load_legacy_help(&executable).await;
        assert!(help
            .version
            .is_some_and(|version| version.starts_with("oxvif ")));
        for name in ["device.info", "media.profiles"] {
            let command = help
                .commands
                .iter()
                .find(|command| command.name == name)
                .unwrap();
            assert!(!command.summary.is_empty());
            assert!(!command.examples.is_empty());
        }
    }

    #[test]
    fn export_discards_the_partial_record_and_keeps_complete_lines() {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "oxdm-partial-export-{}-{now}.jsonl",
            std::process::id()
        ));
        let mut file = File::create_new(&path).unwrap();
        let mut written = 0;
        write_export_batch(
            &mut file,
            &mut written,
            &mut [br#"{"record":1}"#.to_vec()],
            File::write_all,
        )
        .unwrap();
        let mut records = [br#"{"record":2}"#.to_vec(), br#"{"record":3}"#.to_vec()];
        let error = write_export_batch(&mut file, &mut written, &mut records, |file, record| {
            if record == b"{\"record\":3}\n" {
                file.write_all(&record[..5])?;
                return Err(io::Error::other("injected disk write failure"));
            }
            file.write_all(record)
        });
        drop(file);
        let contents = std::fs::read_to_string(&path);
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            error.unwrap_err().to_string(),
            "injected disk write failure"
        );
        assert_eq!(contents.unwrap(), "{\"record\":1}\n{\"record\":2}\n");
    }

    #[tokio::test]
    async fn a_full_export_queue_does_not_block_events_or_the_final_summary() {
        const EVENT: &str = concat!(
            r#"{"schema_version":"3","data":{"kind":"event_notification","sequence":1}}"#,
            "\n",
        );
        const GAP: &str = concat!(
            r#"{"schema_version":"3","data":{"kind":"event_gap","reason":"subscription_recovery"}}"#,
            "\n",
        );
        const SUMMARY: &str = concat!(
            r#"{"schema_version":"3","data":{"kind":"event_watch_summary","summary":{"reason":"interrupted","complete":false,"known_undelivered":0,"recoveries":1,"cleanup_failed":false,"counts":{"delivered":1,"gaps":1}}}}"#,
            "\n",
        );
        let run = Arc::new(Run {
            data: Mutex::new(Snapshot::default()),
            stop: Notify::new(),
        });
        let (sender, mut receiver) = mpsc::channel(1);
        let output = [EVENT, GAP, SUMMARY].concat();
        timeout(
            Duration::from_secs(1),
            read_output(
                run.clone(),
                output.as_bytes(),
                false,
                true,
                true,
                Some(sender),
            ),
        )
        .await
        .expect("reading the event stream blocked on the export queue")
        .unwrap();

        assert_eq!(receiver.try_recv().unwrap(), EVENT.as_bytes());
        assert_eq!(
            receiver.try_recv(),
            Err(mpsc::error::TryRecvError::Disconnected)
        );
        let state = run.data.lock().unwrap();
        assert_eq!(state.events, 1);
        assert_eq!(state.gaps, 1);
        assert_eq!(state.summary.as_ref().unwrap().reason, "interrupted");
        assert!(state.export_error.is_some());
    }
}
