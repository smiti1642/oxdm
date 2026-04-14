//! Multi-NIC WS-Discovery probe.
//!
//! oxvif's `discovery::probe()` binds to `0.0.0.0` — the OS picks one
//! interface for multicast. On multi-NIC setups (VLAN, Docker, VPN) cameras
//! on other subnets are never reached.
//!
//! This module iterates all IPv4 interfaces and sends a WS-Discovery Probe
//! on each, then merges and deduplicates results.

use oxvif::DiscoveredDevice;
use std::collections::HashSet;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};
use std::time::Duration;
use tracing::{debug, info, warn};

const MULTICAST_ADDR: Ipv4Addr = Ipv4Addr::new(239, 255, 255, 250);
const MULTICAST_PORT: u16 = 3702;
const RECV_BUF_SIZE: usize = 65535;

/// Probe on all IPv4 interfaces, multiple rounds, deduplicated.
pub async fn probe_all_interfaces(
    rounds: usize,
    timeout_per_round: Duration,
    interval: Duration,
) -> Vec<DiscoveredDevice> {
    let addrs = list_ipv4_addrs();
    if addrs.is_empty() {
        warn!("No IPv4 interfaces found for WS-Discovery");
        return Vec::new();
    }

    info!(
        interfaces = addrs.len(),
        rounds,
        timeout_ms = timeout_per_round.as_millis() as u64,
        "Multi-NIC WS-Discovery starting"
    );

    let mut seen = HashSet::new();
    let mut all = Vec::new();

    for round in 0..rounds {
        // Probe all interfaces in parallel using blocking tasks
        let mut handles = Vec::new();
        for &addr in &addrs {
            let timeout = timeout_per_round;
            handles.push(tokio::task::spawn_blocking(move || {
                probe_on_interface(addr, timeout)
            }));
        }

        let mut round_new = 0;
        for handle in handles {
            if let Ok(devices) = handle.await {
                for dev in devices {
                    if seen.insert(dev.endpoint.clone()) {
                        round_new += 1;
                        all.push(dev);
                    }
                }
            }
        }

        debug!(
            round = round + 1,
            new = round_new,
            total = all.len(),
            "Probe round complete"
        );

        if round + 1 < rounds {
            tokio::time::sleep(interval).await;
        }
    }

    info!(
        total = all.len(),
        interfaces = addrs.len(),
        "Multi-NIC WS-Discovery complete"
    );
    all
}

/// List all non-loopback IPv4 addresses on this machine.
fn list_ipv4_addrs() -> Vec<Ipv4Addr> {
    let ifaces = match if_addrs::get_if_addrs() {
        Ok(v) => v,
        Err(e) => {
            warn!(error = %e, "Failed to list network interfaces");
            return Vec::new();
        }
    };

    let addrs: Vec<Ipv4Addr> = ifaces
        .into_iter()
        .filter_map(|i| {
            if i.is_loopback() {
                return None;
            }
            match i.addr.ip() {
                std::net::IpAddr::V4(v4) => {
                    debug!(iface = %i.name, addr = %v4, "Found IPv4 interface");
                    Some(v4)
                }
                _ => None,
            }
        })
        .collect();

    addrs
}

/// Send a WS-Discovery Probe on a specific interface and collect responses.
fn probe_on_interface(bind_addr: Ipv4Addr, timeout: Duration) -> Vec<DiscoveredDevice> {
    let socket = match create_multicast_socket(bind_addr) {
        Ok(s) => s,
        Err(e) => {
            debug!(addr = %bind_addr, error = %e, "Failed to create socket");
            return Vec::new();
        }
    };

    let probe_xml = build_probe_xml();

    let dest = SocketAddrV4::new(MULTICAST_ADDR, MULTICAST_PORT);
    if let Err(e) = socket.send_to(probe_xml.as_bytes(), dest) {
        debug!(addr = %bind_addr, error = %e, "Failed to send probe");
        return Vec::new();
    }

    // Collect responses until timeout
    let mut devices = Vec::new();
    let mut buf = vec![0u8; RECV_BUF_SIZE];
    let deadline = std::time::Instant::now() + timeout;

    loop {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }

        if socket.set_read_timeout(Some(remaining)).is_err() {
            break;
        }

        match socket.recv_from(&mut buf) {
            Ok((len, _src)) => {
                if let Ok(xml) = std::str::from_utf8(&buf[..len]) {
                    devices.extend(parse_probe_matches(xml));
                }
            }
            Err(e) => {
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut
                {
                    break; // Timeout — done collecting
                }
                // Other error — stop
                break;
            }
        }
    }

    debug!(
        addr = %bind_addr,
        found = devices.len(),
        "Interface probe done"
    );
    devices
}

