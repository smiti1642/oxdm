pub fn get(key: &str) -> Option<&'static str> {
    Some(match key {
        // ── Topbar ──────────────────────────────────────────────────────────
        "search_placeholder" => "Search devices\u{2026}",
        "tooltip_credentials" => "Credentials",
        "tooltip_theme" => "Theme",
        "tooltip_language" => "Language",
        "tooltip_help" => "Help",

        // ── Sidebar ─────────────────────────────────────────────────────────
        "sidebar_title" => "Devices",
        "filter_placeholder" => "Name or address\u{2026}",
        "no_devices" => "No devices found.\nClick Scan to discover.",
        "no_manual_devices" => "No manual devices.\nClick Add to create one.",
        "no_matches" => "No matches.",
        "devtab_discovered" => "Discovered",
        "devtab_manual" => "Manual",
        "btn_scan_label" => "Scan",
        "btn_scanning" => "Scanning\u{2026}",
        "btn_add_label" => "Add",

        // ── Device Panel ────────────────────────────────────────────────────
        "select_device" => "\u{2190} Select a device",
        "section_general" => "General",
        "nav_settings" => "Settings",
        "nav_events" => "Events",
        "section_nvt" => "NVT",
        "section_streams" => "Streams",
        "no_streams" => "No streams available.",
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
        "not_logged_in" => "Not Login",
        "cred_global_title" => "Global Credentials",
        "cred_global_hint" => "Default username and password used for all discovered devices.",
        "cred_username" => "Username",
        "cred_password" => "Password",

        // ── Add Device ──────────────────────────────────────────────────────
        "add_device_title" => "Add Device",
        "add_device_addr" => "IP address or ONVIF URL",
        "add_device_addr_hint" => "192.168.1.100",
        "add_device_addr_auto" => "Just enter the IP \u{2014} the ONVIF path is added automatically.",
        "add_device_name" => "Display name",
        "add_device_name_hint" => "Optional",
        "add_device_custom_creds" => "Custom credentials",
        "scan_none" => "No devices found. Try adding manually or check network settings.",

        // ── Context Menu ────────────────────────────────────────────────────
        "ctx_copy_addr" => "Copy address",
        "ctx_copied" => "Copied to clipboard.",
        "ctx_delete" => "Delete",
        "ctx_delete_confirm" => "Delete device \"{name}\"?",
        "ctx_add_manual" => "Add to Manual",
        "ctx_added_manual" => "Device added to Manual list.",
        "ctx_edit" => "Edit",

        // ── Edit Device ─────────────────────────────────────────────────────
        "edit_device_title" => "Edit Device",
        "edit_device_cred_hint" => "Leave empty to use global",
        "edit_device_cred_fallback" => "Empty fields fall back to the global credentials.",
        "edit_device_saved" => "Device updated.",
        "add_device_ok" => "Device added.",
        "cred_saved" => "Credentials saved.",
        "scan_found" => "Found {n} device(s).",

        // ── Shared ──────────────────────────────────────────────────────────
        "loading" => "Loading\u{2026}",
        "yes" => "Yes",
        "no" => "No",
        "enabled" => "Enabled",
        "disabled" => "Disabled",

        // ── Identification tab ──────────────────────────────────────────────
        "prop_manufacturer" => "Manufacturer",
        "prop_model" => "Model",
        "prop_firmware" => "Firmware",
        "prop_serial" => "Serial number",
        "prop_hardware_id" => "Hardware ID",
        "prop_scopes" => "Scopes",

        // ── Time tab ────────────────────────────────────────────────────────
        "prop_utc_time" => "UTC time",
        "prop_timezone" => "Timezone",
        "prop_dst" => "Daylight savings",

        // ── Network tab ─────────────────────────────────────────────────────
        "net_hostname" => "Hostname",
        "net_interface" => "Interface",
        "net_gateway" => "Gateway",
        "net_protocols" => "Protocols",

        // ── Users tab ───────────────────────────────────────────────────────
        "user_name" => "Username",
        "user_level" => "Level",
        "user_none" => "No users found.",

        // ── Maintenance tab ─────────────────────────────────────────────────
        "maint_reboot" => "Reboot",
        "maint_reboot_desc" => "Restart the device. This may take a few minutes.",
        "maint_reboot_confirm" => "Are you sure you want to reboot this device?",
        "maint_factory_reset" => "Factory reset",
        "maint_factory_reset_desc" => "Restore all settings to factory defaults. This cannot be undone.",
        "maint_factory_reset_confirm" => "This will erase all settings. Are you sure?",
        "maint_factory_reset_ok" => "Factory reset initiated.",

        _ => return None,
    })
}
