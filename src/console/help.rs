use dioxus::prelude::*;

use crate::{
    components::{DeviceList, Icon},
    i18n,
    state::{Ctx, View},
};

use super::{
    view::{current_grammar, set_editor},
    ConsoleState,
};

const SIDEBAR_RESIZER: &str = include_str!("../../assets/console-sidebar.js");

#[component]
pub fn ConsoleSidebar() -> Element {
    let ctx = use_context::<Ctx>();
    let mut state = use_context::<ConsoleState>();
    let in_console = *ctx.view.read() == View::Console;
    let help = use_memo(move || state.help_visible());
    let picking_target = in_console && *state.picking_target.read();
    let show_help = in_console && !picking_target && *help.read();
    let collapsed = in_console && !picking_target && *state.sidebar_collapsed.read();
    let locale = *ctx.locale.read();
    let tr = |key| i18n::t(locale, key);
    let toggle_label = tr(if collapsed {
        "console_sidebar_expand"
    } else {
        "console_sidebar_collapse"
    });
    let toggle_icon = if collapsed {
        "arrow-right"
    } else {
        "arrow-left"
    };

    rsx! {
        div {
            class: if in_console { "console-sidebar-frame" } else { "console-sidebar-shell" },
            "data-collapsed": "{collapsed}",
            div {
                class: if in_console { "sidebar console-sidebar" } else { "console-sidebar-shell" },
                if picking_target {
                    div {
                        class: "console-picker-header",
                        h3 { id: "oxdm-console-target-picker", {tr("console_pick_camera")} }
                        button {
                            class: "btn btn-sm",
                            onclick: move |_| state.close_target_picker("oxdm-console-target"),
                            {tr("btn_cancel")}
                        }
                    }
                    p { id: "oxdm-console-picker-hint", class: "console-picker-hint", {tr("console_picker_hint")} }
                } else if in_console {
                    div { class: "console-sidebar-toolbar",
                        if !collapsed {
                            div { class: "sidebar-tabs",
                                button {
                                    class: if show_help { "sidebar-tab" } else { "sidebar-tab sidebar-tab--active" },
                                    aria_pressed: !show_help,
                                    onclick: move |_| state.show_devices(),
                                    {tr("sidebar_title")}
                                }
                                button {
                                    class: if show_help { "sidebar-tab sidebar-tab--active" } else { "sidebar-tab" },
                                    aria_pressed: show_help,
                                    onclick: move |_| state.sidebar_help.set(Some(true)),
                                    {tr("console_help")}
                                }
                            }
                        }
                        button {
                            class: "icon-btn console-sidebar-toggle",
                            aria_expanded: !collapsed,
                            aria_controls: "oxdm-console-sidebar-content",
                            aria_label: toggle_label,
                            title: toggle_label,
                            onclick: move |_| state.sidebar_collapsed.set(!collapsed),
                            Icon { name: toggle_icon, size: 16 }
                        }
                    }
                }
                div {
                    id: "oxdm-console-sidebar-content", class: "console-sidebar-content", hidden: collapsed,
                    div { class: "console-device-list", hidden: show_help,
                        DeviceList { picking_target }
                    }
                    if show_help {
                        CommandGuide {}
                    }
                }
            }
            if in_console && !collapsed && !picking_target {
                div {
                    id: "oxdm-console-sidebar-resizer", class: "console-sidebar-resizer",
                    role: "separator", aria_orientation: "vertical",
                    aria_label: tr("console_sidebar_resize"), title: tr("console_sidebar_resize"),
                    aria_controls: "oxdm-console-sidebar-content", tabindex: 0,
                    onmounted: move |_| { let _ = document::eval(SIDEBAR_RESIZER); },
                }
            }
        }
    }
}

