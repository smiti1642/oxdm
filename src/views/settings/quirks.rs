#![allow(non_snake_case)]
use crate::i18n;
use crate::state::Ctx;
use dioxus::prelude::*;

/// Quirks tab — shown only for a served clone. Renders the structural quirk diff
/// (clone response shapes vs oxvif's synthetic baseline) that `mock_servers`
/// computed when the clone was served. Structure only: which element paths the
/// real camera adds (`+`) or omits (`−`) versus the spec-ideal mock.
#[component]
pub fn QuirkTab(addr: ReadSignal<String>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    // Cheap pool lookup + clone; recomputed each render (no PartialEq on the
    // report, so `use_memo` doesn't apply).
    let report = crate::mock_servers::quirks(&addr.read());

    rsx! {
        div { class: "health-view",
            match report.as_ref() {
                None => rsx! {
                    div { class: "health-empty", {i18n::t(locale, "quirk_none_data")} }
                },
                Some(rep) => rsx! {
                    div { class: "health-header",
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
                                div { key: "{q.key_canon}", class: "quirk-item",
                                    div { class: "quirk-op", {op_name(&q.action)} }
                                    if !q.only_in_clone.is_empty() {
                                        div { class: "quirk-added",
                                            {format!("+ {}", q.only_in_clone.join(", "))}
                                        }
                                    }
                                    if !q.only_in_synthetic.is_empty() {
                                        div { class: "quirk-removed",
                                            {format!("\u{2212} {}", q.only_in_synthetic.join(", "))}
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

/// Last path segment of a SOAP action URI — the operation name.
fn op_name(action: &str) -> &str {
    action.rsplit('/').next().unwrap_or(action)
}
