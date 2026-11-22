//! HTTP for the Dictionary Center (ureq) — the same requests as the Python
//! edition (`dictz/center/net.py`): user agent `dictz/<version>`, the
//! source's sign-in, a proxy from the settings (`center.proxy`: empty = the
//! system's, `none` = direct, else `http://host:port`), resumable downloads.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;
use std::time::Duration;

use super::sources::Source;
use super::util::base64;

/// A request failed; `status` is the HTTP status (0 = no answer).
#[derive(Debug, Clone)]
pub struct NetError {
    /// What went wrong.
    pub message: String,
    /// HTTP status.
    pub status: u16,
}

impl std::fmt::Display for NetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

/// The user stopped a download (the partial file stays, to resume).
#[derive(Debug, Clone)]
pub struct Cancelled;

/// What a download ended with.
#[derive(Debug)]
pub enum DownloadError {
    /// Network / server.
    Net(NetError),
    /// Stopped.
    Cancelled,
}

/// The user agent.
pub fn user_agent() -> String {
    format!(
        "dictz/{} (+https://github.com/bayramkotan/dictz)",
        crate::VERSION
    )
}

/// The headers a source's sign-in adds.
pub fn auth_headers(src: &Source, password: &str, token: &str) -> Vec<(String, String)> {
    match src.auth.as_str() {
        "basic" if !src.user.is_empty() || !password.is_empty() => vec![(
            "Authorization".into(),
            format!(
                "Basic {}",
                base64(format!("{}:{}", src.user, password).as_bytes())
            ),
        )],
        "bearer" if !token.is_empty() => {
            vec![("Authorization".into(), format!("Bearer {}", token))]
        }
        "header" if !src.user.is_empty() && !token.is_empty() => {
            vec![(src.user.clone(), token.to_string())]
        }
        _ => vec![],
    }
}

/// The path of a `file:` link (`file:///x`, `file:/x`; `/C:/x` → `C:/x` on
/// Windows); `None` for other links.
pub fn local_path(url: &str) -> Option<std::path::PathBuf> {
    if !url.starts_with("file:") {
        return None;
    }
    let mut p = crate::center::util::unquote(&crate::center::util::urlsplit(url).2);
    if cfg!(windows) && p.len() > 2 && p.as_bytes()[2] == b':' {
        p.remove(0);
    }
    Some(std::path::PathBuf::from(p))
}

/// Anything adapters can read through (tests answer from files).
pub trait Fetch: Sync {
    /// `method url` with headers; the body.
    fn request(
        &self,
        method: &str,
        url: &str,
        body: Option<&[u8]>,
        headers: &[(String, String)],
    ) -> Result<Vec<u8>, NetError>;

    /// GET.
    fn get(&self, url: &str, headers: &[(String, String)]) -> Result<Vec<u8>, NetError> {
        self.request("GET", url, None, headers)
    }
}

/// The real client.
pub struct Http {
    agent: ureq::Agent,
    /// Extra headers on every request.
    pub headers: Vec<(String, String)>,
}

impl Http {
    /// `timeout` seconds; `proxy` as in the setting.
    pub fn new(timeout: f64, proxy: &str) -> Self {
        let mut b = ureq::AgentBuilder::new()
            .timeout_connect(Duration::from_secs_f64(timeout.max(1.0)))
            .timeout_read(Duration::from_secs_f64(timeout.max(1.0)))
            .user_agent(&user_agent());
        let p = proxy.trim();
        if p == "none" {
            b = b.try_proxy_from_env(false);
        } else if !p.is_empty() {
            if let Ok(px) = ureq::Proxy::new(p) {
                b = b.proxy(px);
            }
        } else {
            b = b.try_proxy_from_env(true);
        }
        Self {
            agent: b.build(),
            headers: vec![],
        }
    }

    fn req(&self, method: &str, url: &str, headers: &[(String, String)]) -> ureq::Request {
        let mut r = self.agent.request(method, url);
        let mut has_auth = false;
        for (k, v) in self.headers.iter().chain(headers.iter()) {
            if k.eq_ignore_ascii_case("authorization") {
                has_auth = true;
            }
            r = r.set(k, v);
        }
        if url.contains("api.github.com") && !has_auth {
            if let Ok(t) = std::env::var("GITHUB_TOKEN") {
                if !t.is_empty() {
                    r = r.set("Authorization", &format!("Bearer {}", t)); // more requests per hour
                }
            }
        }
        r
    }