#[component]
fn CommandGuide() -> Element {
    let ctx = use_context::<Ctx>();
    let state = use_context::<ConsoleState>();
    let locale = *ctx.locale.read();
    let tr = |key| i18n::t(locale, key);
    let loaded = current_grammar(state);
    let draft = state.draft.read();
    let help = loaded
        .as_ref()
        .map(|(_, grammar)| grammar.command_help(&draft, locale));
    let title = help
        .as_ref()
        .filter(|help| !help.path.is_empty())
        .map_or(tr("console_help"), |help| help.path.as_str());
    let has_subcommands = help
        .as_ref()
        .is_some_and(|help| !help.subcommands.is_empty());

    rsx! {
        aside { class: "console-guide", key: "{title}", aria_label: tr("console_help"), tabindex: 0,
            if let Some(help) = help.as_ref() {
                if help.path.is_empty() {
                    p { {tr("console_help_start")} }
                    p { {tr("console_help_target")} }
                } else {
                    h2 { "{title}" }
                    if let Some(summary) = help.summary {
                        p { "{summary}" }
                    }
                }
                if has_subcommands {
                    h3 { {tr("console_help_commands")} }
                    ul { class: "console-guide-commands",
                        for node in &help.subcommands {
                            li { key: "{node.name}",
                                DraftButton {
                                    command: if help.path.is_empty() { node.name.clone() } else { format!("{} {}", help.path, node.name) },
                                    label: node.name.clone(),
                                }
                                if let Some(about) = node.summary(locale) {
                                    p { "{about}" }
                                }
                            }
                        }
                    }
                }
                if let Some(template) = &help.template {
                    h3 { {tr("console_help_usage")} }
                    pre { code { "{help.path} " {tr("console_help_usage_options")} } }
                    if help.camera {
                        p { {tr("console_help_target")} }
                    } else if help.path == "config validate" {
                        p { {tr("console_help_config_validate")} }
                    }
                    if help.workspace_selector {
                        p { {tr("console_help_workspace_selector")} }
                    }
                    if help.workspace_change.is_some() {
                        p { {tr("console_workspace_write")} }
                    }
                    if help.credential_input {
                        p { {tr("console_credential_hint")} }
                    }
                    if help.path == "device import" {
                        p { {tr("console_import_plan_required")} }
                    }
                    h3 { {tr("console_help_examples")} }
                    if help.watch {
                        p { {tr("console_help_watch")} }
                    }
                    CommandExample { command: template.clone() }
                    if help.watch && help.arguments.iter().any(|arg| arg.label() == "--topic") {
                        p { {tr("console_help_watch_topic")} }
                        CommandExample { command: format!("{} --duration 30s --topic VideoSource/MotionAlarm", help.path) }
                    }
                    if !help.arguments.is_empty() {
                        h3 { {tr("console_help_options")} }
                        dl {
                            for arg in &help.arguments {
                                div { key: "{arg.label()}",
                                    dt {
                                        code { {arg.help_label()} }
                                        if arg.takes_value() {
                                            span { " " {tr("console_help_option_value")} }
                                        }
                                        if arg.required {
                                            span { " · " {tr("console_required")} }
                                        }
                                        if arg.repeatable() {
                                            span { " · " {tr("console_repeatable")} }
                                        }
                                    }
                                    dd {
                                        if let Some(help) = arg.description(locale) {
                                            p { "{help}" }
                                        }
                                        if !arg.values().is_empty() {
                                            p { {tr("console_help_values")} " " code { {arg.values().join(", ")} } }
                                        }
                                        if !arg.defaults.is_empty() {
                                            p { {tr("console_default")} " " code { {arg.defaults.join(", ")} } }
                                        }
                                    }
                                }
                            }
                        }
                    }
                } else if help.external_terminal {
                    p { {tr("console_external_help")} }
                    CommandExample { command: help.path.clone() }
                    dl {
                        for arg in &help.arguments {
                            div { key: "{arg.label()}",
                                dt { code { {arg.label()} } }
                                dd { {arg.description(locale).unwrap_or_default()} }
                            }
                        }
                    }
                } else if !help.path.is_empty() && !has_subcommands {
                    p { {tr("console_unsupported_command")} }
                }
            } else {
                p { {tr("console_help_start")} }
            }
        }
    }
}

#[component]
fn CommandExample(command: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        div { class: "console-guide-example",
            pre { code { "{command}" } }
            DraftButton { command, label: i18n::t(locale, "console_use_example").to_owned() }
        }
    }
}

#[component]
fn DraftButton(command: String, label: String) -> Element {
    let state = use_context::<ConsoleState>();
    rsx! {
        button { class: "btn btn--small",
            onclick: move |_| set_editor(command.clone(), command.len(), state.draft.peek().as_str()),
            "{label}"
        }
    }
}
