//! Port of `route_recording/gamehook_client.py`: the GameHook SignalR hub
//! client (hand-written JSON-protocol framing over a websocket), the REST
//! mapper endpoints, the property store with change callbacks, the 60 s
//! mapper auto-refresh and — new in the port (Appendix B item 12) —
//! automatic reconnection with backoff.

use std::collections::{HashMap, HashSet};
use std::io::ErrorKind;
use std::net::TcpStream;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};

pub const DEFAULT_CONNECTION_STRING: &str = "http://localhost:8085";
const RECORD_SEPARATOR: char = '\u{1e}';
const AUTOMATIC_REFRESH: Duration = Duration::from_secs(60);
const PING_INTERVAL: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, PartialEq)]
pub struct GameHookProperty {
    pub path: String,
    pub value: Value,
    pub bytes_value: Value,
    pub length: Option<i64>,
    pub address: Value,
    pub frozen: Value,
}

impl GameHookProperty {
    fn from_mapper(data: &Value) -> Option<GameHookProperty> {
        let path = data.get("path")?.as_str()?.to_string();
        Some(GameHookProperty {
            path,
            value: data.get("value").cloned().unwrap_or(Value::Null),
            bytes_value: data.get("bytes").cloned().unwrap_or(Value::Null),
            length: data
                .get("size")
                .or_else(|| data.get("length"))
                .and_then(|v| v.as_i64()),
            address: data.get("address").cloned().unwrap_or(Value::Null),
            frozen: data.get("frozen").cloned().unwrap_or(Value::Null),
        })
    }

    /// Python `int` comparisons: numbers (and bools) as i64, anything else `None`.
    pub fn as_i64(&self) -> Option<i64> {
        value_as_i64(&self.value)
    }

    pub fn as_str(&self) -> Option<&str> {
        self.value.as_str()
    }

    pub fn is_null(&self) -> bool {
        self.value.is_null()
    }

    /// Python truthiness of the value.
    pub fn truthy(&self) -> bool {
        xpr_core::pyjson::truthy(&self.value)
    }

    pub fn eq_str(&self, s: &str) -> bool {
        self.value.as_str() == Some(s)
    }

    pub fn eq_i64(&self, i: i64) -> bool {
        self.as_i64() == Some(i)
    }
}

pub fn value_as_i64(v: &Value) -> Option<i64> {
    match v {
        Value::Number(n) => n.as_i64().or_else(|| n.as_f64().map(|f| f as i64)),
        Value::Bool(b) => Some(*b as i64),
        _ => None,
    }
}

/// The loaded mapper: meta, glossary and the properties by path.
#[derive(Clone, Debug, Default)]
pub struct PropertyStore {
    pub meta: Value,
    pub glossary: Value,
    props: HashMap<String, GameHookProperty>,
    order: Vec<String>,
}

impl PropertyStore {
    pub fn from_mapper(mapper: &Value) -> Result<PropertyStore, String> {
        let meta = mapper.get("meta").cloned().ok_or("mapper has no meta")?;
        let glossary = mapper.get("glossary").cloned().unwrap_or(Value::Null);
        let mut props = HashMap::new();
        let mut order = Vec::new();
        for p in mapper.get("properties").and_then(|p| p.as_array()).ok_or("mapper has no properties")? {
            if let Some(prop) = GameHookProperty::from_mapper(p) {
                order.push(prop.path.clone());
                props.insert(prop.path.clone(), prop);
            }
        }
        Ok(PropertyStore { meta, glossary, props, order })
    }

    pub fn is_loaded(&self) -> bool {
        !self.meta.is_null() && self.meta.as_object().map(|o| !o.is_empty()).unwrap_or(false)
    }

    pub fn game_name(&self) -> Option<&str> {
        self.meta.get("gameName").and_then(|g| g.as_str())
    }

    /// `client.get(path)`: `None` (with a warning) for an unknown path.
    pub fn get(&self, path: &str) -> Option<&GameHookProperty> {
        let result = self.props.get(path);
        if result.is_none() {
            log::warn!("[GameHook Client]Empty property path: {}", path);
        }
        result
    }

    /// `client.get_value(path, default=None)`: no warning, `Null` when absent
    /// or when `path` is `None` (an intentionally unmapped constant).
    pub fn get_value(&self, path: Option<&str>) -> Value {
        match path {
            None => Value::Null,
            Some(p) => self.props.get(p).map(|x| x.value.clone()).unwrap_or(Value::Null),
        }
    }

