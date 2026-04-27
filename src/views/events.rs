#![allow(non_snake_case)]
use crate::components::Icon;
use crate::state::{Credentials, Ctx};
use crate::{api, i18n};
use dioxus::prelude::*;
use std::collections::{HashSet, VecDeque};

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
/// Topic namespace prefix used when building filter expressions. oxvif's
/// `EventProperties` flattens the TopicSet without preserving prefixes, so
/// we reattach the standard ONVIF `tns1:` here. Cameras that advertise
/// topics in other namespaces (tnsaxis:, etc.) would need their events
/// filtered server-side by a different mechanism.
const TOPIC_NS_PREFIX: &str = "tns1:";

#[derive(Clone, PartialEq)]
struct EventRow {
    seq: u64,
    received_at: String,
    utc_time: String,
    topic: String,
    operation: String,
    fields: String,
    source: Vec<(String, String)>,
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
    let search = use_signal(String::new);
    // Default: newest-first by receive sequence. Clicking a column header
    // cycles that column between asc/desc; clicking Time resets to default.
    let sort_mode = use_signal(|| (SortColumn::Time, SortDir::Desc));

    // Topics the user has toggled OFF. Empty set = receive everything.
    // This inversion means the default (no interaction) is "no filter",
    // and building the subscription filter only needs the subset list
    // when the user has actively hidden something.
    let hidden_topics = use_signal(HashSet::<String>::new);
    let topics_panel_open = use_signal(|| false);

    // Fetch available topics once per device. We don't force the
    // subscription to wait for this — the subscription uses whatever
    // filter is currently derivable from hidden_topics (empty at first,
    // so no filter).
    let topics_info = use_resource(move || {
        let addr_now = addr.read().clone();
        let creds_now = creds.read().clone();
        async move {
            let (u, p) = creds_now.as_options();
            let events_url = api::get_events_url(&addr_now, u, p).await?;
            api::get_event_properties(&addr_now, u, p, &events_url).await
        }
    });

    // Long-running subscription task. Reads hidden_topics so any toggle
    // triggers Dioxus to drop this future (running the SubscriptionGuard)
    // and spawn a new one with the updated filter.
    let _task = use_future(move || {
        let addr_now = addr.read().clone();
        let creds_now = creds.read().clone();
        let filter = build_filter_expression(&hidden_topics.read(), topics_info);
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
                filter.as_deref(),
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
                        status.set(StatusKind::Error(format!("Renew: {e}")));
                        return;
                    }
                }
            }
        }
    });

    let total_count = events.read().len();
    let status_text = match &*status.read() {
        StatusKind::Connecting => i18n::t(locale, "events_status_connecting").to_string(),
        StatusKind::Connected => i18n::t(locale, "events_status_connected").to_string(),
        StatusKind::Error(e) => format!("{}: {e}", i18n::t(locale, "events_status_error")),
    };

    let topics_label = topics_button_label(locale, &hidden_topics.read(), topics_info);

    // Build the render list: clone → filter by search → sort.
    // Sorting a live log feels jarring at very high rates, but the mock
    // (and most cameras) cap out at a few events/sec so re-sorting on
    // each render is cheap enough. Newest-first by seq is the default,
    // matching the behaviour before this feature.
    let visible_rows: Vec<EventRow> = {
        let needle = search.read().to_lowercase();
        let (col, dir) = *sort_mode.read();
        let mut v: Vec<EventRow> = events
            .read()
            .iter()
            .filter(|r| needle.is_empty() || searchable_text(r).contains(&needle))
            .cloned()
            .collect();
        sort_rows(&mut v, col, dir);
        v
    };
    let visible_count = visible_rows.len();
    let count_label = if visible_count == total_count {
        format!("{total_count}")
    } else {
        format!("{visible_count}/{total_count}")
    };

    rsx! {
        div { class: "events-view",
            div { class: "content-header",
                Icon { name: "bell", size: 20 }
                span { class: "content-title", {i18n::t(locale, "nav_events")} }
                span { class: "events-status", "{status_text}" }
                span { class: "events-count", "{count_label}" }
                input {
                    class: "events-search",
                    r#type: "text",
                    placeholder: i18n::t(locale, "events_search_placeholder"),
                    value: "{*search.read()}",
                    oninput: {
                        let mut search = search;
                        move |evt: Event<FormData>| search.set(evt.value())
                    },
                }
                TopicsButton { topics_info, hidden_topics, topics_panel_open, label: topics_label }
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
                if total_count == 0 {
                    div { class: "events-empty", {i18n::t(locale, "events_empty")} }
                } else if visible_count == 0 {
                    div { class: "events-empty", {i18n::t(locale, "events_empty_filtered")} }
                } else if *show_details.read() {
                    EventsDetailedTable { rows: visible_rows, sort_mode }
                } else {
                    EventsCompactTable { rows: visible_rows, sort_mode }
                }
            }
        }
    }
}

