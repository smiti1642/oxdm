pub fn get(key: &str) -> Option<&'static str> {
    Some(match key {
        // ── Topbar ──────────────────────────────────────────────────────────
        "search_placeholder" => "\u{641C}\u{5C0B}\u{88DD}\u{7F6E}\u{2026}",
        "tooltip_credentials" => "\u{6191}\u{8B49}",
        "tooltip_theme" => "\u{4E3B}\u{984C}",
        "tooltip_language" => "\u{8A9E}\u{8A00}",
        "tooltip_help" => "\u{8AAA}\u{660E}",

        // ── Sidebar ─────────────────────────────────────────────────────────
        "sidebar_title" => "\u{88DD}\u{7F6E}",
        "filter_placeholder" => "\u{540D}\u{7A31}\u{6216}\u{4F4D}\u{5740}\u{2026}",
        "no_devices" => "\u{672A}\u{627E}\u{5230}\u{88DD}\u{7F6E}\u{3002}\n\u{9EDE}\u{64CA}\u{300C}\u{6383}\u{63CF}\u{300D}\u{4EE5}\u{63A2}\u{7D22}\u{3002}",
        "no_manual_devices" => "\u{7121}\u{624B}\u{52D5}\u{88DD}\u{7F6E}\u{3002}\n\u{9EDE}\u{64CA}\u{300C}\u{65B0}\u{589E}\u{300D}\u{4EE5}\u{5EFA}\u{7ACB}\u{3002}",
        "no_matches" => "\u{7121}\u{7B26}\u{5408}\u{7D50}\u{679C}\u{3002}",
        "devtab_discovered" => "\u{63A2}\u{7D22}",
        "devtab_manual" => "\u{624B}\u{52D5}",
        "btn_scan_label" => "\u{6383}\u{63CF}",
        "btn_scanning" => "\u{6383}\u{63CF}\u{4E2D}\u{2026}",
        "btn_add_label" => "\u{65B0}\u{589E}",

        // ── Device Panel ────────────────────────────────────────────────────
        "select_device" => "\u{2190} \u{9078}\u{64C7}\u{88DD}\u{7F6E}",
        "section_general" => "\u{4E00}\u{822C}",
        "nav_settings" => "\u{8A2D}\u{5B9A}",
        "nav_events" => "\u{4E8B}\u{4EF6}",
        "no_profiles" => "\u{7121}\u{53EF}\u{7528}\u{8A2D}\u{5B9A}\u{6A94}\u{3002}",
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
        "not_logged_in" => "\u{672A}\u{767B}\u{5165}",
        "cred_global_title" => "\u{5168}\u{57DF}\u{6191}\u{8B49}",
        "cred_global_hint" => "\u{6383}\u{63CF}\u{5230}\u{7684}\u{88DD}\u{7F6E}\u{5747}\u{4F7F}\u{7528}\u{6B64}\u{5E33}\u{5BC6}\u{3002}",
        "cred_username" => "\u{5E33}\u{865F}",
        "cred_password" => "\u{5BC6}\u{78BC}",

        // ── Add Device ──────────────────────────────────────────────────────
        "add_device_title" => "\u{65B0}\u{589E}\u{88DD}\u{7F6E}",
        "add_device_addr" => "IP \u{4F4D}\u{5740}\u{6216} ONVIF URL",
        "add_device_addr_hint" => "192.168.1.100",
        "add_device_addr_auto" => "\u{53EA}\u{9700}\u{8F38}\u{5165} IP\u{FF0C}ONVIF \u{8DEF}\u{5F91}\u{6703}\u{81EA}\u{52D5}\u{88DC}\u{5168}\u{3002}",
        "scan_none" => "\u{672A}\u{627E}\u{5230}\u{88DD}\u{7F6E}\u{3002}\u{8ACB}\u{5617}\u{8A66}\u{624B}\u{52D5}\u{65B0}\u{589E}\u{6216}\u{6AA2}\u{67E5}\u{7DB2}\u{8DEF}\u{8A2D}\u{5B9A}\u{3002}",

        // ── Context Menu ────────────────────────────────────────────────────
        "ctx_copy_addr" => "\u{8907}\u{88FD}\u{4F4D}\u{5740}",
        "ctx_copied" => "\u{5DF2}\u{8907}\u{88FD}\u{5230}\u{526A}\u{8CBC}\u{7C3F}\u{3002}",
        "ctx_delete" => "\u{522A}\u{9664}",
        "ctx_delete_confirm" => "\u{78BA}\u{5B9A}\u{522A}\u{9664}\u{88DD}\u{7F6E}\u{300C}{name}\u{300D}\u{FF1F}",
        "ctx_add_manual" => "\u{52A0}\u{5165}\u{624B}\u{52D5}\u{6E05}\u{55AE}",
        "ctx_added_manual" => "\u{88DD}\u{7F6E}\u{5DF2}\u{52A0}\u{5165}\u{624B}\u{52D5}\u{6E05}\u{55AE}\u{3002}",
        "ctx_edit" => "\u{7DE8}\u{8F2F}",

        // ── Edit Device ─────────────────────────────────────────────────────
        "edit_device_title" => "\u{7DE8}\u{8F2F}\u{88DD}\u{7F6E}",
        "edit_device_cred_hint" => "\u{7559}\u{7A7A}\u{5247}\u{4F7F}\u{7528}\u{5168}\u{57DF}\u{6191}\u{8B49}",
        "edit_device_cred_fallback" => "\u{7559}\u{7A7A}\u{6B04}\u{4F4D}\u{5C07}\u{4F7F}\u{7528}\u{5168}\u{57DF}\u{6191}\u{8B49}\u{3002}",
        "edit_device_saved" => "\u{88DD}\u{7F6E}\u{5DF2}\u{66F4}\u{65B0}\u{3002}",
        "add_device_name" => "\u{986F}\u{793A}\u{540D}\u{7A31}",
        "add_device_name_hint" => "\u{9078}\u{586B}",
        "add_device_custom_creds" => "\u{81EA}\u{8A02}\u{6191}\u{8B49}",
        "add_device_ok" => "\u{88DD}\u{7F6E}\u{5DF2}\u{65B0}\u{589E}\u{3002}",
        "cred_saved" => "\u{6191}\u{8B49}\u{5DF2}\u{5132}\u{5B58}\u{3002}",
        "scan_found" => "\u{627E}\u{5230} {n} \u{53F0}\u{88DD}\u{7F6E}\u{3002}",

        // ── Live Video ──────────────────────────────────────────────────────
        "live_video_no_device" => "\u{8ACB}\u{9078}\u{53D6}\u{4E00}\u{53F0}\u{88DD}\u{7F6E}\u{4EE5}\u{958B}\u{59CB}\u{4E32}\u{6D41}\u{3002}",
        "live_video_no_profile" => "\u{8ACB}\u{5F9E}\u{88DD}\u{7F6E}\u{9762}\u{677F}\u{9078}\u{53D6}\u{4E00}\u{500B} profile\u{3002}",
        "live_video_no_backend" => "\u{5C1A}\u{672A}\u{5B89}\u{88DD}\u{5F71}\u{50CF}\u{5F8C}\u{7AEF}\u{3002}",
        "live_video_error" => "\u{4E32}\u{6D41}\u{555F}\u{52D5}\u{5931}\u{6557}\u{3002}",

        // ── Shared ──────────────────────────────────────────────────────────
        "loading" => "\u{8F09}\u{5165}\u{4E2D}\u{2026}",
        "yes" => "\u{662F}",
        "no" => "\u{5426}",
        "enabled" => "\u{5DF2}\u{555F}\u{7528}",
        "disabled" => "\u{5DF2}\u{505C}\u{7528}",

        // ── Identification tab ──────────────────────────────────────────────
        "prop_manufacturer" => "\u{88FD}\u{9020}\u{5546}",
        "prop_model" => "\u{578B}\u{865F}",
        "prop_firmware" => "\u{97CC}\u{9AD4}",
        "prop_serial" => "\u{5E8F}\u{865F}",
        "prop_hardware_id" => "\u{786C}\u{9AD4} ID",
        "prop_scopes" => "Scopes",

        // ── Time tab ────────────────────────────────────────────────────────
        "prop_utc_time" => "UTC \u{6642}\u{9593}",
        "prop_timezone" => "\u{6642}\u{5340}",
        "prop_dst" => "\u{65E5}\u{5149}\u{7BC0}\u{7D04}",

        // ── Network tab ─────────────────────────────────────────────────────
        "net_hostname" => "\u{4E3B}\u{6A5F}\u{540D}\u{7A31}",
        "net_interface" => "\u{7DB2}\u{8DEF}\u{4ECB}\u{9762}",
        "net_gateway" => "\u{9810}\u{8A2D}\u{9598}\u{9053}",
        "net_protocols" => "\u{5354}\u{5B9A}",

        // ── Users tab ───────────────────────────────────────────────────────
        "user_name" => "\u{5E33}\u{865F}",
        "user_level" => "\u{6B0A}\u{9650}",
        "user_none" => "\u{672A}\u{627E}\u{5230}\u{4F7F}\u{7528}\u{8005}\u{3002}",

        // ── Maintenance tab ─────────────────────────────────────────────────
        "maint_reboot" => "\u{91CD}\u{65B0}\u{958B}\u{6A5F}",
        "maint_reboot_desc" => "\u{91CD}\u{65B0}\u{555F}\u{52D5}\u{88DD}\u{7F6E}\u{FF0C}\u{53EF}\u{80FD}\u{9700}\u{8981}\u{5E7E}\u{5206}\u{9418}\u{3002}",
        "maint_reboot_confirm" => "\u{78BA}\u{5B9A}\u{8981}\u{91CD}\u{65B0}\u{958B}\u{6A5F}\u{55CE}\u{FF1F}",
        "maint_factory_reset" => "\u{6062}\u{5FA9}\u{51FA}\u{5EE0}\u{8A2D}\u{5B9A}",
        "maint_factory_reset_desc" => "\u{5C07}\u{6240}\u{6709}\u{8A2D}\u{5B9A}\u{6062}\u{5FA9}\u{70BA}\u{51FA}\u{5EE0}\u{9810}\u{8A2D}\u{3002}\u{6B64}\u{64CD}\u{4F5C}\u{7121}\u{6CD5}\u{5FA9}\u{539F}\u{3002}",
        "maint_factory_reset_confirm" => "\u{9019}\u{5C07}\u{6E05}\u{9664}\u{6240}\u{6709}\u{8A2D}\u{5B9A}\u{3002}\u{78BA}\u{5B9A}\u{55CE}\u{FF1F}",
        "maint_factory_reset_ok" => "\u{5DF2}\u{555F}\u{52D5}\u{51FA}\u{5EE0}\u{91CD}\u{8A2D}\u{3002}",

        // ── Imaging ─────────────────────────────────────────────────────────
        "img_basic" => "\u{57FA}\u{672C}",
        "img_brightness" => "\u{4EAE}\u{5EA6}",
        "img_contrast" => "\u{5C0D}\u{6BD4}",
        "img_saturation" => "\u{98FD}\u{548C}\u{5EA6}",
        "img_sharpness" => "\u{92B3}\u{5229}\u{5EA6}",
        "img_exposure" => "\u{66DD}\u{5149}",
        "img_white_balance" => "\u{767D}\u{5E73}\u{8861}",
        "img_backlight" => "\u{80CC}\u{5149}\u{88DC}\u{511F}",
        "img_wdr" => "\u{5BEC}\u{52D5}\u{614B}",
        "img_ir_cut" => "\u{7D05}\u{5916}\u{7DDA}\u{6FFE}\u{5149}\u{7247}",
        "img_focus" => "\u{5C0D}\u{7126}",
        "img_mode" => "\u{6A21}\u{5F0F}",
        "img_level" => "\u{7B49}\u{7D1A}",
        "img_saved" => "\u{5F71}\u{50CF}\u{8A2D}\u{5B9A}\u{5DF2}\u{5132}\u{5B58}\u{3002}",
        "btn_apply" => "\u{5957}\u{7528}",

        _ => return None,
    })
}
