#![allow(non_snake_case)]
use std::collections::{HashMap, HashSet};

use crate::components::Icon;
use crate::i18n;
use crate::state::{Credentials, Ctx, Locale, ToastLevel};
use crate::util::{line_diff, DiffRow};
use dioxus::prelude::*;
use oxvif::metamorph::{
    OpOutcome, OperationDiff, OperationQuirk, ParseStatus, SurfaceGroup, SurfaceOp,
    SurfaceSelection,
};

/// Quirks tab. Two entry states:
///
/// - A **served clone** was analysed when it was served, so its results render
///   immediately and there is nothing to run.
/// - A **real camera** has no analysis until it is tested, so it gets the
///   live-test panel: pick a read surface, run the strictly read-only sweep
///   straight against the device, watch it, then read the same results.
///
/// Either way the results are the operations whose response shape drifts from
/// oxvif's synthetic baseline and/or whose response oxvif's own parser rejects;
/// each row expands into a git-style **left/right** line diff of the two SOAP
/// responses (baseline vs camera), and the checked rows export to JSON.
#[component]
pub fn QuirkTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut selected = use_signal(HashSet::<String>::new);
    let expanded = use_signal(HashSet::<String>::new);
    // Result groups whose open/closed state the user flipped away from the
    // default (open iff the group has something wrong in it), so no effect has
    // to seed the set before the first render.
    let mut flipped = use_signal(HashSet::<&'static str>::new);
    let mut show_clean = use_signal(|| false);
    // Bumped when a live test lands, so the pool lookups below and the parse
    // resource both re-read.
    let run_seq = use_signal(|| 0u32);
    // Re-opens the test panel for a camera that already carries a result.
    let mut panel_open = use_signal(|| false);

    // Parse verification (value/type layer) runs oxvif's own parser over the
    // recorded responses — async, so it loads via a resource and is joined to
    // the structural quirks below by `key_canon`.
    let parse = use_resource(move || {
        let url = addr.read().clone();
        // Subscribe to the run counter: a fresh sweep replaces the store.
        let _ = run_seq.read();
        async move { crate::mock_servers::parse_report(&url).await }
    });

    // Bumped when a baseline is written, so the two hooks below re-read it and
    // the note + diff update without waiting for the address to change.
    let mut baseline_seq = use_signal(|| 0u32);
    // `QuirkReport` is not `PartialEq`, so this cannot be a `use_memo` — an
    // effect keyed on the same two signals reloads it, and the read stays off
    // the render path (it parses a JSON file).
    let mut quirk_baseline = use_signal(|| None::<crate::persist::SavedQuirkBaseline>);
    use_effect(move || {
        let _ = baseline_seq.read();
        quirk_baseline.set(crate::persist::read_quirk_baseline(&addr.read()));
    });
    let quirk_baseline_at = use_memo(move || {
        let _ = baseline_seq.read();
        crate::persist::quirk_baseline_saved_at(&addr.read())
    });

    // Cheap pool lookups each render (no PartialEq on the report → no use_memo).
    let _ = run_seq.read();
    let report = crate::mock_servers::quirks(&addr.read());
    let served = crate::mock_servers::is_served(&addr.read());
    // A served clone replays a store recorded elsewhere; only a real device can
    // be swept, and only it needs the panel.
    let show_panel = !served && (report.is_none() || *panel_open.read());

    // Verdict per operation (for the row badge), the failure list (ops oxvif
    // can't parse — including structurally-clean ones the SOAP diff can't see)
    // and the declined list (ops the *device* refused with a SOAP Fault).
    let (verdicts, parse_failures, parse_declined) = match &*parse.read_unchecked() {
        Some(Some(pr)) => (
            pr.verdicts
                .iter()
                .map(|v| (v.key_canon.clone(), v.status))
                .collect::<HashMap<String, ParseStatus>>(),
            pr.failures().map(verdict_line).collect::<Vec<_>>(),
            pr.faulted().map(verdict_line).collect::<Vec<_>>(),
        ),
        _ => (HashMap::new(), Vec::new(), Vec::new()),
    };

    // Per-group result blocks. 52 operations do not read as a flat list, so
    // rows are bucketed by service zone and each zone collapses.
    let blocks = report.as_ref().map(|rep| {
        group_results(
            crate::mock_servers::details(&addr.read()).unwrap_or_default(),
            &rep.quirks,
            &verdicts,
            crate::mock_servers::sweep_outcomes(&addr.read()),
            i18n::t(locale, "quirk_group_other"),
        )
    });

    // Everything the default view would show: structural drift, parse failures
    // and operations the device errored on — skips are deliberately not counted.
    let problems: usize = blocks
        .as_ref()
        .map(|bs| bs.iter().map(|b| b.problems()).sum())
        .unwrap_or(0);

    let all_keys: Vec<String> = report
        .as_ref()
        .map(|r| r.quirks.iter().map(|q| q.key_canon.clone()).collect())
        .unwrap_or_default();
    let sel_count = selected.read().len();
    let all_selected = !all_keys.is_empty() && all_keys.iter().all(|k| selected.read().contains(k));

    let export = move |_| {
        let Some(rep) = crate::mock_servers::quirks(&addr.read()) else {
            return;
        };
        let sel = selected.read().clone();
        let chosen: Vec<&oxvif::metamorph::OperationQuirk> = rep
            .quirks
            .iter()
            .filter(|q| sel.contains(&q.key_canon))
            .collect();
        if chosen.is_empty() {
            ctx.push_toast(ToastLevel::Info, i18n::t(locale, "quirk_export_nothing"));
            return;
        }
        #[derive(serde::Serialize)]
        struct Export<'a> {
            device: &'a str,
            compared: usize,
            exported: usize,
            quirks: Vec<&'a oxvif::metamorph::OperationQuirk>,
        }
        let json = serde_json::to_string_pretty(&Export {
            device: &rep.device,
            compared: rep.compared,
            exported: chosen.len(),
            quirks: chosen,
        })
        .unwrap_or_default();
        let file_name = format!("oxdm-quirks-{}.json", crate::util::now_file_stamp());
        spawn(async move {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name(&file_name)
                .add_filter("JSON", &["json"])
                .save_file()
                .await
            else {
                return;
            };
            let path = handle.path().to_path_buf();
            match std::fs::write(&path, json.as_bytes()) {
                Ok(()) => ctx.push_toast(
                    ToastLevel::Success,
                    format!("{}: {}", i18n::t(locale, "quirk_exported"), path.display()),
                ),
                Err(e) => ctx.push_toast(
                    ToastLevel::Error,
                    format!("{}: {e}", i18n::t(locale, "quirk_export_failed")),
                ),
            }
        });
    };

    // The diff only exists once this device has both a baseline and a current
    // report; `QuirkReport::diff` is keyed on `(action, key_canon)`.
    let baseline_diff = match (report.as_ref(), &*quirk_baseline.read()) {
        (Some(now), Some(prev)) => Some(now.diff(&prev.report)),
        _ => None,
    };

    // The baseline's synthetic reference is oxvif's mock, so a baseline saved
    // under a different oxvif than this build links is not a like-for-like
    // comparison — part of the diff below is the library moving, not the
    // camera. Only says so when there *is* a diff to qualify; the stamp alone
    // is not worth a line.
    let baseline_oxvif_moved = baseline_diff.is_some()
        && quirk_baseline
            .read()
            .as_ref()
            .is_some_and(|b| !b.matches_running_oxvif());
    let baseline_saved_under = quirk_baseline
        .read()
        .as_ref()
        .and_then(|b| b.oxvif.clone())
        .unwrap_or_else(|| i18n::t(locale, "quirk_baseline_oxvif_unknown").to_string());

    let save_baseline = move |_| {
        let Some(rep) = crate::mock_servers::quirks(&addr.read()) else {
            return;
        };
        let addr_s = addr.read().clone();
        if addr_s.is_empty() {
            return;
        }
        match crate::persist::write_quirk_baseline(&addr_s, &rep) {
            Ok(_path) => {
                baseline_seq += 1;
                ctx.push_toast(ToastLevel::Success, i18n::t(locale, "quirk_baseline_saved"));
            }
            Err(e) => ctx.push_toast(
                ToastLevel::Error,
                format!("{}: {e}", i18n::t(locale, "quirk_baseline_save_failed")),
            ),
        }
    };

    rsx! {
        div { class: "health-view",

            if show_panel {
                LiveTestPanel { addr, creds, has_result: report.is_some(), run_seq, panel_open }
            }

            if let Some(rep) = report.as_ref() {
                div { class: "health-header quirk-header",
                    label { class: "quirk-selectall",
                        input {
                            r#type: "checkbox",
                            checked: all_selected,
                            onchange: move |_| {
                                let mut s = selected.write();
                                if all_keys.iter().all(|k| s.contains(k)) {
                                    s.clear();
                                } else {
                                    for k in &all_keys {
                                        s.insert(k.clone());
                                    }
                                }
                            },
                        }
                        {i18n::t(locale, "quirk_select_all")}
                    }
                    label { class: "quirk-selectall",
                        input {
                            r#type: "checkbox",
                            checked: *show_clean.read(),
                            onchange: move |_| {
                                let v = *show_clean.peek();
                                show_clean.set(!v);
                            },
                        }
                        {i18n::t(locale, "quirk_show_clean")}
                    }
                    button {
                        class: "btn btn-md btn-secondary",
                        disabled: sel_count == 0,
                        onclick: export,
                        Icon { name: "download", size: 14 }
                        {i18n::t(locale, "quirk_export").replace("{n}", &sel_count.to_string())}
                    }
                    button {
                        class: "btn btn-md btn-secondary",
                        onclick: save_baseline,
                        Icon { name: "save", size: 14 }
                        {i18n::t(locale, "quirk_save_baseline")}
                    }
                    // A real camera can always be swept again; a served clone
                    // has no live device behind it to sweep.
                    if !served && !*panel_open.read() {
                        button {
                            class: "btn btn-md btn-secondary",
                            onclick: move |_| panel_open.set(true),
                            Icon { name: "refresh-cw", size: 14 }
                            {i18n::t(locale, "quirk_live_retest")}
                        }
                    }
                    span { class: "health-summary",
                        {i18n::t(locale, "quirk_summary")
                            .replace("{device}", &rep.device)
                            .replace("{compared}", &rep.compared.to_string())
                            .replace("{quirks}", &rep.quirks.len().to_string())}
                    }
                }

                // Honest scope note — what the quirk finder does and does not cover.
                div { class: "health-baseline-note",
                    Icon { name: "info", size: 12 }
                    {i18n::t(locale, "quirk_scope")}
                }

                if let Some(when) = quirk_baseline_at.read().as_ref() {
                    div { class: "health-baseline-note",
                        Icon { name: "clock", size: 12 }
                        {format!("{}: {}", i18n::t(locale, "quirk_baseline_loaded"), when)}
                    }
                }

                if baseline_oxvif_moved {
                    div { class: "quirk-parse-fail",
                        div { class: "quirk-parse-fail-head",
                            Icon { name: "alert-triangle", size: 12 }
                            {i18n::t(locale, "quirk_baseline_oxvif_moved")
                                .replace("{saved}", &baseline_saved_under)
                                .replace("{now}", crate::components::OXVIF_VERSION)}
                        }
                    }
                }

                // Parse failures — the highest-value signal (oxvif will choke on
                // these). Surfaced separately because a value/type failure can land
                // on an op with no structural drift, invisible in the list below.
                if !parse_failures.is_empty() {
                    div { class: "quirk-parse-fail",
                        div { class: "quirk-parse-fail-head",
                            Icon { name: "alert-triangle", size: 12 }
                            {i18n::t(locale, "quirk_parse_fail_head")
                                .replace("{n}", &parse_failures.len().to_string())}
                        }
                        ul { class: "quirk-parse-fail-list",
                            for (i , (op , err)) in parse_failures.iter().enumerate() {
                                li { key: "{i}",
                                    span { class: "qpf-op", "{op}" }
                                    if !err.is_empty() {
                                        span { class: "qpf-err", "{err}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // Operations the device *declined* with a well-formed SOAP Fault.
                // Correct device behaviour (a restricted account, an unsupported
                // command) — never oxvif's problem, so never in the red banner.
                if !parse_declined.is_empty() {
                    div { class: "quirk-parse-declined",
                        div { class: "quirk-parse-declined-head",
                            Icon { name: "info", size: 12 }
                            {i18n::t(locale, "quirk_parse_declined_head")
                                .replace("{n}", &parse_declined.len().to_string())}
                        }
                        ul { class: "quirk-parse-fail-list",
                            for (i , (op , err)) in parse_declined.iter().enumerate() {
                                li { key: "{i}",
                                    span { class: "qpf-op", "{op}" }
                                    if !err.is_empty() {
                                        span { class: "qpf-err", "{err}" }
                                    }
                                }
                            }
                        }
                    }
                }

                // What moved since the saved baseline. Above the group list
                // because "did anything change?" is the question a returning
                // tester has; the 52-row list answers "what is wrong today".
                if let Some(d) = baseline_diff.as_ref() {
                    QuirkDiffSection { locale, diff: d.clone() }
                }

                // Nothing wrong anywhere: keep the one-line verdict rather than
                // seven collapsed groups the user has to open to learn that.
                // The "show clean operations" tick still reveals them all.
                if problems == 0 && !*show_clean.read() {
                    div { class: "health-empty", {i18n::t(locale, "quirk_clean")} }
                } else if let Some(blocks) = blocks.as_ref() {
                    div { class: "quirk-groups",
                        for b in blocks.iter() {
                            {
                                let open = (b.problems() > 0) != flipped.read().contains(b.label);
                                let key = b.label;
                                let show_all = *show_clean.read();
                                rsx! {
                                    div { key: "{b.label}", class: "quirk-group",
                                        button {
                                            class: "quirk-group-head",
                                            onclick: move |_| {
                                                let mut f = flipped.write();
                                                if !f.remove(key) {
                                                    f.insert(key);
                                                }
                                            },
                                            span { class: "quirk-caret", {if open { "▾" } else { "▸" }} }
                                            span { class: "quirk-group-name", "{b.label}" }
                                            span { class: "quirk-group-stats", {b.stats(locale)} }
                                        }
                                        if open {
                                            div { class: "quirk-list",
                                                for row in b.rows.iter().filter(|r| show_all || r.problem) {
                                                    QuirkRow {
                                                        key: "{row.key_canon}",
                                                        key_canon: row.key_canon.clone(),
                                                        op: row.op.clone(),
                                                        added_count: row.added,
                                                        removed_count: row.removed,
                                                        parse: row.parse,
                                                        baseline: row.baseline.clone(),
                                                        clone: row.clone_xml.clone(),
                                                        selected,
                                                        expanded,
                                                    }
                                                }
                                                for (op , outcome) in b.skips.iter().filter(|(_, o)| show_all || *o == OpOutcome::Failed) {
                                                    OutcomeRow {
                                                        key: "{op.action_name()}-{outcome:?}",
                                                        op: op.action_name().to_string(),
                                                        outcome: *outcome,
                                                    }
                                                }
                                                if !show_all && b.problems() == 0 {
                                                    div { class: "quirk-group-empty",
                                                        {i18n::t(locale, "quirk_group_all_clean")}
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            } else if served {
                div { class: "health-empty", {i18n::t(locale, "quirk_none_data")} }
            }
        }
    }
}

// ── Live camera test ────────────────────────────────────────────────────────

/// What the run task hands back to the UI task. The result travels down the
/// *same* channel as the progress events so it is processed strictly after the
/// last of them — the bar can never be left frozen mid-sweep by a race.
enum LiveEvent {
    Progress(crate::api::LiveTestProgress),
    Done(Result<(), crate::api::ApiError>),
}

/// The live-test panel: pick the read surface per operation, run the sweep
/// against the real camera, watch it phase by phase.
#[component]
fn LiveTestPanel(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    /// Whether a result already sits behind this panel — i.e. whether closing
    /// it leads anywhere.
    has_result: bool,
    mut run_seq: Signal<u32>,
    mut panel_open: Signal<bool>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut selection = use_signal(|| {
        crate::persist::read_surface_selection().unwrap_or_else(SurfaceSelection::recommended)
    });
    let mut sel_open = use_signal(HashSet::<&'static str>::new);
    let mut running = use_signal(|| false);
    let mut progress = use_signal(|| None::<crate::api::LiveTestProgress>);
    let mut run_error = use_signal(|| None::<String>);

    // Prerequisites the picks drag in across group boundaries (ticking an
    // Imaging op silently drives two Media operations), mapped back to the
    // picks that need them so the UI can say *why* they are on.
    let implied = implied_prereqs(&selection.read());
    let effective = selection.read().len() + implied.len();
    let busy = *running.read();

    let run = move |_| {
        if *running.peek() {
            return;
        }
        let a = addr.peek().clone();
        let sel = selection.peek().clone();
        if a.is_empty() || sel.is_empty() {
            return;
        }
        let creds_v = creds.peek().clone();
        let label = ctx
            .devices
            .peek()
            .iter()
            .find(|d| d.addr == a)
            .map(|d| d.name.clone())
            .unwrap_or_else(|| a.clone());

        running.set(true);
        run_error.set(None);
        progress.set(None);

        // The progress callback is bound `Fn + Send + Sync`, and a default
        // `Signal` is `UnsyncStorage` — not `Sync` — so a closure that writes
        // one straight from the sweep cannot satisfy that bound. An
        // `UnboundedSender` is `Send + Sync`, so the run task only *sends* and
        // this task owns every signal write, draining events in order.
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<LiveEvent>();
        let tx_progress = tx.clone();
        spawn(async move {
            let r = crate::mock_servers::run_live_test(&a, &creds_v, &label, &sel, move |p| {
                let _ = tx_progress.send(LiveEvent::Progress(p));
            })
            .await;
            let _ = tx.send(LiveEvent::Done(r));
        });
        spawn(async move {
            while let Some(ev) = rx.recv().await {
                match ev {
                    LiveEvent::Progress(p) => progress.set(Some(p)),
                    LiveEvent::Done(r) => {
                        // This panel's own signals first: the success arm below
                        // unmounts the panel, and nothing may write a signal
                        // this scope owns after that.
                        progress.set(None);
                        running.set(false);
                        match r {
                            Ok(()) => {
                                ctx.push_toast(
                                    ToastLevel::Success,
                                    i18n::t(locale, "quirk_live_done"),
                                );
                                // The result is in the pool now: fold the panel
                                // away and let the tab re-read it.
                                panel_open.set(false);
                                *run_seq.write() += 1;
                            }
                            Err(e) => {
                                run_error.set(Some(e.clone()));
                                ctx.push_toast(
                                    ToastLevel::Error,
                                    format!("{}: {e}", i18n::t(locale, "quirk_live_failed")),
                                );
                            }
                        }
                    }
                }
            }
        });
    };

    rsx! {
        div { class: "quirk-live",
            div { class: "quirk-live-head",
                button {
                    class: "btn btn-md btn-primary",
                    disabled: busy || effective == 0 || addr.read().is_empty(),
                    onclick: run,
                    Icon { name: "activity", size: 16 }
                    if busy {
                        {i18n::t(locale, "quirk_live_running")}
                    } else {
                        {i18n::t(locale, "quirk_live_run")}
                    }
                }
                span { class: "quirk-live-count",
                    {i18n::t(locale, "quirk_live_count")
                        .replace("{n}", &effective.to_string())
                        .replace("{picked}", &selection.read().len().to_string())
                        .replace("{auto}", &implied.len().to_string())}
                }
                // Only meaningful once a result exists behind this panel.
                if has_result && !busy {
                    button {
                        class: "btn btn-md btn-secondary",
                        onclick: move |_| panel_open.set(false),
                        {i18n::t(locale, "btn_close")}
                    }
                }
            }

            // Without this nobody will dare point the sweep at a working camera.
            div { class: "quirk-live-safe",
                Icon { name: "shield-off", size: 12 }
                {i18n::t(locale, "quirk_live_readonly")}
            }

            if let Some(p) = progress.read().as_ref() {
                LiveProgress { locale, phase: p.phase, done: p.done, total: p.total, label: p.label.clone() }
            }

            if let Some(e) = run_error.read().as_ref() {
                div { class: "quirk-parse-fail",
                    div { class: "quirk-parse-fail-head",
                        Icon { name: "alert-triangle", size: 12 }
                        {i18n::t(locale, "quirk_live_failed")}
                    }
                    span { class: "qpf-err", "{e}" }
                }
            }

            // ── the selection tree ──────────────────────────────────────────
            div { class: "quirk-surface",
                for g in SurfaceGroup::ALL.iter().copied() {
                    {
                        let ops = g.ops();
                        let label = g.label();
                        let picked = ops.iter().filter(|o| selection.read().contains(**o)).count();
                        let auto = ops.iter().filter(|o| implied.contains_key(o)).count();
                        let open = sel_open.read().contains(label);
                        let tri = if picked == 0 {
                            "qs-tri--none"
                        } else if picked == ops.len() {
                            "qs-tri--all"
                        } else {
                            "qs-tri--some"
                        };
                        rsx! {
                            div { key: "{label}", class: "qs-group",
                                div { class: "qs-group-head",
                                    button {
                                        class: "qs-tri {tri}",
                                        disabled: busy,
                                        title: i18n::t(locale, "quirk_sel_group_toggle"),
                                        onclick: move |_| {
                                            {
                                                let mut s = selection.write();
                                                let ops = g.ops();
                                                if ops.iter().all(|o| s.contains(*o)) {
                                                    for o in ops {
                                                        s.remove(o);
                                                    }
                                                } else {
                                                    for o in ops {
                                                        s.insert(o);
                                                    }
                                                }
                                            }
                                            crate::persist::write_surface_selection(&selection.peek());
                                        },
                                    }
                                    button {
                                        class: "qs-group-toggle",
                                        onclick: move |_| {
                                            let mut o = sel_open.write();
                                            if !o.remove(label) {
                                                o.insert(label);
                                            }
                                        },
                                        span { class: "quirk-caret", {if open { "▾" } else { "▸" }} }
                                        span { class: "qs-group-name", "{label}" }
                                        span { class: "qs-group-count", {format!("{picked}/{}", ops.len())} }
                                        if auto > 0 {
                                            span { class: "qs-auto",
                                                {i18n::t(locale, "quirk_sel_auto_n").replace("{n}", &auto.to_string())}
                                            }
                                        }
                                    }
                                }
                                if open {
                                    div { class: "qs-ops",
                                        for op in ops {
                                            {
                                                let is_picked = selection.read().contains(op);
                                                let auto_for = implied.get(&op);
                                                let title = match (auto_for, op.requires()) {
                                                    (Some(by), _) => i18n::t(locale, "quirk_sel_auto_title")
                                                        .replace("{ops}", &join_ops(by)),
                                                    (None, Some(req)) => i18n::t(locale, "quirk_sel_requires")
                                                        .replace("{op}", req.action_name())
                                                        .replace("{group}", req.group().label()),
                                                    (None, None) => String::new(),
                                                };
                                                rsx! {
                                                    label {
                                                        key: "{op:?}",
                                                        class: if auto_for.is_some() { "qs-op qs-op--implied" } else { "qs-op" },
                                                        title,
                                                        input {
                                                            r#type: "checkbox",
                                                            checked: is_picked || auto_for.is_some(),
                                                            // An auto-included prerequisite cannot be
                                                            // turned off here: it is on because another
                                                            // pick needs it. Untick that pick instead.
                                                            disabled: busy || auto_for.is_some(),
                                                            onchange: move |_| {
                                                                {
                                                                    let mut s = selection.write();
                                                                    if s.contains(op) {
                                                                        s.remove(op);
                                                                    } else {
                                                                        s.insert(op);
                                                                    }
                                                                }
                                                                crate::persist::write_surface_selection(&selection.peek());
                                                            },
                                                        }
                                                        span { class: "qs-op-name", {op.action_name()} }
                                                        if auto_for.is_some() {
                                                            span { class: "qs-auto-badge",
                                                                {i18n::t(locale, "quirk_sel_auto")}
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The run's progress bar. Each phase counts a different unit and restarts the
/// bar, so the step counter is shown next to the phase name — a restart reads
/// as "step 3 of 4", not as a stuck bar.
#[component]
fn LiveProgress(
    locale: Locale,
    phase: crate::api::LiveTestPhase,
    done: usize,
    total: Option<usize>,
    label: String,
) -> Element {
    use crate::api::LiveTestPhase as P;
    let (step, name) = match phase {
        P::Connecting => (1, "quirk_phase_connecting"),
        P::Sweep => (2, "quirk_phase_sweep"),
        P::Verifying => (3, "quirk_phase_verifying"),
        P::Diffing => (4, "quirk_phase_diffing"),
    };
    // `Connecting` carries no total — nothing is countable until the first
    // swept operation, so the bar animates instead of lying about a fraction.
    let pct = match total {
        Some(t) if t > 0 => (done * 100 / t).min(100),
        _ => 0,
    };
    rsx! {
        div { class: "quirk-progress",
            div { class: "qp-head",
                span { class: "qp-phase",
                    {i18n::t(locale, "quirk_phase_step")
                        .replace("{step}", &step.to_string())
                        .replace("{phase}", i18n::t(locale, name))}
                }
                span { class: "qp-label", "{label}" }
                span { class: "qp-count",
                    {match total {
                        Some(t) => format!("{done}/{t}"),
                        None => "…".to_string(),
                    }}
                }
            }
            div { class: "qp-bar",
                if total.is_some() {
                    div { class: "qp-fill", style: "width: {pct}%" }
                } else {
                    div { class: "qp-fill qp-fill--indeterminate" }
                }
            }
        }
    }
}

// ── Diff vs saved baseline ──────────────────────────────────────────────────

/// What this device's structural quirks gained, lost or reshaped since the
/// saved baseline — [`oxvif::metamorph::QuirkDiff`].
///
/// Rendered as operation names, not path lists: the question here is *which
/// operations moved*, and the paths for any one of them are a click away in
/// the group list below. `changed` is the subtle case — an operation that
/// drifted before and drifts now, but not in the same places — so it carries
/// the net path counts rather than only the name.
#[component]
fn QuirkDiffSection(locale: Locale, diff: oxvif::metamorph::QuirkDiff) -> Element {
    rsx! {
        div { class: "health-group health-diff",
            div { class: "health-group-title", {i18n::t(locale, "quirk_diff_title")} }
            if diff.is_empty() {
                div { class: "health-row health-pass",
                    span { class: "health-row-status",
                        Icon { name: "check", size: 14 }
                    }
                    span { class: "health-row-detail", {i18n::t(locale, "quirk_diff_none")} }
                }
            } else {
                if !diff.appeared.is_empty() {
                    QuirkDiffRow {
                        icon: "plus",
                        cls: "health-fail",
                        label: i18n::t(locale, "quirk_diff_appeared").to_string(),
                        ops: diff.appeared.iter().map(|q| op_name(&q.action)).collect::<Vec<_>>().join(", "),
                    }
                }
                if !diff.resolved.is_empty() {
                    QuirkDiffRow {
                        icon: "check",
                        cls: "health-pass",
                        label: i18n::t(locale, "quirk_diff_resolved").to_string(),
                        ops: diff.resolved.iter().map(|q| op_name(&q.action)).collect::<Vec<_>>().join(", "),
                    }
                }
                for (i , c) in diff.changed.iter().enumerate() {
                    div { key: "{i}", class: "health-row health-warn",
                        span { class: "health-row-status",
                            Icon { name: "alert-triangle", size: 14 }
                        }
                        span { class: "health-row-name", {op_name(&c.action)} }
                        span { class: "health-row-detail",
                            {format!(
                                "{}: +{} / -{}",
                                i18n::t(locale, "quirk_diff_changed"),
                                c.clone_only_added.len() + c.synthetic_only_added.len(),
                                c.clone_only_removed.len() + c.synthetic_only_removed.len(),
                            )}
                        }
                    }
                }
            }
        }
    }
}

#[component]
fn QuirkDiffRow(icon: &'static str, cls: &'static str, label: String, ops: String) -> Element {
    rsx! {
        div { class: "health-row {cls}",
            span { class: "health-row-status",
                Icon { name: icon, size: 14 }
            }
            span { class: "health-row-name", "{label}" }
            span { class: "health-row-detail", "{ops}" }
        }
    }
}

// ── Result rows ─────────────────────────────────────────────────────────────

/// One recorded operation in the result list.
struct ResultRow {
    key_canon: String,
    op: String,
    added: usize,
    removed: usize,
    parse: Option<ParseStatus>,
    baseline: String,
    clone_xml: String,
    /// Structural drift or a parse failure — what the default view shows.
    /// A device-declined ([`ParseStatus::Faulted`]) response is not a problem.
    problem: bool,
}

/// One service zone's worth of results.
struct GroupBlock {
    label: &'static str,
    rows: Vec<ResultRow>,
    /// Selected operations that produced no fixture, with the reason.
    skips: Vec<(SurfaceOp, OpOutcome)>,
}

impl GroupBlock {
    fn is_empty(&self) -> bool {
        self.rows.is_empty() && self.skips.is_empty()
    }

    /// Rows worth the user's attention: drift / parse failure, plus operations
    /// the command itself broke on. Skips are never counted here — a camera
    /// with no PTZ is not a camera with eight problems.
    fn problems(&self) -> usize {
        self.rows.iter().filter(|r| r.problem).count()
            + self
                .skips
                .iter()
                .filter(|(_, o)| *o == OpOutcome::Failed)
                .count()
    }

    fn stats(&self, locale: Locale) -> String {
        let problems = self.problems();
        i18n::t(locale, "quirk_group_stats")
            .replace("{issues}", &problems.to_string())
            .replace(
                "{clean}",
                &self.rows.iter().filter(|r| !r.problem).count().to_string(),
            )
            .replace(
                "{skipped}",
                &self
                    .skips
                    .iter()
                    .filter(|(_, o)| o.is_skipped())
                    .count()
                    .to_string(),
            )
    }
}

/// Bucket every recorded operation and every non-recorded sweep outcome into
/// its service zone, in `SurfaceGroup::ALL` order. Anything whose action maps to
/// no zone (`GetCapabilities`, which the session build issues and no
/// `SurfaceOp` covers) lands in a trailing "other" block.
fn group_results(
    details: Vec<OperationDiff>,
    quirks: &[OperationQuirk],
    verdicts: &HashMap<String, ParseStatus>,
    outcomes: Vec<(SurfaceOp, OpOutcome)>,
    other_label: &'static str,
) -> Vec<GroupBlock> {
    let quirk_by_key: HashMap<&str, &OperationQuirk> = quirks
        .iter()
        .map(|q| (q.key_canon.as_str(), q))
        .collect::<HashMap<_, _>>();

    let mut blocks: Vec<GroupBlock> = SurfaceGroup::ALL
        .iter()
        .map(|g| GroupBlock {
            label: g.label(),
            rows: Vec::new(),
            skips: Vec::new(),
        })
        .chain(std::iter::once(GroupBlock {
            label: other_label,
            rows: Vec::new(),
            skips: Vec::new(),
        }))
        .collect();
    let other = blocks.len() - 1;
    let index_of = |g: Option<SurfaceGroup>| {
        g.and_then(|g| SurfaceGroup::ALL.iter().position(|x| *x == g))
            .unwrap_or(other)
    };

    for d in details {
        let q = quirk_by_key.get(d.key_canon.as_str());
        let parse = verdicts.get(&d.key_canon).copied();
        blocks[index_of(group_of_action(&d.action))]
            .rows
            .push(ResultRow {
                op: op_name(&d.action).to_string(),
                added: q.map(|q| q.only_in_clone.len()).unwrap_or(0),
                removed: q.map(|q| q.only_in_synthetic.len()).unwrap_or(0),
                problem: q.is_some() || parse == Some(ParseStatus::Failed),
                key_canon: d.key_canon,
                parse,
                baseline: d.baseline_xml,
                clone_xml: d.clone_xml,
            });
    }

    // Recorded operations already have a row above; the rest are the honest
    // half of the sweep report — broke, or genuinely not present on this camera.
    for (op, outcome) in outcomes {
        if outcome != OpOutcome::Recorded {
            blocks[index_of(Some(op.group()))].skips.push((op, outcome));
        }
    }

    blocks.retain(|b| !b.is_empty());
    blocks
}

/// One operation row: a select checkbox and an expandable header revealing a
/// side-by-side (baseline | clone) line diff of the two SOAP responses.
#[component]
fn QuirkRow(
    key_canon: String,
    op: String,
    added_count: usize,
    removed_count: usize,
    parse: Option<ParseStatus>,
    baseline: String,
    clone: String,
    mut selected: Signal<HashSet<String>>,
    mut expanded: Signal<HashSet<String>>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let is_sel = selected.read().contains(&key_canon);
    let is_exp = expanded.read().contains(&key_canon);
    let k_sel = key_canon.clone();
    let k_exp = key_canon.clone();

    rsx! {
        div { class: "quirk-item",
            div { class: "quirk-row",
                input {
                    r#type: "checkbox",
                    checked: is_sel,
                    onchange: move |_| {
                        let mut s = selected.write();
                        if !s.remove(&k_sel) {
                            s.insert(k_sel.clone());
                        }
                    },
                }
                button {
                    class: "quirk-op-toggle",
                    onclick: move |_| {
                        let mut e = expanded.write();
                        if !e.remove(&k_exp) {
                            e.insert(k_exp.clone());
                        }
                    },
                    span { class: "quirk-caret", {if is_exp { "▾" } else { "▸" }} }
                    span { class: "quirk-op", "{op}" }
                    match parse {
                        Some(ParseStatus::Parsed) => rsx! {
                            span { class: "quirk-badge qb-ok", {i18n::t(locale, "quirk_parse_ok")} }
                        },
                        Some(ParseStatus::Failed) => rsx! {
                            span { class: "quirk-badge qb-fail", {i18n::t(locale, "quirk_parse_bad")} }
                        },
                        // The device declined this one with a SOAP Fault — it
                        // said no, correctly. Not an oxvif parse problem, so
                        // deliberately not the red treatment.
                        Some(ParseStatus::Faulted) => rsx! {
                            span { class: "quirk-badge qb-declined", {i18n::t(locale, "quirk_parse_declined")} }
                        },
                        _ => rsx! {},
                    }
                    span { class: "quirk-count", {format!("+{added_count} \u{2212}{removed_count}")} }
                }
            }
            if is_exp {
                div { class: "quirk-diff",
                    div { class: "qd-row qd-head",
                        div { class: "qd-cell", {i18n::t(locale, "quirk_baseline")} }
                        div { class: "qd-cell", {i18n::t(locale, "quirk_clone")} }
                    }
                    for (i, row) in line_diff(&baseline, &clone).iter().enumerate() {
                        div { key: "{i}", class: "qd-row",
                            match row {
                                DiffRow::Equal(s) => rsx! {
                                    div { class: "qd-cell qd-eq", "{s}" }
                                    div { class: "qd-cell qd-eq", "{s}" }
                                },
                                DiffRow::Left(s) => rsx! {
                                    div { class: "qd-cell qd-del", "{s}" }
                                    div { class: "qd-cell qd-blank" }
                                },
                                DiffRow::Right(s) => rsx! {
                                    div { class: "qd-cell qd-blank" }
                                    div { class: "qd-cell qd-add", "{s}" }
                                },
                                DiffRow::Changed { left, right } => rsx! {
                                    div { class: "qd-cell qd-chg",
                                        for (k, seg) in left.iter().enumerate() {
                                            span {
                                                key: "{k}",
                                                class: if seg.changed { "qd-seg-del" } else { "" },
                                                "{seg.text}"
                                            }
                                        }
                                    }
                                    div { class: "qd-cell qd-chg",
                                        for (k, seg) in right.iter().enumerate() {
                                            span {
                                                key: "{k}",
                                                class: if seg.changed { "qd-seg-add" } else { "" },
                                                "{seg.text}"
                                            }
                                        }
                                    }
                                },
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A selected operation that produced no recorded response, with the reason.
/// The three reasons are not interchangeable: only `Failed` is a fault.
#[component]
fn OutcomeRow(op: String, outcome: OpOutcome) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let (cls, icon, badge, note) = match outcome {
        OpOutcome::Failed => (
            "qo-failed",
            "x",
            "quirk_out_failed",
            "quirk_out_failed_note",
        ),
        // The camera answered the token list and it was empty — a fixed camera
        // with no PTZ profiles. Muted, never red.
        OpOutcome::SkippedNoData => (
            "qo-nodata",
            "minus",
            "quirk_out_nodata",
            "quirk_out_nodata_note",
        ),
        OpOutcome::SkippedPrerequisite => (
            "qo-prereq",
            "minus",
            "quirk_out_prereq",
            "quirk_out_prereq_note",
        ),
        _ => ("qo-nodata", "check", "quirk_out_nodata", ""),
    };
    rsx! {
        div { class: "quirk-item quirk-outcome {cls}",
            div { class: "quirk-row",
                span { class: "quirk-outcome-icon", Icon { name: icon, size: 12 } }
                span { class: "quirk-op", "{op}" }
                span { class: "quirk-badge", {i18n::t(locale, badge)} }
                if !note.is_empty() {
                    span { class: "quirk-outcome-note", {i18n::t(locale, note)} }
                }
            }
        }
    }
}

// ── Helpers ─────────────────────────────────────────────────────────────────

/// Last path segment of a SOAP action URI — the operation name.
fn op_name(action: &str) -> &str {
    action.rsplit('/').next().unwrap_or(action)
}

/// A parse verdict as `(operation, error)` for the banner lists.
fn verdict_line(v: &oxvif::metamorph::ParseVerdict) -> (String, String) {
    (
        op_name(&v.action).to_string(),
        v.error.clone().unwrap_or_default(),
    )
}

/// The service zone a recorded exchange belongs to, from its SOAP action URI
/// (`http://www.onvif.org/ver10/media/wsdl/GetProfiles`). The namespace decides
/// the zone — the operation name alone cannot, since PTZ and Imaging both have
/// a `GetStatus` and Media1/Media2 share most names. Identity and Network share
/// the device service, so there the name breaks the tie.
fn group_of_action(action: &str) -> Option<SurfaceGroup> {
    if action.contains("/ptz/wsdl/") {
        Some(SurfaceGroup::Ptz)
    } else if action.contains("/imaging/wsdl/") {
        Some(SurfaceGroup::Imaging)
    } else if action.contains("/events/wsdl/") {
        Some(SurfaceGroup::Events)
    } else if action.contains("/ver20/media/wsdl/") {
        Some(SurfaceGroup::Media2)
    } else if action.contains("/ver10/media/wsdl/") {
        Some(SurfaceGroup::Media)
    } else if action.contains("/device/wsdl/") {
        let name = op_name(action);
        if SurfaceGroup::Network
            .ops()
            .iter()
            .any(|o| o.action_name() == name)
        {
            Some(SurfaceGroup::Network)
        } else {
            Some(SurfaceGroup::Identity)
        }
    } else {
        None
    }
}

/// Prerequisites the sweep will run that the user did **not** tick, mapped to
/// the picks that dragged them in.
///
/// These cross group boundaries — `GetPtzPresets` (PTZ) needs `GetProfiles`
/// (Media), the Imaging operations need `GetVideoSources` (Media) — so ticking
/// one operation can silently light up two in another zone. The UI shows them
/// in their own state, and they are what makes the effective count (and the
/// sweep's progress `total`) larger than the number of ticks.
fn implied_prereqs(selection: &SurfaceSelection) -> HashMap<SurfaceOp, Vec<SurfaceOp>> {
    let mut map: HashMap<SurfaceOp, Vec<SurfaceOp>> = HashMap::new();
    for op in selection.iter() {
        // The chain is acyclic (a token source is always a top-level list read),
        // and depth-1 in practice.
        let mut cur = op;
        while let Some(req) = cur.requires() {
            if !selection.contains(req) {
                let by = map.entry(req).or_default();
                if !by.contains(&op) {
                    by.push(op);
                }
            }
            cur = req;
        }
    }
    for by in map.values_mut() {
        by.sort();
    }
    map
}

/// Operation names for a tooltip, e.g. `GetPresets, GetStatus`.
fn join_ops(ops: &[SurfaceOp]) -> String {
    ops.iter()
        .map(|o| o.action_name())
        .collect::<Vec<_>>()
        .join(", ")
}

// These helpers are private to this module, so their unit tests live inline
// rather than in `src/tests/` (which can only reach `pub` items).
#[cfg(test)]
mod tests {
    use super::*;

    fn diff(action: &str, key: &str) -> OperationDiff {
        OperationDiff {
            action: action.to_string(),
            key_canon: key.to_string(),
            baseline_xml: String::new(),
            clone_xml: String::new(),
            differs: false,
        }
    }

    use oxvif::metamorph::{OperationQuirk, QuirkReport};

    fn quirk(action: &str, clone_only: &[&str], synth_only: &[&str]) -> OperationQuirk {
        OperationQuirk {
            action: action.to_string(),
            key_canon: format!("<{}/>", op_name(action)),
            only_in_clone: clone_only.iter().map(|s| s.to_string()).collect(),
            only_in_synthetic: synth_only.iter().map(|s| s.to_string()).collect(),
        }
    }

    fn report(quirks: Vec<OperationQuirk>) -> QuirkReport {
        QuirkReport {
            device: "cam-4417".to_string(),
            compared: 9,
            quirks,
        }
    }

    /// `write_quirk_baseline` stores `to_json_pretty()`; `read_quirk_baseline`
    /// parses with `serde_json::from_str::<QuirkReport>`. If those two ever
    /// disagree the Quirks tab shows no diff at all and says nothing — the read
    /// logs "stale baseline ignored" and returns `None`. Pin the pair.
    #[test]
    fn a_saved_quirk_baseline_parses_back_field_for_field() {
        let saved = report(vec![quirk(
            "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
            &["Envelope/Body/GetProfilesResponse/Profiles/Extension"],
            &["Envelope/Body/GetProfilesResponse/Profiles/Name"],
        )]);

        let loaded: QuirkReport = serde_json::from_str(&saved.to_json_pretty())
            .expect("what write_quirk_baseline writes is what read_quirk_baseline parses");

        assert_eq!(loaded.device, "cam-4417");
        assert_eq!(loaded.compared, 9);
        // OperationQuirk is PartialEq as of oxvif 0.14, so this compares the
        // action, the key and both path lists — not just the length.
        assert_eq!(loaded.quirks, saved.quirks);
    }

    /// The diff section renders `appeared` as a failure and `resolved` as a
    /// pass, so which bucket an operation lands in is load-bearing for what the
    /// user is told. `diff` is called as `now.diff(&baseline)`; this pins that
    /// argument order too — swapping it inverts both labels.
    #[test]
    fn diff_buckets_are_oriented_now_against_baseline() {
        let baseline = report(vec![
            quirk("svc/GetGone", &["Envelope/Body/A"], &[]),
            quirk("svc/GetShifted", &["Envelope/Body/B"], &[]),
        ]);
        let now = report(vec![
            quirk("svc/GetShifted", &["Envelope/Body/C"], &[]),
            quirk("svc/GetNew", &["Envelope/Body/D"], &[]),
        ]);

        let d = now.diff(&baseline);

        assert_eq!(
            d.appeared
                .iter()
                .map(|q| op_name(&q.action))
                .collect::<Vec<_>>(),
            ["GetNew"],
            "quirky now but not in the baseline"
        );
        assert_eq!(
            d.resolved
                .iter()
                .map(|q| op_name(&q.action))
                .collect::<Vec<_>>(),
            ["GetGone"],
            "quirky in the baseline but not now"
        );
        assert_eq!(
            d.changed
                .iter()
                .map(|c| op_name(&c.action))
                .collect::<Vec<_>>(),
            ["GetShifted"],
            "quirky in both, but the paths moved"
        );

        // The counts the row renders: one path in, one path out.
        let c = &d.changed[0];
        assert_eq!(c.clone_only_added, ["Envelope/Body/C"]);
        assert_eq!(c.clone_only_removed, ["Envelope/Body/B"]);

        assert!(!d.is_empty(), "this diff must not render as 'no change'");
    }

    /// The `is_empty` branch is what tells a returning tester "nothing moved",
    /// which is the whole point of keeping a baseline.
    #[test]
    fn an_unchanged_device_diffs_to_nothing() {
        let quirks = vec![quirk("svc/GetSame", &["Envelope/Body/A"], &[])];
        let d = report(quirks.clone()).diff(&report(quirks));
        assert!(d.is_empty(), "same quirks, same paths: {d:?}");
    }

    /// A prerequisite pulled in from *another* group is what makes the effective
    /// count exceed the tick count — the whole reason the UI shows
    /// auto-included operations in their own state.
    #[test]
    fn prerequisites_cross_group_boundaries() {
        // One Imaging tick drives GetVideoSources, which lives in Media.
        let sel = SurfaceSelection::none().with(SurfaceOp::GetImagingSettings);
        let implied = implied_prereqs(&sel);
        assert_eq!(implied.len(), 1);
        assert_eq!(
            implied.get(&SurfaceOp::GetVideoSources).map(Vec::as_slice),
            Some([SurfaceOp::GetImagingSettings].as_slice())
        );
        assert_eq!(
            SurfaceOp::GetVideoSources.group(),
            SurfaceGroup::Media,
            "the parent is in a different group from the tick"
        );
        // Two PTZ ticks share one Media prerequisite: counted once, and it
        // names both requirers.
        let sel = SurfaceSelection::none()
            .with(SurfaceOp::GetPtzPresets)
            .with(SurfaceOp::GetPtzStatus);
        let implied = implied_prereqs(&sel);
        assert_eq!(implied.len(), 1, "GetProfiles is one operation, not two");
        assert_eq!(
            implied.get(&SurfaceOp::GetProfiles).map(Vec::len),
            Some(2),
            "both picks are named as requirers"
        );
        // 2 ticks + 1 implied — exactly the sweep's progress total.
        assert_eq!(sel.len() + implied.len(), 3);
    }

    /// An explicitly ticked prerequisite is not also reported as implied, or it
    /// would be double-counted.
    #[test]
    fn an_explicit_pick_is_never_implied() {
        let sel = SurfaceSelection::none()
            .with(SurfaceOp::GetStreamUri)
            .with(SurfaceOp::GetProfiles);
        assert!(implied_prereqs(&sel).is_empty());
        assert!(implied_prereqs(&SurfaceSelection::recommended()).is_empty());
    }

    /// The operation name alone cannot place a row: PTZ and Imaging both have a
    /// `GetStatus`, Media1 and Media2 share most names, and Identity/Network
    /// share the device service.
    #[test]
    fn action_uri_maps_to_its_service_zone() {
        let cases = [
            (
                "http://www.onvif.org/ver20/ptz/wsdl/GetStatus",
                SurfaceGroup::Ptz,
            ),
            (
                "http://www.onvif.org/ver20/imaging/wsdl/GetStatus",
                SurfaceGroup::Imaging,
            ),
            (
                "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
                SurfaceGroup::Media,
            ),
            (
                "http://www.onvif.org/ver20/media/wsdl/GetProfiles",
                SurfaceGroup::Media2,
            ),
            (
                "http://www.onvif.org/ver10/events/wsdl/GetEventProperties",
                SurfaceGroup::Events,
            ),
            (
                "http://www.onvif.org/ver10/device/wsdl/GetHostname",
                SurfaceGroup::Identity,
            ),
            (
                "http://www.onvif.org/ver10/device/wsdl/GetDNS",
                SurfaceGroup::Network,
            ),
        ];
        for (action, want) in cases {
            assert_eq!(group_of_action(action), Some(want), "{action}");
        }
        // GetCapabilities is no SurfaceOp, but it is still a device-service
        // read, so it lands in Identity rather than the "other" block.
        assert_eq!(
            group_of_action("http://www.onvif.org/ver10/device/wsdl/GetCapabilities"),
            Some(SurfaceGroup::Identity),
        );
        assert_eq!(group_of_action("urn:vendor:GetSomething"), None);
    }

    /// Skips are not failures: a fixed camera with no PTZ profiles must not
    /// read as a camera with problems.
    #[test]
    fn only_failures_count_as_problems() {
        let blocks = group_results(
            vec![diff(
                "http://www.onvif.org/ver10/media/wsdl/GetProfiles",
                "k1",
            )],
            &[],
            &HashMap::new(),
            vec![
                (SurfaceOp::GetPtzPresets, OpOutcome::SkippedNoData),
                (SurfaceOp::GetPtzStatus, OpOutcome::SkippedPrerequisite),
                (SurfaceOp::GetPtzNodes, OpOutcome::Failed),
                (SurfaceOp::GetProfiles, OpOutcome::Recorded),
            ],
            "Other",
        );
        let media = blocks
            .iter()
            .find(|b| b.label == SurfaceGroup::Media.label())
            .expect("the recorded read is bucketed");
        assert_eq!(media.rows.len(), 1);
        assert_eq!(media.problems(), 0, "a clean row is not a problem");
        assert!(
            media.skips.is_empty(),
            "a Recorded outcome already has a row"
        );

        let ptz = blocks
            .iter()
            .find(|b| b.label == SurfaceGroup::Ptz.label())
            .expect("outcomes are bucketed by their operation's group");
        assert_eq!(ptz.skips.len(), 3);
        assert_eq!(ptz.problems(), 1, "only the Failed outcome is a problem");
    }

    /// A parse failure is a problem even with no structural drift; a device
    /// declining with a SOAP Fault is not.
    #[test]
    fn a_declined_operation_is_not_a_problem() {
        let verdicts = HashMap::from([
            ("k1".to_string(), ParseStatus::Failed),
            ("k2".to_string(), ParseStatus::Faulted),
            ("k3".to_string(), ParseStatus::Parsed),
        ]);
        let action = "http://www.onvif.org/ver10/device/wsdl/GetHostname";
        let blocks = group_results(
            vec![diff(action, "k1"), diff(action, "k2"), diff(action, "k3")],
            &[],
            &verdicts,
            Vec::new(),
            "Other",
        );
        let identity = &blocks[0];
        assert_eq!(identity.label, SurfaceGroup::Identity.label());
        assert_eq!(identity.problems(), 1, "only the Failed verdict");
    }
}
