use std::{net::IpAddr, ops::Range, path::PathBuf};

use serde::Deserialize;

use crate::{
    i18n,
    state::{Credentials, Locale},
};

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
    about: Option<String>,
    command_id: Option<String>,
    description: Option<Description>,
    console: Option<Console>,
    external_terminal: Option<String>,
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
    target_flag: Option<String>,
    output: String,
    control: Option<Control>,
    share_report_version: Option<u8>,
    #[serde(default)]
    root_options: Vec<String>,
    #[serde(default)]
    excluded_options: Vec<String>,
    workspace_change: Option<WorkspaceChange>,
    credential_input: Option<String>,
}

#[derive(Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum WorkspaceChange {
    Write,
    Remove,
    #[serde(other)]
    Unknown,
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
    help: Option<String>,
    help_id: Option<String>,
    conflicts: Vec<String>,
    value_source: Option<String>,
    #[serde(default)]
    value_kind: ValueKind,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum ValueKind {
    InputFile,
    OutputFile,
    Duration,
    WatchDuration,
    Concurrency,
    #[default]
    #[serde(other)]
    Text,
}

impl ValueKind {
    fn accepts(self, value: &str) -> bool {
        match self {
            Self::Concurrency => value.parse::<u8>().is_ok_and(|n| (1..=64).contains(&n)),
            Self::Duration => duration_value(value).is_some(),
            Self::WatchDuration => duration_value(value)
                .is_some_and(|duration| duration <= std::time::Duration::from_secs(3600)),
            _ => true,
        }
    }
}

fn duration_value(value: &str) -> Option<std::time::Duration> {
    use std::time::Duration;

    let (number, unit) = value.split_at(value.bytes().take_while(u8::is_ascii_digit).count());
    let number = number.parse::<u64>().ok().filter(|number| *number > 0)?;
    match unit {
        "ms" => Some(Duration::from_millis(number)),
        "s" | "" => Some(Duration::from_secs(number)),
        "m" => number.checked_mul(60).map(Duration::from_secs),
        _ => None,
    }
}

impl Argument {
    fn value_kind(&self) -> ValueKind {
        if self.id == "duration" {
            ValueKind::WatchDuration
        } else {
            self.value_kind
        }
    }

    fn kind(&self) -> ArgumentKind {
        match (self.position, self.takes_value(), self.repeatable) {
            (Some(_), _, _) => ArgumentKind::Positional,
            (_, true, _) => ArgumentKind::Value,
            (_, false, true) => ArgumentKind::Count,
            _ => ArgumentKind::Flag,
        }
    }

    pub fn label(&self) -> String {
        self.names()
            .into_iter()
            .next()
            .unwrap_or_else(|| self.id.clone())
    }