    /// `client.get(path).value` for a mandatory key (Null when unmapped).
    pub fn value(&self, path: &str) -> Value {
        self.get(path).map(|p| p.value.clone()).unwrap_or(Value::Null)
    }

    pub fn i64_of(&self, path: &str) -> Option<i64> {
        self.get(path).and_then(|p| p.as_i64())
    }

    pub fn str_of(&self, path: &str) -> Option<String> {
        self.get(path).and_then(|p| p.as_str().map(|s| s.to_string()))
    }

    pub fn truthy(&self, path: &str) -> bool {
        self.get(path).map(|p| p.truthy()).unwrap_or(false)
    }

    pub fn contains(&self, path: &str) -> bool {
        self.props.contains_key(path)
    }

    pub fn paths(&self) -> &[String] {
        &self.order
    }

    /// Test/replay helper: change a property's value, returning `(new, old)`.
    pub fn set_value(&mut self, path: &str, value: Value) -> Option<(GameHookProperty, GameHookProperty)> {
        let p = self.props.get_mut(path)?;
        let old = p.clone();
        p.value = value;
        Some((p.clone(), old))
    }

    /// Case-insensitive resolution of `path` to the mapper's real spelling.
    pub fn resolve_case(&self, path: &str) -> Option<String> {
        if self.props.contains_key(path) {
            return Some(path.to_string());
        }
        let lower = path.to_lowercase();
        self.order.iter().find(|p| p.to_lowercase() == lower).cloned()
    }
}

/// The callbacks of `RecorderGameHookClient` / the FSM, invoked on the
/// connection thread.
pub trait SessionEvents: Send {
    fn on_connected(&mut self);
    fn on_connection_error(&mut self);
    fn on_disconnected(&mut self);
    fn on_game_hook_error(&mut self, err: &str);
    fn on_driver_error(&mut self, err: &str);
    fn on_driver_recovered(&mut self) {}
    fn on_ui_configuration_changed(&mut self, _config: &Value) {}
    /// The mapper was loaded and propagated; return the property paths whose
    /// value changes should be delivered to `on_property_changed`.
    fn on_mapper_loaded(&mut self, store: &PropertyStore) -> Vec<String>;
    fn on_mapper_load_error(&mut self, err: &str);
    /// A watched property's value changed.
    fn on_property_changed(&mut self, store: &PropertyStore, new: &GameHookProperty, old: &GameHookProperty);
    /// The client is shutting down for good.
    fn on_shutdown(&mut self) {}
}

#[derive(Default)]
pub struct ClientShared {
    pub connected: bool,
    pub mapper_loaded: bool,
    pub game_name: Option<String>,
}

/// One GameHook connection (a background thread) delivering events to a
/// `SessionEvents` implementation.
pub struct GameHookClient {
    connection_string: String,
    shared: Arc<Mutex<ClientShared>>,
    stop: Arc<AtomicBool>,
    reconnect_now: Arc<AtomicBool>,
    thread: Mutex<Option<std::thread::JoinHandle<()>>>,
}

impl GameHookClient {
    /// `connect()` (non-blocking): start the connection thread.
    pub fn connect(connection_string: &str, session: Box<dyn SessionEvents>) -> Arc<GameHookClient> {
        let client = Arc::new(GameHookClient {
            connection_string: connection_string.trim_end_matches('/').to_string(),
            shared: Arc::new(Mutex::new(ClientShared::default())),
            stop: Arc::new(AtomicBool::new(false)),
            reconnect_now: Arc::new(AtomicBool::new(false)),
            thread: Mutex::new(None),
        });
        let worker = Worker {
            url: client.connection_string.clone(),
            shared: client.shared.clone(),
            stop: client.stop.clone(),
            reconnect_now: client.reconnect_now.clone(),
            session,
            watched: HashSet::new(),
            store: PropertyStore::default(),
            ignore_properties: HashSet::new(),
            ignored_updates: HashMap::new(),
            last_mapper_load: None,
        };
        let handle = std::thread::Builder::new()
            .name("gamehook-client".into())
            .spawn(move || worker.run())
            .expect("spawn gamehook thread");
        *client.thread.lock().unwrap() = Some(handle);
        client
    }

