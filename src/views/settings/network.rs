#![allow(non_snake_case)]
use crate::components::{Icon, PropRow};
use crate::state::{ConfirmDialog, Credentials, Ctx, ToastLevel};
use crate::{api, i18n};
use dioxus::prelude::*;

#[derive(Debug, Clone)]
struct NetworkData {
    hostname: Option<String>,
    hostname_dhcp: bool,
    interfaces: Vec<oxvif::NetworkInterface>,
    dns_servers: Vec<String>,
    dns_from_dhcp: bool,
    ntp_servers: Vec<String>,
    ntp_from_dhcp: bool,
    gateways: Vec<String>,
    protocols: Vec<oxvif::NetworkProtocol>,
}

#[component]
pub fn NetworkTab(addr: ReadSignal<String>, creds: Memo<Credentials>) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let mut info = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move {
            let hostname = api::get_hostname(&addr, &creds).await.ok();
            let ifaces = api::get_network_interfaces(&addr, &creds)
                .await
                .unwrap_or_default();
            let dns = api::get_dns(&addr, &creds).await.ok();
            let ntp = api::get_ntp(&addr, &creds).await.ok();
            let gw = api::get_network_default_gateway(&addr, &creds).await.ok();
            let protocols = api::get_network_protocols(&addr, &creds)
                .await
                .unwrap_or_default();
            Ok::<_, String>(NetworkData {
                hostname: hostname.as_ref().and_then(|h| h.name.clone()),
                hostname_dhcp: hostname.map(|h| h.from_dhcp).unwrap_or(false),
                interfaces: ifaces,
                dns_servers: dns.as_ref().map(|d| d.servers.clone()).unwrap_or_default(),
                dns_from_dhcp: dns.map(|d| d.from_dhcp).unwrap_or(false),
                ntp_servers: ntp.as_ref().map(|n| n.servers.clone()).unwrap_or_default(),
                ntp_from_dhcp: ntp.map(|n| n.from_dhcp).unwrap_or(false),
                gateways: gw
                    .map(|g| {
                        g.ipv4_addresses
                            .into_iter()
                            .chain(g.ipv6_addresses)
                            .collect()
                    })
                    .unwrap_or_default(),
                protocols,
            })
        }
    });

    // Shared refresh callback — sections call this after a successful
    // Save to re-pull the full network state. Cheaper than a dedicated
    // per-section fetcher and keeps sections reading consistent data.
    let refresh = use_callback(move |_: ()| info.restart());

    rsx! {
        match &*info.read_unchecked() {
            None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
            Some(Err(e)) => rsx! {
                crate::components::TabError {
                    error: e.clone(),
                    on_retry: move |_| info.restart(),
                }
            },
            Some(Ok(data)) => rsx! {
                HostnameSection {
                    addr, creds, refresh,
                    current_name: data.hostname.clone().unwrap_or_default(),
                    from_dhcp: data.hostname_dhcp,
                }
                for iface in &data.interfaces {
                    InterfaceSection {
                        key: "{iface.token}",
                        addr, creds, refresh,
                        token: iface.token.clone(),
                        iface_name: iface.name.clone(),
                        mac: iface.hw_address.clone(),
                        mtu: iface.mtu,
                        enabled: iface.enabled,
                        ipv4_address: iface.ipv4_address.clone(),
                        ipv4_prefix_length: iface.ipv4_prefix_length,
                        ipv4_from_dhcp: iface.ipv4_from_dhcp,
                        ipv6_enabled: iface.ipv6_enabled,
                        ipv6_address: iface.ipv6_address.clone().unwrap_or_default(),
                    }
                }
                GatewaySection {
                    addr, creds, refresh,
                    current: data.gateways.clone(),
                }
                ServerGroupSection {
                    header: "DNS",
                    addr, creds, refresh,
                    current_servers: data.dns_servers.clone(),
                    from_dhcp: data.dns_from_dhcp,
                    kind: ServerKind::Dns,
                }
                ServerGroupSection {
                    header: "NTP",
                    addr, creds, refresh,
                    current_servers: data.ntp_servers.clone(),
                    from_dhcp: data.ntp_from_dhcp,
                    kind: ServerKind::Ntp,
                }
                ProtocolsSection {
                    addr, creds, refresh,
                    entries: data.protocols.iter().map(|p| {
                        (
                            p.name.clone(),
                            p.enabled,
                            p.ports.iter().map(u32::to_string).collect::<Vec<_>>().join(", "),
                        )
                    }).collect(),
                }
            },
        }
    }
}

// ── Hostname ──────────────────────────────────────────────────────────────