/// Build the `tns1:a|tns1:b|...` filter expression. Returns `None` when no
/// topics are hidden (= receive everything, no filter header sent) or when
/// every topic is hidden (no useful events possible — but we still send a
/// harmless filter so the caller sees zero events instead of all).
fn build_filter_expression(
    hidden: &HashSet<String>,
    topics_info: Resource<Result<oxvif::EventProperties, String>>,
) -> Option<String> {
    if hidden.is_empty() {
        return None;
    }
    let guard = topics_info.read();
    let Some(Ok(props)) = guard.as_ref() else {
        // Topics haven't finished loading yet — we can't compute the
        // positive list. Falling back to no filter means the user will
        // briefly see hidden topics until the list arrives and the
        // subscription rebuilds; that's better than seeing nothing.
        return None;
    };
    let included: Vec<String> = props
        .topics
        .iter()
        .filter(|t| !hidden.contains(*t))
        .map(|t| format!("{TOPIC_NS_PREFIX}{t}"))
        .collect();
    if included.is_empty() {
        // Everything hidden — send a filter that matches nothing so the
        // camera stops sending. Empty string would be ambiguous, so we
        // use a synthetic never-match topic.
        Some("tns1:__none__".to_string())
    } else {
        Some(included.join("|"))
    }
}

fn topics_button_label(
    locale: crate::state::Locale,
    hidden: &HashSet<String>,
    topics_info: Resource<Result<oxvif::EventProperties, String>>,
) -> String {
    let base = i18n::t(locale, "events_topics");
    let guard = topics_info.read();
    match guard.as_ref() {
        Some(Ok(props)) => {
            let total = props.topics.len();
            let shown = total.saturating_sub(hidden.len());
            format!("{base} ({shown}/{total})")
        }
        _ => base.to_string(),
    }
}

#[component]
fn TopicsButton(
    topics_info: Resource<Result<oxvif::EventProperties, String>>,
    hidden_topics: Signal<HashSet<String>>,
    topics_panel_open: Signal<bool>,
    label: String,
) -> Element {
    let is_open = *topics_panel_open.read();

    rsx! {
        div { class: "events-topics-wrap",
            button {
                class: if is_open { "btn btn--small btn--active" } else { "btn btn--small" },
                onclick: {
                    let mut topics_panel_open = topics_panel_open;
                    move |_| {
                        let cur = *topics_panel_open.read();
                        topics_panel_open.set(!cur);
                    }
                },
                "{label}"
            }
            if is_open {
                TopicsPanel { topics_info, hidden_topics, topics_panel_open }
            }
        }
    }
}

#[component]
fn TopicsPanel(
    topics_info: Resource<Result<oxvif::EventProperties, String>>,
    hidden_topics: Signal<HashSet<String>>,
    topics_panel_open: Signal<bool>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let guard = topics_info.read();
    let body = match guard.as_ref() {
        None => rsx! { div { class: "events-topics-loading", {i18n::t(locale, "loading")} } },
        Some(Err(e)) => rsx! {
            div { class: "events-topics-error",
                div { "{e}" }
                button {
                    class: "btn btn--small",
                    onclick: move |_| topics_info.restart(),
                    {i18n::t(locale, "btn_retry")}
                }
            }
        },
        Some(Ok(props)) if props.topics.is_empty() => rsx! {
            div { class: "events-topics-empty", {i18n::t(locale, "events_topics_empty")} }
        },
        Some(Ok(props)) => {
            let items: Vec<String> = props.topics.clone();
            let items_for_none = items.clone();
            rsx! {
                div { class: "events-topics-actions",
                    button {
                        class: "btn btn--small",
                        onclick: move |_| hidden_topics.write().clear(),
                        {i18n::t(locale, "events_topics_all")}
                    }
                    button {
                        class: "btn btn--small",
                        onclick: move |_| {
                            let mut h = hidden_topics;
                            let mut w = h.write();
                            w.clear();
                            for t in &items_for_none {
                                w.insert(t.clone());
                            }
                        },
                        {i18n::t(locale, "events_topics_none")}
                    }
                }
                div { class: "events-topics-list",
                    for topic in items.into_iter() {
                        TopicRow { topic, hidden_topics }
                    }
                }
            }
        }
    };
    rsx! {
        div { class: "events-topics-panel",
            div { class: "events-topics-header",
                span { {i18n::t(locale, "events_topics")} }
                button {
                    class: "btn btn--small",
                    onclick: {
                        let mut topics_panel_open = topics_panel_open;
                        move |_| topics_panel_open.set(false)
                    },
                    "×"
                }
            }
            {body}
        }
    }
}

