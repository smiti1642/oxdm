//! Shared utility functions.

/// A local-time filename stamp, `YYYYMMDD-HHMMSS` — for export filenames so the
/// user never has to rename (health reports and quirk exports share it). Falls
/// back to UTC if the local offset can't be determined.
pub(crate) fn now_file_stamp() -> String {
    use time::OffsetDateTime;
    let t = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        t.year(),
        u8::from(t.month()),
        t.day(),
        t.hour(),
        t.minute(),
        t.second(),
    )
}

/// Extract the IP address (or host) from an ONVIF device service URL.
///
/// Examples:
/// - `http://192.168.1.10/onvif/device_service` → `192.168.1.10`
/// - `http://192.168.1.10:8080/onvif/device_service` → `192.168.1.10`
/// - `192.168.1.10` → `192.168.1.10`
pub fn extract_ip(addr: &str) -> String {
    let stripped = addr
        .strip_prefix("http://")
        .or_else(|| addr.strip_prefix("https://"))
        .unwrap_or(addr);
    stripped
        .split('/')
        .next()
        .and_then(|h| h.split(':').next())
        .unwrap_or(addr)
        .to_string()
}

/// Decode percent-encoded UTF-8 strings (e.g. from ONVIF scopes).
///
/// Handles multi-byte UTF-8 sequences correctly (e.g. `%C3%A9` → `é`).
pub fn urldecode(s: &str) -> String {
    let mut bytes = Vec::with_capacity(s.len());
    let mut chars = s.bytes().peekable();
    while let Some(b) = chars.next() {
        if b == b'%' {
            let hi = chars.next().unwrap_or(0);
            let lo = chars.next().unwrap_or(0);
            let hex = [hi, lo];
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(&hex).unwrap_or(""), 16) {
                bytes.push(val);
            }
        } else if b == b'+' {
            bytes.push(b' ');
        } else {
            bytes.push(b);
        }
    }
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Validation outcome for the Add Device address field. Lets the dialog
/// show a specific reason inline instead of a generic "looks bad" hint
/// (and keeps the "what counts as a valid address" rule in one place).
#[derive(Debug, PartialEq, Eq)]
pub enum AddrError {
    Empty,
    InvalidIpOctet,
    InvalidPort,
    InvalidHostname,
    NoHost,
}

impl AddrError {
    /// i18n key for the message shown under the input field.
    pub fn i18n_key(&self) -> &'static str {
        match self {
            Self::Empty => "addr_err_empty",
            Self::InvalidIpOctet => "addr_err_ip",
            Self::InvalidPort => "addr_err_port",
            Self::InvalidHostname => "addr_err_hostname",
            Self::NoHost => "addr_err_no_host",
        }
    }
}

/// Validate a device address string.
///
/// Accepts:
/// - Bare IPv4: `192.168.1.10`
/// - IPv4 with port: `192.168.1.10:8080`
/// - With path: `192.168.1.10/onvif/device`
/// - With scheme: `http://192.168.1.10/onvif/device`
/// - Hostname: `camera.local`, `cam-01`
///
/// Permissive on accepted shape, strict on the parts: 0–255 octets,
/// 1–65535 ports, RFC1123-ish hostname chars only.
pub fn validate_device_addr(input: &str) -> Result<(), AddrError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(AddrError::Empty);
    }
    // Strip scheme.
    let rest = trimmed
        .strip_prefix("http://")
        .or_else(|| trimmed.strip_prefix("https://"))
        .unwrap_or(trimmed);
    // Strip path.
    let host_port = rest.split('/').next().unwrap_or("");
    if host_port.is_empty() {
        return Err(AddrError::NoHost);
    }
    // Split host:port.
    let (host, port_opt) = match host_port.rsplit_once(':') {
        Some((h, p)) => (h, Some(p)),
        None => (host_port, None),
    };
    if host.is_empty() {
        return Err(AddrError::NoHost);
    }
    if let Some(port) = port_opt {
        match port.parse::<u32>() {
            Ok(p) if (1..=65535).contains(&p) => {}
            _ => return Err(AddrError::InvalidPort),
        }
    }
    // If host parses as 4 dotted octets, validate as IPv4. Otherwise
    // validate as hostname (RFC1123: letters, digits, hyphens, dots).
    let parts: Vec<&str> = host.split('.').collect();
    let looks_like_ipv4 =
        parts.len() == 4 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit()));
    if looks_like_ipv4 {
        for p in &parts {
            match p.parse::<u32>() {
                Ok(o) if o <= 255 => {}
                _ => return Err(AddrError::InvalidIpOctet),
            }
        }
        Ok(())
    } else {
        let valid = host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.');
        if valid && !host.starts_with('-') && !host.ends_with('-') {
            Ok(())
        } else {
            Err(AddrError::InvalidHostname)
        }
    }
}