#[component]
fn HostnameSection(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    refresh: Callback<()>,
    current_name: String,
    from_dhcp: bool,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let name_in = use_signal(|| current_name);

    rsx! {
        div { class: "prop-section-header", {i18n::t(locale, "net_hostname")} }
        table { class: "prop-table",
            PropRow { label: "DHCP", value: yn(locale, from_dhcp) }
        }
        div { class: "id-edit-form",
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_hostname")} }
                input {
                    class: "id-edit-input",
                    r#type: "text",
                    value: "{*name_in.read()}",
                    oninput: move |e| name_in.clone().set(e.value()),
                }
            }
            div { class: "id-edit-actions",
                button {
                    class: "btn btn-md btn-primary",
                    onclick: move |_| {
                        let addr_s = addr.read().clone();
                        let creds_s = creds.read().clone();
                        let name = name_in.peek().clone();
                        spawn(async move {
                            match api::set_hostname(&addr_s, &creds_s, &name).await {
                                Ok(()) => {
                                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "net_saved"));
                                    refresh.call(());
                                }
                                Err(e) => ctx.push_toast(ToastLevel::Error, e),
                            }
                        });
                    },
                    Icon { name: "check", size: 14 } " " {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}

// ── Network interface (IP/DHCP) ───────────────────────────────────────────

// Fields are passed individually because `oxvif::NetworkInterface` doesn't
// implement PartialEq (required for Dioxus component props).
#[component]
fn InterfaceSection(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    refresh: Callback<()>,
    token: String,
    iface_name: String,
    mac: String,
    mtu: u32,
    enabled: bool,
    ipv4_address: String,
    ipv4_prefix_length: u32,
    ipv4_from_dhcp: bool,
    ipv6_enabled: bool,
    ipv6_address: String,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let enabled_in = use_signal(|| enabled);
    let dhcp_in = use_signal(|| ipv4_from_dhcp);
    let addr_in = use_signal(|| ipv4_address);
    let prefix_in = use_signal(|| ipv4_prefix_length.to_string());

    // IPv6 + MTU controls (oxvif 0.9.8). Initialised from the read-side
    // values returned by GetNetworkInterfaces. Saved through the new
    // struct-based set_network_interfaces_full.
    let ipv6_enabled_in = use_signal(|| ipv6_enabled);
    let ipv6_dhcp_in = use_signal(|| false);
    let ipv6_address_in = use_signal(|| ipv6_address.clone());
    let ipv6_prefix_in = use_signal(|| "64".to_string());
    let mtu_in = use_signal(|| mtu.to_string());

    let header = format!("{} ({})", i18n::t(locale, "net_interface"), iface_name);
    let token_for_save = token.clone();
    let token_for_save_v6 = token.clone();

    rsx! {
        div { class: "prop-section-header", "{header}" }
        table { class: "prop-table",
            PropRow { label: "Token",  value: token }
            PropRow { label: "MAC",    value: mac }
            PropRow { label: "MTU",    value: mtu.to_string() }
            if ipv6_enabled && !ipv6_address.is_empty() {
                PropRow { label: "IPv6", value: ipv6_address }
            }
        }
        div { class: "id-edit-form",
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_iface_enabled")} }
                input {
                    r#type: "checkbox",
                    checked: "{*enabled_in.read()}",
                    oninput: move |e| enabled_in.clone().set(e.value() == "true"),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", "DHCP" }
                input {
                    r#type: "checkbox",
                    checked: "{*dhcp_in.read()}",
                    oninput: move |e| dhcp_in.clone().set(e.value() == "true"),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", "IPv4" }
                input {
                    class: "id-edit-input",
                    r#type: "text",
                    disabled: "{*dhcp_in.read()}",
                    title: if *dhcp_in.read() { i18n::t(locale, "net_disabled_by_dhcp") } else { "" },
                    value: "{*addr_in.read()}",
                    oninput: move |e| addr_in.clone().set(e.value()),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_iface_prefix")} }
                input {
                    class: "id-edit-input",
                    r#type: "number",
                    min: "1", max: "32",
                    disabled: "{*dhcp_in.read()}",
                    title: if *dhcp_in.read() { i18n::t(locale, "net_disabled_by_dhcp") } else { "" },
                    value: "{*prefix_in.read()}",
                    oninput: move |e| prefix_in.clone().set(e.value()),
                }
            }
            div { class: "id-edit-actions",
                button {
                    class: "btn btn-md btn-primary",
                    onclick: move |_| {
                        let tok = token_for_save.clone();
                        let enabled = *enabled_in.peek();
                        let dhcp = *dhcp_in.peek();
                        let ip = addr_in.peek().clone();
                        let prefix: u32 = prefix_in.peek().parse().unwrap_or(24);
                        // Changing interface config can disconnect the
                        // camera from us. Gate behind a destructive
                        // confirm so accidental clicks don't brick
                        // access — matches the factory-reset UX.
                        ctx.dialog.clone().set(Some(ConfirmDialog {
                            title: i18n::t(locale, "net_iface_confirm_title").to_string(),
                            message: i18n::t(locale, "net_iface_confirm_msg").to_string(),
                            confirm_label: i18n::t(locale, "btn_confirm").to_string(),
                            cancel_label: i18n::t(locale, "btn_cancel").to_string(),
                            dangerous: true,
                            on_confirm: EventHandler::new(move |_| {
                                let tok = tok.clone();
                                let ip = ip.clone();
                                let addr_s = addr.read().clone();
                                let creds_s = creds.read().clone();
                                spawn(async move {
                                            match api::set_network_interfaces(
                                        &addr_s, &creds_s, &tok,
                                        enabled, &ip, prefix, dhcp,
                                    ).await {
                                        Ok(reboot) => {
                                            let key = if reboot { "net_saved_reboot" } else { "net_saved" };
                                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, key));
                                            refresh.call(());
                                        }
                                        Err(e) => ctx.push_toast(ToastLevel::Error, e),
                                    }
                                });
                            }),
                        }));
                    },
                    Icon { name: "check", size: 14 } " " {i18n::t(locale, "btn_save")}
                }
            }
        }

        // IPv6 + MTU panel (oxvif 0.9.8 NetworkInterfaceConfig).
        div { class: "prop-section-header", {i18n::t(locale, "net_ipv6")} }
        div { class: "id-edit-form",
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_ipv6_enabled")} }
                input {
                    r#type: "checkbox",
                    checked: "{*ipv6_enabled_in.read()}",
                    oninput: move |e| ipv6_enabled_in.clone().set(e.value() == "true"),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_ipv6_dhcp")} }
                input {
                    r#type: "checkbox",
                    checked: "{*ipv6_dhcp_in.read()}",
                    oninput: move |e| ipv6_dhcp_in.clone().set(e.value() == "true"),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_ipv6_manual")} }
                input {
                    class: "id-edit-input",
                    r#type: "text",
                    disabled: "{*ipv6_dhcp_in.read()}",
                    title: if *ipv6_dhcp_in.read() { i18n::t(locale, "net_disabled_by_dhcp") } else { "" },
                    value: "{ipv6_address_in.read().clone()}",
                    oninput: move |e| ipv6_address_in.clone().set(e.value()),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_iface_prefix")} }
                input {
                    class: "id-edit-input",
                    r#type: "number",
                    min: "1", max: "128",
                    disabled: "{*ipv6_dhcp_in.read()}",
                    value: "{ipv6_prefix_in.read().clone()}",
                    oninput: move |e| ipv6_prefix_in.clone().set(e.value()),
                }
            }
            div { class: "id-edit-row",
                label { class: "id-edit-label", {i18n::t(locale, "net_mtu")} }
                input {
                    class: "id-edit-input",
                    r#type: "number",
                    min: "576", max: "9000",
                    value: "{mtu_in.read().clone()}",
                    oninput: move |e| mtu_in.clone().set(e.value()),
                }
            }
            div { class: "id-edit-actions",
                button {
                    class: "btn btn-md btn-primary",
                    onclick: move |_| {
                        let tok = token_for_save_v6.clone();
                        let v4_enabled = *enabled_in.peek();
                        let v4_dhcp = *dhcp_in.peek();
                        let v4_ip = addr_in.peek().clone();
                        let v4_prefix: u32 = prefix_in.peek().parse().unwrap_or(24);
                        let v6_enabled = *ipv6_enabled_in.peek();
                        let v6_dhcp = *ipv6_dhcp_in.peek();
                        let v6_ip = ipv6_address_in.peek().clone();
                        let v6_prefix: u32 = ipv6_prefix_in.peek().parse().unwrap_or(64);
                        let mtu_val: u32 = mtu_in.peek().parse().unwrap_or(0);
                        ctx.dialog.clone().set(Some(ConfirmDialog {
                            title: i18n::t(locale, "net_iface_confirm_title").to_string(),
                            message: i18n::t(locale, "net_iface_confirm_msg").to_string(),
                            confirm_label: i18n::t(locale, "btn_confirm").to_string(),
                            cancel_label: i18n::t(locale, "btn_cancel").to_string(),
                            dangerous: true,
                            on_confirm: EventHandler::new(move |_| {
                                let tok = tok.clone();
                                let v4_ip = v4_ip.clone();
                                let v6_ip = v6_ip.clone();
                                let addr_s = addr.read().clone();
                                let creds_s = creds.read().clone();
                                spawn(async move {
                                    let cfg = oxvif::NetworkInterfaceConfig {
                                        enabled: *enabled_in.peek(),
                                        mtu: if mtu_val > 0 { Some(mtu_val) } else { None },
                                        ipv4: Some(oxvif::IpStackConfig {
                                            enabled: v4_enabled,
                                            from_dhcp: v4_dhcp,
                                            manual: if v4_ip.is_empty() {
                                                vec![]
                                            } else {
                                                vec![oxvif::ManualAddress {
                                                    address: v4_ip,
                                                    prefix_length: v4_prefix,
                                                }]
                                            },
                                        }),
                                        ipv6: Some(oxvif::IpStackConfig {
                                            enabled: v6_enabled,
                                            from_dhcp: v6_dhcp,
                                            manual: if v6_ip.is_empty() {
                                                vec![]
                                            } else {
                                                vec![oxvif::ManualAddress {
                                                    address: v6_ip,
                                                    prefix_length: v6_prefix,
                                                }]
                                            },
                                        }),
                                    };
                                    match api::set_network_interfaces_full(
                                        &addr_s, &creds_s, &tok, &cfg,
                                    ).await {
                                        Ok(reboot) => {
                                            let key = if reboot { "net_saved_reboot" } else { "net_saved" };
                                            ctx.push_toast(ToastLevel::Success, i18n::t(locale, key));
                                            refresh.call(());
                                        }
                                        Err(e) => ctx.push_toast(ToastLevel::Error, e),
                                    }
                                });
                            }),
                        }));
                    },
                    Icon { name: "check", size: 14 } " " {i18n::t(locale, "net_save_v6")}
                }
            }
        }
    }
}

// ── Gateway ───────────────────────────────────────────────────────────────

#[component]
fn GatewaySection(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    refresh: Callback<()>,
    current: Vec<String>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let initial = if current.is_empty() {
        vec![String::new()]
    } else {
        current
    };
    let servers = use_signal(|| initial);

    rsx! {
        div { class: "prop-section-header", {i18n::t(locale, "net_gateway")} }
        ServerList { servers }
        div { class: "id-edit-actions",
            button {
                class: "btn btn-md btn-primary",
                onclick: move |_| {
                    let addr_s = addr.read().clone();
                    let creds_s = creds.read().clone();
                    let list: Vec<String> = servers
                        .peek()
                        .iter()
                        .filter(|s| !s.trim().is_empty())
                        .cloned()
                        .collect();
                    spawn(async move {
                        match api::set_network_default_gateway(&addr_s, &creds_s, &list).await {
                            Ok(()) => {
                                ctx.push_toast(ToastLevel::Success, i18n::t(locale, "net_saved"));
                                refresh.call(());
                            }
                            Err(e) => ctx.push_toast(ToastLevel::Error, e),
                        }
                    });
                },
                Icon { name: "check", size: 14 } " " {i18n::t(locale, "btn_save")}
            }
        }
    }
}

// ── DNS / NTP shared section ─────────────────────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum ServerKind {
    Dns,
    Ntp,
}

#[component]
fn ServerGroupSection(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    refresh: Callback<()>,
    header: &'static str,
    current_servers: Vec<String>,
    from_dhcp: bool,
    kind: ServerKind,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    let dhcp_in = use_signal(|| from_dhcp);
    let initial = if current_servers.is_empty() {
        vec![String::new()]
    } else {
        current_servers
    };
    let servers = use_signal(|| initial);

    rsx! {
        div { class: "prop-section-header", "{header}" }
        div { class: "id-edit-form",
            div { class: "id-edit-row",
                label { class: "id-edit-label", "DHCP" }
                input {
                    r#type: "checkbox",
                    checked: "{*dhcp_in.read()}",
                    oninput: move |e| dhcp_in.clone().set(e.value() == "true"),
                }
            }
        }
        if !*dhcp_in.read() {
            ServerList { servers }
        }
        div { class: "id-edit-actions",
            button {
                class: "btn btn-md btn-primary",
                onclick: move |_| {
                    let dhcp = *dhcp_in.peek();
                    let list: Vec<String> = servers
                        .peek()
                        .iter()
                        .filter(|s| !s.trim().is_empty())
                        .cloned()
                        .collect();
                    let addr_s = addr.read().clone();
                    let creds_s = creds.read().clone();
                    spawn(async move {
                        let result = match kind {
                            ServerKind::Dns => api::set_dns(&addr_s, &creds_s, dhcp, &list).await,
                            ServerKind::Ntp => api::set_ntp(&addr_s, &creds_s, dhcp, &list).await,
                        };
                        match result {
                            Ok(()) => {
                                ctx.push_toast(ToastLevel::Success, i18n::t(locale, "net_saved"));
                                refresh.call(());
                            }
                            Err(e) => ctx.push_toast(ToastLevel::Error, e),
                        }
                    });
                },
                Icon { name: "check", size: 14 } " " {i18n::t(locale, "btn_save")}
            }
        }
    }
}

/// Editable list of server addresses with add / remove buttons. Owned by
/// the caller via `Signal<Vec<String>>` so save logic reads the final state.
#[component]
fn ServerList(servers: Signal<Vec<String>>) -> Element {
    let count = servers.read().len();
    rsx! {
        div { class: "id-edit-form",
            for i in 0..count {
                div {
                    class: "id-edit-row",
                    key: "{i}",
                    label { class: "id-edit-label", "{i + 1}" }
                    input {
                        class: "id-edit-input",
                        r#type: "text",
                        value: "{servers.read()[i]}",
                        oninput: move |e| {
                            if let Some(v) = servers.write().get_mut(i) {
                                *v = e.value();
                            }
                        },
                    }
                    button {
                        class: "user-row-btn user-row-btn--danger",
                        onclick: move |_| {
                            let mut guard = servers.write();
                            if i < guard.len() { guard.remove(i); }
                        },
                        Icon { name: "x", size: 12 }
                    }
                }
            }
            div { class: "id-edit-actions",
                button {
                    class: "btn btn-sm",
                    onclick: move |_| servers.write().push(String::new()),
                    Icon { name: "plus", size: 12 }
                }
            }
        }
    }
}

// ── Protocols ─────────────────────────────────────────────────────────────

#[component]
fn ProtocolsSection(
    addr: ReadSignal<String>,
    creds: Memo<Credentials>,
    refresh: Callback<()>,
    /// Pre-converted: (name, enabled, port_csv).
    entries: Vec<(String, bool, String)>,
) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();

    if entries.is_empty() {
        return rsx! {};
    }
    let mut items = use_signal(|| entries);

    rsx! {
        div { class: "prop-section-header", {i18n::t(locale, "net_protocols")} }
        div { class: "id-edit-form",
            for i in 0..items.read().len() {
                {
                    let row = items.read()[i].clone();
                    rsx! {
                        div {
                            class: "id-edit-row",
                            key: "{row.0}",
                            label { class: "id-edit-label", "{row.0}" }
                            input {
                                r#type: "checkbox",
                                checked: "{row.1}",
                                oninput: move |e| {
                                    if let Some(entry) = items.write().get_mut(i) {
                                        entry.1 = e.value() == "true";
                                    }
                                },
                            }
                            input {
                                class: "id-edit-input",
                                r#type: "text",
                                placeholder: "80",
                                value: "{row.2}",
                                oninput: move |e| {
                                    if let Some(entry) = items.write().get_mut(i) {
                                        entry.2 = e.value();
                                    }
                                },
                            }
                        }
                    }
                }
            }
            div { class: "id-edit-actions",
                button {
                    class: "btn btn-md btn-primary",
                    onclick: move |_| {
                        let addr_s = addr.read().clone();
                        let creds_s = creds.read().clone();
                        let payload: Vec<(String, bool, Vec<u32>)> = items
                            .peek()
                            .iter()
                            .map(|(name, enabled, port_csv)| {
                                let ports: Vec<u32> = port_csv
                                    .split(',')
                                    .filter_map(|p| p.trim().parse().ok())
                                    .collect();
                                (name.clone(), *enabled, ports)
                            })
                            .collect();
                        spawn(async move {
                            match api::set_network_protocols(&addr_s, &creds_s, &payload).await {
                                Ok(()) => {
                                    ctx.push_toast(ToastLevel::Success, i18n::t(locale, "net_saved"));
                                    refresh.call(());
                                }
                                Err(e) => ctx.push_toast(ToastLevel::Error, e),
                            }
                        });
                    },
                    Icon { name: "check", size: 14 } " " {i18n::t(locale, "btn_save")}
                }
            }
        }
    }
}

fn yn(locale: crate::state::Locale, b: bool) -> String {
    if b {
        i18n::t(locale, "yes")
    } else {
        i18n::t(locale, "no")
    }
    .to_string()
}