    /// `disconnect()`: stop the connection thread (without joining it, so the
    /// UI thread can never dead-lock on a call the worker is waiting on).
    pub fn disconnect(&self) {
        log::info!("[GameHook Client] Disconnect called, shutting down SignalR connection");
        self.stop.store(true, Ordering::SeqCst);
        let mut s = self.shared.lock().unwrap();
        s.connected = false;
        s.mapper_loaded = false;
        s.game_name = None;
    }

    /// The "reconnect" button: retry immediately instead of waiting out the backoff.
    pub fn reconnect(&self) {
        self.reconnect_now.store(true, Ordering::SeqCst);
    }

    pub fn is_connected(&self) -> bool {
        self.shared.lock().unwrap().connected
    }

    pub fn is_mapper_loaded(&self) -> bool {
        self.shared.lock().unwrap().mapper_loaded
    }

    pub fn connection_string(&self) -> &str {
        &self.connection_string
    }

    /// `_edit_property`: `PUT /mapper/properties/{path}` with `{"bytes", "freeze"}`.
    pub fn edit_property(&self, path: &str, freeze: bool, new_bytes: Option<&[i64]>) -> Result<(), String> {
        edit_property(&self.connection_string, path, freeze, new_bytes)
    }
}

pub fn edit_property(connection_string: &str, path: &str, freeze: bool, new_bytes: Option<&[i64]>) -> Result<(), String> {
    let path = path.replace('.', "/");
    let body = json!({"bytes": new_bytes, "freeze": freeze});
    log::debug!("Trying to set property with path {} to {}", path, body);
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .put(format!("{}/mapper/properties/{}", connection_string, path))
        .header("Content-Type", "application/json")
        .body(body.to_string())
        .send()
        .map_err(|e| e.to_string())?;
    if resp.status().as_u16() == 200 {
        return Ok(());
    }
    let data = resp.text().unwrap_or_default();
    let msg = if !data.is_empty() {
        format!("[GameHook Client] Error setting property {} with new_bytes {:?}: {}", path, new_bytes, data)
    } else {
        format!("[GameHook Client] Unknown error setting property {} to new_bytes {:?}", path, new_bytes)
    };
    log::error!("{}", msg);
    Err(msg)
}

fn load_mapper_json(connection_string: &str) -> Result<Value, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client.get(format!("{}/mapper", connection_string)).send().map_err(|e| e.to_string())?;
    if resp.status().as_u16() != 200 {
        let data = resp.text().unwrap_or_default();
        let msg = if !data.is_empty() {
            format!("[GameHook Client] Error loading mapper: {}", data)
        } else {
            "[GameHook Client] Error loading mapper".to_string()
        };
        log::error!("{}", msg);
        return Err(msg);
    }
    resp.json::<Value>().map_err(|e| e.to_string())
}

type Socket = WebSocket<MaybeTlsStream<TcpStream>>;

struct Worker {
    url: String,
    shared: Arc<Mutex<ClientShared>>,
    stop: Arc<AtomicBool>,
    reconnect_now: Arc<AtomicBool>,
    session: Box<dyn SessionEvents>,
    watched: HashSet<String>,
    store: PropertyStore,
    ignore_properties: HashSet<String>,
    ignored_updates: HashMap<String, Value>,
    last_mapper_load: Option<Instant>,
}

