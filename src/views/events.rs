#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{Credentials, Ctx};
use crate::{api, i18n};
use dioxus::prelude::*;
use std::collections::VecDeque;

/// Maximum number of events kept in the in-memory log. Older events are
/// dropped from the front of the deque as new ones arrive.
const MAX_EVENTS: usize = 500;
/// Long-poll timeout per PullMessages. Cameras hold the request open for at
/// most this long before returning whatever they've buffered. Short enough
/// that the user sees the UI react quickly when they pause/leave.
const PULL_TIMEOUT: &str = "PT5S";
const PULL_MAX_MESSAGES: u32 = 20;
/// Initial subscription lifetime requested. We renew well before this fires.
const SUBSCRIPTION_LIFETIME: &str = "PT60S";
/// Renew the subscription every N pull cycles to stay well clear of the
/// 60 s default expiry. With PULL_TIMEOUT=PT5S, this means renew roughly
/// every 25 s — enough headroom even if a pull takes the full timeout.
const RENEW_EVERY_N_PULLS: u32 = 5;

#[derive(Clone, PartialEq)]
struct EventRow {
    seq: u64,
    received_at: String,
    utc_time: String,
    topic: String,
    operation: String,
    /// Pre-rendered "k=v, k=v" for the compact view.
    fields: String,
    /// Sorted source items for the details view.
    source: Vec<(String, String)>,
    /// Sorted data items for the details view.
    data: Vec<(String, String)>,
}

#[component]
pub fn EventsView(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let mut events = use_signal(VecDeque::<EventRow>::new);
    let mut status = use_signal(|| StatusKind::Connecting);
    let paused = use_signal(|| false);
    let show_details = use_signal(|| false);
    let mut next_seq = use_signal(|| 0u64);

    // Long-running subscription task. Lives for the EventsView's component
    // lifetime — when the user navigates away, Dioxus drops the future and
    // the SubscriptionGuard runs unsubscribe in the background.
    let _task = use_future(move || {
        let addr_now = addr.read().clone();
        let creds_now = creds.read().clone();
        async move {
            let (u, p) = creds_now.as_options();
            let user_owned = u.map(str::to_string);
            let pass_owned = p.map(str::to_string);

            let events_url = match api::get_events_url(&addr_now, u, p).await {
                Ok(url) => url,
                Err(e) => {
                    status.set(StatusKind::Error(format!("Capabilities: {e}")));
                    return;
                }
            };

            let sub = match api::create_pull_subscription(
                &addr_now,
                u,
                p,
                &events_url,
                Some(SUBSCRIPTION_LIFETIME),
            )
            .await
            {
                Ok(s) => s,
                Err(e) => {
                    status.set(StatusKind::Error(format!("Subscribe: {e}")));
                    return;
                }
            };
            let sub_url = sub.reference_url.clone();

            // RAII: when the future is dropped (component unmount or task
            // cancellation), spawn a fire-and-forget unsubscribe so we don't
            // leak subscriptions on the camera. Cameras enforce a max number
            // of concurrent subscriptions and won't reclaim them until the
            // termination_time expires.
            let _guard = SubscriptionGuard {
                addr: addr_now.clone(),
                username: user_owned.clone(),
                password: pass_owned.clone(),
                sub_url: sub_url.clone(),
            };

            status.set(StatusKind::Connected);

            let mut pulls_since_renew: u32 = 0;
            loop {
                if *paused.read() {
                    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
                    continue;
                }

                match api::pull_event_messages(
                    &addr_now,
                    u,
                    p,
                    &sub_url,
                    PULL_TIMEOUT,
                    PULL_MAX_MESSAGES,
                )
                .await
                {
                    Ok(msgs) => {
                        if !msgs.is_empty() {
                            let mut log = events.write();
                            for msg in msgs {
                                let mut seq = next_seq.write();
                                *seq += 1;
                                log.push_back(EventRow {
                                    seq: *seq,
                                    received_at: format_now(),
                                    utc_time: msg.utc_time,
                                    topic: msg.topic,
                                    operation: msg.property_operation,
                                    fields: format_fields(&msg.source, &msg.data),
                                    source: sorted_pairs(&msg.source),
                                    data: sorted_pairs(&msg.data),
                                });
                                while log.len() > MAX_EVENTS {
                                    log.pop_front();
                                }
                            }
                        }
                    }
                    Err(e) => {
                        // Transient errors (network blip, camera busy) shouldn't
                        // kill the stream — surface in status and back off briefly.
                        status.set(StatusKind::Error(format!("Pull: {e}")));
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                        status.set(StatusKind::Connected);
                        continue;
                    }
                }

                pulls_since_renew += 1;
                if pulls_since_renew >= RENEW_EVERY_N_PULLS {
                    pulls_since_renew = 0;
                    if let Err(e) = api::renew_event_subscription(
                        &addr_now,
                        u,
                        p,
                        &sub_url,
                        SUBSCRIPTION_LIFETIME,
                    )
                    .await
                    {
                        // Renewal failure means the subscription is probably
                        // gone — surface the error and exit so the user can
                        // navigate away and re-enter to reconnect.
                        status.set(StatusKind::Error(format!("Renew: {e}")));
                        return;
                    }
                }
            }
        }
    });

    let event_count = events.read().len();
    let status_text = match &*status.read() {
        StatusKind::Connecting => i18n::t(locale, "events_status_connecting").to_string(),
        StatusKind::Connected => i18n::t(locale, "events_status_connected").to_string(),
        StatusKind::Error(e) => format!("{}: {e}", i18n::t(locale, "events_status_error")),
    };

    rsx! {
        div { class: "events-view",
            div { class: "content-header",
                Icon { name: "bell", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_events")} }
                span { class: "events-status", "{status_text}" }
                span { class: "events-count", "{event_count}" }
                button {
                    class: "btn btn--small",
                    onclick: {
                        let mut paused = paused;
                        move |_| {
                            let cur = *paused.read();
                            paused.set(!cur);
                        }
                    },
                    {if *paused.read() { i18n::t(locale, "events_resume") } else { i18n::t(locale, "events_pause") }}
                }
                button {
                    class: "btn btn--small",
                    onclick: move |_| events.write().clear(),
                    {i18n::t(locale, "events_clear")}
                }
                label { class: "events-details-toggle",
                    input {
                        r#type: "checkbox",
                        checked: *show_details.read(),
                        onchange: {
                            let mut show_details = show_details;
                            move |evt: Event<FormData>| show_details.set(evt.checked())
                        },
                    }
                    {i18n::t(locale, "events_show_details")}
                }
            }
            div { class: "events-log",
                if event_count == 0 {
                    div { class: "events-empty", {i18n::t(locale, "events_empty")} }
                } else if *show_details.read() {
                    EventsDetailedTable { rows: events.read().clone() }
                } else {
                    EventsCompactTable { rows: events.read().clone() }
                }
            }
        }
    }
}

#[component]
fn EventsCompactTable(rows: VecDeque<EventRow>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        table { class: "events-table",
            thead { tr {
                th { {i18n::t(locale, "events_col_time")} }
                th { {i18n::t(locale, "events_col_topic")} }
                th { {i18n::t(locale, "events_col_data")} }
            }}
            tbody {
                for row in rows.iter().rev().cloned() {
                    tr { key: "{row.seq}",
                        td { class: "events-time", "{row.received_at}" }
                        td { class: "events-topic", "{row.topic}" }
                        td { class: "events-data", "{row.fields}" }
                    }
                }
            }
        }
    }
}

