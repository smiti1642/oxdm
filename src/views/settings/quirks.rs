#![allow(non_snake_case)]
use std::collections::{HashMap, HashSet};

use crate::components::Icon;
use crate::i18n;
use crate::state::{Ctx, ToastLevel};
use crate::util::{line_diff, DiffRow};
use dioxus::prelude::*;
use oxvif::metamorph::ParseStatus;

/// Quirks tab — shown only for a served clone. Lists the operations whose
/// response shape drifts from oxvif's synthetic baseline; each row expands into
/// a git-style **left/right** line diff of the two SOAP responses (baseline vs
/// camera), and the checked rows export to JSON.
#[component]
pub fn QuirkTab(addr: ReadSignal<String>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut selected = use_signal(HashSet::<String>::new);
    let expanded = use_signal(HashSet::<String>::new);

    // Parse verification (value/type layer) runs oxvif's own parser over the
    // recorded responses — async, so it loads via a resource and is joined to
    // the structural quirks below by `key_canon`.
    let parse = use_resource(move || {
        let url = addr.read().clone();
        async move { crate::mock_servers::parse_report(&url).await }
    });

    // Cheap pool lookups each render (no PartialEq on the report → no use_memo).
    let Some(rep) = crate::mock_servers::quirks(&addr.read()) else {
        return rsx! {
            div { class: "health-view",
                div { class: "health-empty", {i18n::t(locale, "quirk_none_data")} }
            }
        };
    };
    let details = crate::mock_servers::details(&addr.read()).unwrap_or_default();

    // Verdict per operation (for the row badge) and the failure list (ops oxvif
    // can't parse — including structurally-clean ones the SOAP diff can't see).
    let (verdicts, parse_failures): (HashMap<String, ParseStatus>, Vec<(String, String)>) =
        match &*parse.read_unchecked() {
            Some(Some(pr)) => (
                pr.verdicts
                    .iter()
                    .map(|v| (v.key_canon.clone(), v.status))
                    .collect(),
                pr.failures()
                    .map(|v| {
                        (
                            op_name(&v.action).to_string(),
                            v.error.clone().unwrap_or_default(),
                        )
                    })
                    .collect(),
            ),
            _ => (HashMap::new(), Vec::new()),
        };

    let all_keys: Vec<String> = rep.quirks.iter().map(|q| q.key_canon.clone()).collect();
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

    rsx! {
        div { class: "health-view",
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
                button {
                    class: "btn btn-md btn-secondary",
                    disabled: sel_count == 0,
                    onclick: export,
                    Icon { name: "download", size: 14 }
                    {i18n::t(locale, "quirk_export").replace("{n}", &sel_count.to_string())}
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

            if rep.quirks.is_empty() {
                div { class: "health-empty", {i18n::t(locale, "quirk_clean")} }
            } else {
                div { class: "quirk-list",
                    for q in rep.quirks.iter() {
                        {
                            let d = details.iter().find(|d| d.key_canon == q.key_canon);
                            let (baseline, clone) = d
                                .map(|d| (d.baseline_xml.clone(), d.clone_xml.clone()))
                                .unwrap_or_default();
                            rsx! {
                                QuirkRow {
                                    key: "{q.key_canon}",
                                    key_canon: q.key_canon.clone(),
                                    op: op_name(&q.action).to_string(),
                                    added_count: q.only_in_clone.len(),
                                    removed_count: q.only_in_synthetic.len(),
                                    parse: verdicts.get(&q.key_canon).copied(),
                                    baseline,
                                    clone,
                                    selected,
                                    expanded,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
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

/// Last path segment of a SOAP action URI — the operation name.
fn op_name(action: &str) -> &str {
    action.rsplit('/').next().unwrap_or(action)
}