impl Worker {
    fn run(mut self) {
        let mut backoff = Duration::from_secs(1);
        let mut first = true;
        while !self.stop.load(Ordering::SeqCst) {
            if !first {
                // automatic reconnection with backoff (the button skips the wait)
                let deadline = Instant::now() + backoff;
                while Instant::now() < deadline && !self.stop.load(Ordering::SeqCst) {
                    if self.reconnect_now.swap(false, Ordering::SeqCst) {
                        break;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }
                if self.stop.load(Ordering::SeqCst) {
                    break;
                }
                log::warn!("[GameHook Client] Attempting automatic reconnection...");
            }
            first = false;
            match self.connect_socket() {
                Ok(socket) => {
                    backoff = Duration::from_secs(1);
                    log::info!("[GameHook Client] GameHook successfully established SignalR connection");
                    {
                        let mut s = self.shared.lock().unwrap();
                        s.connected = true;
                    }
                    self.session.on_connected();
                    self.establish_connection();
                    self.read_loop(socket);
                    // connection lost (or stop requested)
                    log::warn!("[GameHook Client] SignalR connection lost");
                    self.unload_mapper();
                    {
                        let mut s = self.shared.lock().unwrap();
                        s.connected = false;
                    }
                    self.session.on_disconnected();
                }
                Err(e) => {
                    log::error!("[GameHook Client] Exception encountered while building SignalR connection: {}", e);
                    self.session.on_connection_error();
                    backoff = (backoff * 2).min(Duration::from_secs(30));
                }
            }
        }
        self.session.on_shutdown();
    }

    fn connect_socket(&self) -> Result<Socket, String> {
        log::info!("[GameHook Client] GameHook Creating SignalR connection from scratch");
        // negotiate
        let client = reqwest::blocking::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| e.to_string())?;
        let resp = client
            .post(format!("{}/updates/negotiate?negotiateVersion=1", self.url))
            .header("Content-Length", "0")
            .send()
            .map_err(|e| e.to_string())?;
        if !resp.status().is_success() {
            return Err(format!("negotiate failed: HTTP {}", resp.status()));
        }
        let data: Value = resp.json().map_err(|e| e.to_string())?;
        let id = data
            .get("connectionToken")
            .or_else(|| data.get("connectionId"))
            .and_then(|v| v.as_str())
            .ok_or("negotiate response has no connection id")?
            .to_string();
        let ws_base = if let Some(rest) = self.url.strip_prefix("https://") {
            format!("wss://{}", rest)
        } else if let Some(rest) = self.url.strip_prefix("http://") {
            format!("ws://{}", rest)
        } else {
            self.url.clone()
        };
        let ws_url = format!("{}/updates?id={}", ws_base, id);
        let (mut socket, _response) = tungstenite::connect(ws_url.as_str()).map_err(|e| e.to_string())?;
        if let MaybeTlsStream::Plain(stream) = socket.get_mut() {
            let _ = stream.set_read_timeout(Some(READ_TIMEOUT));
        }
        // handshake
        socket
            .send(Message::Text(format!("{{\"protocol\":\"json\",\"version\":1}}{}", RECORD_SEPARATOR).into()))
            .map_err(|e| e.to_string())?;
        let start = Instant::now();
        loop {
            match socket.read() {
                Ok(Message::Text(t)) => {
                    let first = t.split(RECORD_SEPARATOR).next().unwrap_or("");
                    if let Ok(v) = serde_json::from_str::<Value>(first) {
                        if let Some(err) = v.get("error").and_then(|e| e.as_str()) {
                            return Err(format!("handshake rejected: {}", err));
                        }
                    }
                    break;
                }
                Ok(_) => continue,
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {
                    if start.elapsed() > Duration::from_secs(10) {
                        return Err("handshake timed out".into());
                    }
                }
                Err(e) => return Err(e.to_string()),
            }
        }
        Ok(socket)
    }

    /// `_establish_connection`: load the mapper (propagating the event).
    fn establish_connection(&mut self) -> bool {
        match self.load_mapper(true) {
            Ok(()) => true,
            Err(e) => {
                self.unload_mapper();
                log::error!("[GameHook Client] Error encountered trying to establish SignalR connection: {}", e);
                self.session.on_mapper_load_error(&e);
                false
            }
        }
    }

    fn load_mapper(&mut self, propagate_event: bool) -> Result<(), String> {
        log::debug!("[GameHook Client] Loading mapper");
        let mapper = load_mapper_json(&self.url)?;
        self.store = PropertyStore::from_mapper(&mapper)?;
        self.ignore_properties.clear();
        self.ignored_updates.clear();
        self.last_mapper_load = Some(Instant::now());
        {
            let mut s = self.shared.lock().unwrap();
            s.mapper_loaded = true;
            s.game_name = self.store.game_name().map(|s| s.to_string());
        }
        if propagate_event {
            log::info!("[GameHook Client] Mapper loaded successfully!");
            // clear_callbacks_on_load=True: the session re-registers what it wants
            self.watched.clear();
            let watched = self.session.on_mapper_loaded(&self.store);
            self.watched = watched.into_iter().collect();
        }
        Ok(())
    }

