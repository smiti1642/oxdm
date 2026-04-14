pub fn get(key: &str) -> Option<&'static str> {
    Some(match key {
        // ── Topbar ──────────────────────────────────────────────────────────
        "search_placeholder" => "\u{641C}\u{5C0B}\u{88DD}\u{7F6E}\u{2026}",
        "tooltip_settings" => "\u{8A2D}\u{5B9A}",
        "tooltip_theme" => "\u{4E3B}\u{984C}",
        "tooltip_language" => "\u{8A9E}\u{8A00}",
        "tooltip_help" => "\u{8AAA}\u{660E}",

        // ── Sidebar ─────────────────────────────────────────────────────────
        "sidebar_title" => "\u{88DD}\u{7F6E}",
        "filter_placeholder" => "\u{540D}\u{7A31}\u{6216}\u{4F4D}\u{5740}\u{2026}",
        "no_devices" => "\u{672A}\u{627E}\u{5230}\u{88DD}\u{7F6E}\u{3002}\n\u{9EDE}\u{64CA}\u{300C}\u{6383}\u{63CF}\u{300D}\u{4EE5}\u{63A2}\u{7D22}\u{3002}",
        "no_matches" => "\u{7121}\u{7B26}\u{5408}\u{7D50}\u{679C}\u{3002}",
        "btn_scan" => "\u{27F3}  \u{6383}\u{63CF}",
        "btn_scanning" => "\u{6383}\u{63CF}\u{4E2D}\u{2026}",
        "btn_add" => "\u{FF0B} \u{65B0}\u{589E}",

        // ── Device Panel ────────────────────────────────────────────────────
        "select_device" => "\u{2190} \u{9078}\u{64C7}\u{88DD}\u{7F6E}",
        "section_general" => "\u{4E00}\u{822C}",
        "nav_settings" => "\u{8A2D}\u{5B9A}",
        "nav_events" => "\u{4E8B}\u{4EF6}",
        "section_nvt" => "NVT",
        "live_preview" => "\u{25B6}  \u{5373}\u{6642}\u{9810}\u{89BD}",
        "nav_live_video" => "\u{5373}\u{6642}\u{5F71}\u{50CF}",
        "nav_imaging" => "\u{5F71}\u{50CF}\u{8A2D}\u{5B9A}",
        "nav_ptz" => "PTZ \u{63A7}\u{5236}",

        // ── Main Content ────────────────────────────────────────────────────
        "app_name" => "OxDM",
        "app_subtitle" => "ONVIF \u{88DD}\u{7F6E}\u{7BA1}\u{7406}\u{5668}",
        "welcome_hint" => "\u{5F9E}\u{5DE6}\u{5074}\u{9762}\u{677F}\u{9078}\u{64C7}\u{88DD}\u{7F6E}\u{FF0C}\n\u{6216}\u{9EDE}\u{64CA}  \u{27F3} \u{6383}\u{63CF}  \u{4EE5}\u{63A2}\u{7D22}\u{7DB2}\u{8DEF}\u{4E0A}\u{7684}\u{88DD}\u{7F6E}\u{3002}",
        "coming_soon" => "\u{5373}\u{5C07}\u{63A8}\u{51FA}",

        // ── Settings Tabs ───────────────────────────────────────────────────
        "tab_identification" => "\u{88DD}\u{7F6E}\u{8B58}\u{5225}",
        "tab_network" => "\u{7DB2}\u{8DEF}",
        "tab_time" => "\u{6642}\u{9593}",
        "tab_users" => "\u{4F7F}\u{7528}\u{8005}",
        "tab_maintenance" => "\u{7DAD}\u{8B77}",

        // ── Status Bar ──────────────────────────────────────────────────────
        "status_devices" => "\u{53F0}\u{88DD}\u{7F6E}",
        "status_device" => "\u{53F0}\u{88DD}\u{7F6E}",
        "status_online" => "\u{53F0}\u{5728}\u{7DDA}",
        "status_scanning" => "\u{6383}\u{63CF}\u{4E2D}\u{2026}",
        "status_ws_discovery" => "WS-Discovery",

        // ── Theme names ─────────────────────────────────────────────────────
        "theme_dark" => "\u{6DF1}\u{8272}",
        "theme_light" => "\u{6DFA}\u{8272}",
        "theme_classic" => "\u{7D93}\u{5178} (ODM)",

        // ── Buttons (shared) ────────────────────────────────────────────────
        "btn_cancel" => "\u{53D6}\u{6D88}",
        "btn_save" => "\u{5132}\u{5B58}",
        "btn_confirm" => "\u{78BA}\u{8A8D}",
        "btn_add_short" => "\u{65B0}\u{589E}",

        // ── Credentials ─────────────────────────────────────────────────────
        "cred_global_title" => "\u{5168}\u{57DF}\u{6191}\u{8B49}",
        "cred_global_hint" => "\u{6383}\u{63CF}\u{5230}\u{7684}\u{88DD}\u{7F6E}\u{5747}\u{4F7F}\u{7528}\u{6B64}\u{5E33}\u{5BC6}\u{3002}",
        "cred_username" => "\u{5E33}\u{865F}",
        "cred_password" => "\u{5BC6}\u{78BC}",

        // ── Add Device ──────────────────────────────────────────────────────
        "add_device_title" => "\u{65B0}\u{589E}\u{88DD}\u{7F6E}",
        "add_device_addr" => "\u{88DD}\u{7F6E}\u{4F4D}\u{5740} (ONVIF URL)",
        "add_device_name" => "\u{986F}\u{793A}\u{540D}\u{7A31}",
        "add_device_name_hint" => "\u{9078}\u{586B}",
        "add_device_custom_creds" => "\u{81EA}\u{8A02}\u{6191}\u{8B49}",

        _ => return None,
    })
}
