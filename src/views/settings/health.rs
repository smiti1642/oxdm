#![allow(non_snake_case)]
use crate::components::Icon;
use crate::persist;
use crate::state::{Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;
use oxvif::health::{CheckResult, CheckStatus, HealthReport, ProfileVerdict, ReportDiff};

/// Device diagnostics tab — runs oxvif's read-only ONVIF health check on
/// demand and renders the per-check report (grouped by category) plus the
/// Profile S/T/G assessment. Manual trigger: the check makes ~15 network
/// calls, so we never auto-run it on tab open.
///
/// Once a report is on screen, the user can "Save as baseline" — the JSON
/// representation lands in `~/.oxdm/baselines/<addr>.json`. On the next
/// run, if a baseline exists we render a "Diff vs baseline" section
/// flagging checks that flipped, were added/removed, or slowed down by
/// more than 2× (the oxvif 0.9.8 ReportDiff thresholds).
#[component]
pub fn HealthTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let ctx_for_save = ctx;
    let locale = *ctx.locale.read();
    let mut report = use_signal(|| None::<HealthReport>);
    let mut running = use_signal(|| false);

    // Eagerly load any saved baseline for this address so we can render
    // the "baseline from <date>" note and compute a diff after the next run.
    let baseline = use_memo(move || persist::read_baseline(&addr.read()));
    let baseline_saved_at = use_memo(move || persist::baseline_saved_at(&addr.read()));

    let run = move |_| {
        if *running.read() {
            return;
        }
        let addr = addr.read().clone();
        if addr.is_empty() {
            return;
        }
        let creds = creds.peek().clone();
        running.set(true);
        spawn(async move {
            let r = api::run_health_check(&addr, &creds, false, false, false).await;
            report.set(Some(r));
            running.set(false);
        });
    };

    let save_baseline = move |_| {
        let Some(rep) = report.peek().clone() else {
            return;
        };
        let addr_s = addr.read().clone();
        if addr_s.is_empty() {
            return;
        }
        match persist::write_baseline(&addr_s, &rep) {
            Ok(_path) => {
                ctx_for_save.push_toast(
                    ToastLevel::Success,
                    i18n::t(locale, "health_baseline_saved"),
                );
            }
            Err(e) => {
                ctx_for_save.push_toast(
                    ToastLevel::Error,
                    format!("{}: {e}", i18n::t(locale, "health_baseline_save_failed")),
                );
            }
        }
    };

    let busy = *running.read();
    let guard = report.read();
    let baseline_guard = baseline.read();
    let baseline_when = baseline_saved_at.read();

    // Compute the diff only when both a baseline and a fresh report exist.
    let diff: Option<ReportDiff> = match (guard.as_ref(), baseline_guard.as_ref()) {
        (Some(now), Some(prev)) => Some(now.diff(prev)),
        _ => None,
    };

    rsx! {
        div { class: "health-view",
            div { class: "health-header",
                button {
                    class: "btn btn-md btn-primary",
                    disabled: busy || addr.read().is_empty(),
                    onclick: run,
                    Icon { name: "activity", size: 16 }
                    if busy { {i18n::t(locale, "health_running")} } else { {i18n::t(locale, "health_run")} }
                }
                // Save baseline button — only meaningful once a run has produced a report.
                if guard.is_some() && !busy {
                    button {
                        class: "btn btn-md btn-secondary",
                        onclick: save_baseline,
                        Icon { name: "save", size: 14 }
                        {i18n::t(locale, "health_save_baseline")}
                    }
                }
                if let Some(rep) = guard.as_ref() {
                    span { class: "health-summary", {summary_text(locale, rep)} }
                }
            }

            // Small note under the header showing the loaded baseline's mtime.
            if let Some(when) = baseline_when.as_ref() {
                div { class: "health-baseline-note",
                    Icon { name: "clock", size: 12 }
                    {format!("{}: {}", i18n::t(locale, "health_baseline_loaded"), when)}
                }
            }

            match (guard.as_ref(), busy) {
                (_, true) => rsx! {
                    div { class: "health-empty", {i18n::t(locale, "health_running")} }
                },
                (None, false) => rsx! {
                    div { class: "health-empty",
                        Icon { name: "activity", size: 28 }
                        p { {i18n::t(locale, "health_empty")} }
                    }
                },
                (Some(rep), false) => rsx! {
                    div { class: "health-results",
                        div { class: "health-target", code { "{rep.target}" } }

                        for (category , checks) in group_by_category(rep) {
                            div { class: "health-group",
                                div { class: "health-group-title", "{category}" }
                                for c in checks {
                                    div { class: "health-row {status_class(&c.status)}",
                                        span { class: "health-row-status",
                                            Icon { name: status_icon(&c.status), size: 14 }
                                        }
                                        span { class: "health-row-name", "{c.id}" }
                                        span { class: "health-row-time", "{c.elapsed.unwrap_or_default().as_millis()} ms" }
                                        span { class: "health-row-detail", "{row_message(c)}" }
                                    }
                                }
                            }
                        }

                        div { class: "health-group",
                            div { class: "health-group-title", {i18n::t(locale, "health_profiles")} }
                            ProfileRow { locale, name: "Profile S", verdict: rep.profiles.profile_s.verdict, missing: rep.profiles.profile_s.missing.clone() }
                            ProfileRow { locale, name: "Profile T", verdict: rep.profiles.profile_t.verdict, missing: rep.profiles.profile_t.missing.clone() }
                            ProfileRow { locale, name: "Profile G", verdict: rep.profiles.profile_g.verdict, missing: rep.profiles.profile_g.missing.clone() }
                        }

                        // Diff vs baseline — only when one was loaded.
                        if let Some(d) = diff.as_ref() {
                            DiffSection { locale, diff: d.clone() }
                        }
                    }
                },
            }
        }
    }
}

