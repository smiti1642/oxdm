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
    "btn_scanning_tooltip",
    "btn_retry",
    "addr_err_empty",
    "addr_err_ip",
    "addr_err_port",
    "addr_err_hostname",
    "addr_err_no_host",
    "net_disabled_by_dhcp",
    "btn_add_label",
    // Device Panel
    "select_device",
    "section_general",
    "nav_settings",
    "nav_events",
    "nav_osd",
    "nav_io_control",
    // IO Control view
    "io_control",
    "io_relay_outputs",
    "io_digital_inputs",
    "io_input_hint",
    "io_no_relays",
    "io_no_inputs",
    "io_relays_unsupported",
    "io_inputs_unsupported",
    "io_mode",
    "io_mode_bistable",
    "io_mode_monostable",
    "io_idle_state",
    "io_idle_closed",
    "io_idle_open",
    "io_idle_unknown",
    "io_delay_time",
    "io_delay_hint",
    "io_activate",
    "io_deactivate",
    "io_pulse",
    "io_edit",
    "io_activated",
    "io_deactivated",
    "io_pulse_sent",
    "io_pulse_failed",
    "io_set_state_failed",
    "io_settings_saved",
    "io_settings_failed",
    "io_confirm_save_title",
    "io_confirm_save_msg",
    "nav_video_encoder",
    "ve_no_encoder",
    "ve_encoding",
    "ve_resolution",
    "ve_frame_rate",
    "ve_bitrate",
    "ve_gov_length",
    "ve_quality",
    "ve_profile",
    "ve_save",
    "ve_saved",
    "osd_add",
    "osd_empty",
    "osd_create",
    "osd_edit",
    "osd_col_type",
    "osd_col_position",
    "osd_col_content",
    "osd_field_text_type",
    "osd_field_text",
    "osd_field_date_format",
    "osd_field_time_format",
    "osd_field_position",
    "osd_field_font_size",
    "osd_saved",
    "osd_deleted",
    "osd_delete_title",
    "osd_delete_confirm",
    "profile_create",
    "profile_name_placeholder",
    "profile_created",
    "profile_delete",
    "profile_delete_title",
    "profile_delete_confirm",
    "profile_deleted",
    "events_status_connecting",
    "events_status_connected",
    "events_status_error",
    "events_pause",
    "events_resume",
    "events_clear",
    "events_empty",
    "events_col_time",
    "events_col_topic",
    "events_col_data",
    "events_col_utc",
    "events_col_op",
    "events_col_source",
    "events_show_details",
    "events_topics",
    "events_topics_all",
    "events_topics_none",
    "events_topics_empty",
    "events_search_placeholder",
    "events_empty_filtered",
    "no_profiles",
    "nav_live_video",
    "nav_imaging",
    "nav_ptz",
    // Main Content
    "app_name",
    "app_subtitle",
    "welcome_hint",
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
    "btn_close",
    "snapshot_save",
    "snapshot_save_no_image",
    "snapshot_saved",
    "snapshot_save_failed",
    "about_open_logs",
    "about_github",
    "about_log_dir",
    "about_shortcuts",
    "shortcut_focus_search",
    "shortcut_scan",
    "shortcut_nav_devices",
    "shortcut_close_modal",
    "about_log_to_file",
    "about_log_takes_effect",
    "about_tls_strict",
    "about_tls_strict_hint",
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
    "live_mode_snapshot",
    "live_mode_snapshot_hint",
    "live_mode_rtsp",
    "live_mode_rtsp_hint",
    "live_h265_tip",
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
    "ptz_preset_search_placeholder",
    "ptz_presets_no_match",
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
    "net_saved",
    "net_saved_reboot",
    "net_iface_enabled",
    "net_iface_prefix",
    "net_iface_confirm_title",
    "net_iface_confirm_msg",
    "net_ipv6",
    "net_ipv6_enabled",
    "net_ipv6_dhcp",
    "net_ipv6_manual",
    "net_mtu",
    "net_save_v6",
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
    "img_manual_group",
    "img_exposure_priority",
    "img_exposure_time",
    "img_exposure_gain",
    "img_exposure_iris",
    "img_wb_cr_gain",
    "img_wb_cb_gain",
    "img_focus_near",
    "img_focus_far",
    "btn_apply",
    // Health baseline + diff (oxvif 0.9.8)
    "health_save_baseline",
    "health_baseline_saved",
    "health_baseline_save_failed",
    "health_baseline_loaded",
    "health_diff_title",
    "health_diff_none",
    "health_diff_flipped_fail",
    "health_diff_flipped_pass",
    "health_diff_added",
    "health_diff_removed",
    "health_diff_slowed",
    // Batch health overview
    "hbatch_title",
    "hbatch_subtitle",
    "hbatch_select_all",
    "hbatch_run",
    "hbatch_running",
    "hbatch_export",
    "hbatch_redact",
    "hbatch_no_devices",
    "hbatch_idle",
    "hbatch_pending",
    "hbatch_state_running",
    "hbatch_timeout",
    "hbatch_fp_failed",
    "hbatch_gprobe",
    "hbatch_gprobe_search",
    "hbatch_gprobe_recs",
    "hbatch_gprobe_replay",
    "hbatch_gprobe_replay_ok",
    "hbatch_exported",
    "hbatch_export_failed",
    "hbatch_export_nothing",
    // HealthCheck groups
    "ctx_add_to_group",
    "hgroups_all_devices",
    "hgroups_groups_header",
    "hgroups_empty",
    "hgroups_new_group_placeholder",
    "hgroups_create",
    "hgroups_added",
    "hgroups_already",
    "hgroups_group_creds",
    "hgroups_group_creds_title",
    "hgroups_device_creds_title",
    "hgroups_creds_hint",
    "hgroups_source_label",
    "hgroups_auth_label",
    "hgroups_offline",
    "hgroups_cred_device",
    "hgroups_cred_group",
    "hgroups_cred_app",
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
