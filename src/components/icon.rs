#![allow(non_snake_case)]
use dioxus::prelude::*;

/// Render an inline SVG icon by name. Uses Lucide icon paths (MIT licensed).
/// Default size is 16px; pass `size` to override.
#[component]
pub fn Icon(name: &'static str, #[props(default = 16)] size: u32) -> Element {
    // Some icons need multiple paths/elements — handled via match returning full SVG children
    let paths = icon_paths(name);

    rsx! {
        svg {
            class: "icon",
            width: "{size}",
            height: "{size}",
            view_box: "0 0 24 24",
            fill: "none",
            stroke: "currentColor",
            stroke_width: "2",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            {paths}
        }
    }
}

fn icon_paths(name: &str) -> Element {
    match name {
        "search" => rsx! {
            circle { cx: "11", cy: "11", r: "8" }
            path { d: "m21 21-4.3-4.3" }
        },
        "key" => rsx! {
            path { d: "M2.586 17.414A2 2 0 0 0 2 18.828V21a1 1 0 0 0 1 1h3a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h1a1 1 0 0 0 1-1v-1a1 1 0 0 1 1-1h.172a2 2 0 0 0 1.414-.586l.814-.814a6.5 6.5 0 1 0-4-4z" }
            circle { cx: "16.5", cy: "7.5", r: ".5", fill: "currentColor" }
        },
        "sun" => rsx! {
            circle { cx: "12", cy: "12", r: "4" }
            path { d: "M12 2v2" }
            path { d: "M12 20v2" }
            path { d: "m4.93 4.93 1.41 1.41" }
            path { d: "m17.66 17.66 1.41 1.41" }
            path { d: "M2 12h2" }
            path { d: "M20 12h2" }
            path { d: "m6.34 17.66-1.41 1.41" }
            path { d: "m19.07 4.93-1.41 1.41" }
        },
        "moon" => rsx! {
            path { d: "M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" }
        },
        "monitor" => rsx! {
            rect { width: "20", height: "14", x: "2", y: "3", rx: "2" }
            path { d: "M8 21h8" }
            path { d: "M12 17v4" }
        },
        "help-circle" => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M9.09 9a3 3 0 0 1 5.83 1c0 2-3 3-3 3" }
            path { d: "M12 17h.01" }
        },
        "camera" => rsx! {
            path { d: "M14.5 4h-5L7 7H4a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V9a2 2 0 0 0-2-2h-3l-2.5-3z" }
            circle { cx: "12", cy: "13", r: "3" }
        },
        "bell" => rsx! {
            path { d: "M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" }
            path { d: "M10.3 21a1.94 1.94 0 0 0 3.4 0" }
        },
        "play" => rsx! {
            polygon { points: "6 3 20 12 6 21 6 3" }
        },
        "video" => rsx! {
            path { d: "m16 13 5.223 3.482a.5.5 0 0 0 .777-.416V7.934a.5.5 0 0 0-.777-.416L16 11" }
            rect { x: "2", y: "6", width: "14", height: "12", rx: "2" }
        },
        "sliders" => rsx! {
            path { d: "M4 21v-7" }
            path { d: "M4 10V3" }
            path { d: "M12 21v-9" }
            path { d: "M12 8V3" }
            path { d: "M20 21v-5" }
            path { d: "M20 12V3" }
            path { d: "M2 14h4" }
            path { d: "M10 8h4" }
            path { d: "M18 16h4" }
        },
        "crosshair" => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M22 12h-4" }
            path { d: "M6 12H2" }
            path { d: "M12 6V2" }
            path { d: "M12 22v-4" }
        },
        "wrench" => rsx! {
            path { d: "M14.7 6.3a1 1 0 0 0 0 1.4l1.6 1.6a1 1 0 0 0 1.4 0l3.77-3.77a6 6 0 0 1-7.94 7.94l-6.91 6.91a2.12 2.12 0 0 1-3-3l6.91-6.91a6 6 0 0 1 7.94-7.94l-3.76 3.76z" }
        },
        "settings" => rsx! {
            path { d: "M12.22 2h-.44a2 2 0 0 0-2 2v.18a2 2 0 0 1-1 1.73l-.43.25a2 2 0 0 1-2 0l-.15-.08a2 2 0 0 0-2.73.73l-.22.38a2 2 0 0 0 .73 2.73l.15.1a2 2 0 0 1 1 1.72v.51a2 2 0 0 1-1 1.74l-.15.09a2 2 0 0 0-.73 2.73l.22.38a2 2 0 0 0 2.73.73l.15-.08a2 2 0 0 1 2 0l.43.25a2 2 0 0 1 1 1.73V20a2 2 0 0 0 2 2h.44a2 2 0 0 0 2-2v-.18a2 2 0 0 1 1-1.73l.43-.25a2 2 0 0 1 2 0l.15.08a2 2 0 0 0 2.73-.73l.22-.39a2 2 0 0 0-.73-2.73l-.15-.08a2 2 0 0 1-1-1.74v-.5a2 2 0 0 1 1-1.74l.15-.09a2 2 0 0 0 .73-2.73l-.22-.38a2 2 0 0 0-2.73-.73l-.15.08a2 2 0 0 1-2 0l-.43-.25a2 2 0 0 1-1-1.73V4a2 2 0 0 0-2-2z" }
            circle { cx: "12", cy: "12", r: "3" }
        },
        "globe" => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 2a14.5 14.5 0 0 0 0 20 14.5 14.5 0 0 0 0-20" }
            path { d: "M2 12h20" }
        },
        "clock" => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 6v6l4 2" }
        },
        "users" => rsx! {
            path { d: "M16 21v-2a4 4 0 0 0-4-4H6a4 4 0 0 0-4 4v2" }
            circle { cx: "9", cy: "7", r: "4" }
            path { d: "M22 21v-2a4 4 0 0 0-3-3.87" }
            path { d: "M16 3.13a4 4 0 0 1 0 7.75" }
        },
        "info" => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 16v-4" }
            path { d: "M12 8h.01" }
        },
        "check" => rsx! {
            path { d: "M20 6 9 17l-5-5" }
        },
        "x" => rsx! {
            path { d: "M18 6 6 18" }
            path { d: "m6 6 12 12" }
        },
        "alert-triangle" => rsx! {
            path { d: "m21.73 18-8-14a2 2 0 0 0-3.48 0l-8 14A2 2 0 0 0 4 21h16a2 2 0 0 0 1.73-3" }
            path { d: "M12 9v4" }
            path { d: "M12 17h.01" }
        },
        "rotate-cw" => rsx! {
            path { d: "M21 12a9 9 0 1 1-9-9c2.52 0 4.93 1 6.74 2.74L21 8" }
            path { d: "M21 3v5h-5" }
        },
        "shield-off" => rsx! {
            path { d: "m2 2 20 20" }
            path { d: "M5 5a1 1 0 0 0-1 1v7c0 5 3.5 7.5 8 8.5a14.6 14.6 0 0 0 4-1.33" }
            path { d: "M9.41 9.41A5.5 5.5 0 0 1 12 8.5c.94 0 1.82.24 2.59.66" }
            path { d: "M20 13V6a1 1 0 0 0-1-1h-1" }
        },
        "eye" => rsx! {
            path { d: "M2.062 12.348a1 1 0 0 1 0-.696 10.75 10.75 0 0 1 19.876 0 1 1 0 0 1 0 .696 10.75 10.75 0 0 1-19.876 0" }
            circle { cx: "12", cy: "12", r: "3" }
        },
        "eye-off" => rsx! {
            path { d: "M10.733 5.076a10.744 10.744 0 0 1 11.205 6.575 1 1 0 0 1 0 .696 10.747 10.747 0 0 1-1.444 2.49" }
            path { d: "M14.084 14.158a3 3 0 0 1-4.242-4.242" }
            path { d: "M17.479 17.499a10.75 10.75 0 0 1-15.417-5.151 1 1 0 0 1 0-.696 10.75 10.75 0 0 1 4.446-5.143" }
            path { d: "m2 2 20 20" }
        },
        "pencil" => rsx! {
            path { d: "M21.174 6.812a1 1 0 0 0-3.986-3.987L3.842 16.174a2 2 0 0 0-.5.83l-1.321 4.352a.5.5 0 0 0 .623.622l4.353-1.32a2 2 0 0 0 .83-.497z" }
            path { d: "m15 5 4 4" }
        },
        "clipboard-copy" => rsx! {
            rect { width: "8", height: "4", x: "8", y: "2", rx: "1", ry: "1" }
            path { d: "M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2" }
        },
        "trash-2" => rsx! {
            path { d: "M3 6h18" }
            path { d: "M19 6v14c0 1-1 2-2 2H7c-1 0-2-1-2-2V6" }
            path { d: "M8 6V4c0-1 1-2 2-2h4c1 0 2 1 2 2v2" }
            path { d: "M10 11v6" }
            path { d: "M14 11v6" }
        },
        "plus" => rsx! {
            path { d: "M5 12h14" }
            path { d: "M12 5v14" }
        },
        "refresh-cw" => rsx! {
            path { d: "M3 12a9 9 0 0 1 9-9 9.75 9.75 0 0 1 6.74 2.74L21 8" }
            path { d: "M21 3v5h-5" }
            path { d: "M21 12a9 9 0 0 1-9 9 9.75 9.75 0 0 1-6.74-2.74L3 16" }
            path { d: "M8 16H3v5" }
        },
        "chevron-right" => rsx! {
            path { d: "m9 18 6-6-6-6" }
        },
        "chevron-down" => rsx! {
            path { d: "m6 9 6 6 6-6" }
        },
        "hexagon" => rsx! {
            path { d: "M21 16V8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" }
        },
        "arrow-up" => rsx! {
            path { d: "m5 12 7-7 7 7" }
            path { d: "M12 19V5" }
        },
        "arrow-down" => rsx! {
            path { d: "M12 5v14" }
            path { d: "m19 12-7 7-7-7" }
        },
        "arrow-left" => rsx! {
            path { d: "m12 19-7-7 7-7" }
            path { d: "M19 12H5" }
        },
        "arrow-right" => rsx! {
            path { d: "M5 12h14" }
            path { d: "m12 5 7 7-7 7" }
        },
        "arrow-up-left" => rsx! {
            path { d: "M7 17V7h10" }
            path { d: "m7 7 10 10" }
        },
        "arrow-up-right" => rsx! {
            path { d: "M7 7h10v10" }
            path { d: "m7 17 10-10" }
        },
        "arrow-down-left" => rsx! {
            path { d: "M17 7 7 17" }
            path { d: "M17 17H7V7" }
        },
        "arrow-down-right" => rsx! {
            path { d: "m7 7 10 10" }
            path { d: "M17 7v10H7" }
        },
        "square" => rsx! {
            rect { width: "18", height: "18", x: "3", y: "3", rx: "2" }
        },
        "minus" => rsx! {
            path { d: "M5 12h14" }
        },
        "home" => rsx! {
            path { d: "m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" }
            path { d: "M9 22V12h6v10" }
        },
        "bookmark" => rsx! {
            path { d: "m19 21-7-4-7 4V5a2 2 0 0 1 2-2h10a2 2 0 0 1 2 2v16z" }
        },
        "navigation-2" => rsx! {
            path { d: "M12 2 19 21l-7-4-7 4z" }
        },
        "download" => rsx! {
            path { d: "M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4" }
            polyline { points: "7 10 12 15 17 10" }
            line { x1: "12", x2: "12", y1: "15", y2: "3" }
        },
        _ => rsx! {
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M12 16v-4" }
            path { d: "M12 8h.01" }
        },
    }
}
