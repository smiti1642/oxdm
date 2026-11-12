use std::{net::IpAddr, ops::Range, path::PathBuf};

use serde::Deserialize;

use crate::state::Credentials;

#[derive(Deserialize)]
pub struct Envelope {
    schema_version: String,
    ok: bool,
    data: Grammar,
}

#[derive(Deserialize)]
pub struct Grammar {
    kind: String,
    grammar_version: u32,
    pub cli_version: String,
    event_schema_version: String,
    pub root: Node,
}

impl Envelope {
    pub fn parse(bytes: &[u8]) -> Result<Grammar, String> {
        let envelope: Self = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
        let data = envelope.data;
        if !envelope.ok
            || envelope.schema_version != "3"
            || data.kind != "command_grammar"
            || data.grammar_version != 1
            || data.root.name != "oxvif"
        {
            return Err("Unsupported oxvif command grammar".into());
        }
        Ok(data)
    }
}

#[derive(Deserialize)]
pub struct Node {
    pub name: String,
    aliases: Vec<String>,
    pub about: Option<String>,
    pub description: Option<Description>,
    console: Option<Console>,
    pub arguments: Vec<Argument>,
    subcommands: Vec<Self>,
}

#[derive(Deserialize)]
pub struct Description {
    pub name: String,
    pub summary: String,
    pub examples: Vec<String>,
}

#[derive(Deserialize)]
struct Console {
    target_flag: String,
    output: String,
    control: Option<Control>,
}

#[derive(Clone, Deserialize)]
pub struct Control {
    kind: String,
    pub flag: String,
    pub stop_grace_ms: u64,
}

impl Control {
    fn supported(&self) -> bool {
        self.kind == "stdin_stop_v1"
            && self.flag == "--control-stdin"
            && (1_000..=60_000).contains(&self.stop_grace_ms)
    }
}

#[derive(Deserialize)]
pub struct Argument {
    id: String,
    long: Option<String>,
    short: Option<char>,
    long_aliases: Vec<String>,
    short_aliases: Vec<char>,
    position: Option<usize>,
    global: bool,
    pub required: bool,
    max_values: Option<usize>,
    repeatable: bool,
    values: Vec<String>,
    pub defaults: Vec<String>,
    pub help: Option<String>,
    conflicts: Vec<String>,
    value_source: Option<String>,
}

impl Argument {
    pub fn label(&self) -> String {
        self.names()
            .into_iter()
            .next()
            .unwrap_or_else(|| self.id.clone())
    }

    fn names(&self) -> Vec<String> {
        self.long
            .iter()
            .chain(&self.long_aliases)
            .map(|s| format!("--{s}"))
            .chain(
                self.short
                    .iter()
                    .chain(&self.short_aliases)
                    .map(|s| format!("-{s}")),
            )
            .collect()
    }

    fn matches(&self, name: &str) -> bool {
        self.names().iter().any(|spelling| spelling == name)
    }

    fn takes_value(&self) -> bool {
        self.max_values != Some(0)
    }

    fn exposed(&self) -> bool {
        // The app supplies control, single-device selection and execution budgets.
        !matches!(
            self.id.as_str(),
            "control_stdin"
                | "device"
                | "group"
                | "view"
                | "jobs"
                | "timeout"
                | "retries"
                | "help"
                | "version"
        ) && self.position.is_none()
    }
}

#[derive(Clone, PartialEq)]
pub struct Camera {
    pub name: String,
    pub address: String,
    pub credentials: Credentials,
}

pub struct Invocation {
    pub executable: PathBuf,
    pub argv: Vec<String>,
    pub target: String,
    pub camera_name: Option<String>,
    pub credentials: Option<Credentials>,
    pub control: Option<Control>,
    pub output: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Syntax {
    Command,
    Flag,
    Value,
    Quoted,
    Invalid,
}

impl Syntax {
    pub fn class(self) -> &'static str {
        match self {
            Self::Command => "console-syn-command",
            Self::Flag => "console-syn-flag",
            Self::Value => "console-syn-value",
            Self::Quoted => "console-syn-quoted",
            Self::Invalid => "console-syn-invalid",
        }
    }
}

struct Token {
    range: Range<usize>,
    value: String,
    quoted: bool,
    complete: bool,
}