#[component]
fn EventsDetailedTable(rows: VecDeque<EventRow>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        table { class: "events-table events-table--detailed",
            thead { tr {
                th { {i18n::t(locale, "events_col_time")} }
                th { {i18n::t(locale, "events_col_utc")} }
                th { {i18n::t(locale, "events_col_op")} }
                th { {i18n::t(locale, "events_col_topic")} }
                th { {i18n::t(locale, "events_col_source")} }
                th { {i18n::t(locale, "events_col_data")} }
            }}
            tbody {
                for row in rows.iter().rev().cloned() {
                    tr { key: "{row.seq}",
                        td { class: "events-time", "{row.received_at}" }
                        td { class: "events-time", "{row.utc_time}" }
                        td {
                            class: operation_class(&row.operation),
                            "{row.operation}"
                        }
                        td { class: "events-topic", "{row.topic}" }
                        td { class: "events-data",
                            for (k, v) in row.source.iter() {
                                div { "{k}={v}" }
                            }
                        }
                        td { class: "events-data",
                            for (k, v) in row.data.iter() {
                                div { "{k}={v}" }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn operation_class(op: &str) -> &'static str {
    match op {
        "Initialized" => "events-op events-op--init",
        "Changed" => "events-op events-op--changed",
        "Deleted" => "events-op events-op--deleted",
        _ => "events-op",
    }
}

fn sorted_pairs(map: &std::collections::HashMap<String, String>) -> Vec<(String, String)> {
    let mut v: Vec<(String, String)> = map.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
    v.sort_by(|a, b| a.0.cmp(&b.0));
    v
}

#[derive(Clone)]
enum StatusKind {
    Connecting,
    Connected,
    Error(String),
}

impl PartialEq for StatusKind {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (StatusKind::Connecting, StatusKind::Connecting)
                | (StatusKind::Connected, StatusKind::Connected)
        ) || matches!((self, other), (StatusKind::Error(a), StatusKind::Error(b)) if a == b)
    }
}

struct SubscriptionGuard {
    addr: String,
    username: Option<String>,
    password: Option<String>,
    sub_url: String,
}

impl Drop for SubscriptionGuard {
    fn drop(&mut self) {
        let addr = std::mem::take(&mut self.addr);
        let user = self.username.take();
        let pass = self.password.take();
        let sub_url = std::mem::take(&mut self.sub_url);
        if sub_url.is_empty() {
            return;
        }
        // Best-effort: spawn an unsubscribe and forget. If the runtime is
        // shutting down or the camera is unreachable, we accept the leak —
        // the subscription will time out on the camera side anyway.
        tokio::spawn(async move {
            let _ =
                api::unsubscribe_events(&addr, user.as_deref(), pass.as_deref(), &sub_url).await;
        });
    }
}

fn format_now() -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!("{:02}:{:02}:{:02}", now.hour(), now.minute(), now.second())
}

fn format_fields(
    source: &std::collections::HashMap<String, String>,
    data: &std::collections::HashMap<String, String>,
) -> String {
    // Sort for stable display — HashMap iteration order is otherwise random.
    let mut parts: Vec<String> = source
        .iter()
        .chain(data.iter())
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    parts.sort();
    parts.join(", ")
}
