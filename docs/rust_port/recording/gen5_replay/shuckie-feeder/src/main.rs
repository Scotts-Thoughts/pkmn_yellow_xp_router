//! Headless Super Shuckie replay player that serves the game's memory to Poke-A-Byte.
//!
//! ```text
//! shuckie-feeder --rom <rom.nds> --replay <file.replay> [--port 55390] [--http 30190]
//!                [--speed 4] [--start <frame>] [--paused]
//! ```
//!
//! HTTP control (GET, all replies are JSON or plain text):
//!   /status                      frame, playing, speed, total frames
//!   /play  /pause                start/stop paced playback
//!   /speed?x=4                   playback speed multiplier (0 = unpaced)
//!   /goto?frame=N                seek (keyframe + hidden fast-forward, no memory served on the way)
//!   /runto?frame=N               play (paced, memory served every frame) until N, then pause
//!   /step?n=N                    run N frames immediately (unpaced, memory served), stay paused
//!   /screenshot?path=P           write both screens stacked as a PNG
//!   /read?addr=0x2000000&len=16  hex bytes of memory at the current frame
//!   /dump?path=P&addr=..&len=..  raw memory dump to a file
//!   /bookmarks                   the replay's bookmarks
//!   /keyframes                   first/last keyframe frame and count
//!   /trace?to=F&spec=name@0xADDR:LEN,...&out=P[&every=N]
//!                                run unpaced to F, writing every byte change of the regions
//!   /savescan?to=F&out=P[&every=N]
//!                                run unpaced to F, listing the frames the cartridge save changed
//!   /press?keys=a,start&frames=N[&touch=X,Y][&paced=1]
//!                                run N frames with this input instead of the replay's (the game
//!                                leaves the replay until the next /goto)
//!   /savestate?path=P  /loadstate?path=P
//!   /quit

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::time::{Duration, Instant};

use supershuckie_core::emulator::{EmulatorCore, NintendoDS};
use supershuckie_core::std_timestamp_provider;
use supershuckie_pokeabyte_integration::{PokeAByteEmulatorCommand, PokeAByteIntegrationServer};
use supershuckie_replay_recorder::replay_file::playback::{ReplayFilePlayer, ReplaySeekError};
use supershuckie_replay_recorder::Packet;

/// melonDS's native refresh: 33513982 / 560190 frames per second.
const NDS_FRAME: f64 = 560190.0 / 33513982.0;

struct Request {
    path: String,
    query: HashMap<String, String>,
    reply: Sender<String>,
}

struct Feeder {
    core: NintendoDS,
    player: ReplayFilePlayer,
    server: PokeAByteIntegrationServer,
    frame: u64,
    total_frames: u64,
    playing: bool,
    speed: f64,
    run_until: Option<u64>,
    stalled: bool,
    served_frames: u64,
    done: bool,
}

fn parse_num(s: &str) -> Option<u64> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        u64::from_str_radix(h, 16).ok()
    } else {
        s.parse().ok()
    }
}

impl Feeder {
    /// Feed replay packets up to and including the next `NextFrame`. `false` when the replay ends.
    fn feed_frame(&mut self) -> bool {
        loop {
            match self.player.next_packet() {
                Ok(None) | Err(_) => return false,
                Ok(Some(packet)) => match packet {
                    Packet::NextFrame { .. } => return true,
                    Packet::ChangeInput { data } => self.core.set_input_encoded(data.as_slice()),
                    Packet::WriteMemory { address, data } => {
                        if let Ok(a) = u32::try_from(*address) {
                            let _ = self.core.write_ram(a, data.as_slice());
                        }
                    }
                    Packet::ResetConsole => self.core.hard_reset(),
                    Packet::LoadSaveState { state } => {
                        let _ = self.core.load_save_state(state.as_slice());
                    }
                    _ => {}
                },
            }
        }
    }

    /// Emulate one frame of the replay. `serve` copies memory to Poke-A-Byte afterwards.
    fn run_frame(&mut self, serve: bool) -> bool {
        if self.stalled {
            return false;
        }
        if !self.feed_frame() {
            self.stalled = true;
            self.done = true;
            self.playing = false;
            eprintln!("[feeder] replay ended at frame {}", self.frame);
            return false;
        }
        self.core.run_unlocked();
        self.frame += 1;
        if serve {
            self.serve_memory();
        }
        true
    }