#[component]
fn ProfileRow(
    locale: crate::state::Locale,
    name: &'static str,
    verdict: ProfileVerdict,
    missing: Vec<String>,
) -> Element {
    rsx! {
        div { class: "health-prow {verdict_class(&verdict)}",
            span { class: "health-prow-name", "{name}" }
            span { class: "health-prow-verdict",
                {verdict_label(locale, &verdict)}
                if !missing.is_empty() {
                    {format!("  ({}: {})", i18n::t(locale, "health_missing"), missing.join(", "))}
                }
            }
        }
    }
}

#[component]
fn DiffSection(locale: crate::state::Locale, diff: ReportDiff) -> Element {
    let is_empty = diff.is_empty();
    rsx! {
        div { class: "health-group health-diff",
            div { class: "health-group-title", {i18n::t(locale, "health_diff_title")} }
            if is_empty {
                div { class: "health-row health-pass",
                    span { class: "health-row-status",
                        Icon { name: "check", size: 14 }
                    }
                    span { class: "health-row-detail", {i18n::t(locale, "health_diff_none")} }
                }
            } else {
                if !diff.flipped_to_fail.is_empty() {
                    DiffRow {
                        icon: "x",
                        cls: "health-fail",
                        label: i18n::t(locale, "health_diff_flipped_fail").to_string(),
                        ids: diff.flipped_to_fail.join(", "),
                    }
                }
                if !diff.flipped_to_pass.is_empty() {
                    DiffRow {
                        icon: "check",
                        cls: "health-pass",
                        label: i18n::t(locale, "health_diff_flipped_pass").to_string(),
                        ids: diff.flipped_to_pass.join(", "),
                    }
                }
                if !diff.new_checks.is_empty() {
                    DiffRow {
                        icon: "plus",
                        cls: "health-warn",
                        label: i18n::t(locale, "health_diff_added").to_string(),
                        ids: diff.new_checks.join(", "),
                    }
                }
                if !diff.removed_checks.is_empty() {
                    DiffRow {
                        icon: "minus",
                        cls: "health-warn",
                        label: i18n::t(locale, "health_diff_removed").to_string(),
                        ids: diff.removed_checks.join(", "),
                    }
                }
                if !diff.slowed.is_empty() {
                    for s in diff.slowed.iter() {
                        div { class: "health-row health-warn",
                            span { class: "health-row-status",
                                Icon { name: "clock", size: 14 }
                            }
                            span { class: "health-row-name", "{s.id}" }
                            span { class: "health-row-detail",
                                {format!(
                                    "{}: {} ms → {} ms",
                                    i18n::t(locale, "health_diff_slowed"),
                                    s.prev_ms,
                                    s.now_ms,
                                )}
                            }
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn DiffRow(icon: &'static str, cls: &'static str, label: String, ids: String) -> Element {
    rsx! {
        div { class: "health-row {cls}",
            span { class: "health-row-status",
                Icon { name: icon, size: 14 }
            }
            span { class: "health-row-name", "{label}" }
            span { class: "health-row-detail", "{ids}" }
        }
    }
}

/// Group checks by category label, preserving first-appearance order
/// (oxvif already emits them grouped, but don't rely on that).
pub(crate) fn group_by_category(rep: &HealthReport) -> Vec<(&'static str, Vec<&CheckResult>)> {
    let mut groups: Vec<(&'static str, Vec<&CheckResult>)> = Vec::new();
    for c in &rep.checks {
        let label = c.category.label();
        match groups.last_mut() {
            Some(g) if g.0 == label => g.1.push(c),
            _ => groups.push((label, vec![c])),
        }
    }
    groups
}

fn summary_text(locale: crate::state::Locale, rep: &HealthReport) -> String {
    let pass = rep.count(|s| matches!(s, CheckStatus::Pass));
    let warn = rep.count(|s| matches!(s, CheckStatus::Warn(_)));
    let fail = rep.count(|s| matches!(s, CheckStatus::Fail(_)));
    let skip = rep.count(|s| matches!(s, CheckStatus::Skip(_)));
    format!(
        "{pass} {} · {warn} {} · {fail} {} · {skip} {}  ({} ms)",
        i18n::t(locale, "health_pass"),
        i18n::t(locale, "health_warn"),
        i18n::t(locale, "health_fail"),
        i18n::t(locale, "health_skip"),
        rep.total_elapsed.as_millis(),
    )
}

pub(crate) fn status_class(s: &CheckStatus) -> &'static str {
    match s {
        CheckStatus::Pass => "health-pass",
        CheckStatus::Warn(_) => "health-warn",
        CheckStatus::Fail(_) => "health-fail",
        CheckStatus::Skip(_) => "health-skip",
    }
}

pub(crate) fn status_icon(s: &CheckStatus) -> &'static str {
    match s {
        CheckStatus::Pass => "check",
        CheckStatus::Warn(_) => "alert-triangle",
        CheckStatus::Fail(_) => "x",
        CheckStatus::Skip(_) => "minus",
    }
}

/// The message shown on a row: Pass uses the info `detail`; Warn/Fail/Skip
/// carry their reason in the status payload (matching `HealthReport`'s own
/// `Display`).
pub(crate) fn row_message(c: &CheckResult) -> &str {
    match &c.status {
        CheckStatus::Pass => &c.detail,
        CheckStatus::Warn(r) | CheckStatus::Fail(r) | CheckStatus::Skip(r) => r,
    }
}

pub(crate) fn verdict_class(v: &ProfileVerdict) -> &'static str {
    match v {
        ProfileVerdict::Conformant => "health-pass",
        ProfileVerdict::Partial => "health-warn",
        ProfileVerdict::Unsupported => "health-skip",
        // Couldn't verify (auth-blocked / skipped) — a warning, not a failure.
        ProfileVerdict::Inconclusive => "health-warn",
    }
}

pub(crate) fn verdict_label(locale: crate::state::Locale, v: &ProfileVerdict) -> &'static str {
    match v {
        ProfileVerdict::Conformant => i18n::t(locale, "health_conformant"),
        ProfileVerdict::Partial => i18n::t(locale, "health_partial"),
        ProfileVerdict::Unsupported => i18n::t(locale, "health_unsupported"),
        ProfileVerdict::Inconclusive => i18n::t(locale, "health_inconclusive"),
    }
}
