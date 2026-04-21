#![allow(non_snake_case)]
use crate::components::{Icon, PropRow};
use crate::state::{Credentials, ToastLevel};
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

/// Sentinel value used as the `<option value>` for the "Custom…" entry
/// in the timezone dropdown. Cannot collide with any valid POSIX TZ
/// string because POSIX forbids bare `__`.
const CUSTOM_SENTINEL: &str = "__custom__";

/// Curated UTC-offset entries for the timezone dropdown. Format is
/// `(display_label, POSIX_TZ_string)`. POSIX TZ inverts the sign of the
/// offset (positive = west of UTC), so UTC+8 → `"UTC-8"`.
///
/// DST-observing zones (US/EU/etc.) ship as the no-DST offset entry plus
/// the user-controlled DST checkbox below — keeps the list short and
/// avoids the complex `M3.2.0,M11.1.0` rule strings most users never
/// touch correctly. Cameras that need rule-based DST can fall through to
/// the optional custom override field.
const COMMON_TIMEZONES: &[(&str, &str)] = &[
    ("UTC-12:00", "UTC12"),
    ("UTC-11:00", "UTC11"),
    ("UTC-10:00", "UTC10"),
    ("UTC-09:00", "UTC9"),
    ("UTC-08:00", "UTC8"),
    ("UTC-07:00", "UTC7"),
    ("UTC-06:00", "UTC6"),
    ("UTC-05:00", "UTC5"),
    ("UTC-04:00", "UTC4"),
    ("UTC-03:00", "UTC3"),
    ("UTC-02:00", "UTC2"),
    ("UTC-01:00", "UTC1"),
    ("UTC+00:00", "UTC0"),
    ("UTC+01:00", "UTC-1"),
    ("UTC+02:00", "UTC-2"),
    ("UTC+03:00", "UTC-3"),
    ("UTC+04:00", "UTC-4"),
    ("UTC+05:00", "UTC-5"),
    ("UTC+05:30", "UTC-5:30"),
    ("UTC+06:00", "UTC-6"),
    ("UTC+07:00", "UTC-7"),
    ("UTC+08:00", "CST-8"),
    ("UTC+09:00", "JST-9"),
    ("UTC+09:30", "UTC-9:30"),
    ("UTC+10:00", "UTC-10"),
    ("UTC+11:00", "UTC-11"),
    ("UTC+12:00", "UTC-12"),
];

