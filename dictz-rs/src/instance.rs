//! Talking to the running dictz (single instance, `dictz show` / `popup` /
//! `watch`) — the same protocol as the Python edition's
//! `dictz/core/instance.py`, so either edition's CLI can drive either window.
//!
//! The running window listens on 127.0.0.1 and writes its port and a random
//! token to `instance.json` in the data folder. A request is one JSON line
//! with the token; the answer is one JSON line.

use std::io::{BufRead, BufReader, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use serde_json::{json, Value};

/// File in the data folder.
pub const INSTANCE_FILE: &str = "instance.json";
const MAX_REQUEST: u64 = 1 << 20;

/// Path of `instance.json`.
pub fn info_path(data_dir: &Path) -> PathBuf {
    data_dir.join(INSTANCE_FILE)
}

/// (port, token) of the running dictz, if the file is there.
pub fn read_info(data_dir: &Path) -> Option<(u16, String)> {
    let v: Value =
        serde_json::from_str(&std::fs::read_to_string(info_path(data_dir)).ok()?).ok()?;
    let port = v.get("port")?.as_u64()?;
    let token = v.get("token")?.as_str()?.to_string();
    u16::try_from(port).ok().map(|p| (p, token))
}

fn read_line(stream: &mut TcpStream) -> std::io::Result<String> {
    let mut r = BufReader::new(std::io::Read::take(stream, MAX_REQUEST));
    let mut line = String::new();
    r.read_line(&mut line)?;
    Ok(line)
}

/// Send `message` to the running dictz; `None` when none is running.
pub fn send(data_dir: &Path, message: &Value, timeout: Duration) -> Option<Value> {
    let (port, token) = read_info(data_dir)?;
    let mut payload = message.as_object()?.clone();
    payload.insert("token".into(), Value::from(token));
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let mut s = TcpStream::connect_timeout(&addr, timeout).ok()?;
    s.set_read_timeout(Some(timeout)).ok()?;
    s.set_write_timeout(Some(timeout)).ok()?;
    let line = format!("{}\n", Value::Object(payload));
    s.write_all(line.as_bytes()).ok()?;
    let answer = read_line(&mut s).ok()?;
    let v: Value = serde_json::from_str(answer.trim()).ok()?;
    v.is_object().then_some(v)
}

/// `true` when a dictz window answers.
pub fn is_running(data_dir: &Path) -> bool {
    send(data_dir, &json!({"cmd": "ping"}), Duration::from_secs(1))
        .map(|v| v["ok"] == true)
        .unwrap_or(false)
}

fn random_token() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    let mut out = String::new();
    for i in 0..2u64 {
        let mut h = RandomState::new().build_hasher();
        h.write_u64(i);
        h.write_u128(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
        );
        h.write_u32(std::process::id());
        out.push_str(&format!("{:016x}", h.finish()));
    }
    out
}

/// Request handler: called in the server thread, must return the answer.
pub type Handler = Arc<dyn Fn(Value) -> Value + Send + Sync>;

/// Accepts requests in a background thread.
pub struct Server {
    data_dir: PathBuf,
    token: String,
    port: u16,
}

impl Server {
    /// Listen, write `instance.json` and serve in a background thread.
    pub fn start(
        data_dir: &Path,
        handler: Handler,
        edition: &str,
        version: &str,
    ) -> Option<Server> {
        let listener = TcpListener::bind(("127.0.0.1", 0)).ok()?;
        let port = listener.local_addr().ok()?.port();
        let token = random_token();
        let info = json!({"port": port, "token": token, "pid": std::process::id(),
                          "edition": edition, "version": version});
        let path = info_path(data_dir);
        std::fs::create_dir_all(data_dir).ok()?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, info.to_string()).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600));
        }
        std::fs::rename(&tmp, &path).ok()?;
        let tok = token.clone();
        std::thread::Builder::new()
            .name("dictz-instance".into())
            .spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { continue };
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
                    let answer = match read_line(&mut stream)
                        .ok()
                        .and_then(|l| serde_json::from_str::<Value>(l.trim()).ok())
                    {
                        Some(Value::Object(mut req))
                            if req.get("token").and_then(Value::as_str) == Some(tok.as_str()) =>
                        {
                            req.remove("token");
                            if req.get("cmd").and_then(Value::as_str) == Some("__stop__") {
                                let _ = stream.write_all(b"{\"ok\": true}\n");
                                return;
                            }
                            handler(Value::Object(req))
                        }
                        _ => json!({"ok": false, "error": "bad token"}),
                    };
                    let _ = stream.write_all(format!("{}\n", answer).as_bytes());
                    let _ = stream.shutdown(Shutdown::Both);
                }
            })
            .ok()?;
        Some(Server {
            data_dir: data_dir.to_path_buf(),
            token,
            port,
        })
    }

    /// Port the server listens on.
    pub fn port(&self) -> u16 {
        self.port
    }

    /// Stop serving and remove `instance.json` (if it is still ours).
    pub fn stop(&self) {
        if read_info(&self.data_dir)
            .map(|(_, t)| t == self.token)
            .unwrap_or(false)
        {
            let _ = send(
                &self.data_dir,
                &json!({"cmd": "__stop__"}),
                Duration::from_secs(1),
            );
            let _ = std::fs::remove_file(info_path(&self.data_dir));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip() {
        let d = std::env::temp_dir().join(format!("dictz-inst-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        assert!(send(&d, &json!({"cmd": "ping"}), Duration::from_millis(300)).is_none());
        let h: Handler = Arc::new(|req| json!({"ok": true, "echo": req["cmd"]}));
        let s = Server::start(&d, h, "rust", "0").unwrap();
        let a = send(&d, &json!({"cmd": "show"}), Duration::from_secs(2)).unwrap();
        assert_eq!(a["echo"], "show");
        assert!(is_running(&d));
        s.stop();
        assert!(!info_path(&d).exists());
        let _ = std::fs::remove_dir_all(&d);
    }
}
