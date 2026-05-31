#![allow(non_snake_case)]
use crate::components::{DialogOverlay, Icon};
use crate::i18n;
use crate::state::Ctx;
use dioxus::prelude::*;
use std::path::{Path, PathBuf};

/// Cap on lines shown so a multi-megabyte daily log doesn't lock up the
/// webview. We read the whole file but only ever render the tail.
const TAIL_LINES: usize = 800;

/// In-app log viewer modal. Tails the newest `~/.oxdm/logs/oxdm.log.*`
/// file (the daily-rolling appender's output), with a substring filter
/// and a manual refresh. Read-only — the "Open folder" affordance for
/// the raw files lives in the About dialog.
///
/// The early return before any hook is deliberate (same pattern as
/// `AboutDialog`): while closed the hooks aren't created, so each reopen
/// re-runs the file read and shows fresh tail without a manual refresh.
#[component]
pub fn LogViewer(open: Signal<bool>) -> Element {
    if !*open.read() {
        return rsx! {};
    }
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let mut reload = use_signal(|| 0u32);
    let mut filter = use_signal(String::new);

    let logs = use_resource(move || {
        // Read the counter so bumping it re-runs the fetch.
        let _ = reload();
        async move {
            let dir = crate::log_dir().ok_or("logs_no_dir")?;
            let file = newest_log_file(&dir).ok_or("logs_empty")?;
            let content = std::fs::read_to_string(&file).map_err(|_| "logs_read_failed")?;
            Ok::<_, &'static str>((file.display().to_string(), tail_lines(&content, TAIL_LINES)))
        }
    });

    let needle = filter.read().to_lowercase();

    rsx! {
        DialogOverlay {
            on_close: {
                let mut open = open;
                move |_| open.set(false)
            },
            inner_class: "dialog log-viewer".to_string(),

            div { class: "dialog-header",
                span { class: "dialog-title", {i18n::t(locale, "logs_title")} }
                input {
                    class: "log-viewer-filter",
                    r#type: "text",
                    placeholder: i18n::t(locale, "logs_filter_placeholder"),
                    value: "{filter}",
                    oninput: move |e| filter.set(e.value()),
                }
                button {
                    class: "icon-btn",
                    title: i18n::t(locale, "logs_refresh"),
                    onclick: move |_| reload += 1,
                    Icon { name: "refresh-cw", size: 16 }
                }
            }
            div { class: "dialog-body log-viewer-body",
                match &*logs.read_unchecked() {
                    None => rsx! {
                        div { class: "log-viewer-empty", {i18n::t(locale, "loading")} }
                    },
                    Some(Err(key)) => rsx! {
                        div { class: "log-viewer-empty",
                            Icon { name: "file-text", size: 28 }
                            p { {i18n::t(locale, key)} }
                        }
                    },
                    Some(Ok((path, lines))) => {
                        let shown: Vec<&String> = lines
                            .iter()
                            .filter(|l| needle.is_empty() || l.to_lowercase().contains(&needle))
                            .collect();
                        rsx! {
                            div { class: "log-viewer-path", code { "{path}" } }
                            if shown.is_empty() {
                                div { class: "log-viewer-empty",
                                    p { {i18n::t(locale, "logs_no_match")} }
                                }
                            } else {
                                pre { class: "log-viewer-pre",
                                    for line in shown {
                                        "{line}\n"
                                    }
                                }
                            }
                        }
                    }
                }
            }
            div { class: "dialog-footer",
                button {
                    class: "btn btn-md btn-primary",
                    onclick: {
                        let mut open = open;
                        move |_| open.set(false)
                    },
                    {i18n::t(locale, "btn_close")}
                }
            }
        }
    }
}

/// Pick the most recent rolling-log file in `dir`. The appender names
/// files `oxdm.log.YYYY-MM-DD`, so the lexicographically-largest name is
/// the newest day.
fn newest_log_file(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("oxdm.log"))
        })
        .max_by_key(|p| p.file_name().map(|n| n.to_os_string()))
}

/// Keep only the last `max` lines of `content`.
fn tail_lines(content: &str, max: usize) -> Vec<String> {
    let all: Vec<&str> = content.lines().collect();
    let start = all.len().saturating_sub(max);
    all[start..].iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tail_returns_all_when_under_cap() {
        let out = tail_lines("a\nb\nc", 10);
        assert_eq!(out, vec!["a", "b", "c"]);
    }

    #[test]
    fn tail_keeps_only_last_lines() {
        let content = (0..100)
            .map(|i| i.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let out = tail_lines(&content, 3);
        assert_eq!(out, vec!["97", "98", "99"]);
    }

    #[test]
    fn tail_handles_empty() {
        assert!(tail_lines("", 5).is_empty());
    }
}