fn create_multicast_socket(bind_addr: Ipv4Addr) -> std::io::Result<UdpSocket> {
    let socket = UdpSocket::bind(SocketAddrV4::new(bind_addr, 0))?;
    socket.join_multicast_v4(&MULTICAST_ADDR, &bind_addr)?;
    socket.set_multicast_ttl_v4(4)?;
    // Allow multiple sockets on same port (for parallel probes)
    // Not strictly needed since we bind to different IPs, but safe
    Ok(socket)
}

// ── Probe XML ───────────────────────────────────────────────────────────────

fn build_probe_xml() -> String {
    let uuid = generate_uuid();
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<s:Envelope xmlns:s="http://www.w3.org/2003/05/soap-envelope"
            xmlns:a="http://schemas.xmlsoap.org/ws/2004/08/addressing"
            xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery"
            xmlns:dn="http://www.onvif.org/ver10/network/wsdl">
  <s:Header>
    <a:Action s:mustUnderstand="1">http://schemas.xmlsoap.org/ws/2005/04/discovery/Probe</a:Action>
    <a:MessageID>uuid:{uuid}</a:MessageID>
    <a:ReplyTo>
      <a:Address>http://schemas.xmlsoap.org/ws/2004/08/addressing/role/anonymous</a:Address>
    </a:ReplyTo>
    <a:To s:mustUnderstand="1">urn:schemas-xmlsoap-org:ws:2005:04:discovery</a:To>
  </s:Header>
  <s:Body>
    <d:Probe>
      <d:Types>dn:NetworkVideoTransmitter</d:Types>
    </d:Probe>
  </s:Body>
</s:Envelope>"#
    )
}

fn generate_uuid() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    let rand = ts.wrapping_mul(6364136223846793005).wrapping_add(1);
    format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        (rand >> 96) as u32,
        (rand >> 80) as u16,
        (rand >> 64) as u16 & 0x0FFF | 0x4000,
        (rand >> 48) as u16 & 0x3FFF | 0x8000,
        (pid as u64) << 16 | (rand & 0xFFFF_FFFF_FFFF) as u64,
    )
}

// ── Response parsing ────────────────────────────────────────────────────────

/// Parse WS-Discovery ProbeMatch responses into DiscoveredDevice entries.
///
/// Handles various namespace prefixes used by different camera vendors
/// (d:, wsdd:, wsd:, etc.) by matching on local tag names only.
fn parse_probe_matches(xml: &str) -> Vec<DiscoveredDevice> {
    let mut devices = Vec::new();

    // Find each ProbeMatch block (not ProbeMatches)
    for block in extract_blocks(xml, "ProbeMatch") {
        let endpoint = extract_first_tag(&block, "Address").unwrap_or_default();
        if endpoint.is_empty() {
            continue;
        }

        let types_str = extract_first_tag(&block, "Types").unwrap_or_default();
        let scopes_str = extract_first_tag(&block, "Scopes").unwrap_or_default();
        let xaddrs_str = extract_first_tag(&block, "XAddrs").unwrap_or_default();

        let types = split_ws(&types_str);
        let scopes = split_ws(&scopes_str);
        let xaddrs = split_ws(&xaddrs_str);

        if !xaddrs.is_empty() {
            devices.push(DiscoveredDevice {
                endpoint,
                types,
                scopes,
                xaddrs,
            });
        }
    }

    devices
}

fn split_ws(s: &str) -> Vec<String> {
    s.split_whitespace()
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

/// Extract all blocks delimited by a tag with the given local name.
/// Handles namespace prefixes: `<d:ProbeMatch>`, `<wsdd:ProbeMatch>`, `<ProbeMatch>`.
/// Does NOT match `<ProbeMatches>` when looking for `ProbeMatch`.
fn extract_blocks(xml: &str, local_name: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut search_from = 0;

    while search_from < xml.len() {
        // Find opening tag: something like <d:ProbeMatch> or <ProbeMatch>
        let open = match find_open_tag(&xml[search_from..], local_name) {
            Some((start, end)) => (search_from + start, search_from + end),
            None => break,
        };

        // Find the corresponding closing tag after the opening tag
        let close = match find_close_tag(&xml[open.1..], local_name) {
            Some(pos) => open.1 + pos,
            None => break,
        };

        blocks.push(xml[open.1..close].to_string());
        search_from = close;
    }

    blocks
}

/// Find an opening tag like `<d:Name>` or `<Name>` (not `<d:NameSuffix>` or `</d:Name>`).
/// Returns (start_of_<, end_of_>) positions relative to the input.
fn find_open_tag(xml: &str, local_name: &str) -> Option<(usize, usize)> {
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        let lt = rest.find('<')?;
        let abs_lt = pos + lt;
        let after_lt = &xml[abs_lt + 1..];

        // Skip closing tags and processing instructions
        if after_lt.starts_with('/') || after_lt.starts_with('?') || after_lt.starts_with('!') {
            pos = abs_lt + 1;
            continue;
        }

        // Find the end of this tag
        let gt = match after_lt.find('>') {
            Some(p) => p,
            None => break,
        };
        let tag_content = &after_lt[..gt]; // e.g. "d:ProbeMatch" or "ProbeMatch attr=..."

        // Extract the tag name (before any attributes)
        let tag_name = tag_content.split_whitespace().next().unwrap_or("");

        // Get local part (after last ':')
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);

        // Must match exactly (not "ProbeMatches" when looking for "ProbeMatch")
        if local == local_name {
            return Some((abs_lt, abs_lt + 1 + gt + 1));
        }

        pos = abs_lt + 1;
    }
    None
}

