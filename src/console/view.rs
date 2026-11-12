use std::{path::PathBuf, rc::Rc, time::Duration};

use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;

use crate::{
    api, i18n,
    state::{Ctx, Locale, ToastLevel},
    util,
};

use super::{
    grammar::{shell_command, Camera, DynamicValues, Grammar, Syntax},
    process::{OutputKind, OutputLine, Snapshot},
    ConsoleState,
};

const EDITOR_BRIDGE: &str = include_str!("../../assets/console-editor.js");
const CONSOLE_CSS: &str = include_str!("../../assets/console.css");

#[derive(Deserialize)]
struct EditorEvent {
    action: String,
    text: String,
    caret: usize,
}

fn byte_offset(text: &str, utf16_offset: usize) -> usize {
    let mut index = 0;
    for (start, ch) in text.char_indices() {
        if index >= utf16_offset || index + ch.len_utf16() > utf16_offset {
            return start;
        }
        index += ch.len_utf16();
    }
    text.len()
}

fn set_editor(text: String, caret: usize, expected: &str) {
    let caret = text[..caret].encode_utf16().count();
    let eval = document::eval(
        r#"
        const { text, caret, expected } = await dioxus.recv();
        const input = document.getElementById("oxdm-console-input");
        if (input && input.value === expected) {
            input.focus();
            input.setRangeText(text, 0, input.value.length, "end");
            input.setSelectionRange(caret, caret);
            input.dispatchEvent(new Event("console-replace"));
        }
    "#,
    );
    let _ = eval.send(json!({ "text": text, "caret": caret, "expected": expected }));
}

fn use_camera(ctx: Ctx) -> Memo<Option<Camera>> {
    use_memo(move || {
        let devices = ctx.devices.read();
        let selected = (*ctx.selected.read())?;
        let device = devices.get(selected)?;
        let _global = ctx.global_credentials.read();
        Some(Camera {
            name: device.name.clone(),
            address: device.addr.clone(),
            credentials: ctx.credentials_for(device),
        })
    })
}

fn current_grammar(state: ConsoleState) -> Option<(PathBuf, Rc<Grammar>)> {
    let current = state.executable.read();
    let resource = state.grammar.read();
    let (loaded, result) = resource.as_ref()?.as_ref()?;
    (current.as_ref() == Some(loaded))
        .then(|| result.as_ref().ok().map(|g| (loaded.clone(), g.clone())))
        .flatten()
}

fn execute(
    mut state: ConsoleState,
    camera: Option<&Camera>,
    mut error: Signal<Option<&'static str>>,
) {
    let Some((path, grammar)) = current_grammar(state) else {
        return;
    };
    let text = state.draft.peek().clone();
    let request = match grammar.prepare(path, &text, camera) {
        Ok(request) => request,
        Err(reason) => {
            error.set(Some(reason));
            return;
        }
    };
    let command = shell_command(&request.argv);
    let export = request
        .control
        .as_ref()
        .and_then(|_| state.export.peek().clone());
    if state.controller.peek().start(request, command, export) {
        let mut history = state.history.write();
        if history.last() != Some(&text) {
            history.push(text);
            if history.len() > 40 {
                history.remove(0);
            }
        }
        state.export.set(None);
        error.set(None);
    }
}