fn tokenize(input: &str) -> Vec<Token> {
    let mut chars = input.char_indices().peekable();
    let mut tokens = Vec::new();
    while let Some((start, first)) = chars.peek().copied() {
        if first.is_whitespace() {
            chars.next();
            continue;
        }
        let mut value = String::new();
        let mut quote = None;
        let mut quoted = false;
        while let Some((_, ch)) = chars.peek().copied() {
            if quote.is_none() && ch.is_whitespace() {
                break;
            }
            chars.next();
            match ch {
                ch if quote == Some(ch) => quote = None,
                '\'' | '"' if quote.is_none() => {
                    quote = Some(ch);
                    quoted = true;
                }
                '\\' if quote != Some('\'') => {
                    let escaped = chars.peek().is_some_and(|&(_, next)| match quote {
                        Some('"') => matches!(next, '"' | '\\'),
                        None => matches!(next, '"' | '\'') || next.is_whitespace(),
                        _ => false,
                    });
                    if escaped {
                        value.push(chars.next().unwrap().1);
                    } else {
                        value.push(ch);
                    }
                }
                _ => value.push(ch),
            }
        }
        let end = chars.peek().map_or(input.len(), |&(index, _)| index);
        tokens.push(Token {
            range: start..end,
            value,
            quoted,
            complete: quote.is_none(),
        });
    }
    tokens
}

#[derive(Clone)]
pub struct Candidate {
    pub label: String,
    pub detail: String,
    pub range: Range<usize>,
    pub replacement: String,
}

#[derive(Default)]
pub struct DynamicValues {
    pub event_topics: Vec<String>,
}

pub struct Analysis<'a> {
    pub node: &'a Node,
    pub selected_argument: Option<&'a Argument>,
    pub spans: Vec<(Range<usize>, Syntax)>,
    pub candidates: Vec<Candidate>,
    pub template: Option<String>,
}

struct Scan<'a> {
    node: &'a Node,
    path: Vec<&'a Node>,
    pending: Option<&'a Argument>,
    seen: Vec<(&'a Argument, Option<String>, Range<usize>)>,
    spans: Vec<(Range<usize>, Syntax)>,
    invalid: bool,
}

impl<'a> Scan<'a> {
    fn flags(&self) -> impl Iterator<Item = &'a Argument> + '_ {
        self.node
            .arguments
            .iter()
            .chain(
                self.path
                    .iter()
                    .filter(move |node| !std::ptr::eq(**node, self.node))
                    .flat_map(|node| node.arguments.iter().filter(|arg| arg.global)),
            )
            .filter(|arg| arg.position.is_none())
    }

    fn flag(&self, name: &str) -> Option<&'a Argument> {
        self.flags().find(|arg| arg.matches(name))
    }

    fn value(&self, id: &str) -> Option<&str> {
        self.seen
            .iter()
            .find(|(arg, _, _)| arg.id == id)
            .and_then(|(_, value, _)| value.as_deref())
    }

    fn has(&self, id: &str) -> bool {
        self.seen.iter().any(|(arg, _, _)| arg.id == id)
    }
}

fn scan<'a>(root: &'a Node, tokens: &[Token]) -> Scan<'a> {
    let mut result = Scan {
        node: root,
        path: vec![root],
        pending: None,
        seen: Vec::new(),
        spans: Vec::new(),
        invalid: false,
    };
    for (index, token) in tokens.iter().enumerate() {
        let value = token.value.as_str();
        let value_syntax = if token.quoted {
            Syntax::Quoted
        } else {
            Syntax::Value
        };
        let syntax =
            if let Some(arg) = result.pending.take() {
                if let Some((_, current, _)) = result.seen.last_mut() {
                    *current = Some(value.to_owned());
                }
                if (arg.values.is_empty() || arg.values.iter().any(|v| v == value))
                    && !value.starts_with("--")
                {
                    value_syntax
                } else {
                    Syntax::Invalid
                }
            } else if index == 0 && value == "oxvif" {
                Syntax::Command
            } else if value.starts_with('-') {
                let (name, inline) = value
                    .split_once('=')
                    .map_or((value, None), |(n, v)| (n, Some(v)));
                if let Some(arg) = result.flag(name).filter(|arg| arg.exposed()) {
                    let clashes = result.seen.iter().any(|(prior, _, _)| {
                        (!arg.repeatable && arg.id == prior.id)
                            || arg.conflicts.contains(&prior.id)
                            || prior.conflicts.contains(&arg.id)
                    });
                    result
                        .seen
                        .push((arg, inline.map(str::to_owned), token.range.clone()));
                    if arg.takes_value() && inline.is_none() {
                        result.pending = Some(arg);
                    }
                    if clashes
                        || inline.is_some_and(|v| {
                            !arg.takes_value()
                                || (!arg.values.is_empty()
                                    && !arg.values.iter().any(|option| option == v))
                        })
                    {
                        Syntax::Invalid
                    } else {
                        Syntax::Flag
                    }
                } else {
                    Syntax::Invalid
                }
            } else if let Some(child) = result.node.subcommands.iter().find(|child| {
                child.name == value || child.aliases.iter().any(|alias| alias == value)
            }) {
                result.node = child;
                result.path.push(child);
                Syntax::Command
            } else {
                Syntax::Invalid
            };
        let syntax = if token.complete {
            syntax
        } else {
            Syntax::Invalid
        };
        result.invalid |= syntax == Syntax::Invalid;
        result.spans.push((token.range.clone(), syntax));
    }
    result
}

