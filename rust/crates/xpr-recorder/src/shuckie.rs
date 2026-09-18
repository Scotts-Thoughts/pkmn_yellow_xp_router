//! Port of `route_recording/supershuckie_client.py`.

use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};

use serde_json::Value;

use xpr_core::consts;

/// Render a millisecond timer value as `H:MM:SS.CC` (centiseconds truncated).
pub fn format_time_ms(time_ms: i64) -> String {
    let total_seconds = time_ms.div_euclid(1000);
    let milliseconds = time_ms.rem_euclid(1000);
    let total_minutes = total_seconds.div_euclid(60);
    let seconds = total_seconds.rem_euclid(60);
    let hours = total_minutes.div_euclid(60);
    let minutes = total_minutes.rem_euclid(60);
    format!("{}:{:02}:{:02}.{:02}", hours, minutes, seconds, milliseconds / 10)
}

const POLL_INTERVAL: Duration = Duration::from_millis(100);
/// how long a cached value stays usable after Super Shuckie stops responding
const STALE_THRESHOLD: Duration = Duration::from_secs(2);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(1);
const ERROR_LOG_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
struct Cached {
    time_ms: Option<i64>,
    last_success: Option<Instant>,
    last_error_logged: Option<Instant>,
}

/// Keeps a cached copy of Super Shuckie's run timer, polled at 10 Hz on a
/// background thread, so that recording code never blocks on a request.
pub struct SuperShuckieClient {
    connection_string: String,
    active: Arc<AtomicBool>,
    generation: Arc<AtomicU64>,
    cached: Arc<Mutex<Cached>>,
}

impl SuperShuckieClient {
    pub fn new(connection_string: Option<&str>) -> SuperShuckieClient {
        SuperShuckieClient {
            connection_string: connection_string.unwrap_or(consts::SUPER_SHUCKIE_URL).to_string(),
            active: Arc::new(AtomicBool::new(false)),
            generation: Arc::new(AtomicU64::new(0)),
            cached: Arc::new(Mutex::new(Cached::default())),
        }
    }

    pub fn start(&self) {
        if self.active.swap(true, Ordering::SeqCst) {
            return;
        }
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let url = format!("{}/stats", self.connection_string);
        let active = self.active.clone();
        let gen_counter = self.generation.clone();
        let cached = self.cached.clone();
        std::thread::Builder::new()
            .name("supershuckie-poll".into())
            .spawn(move || {
                let client = reqwest::blocking::Client::builder().timeout(REQUEST_TIMEOUT).build().ok();
                while active.load(Ordering::SeqCst) && gen_counter.load(Ordering::SeqCst) == generation {
                    refresh_stats(client.as_ref(), &url, &cached);
                    std::thread::sleep(POLL_INTERVAL);
                }
            })
            .ok();
    }

    pub fn stop(&self) {
        self.active.store(false, Ordering::SeqCst);
        let mut c = self.cached.lock().unwrap();
        c.time_ms = None;
        c.last_success = None;
    }

    /// The most recent timer value formatted as `H:MM:SS.CC`, or `None`.
    pub fn get_current_time(&self) -> Option<String> {
        self.get_current_time_ms().map(format_time_ms)
    }

    /// `None` when Super Shuckie isn't running, the timer hasn't started, or
    /// the cached value has gone stale.
    pub fn get_current_time_ms(&self) -> Option<i64> {
        let c = self.cached.lock().unwrap();
        let last = c.last_success?;
        if last.elapsed() > STALE_THRESHOLD {
            return None;
        }
        c.time_ms
    }
}

fn refresh_stats(client: Option<&reqwest::blocking::Client>, url: &str, cached: &Mutex<Cached>) {
    let result: Result<Value, String> = (|| {
        let client = client.ok_or("no http client")?;
        let resp = client.get(url).send().map_err(|e| format!("{}: {}", "RequestError", e))?;
        resp.json::<Value>().map_err(|e| format!("JSONDecodeError: {}", e))
    })();
    match result {
        Ok(stats) => {
            // null until a run has been started with mark-start; bools are not ints
            let time_current = match stats.get("time_current") {
                Some(Value::Number(n)) if n.is_i64() || n.is_u64() => n.as_i64(),
                _ => None,
            };
            let mut c = cached.lock().unwrap();
            c.time_ms = time_current;
            c.last_success = Some(Instant::now());
        }
        Err(e) => {
            let mut c = cached.lock().unwrap();
            let should_log = c.last_error_logged.map(|t| t.elapsed() >= ERROR_LOG_INTERVAL).unwrap_or(true);
            if should_log {
                c.last_error_logged = Some(Instant::now());
                log::info!("Could not read Super Shuckie stats from {}: {}", url.trim_end_matches("/stats"), e);
            }
        }
    }
}

/// The module-level singleton (`supershuckie_client`).
pub fn supershuckie() -> &'static SuperShuckieClient {
    static CLIENT: OnceLock<SuperShuckieClient> = OnceLock::new();
    CLIENT.get_or_init(|| SuperShuckieClient::new(None))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_like_the_overlays() {
        assert_eq!(format_time_ms(679), "0:00:00.67");
        assert_eq!(format_time_ms(3_600_000 + 61_000 + 5), "1:01:01.00");
        assert_eq!(format_time_ms(59_999), "0:00:59.99");
        assert_eq!(format_time_ms(0), "0:00:00.00");
    }
}