#[component]
pub fn ConsoleView() -> Element {
    let ctx = use_context::<Ctx>();
    let mut state = use_context::<ConsoleState>();
    let locale = *ctx.locale.read();
    let tr = |key| i18n::t(locale, key);
    let camera = use_camera(ctx);
    let mut caret = use_signal(|| state.draft.peek().len());
    let mut menu = use_signal(|| false);
    let mut candidate_index = use_signal(|| 0usize);
    let mut history_index = use_signal(|| None::<usize>);
    let mut saved_draft = use_signal(String::new);
    let mut error = use_signal(|| None::<&'static str>);
    let mut snapshot = use_signal(|| None::<Snapshot>);
    let follow = use_signal(|| true);

    use_effect(move || state.requested.set(true));
    use_effect(move || {
        if *menu.read() {
            let eval = document::eval(
                r#"
                const index = await dioxus.recv();
                document.getElementById("oxdm-console-option-" + index)?.scrollIntoView({ block: "nearest" });
            "#,
            );
            let _ = eval.send(*candidate_index.read());
        }
    });
    use_future(move || async move {
        let controller = state.controller.peek().clone();
        let mut revision = 0;
        loop {
            if let Some(next) = controller.snapshot_after(revision) {
                revision = next.revision;
                snapshot.set(Some(next));
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    });
    use_effect(move || {
        if snapshot.read().is_some() && *follow.read() {
            let _ = document::eval(
                r#"
                requestAnimationFrame(() => {
                    const output = document.getElementById("oxdm-console-output");
                    if (output) output.scrollTop = output.scrollHeight;
                });
            "#,
            );
        }
    });

    let needs_topics = use_memo(move || {
        current_grammar(state).is_some_and(|(_, grammar)| {
            camera.read().as_ref().is_some_and(|camera| {
                grammar.needs_camera_topics(&state.draft.read(), &camera.address)
            })
        })
    });
    let topics = use_resource(move || {
        let source = (*needs_topics.read())
            .then(|| camera.read().clone())
            .flatten();
        async move {
            let camera = source?;
            let props = api::get_event_properties(&camera.address, &camera.credentials)
                .await
                .ok()?;
            let mut topics = props.topics;
            topics.sort_unstable();
            topics.dedup();
            topics.truncate(2048);
            Some((camera, topics))
        }
    });
    let dynamic = move || DynamicValues {
        event_topics: topics
            .peek()
            .as_ref()
            .and_then(|result| result.as_ref())
            .filter(|(source, _)| *needs_topics.peek() && camera.peek().as_ref() == Some(source))
            .map_or_else(Vec::new, |(_, values)| values.clone()),
    };

    let editor = move |_| {
        let mut bridge = document::eval(EDITOR_BRIDGE);
        let _ = bridge.send(state.draft.peek().as_str());
        spawn(async move {
            while let Ok(event) = bridge.recv::<EditorEvent>().await {
                let position = byte_offset(&event.text, event.caret);
                let changed = *state.draft.peek() != event.text || *caret.peek() != position;
                if changed {
                    state.draft.set(event.text);
                    caret.set(position);
                    error.set(None);
                }
                match event.action.as_str() {
                    "change" | "focus" => {
                        if changed || event.action == "focus" {
                            menu.set(true);
                            candidate_index.set(0);
                            history_index.set(None);
                        }
                    }
                    "blur" | "Escape" | "accept" | "replace" => menu.set(false),
                    "multiline" => error.set(Some("console_invalid_input")),
                    action => {
                        let list = current_grammar(state)
                            .filter(|_| *menu.peek())
                            .map(|(_, grammar)| {
                                grammar
                                    .analyze(&state.draft.peek(), *caret.peek(), &dynamic())
                                    .candidates
                            })
                            .unwrap_or_default();
                        let index = (*candidate_index.peek()).min(list.len().saturating_sub(1));
                        match action {
                            "ArrowUp" if !list.is_empty() => {
                                candidate_index.set(index.saturating_sub(1))
                            }
                            "ArrowDown" if !list.is_empty() => {
                                candidate_index.set((index + 1).min(list.len() - 1))
                            }
                            "Enter" | "run" => execute(state, camera.peek().as_ref(), error),
                            "ArrowUp" | "ArrowDown" => {
                                let history = state.history.peek();
                                if history.is_empty() {
                                    continue;
                                }
                                let prior = *history_index.peek();
                                if prior.is_none() && action == "ArrowUp" {
                                    saved_draft.set(state.draft.peek().clone());
                                }
                                let next = match (action, prior) {
                                    ("ArrowUp", Some(index)) => Some(index.saturating_sub(1)),
                                    ("ArrowUp", None) => Some(history.len() - 1),
                                    ("ArrowDown", Some(index)) if index + 1 < history.len() => {
                                        Some(index + 1)
                                    }
                                    ("ArrowDown", Some(_)) => None,
                                    _ => continue,
                                };
                                let text = next.map_or_else(
                                    || saved_draft.peek().clone(),
                                    |index| history[index].clone(),
                                );
                                let end = text.len();
                                history_index.set(next);
                                set_editor(text, end, state.draft.peek().as_str());
                            }
                            _ => {}
                        }
                    }
                }
            }
        });
    };

    let text = state.draft.read().clone();
    let loaded = current_grammar(state);
    let _topics_revision = topics.read();
    let analysis = loaded
        .as_ref()
        .map(|(_, grammar)| grammar.analyze(&text, (*caret.read()).min(text.len()), &dynamic()));
    let paint = input_spans(&text, analysis.as_ref().map_or(&[], |a| &a.spans));
    let suggestions = analysis
        .as_ref()
        .map_or(&[][..], |a| a.candidates.as_slice());
    let suggestions_open = *menu.read() && !suggestions.is_empty();
    let index = (*candidate_index.read()).min(suggestions.len().saturating_sub(1));
    let preview = loaded.as_ref().and_then(|(path, grammar)| {
        grammar
            .prepare(path.clone(), &text, camera.read().as_ref())
            .ok()
    });
    let current_snapshot = snapshot.read();
    let busy = current_snapshot
        .as_ref()
        .is_some_and(|s| s.finished.is_none());
    let export = state.export.read();
    let path = state.executable.read();
    let path_text = path
        .as_ref()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let load_error = state
        .grammar
        .read()
        .as_ref()
        .and_then(|value| value.as_ref())
        .and_then(|(loaded, result)| {
            (path.as_ref() == Some(loaded))
                .then(|| result.as_ref().err().cloned())
                .flatten()
        });
    let target = preview
        .as_ref()
        .map(|request| {
            request.camera_name.as_ref().map_or_else(
                || request.target.clone(),
                |name| format!("{name} · {}", request.target),
            )
        })
        .or_else(|| {
            camera
                .read()
                .as_ref()
                .map(|camera| format!("{} · {}", camera.name, camera.address))
        });
    let command = preview.as_ref().map(|request| shell_command(&request.argv));
    let watch = preview
        .as_ref()
        .is_some_and(|request| request.control.is_some());

    rsx! {
        style { {CONSOLE_CSS} }
        section { class: "console-workspace",
            div { class: "console-header",
                div {
                    h2 { {tr("console_title")} }
                    if let Some(target) = target {
                        div { class: "console-target", title: "{target}", "{target}" }
                    } else {
                        div { class: "console-target", {tr("console_missing_target")} }
                    }
                }
                div { class: "console-binary",
                    if let Some((_, grammar)) = loaded.as_ref() {
                        span { title: "{path_text}", "oxvif {grammar.cli_version}" }
                    } else if let Some(version) = load_error.as_ref().and_then(|error| error.legacy.version.as_deref()) {
                        span { title: "{path_text}", "{version}" }
                    }
                    button { class: "btn btn--small", disabled: busy,
                        onclick: move |_| async move {
                            if let Some(file) = rfd::AsyncFileDialog::new().pick_file().await {
                                state.executable.set(Some(file.path().to_owned()));
                            }
                        },
                        {tr("console_select_cli")}
                    }
                }
            }
            if let Some(error) = load_error.as_ref() {
                div { class: "console-notice", role: "status", {tr("console_incompatible")} " · {error.message}" }
                if !error.legacy.commands.is_empty() {
                    details { class: "console-help",
                        summary { {tr("console_help")} }
                        for command in &error.legacy.commands {
                            p { code { "{command.name}" } " {command.summary}" }
                            for example in &command.examples {
                                code { class: "console-example", "{example}" }
                            }
                        }
                    }
                }
            } else if path.is_none() {
                div { class: "console-notice", role: "status", {tr("console_cli_missing")} }
            }
            div { class: "console-entry",
                div { class: "console-editor",
                    span { class: "console-prompt", aria_hidden: "true", ">" }
                    div { class: "console-field",
                        div { class: "console-paint-clip", aria_hidden: "true",
                            pre { id: "oxdm-console-paint",
                                for (index, (class, piece)) in paint.into_iter().enumerate() {
                                    span { key: "{index}", class: "{class}", "{piece}" }
                                }
                            }
                        }
                        input {
                            id: "oxdm-console-input", class: "console-input", r#type: "text",
                            role: "combobox", aria_label: tr("console_title"), aria_autocomplete: "list",
                            aria_expanded: suggestions_open, aria_controls: "oxdm-console-candidates",
                            aria_activedescendant: suggestions_open.then(|| format!("oxdm-console-option-{index}")),
                            autocomplete: "off", spellcheck: "false", maxlength: 8192,
                            placeholder: "device info",
                            onmounted: editor,
                        }
                    }
                    if suggestions_open {
                        div { class: "console-candidates", id: "oxdm-console-candidates", role: "listbox",
                            "data-source": "{text}",
                            for (i, item) in suggestions.iter().enumerate() {
                                div {
                                    key: "{item.label}",
                                    id: "oxdm-console-option-{i}", role: "option", aria_selected: i == index,
                                    class: if i == index { "console-candidate selected" } else { "console-candidate" },
                                    "data-start": text[..item.range.start].encode_utf16().count(),
                                    "data-end": text[..item.range.end].encode_utf16().count(),
                                    "data-caret": text[..(*caret.read()).min(text.len())].encode_utf16().count(),
                                    "data-replacement": if item.range.end == text.len() { format!("{} ", item.replacement) } else { item.replacement.clone() },
                                    code { "{item.label}" }
                                    small { "{item.detail}" }
                                }
                            }
                        }
                    }
                }
                if busy {
                    button { class: "btn", disabled: current_snapshot.as_ref().is_some_and(|s| s.stopping),
                        onclick: move |_| state.controller.peek().stop(), {tr("console_stop")}
                    }
                } else {
                    button { class: "btn btn-primary", disabled: loaded.is_none() || text.trim().is_empty(),
                        onclick: move |_| {
                            let _ = document::eval(r#"
                                document.getElementById("oxdm-console-input")?.dispatchEvent(new Event("console-run"));
                            "#);
                        },
                        {tr("console_run")}
                    }
                }
            }
            if let Some(key) = *error.read() {
                div { class: "console-notice", role: "alert", {tr(key)} }
            }
            if let Some(command) = command {
                div { class: "console-preview",
                    code { title: "{command}", "{command}" }
                    button { class: "btn btn--small",
                        onclick: move |_| {
                            if let Err(message) = util::copy_to_clipboard(&command) {
                                ctx.push_toast(ToastLevel::Error, message);
                            }
                        },
                        {tr("console_copy")}
                    }
                    if watch && !busy {
                        button { class: "btn btn--small",
                            onclick: move |_| async move {
                                if let Some(file) = rfd::AsyncFileDialog::new().set_file_name("events.jsonl")
                                    .add_filter("JSONL", &["jsonl"]).save_file().await
                                {
                                    state.export.set(Some(file.path().to_owned()));
                                }
                            },
                            {tr("console_export")}
                        }
                    }
                }
            }
            if let Some(file) = export.as_ref() {
                div { class: "console-preview",
                    code { "{file.display()}" }
                    button { class: "btn btn--small", onclick: move |_| state.export.set(None), {tr("console_cancel_export")} }
                }
            }
            if let Some(details) = analysis.as_ref() {
                details { class: "console-help", open: details.selected_argument.is_some(),
                    summary { {tr("console_help")} }
                    if let Some(arg) = details.selected_argument {
                        p { code { "{arg.label()}" } " " {arg.help.as_deref().unwrap_or("")} }
                        if !arg.defaults.is_empty() {
                            p { {tr("console_default")} " " code { {arg.defaults.join(", ")} } }
                        }
                    } else if let Some(description) = details.node.description.as_ref() {
                        p { "{description.summary}" }
                        for example in &description.examples {
                            code { class: "console-example", "{example}" }
                        }
                    } else if let Some(about) = details.node.about.as_deref() {
                        p { "{about}" }
                    }
                    if let Some(template) = details.template.clone() {
                        button { class: "btn btn--small",
                            onclick: move |_| {
                                let end = template.len();
                                menu.set(false);
                                set_editor(template.clone(), end, state.draft.peek().as_str());
                            },
                            {tr("console_use_example")}
                        }
                    }
                    for arg in details.node.arguments.iter().filter(|arg| arg.required) {
                        p { {tr("console_required")} " " code { "{arg.label()}" } }
                    }
                }
            }
            if let Some(run) = current_snapshot.as_ref() {
                Output { key: "{run.run_id}", snapshot: run.clone(), locale, follow }
            }
        }
    }
}

fn input_spans<'a>(
    text: &'a str,
    tokens: &[(std::ops::Range<usize>, Syntax)],
) -> Vec<(&'static str, &'a str)> {
    let mut last = 0;
    let mut spans = Vec::new();
    for (range, syntax) in tokens {
        spans.push(("", &text[last..range.start]));
        let token = &text[range.clone()];
        if *syntax == Syntax::Flag {
            if let Some((flag, value)) = token.split_once('=') {
                spans.push((syntax.class(), flag));
                spans.push(("", "="));
                spans.push((
                    if value.starts_with(['"', '\'']) {
                        Syntax::Quoted.class()
                    } else {
                        Syntax::Value.class()
                    },
                    value,
                ));
            } else {
                spans.push((syntax.class(), token));
            }
        } else {
            spans.push((syntax.class(), token));
        }
        last = range.end;
    }
    spans.push(("", &text[last..]));
    spans
}

#[component]
fn Output(snapshot: Snapshot, locale: Locale, mut follow: Signal<bool>) -> Element {
    let tr = |key| i18n::t(locale, key);
    let finished = snapshot.finished.as_ref();
    let status = if let Some(finished) = finished {
        if !finished.started && finished.error.is_none() {
            tr("console_stopped")
        } else if finished.forced {
            tr("console_forced")
        } else if snapshot
            .summary
            .as_ref()
            .is_some_and(|summary| !summary.complete || summary.cleanup_failed)
        {
            tr("console_partial")
        } else if snapshot
            .summary
            .as_ref()
            .is_some_and(|summary| summary.reason == "interrupted")
        {
            tr("console_stopped")
        } else if finished.exit_code == Some(0) && finished.error.is_none() {
            tr("console_finished")
        } else {
            tr("console_failed")
        }
    } else if snapshot.stopping {
        tr("console_stopping")
    } else if snapshot.waiting_for_subscription {
        tr("console_waiting_for_subscription")
    } else {
        tr("console_running")
    };
    rsx! {
        div { class: "console-run",
            div { class: "console-output-toolbar",
                div { class: "console-output-status", role: "status", "{status} · "
                    if let Some(name) = snapshot.camera_name.as_deref() { "{name} · " }
                    "{snapshot.target}"
                    if let Some(code) = finished.and_then(|result| result.exit_code) { " · " {tr("console_exit")} " {code}" }
                }
                button { class: "btn btn--small",
                    onclick: move |_| follow.toggle(),
                    if *follow.read() { {tr("console_pause")} } else { {tr("console_follow")} }
                }
            }
            code { class: "console-last-command", "{snapshot.command}" }
            if snapshot.watch {
                div { class: "console-counters",
                    span { {tr("console_events")} " {snapshot.events}" }
                    span { {tr("console_gaps")} " {snapshot.gaps}" }
                    if let Some(gap) = snapshot.last_gap.as_deref() { span { "{gap} · " {tr("console_gap_unknown")} } }
                    if let Some(summary) = snapshot.summary.as_ref() {
                        span { {tr("console_end_reason")} " {summary.reason}" }
                        span { {tr("console_cli_events")} " {summary.counts.delivered}" }
                        span { {tr("console_cli_gaps")} " {summary.counts.gaps}" }
                        span { {tr("console_undelivered")} " {summary.known_undelivered}" }
                        span { {tr("console_recoveries")} " {summary.recoveries}" }
                        if !summary.complete { span { {tr("console_stream_incomplete")} } }
                    }
                    if finished.is_some_and(|result| result.started) {
                        span {
                            if snapshot.summary.as_ref().is_some_and(|summary| !summary.cleanup_failed)
                                && !finished.is_some_and(|result| result.forced)
                            { {tr("console_cleanup_confirmed")} } else { {tr("console_cleanup_unconfirmed")} }
                        }
                    }
                }
            }
            if snapshot.evicted > 0 || snapshot.malformed > 0 {
                div { class: "console-counters",
                    if snapshot.evicted > 0 { span { {tr("console_evicted")} " {snapshot.evicted}" } }
                    if snapshot.malformed > 0 { span { {tr("console_malformed")} " {snapshot.malformed}" } }
                }
            }
            if let Some(message) = finished.and_then(|result| result.error.as_deref()) {
                div { class: "console-notice", "{message}" }
            }
            if let Some(file) = snapshot.export.as_ref() {
                div { class: "console-counters",
                    if let Some(message) = snapshot.export_error.as_deref() {
                        span { {tr("console_export_incomplete")} " · {message}" }
                    } else if finished.is_some() {
                        span { {tr("console_exported")} }
                    } else {
                        span { {tr("console_exporting")} }
                    }
                    code { "{file.display()}" }
                }
            }
            div { class: "console-output", id: "oxdm-console-output", tabindex: 0,
                for line in &snapshot.lines {
                    OutputRow { key: "{line.index}", line: line.clone(), locale }
                }
            }
        }
    }
}

#[component]
fn OutputRow(line: OutputLine, locale: Locale) -> Element {
    let mut expanded = use_signal(|| false);
    rsx! {
        if let Some(event) = line.event.as_ref() {
            button {
                class: "console-event-row",
                aria_expanded: *expanded.read(),
                onclick: move |_| expanded.toggle(),
                span { class: "console-syn-value", "#{event.sequence}" }
                time { "{event.time}" }
                span { class: "console-syn-quoted", "{event.topic}" }
            }
        }
        if line.event.is_none() || *expanded.read() {
            pre {
                if line.kind == OutputKind::Error {
                    span { class: "console-syn-invalid", {i18n::t(locale, "console_error")} "  " }
                } else if line.kind == OutputKind::Gap {
                    span { class: "console-syn-invalid", {i18n::t(locale, "console_gap")} "  " }
                }
                if matches!(line.kind, OutputKind::Json | OutputKind::Event | OutputKind::Gap | OutputKind::Summary) {
                    for (i, (class, value)) in json_spans(&line.text).into_iter().enumerate() {
                        span { key: "{i}", class: "{class}", "{value}" }
                    }
                } else {
                    "{line.text}"
                }
            }
        }
    }
}

fn json_spans(text: &str) -> Vec<(&'static str, &str)> {
    let mut chars = text.char_indices().peekable();
    let mut spans = Vec::new();
    while let Some((start, first)) = chars.next() {
        if start >= 8192 {
            spans.push(("", &text[start..]));
            break;
        }
        let class = match first {
            '"' => {
                while let Some((_, ch)) = chars.next() {
                    match ch {
                        '"' => break,
                        '\\' => {
                            chars.next();
                        }
                        _ => {}
                    }
                }
                if chars.peek().is_some_and(|(_, ch)| *ch == ':') {
                    "console-syn-flag"
                } else {
                    "console-syn-quoted"
                }
            }
            ch if ch.is_ascii_alphanumeric() || ch == '-' => {
                while chars
                    .peek()
                    .is_some_and(|(_, ch)| ch.is_ascii_alphanumeric() || "-+.".contains(*ch))
                {
                    chars.next();
                }
                "console-syn-value"
            }
            _ => "",
        };
        let end = chars.peek().map_or(text.len(), |(i, _)| *i);
        spans.push((class, &text[start..end]));
    }
    spans
}
