#![allow(non_snake_case)]
use std::collections::HashSet;

use crate::components::Icon;
use crate::i18n;
use crate::state::{Ctx, ToastLevel};
use dioxus::prelude::*;

/// Quirks tab — shown only for a served clone. Renders the structural quirk diff
/// (which response element paths the real camera adds `+` or omits `−` versus
/// oxvif's synthetic baseline) as an expandable, selectable list, with export of
/// the checked operations to JSON. Structure only, Body shape (the SOAP Header
/// is excluded upstream).
#[component]
pub fn QuirkTab(addr: ReadSignal<String>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let mut selected = use_signal(HashSet::<String>::new);
    let expanded = use_signal(HashSet::<String>::new);

    // Cheap pool lookup + clone each render (no PartialEq on the report, so
    // `use_memo` doesn't apply).
    let Some(rep) = crate::mock_servers::quirks(&addr.read()) else {
        return rsx! {
            div { class: "health-view",
                div { class: "health-empty", {i18n::t(locale, "quirk_none_data")} }
            }
        };
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
        spawn(async move {
            let Some(handle) = rfd::AsyncFileDialog::new()
                .set_file_name("oxdm-quirks.json")
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

            if rep.quirks.is_empty() {
                div { class: "health-empty", {i18n::t(locale, "quirk_clean")} }
            } else {
                div { class: "quirk-list",
                    for q in rep.quirks.iter() {
                        QuirkRow {
                            key: "{q.key_canon}",
                            key_canon: q.key_canon.clone(),
                            op: op_name(&q.action).to_string(),
                            added: q.only_in_clone.clone(),
                            removed: q.only_in_synthetic.clone(),
                            selected,
                            expanded,
                        }
                    }
                }
            }
        }
    }
}

/// One operation row: a select checkbox and an expandable header revealing the
/// added / removed element paths.
#[component]
fn QuirkRow(
    key_canon: String,
    op: String,
    added: Vec<String>,
    removed: Vec<String>,
    mut selected: Signal<HashSet<String>>,
    mut expanded: Signal<HashSet<String>>,
) -> Element {
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
                    span { class: "quirk-count", {format!("+{} −{}", added.len(), removed.len())} }
                }
            }
            if is_exp {
                div { class: "quirk-detail",
                    for p in added.iter() {
                        div { class: "quirk-added", {format!("+ {p}")} }
                    }
                    for p in removed.iter() {
                        div { class: "quirk-removed", {format!("\u{2212} {p}")} }
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
