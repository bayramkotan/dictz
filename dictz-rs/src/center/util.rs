//! Small helpers that behave exactly like the Python standard library calls
//! the Python edition uses (`urllib.parse`, `html.unescape`, dates), so both
//! editions turn the same answers into the same entries.

use std::time::{SystemTime, UNIX_EPOCH};

/// `urllib.parse.urlsplit` → (scheme, netloc, path, query, fragment).
pub fn urlsplit(url: &str) -> (String, String, String, String, String) {
    let mut rest = url;
    let mut scheme = String::new();
    if let Some(i) = rest.find(':') {
        let s = &rest[..i];
        if !s.is_empty()
            && s.chars().next().is_some_and(|c| c.is_ascii_alphabetic())
            && s.chars()
                .all(|c| c.is_ascii_alphanumeric() || "+-.".contains(c))
        {
            scheme = s.to_ascii_lowercase();
            rest = &rest[i + 1..];
        }
    }
    let mut netloc = String::new();
    if let Some(r) = rest.strip_prefix("//") {
        let end = r.find(['/', '?', '#']).unwrap_or(r.len());
        netloc = r[..end].to_string();
        rest = &r[end..];
    }
    let (rest, fragment) = match rest.find('#') {
        Some(i) => (&rest[..i], rest[i + 1..].to_string()),
        None => (rest, String::new()),
    };
    let (path, query) = match rest.find('?') {
        Some(i) => (rest[..i].to_string(), rest[i + 1..].to_string()),
        None => (rest.to_string(), String::new()),
    };
    (scheme, netloc, path, query, fragment)
}

fn remove_dot_segments(path: &str) -> String {
    let mut out: Vec<&str> = Vec::new();
    let segs: Vec<&str> = path.split('/').collect();
    let last = segs.len().saturating_sub(1);
    for (i, s) in segs.iter().enumerate() {
        match *s {
            "." => {
                if i == last {
                    out.push("");
                }
            }
            ".." => {
                if out.len() > 1 {
                    out.pop();
                }
                if i == last {
                    out.push("");
                }
            }
            _ => out.push(s),
        }
    }
    out.join("/")
}

/// `urllib.parse.urljoin` for http(s) / file addresses.
pub fn urljoin(base: &str, url: &str) -> String {
    if base.is_empty() {
        return url.to_string();
    }
    if url.is_empty() {
        return base.to_string();
    }
    let (bs, bn, bp, bq, _bf) = urlsplit(base);
    let (s, n, p, q, f) = urlsplit(url);
    if !s.is_empty() && s != bs {
        return url.to_string();
    }
    let build = |n: &str, p: &str, q: &str, f: &str| {
        let mut o = format!("{}:", bs);
        if !n.is_empty() || bs == "http" || bs == "https" || bs == "file" {
            o.push_str("//");
            o.push_str(n);
        }
        o.push_str(p);
        if !q.is_empty() {
            o.push('?');
            o.push_str(q);
        }
        if !f.is_empty() {
            o.push('#');
            o.push_str(f);
        }
        o
    };
    if !n.is_empty() {
        return build(&n, &remove_dot_segments(&p), &q, &f);
    }
    if p.is_empty() {
        let q2 = if q.is_empty() { bq } else { q };
        return build(&bn, &bp, &q2, &f);
    }
    if p.starts_with('/') {
        return build(&bn, &remove_dot_segments(&p), &q, &f);
    }
    let dir = match bp.rfind('/') {
        Some(i) => &bp[..=i],
        None => "/",
    };
    let joined = format!("{}{}", dir, p);
    let mut path = remove_dot_segments(&joined);
    if !path.starts_with('/') {
        path.insert(0, '/');
    }
    build(&bn, &path, &q, &f)
}