impl Node {
    fn has_console_command(&self, event_schema: bool) -> bool {
        self.console
            .as_ref()
            .is_some_and(|spec| spec.available(event_schema))
            || self
                .subcommands
                .iter()
                .any(|node| node.has_console_command(event_schema))
    }
}

impl Console {
    fn available(&self, event_schema: bool) -> bool {
        self.target_flag == "--target"
            && match (&*self.output, &self.control) {
                ("json", None) => true,
                ("jsonl", Some(control)) => event_schema && control.supported(),
                _ => false,
            }
    }
}

impl Grammar {
    pub fn analyze<'a>(
        &'a self,
        input: &str,
        caret: usize,
        values: &DynamicValues,
    ) -> Analysis<'a> {
        let tokens = tokenize(input);
        let active = tokens
            .iter()
            .position(|token| token.range.start <= caret && caret <= token.range.end);
        let before =
            active.unwrap_or_else(|| tokens.partition_point(|token| token.range.end < caret));
        let scan_before = scan(&self.root, &tokens[..before]);
        let full = scan(&self.root, &tokens);
        let range = active.map_or(caret..caret, |index| tokens[index].range.clone());
        let raw_prefix = input.get(range.start..caret).unwrap_or("");
        let prefix = tokenize(raw_prefix)
            .into_iter()
            .next()
            .map_or(String::new(), |token| token.value);
        let mut selected_argument = scan_before.pending;
        let mut candidates = Vec::new();
        let mut candidate_range = range.clone();
        let mut value_prefix = prefix.as_str();
        if let Some((name, value)) = prefix
            .split_once('=')
            .filter(|(name, _)| raw_prefix.starts_with(name) && name.starts_with("--"))
        {
            selected_argument = scan_before
                .flag(name)
                .filter(|arg| arg.exposed() && arg.takes_value());
            candidate_range.start += name.len() + 1;
            value_prefix = value;
        } else if prefix.starts_with('-') && selected_argument.is_none() {
            selected_argument = scan_before.flag(&prefix);
        }
        if let Some(arg) =
            selected_argument.filter(|_| scan_before.pending.is_some() || value_prefix != prefix)
        {
            let dynamic = match arg.value_source.as_deref() {
                Some("event_topic") => values.event_topics.as_slice(),
                _ => &[],
            };
            for value in arg.values.iter().chain(dynamic).chain(
                arg.defaults
                    .iter()
                    .filter(|value| !arg.values.contains(value)),
            ) {
                if value.starts_with(value_prefix) && value != value_prefix {
                    candidates.push(Candidate {
                        label: value.clone(),
                        detail: arg.label(),
                        range: candidate_range.clone(),
                        replacement: quote(value),
                    });
                }
            }
        } else if !prefix.starts_with('-') {
            for child in &scan_before.node.subcommands {
                if child.has_console_command(self.event_schema_version == "3") {
                    for name in std::iter::once(&child.name).chain(&child.aliases) {
                        if name.starts_with(&prefix) && name != &prefix {
                            candidates.push(Candidate {
                                label: name.clone(),
                                detail: child.about.clone().unwrap_or_default(),
                                range: range.clone(),
                                replacement: name.clone(),
                            });
                        }
                    }
                }
            }
        }
        if (selected_argument.is_none() || (!prefix.contains('=') && scan_before.pending.is_none()))
            && (prefix.is_empty() || prefix.starts_with('-'))
        {
            for arg in scan_before.flags().filter(|arg| arg.exposed()) {
                let conflicts =
                    full.seen
                        .iter()
                        .filter(|(_, _, r)| *r != range)
                        .any(|(prior, _, _)| {
                            (!arg.repeatable && arg.id == prior.id)
                                || arg.conflicts.contains(&prior.id)
                                || prior.conflicts.contains(&arg.id)
                        });
                if !conflicts {
                    for name in arg.names() {
                        if name.starts_with(&prefix) && name != prefix {
                            candidates.push(Candidate {
                                label: name.clone(),
                                detail: arg.help.clone().unwrap_or_default(),
                                range: range.clone(),
                                replacement: name,
                            });
                        }
                    }
                }
            }
        }
        candidates.truncate(40);
        let template = full
            .node
            .console
            .as_ref()
            .filter(|spec| spec.available(self.event_schema_version == "3"))
            .map(|spec| {
                let mut command = full
                    .path
                    .iter()
                    .skip(1)
                    .map(|node| node.name.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                if spec.control.is_some() {
                    command.push_str(" --duration 30s");
                }
                command
            });
        Analysis {
            node: full.node,
            selected_argument,
            spans: full.spans,
            candidates,
            template,
        }
    }

    pub fn needs_camera_topics(&self, input: &str, address: &str) -> bool {
        let tokens = tokenize(input);
        let parsed = scan(&self.root, &tokens);
        parsed
            .seen
            .iter()
            .any(|(arg, _, _)| arg.value_source.as_deref() == Some("event_topic"))
            && parsed
                .value("target")
                .is_none_or(|target| normalize_target(target) == normalize_target(address))
    }

    pub fn prepare(
        &self,
        executable: PathBuf,
        input: &str,
        camera: Option<&Camera>,
    ) -> Result<Invocation, &'static str> {
        if input.chars().any(|ch| ch.is_control() && ch != '\t') {
            return Err("console_invalid_input");
        }
        let tokens = tokenize(input);
        let parsed = scan(&self.root, &tokens);
        if parsed.invalid || parsed.pending.is_some() {
            return Err("console_invalid_input");
        }
        let spec = parsed
            .node
            .console
            .as_ref()
            .filter(|spec| spec.available(self.event_schema_version == "3"))
            .ok_or("console_unsupported_command")?;
        let selected_url = camera.and_then(|camera| normalize_target(&camera.address));
        let explicit = parsed.value("target");
        let target = match explicit {
            Some(address) => normalize_target(address).ok_or("console_invalid_target")?,
            None => selected_url.clone().ok_or("console_missing_target")?,
        };
        let camera = camera.filter(|_| selected_url.as_ref() == Some(&target));
        let camera_name = camera.map(|camera| camera.name.clone());
        let credentials = camera.map(|camera| camera.credentials.clone());
        let output = parsed.value("output").unwrap_or(if parsed.has("json") {
            "json"
        } else if parsed.has("jsonl") {
            "jsonl"
        } else {
            &spec.output
        });
        if spec.control.is_some() && output != "jsonl" {
            return Err("console_watch_jsonl");
        }
        let mut argv: Vec<_> = tokens
            .iter()
            .enumerate()
            .filter(|(index, token)| !(*index == 0 && token.value == "oxvif"))
            .map(|(_, token)| token.value.clone())
            .collect();
        if !parsed.has("non_interactive") {
            argv.push("--non-interactive".into());
        }
        if !parsed.has("output") && !parsed.has("json") && !parsed.has("jsonl") {
            argv.extend(["--output".into(), output.into()]);
        }
        if explicit.is_none() {
            argv.extend([spec.target_flag.clone(), target.clone()]);
        }
        Ok(Invocation {
            executable,
            argv,
            target,
            camera_name,
            credentials,
            control: spec.control.clone(),
            output: output.to_owned(),
        })
    }
}

