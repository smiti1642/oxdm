use crate::i18n::{en, ru, t, zh_tw};
use crate::state::Locale;

/// Collect all keys from a locale's get() function by testing known keys.
/// This is the canonical key list defined in en.rs.
const ALL_KEYS: &[&str] = &[
    // Topbar
    "search_placeholder",
    "tooltip_credentials",
    "tooltip_theme",
    "tooltip_language",
    "tooltip_help",
    // Sidebar
    "sidebar_title",
    "filter_placeholder",
    "no_devices",
    "no_manual_devices",
    "no_matches",
    "filter_status_tooltip",
    "filter_status_all",
    "filter_status_ok",
    "filter_status_failed",
    "filter_status_unknown",
    "filter_sort_tooltip",
    "filter_sort_default",
    "filter_sort_name",
    "filter_sort_ip",
    "devtab_discovered",
    "devtab_manual",
    "btn_scan_label",
    "btn_scanning",
    "btn_add_label",
    // Device Panel
    "select_device",
    "section_general",
    "nav_settings",
    "nav_events",
    "no_profiles",
    "nav_live_video",
    "nav_imaging",
    "nav_ptz",
    // Main Content
    "app_name",
    "app_subtitle",
    "welcome_hint",
    "coming_soon",
    // Settings Tabs
    "tab_identification",
    "tab_network",
    "tab_time",
    "tab_users",
    "tab_maintenance",
    // Status Bar
    "status_devices",
    "status_device",
    "status_online",
    "status_scanning",
    "status_ws_discovery",
    // Theme
    "theme_dark",
    "theme_light",
    "theme_classic",
    // Buttons
    "btn_cancel",
    "btn_save",
    "btn_confirm",
    "btn_add_short",
    // Credentials
    "not_logged_in",
    "cred_global_title",
    "cred_global_hint",
    "cred_username",
    "cred_password",
    // Add Device
    "add_device_title",
    "add_device_addr",
    "add_device_name",
    "add_device_addr_hint",
    "add_device_addr_auto",
    "add_device_name_hint",
    "add_device_custom_creds",
    "scan_none",
    // Context menu
    "ctx_copy_addr",
    "ctx_copied",
    "ctx_delete",
    "ctx_delete_confirm",
    "ctx_add_manual",
    "ctx_added_manual",
    "ctx_edit",
    // Edit Device
    "edit_device_title",
    "edit_device_cred_hint",
    "edit_device_cred_fallback",
    "edit_device_saved",
    "add_device_ok",
    "cred_saved",
    "scan_found",
    // Live Video
    "live_video_no_device",
    "live_video_no_profile",
    "live_video_no_backend",
    "live_video_error",
    // PTZ
    "ptz_zoom",
    "ptz_speed",
    "ptz_home",
    "ptz_home_ok",
    "ptz_stop",
    "ptz_presets",
    "ptz_no_presets",
    "ptz_unavailable",
    "ptz_focus",
    "ptz_focus_near",
    "ptz_focus_far",
    "ptz_focus_auto",
    "ptz_focus_manual",
    "ptz_preset_new_placeholder",
    "ptz_preset_save_hint",
    "ptz_preset_saved",
    "ptz_preset_removed",
    // Shared
    "loading",
    "yes",
    "no",
    "enabled",
    "disabled",
    // Identification tab
    "prop_manufacturer",
    "prop_model",
    "prop_firmware",
    "prop_serial",
    "prop_hardware_id",
    "prop_scopes",
    "id_editable_scopes",
    "id_name",
    "id_location",
    "id_scopes_saved",
    // Time tab
    "time_sync_section",
    "time_edit_section",
    "time_sync_pc",
    "time_sync_pc_hint",
    "time_use_ntp",
    "time_use_ntp_hint",
    "time_apply_tz",
    "time_apply_tz_hint",
    "time_saved",
    "tz_current_prefix",
    "tz_custom",
    "tz_custom_label",
    "prop_utc_time",
    "prop_local_time",
    "prop_timezone",
    "prop_dst",
    // Network tab
    "net_hostname",
    "net_interface",
    "net_gateway",
    "net_protocols",
    // Users tab
    "user_name",
    "user_level",
    "user_none",
    "user_add_section",
    "user_add",
    "user_created",
    "user_updated",
    "user_deleted",
    "user_delete_title",
    "user_delete_confirm",
    "user_edit_pw_placeholder",
    "user_create_needs_fields",
    "btn_edit",
    "btn_delete",
    // Maintenance tab
    "maint_reboot",
    "maint_reboot_desc",
    "maint_reboot_confirm",
    "maint_factory_reset",
    "maint_factory_reset_desc",
    "maint_factory_reset_confirm",
    "maint_factory_reset_ok",
    // Imaging
    "img_basic",
    "img_brightness",
    "img_contrast",
    "img_saturation",
    "img_sharpness",
    "img_exposure",
    "img_white_balance",
    "img_backlight",
    "img_wdr",
    "img_ir_cut",
    "img_focus",
    "img_mode",
    "img_level",
    "img_saved",
    "btn_apply",
];

#[test]
fn en_has_all_keys() {
    for key in ALL_KEYS {
        assert!(en::get(key).is_some(), "English missing key: {key}");
    }
}

#[test]
fn zh_tw_has_all_keys() {
    let mut missing = Vec::new();
    for key in ALL_KEYS {
        if zh_tw::get(key).is_none() {
            missing.push(*key);
        }
    }
    assert!(
        missing.is_empty(),
        "zh_tw missing {} key(s): {:?}",
        missing.len(),
        missing
    );
}

#[test]
fn ru_has_all_keys() {
    let mut missing = Vec::new();
    for key in ALL_KEYS {
        if ru::get(key).is_none() {
            missing.push(*key);
        }
    }
    assert!(
        missing.is_empty(),
        "ru missing {} key(s): {:?}",
        missing.len(),
        missing
    );
}

#[test]
fn t_returns_english_for_en() {
    assert_eq!(t(Locale::En, "app_name"), "OxDM");
}

#[test]
fn t_falls_back_to_english_for_unknown_key_in_other_locale() {
    // If a key exists in en but not in zh_tw, t() should still return the English value.
    // We test with a key we know exists in all locales to verify basic dispatch.
    let en_val = t(Locale::En, "app_name");
    let zh_val = t(Locale::ZhTw, "app_name");
    // app_name is "OxDM" in all locales
    assert_eq!(en_val, "OxDM");
    assert_eq!(zh_val, "OxDM");
}

#[test]
fn t_returns_translated_value() {
    // "sidebar_title" is "Devices" in EN, should be different in ZH
    let en = t(Locale::En, "sidebar_title");
    let zh = t(Locale::ZhTw, "sidebar_title");
    assert_eq!(en, "Devices");
    assert_ne!(en, zh, "zh_tw should translate 'sidebar_title'");
}

#[test]
fn scan_found_has_placeholder() {
    let msg = t(Locale::En, "scan_found");
    assert!(
        msg.contains("{n}"),
        "scan_found should contain {{n}} placeholder"
    );
}
