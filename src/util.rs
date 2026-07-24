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

/// A run of text within a changed line, flagged as differing or shared — for
/// intra-line (word-level) highlighting.
pub(crate) struct Seg {
    pub text: String,
    /// `true` = this run is the part that differs from the other side.
    pub changed: bool,
}

/// One aligned row of a side-by-side (git-style) line diff.
pub(crate) enum DiffRow {
    /// Unchanged line, present on both sides.
    Equal(String),
    /// Present only on the left (baseline) — a removed line.
    Left(String),
    /// Present only on the right (clone) — an added line.
    Right(String),
    /// A line that changed: same slot on both sides, with the differing spans
    /// marked (`left`/`right` are the two sides, word-segmented).
    Changed { left: Vec<Seg>, right: Vec<Seg> },
}

/// Align `left` vs `right` line-by-line via an LCS for a side-by-side diff, then
/// pair each adjacent delete+insert into a single `Changed` row with the
/// differing words highlighted (so a line that changed in one token isn't a
/// wholesale red/green line pair).
pub(crate) fn line_diff(left: &str, right: &str) -> Vec<DiffRow> {
    merge_changes(raw_line_diff(left, right))
}

/// Line-level LCS producing only Equal / Left / Right rows.
fn raw_line_diff(left: &str, right: &str) -> Vec<DiffRow> {
    let l: Vec<&str> = left.lines().collect();
    let r: Vec<&str> = right.lines().collect();
    let (common_l, common_r) = lcs_flags(&l, &r);

    let mut rows = Vec::new();
    let (mut i, mut j) = (0, 0);
    let (n, m) = (l.len(), r.len());
    while i < n || j < m {
        let li_common = i < n && common_l[i];
        let rj_common = j < m && common_r[j];
        if i < n && j < m && li_common && rj_common {
            rows.push(DiffRow::Equal(l[i].to_string()));
            i += 1;
            j += 1;
        } else if i < n && !li_common {
            rows.push(DiffRow::Left(l[i].to_string()));
            i += 1;
        } else {
            rows.push(DiffRow::Right(r[j].to_string()));
            j += 1;
        }
    }
    rows
}

/// Merge each `Left` immediately followed by a `Right` into a `Changed` row with
/// word-level segments; everything else passes through unchanged.
fn merge_changes(rows: Vec<DiffRow>) -> Vec<DiffRow> {
    let mut out = Vec::with_capacity(rows.len());
    let mut it = rows.into_iter().peekable();
    while let Some(row) = it.next() {
        match row {
            DiffRow::Left(l) if matches!(it.peek(), Some(DiffRow::Right(_))) => {
                let DiffRow::Right(r) = it.next().unwrap() else {
                    unreachable!()
                };
                let (left, right) = word_segments(&l, &r);
                out.push(DiffRow::Changed { left, right });
            }
            other => out.push(other),
        }
    }
    out
}

/// Word-level diff of two lines → the segments to render on each side, with the
/// differing runs flagged.
fn word_segments(left: &str, right: &str) -> (Vec<Seg>, Vec<Seg>) {
    let lt = tokenize(left);
    let rt = tokenize(right);
    let (common_l, common_r) = lcs_flags(&lt, &rt);
    (coalesce(&lt, &common_l), coalesce(&rt, &common_r))
}

/// LCS membership flags: `out.0[i]` / `out.1[j]` = true iff that element is part
/// of the longest common subsequence (i.e. shared, not changed).
fn lcs_flags<T: PartialEq>(l: &[T], r: &[T]) -> (Vec<bool>, Vec<bool>) {
    let (n, m) = (l.len(), r.len());
    let mut dp = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            dp[i][j] = if l[i] == r[j] {
                dp[i + 1][j + 1] + 1
            } else {
                dp[i + 1][j].max(dp[i][j + 1])
            };
        }
    }
    let mut lf = vec![false; n];
    let mut rf = vec![false; m];
    let (mut i, mut j) = (0, 0);
    while i < n && j < m {
        if l[i] == r[j] {
            lf[i] = true;
            rf[j] = true;
            i += 1;
            j += 1;
        } else if dp[i + 1][j] >= dp[i][j + 1] {
            i += 1;
        } else {
            j += 1;
        }
    }
    (lf, rf)
}

/// Merge consecutive tokens with the same changed-flag into one [`Seg`].
fn coalesce(tokens: &[&str], common: &[bool]) -> Vec<Seg> {
    let mut segs: Vec<Seg> = Vec::new();
    for (t, &c) in tokens.iter().zip(common) {
        let changed = !c;
        match segs.last_mut() {
            Some(last) if last.changed == changed => last.text.push_str(t),
            _ => segs.push(Seg {
                text: (*t).to_string(),
                changed,
            }),
        }
    }
    segs
}

/// Split a line into alternating word / non-word runs (whitespace and XML
/// punctuation form their own tokens), so an XML value change diffs at word
/// granularity rather than replacing the whole line.
fn tokenize(s: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let mut chars = s.char_indices().peekable();
    while let Some(&(start, c)) = chars.peek() {
        let word = is_word_char(c);
        let mut end = start;
        while let Some(&(idx, c2)) = chars.peek() {
            if is_word_char(c2) == word {
                end = idx + c2.len_utf8();
                chars.next();
            } else {
                break;
            }
        }
        out.push(&s[start..end]);
    }
    out
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '.' | ':' | '-' | '_')
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

    fn shape(rows: &[DiffRow]) -> Vec<&'static str> {
        rows.iter()
            .map(|r| match r {
                DiffRow::Equal(_) => "=",
                DiffRow::Left(_) => "-",
                DiffRow::Right(_) => "+",
                DiffRow::Changed { .. } => "~",
            })
            .collect()
    }

    #[test]
    fn line_diff_merges_adjacent_change_into_a_changed_row() {
        // left : A B D ; right : A C D → B/C are an adjacent delete+insert, so
        // they merge into one Changed row between the equal A and D.
        let rows = line_diff("A\nB\nD", "A\nC\nD");
        assert_eq!(shape(&rows), ["=", "~", "="]);
        let DiffRow::Changed { left, right } = &rows[1] else {
            panic!("expected a Changed row: {:?}", shape(&rows));
        };
        assert!(left.iter().all(|s| s.changed) && left.iter().any(|s| s.text == "B"));
        assert!(right.iter().all(|s| s.changed) && right.iter().any(|s| s.text == "C"));
    }

    #[test]
    fn line_diff_highlights_only_the_changed_word() {
        // Same element, one differing value → the tag stays shared, only the
        // value is flagged.
        let rows = line_diff(
            "  <tt:Manufacturer>oxvif-mock</tt:Manufacturer>",
            "  <tt:Manufacturer>Hikvision</tt:Manufacturer>",
        );
        assert_eq!(shape(&rows), ["~"]);
        let DiffRow::Changed { left, right } = &rows[0] else {
            panic!("expected Changed");
        };
        // The shared markup is not flagged; the value words are.
        assert!(left
            .iter()
            .any(|s| !s.changed && s.text.contains("Manufacturer")));
        assert!(left.iter().any(|s| s.changed && s.text.contains("oxvif")));
        assert!(right
            .iter()
            .any(|s| s.changed && s.text.contains("Hikvision")));
        // Reassembling each side reproduces the original line.
        let l: String = left.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(l, "  <tt:Manufacturer>oxvif-mock</tt:Manufacturer>");
    }

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