/// Copy text to the system clipboard. Returns `Ok(())` or an error message.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text))
        .map_err(|e| e.to_string())
}

/// Strip non-filesystem-safe characters and collapse whitespace so the
/// suggested filename in a Save dialog doesn't get rejected on Windows
/// (which forbids `<>:"/\|?*`).
pub fn sanitize_filename(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "snapshot".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Decode a `data:image/jpeg;base64,...` URI into raw JPEG bytes.
/// Returns `None` if the URI doesn't have the expected prefix or the
/// base64 payload doesn't decode.
pub fn decode_jpeg_data_uri(uri: &str) -> Option<Vec<u8>> {
    use base64::{engine::general_purpose::STANDARD, Engine as _};
    // We accept both "image/jpeg" and "image/jpg" since some cameras
    // (and our own snapshot fetcher) have shipped both at various points.
    let payload = uri
        .strip_prefix("data:image/jpeg;base64,")
        .or_else(|| uri.strip_prefix("data:image/jpg;base64,"))?;
    STANDARD.decode(payload).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_ip_http() {
        assert_eq!(
            extract_ip("http://192.168.1.10/onvif/device_service"),
            "192.168.1.10"
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
    fn extract_ip_bare() {
        assert_eq!(extract_ip("192.168.1.10"), "192.168.1.10");
    }

    #[test]
    fn urldecode_ascii() {
        assert_eq!(urldecode("hello%20world"), "hello world");
    }

    #[test]
    fn urldecode_utf8_multibyte() {
        // é = U+00E9 = 0xC3 0xA9 in UTF-8
        assert_eq!(urldecode("caf%C3%A9"), "café");
    }

    #[test]
    fn urldecode_plus() {
        assert_eq!(urldecode("a+b"), "a b");
    }

    #[test]
    fn urldecode_passthrough() {
        assert_eq!(urldecode("plain"), "plain");
    }

    #[test]
    fn validate_addr_accepts_common_shapes() {
        assert!(validate_device_addr("192.168.1.10").is_ok());
        assert!(validate_device_addr("192.168.1.10:8080").is_ok());
        assert!(validate_device_addr("192.168.1.10/onvif/device").is_ok());
        assert!(validate_device_addr("http://192.168.1.10/onvif/device").is_ok());
        assert!(validate_device_addr("https://camera.local:443/onvif/device").is_ok());
        assert!(validate_device_addr("cam-01").is_ok());
    }

    #[test]
    fn validate_addr_rejects_bad_shapes() {
        assert_eq!(validate_device_addr(""), Err(AddrError::Empty));
        assert_eq!(validate_device_addr("   "), Err(AddrError::Empty));
        assert_eq!(
            validate_device_addr("999.1.1.1"),
            Err(AddrError::InvalidIpOctet)
        );
        assert_eq!(
            validate_device_addr("192.168.1.10:99999"),
            Err(AddrError::InvalidPort)
        );
        assert_eq!(
            validate_device_addr("cam!@#"),
            Err(AddrError::InvalidHostname)
        );
        assert_eq!(
            validate_device_addr("-cam"),
            Err(AddrError::InvalidHostname)
        );
        assert_eq!(
            validate_device_addr("http:///onvif"),
            Err(AddrError::NoHost)
        );
    }
}
