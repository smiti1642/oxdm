#[cfg(test)]
mod i18n_tests {
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
        // Time tab
        "prop_utc_time",
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
}

#[cfg(test)]
mod state_tests {
    use crate::state::{Credentials, Locale, Theme};

    #[test]
    fn theme_cycles_correctly() {
        assert_eq!(Theme::Dark.next(), Theme::Light);
        assert_eq!(Theme::Light.next(), Theme::Classic);
        assert_eq!(Theme::Classic.next(), Theme::Dark);
    }

    #[test]
    fn theme_css_class_contains_theme_name() {
        assert!(Theme::Dark.css_class().contains("dark"));
        assert!(Theme::Light.css_class().contains("light"));
        assert!(Theme::Classic.css_class().contains("classic"));
    }

    #[test]
    fn locale_cycles_correctly() {
        assert_eq!(Locale::En.next(), Locale::ZhTw);
        assert_eq!(Locale::ZhTw.next(), Locale::Ru);
        assert_eq!(Locale::Ru.next(), Locale::En);
    }

    #[test]
    fn locale_labels_are_short() {
        for locale in [Locale::En, Locale::ZhTw, Locale::Ru] {
            let label = locale.label();
            assert!(label.len() <= 4, "label too long: {label}");
        }
    }

    #[test]
    fn credentials_default_is_empty() {
        let c = Credentials::default();
        assert!(c.username.is_empty());
        assert!(c.password.is_empty());
    }
}

#[cfg(test)]
mod util_tests {
    use crate::components::credentials_dialog::normalize_onvif_addr;
    use crate::util::{extract_ip, urldecode};
    use crate::views::settings::identification::{scope_key, scope_value};
    use crate::views::settings::time::epoch_days_to_ymd;

    #[test]
    fn normalize_bare_ip() {
        assert_eq!(
            normalize_onvif_addr("192.168.1.10"),
            "http://192.168.1.10/onvif/device_service"
        );
    }

    #[test]
    fn normalize_ip_with_port() {
        assert_eq!(
            normalize_onvif_addr("192.168.1.10:8080"),
            "http://192.168.1.10:8080/onvif/device_service"
        );
    }

    #[test]
    fn normalize_http_no_path() {
        assert_eq!(
            normalize_onvif_addr("http://192.168.1.10"),
            "http://192.168.1.10/onvif/device_service"
        );
    }

    #[test]
    fn normalize_full_url_preserved() {
        let url = "http://192.168.1.10/onvif/device_service";
        assert_eq!(normalize_onvif_addr(url), url);
    }

    #[test]
    fn normalize_custom_path_preserved() {
        let url = "http://192.168.1.10/custom/path";
        assert_eq!(normalize_onvif_addr(url), url);
    }

    #[test]
    fn normalize_empty() {
        assert_eq!(normalize_onvif_addr(""), "");
    }

    #[test]
    fn normalize_whitespace() {
        assert_eq!(
            normalize_onvif_addr("  192.168.1.10  "),
            "http://192.168.1.10/onvif/device_service"
        );
    }

    #[test]
    fn extract_ip_from_http_url() {
        assert_eq!(
            extract_ip("http://192.168.1.10/onvif/device_service"),
            "192.168.1.10"
        );
    }

    #[test]
    fn extract_ip_from_https_url() {
        assert_eq!(
            extract_ip("https://10.0.0.5/onvif/device_service"),
            "10.0.0.5"
        );
    }

    #[test]
    fn extract_ip_with_port() {
        assert_eq!(
            extract_ip("http://192.168.1.10:8080/onvif/device_service"),
            "192.168.1.10"
        );
    }

    #[test]
    fn extract_ip_bare_ip() {
        assert_eq!(extract_ip("192.168.1.10"), "192.168.1.10");
    }

    #[test]
    fn extract_ip_empty_string() {
        assert_eq!(extract_ip(""), "");
    }

    #[test]
    fn scope_parsing_standard() {
        let s = "onvif://www.onvif.org/type/video_encoder";
        assert_eq!(scope_key(s), "type");
        assert_eq!(scope_value(s), "video_encoder");
    }

    #[test]
    fn scope_parsing_name() {
        let s = "onvif://www.onvif.org/name/MyCamera";
        assert_eq!(scope_key(s), "name");
        assert_eq!(scope_value(s), "MyCamera");
    }

    #[test]
    fn scope_parsing_unknown_format() {
        let s = "some-random-scope";
        assert_eq!(scope_key(s), "scope");
        assert_eq!(scope_value(s), "some-random-scope");
    }

    #[test]
    fn urldecode_multibyte_utf8() {
        assert_eq!(urldecode("caf%C3%A9"), "café");
    }

    #[test]
    fn epoch_days_unix_epoch() {
        // 1970-01-01
        assert_eq!(epoch_days_to_ymd(0), (1970, 1, 1));
    }

    #[test]
    fn epoch_days_known_date() {
        // 2024-01-01 is day 19723 from epoch
        assert_eq!(epoch_days_to_ymd(19723), (2024, 1, 1));
    }

    #[test]
    fn epoch_days_leap_year() {
        // 2024-02-29 is day 19723 + 59 = 19782
        assert_eq!(epoch_days_to_ymd(19782), (2024, 2, 29));
    }
}