#[component]
pub fn TimeTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let mut info = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move {
            let (u, p) = creds.as_options();
            api::get_system_date_and_time(&addr, u, p).await
        }
    });

    // Editable buffers seeded from the fetched state on first read.
    // `tz_in` holds the POSIX string that will actually be sent.
    // `selected_preset` drives the dropdown — equals tz_in for presets,
    // `CUSTOM_SENTINEL` when the user chose "Custom…" so the text input
    // below becomes active.
    let tz_in = use_signal(String::new);
    let selected_preset = use_signal(String::new);
    let dst_in = use_signal(|| false);
    let initialized = use_signal(|| false);

    // `(utc_unix, snapshot_instant)` — the camera's reported UTC and the
    // host-side instant at which we captured it. Ticking clocks subtract
    // `snapshot_instant.elapsed()` to project forward. Updated every time
    // `info` delivers a fresh utc_unix (initial fetch, Sync-from-PC
    // restart, device switch, manual `info.restart()`).
    let last_snapshot = use_signal(|| (0i64, Instant::now()));

    // One-second tick to drive the live clock displays. Subscribing to
    // this signal anywhere in the render forces that part to re-evaluate
    // every second.
    let mut tick = use_signal(|| 0u64);
    use_future(move || async move {
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            // Read + drop the borrow before writing — otherwise the two
            // ref-cell accesses overlap and the borrow checker complains.
            let next = tick.peek().wrapping_add(1);
            tick.set(next);
        }
    });

    // Helper to push a SetSystemDateAndTime given the current form values
    // plus a chosen mode + optional manual UTC time.
    let apply = move |datetime_type: &'static str, manual_utc: Option<oxvif::UtcDateTime>| {
        let addr_s = addr.read().clone();
        let creds_s = creds.read().clone();
        let tz = tz_in.peek().clone();
        let dst = *dst_in.peek();
        spawn(async move {
            let (u, p) = creds_s.as_options();
            let req = oxvif::SetDateTimeRequest {
                datetime_type: datetime_type.to_string(),
                daylight_savings: dst,
                timezone: tz,
                utc_datetime: manual_utc,
            };
            match api::set_system_date_and_time(&addr_s, u, p, &req).await {
                Ok(()) => {
                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "time_saved"));
                    info.restart();
                }
                Err(e) => {
                    ctx.push_toast(ToastLevel::Error, e);
                }
            }
        });
    };

    rsx! {
        match &*info.read_unchecked() {
            None => rsx! {
                div { class: "tab-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(e)) => rsx! {
                div { class: "tab-error", "{e}" }
            },
            Some(Ok(dt)) => {
                if !*initialized.peek() {
                    tz_in.clone().set(dt.timezone.clone());
                    // If device's TZ matches a preset, preselect it;
                    // otherwise show the camera's raw value as the
                    // first option and let the user change from there.
                    selected_preset.clone().set(dt.timezone.clone());
                    dst_in.clone().set(dt.daylight_savings);
                    initialized.clone().set(true);
                }

                // Whenever the camera reports a new utc_unix (fresh fetch
                // after Sync-from-PC, initial load, device switch, etc.),
                // snapshot it alongside the current host-side Instant.
                // The ticking clocks below project forward from this.
                if let Some(current_utc) = dt.utc_unix {
                    if last_snapshot.peek().0 != current_utc {
                        last_snapshot.clone().set((current_utc, Instant::now()));
                    }
                }

                // Subscribe this render to the 1-second tick so the
                // displayed clocks update.
                let _ = *tick.read();

                let (snap_utc, snap_at) = *last_snapshot.read();
                let elapsed_secs = snap_at.elapsed().as_secs() as i64;
                let effective_utc = snap_utc + elapsed_secs;

                let utc_str = if snap_utc == 0 {
                    "N/A".to_string()
                } else {
                    format_unix_timestamp(effective_utc)
                };

                // Local time: projected UTC shifted by the offset implied
                // by the currently-selected preset. Skipped for Custom
                // (arbitrary POSIX rule strings aren't worth parsing here)
                // and for any preset whose offset we can't cleanly extract.
                let preset_val = selected_preset.read().clone();
                let local_str = if preset_val == CUSTOM_SENTINEL || snap_utc == 0 {
                    "\u{2014}".to_string()
                } else {
                    match parse_posix_offset_seconds(&preset_val) {
                        Some(off) => format_offset_timestamp(effective_utc, off),
                        None => "\u{2014}".to_string(),
                    }
                };

                let dst_label = if dt.daylight_savings {
                    i18n::t(locale, "yes")
                } else {
                    i18n::t(locale, "no")
                };

                rsx! {
                    table { class: "prop-table",
                        PropRow { label: i18n::t(locale, "prop_utc_time"),   value: utc_str }
                        PropRow { label: i18n::t(locale, "prop_local_time"), value: local_str }
                        PropRow { label: i18n::t(locale, "prop_timezone"),   value: dt.timezone.clone() }
                        PropRow { label: i18n::t(locale, "prop_dst"),        value: dst_label.to_string() }
                    }

                    // ── Quick sync section ────────────────────────────
                    // Dedicated section for the one-click "match PC"
                    // workflow. Separated from the timezone-editor below
                    // so users don't accidentally interpret Sync-from-PC
                    // as needing the dropdown above to be set first.
                    div { class: "prop-section-header", {i18n::t(locale, "time_sync_section")} }
                    div { class: "id-edit-form",
                        div { class: "id-edit-actions",
                            button {
                                class: "btn btn-md btn-primary",
                                title: i18n::t(locale, "time_sync_pc_hint"),
                                onclick: move |_| {
                                    let pc_tz = detect_local_posix_tz();
                                    tz_in.clone().set(pc_tz.clone());
                                    selected_preset.clone().set(pc_tz);
                                    apply("Manual", Some(pc_utc_now()));
                                },
                                Icon { name: "refresh-cw", size: 14 }
                                " "
                                {i18n::t(locale, "time_sync_pc")}
                            }
                        }
                    }

                    // ── Manual timezone section ──────────────────────
                    div { class: "prop-section-header", {i18n::t(locale, "time_edit_section")} }
                    div { class: "id-edit-form",
                        div { class: "id-edit-row",
                            label { class: "id-edit-label", {i18n::t(locale, "prop_timezone")} }
                            {
                                let current_preset = selected_preset.read().clone();
                                let in_list = COMMON_TIMEZONES.iter().any(|(_, p)| *p == current_preset);
                                let show_custom_original = !in_list && !current_preset.is_empty()
                                    && current_preset != CUSTOM_SENTINEL;
                                rsx! {
                                    select {
                                        class: "id-edit-input",
                                        value: "{current_preset}",
                                        onchange: move |e| {
                                            let v = e.value();
                                            selected_preset.clone().set(v.clone());
                                            // For a concrete preset, mirror
                                            // to tz_in immediately. For
                                            // Custom, leave tz_in alone so
                                            // the user's typed value survives.
                                            if v != CUSTOM_SENTINEL {
                                                tz_in.clone().set(v);
                                            }
                                        },
                                        // Order: Custom first (most
                                        // flexible, handles exotic POSIX
                                        // with DST rules), then camera's
                                        // raw value if it doesn't match a
                                        // preset, then the curated presets.
                                        option {
                                            value: "{CUSTOM_SENTINEL}",
                                            selected: current_preset == CUSTOM_SENTINEL,
                                            {i18n::t(locale, "tz_custom")}
                                        }
                                        if show_custom_original {
                                            option {
                                                value: "{current_preset}",
                                                selected: true,
                                                {i18n::t(locale, "tz_current_prefix").to_string() + &current_preset}
                                            }
                                        }
                                        for (label, posix) in COMMON_TIMEZONES {
                                            option {
                                                value: "{posix}",
                                                selected: *posix == current_preset,
                                                "{label}"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // Custom POSIX TZ input — only rendered when user
                        // picks "Custom…" from the dropdown above.
                        if *selected_preset.read() == CUSTOM_SENTINEL {
                            div { class: "id-edit-row",
                                label { class: "id-edit-label", {i18n::t(locale, "tz_custom_label")} }
                                input {
                                    class: "id-edit-input",
                                    r#type: "text",
                                    placeholder: "EST5EDT,M3.2.0,M11.1.0",
                                    value: "{*tz_in.read()}",
                                    oninput: move |e| tz_in.clone().set(e.value()),
                                }
                            }
                        }
                        div { class: "id-edit-row",
                            label { class: "id-edit-label", {i18n::t(locale, "prop_dst")} }
                            input {
                                r#type: "checkbox",
                                checked: "{*dst_in.read()}",
                                oninput: move |e| {
                                    // Dioxus sends "true"/"false" strings
                                    dst_in.clone().set(e.value() == "true");
                                },
                            }
                        }
                        div { class: "id-edit-actions",
                            button {
                                class: "btn btn-md btn-primary",
                                title: i18n::t(locale, "time_apply_tz_hint"),
                                onclick: move |_| apply("Manual", None),
                                Icon { name: "check", size: 14 }
                                " "
                                {i18n::t(locale, "time_apply_tz")}
                            }
                        }
                    }
                }
            },
        }
    }
}

/// Detect the host machine's current local UTC offset and format it as a
/// POSIX TZ string. Example: Taipei (UTC+8) → `"UTC-8"` (POSIX inverts the
/// sign — east of UTC is negative). Falls back to `"UTC0"` on platforms
/// where the `time` crate can't read the local offset (unusual).
///
/// DST is not encoded: the fixed offset captured here is whatever the
/// machine considers current, so "sync from PC" after DST changes works
/// by re-running it. Users who need POSIX rule strings (`EST5EDT,M3.2.0,M11.1.0`)
/// can pick "Custom…" in the dropdown.
fn detect_local_posix_tz() -> String {
    let offset = time::OffsetDateTime::now_local()
        .map(|dt| dt.offset())
        .unwrap_or(time::UtcOffset::UTC);
    let (h, m, _s) = offset.as_hms();
    // POSIX: east of UTC → negative sign in string; west → positive.
    let posix_hours = -i32::from(h);
    let posix_minutes = (-i32::from(m)).abs();
    match (posix_hours, posix_minutes) {
        (0, 0) => "UTC0".to_string(),
        (_, 0) => format!("UTC{posix_hours}"),
        (_, _) => format!("UTC{posix_hours}:{posix_minutes:02}"),
    }
}

/// Capture the host machine's current UTC wall time for use as the camera's
/// new clock value. System clock drift between host and camera is normal
/// (cameras have poor RTCs) so "sync from PC" is the one-click fix.
fn pc_utc_now() -> oxvif::UtcDateTime {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs / 86400;
    let (y, mo, d) = epoch_days_to_ymd(days);
    let remainder = secs - days * 86400;
    let hour = (remainder / 3600) as u8;
    let minute = ((remainder % 3600) / 60) as u8;
    let second = (remainder % 60) as u8;
    oxvif::UtcDateTime {
        year: y as u16,
        month: mo as u8,
        day: d as u8,
        hour,
        minute,
        second,
    }
}

/// Parse the fixed UTC offset out of a POSIX TZ prefix.
///
/// Recognises `XYZ0`, `XYZ-8`, `XYZ+5:30`, `EST5EDT,…`, etc. Ignores any
/// DST rule trailer. Returns the "real" offset in seconds, positive for
/// east of UTC — this is the inverse of the POSIX sign convention, so the
/// caller can just add this number to a UTC unix timestamp to get local.
/// Returns `None` when the string has no numeric offset we can pull out.
fn parse_posix_offset_seconds(tz: &str) -> Option<i64> {
    // Skip the zone abbreviation (letters) to where the offset starts.
    let idx = tz
        .bytes()
        .position(|b| b == b'-' || b == b'+' || b.is_ascii_digit())?;
    let rest = &tz[idx..];
    let bytes = rest.as_bytes();
    let (sign, digits_start) = match bytes.first() {
        Some(b'-') => (-1i64, 1),
        Some(b'+') => (1i64, 1),
        _ => (1i64, 0),
    };
    let digits = &rest[digits_start..];
    // Stop at first non-digit / non-colon — filters DST trailer like ",M3.2.0".
    let offset_end = digits
        .bytes()
        .position(|b| !(b.is_ascii_digit() || b == b':'))
        .unwrap_or(digits.len());
    let offset_part = &digits[..offset_end];
    let mut parts = offset_part.split(':');
    let h: i64 = parts.next()?.parse().ok()?;
    let m: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let s: i64 = parts.next().and_then(|s| s.parse().ok()).unwrap_or(0);
    let posix_offset = sign * (h * 3600 + m * 60 + s);
    // POSIX sign is inverted: `CST-8` means +8h east → real = -posix.
    Some(-posix_offset)
}

/// Format a unix UTC timestamp shifted by `offset_secs` to a wall-clock
/// string. Uses `rem_euclid` / `div_euclid` so dates before the epoch
/// format correctly (even though camera clocks that far back are very
/// unlikely).
fn format_offset_timestamp(unix_utc: i64, offset_secs: i64) -> String {
    let adjusted = unix_utc + offset_secs;
    let day_secs = adjusted.rem_euclid(86400);
    let secs = day_secs % 60;
    let mins = (day_secs / 60) % 60;
    let hours = day_secs / 3600;
    let days = adjusted.div_euclid(86400);
    let (y, m, d) = epoch_days_to_ymd(days);
    format!("{y:04}-{m:02}-{d:02} {hours:02}:{mins:02}:{secs:02}")
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