    fn unload_mapper(&mut self) {
        log::info!("[GameHook Client] Unloading mapper");
        self.store = PropertyStore::default();
        self.ignore_properties.clear();
        self.ignored_updates.clear();
        let mut s = self.shared.lock().unwrap();
        s.mapper_loaded = false;
        s.game_name = None;
    }

    fn read_loop(&mut self, mut socket: Socket) {
        let mut last_ping = Instant::now();
        while !self.stop.load(Ordering::SeqCst) {
            match socket.read() {
                Ok(Message::Text(text)) => {
                    for record in text.split(RECORD_SEPARATOR) {
                        if record.trim().is_empty() {
                            continue;
                        }
                        match serde_json::from_str::<Value>(record) {
                            Ok(msg) => {
                                if !self.handle_message(&msg, &mut socket) {
                                    return;
                                }
                            }
                            Err(e) => log::warn!("[GameHook Client] Unparseable frame ({}): {}", e, record),
                        }
                    }
                }
                Ok(Message::Close(_)) => return,
                Ok(Message::Ping(payload)) => {
                    let _ = socket.send(Message::Pong(payload));
                }
                Ok(_) => {}
                Err(tungstenite::Error::Io(e)) if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut => {}
                Err(e) => {
                    log::warn!("[GameHook Client] websocket error: {}", e);
                    return;
                }
            }
            if last_ping.elapsed() >= PING_INTERVAL {
                last_ping = Instant::now();
                if socket.send(Message::Text(format!("{{\"type\":6}}{}", RECORD_SEPARATOR).into())).is_err() {
                    return;
                }
            }
            // `_refresh_mapper_helper`: silent reload every minute while loaded
            if let Some(t) = self.last_mapper_load {
                if self.store.is_loaded() && t.elapsed() >= AUTOMATIC_REFRESH {
                    if let Err(e) = self.load_mapper(false) {
                        log::error!("[GameHook Client] automatic mapper refresh failed: {}", e);
                        self.last_mapper_load = Some(Instant::now());
                    }
                }
            }
        }
        let _ = socket.close(None);
    }

    /// Returns false when the connection should be torn down.
    fn handle_message(&mut self, msg: &Value, socket: &mut Socket) -> bool {
        match msg.get("type").and_then(|t| t.as_i64()) {
            Some(1) => {
                let target = msg.get("target").and_then(|t| t.as_str()).unwrap_or("");
                let args = msg.get("arguments").and_then(|a| a.as_array()).cloned().unwrap_or_default();
                match target {
                    "PropertiesChanged" => {
                        if let Some(list) = args.first().and_then(|a| a.as_array()) {
                            for change in list {
                                let positional = json!([
                                    change.get("path").cloned().unwrap_or(Value::Null),
                                    change.get("address").cloned().unwrap_or(Value::Null),
                                    change.get("value").cloned().unwrap_or(Value::Null),
                                    change.get("bytes").cloned().unwrap_or(Value::Null),
                                    change.get("is_frozen").or_else(|| change.get("frozen")).cloned().unwrap_or(Value::Bool(false)),
                                    change.get("fieldsChanged").cloned().unwrap_or(json!([])),
                                ]);
                                if let Err(e) = self.on_single_property_changed(&positional) {
                                    log::error!("Exception generated handling property change: {}", e);
                                    self.session.on_game_hook_error(&e);
                                    self.stop.store(true, Ordering::SeqCst);
                                    return false;
                                }
                            }
                        }
                    }
                    "PropertyChanged" => {
                        let positional = Value::Array(args.clone());
                        if let Err(e) = self.on_single_property_changed(&positional) {
                            log::error!("Exception generated handling property change: {}", e);
                            self.session.on_game_hook_error(&e);
                        }
                    }
                    "MapperLoaded" => {
                        if let Err(e) = self.load_mapper(true) {
                            self.unload_mapper();
                            log::error!("[GameHook Client] Error loading mapper after MapperLoaded: {}", e);
                            self.session.on_mapper_load_error(&e);
                        }
                    }
                    "GameHookError" => {
                        let err = args.first().map(|a| a.to_string()).unwrap_or_default();
                        log::error!("[GameHook Client] GameHook error ocurred: {}", err);
                        self.session.on_game_hook_error(&err);
                    }
                    "DriverError" => {
                        let err = args.first().map(|a| a.to_string()).unwrap_or_default();
                        log::error!("[GameHook Client] Driver error ocurred: {}", err);
                        self.session.on_driver_error(&err);
                    }
                    "SendDriverRecovered" => self.session.on_driver_recovered(),
                    "UiConfigurationChanged" => {
                        let cfg = args.first().cloned().unwrap_or(Value::Null);
                        self.session.on_ui_configuration_changed(&cfg);
                    }
                    other => log::debug!("[GameHook Client] ignoring hub method {}", other),
                }
                true
            }
            Some(6) => {
                // server ping; answer in kind
                let _ = socket.send(Message::Text(format!("{{\"type\":6}}{}", RECORD_SEPARATOR).into()));
                true
            }
            Some(7) => {
                log::warn!("[GameHook Client] server closed the hub connection: {}", msg.get("error").cloned().unwrap_or(Value::Null));
                false
            }
            _ => true,
        }
    }