/// `urllib.parse.unquote` (UTF-8, bad bytes replaced).
pub fn unquote(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            let h = std::str::from_utf8(&b[i + 1..i + 3]).ok();
            if let Some(v) = h.and_then(|h| u8::from_str_radix(h, 16).ok()) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn quote_with(s: &str, safe: &str, plus: bool) -> String {
    let mut out = String::new();
    for &c in s.as_bytes() {
        let ch = c as char;
        if c.is_ascii_alphanumeric() || "_.-~".contains(ch) || (c < 128 && safe.contains(ch)) {
            out.push(ch);
        } else if plus && c == b' ' {
            out.push('+');
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

/// `urllib.parse.quote(s)` (safe = `/`).
pub fn quote(s: &str) -> String {
    quote_with(s, "/", false)
}

/// `urllib.parse.quote(s, safe)`.
pub fn quote_safe(s: &str, safe: &str) -> String {
    quote_with(s, safe, false)
}

/// `urllib.parse.urlencode(pairs)` (quote_plus, nothing safe).
pub fn urlencode(pairs: &[(&str, &str)]) -> String {
    pairs
        .iter()
        .map(|(k, v)| format!("{}={}", quote_with(k, "", true), quote_with(v, "", true)))
        .collect::<Vec<_>>()
        .join("&")
}

/// `html.unescape` for the entities pages use (named ones from the shared
/// table, decimal and hex numbers).
pub fn html_unescape(s: &str) -> String {
    if !s.contains('&') {
        return s.to_string();
    }
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);
        let after = &rest[i + 1..];
        let end = after.find(';').filter(|&e| e <= 32);
        if let Some(e) = end {
            let name = &after[..e];
            let cp = if let Some(num) = name.strip_prefix('#') {
                if let Some(h) = num.strip_prefix('x').or_else(|| num.strip_prefix('X')) {
                    u32::from_str_radix(h, 16).ok()
                } else {
                    num.parse::<u32>().ok()
                }
            } else {
                crate::text::entity_codepoint(name).or(match name {
                    "amp" => Some(38),
                    "lt" => Some(60),
                    "gt" => Some(62),
                    "quot" => Some(34),
                    "apos" => Some(39),
                    "nbsp" => Some(160),
                    _ => None,
                })
            };
            if let Some(ch) = cp.and_then(char::from_u32) {
                out.push(ch);
                rest = &after[e + 1..];
                continue;
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}

/// The parent folder of a `/` path the way `str(PurePosixPath(p).parent)` says it.
pub fn posix_parent(p: &str) -> String {
    let t = p.trim_end_matches('/');
    match t.rfind('/') {
        Some(0) => "/".into(),
        Some(i) => t[..i].to_string(),
        None => ".".into(),
    }
}

/// The last part of a `/` path.
pub fn posix_name(p: &str) -> String {
    p.trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string()
}

/// First `n` characters.
pub fn take(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if m > 2 { m - 3 } else { m + 9 } as i64;
    let doy = (153 * mp + 2) / 5 + d as i64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// Now in UTC: `2026-11-21T10:00:00Z`.
pub fn now_iso() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    iso_from_secs(secs)
}

/// Seconds since 1970 → `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso_from_secs(secs: i64) -> String {
    let (y, m, d) = civil(secs.div_euclid(86400));
    let r = secs.rem_euclid(86400);
    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        y,
        m,
        d,
        r / 3600,
        (r % 3600) / 60,
        r % 60
    )
}

/// `2026-11-21T10:00:00Z` (or with `+00:00`) → seconds since 1970.
pub fn secs_from_iso(s: &str) -> Option<i64> {
    let s = s.trim();
    if s.len() < 19 {
        return None;
    }
    let y: i64 = s.get(0..4)?.parse().ok()?;
    let m: u32 = s.get(5..7)?.parse().ok()?;
    let d: u32 = s.get(8..10)?.parse().ok()?;
    let hh: i64 = s.get(11..13)?.parse().ok()?;
    let mm: i64 = s.get(14..16)?.parse().ok()?;
    let ss: i64 = s.get(17..19)?.parse().ok()?;
    let mut off = 0i64;
    let tail = &s[19..];
    let tail = tail.trim_start_matches(|c: char| c == '.' || c.is_ascii_digit());
    if let Some(t) = tail.strip_prefix('+').or_else(|| tail.strip_prefix('-')) {
        let sign = if tail.starts_with('-') { -1 } else { 1 };
        let h: i64 = t.get(0..2)?.parse().ok()?;
        let mi: i64 = t.get(3..5).and_then(|x| x.parse().ok()).unwrap_or(0);
        off = sign * (h * 3600 + mi * 60);
    }
    Some(days_from_civil(y, m, d) * 86400 + hh * 3600 + mm * 60 + ss - off)
}

/// Age in days of an ISO time (huge when it cannot be read).
pub fn age_days(stamp: &str) -> f64 {
    match secs_from_iso(stamp) {
        Some(t) => {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            (now - t) as f64 / 86400.0
        }
        None => 1e9,
    }
}

/// An HTTP date (`Tue, 01 Sep 2026 10:00:00 GMT`) → `2026-09-01`; '' when not one.
pub fn http_date(text: &str) -> String {
    let parts: Vec<&str> = text.split_whitespace().collect();
    let p: Vec<&str> = if parts.first().is_some_and(|w| w.ends_with(',')) {
        parts[1..].to_vec()
    } else {
        parts
    };
    if p.len() < 3 {
        return String::new();
    }
    const MON: [&str; 12] = [
        "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
    ];
    let Ok(d) = p[0].parse::<u32>() else {
        return String::new();
    };
    let Some(m) = MON
        .iter()
        .position(|x| p[1].to_ascii_lowercase().starts_with(x))
    else {
        return String::new();
    };
    let Ok(mut y) = p[2].parse::<i64>() else {
        return String::new();
    };
    if y < 100 {
        y += if y < 69 { 2000 } else { 1900 };
    }
    if !(1..=31).contains(&d) {
        return String::new();
    }
    format!("{:04}-{:02}-{:02}", y, m + 1, d)
}

/// `~/x` → the home folder's `x`.
pub fn expand_user(p: &str) -> std::path::PathBuf {
    let p = p.trim();
    if p == "~" || p.starts_with("~/") || p.starts_with("~\\") {
        if let Some(h) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
            let rest = p[1..].trim_start_matches(['/', '\\']);
            return std::path::PathBuf::from(h).join(rest);
        }
    }
    std::path::PathBuf::from(p)
}

/// Lower-case hex of bytes.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// Standard base64 (for `Authorization: Basic`).
pub fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if c.len() > 1 {
            T[(n >> 6) as usize & 63] as char
        } else {
            '='
        });
        out.push(if c.len() > 2 {
            T[n as usize & 63] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn like_python() {
        assert_eq!(
            urljoin("https://a.org/data/catalog.json", "files/x.tar"),
            "https://a.org/data/files/x.tar"
        );
        assert_eq!(
            urljoin("https://a.org/data/c.json", "extra/c.json"),
            "https://a.org/data/extra/c.json"
        );
        assert_eq!(urljoin("https://a.org/x/", "/y/z"), "https://a.org/y/z");
        assert_eq!(
            urljoin("https://a.org/x/", "https://b.org/q"),
            "https://b.org/q"
        );
        assert_eq!(urljoin("https://a.org/x/y/", "../z/"), "https://a.org/x/z/");
        assert_eq!(unquote("Longman%20DOCE5.mdx"), "Longman DOCE5.mdx");
        assert_eq!(quote("a b/c"), "a%20b/c");
        assert_eq!(
            urlencode(&[("q", "a (b) \"c\""), ("fl[]", "id")]),
            "q=a+%28b%29+%22c%22&fl%5B%5D=id"
        );
        assert_eq!(html_unescape("a &amp; b &#39;c&#x27;"), "a & b 'c'");
        assert_eq!(http_date("Tue, 01 Sep 2026 10:00:00 GMT"), "2026-09-01");
        assert_eq!(secs_from_iso("1970-01-02T00:00:00Z"), Some(86400));
        assert_eq!(iso_from_secs(86400 * 365), "1971-01-01T00:00:00Z");
        assert_eq!(base64(b"tok:pw"), "dG9rOnB3");
        assert_eq!(posix_parent("a.tar"), ".");
        assert_eq!(posix_parent("x/y.ifo"), "x");
    }
}