    /// Download `url` into `part`, continuing a partial file (`Range`).
    pub fn download(
        &self,
        url: &str,
        part: &Path,
        headers: &[(String, String)],
        progress: &mut dyn FnMut(u64, u64),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<u64, DownloadError> {
        let net = |m: String, s: u16| {
            DownloadError::Net(NetError {
                message: m,
                status: s,
            })
        };
        if let Some(d) = part.parent() {
            let _ = fs::create_dir_all(d);
        }
        let mut have = fs::metadata(part).map(|m| m.len()).unwrap_or(0);
        let mut h = headers.to_vec();
        if have > 0 {
            h.push(("Range".into(), format!("bytes={}-", have)));
        }
        let resp = match self.req("GET", url, &h).call() {
            Ok(r) => r,
            Err(ureq::Error::Status(416, _)) if have > 0 => return Ok(have),
            Err(ureq::Error::Status(code, r)) => {
                return Err(net(
                    format!("HTTP {} {}: {}", code, r.status_text(), url),
                    code,
                ))
            }
            Err(e) => return Err(net(transport_message(&e.to_string(), url), 0)),
        };
        if have > 0 && resp.status() != 206 {
            have = 0; // the server ignored Range: start again
        }
        let total = resp
            .header("Content-Length")
            .and_then(|v| v.parse::<u64>().ok())
            .map(|n| n + have)
            .unwrap_or(0);
        let mut f = if have > 0 {
            fs::OpenOptions::new().append(true).open(part)
        } else {
            fs::File::create(part)
        }
        .map_err(|e| net(format!("{}: {}", e, part.display()), 0))?;
        let mut rd = resp.into_reader();
        let mut buf = vec![0u8; 256 * 1024];
        let mut done = have;
        loop {
            if cancelled() {
                return Err(DownloadError::Cancelled);
            }
            let n = rd
                .read(&mut buf)
                .map_err(|e| net(format!("{}: {}", e, url), 0))?;
            if n == 0 {
                break;
            }
            f.write_all(&buf[..n])
                .map_err(|e| net(format!("{}: {}", e, part.display()), 0))?;
            done += n as u64;
            progress(done, total);
        }
        if total > 0 && done < total {
            return Err(net(
                format!("incomplete download ({} of {} bytes): {}", done, total, url),
                0,
            ));
        }
        Ok(done)
    }
}

impl Fetch for Http {
    fn request(
        &self,
        method: &str,
        url: &str,
        body: Option<&[u8]>,
        headers: &[(String, String)],
    ) -> Result<Vec<u8>, NetError> {
        if let Some(p) = local_path(url) {
            // file: links (a catalog on this computer), as urllib reads them
            return fs::read(&p).map_err(|e| NetError {
                message: format!("{}: {}", e, p.display()),
                status: 0,
            });
        }
        let r = self.req(method, url, headers);
        let res = match body {
            Some(b) => r.send_bytes(b),
            None => r.call(),
        };
        match res {
            Ok(resp) => {
                let mut out = Vec::new();
                resp.into_reader()
                    .take(512 * 1024 * 1024)
                    .read_to_end(&mut out)
                    .map_err(|e| NetError {
                        message: format!("{}: {}", e, url),
                        status: 0,
                    })?;
                Ok(out)
            }
            Err(ureq::Error::Status(code, r)) => Err(NetError {
                message: format!("HTTP {} {}: {}", code, r.status_text(), url),
                status: code,
            }),
            Err(e) => Err(NetError {
                message: transport_message(&e.to_string(), url),
                status: 0,
            }),
        }
    }
}

/// `error: url` — without the link twice (ureq's own text starts with it).
fn transport_message(err: &str, url: &str) -> String {
    let e = err
        .strip_prefix(url)
        .map(|s| s.trim_start_matches(':').trim())
        .unwrap_or(err);
    format!("{}: {}", e, url)
}
