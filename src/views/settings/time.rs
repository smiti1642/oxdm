#![allow(non_snake_case)]
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;

#[component]
pub fn TimeTab(addr: ReadSignal<String>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let info = use_resource(move || {
        let addr = addr.read().clone();
        let creds = ctx.global_credentials.read().clone();
        async move {
            let (user, pass) = if creds.username.is_empty() {
                (None, None)
            } else {
                (Some(creds.username.as_str()), Some(creds.password.as_str()))
            };
            api::get_system_date_and_time(&addr, user, pass).await
        }
    });

    rsx! {
        match &*info.read_unchecked() {
            None => rsx! {
                div { class: "tab-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(e)) => rsx! {
                div { class: "tab-error", "{e}" }
            },
            Some(Ok(dt)) => {
                let utc_str = dt.utc_unix
                    .map(format_unix_timestamp)
                    .unwrap_or_else(|| "N/A".to_string());
                let dst = if dt.daylight_savings {
                    i18n::t(locale, "yes")
                } else {
                    i18n::t(locale, "no")
                };
                rsx! {
                    table { class: "prop-table",
                        PropRow { label: i18n::t(locale, "prop_utc_time"),   value: utc_str }
                        PropRow { label: i18n::t(locale, "prop_timezone"),   value: dt.timezone.clone() }
                        PropRow { label: i18n::t(locale, "prop_dst"),        value: dst.to_string() }
                    }
                }
            },
        }
    }
}

#[component]
fn PropRow(label: &'static str, value: String) -> Element {
    rsx! {
        tr {
            td { class: "prop-label", "{label}" }
            td { class: "prop-value", "{value}" }
        }
    }
}

fn format_unix_timestamp(ts: i64) -> String {
    let secs = ts % 60;
    let mins = (ts / 60) % 60;
    let hours = (ts / 3600) % 24;
    let days = ts / 86400;
    let (y, m, d) = epoch_days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02} {hours:02}:{mins:02}:{secs:02} UTC")
}

pub fn epoch_days_to_ymd(mut days: i64) -> (i64, i64, i64) {
    days += 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d)
}
