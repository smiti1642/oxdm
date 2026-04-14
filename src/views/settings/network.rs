#![allow(non_snake_case)]
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
pub fn NetworkTab(addr: String) -> Element {
    let ctx = use_context::<Ctx>();
    let locale = *ctx.locale.read();
    let creds = ctx.global_credentials.read().clone();

    let info = use_resource(move || {
        let addr = addr.clone();
        let u = creds.username.clone();
        let p = creds.password.clone();
        async move {
            let (user, pass) = if u.is_empty() {
                (None, None)
            } else {
                (Some(u.as_str()), Some(p.as_str()))
            };

            let hostname = api::get_hostname(&addr, user, pass).await.ok();
            let ifaces = api::get_network_interfaces(&addr, user, pass)
                .await
                .unwrap_or_default();
            let dns = api::get_dns(&addr, user, pass).await.ok();
            let ntp = api::get_ntp(&addr, user, pass).await.ok();
            let gw = api::get_network_default_gateway(&addr, user, pass)
                .await
                .ok();
            let protocols = api::get_network_protocols(&addr, user, pass)
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
            None => rsx! {
                div { class: "tab-loading", {i18n::t(locale, "loading")} }
            },
            Some(Err(e)) => rsx! {
                div { class: "tab-error", "{e}" }
            },
            Some(Ok(data)) => rsx! {
                // Hostname
                div { class: "prop-section-header", {i18n::t(locale, "net_hostname")} }
                table { class: "prop-table",
                    PropRow {
                        label: i18n::t(locale, "net_hostname"),
                        value: data.hostname.clone().unwrap_or_else(|| "N/A".to_string()),
                    }
                    PropRow {
                        label: "DHCP",
                        value: yn(data.hostname_dhcp),
                    }
                }

                // Interfaces
                for iface in &data.interfaces {
                    div { class: "prop-section-header",
                        {format!("{} ({})", i18n::t(locale, "net_interface"), &iface.name)}
                    }
                    table { class: "prop-table",
                        PropRow { label: "Token",      value: iface.token.clone() }
                        PropRow { label: "MAC",        value: iface.hw_address.clone() }
                        PropRow { label: "MTU",        value: iface.mtu.to_string() }
                        PropRow { label: "Enabled",    value: yn(iface.enabled) }
                        PropRow { label: "IPv4",       value: iface.ipv4_address.clone() }
                        PropRow { label: "Prefix",     value: format!("/{}", iface.ipv4_prefix_length) }
                        PropRow { label: "DHCP",       value: yn(iface.ipv4_from_dhcp) }
                        if iface.ipv6_enabled {
                            PropRow {
                                label: "IPv6",
                                value: iface.ipv6_address.clone().unwrap_or_else(|| "N/A".to_string()),
                            }
                        }
                    }
                }

                // Gateway
                if !data.gateways.is_empty() {
                    div { class: "prop-section-header", {i18n::t(locale, "net_gateway")} }
                    table { class: "prop-table",
                        for gw in &data.gateways {
                            PropRow { label: "Gateway", value: gw.clone() }
                        }
                    }
                }

                // DNS
                div { class: "prop-section-header", "DNS" }
                table { class: "prop-table",
                    PropRow { label: "DHCP", value: yn(data.dns_from_dhcp) }
                    for (i, srv) in data.dns_servers.iter().enumerate() {
                        PropRow { label: format!("Server {}", i + 1), value: srv.clone() }
                    }
                }

                // NTP
                div { class: "prop-section-header", "NTP" }
                table { class: "prop-table",
                    PropRow { label: "DHCP", value: yn(data.ntp_from_dhcp) }
                    for (i, srv) in data.ntp_servers.iter().enumerate() {
                        PropRow { label: format!("Server {}", i + 1), value: srv.clone() }
                    }
                }

                // Protocols
                if !data.protocols.is_empty() {
                    div { class: "prop-section-header", {i18n::t(locale, "net_protocols")} }
                    table { class: "prop-table",
                        for proto in &data.protocols {
                            PropRow {
                                label: proto.name.clone(),
                                value: format!(
                                    "{} — port {}",
                                    if proto.enabled { "Enabled" } else { "Disabled" },
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

#[component]
fn PropRow(label: String, value: String) -> Element {
    rsx! {
        tr {
            td { class: "prop-label", "{label}" }
            td { class: "prop-value", "{value}" }
        }
    }
}

fn yn(b: bool) -> String {
    if b { "Yes" } else { "No" }.to_string()
}
