#![allow(non_snake_case)]
use crate::components::PropRow;
use crate::state::Credentials;
use crate::{api, i18n, state::Ctx};
use dioxus::prelude::*;

#[derive(Debug)]
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

    let info = use_resource(move || {
        let addr = addr.read().clone();
        let creds = creds.read().clone();
        async move {
            let (u, p) = creds.as_options();
            let hostname = api::get_hostname(&addr, u, p).await.ok();
            let ifaces = api::get_network_interfaces(&addr, u, p)
                .await
                .unwrap_or_default();
            let dns = api::get_dns(&addr, u, p).await.ok();
            let ntp = api::get_ntp(&addr, u, p).await.ok();
            let gw = api::get_network_default_gateway(&addr, u, p).await.ok();
            let protocols = api::get_network_protocols(&addr, u, p)
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

    rsx! {
        match &*info.read_unchecked() {
            None => rsx! { div { class: "tab-loading", {i18n::t(locale, "loading")} } },
            Some(Err(e)) => rsx! { div { class: "tab-error", "{e}" } },
            Some(Ok(data)) => rsx! {
                div { class: "prop-section-header", {i18n::t(locale, "net_hostname")} }
                table { class: "prop-table",
                    PropRow { label: i18n::t(locale, "net_hostname"), value: data.hostname.clone().unwrap_or("N/A".into()) }
                    PropRow { label: "DHCP", value: yn(locale, data.hostname_dhcp) }
                }
                for iface in &data.interfaces {
                    div { class: "prop-section-header",
                        {format!("{} ({})", i18n::t(locale, "net_interface"), &iface.name)}
                    }
                    table { class: "prop-table",
                        PropRow { label: "Token",   value: iface.token.clone() }
                        PropRow { label: "MAC",     value: iface.hw_address.clone() }
                        PropRow { label: "MTU",     value: iface.mtu.to_string() }
                        PropRow { label: "Enabled", value: yn(locale, iface.enabled) }
                        PropRow { label: "IPv4",    value: iface.ipv4_address.clone() }
                        PropRow { label: "Prefix",  value: format!("/{}", iface.ipv4_prefix_length) }
                        PropRow { label: "DHCP",    value: yn(locale, iface.ipv4_from_dhcp) }
                        if iface.ipv6_enabled {
                            PropRow { label: "IPv6", value: iface.ipv6_address.clone().unwrap_or("N/A".into()) }
                        }
                    }
                }
                if !data.gateways.is_empty() {
                    div { class: "prop-section-header", {i18n::t(locale, "net_gateway")} }
                    table { class: "prop-table",
                        for gw in &data.gateways {
                            PropRow { label: "Gateway", value: gw.clone() }
                        }
                    }
                }
                div { class: "prop-section-header", "DNS" }
                table { class: "prop-table",
                    PropRow { label: "DHCP", value: yn(locale, data.dns_from_dhcp) }
                    for (i, srv) in data.dns_servers.iter().enumerate() {
                        PropRow { label: format!("Server {}", i + 1), value: srv.clone() }
                    }
                }
                div { class: "prop-section-header", "NTP" }
                table { class: "prop-table",
                    PropRow { label: "DHCP", value: yn(locale, data.ntp_from_dhcp) }
                    for (i, srv) in data.ntp_servers.iter().enumerate() {
                        PropRow { label: format!("Server {}", i + 1), value: srv.clone() }
                    }
                }
                if !data.protocols.is_empty() {
                    div { class: "prop-section-header", {i18n::t(locale, "net_protocols")} }
                    table { class: "prop-table",
                        for proto in &data.protocols {
                            PropRow {
                                label: proto.name.clone(),
                                value: format!(
                                    "{} — port {}",
                                    if proto.enabled { i18n::t(locale, "enabled") } else { i18n::t(locale, "disabled") },
                                    proto.ports.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(", "),
                                ),
                            }
                        }
                    }
                }
            },
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