    /// `_on_single_property_changed(args)` with the positional list.
    fn on_single_property_changed(&mut self, args: &Value) -> Result<(), String> {
        let items = args.as_array().ok_or("property change args are not a list")?;
        if items.len() < 6 {
            return Err(format!("property change args have {} elements, expected 6", items.len()));
        }
        let path = items[0].as_str().ok_or("property change without a path")?.to_string();
        let address = items[1].clone();
        let value = items[2].clone();
        let bytes_value = items[3].clone();
        let frozen = items[4].clone();
        let fields_changed: Vec<String> = items[5]
            .as_array()
            .map(|a| a.iter().filter_map(|x| x.as_str().map(|s| s.to_string())).collect())
            .unwrap_or_default();
        if !self.store.is_loaded() {
            log::debug!("[GameHook Client] Mapper is not loaded, ignoring PropertyUpdated event for: {}: {}", path, value);
            return Ok(());
        }
        if !self.store.contains(&path) {
            log::debug!("[GameHook Client] Could not find a related propery in PropertyUpdated event for: {}: {}", path, value);
            return Ok(());
        }
        if self.ignore_properties.contains(&path) {
            self.ignored_updates.insert(path, args.clone());
            return Ok(());
        }
        let old_property = self.store.props.get(&path).cloned().unwrap();
        if value == old_property.value && bytes_value == old_property.bytes_value {
            return Ok(());
        }
        let new_property = {
            let p = self.store.props.get_mut(&path).unwrap();
            p.address = address;
            p.value = value;
            p.bytes_value = bytes_value;
            p.frozen = frozen;
            p.clone()
        };
        if fields_changed.iter().any(|f| f == "value") && new_property.value != old_property.value && self.watched.contains(&path) {
            // the FSM must never take the client down: a panic becomes a game hook error
            let store = &self.store;
            let session = &mut self.session;
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                session.on_property_changed(store, &new_property, &old_property);
            }));
            if let Err(panic) = result {
                let msg = panic_message(&panic);
                log::error!("error encountered running callback_fn for {}: {}", path, msg);
            }
        }
        Ok(())
    }
}

fn panic_message(panic: &Box<dyn std::any::Any + Send>) -> String {
    if let Some(s) = panic.downcast_ref::<&str>() {
        s.to_string()
    } else if let Some(s) = panic.downcast_ref::<String>() {
        s.clone()
    } else {
        "unknown panic".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_from_mapper_and_case_resolution() {
        let mapper = json!({
            "meta": {"gameName": "Pokemon Yellow"},
            "glossary": {},
            "properties": [
                {"path": "player.team.0.species", "value": "Pikachu", "bytes": [25], "size": 1, "address": 100},
                {"path": "bag.money", "value": 3000, "bytes": [0, 48, 0]},
            ]
        });
        let store = PropertyStore::from_mapper(&mapper).unwrap();
        assert!(store.is_loaded());
        assert_eq!(store.game_name(), Some("Pokemon Yellow"));
        assert_eq!(store.get("bag.money").unwrap().as_i64(), Some(3000));
        assert_eq!(store.resolve_case("BAG.Money").as_deref(), Some("bag.money"));
        assert_eq!(store.resolve_case("nope"), None);
        assert_eq!(store.get_value(None), Value::Null);
        assert_eq!(store.get_value(Some("player.team.0.species")), json!("Pikachu"));
    }
}