/// Find a closing tag like `</d:Name>` or `</Name>`.
/// Returns the position of the '<' of the closing tag, relative to input.
fn find_close_tag(xml: &str, local_name: &str) -> Option<usize> {
    let mut pos = 0;
    while pos < xml.len() {
        let rest = &xml[pos..];
        // Look for </
        let lt = rest.find("</")?;
        let abs_lt = pos + lt;
        let after_close = &xml[abs_lt + 2..];

        let gt = after_close.find('>')?;
        let tag_name = after_close[..gt].trim();
        let local = tag_name.rsplit(':').next().unwrap_or(tag_name);

        if local == local_name {
            return Some(abs_lt);
        }

        pos = abs_lt + 2;
    }
    None
}

/// Extract text content of the first tag with the given local name.
fn extract_first_tag(xml: &str, local_name: &str) -> Option<String> {
    let blocks = extract_blocks(xml, local_name);
    blocks.into_iter().next().map(|s| s.trim().to_string())
}

// ── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_RESPONSE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<SOAP-ENV:Envelope xmlns:SOAP-ENV="http://www.w3.org/2003/05/soap-envelope"
                   xmlns:wsa="http://schemas.xmlsoap.org/ws/2004/08/addressing"
                   xmlns:d="http://schemas.xmlsoap.org/ws/2005/04/discovery"
                   xmlns:dn="http://www.onvif.org/ver10/network/wsdl">
  <SOAP-ENV:Body>
    <d:ProbeMatches>
      <d:ProbeMatch>
        <wsa:EndpointReference>
          <wsa:Address>urn:uuid:abcd-1234-efgh-5678</wsa:Address>
        </wsa:EndpointReference>
        <d:Types>dn:NetworkVideoTransmitter</d:Types>
        <d:Scopes>onvif://www.onvif.org/name/TestCam onvif://www.onvif.org/type/video_encoder</d:Scopes>
        <d:XAddrs>http://192.168.1.100/onvif/device_service</d:XAddrs>
      </d:ProbeMatch>
    </d:ProbeMatches>
  </SOAP-ENV:Body>
</SOAP-ENV:Envelope>"#;

    #[test]
    fn parse_standard_response() {
        let devices = parse_probe_matches(SAMPLE_RESPONSE);
        assert_eq!(devices.len(), 1);
        let dev = &devices[0];
        assert_eq!(dev.endpoint, "urn:uuid:abcd-1234-efgh-5678");
        assert_eq!(
            dev.xaddrs,
            vec!["http://192.168.1.100/onvif/device_service"]
        );
        assert_eq!(dev.scopes.len(), 2);
        assert!(dev.scopes[0].contains("name/TestCam"));
    }

    #[test]
    fn parse_multiple_xaddrs() {
        let xml = r#"<d:ProbeMatch>
            <wsa:EndpointReference><wsa:Address>urn:uuid:1111</wsa:Address></wsa:EndpointReference>
            <d:Types>dn:NetworkVideoTransmitter</d:Types>
            <d:Scopes></d:Scopes>
            <d:XAddrs>http://10.0.0.1/onvif/device_service http://192.168.1.1/onvif/device_service</d:XAddrs>
        </d:ProbeMatch>"#;
        let devices = parse_probe_matches(xml);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].xaddrs.len(), 2);
    }

    #[test]
    fn parse_empty_xaddrs_skipped() {
        let xml = r#"<d:ProbeMatch>
            <wsa:EndpointReference><wsa:Address>urn:uuid:2222</wsa:Address></wsa:EndpointReference>
            <d:Types>dn:NetworkVideoTransmitter</d:Types>
            <d:Scopes></d:Scopes>
            <d:XAddrs></d:XAddrs>
        </d:ProbeMatch>"#;
        let devices = parse_probe_matches(xml);
        assert_eq!(devices.len(), 0, "Empty xaddrs should be skipped");
    }

    #[test]
    fn list_ipv4_finds_something() {
        // On any machine, there should be at least one non-loopback IPv4 address
        let addrs = list_ipv4_addrs();
        // This might fail in very unusual CI environments, but should work locally
        assert!(!addrs.is_empty(), "Should find at least one IPv4 interface");
    }

    #[test]
    fn probe_xml_is_valid() {
        let xml = build_probe_xml();
        assert!(xml.contains("Probe"));
        assert!(xml.contains("NetworkVideoTransmitter"));
        assert!(xml.contains("uuid:"));
    }
}