    fn serve_memory(&mut self) {
        let mut lock = self.server.get_session();
        let Some(session) = lock.as_mut() else { return };
        // Poke-A-Byte never writes during a recording test; drop anything it sends.
        for cmd in &mut session.writes {
            if !matches!(cmd, PokeAByteEmulatorCommand::Reset) {
                eprintln!("[feeder] ignoring Poke-A-Byte command {cmd:?}");
            }
        }
        let ram = unsafe { session.shared_memory.get_memory_mut() };
        for block in &session.config.blocks {
            if let Some(into) = ram.get_mut(block.range.clone()) {
                let _ = self.core.read_ram(block.game_address, into);
            }
        }
        session.finish_frame();
        self.served_frames += 1;
    }

    fn seek(&mut self, target: u64) -> Result<(), String> {
        let mut kf = target;
        loop {
            match self.player.go_to_keyframe(kf) {
                Ok(()) => break,
                Err(ReplaySeekError::NoSuchKeyframe { best, .. }) => {
                    if best > target {
                        return Err(format!("no keyframe at or before {target}"));
                    }
                    kf = best;
                }
                Err(e) => return Err(format!("seek error: {e:?}")),
            }
        }
        let meta = match self.player.next_packet() {
            Ok(Some(Packet::Keyframe { metadata, .. })) => metadata.clone(),
            other => return Err(format!("expected a keyframe packet, got {other:?}")),
        };
        self.core.load_save_state(self.player.current_keyframe_state()).map_err(|e| format!("load state: {e}"))?;
        self.core.set_input_encoded(meta.input.as_slice());
        self.frame = meta.elapsed_frames;
        self.stalled = false;
        self.done = false;
        self.core.set_skip_drawing(true);
        while self.frame < target {
            if !self.run_frame(false) {
                break;
            }
        }
        self.core.set_skip_drawing(false);
        // one drawn frame so screenshots show the target
        if self.frame == target && target > 0 {
            // nothing: the screen may be stale by one frame; callers can /step?n=1
        }
        self.serve_memory();
        Ok(())
    }

    fn screenshot(&self, path: &str) -> Result<(), String> {
        let screens = self.core.get_screens();
        let width = screens.iter().map(|s| s.width).max().unwrap_or(0);
        let height: usize = screens.iter().map(|s| s.height).sum();
        let mut rgb = vec![0u8; width * height * 3];
        let mut y0 = 0;
        for s in screens {
            for y in 0..s.height {
                for x in 0..s.width {
                    let p = s.pixels[y * s.width + x];
                    let o = ((y0 + y) * width + x) * 3;
                    rgb[o] = (p >> 16) as u8;
                    rgb[o + 1] = (p >> 8) as u8;
                    rgb[o + 2] = p as u8;
                }
            }
            y0 += s.height;
        }
        let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
        let mut enc = png::Encoder::new(std::io::BufWriter::new(file), width as u32, height as u32);
        enc.set_color(png::ColorType::Rgb);
        enc.set_depth(png::BitDepth::Eight);
        let mut w = enc.write_header().map_err(|e| e.to_string())?;
        w.write_image_data(&rgb).map_err(|e| e.to_string())?;
        Ok(())
    }

    fn status(&self) -> String {
        format!(
            "{{\"done\":{},\"frame\":{},\"total_frames\":{},\"playing\":{},\"speed\":{},\"run_until\":{},\"stalled\":{},\"served_frames\":{},\"session\":{}}}",
            self.done,
            self.frame,
            self.total_frames,
            self.playing,
            self.speed,
            self.run_until.map(|f| f.to_string()).unwrap_or("null".into()),
            self.stalled,
            self.served_frames,
            self.server.get_session().is_some()
        )
    }