#[component]
fn TopicRow(topic: String, hidden_topics: Signal<HashSet<String>>) -> Element {
    let checked = !hidden_topics.read().contains(&topic);
    let topic_for_toggle = topic.clone();
    rsx! {
        label { class: "events-topic-row",
            input {
                r#type: "checkbox",
                checked,
                onchange: move |evt: Event<FormData>| {
                    let mut h = hidden_topics;
                    let mut w = h.write();
                    if evt.checked() {
                        w.remove(&topic_for_toggle);
                    } else {
                        w.insert(topic_for_toggle.clone());
                    }
                },
            }
            span { class: "events-topic-row-name", "{topic}" }
        }
    }
}

#[component]
fn EventsCompactTable(rows: Vec<EventRow>, sort_mode: Signal<(SortColumn, SortDir)>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        table { class: "events-table",
            thead { tr {
                SortableTh { col: SortColumn::Time, label: i18n::t(locale, "events_col_time"), sort_mode }
                SortableTh { col: SortColumn::Topic, label: i18n::t(locale, "events_col_topic"), sort_mode }
                th { {i18n::t(locale, "events_col_data")} }
            }}
            tbody {
                for row in rows.iter().cloned() {
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
fn EventsDetailedTable(rows: Vec<EventRow>, sort_mode: Signal<(SortColumn, SortDir)>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    rsx! {
        table { class: "events-table events-table--detailed",
            thead { tr {
                SortableTh { col: SortColumn::Time, label: i18n::t(locale, "events_col_time"), sort_mode }
                th { {i18n::t(locale, "events_col_utc")} }
                SortableTh { col: SortColumn::Op, label: i18n::t(locale, "events_col_op"), sort_mode }
                SortableTh { col: SortColumn::Topic, label: i18n::t(locale, "events_col_topic"), sort_mode }
                th { {i18n::t(locale, "events_col_source")} }
                th { {i18n::t(locale, "events_col_data")} }
            }}
            tbody {
                for row in rows.iter().cloned() {
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

#[component]
fn SortableTh(
    col: SortColumn,
    label: &'static str,
    sort_mode: Signal<(SortColumn, SortDir)>,
) -> Element {
    let (cur_col, cur_dir) = *sort_mode.read();
    let active = cur_col == col;
    let indicator = if active {
        match cur_dir {
            SortDir::Asc => " ↑",
            SortDir::Desc => " ↓",
        }
    } else {
        ""
    };
    let class_name = if active {
        "events-th-sortable events-th-sortable--active"
    } else {
        "events-th-sortable"
    };
    rsx! {
        th {
            class: class_name,
            onclick: {
                let mut sort_mode = sort_mode;
                move |_| {
                    let (c, d) = *sort_mode.read();
                    if c == col {
                        sort_mode.set((col, d.flip()));
                    } else {
                        // First click on a new column: start descending —
                        // for most columns (time, op) that matches the
                        // usual "newest/latest on top" expectation.
                        sort_mode.set((col, SortDir::Desc));
                    }
                }
            },
            "{label}{indicator}"
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum SortColumn {
    Time,
    Topic,
    Op,
}

#[derive(Clone, Copy, PartialEq)]
enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    fn flip(self) -> Self {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

fn sort_rows(rows: &mut [EventRow], col: SortColumn, dir: SortDir) {
    match col {
        SortColumn::Time => rows.sort_by_key(|r| r.seq),
        SortColumn::Topic => rows.sort_by(|a, b| a.topic.cmp(&b.topic)),
        SortColumn::Op => rows.sort_by(|a, b| a.operation.cmp(&b.operation)),
    }
    if matches!(dir, SortDir::Desc) {
        rows.reverse();
    }
}

fn searchable_text(row: &EventRow) -> String {
    // Build one lowercase blob per row so the hot-loop contains needle
    // check stays O(n) instead of re-formatting per keystroke per row.
    // The fields string already flattens source+data for the compact
    // view, so we don't need to iterate them separately.
    format!(
        "{} {} {} {}",
        row.topic, row.operation, row.fields, row.utc_time
    )
    .to_lowercase()
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
    let mut parts: Vec<String> = source
        .iter()
        .chain(data.iter())
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    parts.sort();
    parts.join(", ")
}
