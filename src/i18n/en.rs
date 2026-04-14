pub fn get(key: &str) -> Option<&'static str> {
    Some(match key {
        // ── Topbar ──────────────────────────────────────────────────────────
        "search_placeholder" => "Search devices\u{2026}",
        "tooltip_settings" => "Settings",
        "tooltip_theme" => "Theme",
        "tooltip_language" => "Language",
        "tooltip_help" => "Help",

        // ── Sidebar ─────────────────────────────────────────────────────────
        "sidebar_title" => "Devices",
        "filter_placeholder" => "Name or address\u{2026}",
        "no_devices" => "No devices found.\nClick Scan to discover.",
        "no_matches" => "No matches.",
        "btn_scan" => "\u{27F3}  Scan",
        "btn_scanning" => "Scanning\u{2026}",
        "btn_add" => "\u{FF0B} Add",

        // ── Device Panel ────────────────────────────────────────────────────
        "select_device" => "\u{2190} Select a device",
        "section_general" => "General",
        "nav_settings" => "Settings",
        "nav_events" => "Events",
        "section_nvt" => "NVT",
        "live_preview" => "\u{25B6}  Live preview",
        "nav_live_video" => "Live video",
        "nav_imaging" => "Imaging settings",
        "nav_ptz" => "PTZ control",

        // ── Main Content ────────────────────────────────────────────────────
        "app_name" => "OxDM",
        "app_subtitle" => "ONVIF Device Manager",
        "welcome_hint" => "Select a device from the left panel,\nor click  \u{27F3} Scan  to discover devices on the network.",
        "coming_soon" => "Coming soon",

        // ── Settings Tabs ───────────────────────────────────────────────────
        "tab_identification" => "Identification",
        "tab_network" => "Network",
        "tab_time" => "Time",
        "tab_users" => "Users",
        "tab_maintenance" => "Maintenance",

        // ── Status Bar ──────────────────────────────────────────────────────
        "status_devices" => "devices",
        "status_device" => "device",
        "status_online" => "online",
        "status_scanning" => "Scanning\u{2026}",
        "status_ws_discovery" => "WS-Discovery",

        // ── Theme names ─────────────────────────────────────────────────────
        "theme_dark" => "Dark",
        "theme_light" => "Light",
        "theme_classic" => "Classic (ODM)",

        // ── Buttons (shared) ────────────────────────────────────────────────
        "btn_cancel" => "Cancel",
        "btn_save" => "Save",
        "btn_confirm" => "Confirm",
        "btn_add_short" => "Add",

        // ── Credentials ─────────────────────────────────────────────────────
        "cred_global_title" => "Global Credentials",
        "cred_global_hint" => "Default username and password used for all discovered devices.",
        "cred_username" => "Username",
        "cred_password" => "Password",

        // ── Add Device ──────────────────────────────────────────────────────
        "add_device_title" => "Add Device",
        "add_device_addr" => "Device address (ONVIF URL)",
        "add_device_name" => "Display name",
        "add_device_name_hint" => "Optional",
        "add_device_custom_creds" => "Custom credentials",

        _ => return None,
    })
}