    fn handle(&mut self, req: &Request) -> String {
        let q = |k: &str| req.query.get(k).cloned();
        match req.path.as_str() {
            "/status" => self.status(),
            "/play" => {
                self.playing = true;
                self.run_until = None;
                self.status()
            }
            "/pause" => {
                self.playing = false;
                self.run_until = None;
                self.status()
            }
            "/speed" => {
                if let Some(x) = q("x").and_then(|s| s.parse::<f64>().ok()) {
                    self.speed = x;
                }
                self.status()
            }
            "/goto" => match q("frame").and_then(|s| parse_num(&s)) {
                Some(f) => match self.seek(f) {
                    Ok(()) => self.status(),
                    Err(e) => format!("{{\"error\":{:?}}}", e),
                },
                None => "{\"error\":\"frame required\"}".into(),
            },
            "/runto" => match q("frame").and_then(|s| parse_num(&s)) {
                Some(f) if f > self.frame => {
                    self.run_until = Some(f);
                    self.playing = true;
                    self.done = false;
                    self.status()
                }
                Some(_) => "{\"error\":\"frame must be ahead\"}".into(),
                None => "{\"error\":\"frame required\"}".into(),
            },
            "/step" => {
                let n = q("n").and_then(|s| parse_num(&s)).unwrap_or(1);
                for _ in 0..n {
                    if !self.run_frame(true) {
                        break;
                    }
                }
                self.status()
            }
            "/screenshot" => match q("path") {
                Some(p) => match self.screenshot(&p) {
                    Ok(()) => format!("{{\"ok\":true,\"frame\":{}}}", self.frame),
                    Err(e) => format!("{{\"error\":{:?}}}", e),
                },
                None => "{\"error\":\"path required\"}".into(),
            },
            "/read" => {
                let addr = q("addr").and_then(|s| parse_num(&s)).unwrap_or(0) as u32;
                let len = q("len").and_then(|s| parse_num(&s)).unwrap_or(16) as usize;
                let mut buf = vec![0u8; len];
                match self.core.read_ram(addr, &mut buf) {
                    Ok(()) => buf.iter().map(|b| format!("{b:02x}")).collect::<String>(),
                    Err(e) => format!("{{\"error\":{:?}}}", e),
                }
            }
            "/dump" => {
                let addr = q("addr").and_then(|s| parse_num(&s)).unwrap_or(0x0200_0000) as u32;
                let len = q("len").and_then(|s| parse_num(&s)).unwrap_or(0x40_0000) as usize;
                let Some(path) = q("path") else { return "{\"error\":\"path required\"}".into() };
                let mut buf = vec![0u8; len];
                match self.core.read_ram(addr, &mut buf) {
                    Ok(()) => match std::fs::write(&path, &buf) {
                        Ok(()) => format!("{{\"ok\":true,\"frame\":{}}}", self.frame),
                        Err(e) => format!("{{\"error\":{:?}}}", e.to_string()),
                    },
                    Err(e) => format!("{{\"error\":{:?}}}", e),
                }
            }
            "/bookmarks" => {
                let t = self.player.bookmark_table();
                let items: Vec<String> = t.bookmarks.iter().map(|b| format!("{{\"name\":{:?},\"frame\":{},\"out\":{}}}", b.name, b.in_frame, b.out_frame().map(|f| f.to_string()).unwrap_or("null".into()))).collect();
                format!("[{}]", items.join(","))
            }
            "/keyframes" => {
                let k = self.player.all_keyframes();
                format!(
                    "{{\"count\":{},\"first\":{},\"last\":{}}}",
                    k.len(),
                    k.keys().next().copied().unwrap_or(0),
                    k.keys().next_back().copied().unwrap_or(0)
                )
            }
            "/trace" => {
                // /trace?to=F&spec=name@0xADDR:LEN,name2@0xADDR:LEN&out=PATH[&every=N]
                // Runs unpaced (memory not served) from the current frame to F and writes every
                // byte change of the given regions: "F <frame>" then "<name> <off>:<hex> ..." lines.
                let Some(to) = q("to").and_then(|s| parse_num(&s)) else { return "{\"error\":\"to required\"}".into() };
                let Some(out) = q("out") else { return "{\"error\":\"out required\"}".into() };
                let every = q("every").and_then(|s| parse_num(&s)).unwrap_or(1).max(1);
                let spec = q("spec").unwrap_or_default();
                let mut regions: Vec<(String, u32, usize, Vec<u8>)> = Vec::new();
                for item in spec.split(',').filter(|s| !s.is_empty()) {
                    let Some((name, rest)) = item.split_once('@') else { return format!("{{\"error\":\"bad spec {item}\"}}") };
                    let Some((addr, len)) = rest.split_once(':') else { return format!("{{\"error\":\"bad spec {item}\"}}") };
                    let (Some(addr), Some(len)) = (parse_num(addr), parse_num(len)) else { return format!("{{\"error\":\"bad spec {item}\"}}") };
                    regions.push((name.to_string(), addr as u32, len as usize, Vec::new()));
                }
                let file = match std::fs::File::create(&out) { Ok(f) => f, Err(e) => return format!("{{\"error\":{:?}}}", e.to_string()) };
                let mut w = std::io::BufWriter::new(file);
                let mut buf = Vec::new();
                let started = Instant::now();
                self.core.set_skip_drawing(true);
                let mut frames = 0u64;
                loop {
                    if self.frame % every == 0 || self.frame >= to {
                        let mut header_written = false;
                        for (name, addr, len, last) in regions.iter_mut() {
                            buf.resize(*len, 0);
                            let _ = self.core.read_ram(*addr, &mut buf);
                            if *last == buf {
                                continue;
                            }
                            if !header_written {
                                let _ = writeln!(w, "F {}", self.frame);
                                header_written = true;
                            }
                            let _ = write!(w, "{}", name);
                            if last.len() != buf.len() {
                                let _ = write!(w, " 0:");
                                for b in buf.iter() { let _ = write!(w, "{b:02x}"); }
                            } else {
                                let mut i = 0;
                                while i < buf.len() {
                                    if buf[i] == last[i] { i += 1; continue; }
                                    let s = i;
                                    // merge runs separated by < 4 equal bytes
                                    let mut e = i;
                                    let mut j = i;
                                    while j < buf.len() && j - e <= 4 {
                                        if buf[j] != last[j] { e = j; }
                                        j += 1;
                                    }
                                    let _ = write!(w, " {}:", s);
                                    for b in &buf[s..=e] { let _ = write!(w, "{b:02x}"); }
                                    i = e + 1;
                                }
                            }
                            let _ = writeln!(w);
                            last.clear();
                            last.extend_from_slice(&buf);
                        }
                    }
                    if self.frame >= to || !self.run_frame(false) {
                        break;
                    }
                    frames += 1;
                }
                self.core.set_skip_drawing(false);
                let _ = w.flush();
                self.serve_memory();
                format!("{{\"ok\":true,\"frame\":{},\"frames\":{},\"secs\":{:.1}}}", self.frame, frames, started.elapsed().as_secs_f64())
            }
            "/savescan" => {
                // /savescan?to=F&every=N&out=PATH: frames at which the cartridge save changed
                let Some(to) = q("to").and_then(|s| parse_num(&s)) else { return "{\"error\":\"to required\"}".into() };
                let Some(out) = q("out") else { return "{\"error\":\"out required\"}".into() };
                let every = q("every").and_then(|s| parse_num(&s)).unwrap_or(30).max(1);
                use std::hash::{Hash, Hasher};
                let hash = |v: &[u8]| { let mut h = std::collections::hash_map::DefaultHasher::new(); v.hash(&mut h); h.finish() };
                let mut last = hash(&self.core.save_sram());
                let mut lines = Vec::new();
                self.core.set_skip_drawing(true);
                while self.frame < to {
                    if !self.run_frame(false) { break; }
                    if self.frame % every == 0 {
                        let h = hash(&self.core.save_sram());
                        if h != last {
                            lines.push(self.frame.to_string());
                            last = h;
                        }
                    }
                }
                self.core.set_skip_drawing(false);
                let _ = std::fs::write(&out, lines.join("
"));
                self.serve_memory();
                format!("{{\"ok\":true,\"frame\":{},\"changes\":{}}}", self.frame, lines.len())
            }
            "/press" => {
                // /press?keys=a,start,l,r,up&frames=N[&touch=X,Y][&paced=1]: run N frames with this
                // input instead of the replay's (the game diverges from the replay until /goto)
                let frames = q("frames").and_then(|s| parse_num(&s)).unwrap_or(1);
                let paced = q("paced").as_deref() == Some("1");
                let mut input = supershuckie_core::emulator::Input::new();
                for k in q("keys").unwrap_or_default().split(',') {
                    match k.trim() {
                        "a" => input.a = true,
                        "b" => input.b = true,
                        "x" => input.x = true,
                        "y" => input.y = true,
                        "l" => input.l = true,
                        "r" => input.r = true,
                        "start" => input.start = true,
                        "select" => input.select = true,
                        "up" => input.d_up = true,
                        "down" => input.d_down = true,
                        "left" => input.d_left = true,
                        "right" => input.d_right = true,
                        _ => {}
                    }
                }
                if let Some((x, y)) = q("touch").and_then(|s| s.split_once(',').map(|(a, b)| (a.parse::<u8>().ok(), b.parse::<u8>().ok()))) {
                    if let (Some(x), Some(y)) = (x, y) {
                        input.touch = Some((x, y));
                    }
                }
                let mut enc = Vec::new();
                self.core.encode_input(input, &mut enc);
                self.core.set_input_encoded(&enc);
                let period = Duration::from_secs_f64(NDS_FRAME / self.speed.max(0.25));
                for _ in 0..frames {
                    let t = Instant::now();
                    self.core.run_unlocked();
                    self.frame += 1;
                    self.serve_memory();
                    if paced {
                        if let Some(rest) = period.checked_sub(t.elapsed()) {
                            std::thread::sleep(rest);
                        }
                    }
                }
                // release everything afterwards
                let mut enc = Vec::new();
                self.core.encode_input(supershuckie_core::emulator::Input::new(), &mut enc);
                self.core.set_input_encoded(&enc);
                self.stalled = true;
                self.status()
            }
            "/savestate" => match q("path") {
                Some(p) => match std::fs::write(&p, self.core.create_save_state()) {
                    Ok(()) => format!("{{\"ok\":true,\"frame\":{}}}", self.frame),
                    Err(e) => format!("{{\"error\":{:?}}}", e.to_string()),
                },
                None => "{\"error\":\"path required\"}".into(),
            },
            "/loadstate" => match q("path").and_then(|p| std::fs::read(p).ok()) {
                Some(state) => match self.core.load_save_state(&state) {
                    Ok(()) => {
                        self.stalled = true;
                        self.serve_memory();
                        self.status()
                    }
                    Err(e) => format!("{{\"error\":{:?}}}", e),
                },
                None => "{\"error\":\"readable path required\"}".into(),
            },
            "/quit" => {
                std::process::exit(0);
            }
            other => format!("{{\"error\":\"unknown route {}\"}}", other),
        }
    }
}

fn http_thread(listener: TcpListener, tx: Sender<Request>) {
    for stream in listener.incoming() {
        let Ok(stream) = stream else { continue };
        let tx = tx.clone();
        std::thread::spawn(move || handle_conn(stream, tx));
    }
}

fn handle_conn(mut stream: TcpStream, tx: Sender<Request>) {
    let mut reader = BufReader::new(stream.try_clone().unwrap());
    let mut line = String::new();
    if reader.read_line(&mut line).is_err() {
        return;
    }
    // drain headers
    loop {
        let mut h = String::new();
        if reader.read_line(&mut h).is_err() || h.trim().is_empty() {
            break;
        }
    }
    let target = line.split_whitespace().nth(1).unwrap_or("/").to_string();
    let (path, qs) = target.split_once('?').unwrap_or((&target, ""));
    let mut query = HashMap::new();
    for pair in qs.split('&').filter(|p| !p.is_empty()) {
        let (k, v) = pair.split_once('=').unwrap_or((pair, ""));
        query.insert(k.to_string(), percent_decode(v));
    }
    let (rtx, rrx) = channel();
    let _ = tx.send(Request { path: path.to_string(), query, reply: rtx });
    let body = rrx.recv_timeout(Duration::from_secs(3600)).unwrap_or_else(|_| "{\"error\":\"timeout\"}".into());
    let _ = write!(stream, "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}", body.len(), body);
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(if b[i] == b'+' { b' ' } else { b[i] });
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut rom = None;
    let mut replay = None;
    let mut port: u16 = 55390;
    let mut http: u16 = 30190;
    let mut speed = 4.0;
    let mut start = 0u64;
    let mut paused = false;
    let mut until: Option<u64> = None;
    while let Some(a) = args.next() {
        match a.as_str() {
            "--rom" => rom = args.next(),
            "--replay" => replay = args.next(),
            "--port" => port = args.next().unwrap().parse().unwrap(),
            "--http" => http = args.next().unwrap().parse().unwrap(),
            "--speed" => speed = args.next().unwrap().parse().unwrap(),
            "--start" => start = parse_num(&args.next().unwrap()).unwrap(),
            "--paused" => paused = true,
            "--until" => until = Some(parse_num(&args.next().unwrap()).unwrap()),
            other => panic!("unexpected argument {other}"),
        }
    }
    let rom = std::fs::read(rom.expect("--rom required")).expect("read rom");
    let replay_path = replay.expect("--replay required");
    let t = Instant::now();
    let bytes = std::fs::read(&replay_path).expect("read replay");
    let player = ReplayFilePlayer::new(bytes, true).expect("parse replay");
    let total_frames = player.get_total_frames();
    eprintln!("[feeder] replay v{} {} frames, {} keyframes, parsed in {:.1}s", player.get_replay_version(), total_frames, player.all_keyframes().len(), t.elapsed().as_secs_f64());
    let core = NintendoDS::new_from_rom(&rom, None, std_timestamp_provider(), false).expect("load rom");
    let server = PokeAByteIntegrationServer::begin_listen(port).expect("bind poke-a-byte port");
    let mut feeder = Feeder { core, player, server, frame: 0, total_frames, playing: !paused, speed, run_until: None, stalled: false, served_frames: 0, done: false };
    feeder.seek(start).expect("initial seek");
    feeder.run_until = until;
    eprintln!("[feeder] at frame {}, serving Poke-A-Byte on udp {port}, control on http://127.0.0.1:{http}", feeder.frame);

    let listener = TcpListener::bind(("127.0.0.1", http)).expect("bind http");
    let (tx, rx): (Sender<Request>, Receiver<Request>) = channel();
    std::thread::spawn(move || http_thread(listener, tx));

    let mut next_due = Instant::now();
    let mut last_report = Instant::now();
    loop {
        // commands
        let wait = if feeder.playing { Duration::ZERO } else { Duration::from_millis(5) };
        match rx.recv_timeout(wait) {
            Ok(req) => {
                let body = feeder.handle(&req);
                let _ = req.reply.send(body);
                next_due = Instant::now();
                continue;
            }
            Err(_) => {}
        }
        if !feeder.playing {
            // keep Poke-A-Byte fed (a fresh session needs its first frame)
            feeder.serve_memory();
            continue;
        }
        if feeder.speed > 0.0 {
            let now = Instant::now();
            if now < next_due {
                let d = next_due - now;
                if d > Duration::from_millis(2) {
                    std::thread::sleep(d - Duration::from_millis(1));
                }
                while Instant::now() < next_due {
                    std::hint::spin_loop();
                }
            }
            next_due += Duration::from_secs_f64(NDS_FRAME / feeder.speed);
            // don't try to catch up more than a few frames after a stall
            if Instant::now() > next_due + Duration::from_millis(100) {
                next_due = Instant::now();
            }
        }
        if !feeder.run_frame(true) {
            continue;
        }
        if let Some(until) = feeder.run_until {
            if feeder.frame >= until {
                feeder.playing = false;
                feeder.run_until = None;
                feeder.done = true;
                eprintln!("[feeder] reached frame {until}, paused");
            }
        }
        if last_report.elapsed() > Duration::from_secs(30) {
            last_report = Instant::now();
            eprintln!("[feeder] frame {} / {}", feeder.frame, feeder.total_frames);
        }
    }
}
