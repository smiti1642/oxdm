use std::{path::PathBuf, rc::Rc, time::Duration};

use dioxus::prelude::*;
use serde::Deserialize;
use serde_json::json;

use crate::{
    api,
    components::{DialogOverlay, Icon},
    i18n,
    state::{ConfirmDialog, Ctx, Locale, ToastLevel},
    util,
};

use super::{
    grammar::{
        shell_command, ArgumentChip, ArgumentKind, Camera, CredentialInput, DynamicValues, Grammar,
        Invocation, ParameterOption, Syntax, ValueKind, WorkspaceChange,
    },
    process::{OutputKind, OutputLine, Snapshot},
    ConsoleState,
};

const EDITOR_BRIDGE: &str = include_str!("../../assets/console-editor.js");

fn focus_parameter_control(id: &str) {
    let eval = document::eval(
        "const id = await dioxus.recv(); requestAnimationFrame(() => document.getElementById(id)?.focus());",
    );
    let _ = eval.send(id);
}

#[component]
fn ConsoleCredentialDialog(
    command: String,
    locale: Locale,
    saves_credentials: bool,
    on_cancel: EventHandler<()>,
    on_submit: EventHandler<CredentialInput>,
) -> Element {
    let mut username = use_signal(|| zeroize::Zeroizing::new(String::new()));
    let mut password = use_signal(|| zeroize::Zeroizing::new(String::new()));
    let tr = |key| i18n::t(locale, key);
    let (title, hint) = if saves_credentials {
        ("console_credential_title", "console_credential_hint")
    } else {
        (
            "console_query_credential_title",
            "console_query_credential_hint",
        )
    };
    let valid = !username.read().trim().is_empty()
        && !username.read().chars().any(char::is_control)
        && !password.read().is_empty()
        && !password.read().chars().any(char::is_control);

    rsx! {
        DialogOverlay { inner_class: "dialog".to_owned(), on_close: move |_| on_cancel.call(()),
            form {
                role: "dialog", aria_modal: true, aria_labelledby: "oxdm-cli-credential-title",
                autocomplete: "off",
                onsubmit: move |event| {
                    event.prevent_default();
                    if valid {
                        on_submit.call(CredentialInput { username: username.take(), password: password.take() });
                    }
                },
                h3 { id: "oxdm-cli-credential-title", {tr(title)} }
                p { {tr(hint)} }
                pre { code { "{command}" } }
                label { r#for: "oxdm-cli-credential-user", {tr("cred_username")} }
                input {
                    id: "oxdm-cli-credential-user", class: "form-input", autofocus: true,
                    maxlength: 1024, required: true,
                    oninput: move |event| username.set(zeroize::Zeroizing::new(event.value())),
                }
                label { r#for: "oxdm-cli-credential-secret", {tr("cred_password")} }
                input {
                    id: "oxdm-cli-credential-secret", class: "form-input", r#type: "password",
                    maxlength: 4096, required: true,
                    oninput: move |event| password.set(zeroize::Zeroizing::new(event.value())),
                }
                div { class: "dialog-actions",
                    button { class: "btn", r#type: "button", onclick: move |_| on_cancel.call(()), {tr("btn_cancel")} }
                    button { class: "btn btn-primary", r#type: "submit", disabled: !valid, {tr("btn_confirm")} }
                }
            }
        }
    }
}

#[component]
fn ParameterValue(
    id: String,
    value: Signal<String>,
    choices: Vec<String>,
    suggestions: Vec<String>,
    kind: ValueKind,
    locale: Locale,
    count: bool,
) -> Element {
    let list = format!("{id}-values");
    let path = matches!(kind, ValueKind::InputFile | ValueKind::OutputFile);
    rsx! {
        if choices.is_empty() {
            input {
                id: "{id}", class: "form-input",
                onmounted: {
                    let id = id.clone();
                    move |_| focus_parameter_control(&id)
                },
                r#type: if count || kind == ValueKind::Concurrency { "number" } else { "text" },
                min: (count || kind == ValueKind::Concurrency).then_some(1),
                max: if count { Some(255) } else { (kind == ValueKind::Concurrency).then_some(64) },
                list: (!suggestions.is_empty()).then_some(list.clone()),
                placeholder: matches!(kind, ValueKind::Duration | ValueKind::WatchDuration).then_some("30s"),
                value: "{value}", oninput: move |event| value.set(event.value()),
            }
            if !suggestions.is_empty() {
                datalist { id: "{list}",
                    for suggestion in &suggestions {
                        option { key: "{suggestion}", value: "{suggestion}" }
                    }
                }
            }
            if path {
                button {
                    class: "btn btn--small", r#type: "button",
                    onclick: move |_| async move {
                        let dialog = rfd::AsyncFileDialog::new();
                        let chosen = if kind == ValueKind::InputFile { dialog.pick_file().await } else { dialog.save_file().await };
                        if let Some(path) = chosen.and_then(|file| file.path().to_str().map(str::to_owned)) {
                            value.set(path);
                        }
                    },
                    {i18n::t(locale, "console_choose_file")}
                }
            }
        } else {
            select {
                id: "{id}", class: "form-input",
                onmounted: move |_| focus_parameter_control(&id),
                value: "{value}", onchange: move |event| value.set(event.value()),
                option { value: "", disabled: true, {i18n::t(locale, "console_help_option_value")} }
                for option in choices {
                    option { key: "{option}", value: "{option}", "{option}" }
                }
            }
        }
    }
}

#[component]
fn ParameterChip(chip: ArgumentChip, draft: String, locale: Locale) -> Element {
    let mut editing = use_signal(|| false);
    let mut value = use_signal(String::new);
    let editable = matches!(chip.kind, ArgumentKind::Value | ArgumentKind::Positional)
        || (chip.kind == ArgumentKind::Count && chip.id == "verbose");
    let edit = i18n::t(locale, "console_edit_argument");
    let label = chip.label.trim_start_matches('-');
    let summary = format!("{label}: {}", chip.value.as_deref().unwrap_or("…"));
    let can_apply = chip.edited(&draft, &value.read()).is_some();
    let edit_chip = chip.clone();
    let submit_draft = draft.clone();
    let edit_value = chip.value.clone().unwrap_or_default();
    let range = chip.range.clone();
    let form_id = format!("oxdm-argument-{}", range.start);
    let button_id = format!("{form_id}-edit");
    let cancel_edit = {
        let button_id = button_id.clone();
        move || {
            editing.set(false);
            focus_parameter_control(&button_id);
        }
    };
    let mut escape_edit = cancel_edit.clone();
    let mut cancel_edit = cancel_edit;
    let removal = chip.removed(&draft);

    rsx! {
        div { class: "console-chip",
            if *editing.read() {
                form {
                    class: "console-chip-editor",
                    onsubmit: move |event| {
                        event.prevent_default();
                        if let Some((text, caret)) = edit_chip.edited(&submit_draft, &value.peek()) {
                            editing.set(false);
                            set_editor(text, caret, &submit_draft);
                        }
                    },
                    onkeydown: move |event| {
                        if event.key() == Key::Escape && !event.is_composing() {
                            event.stop_propagation();
                            escape_edit();
                        }
                    },
                    label { r#for: "{form_id}", code { "{label}" } }
                    ParameterValue { id: form_id.clone(), value, choices: chip.choices, suggestions: chip.suggestions, kind: chip.value_kind, locale, count: chip.kind == ArgumentKind::Count }
                    button { class: "btn btn--small", r#type: "submit", disabled: !can_apply, {i18n::t(locale, "btn_apply")} }
                    button { class: "btn btn--small", r#type: "button", onclick: move |_| cancel_edit(), {i18n::t(locale, "btn_cancel")} }
                }
            } else if editable {
                button {
                    id: "{button_id}",
                    class: "btn btn--small", r#type: "button", title: edit,
                    onclick: move |_| { value.set(edit_value.clone()); editing.set(true); },
                    "{summary}"
                }
            } else {
                code { class: "console-chip-label", "{summary}" }
            }
            if !*editing.read() {
                button {
                    class: "btn btn--small", r#type: "button",
                    aria_label: format!("{} {label}", i18n::t(locale, "console_remove_argument")),
                    onclick: move |_| {
                        set_editor(removal.0.clone(), removal.1, &draft);
                    },
                    "×"
                }
            }
        }
    }
}

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

pub(super) fn set_editor(text: String, caret: usize, expected: &str) {
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

pub(super) fn current_grammar(state: ConsoleState) -> Option<(PathBuf, Rc<Grammar>)> {
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
    mut credential_request: Signal<Option<(String, Invocation)>>,
) {
    let Some((path, grammar)) = current_grammar(state) else {
        return;
    };
    let text = state.draft.peek().clone();
    if let Some(terminal) = grammar.external_terminal_args(&text) {
        error.set(Some(terminal.err().unwrap_or("console_external_help")));
        return;
    }
    let request = match grammar.prepare(path, &text, camera) {
        Ok(request) => request,
        Err("console_missing_target") => {
            state.pick_target();
            error.set(Some("console_missing_target"));
            return;
        }
        Err(reason) => {
            error.set(Some(reason));
            return;
        }
    };
    if request.credential_input {
        credential_request.set(Some((text, request)));
        return;
    }
    if let Some(change) = request.workspace_change {
        let ctx = consume_context::<Ctx>();
        let locale = *ctx.locale.peek();
        let tr = |key| i18n::t(locale, key).to_owned();
        let action = tr(if change == WorkspaceChange::Remove {
            "console_workspace_remove"
        } else {
            "console_workspace_write"
        });
        let command = shell_command(&request.argv);
        let mut pending = Signal::new(Some((text, request)));
        ctx.dialog.clone().set(Some(ConfirmDialog {
            title: tr("console_workspace_confirm"),
            message: format!("{action}\n\n{command}"),
            confirm_label: tr("btn_confirm"),
            cancel_label: tr("btn_cancel"),
            dangerous: change == WorkspaceChange::Remove,
            on_confirm: EventHandler::new(move |_| {
                if let Some((text, request)) = pending.take() {
                    start_invocation(state, error, text, request);
                }
            }),
        }));
        return;
    }
    start_invocation(state, error, text, request);
}

fn start_invocation(
    mut state: ConsoleState,
    mut error: Signal<Option<&'static str>>,
    text: String,
    request: Invocation,
) {
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
    let mut credential_request = use_signal(|| None::<(String, Invocation)>);
    let mut snapshot = use_signal(|| None::<Snapshot>);
    let follow = state.follow_output;
    let terminal = *state.terminal.read();
    let mut arguments_open = use_signal(|| false);
    let mut argument_filter = use_signal(String::new);
    let mut adding_parameter = use_signal(|| None::<(ParameterOption, String)>);
    let mut parameter_value = use_signal(String::new);
    let picking_target = *state.picking_target.read();

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
                    || adding_parameter
                        .read()
                        .as_ref()
                        .is_some_and(|(option, draft)| {
                            option.value_source.as_deref() == Some("event_topic")
                                && grammar.needs_camera_topics(
                                    &format!("{draft} --topic"),
                                    &camera.address,
                                )
                        })
            })
        })
    });
    let needs_profiles = use_memo(move || {
        current_grammar(state).is_some_and(|(_, grammar)| {
            camera.read().as_ref().is_some_and(|camera| {
                grammar.needs_media_profiles(&state.draft.read(), &camera.address)
            })
        })
    });
    let profiles = use_resource(move || {
        let source = (*needs_profiles.read())
            .then(|| camera.read().clone())
            .flatten();
        async move {
            let camera = source?;
            let profiles = api::get_profiles(&camera.address, &camera.credentials)
                .await
                .ok()?;
            let mut tokens = profiles
                .into_iter()
                .map(|profile| profile.token)
                .collect::<Vec<_>>();
            tokens.sort_unstable();
            tokens.dedup();
            tokens.truncate(1024);
            Some((camera, tokens))
        }
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
        media_profiles: profiles
            .peek()
            .as_ref()
            .and_then(|result| result.as_ref())
            .filter(|(source, _)| *needs_profiles.peek() && camera.peek().as_ref() == Some(source))
            .map_or_else(Vec::new, |(_, values)| values.clone()),
    };

    let editor = move |_| {
        let mut bridge = document::eval(EDITOR_BRIDGE);
        let _ = bridge.send(state.draft.peek().as_str());
        spawn(async move {
            while let Ok(event) = bridge.recv::<EditorEvent>().await {
                let position = byte_offset(&event.text, event.caret);
                let text_changed = *state.draft.peek() != event.text;
                let changed = text_changed || *caret.peek() != position;
                if text_changed {
                    state.set_draft(event.text);
                }
                if changed {
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
                                    .analyze(
                                        &state.draft.peek(),
                                        *caret.peek(),
                                        &dynamic(),
                                        *ctx.locale.peek(),
                                    )
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
                            "Enter" | "run" => {
                                execute(state, camera.peek().as_ref(), error, credential_request)
                            }
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
    let _profiles_revision = profiles.read();
    let analysis = loaded.as_ref().map(|(_, grammar)| {
        grammar.analyze(&text, (*caret.read()).min(text.len()), &dynamic(), locale)
    });
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
    let selected_camera = camera.read();
    let explicit_target = analysis.as_ref().and_then(|a| a.explicit_target.as_deref());
    let camera_label = selected_camera
        .as_ref()
        .map(|camera| format!("{} · {}", camera.name, camera.address));
    let target_label = explicit_target
        .map(|address| format!("--target · {address}"))
        .or(camera_label);
    let pick_label = tr(if selected_camera.is_some() {
        "console_change_camera"
    } else {
        "console_pick_camera"
    });
    let button_text = target_label.as_deref().unwrap_or(pick_label);
    let command = preview.as_ref().map(|request| shell_command(&request.argv));
    let external = loaded.as_ref().and_then(|(path, grammar)| {
        grammar
            .external_terminal_args(&text)
            .map(|argv| (path.clone(), argv))
    });
    let watch = preview
        .as_ref()
        .is_some_and(|request| request.control.is_some());

    let (completed, evicted_runs) = state.controller.peek().completed();
    let latest = current_snapshot.as_ref().map(|snapshot| snapshot.run_id);
    let oldest_visible = completed
        .first()
        .map(|run| run.run_id)
        .or(latest)
        .unwrap_or(0);
    let (chips, arguments) = loaded.as_ref().filter(|_| !terminal).map_or_else(
        || (Vec::new(), Vec::new()),
        |(_, grammar)| {
            (
                grammar.argument_chips(&text, &dynamic()),
                grammar.available_arguments(&text, &argument_filter.read(), locale),
            )
        },
    );

    rsx! {
        if let Some((_, request)) = credential_request.read().as_ref() {
            ConsoleCredentialDialog {
                command: shell_command(&request.argv), locale,
                saves_credentials: request.workspace_change.is_some(),
                on_cancel: move |_| {
                    credential_request.set(None);
                    let _ = document::eval("requestAnimationFrame(() => document.getElementById('oxdm-console-input')?.focus());");
                },
                on_submit: move |credential| {
                    if let Some((text, mut request)) = credential_request.take() {
                        request.credential = Some(credential);
                        start_invocation(state, error, text, request);
                    }
                    let _ = document::eval("requestAnimationFrame(() => document.getElementById('oxdm-console-input')?.focus());");
                },
            }
        }
        section { class: if terminal { "console-workspace console-workspace--terminal" } else { "console-workspace" },
            div { class: "console-header",
                div { class: "console-navigation",
                    button {
                        class: "btn btn-sm btn-ghost",
                        r#type: "button",
                        onclick: move |_| state.leave(ctx),
                        Icon { name: "arrow-left", size: 14 }
                        {tr("console_leave")}
                    }
                    h2 { {tr("console_title")} }
                    button {
                        id: "oxdm-console-presentation",
                        class: "btn btn-sm btn-ghost",
                        r#type: "button",
                        onclick: move |_| {
                            arguments_open.set(false);
                            adding_parameter.set(None);
                            state.terminal.set(!terminal);
                        },
                        if !terminal { Icon { name: "terminal", size: 14 } }
                        {tr(if terminal { "console_card_view" } else { "console_terminal" })}
                    }
                }
                div { class: "console-connection",
                    button {
                        id: "oxdm-console-target", r#type: "button",
                        class: if target_label.is_some() { "btn btn-sm console-target" } else { "btn btn-sm console-target console-target--empty" },
                        title: "{pick_label} · {button_text}",
                        aria_label: if target_label.is_some() { format!("{pick_label} · {button_text}") } else { pick_label.to_owned() },
                        aria_expanded: picking_target, aria_controls: "oxdm-console-sidebar-content",
                        onclick: move |_| {
                            if *state.picking_target.peek() {
                                state.close_target_picker("oxdm-console-target");
                            } else {
                                state.pick_target();
                            }
                        },
                        Icon { name: "camera", size: 15 }
                        span { "{button_text}" }
                        Icon { name: "chevron-down", size: 14 }
                    }
                    div { class: "console-binary",
                        if let Some((_, grammar)) = loaded.as_ref() {
                            span { title: "{path_text}", "oxvif {grammar.cli_version}" }
                        } else if let Some(version) = load_error.as_ref().and_then(|error| error.legacy.version.as_deref()) {
                            span { title: "{path_text}", "{version}" }
                        }
                        button { class: "btn btn-sm btn-ghost", r#type: "button", disabled: busy,
                            onclick: move |_| async move {
                                if let Some(file) = rfd::AsyncFileDialog::new().pick_file().await {
                                    state.executable.set(Some(file.path().to_owned()));
                                }
                            },
                            {tr("console_select_cli")}
                        }
                    }
                }
            }
            div { class: "console-body",
                if let Some(error) = load_error.as_ref() {
                    div { class: "console-notice", role: "status", {tr("console_incompatible")} " · {error.message}" }
                    if !error.legacy.commands.is_empty() {
                        details { class: "console-help",
                            summary { {tr("console_help")} }
                            for command in &error.legacy.commands {
                                p {
                                    code { "{command.name}" } " "
                                    {i18n::console_command(locale, &command.name).unwrap_or(&command.summary)}
                                }
                                for example in &command.examples {
                                    code { class: "console-example", "{example}" }
                                }
                            }
                        }
                    }
                } else if path.is_none() {
                    div { class: "console-notice", role: "status", {tr("console_cli_missing")} }
                }
                if let Some(key) = (*error.read()).filter(|&key| key != "console_missing_target" || preview.is_none()) {
                    div { class: "console-notice", role: "alert", {tr(key)} }
                }
                if let Some((path, argv)) = external {
                    div { class: "console-preview",
                        if let Ok(argv) = argv {
                            code { {shell_command(&argv)} }
                            if cfg!(windows) {
                                button {
                                    class: "btn btn--small",
                                    onclick: move |_| {
                                        #[cfg(windows)]
                                        if super::process::open_terminal(&path, &argv).is_err() {
                                            error.set(Some("console_external_failed"));
                                        }
                                        #[cfg(not(windows))]
                                        let _ = (&path, &argv);
                                    },
                                    {tr("console_external_open")}
                                }
                            } else {
                                button {
                                    class: "btn btn--small",
                                    onclick: move |_| {
                                        let _ = util::copy_to_clipboard(&shell_command(&argv));
                                    },
                                    {tr("console_copy")}
                                }
                            }
                        } else if let Err(key) = argv {
                            span { {tr(key)} }
                        }
                    }
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
                if evicted_runs > 0 {
                    p { class: "console-counters", {tr("console_history_evicted")} " {evicted_runs}" }
                }
                for run in completed.iter().filter(|run| Some(run.run_id) != latest).map(|run| run.as_ref()).chain(current_snapshot.iter()) {
                    Output {
                        key: "{run.run_id}", snapshot: run.clone(), locale, follow, terminal,
                        latest: Some(run.run_id) == latest, oldest_visible,
                    }
                }
            }
            if !terminal {
                if loaded.is_some() {
                    div { class: "console-arguments",
                        onkeydown: move |event: KeyboardEvent| {
                            if event.key() == Key::Escape && !event.is_composing() && *arguments_open.peek() {
                                event.stop_propagation();
                                if adding_parameter.peek().is_some() {
                                    adding_parameter.set(None);
                                    focus_parameter_control("oxdm-console-argument-filter");
                                } else {
                                    arguments_open.set(false);
                                    focus_parameter_control("oxdm-console-add-argument");
                                }
                            }
                        },
                        for chip in chips {
                            ParameterChip { key: "{chip.range.start}-{chip.label}", chip, draft: text.clone(), locale }
                        }
                        if *arguments_open.read() || !arguments.is_empty() {
                            button {
                                id: "oxdm-console-add-argument",
                                class: "btn btn--small",
                                aria_expanded: *arguments_open.read(),
                                onclick: move |_| { argument_filter.set(String::new()); adding_parameter.set(None); arguments_open.toggle(); },
                                {tr("console_add_argument")}
                            }
                        }
                        if *arguments_open.read() {
                            if adding_parameter.read().is_none() {
                                div { class: "console-argument-picker",
                                    input {
                                        id: "oxdm-console-argument-filter",
                                        class: "form-input",
                                        onmounted: move |_| focus_parameter_control("oxdm-console-argument-filter"),
                                        aria_label: tr("console_find_argument"),
                                        placeholder: tr("console_find_argument"),
                                        value: "{argument_filter}",
                                        oninput: move |event| argument_filter.set(event.value()),
                                    }
                                    for option in arguments {
                                        button {
                                            class: "btn btn--small",
                                            r#type: "button",
                                            key: "{option.label}",
                                            onclick: {
                                                let option = option.clone();
                                                let expected = text.clone();
                                                move |_| {
                                                    if let Some(token) = option.insertion("") {
                                                        let next = format!("{} {token}", expected.trim_end());
                                                        let end = next.len();
                                                        arguments_open.set(false);
                                                        set_editor(next, end, &expected);
                                                    } else {
                                                        parameter_value.set(String::new());
                                                        adding_parameter.set(Some((option.clone(), expected.clone())));
                                                    }
                                                }
                                            },
                                            code { "{option.label.trim_start_matches('-')}" }
                                            small { "{option.detail}" }
                                        }
                                    }
                                }
                            }
                            if let Some((option, expected)) = adding_parameter.read().as_ref() {
                                form { class: "console-chip-editor",
                                    onsubmit: {
                                        let expected = expected.clone();
                                        let option = option.clone();
                                        move |event| {
                                            event.prevent_default();
                                            if let Some(token) = option.insertion(&parameter_value.peek()) {
                                                let next = format!("{} {token}", expected.trim_end());
                                                let end = next.len();
                                                adding_parameter.set(None);
                                                arguments_open.set(false);
                                                set_editor(next, end, &expected);
                                            }
                                        }
                                    },
                                    label { r#for: "oxdm-parameter-value", "{option.label}" }
                                    ParameterValue {
                                        id: "oxdm-parameter-value".to_owned(), value: parameter_value,
                                        choices: option.choices.clone(), suggestions: dynamic().suggestions(option.value_source.as_deref()).to_vec(),
                                        kind: option.value_kind, locale, count: false,
                                    }
                                    button { class: "btn btn--small", r#type: "submit", disabled: option.insertion(&parameter_value.read()).is_none(), {tr("btn_apply")} }
                                    button { class: "btn btn--small", r#type: "button", onclick: move |_| adding_parameter.set(None), {tr("btn_cancel")} }
                                }
                            }
                        }
                    }
                }
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
                                    "data-replacement": if item.range.end == text.len() { format!("{} ", item.replacement.trim_end()) } else { item.replacement.clone() },
                                    code { "{item.label}" }
                                    small { "{item.detail}" }
                                }
                            }
                        }
                    }
                }
                if busy {
                    button { class: "btn", disabled: current_snapshot.as_ref().is_some_and(|s| s.stopping),
                        onclick: move |_| {
                            state.controller.peek().stop();
                            let _ = document::eval("document.getElementById('oxdm-console-input')?.focus();");
                        },
                        {tr("console_stop")}
                    }
                } else {
                    button { class: "btn btn-primary console-run-button",
                        disabled: loaded.is_none() || text.trim().is_empty(),
                        title: tr("console_run"), aria_label: tr("console_run"),
                        onclick: move |_| {
                            let _ = document::eval(r#"
                                const input = document.getElementById("oxdm-console-input");
                                input?.focus();
                                input?.dispatchEvent(new Event("console-run"));
                            "#);
                        },
                        Icon { name: "arrow-right", size: 19 }
                    }
                }
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
fn Output(
    snapshot: Snapshot,
    locale: Locale,
    mut follow: Signal<bool>,
    terminal: bool,
    latest: bool,
    oldest_visible: u64,
) -> Element {
    let ctx = use_context::<Ctx>();
    let console = use_context::<ConsoleState>();
    let mut presentation = console.presentation;
    let run_id = snapshot.run_id;
    let expanded = presentation.read().is_expanded(run_id, latest, terminal);
    let tr = |key| i18n::t(locale, key);
    let share_report = snapshot.share_report.clone();
    let finished = snapshot.finished.as_ref();
    let completion_script = snapshot.completion_script.clone().filter(|_| {
        finished.is_some_and(|result| {
            result.exit_code == Some(0) && result.error.is_none() && !result.forced
        })
    });
    let import_draft = snapshot
        .import_fingerprint
        .as_deref()
        .filter(|_| {
            finished.is_some_and(|result| {
                result.exit_code == Some(0) && result.error.is_none() && !result.forced
            })
        })
        .and_then(|fingerprint| {
            current_grammar(console)?
                .1
                .apply_import_draft(snapshot.import_preview_draft.as_deref()?, fingerprint)
        });
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
        div { class: if expanded { "console-run" } else { "console-run console-run--collapsed" },
            div { class: "console-output-toolbar",
                div { class: "console-output-status", role: "status", "{status}"
                    if !snapshot.target.is_empty() {
                        " · "
                        if let Some(name) = snapshot.camera_name.as_deref() { "{name} · " }
                        "{snapshot.target}"
                    }
                    if let Some(code) = finished.and_then(|result| result.exit_code) { " · " {tr("console_exit")} " {code}" }
                    " · {snapshot.elapsed_ms} ms"
                }
                if !terminal {
                    button {
                        class: "btn btn--small",
                        aria_expanded: expanded,
                        onclick: move |_| presentation.write().set_expanded(run_id, !expanded, oldest_visible),
                        if expanded { {tr("console_collapse_run")} } else { {tr("console_expand_run")} }
                    }
                }
                if latest && expanded {
                    button { class: "btn btn--small",
                        onclick: move |_| follow.toggle(),
                        if *follow.read() { {tr("console_pause")} } else { {tr("console_follow")} }
                    }
                }
                if snapshot.watch && finished.is_none() {
                    button {
                        class: "btn btn--small",
                        disabled: snapshot.stopping,
                        onclick: move |_| console.controller.peek().stop(),
                        {tr("console_stop")}
                    }
                }
            }
            code { class: "console-last-command",
                if terminal { "> " }
                "{snapshot.command}"
            }
            if expanded {
            if let Some(script) = completion_script {
                button {
                    class: "btn btn--small",
                    onclick: move |_| {
                        if let Err(message) = util::copy_to_clipboard(&script) {
                            ctx.push_toast(ToastLevel::Error, message);
                        }
                    },
                    {tr("console_copy_script")}
                }
            }
            if let Some(command) = import_draft {
                button {
                    class: "btn btn--small",
                    onclick: move |_| set_editor(command.clone(), command.len(), console.draft.peek().as_str()),
                    {tr("console_import_use_plan")}
                }
            }
            if let Some(report) = share_report {
                div { class: "console-preview",
                    if let Some((stage, reason, next)) = report.issue(locale) {
                        div {
                            strong { "{stage}" }
                            p { "{reason}" }
                            p { "{next}" }
                        }
                    }
                    button {
                        class: "btn btn--small",
                        disabled: finished.is_none() || finished.is_some_and(|result| result.forced || result.error.is_some()),
                        onclick: move |_| {
                            let report = report.clone();
                            async move {
                                if let Some(destination) = rfd::AsyncFileDialog::new()
                                    .set_file_name("oxvif-diagnostic.json")
                                    .add_filter("JSON", &["json"])
                                    .save_file()
                                    .await
                                {
                                    let path = destination.path().to_owned();
                                    let saved = tokio::task::spawn_blocking(move || report.save(path))
                                        .await.is_ok_and(|result| result.is_ok());
                                    if !saved {
                                        ctx.push_toast(ToastLevel::Error, i18n::t(locale, "console_share_save_failed"));
                                    }
                                }
                            }
                        },
                        {tr("console_share_save")}
                    }
                }
                if report.skipped(locale).next().is_some() {
                    details { class: "console-help",
                        summary { {tr("console_share_skipped")} }
                        ul {
                            for (index, (stage, status)) in report.skipped(locale).enumerate() {
                                li { key: "{index}", "{stage} · {status}" }
                            }
                        }
                    }
                }
            } else if snapshot.share_requested && finished.is_some() {
                div { class: "console-notice", role: "status", {tr("console_share_invalid")} }
            }
            }
            if snapshot.watch {
                div { class: "console-counters",
                    span { {tr("console_events")} " {snapshot.events}" }
                    span { {tr("console_gaps")} " {snapshot.gaps}" }
                    if let Some(gap) = snapshot.last_gap.as_deref() { span { {event_reason(locale, gap)} " · " {tr("console_gap_unknown")} } }
                    if let Some(summary) = snapshot.summary.as_ref() {
                        span { {tr("console_end_reason")} " " {event_reason(locale, &summary.reason)} }
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
            if expanded {
                div { class: "console-output", id: latest.then_some("oxdm-console-output"), tabindex: 0,
                    for line in &snapshot.lines {
                        OutputRow { key: "{line.index}", line: line.clone(), locale }
                    }
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

fn event_reason(locale: Locale, reason: &str) -> &str {
    let key = match reason {
        "duration_elapsed" => "console_event_duration_elapsed",
        "interrupted" => "console_event_interrupted",
        "queue_full" => "console_event_queue_full",
        "invalid_timing" => "console_event_invalid_timing",
        "request_failed" => "console_event_request_failed",
        "recovery_exhausted" => "console_event_recovery_exhausted",
        "worker_stopped" => "console_event_worker_stopped",
        "output_timeout" => "console_event_output_timeout",
        "output_failed" => "console_event_output_failed",
        "drain_timeout" => "console_event_drain_timeout",
        "subscription_recovery" => "console_event_subscription_recovery",
        "subscription_stopped" => "console_event_subscription_stopped",
        "undelivered_at_stop" => "console_event_undelivered_at_stop",
        _ => return reason,
    };
    i18n::t(locale, key)
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