    pub(super) fn names(&self) -> Vec<String> {
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

    pub(super) fn takes_value(&self) -> bool {
        self.max_values != Some(0)
    }

    pub(super) fn values(&self) -> &[String] {
        &self.values
    }

    pub(super) fn repeatable(&self) -> bool {
        self.repeatable
    }

    pub(super) fn help_label(&self) -> String {
        if self.position.is_some() {
            self.label()
        } else {
            self.names().join(", ")
        }
    }

    pub(super) fn description(&self, locale: Locale) -> Option<&str> {
        i18n::console_option(locale, self.help_id.as_deref().unwrap_or(&self.id))
            .or(self.help.as_deref())
    }

    fn exposed(&self) -> bool {
        // Credentials and process control use dedicated channels.
        !matches!(
            self.id.as_str(),
            "control_stdin" | "username" | "password" | "password_stdin" | "help" | "version"
        )
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
    pub share_report: bool,
    pub control: Option<Control>,
    pub output: String,
    pub(super) workspace_change: Option<WorkspaceChange>,
    pub(super) import_plan: bool,
    pub(super) credential_input: bool,
    pub(super) credential: Option<CredentialInput>,
    pub(super) completion_script: bool,
}

pub(super) struct CredentialInput {
    pub username: zeroize::Zeroizing<String>,
    pub password: zeroize::Zeroizing<String>,
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
    pub media_profiles: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ArgumentChip {
    pub id: String,
    pub label: String,
    pub value: Option<String>,
    pub range: Range<usize>,
    pub choices: Vec<String>,
    pub kind: ArgumentKind,
    pub value_kind: ValueKind,
    pub suggestions: Vec<String>,
    other_occurrences: Vec<Range<usize>>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ArgumentKind {
    Flag,
    Count,
    Value,
    Positional,
}

impl ArgumentChip {
    fn replace(&self, input: &str, replacement: &str) -> (String, usize) {
        let mut next = input.to_owned();
        for range in self.other_occurrences.iter().rev() {
            next.replace_range(range.clone(), "");
        }
        next.replace_range(self.range.clone(), replacement);
        (next, self.range.start + replacement.len())
    }

    pub(super) fn removed(&self, input: &str) -> (String, usize) {
        self.replace(input, "")
    }

    pub(super) fn edited(&self, input: &str, value: &str) -> Option<(String, usize)> {
        if value.trim().is_empty()
            || value.chars().any(char::is_control)
            || !self.value_kind.accepts(value)
            || (!self.choices.is_empty() && !self.choices.iter().any(|choice| choice == value))
        {
            return None;
        }
        let replacement = match self.kind {
            ArgumentKind::Flag => self.label.clone(),
            ArgumentKind::Count => {
                let count = value.parse::<u8>().ok()?;
                if count == 0 {
                    return None;
                }
                // Clap Count uses a single argument group, so it also works across
                // the root and subcommand boundary.
                if self.id != "verbose" {
                    return None;
                }
                format!("-{}", "v".repeat(usize::from(count)))
            }
            ArgumentKind::Value => format!("{}={}", self.label, quote(value)),
            ArgumentKind::Positional
                if value.starts_with('-')
                    && !input[..self.range.start].trim_end().ends_with("--") =>
            {
                return None
            }
            ArgumentKind::Positional => quote(value),
        };
        Some(self.replace(input, &replacement))
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ParameterOption {
    pub label: String,
    pub detail: String,
    pub kind: ArgumentKind,
    pub choices: Vec<String>,
    pub value_kind: ValueKind,
    pub value_source: Option<String>,
}

impl ParameterOption {
    pub(super) fn insertion(&self, value: &str) -> Option<String> {
        match self.kind {
            ArgumentKind::Flag | ArgumentKind::Count => Some(self.label.clone()),
            ArgumentKind::Value | ArgumentKind::Positional
                if !value.trim().is_empty()
                    && !value.chars().any(char::is_control)
                    && self.value_kind.accepts(value)
                    && (self.choices.is_empty()
                        || self.choices.iter().any(|choice| choice == value)) =>
            {
                match self.kind {
                    ArgumentKind::Value => Some(format!("{}={}", self.label, quote(value))),
                    ArgumentKind::Positional if !value.starts_with('-') => Some(quote(value)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl DynamicValues {
    pub(super) fn suggestions(&self, source: Option<&str>) -> &[String] {
        match source {
            Some("event_topic") => &self.event_topics,
            Some("media_profile") => &self.media_profiles,
            _ => &[],
        }
    }

    fn for_argument(&self, argument: &Argument) -> &[String] {
        self.suggestions(argument.value_source.as_deref())
    }
}

pub struct Analysis {
    pub spans: Vec<(Range<usize>, Syntax)>,
    pub candidates: Vec<Candidate>,
    pub explicit_target: Option<String>,
}

pub(super) struct CommandHelp<'a> {
    pub path: String,
    pub summary: Option<&'a str>,
    pub subcommands: Vec<&'a Node>,
    pub arguments: Vec<&'a Argument>,
    pub template: Option<String>,
    pub watch: bool,
    pub camera: bool,
    pub workspace_selector: bool,
    pub external_terminal: bool,
    pub workspace_change: Option<WorkspaceChange>,
    pub credential_input: bool,
}

struct Scan<'a> {
    node: &'a Node,
    path: Vec<&'a Node>,
    pending: Option<&'a Argument>,
    seen: Vec<(&'a Argument, Option<String>, Range<usize>)>,
    spans: Vec<(Range<usize>, Syntax)>,
    invalid: bool,
    end_of_options: bool,
}

impl<'a> Scan<'a> {
    fn root_option(&self, arg: &Argument) -> bool {
        self.path[0]
            .arguments
            .iter()
            .any(|root| std::ptr::eq(root, arg))
            && self
                .node
                .console
                .as_ref()
                .is_some_and(|spec| spec.root_options.contains(&arg.id))
    }

    fn flags(&self) -> impl Iterator<Item = &'a Argument> + '_ {
        self.node
            .arguments
            .iter()
            .chain(
                self.path
                    .iter()
                    .filter(move |node| !std::ptr::eq(**node, self.node))
                    .flat_map(|node| {
                        node.arguments
                            .iter()
                            .filter(|arg| arg.global || self.root_option(arg))
                    }),
            )
            .filter(|arg| arg.position.is_none())
            .filter(|arg| {
                if std::ptr::eq(self.node, self.path[0]) {
                    arg.global
                        || self.path[0]
                            .subcommands
                            .iter()
                            .any(|node| node.has_root_option(&arg.id))
                } else {
                    self.node
                        .console
                        .as_ref()
                        .is_none_or(|spec| !spec.excluded_options.contains(&arg.id))
                }
            })
            .filter(|arg| {
                arg.id != "share"
                    || std::ptr::eq(self.node, self.path[0])
                    || self
                        .node
                        .console
                        .as_ref()
                        .is_some_and(Console::supports_share)
            })
    }

    fn positionals(&self) -> impl Iterator<Item = &'a Argument> + '_ {
        self.node.arguments.iter().filter(|arg| {
            arg.position.is_some()
                && arg.exposed()
                && (self.node.external_manage()
                    || self.node.console.as_ref().is_some_and(|spec| {
                        spec.control.is_none() && !spec.excluded_options.contains(&arg.id)
                    }))
        })
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
        end_of_options: false,
    };
    for (index, token) in tokens.iter().enumerate() {
        let value = token.value.as_str();
        let value_syntax = if token.quoted {
            Syntax::Quoted
        } else {
            Syntax::Value
        };
        let syntax = if let Some(arg) = result.pending.take() {
            if let Some((_, current, range)) = result.seen.last_mut() {
                *current = Some(value.to_owned());
                range.end = token.range.end;
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
        } else if value == "--"
            && !result.end_of_options
            && (result.node.console.is_some() || result.node.external_manage())
        {
            result.end_of_options = true;
            Syntax::Flag
        } else if value.starts_with('-') && !result.end_of_options {
            let (name, inline) = value
                .split_once('=')
                .map_or((value, None), |(n, v)| (n, Some(v)));
            let verbose_cluster = inline.is_none()
                && name.strip_prefix('-').is_some_and(|short| {
                    short.len() > 1 && short.len() <= 255 && short.bytes().all(|c| c == b'v')
                });
            let lookup = if verbose_cluster { "-v" } else { name };
            if let Some(arg) = result.flag(lookup).filter(|arg| arg.exposed()) {
                let clashes = result.seen.iter().any(|(prior, _, _)| {
                    (!arg.repeatable && arg.id == prior.id)
                        || arg.conflicts.contains(&prior.id)
                        || prior.conflicts.contains(&arg.id)
                });
                result.seen.push((
                    arg,
                    if verbose_cluster {
                        Some((name.len() - 1).to_string())
                    } else {
                        inline.map(str::to_owned)
                    },
                    token.range.clone(),
                ));
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
            !result.end_of_options
                && (child.name == value || child.aliases.iter().any(|alias| alias == value))
        }) {
            result.node = child;
            result.path.push(child);
            Syntax::Command
        } else if let Some(arg) = result
            .positionals()
            .filter(|arg| !result.has(&arg.id))
            .min_by_key(|arg| arg.position)
        {
            result
                .seen
                .push((arg, Some(value.to_owned()), token.range.clone()));
            value_syntax
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
    fn external_manage(&self) -> bool {
        self.command_id.as_deref() == Some("manage")
            && self.external_terminal.as_deref() == Some("manage_v1")
    }

    fn requires_saved_selector(&self) -> bool {
        matches!(
            self.command_id.as_deref(),
            Some("info" | "test" | "profiles" | "stream" | "snapshot" | "health")
        )
    }

    fn has_root_option(&self, id: &str) -> bool {
        self.console
            .as_ref()
            .is_some_and(|spec| spec.root_options.iter().any(|option| option == id))
            || self.subcommands.iter().any(|node| node.has_root_option(id))
    }

    pub(super) fn summary(&self, locale: Locale) -> Option<&str> {
        let id = self.command_id.as_deref().unwrap_or(&self.name);
        i18n::console_command(locale, id)
            .or_else(|| self.description.as_ref().map(|desc| desc.summary.as_str()))
            .or(self.about.as_deref())
    }

    fn has_console_command(&self, event_schema: bool) -> bool {
        self.external_manage()
            || self
                .console
                .as_ref()
                .is_some_and(|spec| spec.available(event_schema))
            || self
                .subcommands
                .iter()
                .any(|node| node.has_console_command(event_schema))
    }
}

impl Console {
    fn supports_share(&self) -> bool {
        self.share_report_version == Some(1)
    }

    fn available(&self, event_schema: bool) -> bool {
        self.workspace_change != Some(WorkspaceChange::Unknown)
            && self
                .credential_input
                .as_deref()
                .is_none_or(|mode| mode == "password_stdin_v1")
            && self
                .target_flag
                .as_deref()
                .is_none_or(|flag| flag == "--target")
            && match (&*self.output, &self.control) {
                ("json", None) => true,
                ("table", None) => true,
                ("jsonl", Some(control)) => {
                    self.target_flag.is_some() && event_schema && control.supported()
                }
                _ => false,
            }
    }
}

impl Grammar {
    #[cfg(test)]
    pub(super) fn command_coverage(&self) -> Vec<(String, &'static str)> {
        fn visit(
            node: &Node,
            path: String,
            event_schema: bool,
            rows: &mut Vec<(String, &'static str)>,
        ) {
            if node.command_id.is_some() {
                let mode = if node.external_manage() {
                    "外部終端"
                } else if node
                    .console
                    .as_ref()
                    .is_some_and(|console| console.available(event_schema))
                {
                    "指令台"
                } else {
                    "待支援"
                };
                rows.push((path.clone(), mode));
            }
            for child in &node.subcommands {
                let child_path = if path.is_empty() {
                    child.name.clone()
                } else {
                    format!("{path} {}", child.name)
                };
                visit(child, child_path, event_schema, rows);
            }
        }

        let mut rows = Vec::new();
        visit(
            &self.root,
            String::new(),
            self.event_schema_version == "3",
            &mut rows,
        );
        rows
    }

    #[cfg(test)]
    pub(super) fn missing_translations(&self, locale: Locale) -> Vec<String> {
        fn visit(grammar: &Grammar, path: &str, locale: Locale, missing: &mut Vec<String>) {
            let help = grammar.command_help(path, locale);
            for child in help.subcommands {
                let id = child.command_id.as_deref().unwrap_or(&child.name);
                let child_path = if path.is_empty() {
                    child.name.clone()
                } else {
                    format!("{path} {}", child.name)
                };
                let translated = match locale {
                    Locale::En => child.summary(locale).is_some(),
                    _ => i18n::console_command(locale, id).is_some(),
                };
                if !translated {
                    missing.push(child_path.clone());
                }
                visit(grammar, &child_path, locale, missing);
            }
            for argument in help.arguments {
                let translated = match locale {
                    Locale::En => argument.description(locale).is_some(),
                    _ => i18n::console_option(
                        locale,
                        argument.help_id.as_deref().unwrap_or(&argument.id),
                    )
                    .is_some(),
                };
                if !translated {
                    missing.push(format!("{path} {}", argument.label()));
                }
            }
        }

        let mut missing = Vec::new();
        visit(self, "", locale, &mut missing);
        missing
    }

    pub(super) fn argument_chips(&self, input: &str, dynamic: &DynamicValues) -> Vec<ArgumentChip> {
        let mut chips: Vec<ArgumentChip> = Vec::new();
        for (argument, value, range) in scan(&self.root, &tokenize(input)).seen {
            let kind = argument.kind();
            let count = value
                .as_deref()
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(1);
            if kind == ArgumentKind::Count {
                if let Some(chip) = chips
                    .iter_mut()
                    .find(|chip| chip.id == argument.id && chip.kind == ArgumentKind::Count)
                {
                    let total = chip
                        .value
                        .as_deref()
                        .and_then(|value| value.parse::<usize>().ok())
                        .unwrap_or(0)
                        + count;
                    chip.value = Some(total.to_string());
                    chip.other_occurrences.push(range);
                    continue;
                }
            }
            chips.push(ArgumentChip {
                id: argument.id.clone(),
                label: argument.label(),
                value: match kind {
                    ArgumentKind::Flag => Some("true".into()),
                    ArgumentKind::Count => Some(count.to_string()),
                    _ => value,
                },
                range,
                choices: argument.values.clone(),
                kind,
                value_kind: argument.value_kind(),
                suggestions: dynamic.for_argument(argument).to_vec(),
                other_occurrences: Vec::new(),
            });
        }
        chips
    }

    pub(super) fn available_arguments(
        &self,
        input: &str,
        filter: &str,
        locale: Locale,
    ) -> Vec<ParameterOption> {
        let parsed = scan(&self.root, &tokenize(input));
        if parsed.invalid
            || parsed.pending.is_some()
            || !parsed
                .node
                .console
                .as_ref()
                .is_some_and(|spec| spec.available(self.event_schema_version == "3"))
        {
            return Vec::new();
        }
        let filter = filter.trim().trim_start_matches('-').to_lowercase();
        let next_position = parsed
            .positionals()
            .filter(|arg| !parsed.has(&arg.id))
            .min_by_key(|arg| arg.position);
        parsed
            .flags()
            .filter(|_| !parsed.end_of_options)
            .chain(next_position)
            .filter(|argument| argument.exposed())
            .filter(|argument| {
                !parsed.seen.iter().any(|(prior, _, _)| {
                    (!argument.repeatable && argument.id == prior.id)
                        || argument.conflicts.contains(&prior.id)
                        || prior.conflicts.contains(&argument.id)
                })
            })
            .filter_map(|argument| {
                let label = argument.label();
                let description = argument.description(locale).unwrap_or_default();
                (label.contains(&filter) || description.to_lowercase().contains(&filter)).then(
                    || ParameterOption {
                        label,
                        detail: description.to_owned(),
                        kind: argument.kind(),
                        choices: argument.values.clone(),
                        value_kind: argument.value_kind(),
                        value_source: argument.value_source.clone(),
                    },
                )
            })
            .collect()
    }

    pub fn analyze(
        &self,
        input: &str,
        caret: usize,
        values: &DynamicValues,
        locale: Locale,
    ) -> Analysis {
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
            let dynamic = values.for_argument(arg);
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
                                detail: child.summary(locale).unwrap_or_default().to_owned(),
                                range: range.clone(),
                                replacement: name.clone(),
                            });
                        }
                    }
                }
            }
            if scan_before.node.console.is_some()
                && !scan_before.end_of_options
                && !prefix.is_empty()
                && scan_before
                    .positionals()
                    .all(|arg| !arg.required || scan_before.has(&arg.id))
            {
                for option in self.available_arguments(&input[..range.start], &prefix, locale) {
                    if option.kind == ArgumentKind::Positional {
                        continue;
                    }
                    let replacement = if option.kind == ArgumentKind::Value {
                        format!("{} ", option.label)
                    } else {
                        option.label.clone()
                    };
                    candidates.push(Candidate {
                        label: option.label,
                        detail: option.detail,
                        range: range.clone(),
                        replacement,
                    });
                }
            }
        }
        if (selected_argument.is_none() || (!prefix.contains('=') && scan_before.pending.is_none()))
            && prefix.starts_with('-')
        {
            for arg in scan_before.flags().filter(|arg| arg.exposed()) {
                let conflicts = full
                    .seen
                    .iter()
                    .filter(|(_, _, r)| r.start != range.start)
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
                                detail: arg.description(locale).unwrap_or_default().to_owned(),
                                range: range.clone(),
                                replacement: name,
                            });
                        }
                    }
                }
            }
        }
        candidates.truncate(40);
        Analysis {
            explicit_target: full.value("target").and_then(normalize_target),
            spans: full.spans,
            candidates,
        }
    }

    pub(super) fn command_help(&self, input: &str, locale: Locale) -> CommandHelp<'_> {
        let parsed = scan(&self.root, &tokenize(input));
        let event_schema = self.event_schema_version == "3";
        let path = parsed
            .path
            .iter()
            .skip(1)
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        let spec = parsed
            .node
            .console
            .as_ref()
            .filter(|spec| spec.available(event_schema));
        let watch = spec.is_some_and(|spec| spec.control.is_some());
        let template = spec.map(|spec| {
            if watch {
                format!("{path} --duration 30s")
            } else if spec.supports_share() {
                format!("{path} --share")
            } else if spec.credential_input.is_some() {
                if path == "setup" {
                    "setup 192.0.2.10 --id front-door".to_owned()
                } else {
                    format!("{path} front-door")
                }
            } else if parsed.node.arguments.iter().any(|arg| arg.required)
                || parsed.node.requires_saved_selector()
            {
                parsed
                    .node
                    .description
                    .as_ref()
                    .and_then(|description| description.examples.first())
                    .and_then(|example| example.strip_prefix("oxvif "))
                    .unwrap_or(&path)
                    .to_owned()
            } else {
                path.clone()
            }
        });
        let external_terminal = parsed.node.external_manage();
        let arguments = if external_terminal {
            parsed
                .node
                .arguments
                .iter()
                .filter(|arg| arg.exposed())
                .collect()
        } else {
            spec.map_or_else(Vec::new, |spec| {
                parsed
                    .flags()
                    .chain(parsed.positionals())
                    .filter(|arg| {
                        arg.exposed()
                            && (spec.control.is_none()
                                || !matches!(arg.id.as_str(), "json" | "jsonl" | "output"))
                    })
                    .collect()
            })
        };
        CommandHelp {
            path,
            summary: parsed.node.summary(locale),
            subcommands: parsed
                .node
                .subcommands
                .iter()
                .filter(|node| node.has_console_command(event_schema))
                .collect(),
            arguments,
            template,
            watch,
            camera: spec.is_some_and(|spec| spec.target_flag.is_some()),
            workspace_selector: spec
                .is_some_and(|spec| spec.root_options.iter().any(|option| option == "device")),
            external_terminal,
            workspace_change: spec.and_then(|spec| spec.workspace_change),
            credential_input: spec.is_some_and(|spec| spec.credential_input.is_some()),
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

    pub(super) fn needs_media_profiles(&self, input: &str, address: &str) -> bool {
        let tokens = tokenize(input);
        let parsed = scan(&self.root, &tokens);
        parsed
            .node
            .console
            .as_ref()
            .is_some_and(|console| console.target_flag.is_some())
            && parsed
                .flags()
                .any(|arg| arg.value_source.as_deref() == Some("media_profile"))
            && !["device", "group", "view", "id"]
                .into_iter()
                .any(|id| parsed.has(id))
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
        let mut workspace_change = spec.workspace_change;
        if parsed.node.command_id.as_deref() == Some("discover.scan") && parsed.has("save") {
            workspace_change = Some(WorkspaceChange::Write);
        }
        let credential_input = spec.credential_input.is_some();
        let completion_script = parsed.node.command_id.as_deref() == Some("completion");
        if credential_input && parsed.node.command_id.as_deref() == Some("setup") {
            if !parsed.has("target") || !parsed.has("id") {
                return Err("console_required_argument");
            }
            if parsed.value("target").and_then(normalize_target).is_none() {
                return Err("console_invalid_target");
            }
        }
        let import_plan =
            parsed.node.command_id.as_deref() == Some("device.import") && parsed.has("plan");
        if parsed.node.command_id.as_deref() == Some("device.import") {
            let apply = parsed.has("apply");
            if apply == parsed.has("plan")
                || (apply
                    && !parsed
                        .value("expect_plan")
                        .is_some_and(valid_import_fingerprint))
                || (!apply && parsed.has("expect_plan"))
            {
                return Err("console_import_plan_required");
            }
            if apply {
                workspace_change = Some(WorkspaceChange::Write);
            }
        }
        if parsed
            .flags()
            .chain(parsed.positionals())
            .any(|arg| arg.required && !parsed.has(&arg.id))
        {
            return Err("console_required_argument");
        }
        let scoped = |arg: &Argument| {
            self.root
                .arguments
                .iter()
                .any(|root| std::ptr::eq(root, arg))
        };
        if parsed
            .seen
            .iter()
            .any(|(arg, _, _)| scoped(arg) && !arg.global && !parsed.root_option(arg))
        {
            return Err("console_invalid_input");
        }
        if parsed
            .seen
            .iter()
            .any(|(arg, _, _)| spec.excluded_options.contains(&arg.id))
        {
            return Err("console_invalid_input");
        }
        let selected_url = camera.and_then(|camera| normalize_target(&camera.address));
        if parsed.seen.iter().any(|(arg, value, _)| {
            value.as_ref().is_some_and(|value| {
                (arg.position.is_some() && value.is_empty())
                    || (!arg.values.is_empty() && !arg.values.contains(value))
                    || !arg.value_kind().accepts(value)
            })
        }) {
            return Err("console_invalid_input");
        }
        if ["add_target", "updated_target"].into_iter().any(|id| {
            parsed
                .value(id)
                .is_some_and(|target| normalize_target(target).is_none())
        }) {
            return Err("console_invalid_target");
        }
        let explicit = parsed.value("target");
        let fleet = parsed
            .seen
            .iter()
            .any(|(arg, _, _)| scoped(arg) && matches!(arg.id.as_str(), "group" | "view"));
        let saved = parsed.has("device") || (spec.target_flag.is_some() && parsed.has("id"));
        if (explicit.is_some() && (saved || fleet))
            || (saved && fleet)
            || (parsed.has("id") && parsed.has("device"))
            || (parsed.has("jobs")
                && !fleet
                && parsed.node.command_id.as_deref() != Some("discover.enrich"))
            || (parsed.has("share") && fleet)
        {
            return Err("console_invalid_input");
        }
        if parsed.node.requires_saved_selector() && !(saved || fleet || parsed.has("id")) {
            return Err("console_workspace_selector_required");
        }
        let needs_profile = matches!(
            parsed.node.command_id.as_deref(),
            Some("stream" | "snapshot")
        ) || (parsed.node.command_id.as_deref() == Some("inspect")
            && parsed.value("read") == Some("stream-uri"));
        if needs_profile && !parsed.has("profile") {
            return Err("console_required_argument");
        }
        let target = match (spec.target_flag.as_ref(), explicit, saved || fleet) {
            (Some(_), Some(address), _) => {
                Some(normalize_target(address).ok_or("console_invalid_target")?)
            }
            (Some(_), None, false) => Some(selected_url.clone().ok_or("console_missing_target")?),
            _ => None,
        };
        let camera = camera.filter(|_| target.is_some() && selected_url == target);
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
        let share_report = parsed.has("share");
        if import_plan && output != "json" {
            return Err("console_import_json_required");
        }
        if share_report && output != "json" {
            return Err("console_share_json");
        }
        let root_ranges: Vec<_> = parsed
            .seen
            .iter()
            .filter(|(arg, _, _)| parsed.root_option(arg))
            .map(|(_, _, range)| range)
            .collect();
        let (mut argv, command_tokens): (Vec<_>, Vec<_>) = tokens
            .iter()
            .enumerate()
            .filter(|(index, token)| !(*index == 0 && token.value == "oxvif"))
            .partition(|(_, token)| {
                root_ranges
                    .iter()
                    .any(|range| range.contains(&token.range.start))
            });
        let mut argv: Vec<_> = argv
            .drain(..)
            .map(|(_, token)| token.value.clone())
            .collect();
        // The inserted global and camera flags must precede an explicit end-of-options marker.
        let end_of_options = command_tokens
            .iter()
            .position(|(index, token)| {
                token.value == "--" && parsed.spans[*index].1 == Syntax::Flag
            })
            .unwrap_or(command_tokens.len());
        argv.extend(
            command_tokens[..end_of_options]
                .iter()
                .map(|(_, token)| token.value.clone()),
        );
        if !parsed.has("non_interactive") {
            argv.push("--non-interactive".into());
        }
        if !parsed.has("output") && !parsed.has("json") && !parsed.has("jsonl") {
            argv.extend(["--output".into(), output.into()]);
        }
        if let (None, Some((flag, address))) =
            (explicit, spec.target_flag.as_ref().zip(target.as_ref()))
        {
            argv.extend([flag.clone(), address.clone()]);
        }
        argv.extend(
            command_tokens[end_of_options..]
                .iter()
                .map(|(_, token)| token.value.clone()),
        );
        Ok(Invocation {
            executable,
            argv,
            target: target.unwrap_or_default(),
            camera_name,
            credentials,
            share_report,
            control: spec.control.clone(),
            output: output.to_owned(),
            workspace_change,
            import_plan,
            credential_input,
            credential: None,
            completion_script,
        })
    }

    pub(super) fn external_terminal_args(
        &self,
        input: &str,
    ) -> Option<Result<Vec<String>, &'static str>> {
        let tokens = tokenize(input);
        let parsed = scan(&self.root, &tokens);
        if !parsed.node.external_manage() {
            return None;
        }
        if parsed.invalid
            || parsed.pending.is_some()
            || input.chars().any(char::is_control)
            || parsed
                .seen
                .iter()
                .any(|(arg, _, _)| !matches!(arg.id.as_str(), "target" | "id"))
            || (parsed.has("target") && parsed.has("id"))
        {
            return Some(Err("console_invalid_input"));
        }
        let mut argv = vec!["manage".to_owned()];
        if let Some(target) = parsed.value("target") {
            let Some(target) = normalize_target(target) else {
                return Some(Err("console_invalid_target"));
            };
            argv.extend(["--target".to_owned(), target]);
        } else if let Some(id) = parsed.value("id") {
            argv.extend(["--".to_owned(), id.to_owned()]);
        }
        Some(Ok(argv))
    }

    pub(super) fn apply_import_draft(&self, input: &str, fingerprint: &str) -> Option<String> {
        if !valid_import_fingerprint(fingerprint) {
            return None;
        }
        let parsed = scan(&self.root, &tokenize(input));
        if parsed.invalid || parsed.node.command_id.as_deref() != Some("device.import") {
            return None;
        }
        let (_, _, range) = parsed.seen.iter().find(|(arg, _, _)| arg.id == "plan")?;
        let mut next = input.to_owned();
        next.replace_range(
            range.clone(),
            &format!("--apply --expect-plan={fingerprint}"),
        );
        Some(next)
    }
}

pub(super) fn valid_import_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
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