pub fn normalize_target(address: &str) -> Option<String> {
    let address = address.trim();
    if address.is_empty() {
        return None;
    }
    let candidate = if address.contains("://") {
        address.to_owned()
    } else if address.parse::<IpAddr>().is_ok_and(|ip| ip.is_ipv6()) {
        format!("http://[{address}]/onvif/device_service")
    } else {
        format!("http://{address}/onvif/device_service")
    };
    let url = reqwest::Url::parse(&candidate).ok()?;
    (matches!(url.scheme(), "http" | "https")
        && url.host().is_some()
        && url.username().is_empty()
        && url.password().is_none())
    .then(|| url.into())
}

pub fn quote(value: &str) -> String {
    if !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_alphanumeric() || "_-.:/".contains(c))
    {
        value.to_owned()
    } else {
        format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
    }
}

pub fn shell_command(arguments: &[String]) -> String {
    let mut command = String::from("oxvif");
    for value in arguments {
        command.push(' ');
        if quote(value) == *value {
            command.push_str(value);
        } else {
            command.push('\'');
            #[cfg(windows)]
            command.push_str(&value.replace('\'', "''"));
            #[cfg(not(windows))]
            command.push_str(&value.replace('\'', "'\\''"));
            command.push('\'');
        }
    }
    command
}
