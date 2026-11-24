//! The local move-stats server (spec §2.4): `GET /move-stats` and
//! `GET /status` on `http://127.0.0.1:<port>` (30161 by default), so the
//! overlay can read the numbers while a route is being recorded.
//!
//! It runs only while recording, listens on 127.0.0.1 only, refuses a `Host`
//! header that isn't 127.0.0.1/localhost and refuses `Sec-Fetch-Site:
//! cross-site` (a web page in a browser can't read it). It sends no CORS
//! headers on purpose: the overlay asks from its main process.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use serde_json::{json, Value};

use xpr_recorder::move_stats::MoveStatsHandle;

pub struct MoveStatsServer {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
    pub port: u16,
}

impl MoveStatsServer {
    pub fn start(port: u16, stats: MoveStatsHandle) -> Result<MoveStatsServer, String> {
        let listener = TcpListener::bind(("127.0.0.1", port)).map_err(|e| format!("move stats server: can't listen on 127.0.0.1:{}: {}", port, e))?;
        listener.set_nonblocking(true).map_err(|e| e.to_string())?;
        let stop = Arc::new(AtomicBool::new(false));
        let stop2 = stop.clone();
        let thread = std::thread::Builder::new()
            .name("move-stats-server".into())
            .spawn(move || {
                while !stop2.load(Ordering::SeqCst) {
                    match listener.accept() {
                        Ok((stream, _)) => {
                            let stats = stats.clone();
                            let _ = std::thread::Builder::new().name("move-stats-request".into()).spawn(move || {
                                if let Err(e) = handle(stream, &stats, port) {
                                    log::debug!("move stats server: {}", e);
                                }
                            });
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => std::thread::sleep(Duration::from_millis(50)),
                        Err(e) => {
                            log::warn!("move stats server: accept failed: {}", e);
                            std::thread::sleep(Duration::from_millis(250));
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        log::info!("Move stats server listening on http://127.0.0.1:{}", port);
        Ok(MoveStatsServer { stop, thread: Some(thread), port })
    }

    pub fn stop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
        log::info!("Move stats server stopped");
    }
}

impl Drop for MoveStatsServer {
    fn drop(&mut self) {
        self.stop();
    }
}

/// What a request asks for, after the checks.
#[derive(Debug, PartialEq)]
pub enum Route {
    MoveStats,
    Status,
    NotFound,
    Refused(&'static str),
    BadMethod,
}

/// Decide what to answer from the request line and headers (kept apart from
/// the socket so it can be tested).
pub fn route(request_line: &str, headers: &[(String, String)]) -> Route {
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or("");
    let target = parts.next().unwrap_or("");
    let header = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.trim().to_ascii_lowercase());
    match header("host") {
        Some(h) => {
            let host = h.rsplit_once(':').map(|(a, port)| if port.chars().all(|c| c.is_ascii_digit()) { a.to_string() } else { h.clone() }).unwrap_or(h.clone());
            if host != "127.0.0.1" && host != "localhost" {
                return Route::Refused("bad host");
            }
        }
        None => return Route::Refused("no host"),
    }
    if header("sec-fetch-site").as_deref() == Some("cross-site") {
        return Route::Refused("cross-site");
    }
    if method != "GET" {
        return Route::BadMethod;
    }
    let path = target.split('?').next().unwrap_or("");
    match path {
        "/move-stats" => Route::MoveStats,
        "/status" => Route::Status,
        _ => Route::NotFound,
    }
}

fn handle(stream: TcpStream, stats: &MoveStatsHandle, _port: u16) -> Result<(), String> {
    stream.set_nonblocking(false).map_err(|e| e.to_string())?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).map_err(|e| e.to_string())?;
    let mut reader = BufReader::new(stream.try_clone().map_err(|e| e.to_string())?);
    let mut request_line = String::new();
    reader.read_line(&mut request_line).map_err(|e| e.to_string())?;
    let mut headers = Vec::new();
    let mut total = request_line.len();
    loop {
        let mut line = String::new();
        let n = reader.read_line(&mut line).map_err(|e| e.to_string())?;
        total += n;
        if n == 0 || line == "\r\n" || line == "\n" || total > 16 * 1024 {
            break;
        }
        if let Some((k, v)) = line.split_once(':') {
            headers.push((k.trim().to_string(), v.trim().to_string()));
        }
    }
    let (status, body): (&str, Value) = match route(&request_line, &headers) {
        Route::MoveStats => {
            let s = stats.lock().unwrap();
            if s.is_recording() {
                ("200 OK", s.server_json())
            } else {
                ("503 Service Unavailable", json!({"recording": false}))
            }
        }
        Route::Status => ("200 OK", stats.lock().unwrap().status_json()),
        Route::NotFound => ("404 Not Found", json!({"error": "not found"})),
        Route::BadMethod => ("405 Method Not Allowed", json!({"error": "GET only"})),
        Route::Refused(why) => ("403 Forbidden", json!({"error": why})),
    };
    let body = serde_json::to_string(&body).unwrap_or_else(|_| "{}".into());
    let mut out = stream;
    let response = format!(
        "HTTP/1.1 {}\r\nContent-Type: application/json; charset=utf-8\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        status,
        body.len(),
        body
    );
    out.write_all(response.as_bytes()).map_err(|e| e.to_string())?;
    out.flush().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    #[test]
    fn routes_and_refusals() {
        assert_eq!(route("GET /move-stats HTTP/1.1", &h(&[("Host", "127.0.0.1:30161")])), Route::MoveStats);
        assert_eq!(route("GET /status HTTP/1.1", &h(&[("host", "localhost:30161")])), Route::Status);
        assert_eq!(route("GET /status?x=1 HTTP/1.1", &h(&[("Host", "localhost")])), Route::Status);
        assert_eq!(route("GET /status HTTP/1.1", &h(&[("Host", "evil.example:30161")])), Route::Refused("bad host"));
        assert_eq!(route("GET /status HTTP/1.1", &h(&[])), Route::Refused("no host"));
        assert_eq!(route("GET /status HTTP/1.1", &h(&[("Host", "127.0.0.1"), ("Sec-Fetch-Site", "cross-site")])), Route::Refused("cross-site"));
        assert_eq!(route("POST /status HTTP/1.1", &h(&[("Host", "127.0.0.1")])), Route::BadMethod);
        assert_eq!(route("GET /other HTTP/1.1", &h(&[("Host", "127.0.0.1")])), Route::NotFound);
    }
}
