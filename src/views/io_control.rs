#![allow(non_snake_case)]
//! IO Control view — relay outputs (controllable) + digital inputs (read-only).
//!
//! Relay outputs come with `mode`/`delay_time`/`idle_state` properties that
//! `SetRelayOutputSettings` writes, and a logical state (`active`/`inactive`)
//! that `SetRelayOutputState` flips. Live input transitions arrive via the
//! Events tab (PullPoint subscription on `tns1:Device/Trigger/DigitalInput`);
//! this page only shows the configured idle state.

use crate::components::{Icon, TabError};
use crate::state::{ConfirmDialog, Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;

#[component]
pub fn IoControlView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let mut relays = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move { api::get_relay_outputs(&addr, &creds).await }
    });

    let mut inputs = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move { api::get_digital_inputs(&addr, &creds).await }
    });

    // Edit drawer for relay properties. Some(token) = editing that relay.
    let editing: Signal<Option<String>> = use_signal(|| None);

    rsx! {
        div { class: "io-control-view",
            div { class: "content-header",
                Icon { name: "zap", size: 20 }
                span { class: "content-title", {i18n::t(locale, "io_control")} }
            }

            // ── Relay Outputs ──────────────────────────────────────────
            div { class: "io-section",
                div { class: "io-section-header",
                    h3 { {i18n::t(locale, "io_relay_outputs")} }
                }
                match &*relays.read_unchecked() {
                    None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
                    Some(Err(e)) if e == "unsupported" => rsx! {
                        div { class: "tab-empty", {i18n::t(locale, "io_relays_unsupported")} }
                    },
                    Some(Err(e)) => rsx! {
                        TabError { error: e.to_string(), on_retry: move |_| relays.restart() }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "tab-empty", {i18n::t(locale, "io_no_relays")} }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "io-list",
                            for relay in list.iter().cloned() {
                                RelayCard {
                                    key: "{relay.token}",
                                    relay: relay,
                                    addr,
                                    creds,
                                    on_changed: move |_| relays.restart(),
                                    editing,
                                }
                            }
                        }
                    },
                }
            }

            // ── Digital Inputs ─────────────────────────────────────────
            div { class: "io-section",
                div { class: "io-section-header",
                    h3 { {i18n::t(locale, "io_digital_inputs")} }
                    span { class: "io-section-hint",
                        {i18n::t(locale, "io_input_hint")}
                    }
                }
                match &*inputs.read_unchecked() {
                    None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
                    Some(Err(e)) if e == "unsupported" => rsx! {
                        div { class: "tab-empty", {i18n::t(locale, "io_inputs_unsupported")} }
                    },
                    Some(Err(e)) => rsx! {
                        TabError { error: e.to_string(), on_retry: move |_| inputs.restart() }
                    },
                    Some(Ok(list)) if list.is_empty() => rsx! {
                        div { class: "tab-empty", {i18n::t(locale, "io_no_inputs")} }
                    },
                    Some(Ok(list)) => rsx! {
                        div { class: "io-list",
                            for input in list.iter().cloned() {
                                div { class: "io-card", key: "{input.token}",
                                    div { class: "io-card-row",
                                        span { class: "io-card-token", "{input.token}" }
                                        span { class: "io-card-prop",
                                            {format!("{}: {}", i18n::t(locale, "io_idle_state"),
                                                idle_label(locale, &input.idle_state))}
                                        }
                                    }
                                }
                            }
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn RelayCard(
    relay: oxvif::RelayOutput,
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    on_changed: EventHandler<()>,
    editing: Signal<Option<String>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let token = relay.token.clone();
    let is_monostable = relay.mode == "Monostable";
    let is_editing = editing
        .read()
        .as_ref()
        .map(|t| t == &token)
        .unwrap_or(false);

    rsx! {
        div { class: "io-card",
            div { class: "io-card-row",
                span { class: "io-card-token", "{relay.token}" }
                span { class: "io-card-prop",
                    {format!("{}: {}", i18n::t(locale, "io_mode"), mode_label(locale, &relay.mode))}
                }
                span { class: "io-card-prop",
                    {format!("{}: {}", i18n::t(locale, "io_idle_state"),
                        idle_label(locale, &relay.idle_state))}
                }
                if is_monostable {
                    span { class: "io-card-prop",
                        {format!("{}: {}", i18n::t(locale, "io_delay_time"), relay.delay_time)}
                    }
                }
            }
            div { class: "io-card-actions",
                if is_monostable {
                    button {
                        class: "btn btn-sm btn-primary",
                        onclick: {
                            let token = token.clone();
                            let on_changed = on_changed;
                            move |_| {
                                let token = token.clone();
                                let addr_val = addr.read().clone();
                                let creds_val = creds.read().clone();
                                let ctx = ctx;
                                spawn(async move {
                                    match api::set_relay_output_state(&addr_val, &creds_val, &token, "active").await {
                                        Ok(()) => {
                                            ctx.push_toast(ToastLevel::Success,
                                                i18n::t(locale, "io_pulse_sent"));
                                            on_changed.call(());
                                        }
                                        Err(e) => ctx.push_toast(ToastLevel::Error,
                                            format!("{}: {e}", i18n::t(locale, "io_pulse_failed"))),
                                    }
                                });
                            }
                        },
                        {i18n::t(locale, "io_pulse")}
                    }
                } else {
                    button {
                        class: "btn btn-sm btn-primary",
                        onclick: {
                            let token = token.clone();
                            let on_changed = on_changed;
                            move |_| {
                                let token = token.clone();
                                let addr_val = addr.read().clone();
                                let creds_val = creds.read().clone();
                                let ctx = ctx;
                                spawn(async move {
                                    match api::set_relay_output_state(&addr_val, &creds_val, &token, "active").await {
                                        Ok(()) => {
                                            ctx.push_toast(ToastLevel::Success,
                                                i18n::t(locale, "io_activated"));
                                            on_changed.call(());
                                        }
                                        Err(e) => ctx.push_toast(ToastLevel::Error,
                                            format!("{}: {e}", i18n::t(locale, "io_set_state_failed"))),
                                    }
                                });
                            }
                        },
                        {i18n::t(locale, "io_activate")}
                    }
                    button {
                        class: "btn btn-sm btn-ghost",
                        onclick: {
                            let token = token.clone();
                            let on_changed = on_changed;
                            move |_| {
                                let token = token.clone();
                                let addr_val = addr.read().clone();
                                let creds_val = creds.read().clone();
                                let ctx = ctx;
                                spawn(async move {
                                    match api::set_relay_output_state(&addr_val, &creds_val, &token, "inactive").await {
                                        Ok(()) => {
                                            ctx.push_toast(ToastLevel::Success,
                                                i18n::t(locale, "io_deactivated"));
                                            on_changed.call(());
                                        }
                                        Err(e) => ctx.push_toast(ToastLevel::Error,
                                            format!("{}: {e}", i18n::t(locale, "io_set_state_failed"))),
                                    }
                                });
                            }
                        },
                        {i18n::t(locale, "io_deactivate")}
                    }
                }
                button {
                    class: "btn btn-sm btn-ghost",
                    onclick: {
                        let token = token.clone();
                        let mut editing = editing;
                        move |_| editing.set(Some(token.clone()))
                    },
                    Icon { name: "edit-2", size: 12 }
                    span { style: "margin-left: 4px", {i18n::t(locale, "io_edit")} }
                }
            }

            if is_editing {
                RelayEditDrawer {
                    relay: relay.clone(),
                    addr,
                    creds,
                    on_close: move |_| editing.set(None),
                    on_saved: {
                        let on_changed = on_changed;
                        move |_| {
                            editing.set(None);
                            on_changed.call(());
                        }
                    },
                }
            }
        }
    }
}

#[component]
fn RelayEditDrawer(
    relay: oxvif::RelayOutput,
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut mode = use_signal(|| relay.mode.clone());
    let mut delay_time = use_signal(|| relay.delay_time.clone());
    let mut idle_state = use_signal(|| relay.idle_state.clone());
    let token = relay.token.clone();

    rsx! {
        div { class: "io-edit-drawer",
            div { class: "io-edit-row",
                label { {i18n::t(locale, "io_mode")} }
                select {
                    value: "{mode}",
                    onchange: move |evt| mode.set(evt.value()),
                    option { value: "Bistable", selected: *mode.read() == "Bistable",
                        {i18n::t(locale, "io_mode_bistable")} }
                    option { value: "Monostable", selected: *mode.read() == "Monostable",
                        {i18n::t(locale, "io_mode_monostable")} }
                }
            }
            div { class: "io-edit-row",
                label { {i18n::t(locale, "io_idle_state")} }
                select {
                    value: "{idle_state}",
                    onchange: move |evt| idle_state.set(evt.value()),
                    option { value: "closed", selected: *idle_state.read() == "closed",
                        {i18n::t(locale, "io_idle_closed")} }
                    option { value: "open", selected: *idle_state.read() == "open",
                        {i18n::t(locale, "io_idle_open")} }
                }
            }
            div { class: "io-edit-row",
                label { {i18n::t(locale, "io_delay_time")} }
                input {
                    r#type: "text",
                    value: "{delay_time}",
                    placeholder: "PT1S",
                    oninput: move |evt| delay_time.set(evt.value()),
                }
                span { class: "io-edit-hint", {i18n::t(locale, "io_delay_hint")} }
            }
            div { class: "io-edit-actions",
                button {
                    class: "btn btn-sm btn-ghost",
                    onclick: move |_| on_close.call(()),
                    {i18n::t(locale, "btn_cancel")}
                }
                button {
                    class: "btn btn-sm btn-primary",
                    onclick: {
                        let token = token.clone();
                        move |_| {
                            let token = token.clone();
                            let mode_val = mode.read().clone();
                            let delay_val = delay_time.read().clone();
                            let idle_val = idle_state.read().clone();
                            let addr_val = addr.read().clone();
                            let creds_val = creds.read().clone();
                            let ctx = ctx;
                            let on_saved = on_saved;
                            // Editing relay properties is reversible (it just
                            // changes idle state / pulse duration), but it
                            // still gates electrical behaviour, so confirm.
                            ctx.dialog.clone().set(Some(ConfirmDialog {
                                title: i18n::t(locale, "io_confirm_save_title").to_string(),
                                message: i18n::t(locale, "io_confirm_save_msg").to_string(),
                                confirm_label: i18n::t(locale, "btn_save").to_string(),
                                cancel_label: i18n::t(locale, "btn_cancel").to_string(),
                                dangerous: false,
                                on_confirm: EventHandler::new(move |_| {
                                    let token = token.clone();
                                    let mode_val = mode_val.clone();
                                    let delay_val = delay_val.clone();
                                    let idle_val = idle_val.clone();
                                    let addr_val = addr_val.clone();
                                    let creds_val = creds_val.clone();
                                    spawn(async move {
                                        match api::set_relay_output_settings(
                                            &addr_val, &creds_val, &token,
                                            &mode_val, &delay_val, &idle_val).await {
                                            Ok(()) => {
                                                ctx.push_toast(ToastLevel::Success,
                                                    i18n::t(locale, "io_settings_saved"));
                                                on_saved.call(());
                                            }
                                            Err(e) => ctx.push_toast(ToastLevel::Error,
                                                format!("{}: {e}",
                                                    i18n::t(locale, "io_settings_failed"))),
                                        }
                                    });
                                }),
                            }));
                        }
                    },
                    {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}

fn mode_label(locale: crate::state::Locale, raw: &str) -> String {
    match raw {
        "Bistable" => i18n::t(locale, "io_mode_bistable").to_string(),
        "Monostable" => i18n::t(locale, "io_mode_monostable").to_string(),
        other => other.to_string(),
    }
}

fn idle_label(locale: crate::state::Locale, raw: &str) -> String {
    match raw {
        "closed" => i18n::t(locale, "io_idle_closed").to_string(),
        "open" => i18n::t(locale, "io_idle_open").to_string(),
        "" => i18n::t(locale, "io_idle_unknown").to_string(),
        other => other.to_string(),
    }
}
