//! Shared utility functions.

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

/// Copy text to the system clipboard. Returns `Ok(())` or an error message.
pub fn copy_to_clipboard(text: &str) -> Result<(), String> {
    arboard::Clipboard::new()
        .and_then(|mut cb| cb.set_text(text))
        .map_err(|e| e.to_string())
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
}
