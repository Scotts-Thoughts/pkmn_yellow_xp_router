//! Reads game memory out of a Super Shuckie replay for replay_to_route.
//!
//! ```text
//! xpr-replay-worker --replay <file.replay> --info
//!     Print the replay's header and keyframe frames as JSON and exit (no ROM needed).
//! xpr-replay-worker --replay <file.replay> --find-rom <dir> [--find-rom <dir> ...]
//!     Print the path of the ROM (searched recursively) whose checksum the replay was recorded
//!     with, or nothing.
//! xpr-replay-worker --replay <file.replay> --rom <rom> [--allow-mismatch]
//!     Serve memory: commands on stdin, one per line, answers on stdout.
//! ```
//!
//! Commands:
//!
//! ```text
//! blocks <start>:<length>,...       the memory blocks to read (Poke-A-Byte's transfer blocks)
//! job <id> <piece>;<piece>;...      read memory at the frames the pieces name, in order:
//!     s <from> <to>                 at every keyframe in [from, to] (loaded, not emulated)
//!     d <from> <to> <stride>        at from, from + stride, ... up to to (emulated)
//! quit
//! ```
//!
//! Every answer is a u32 (LE) length and that many bytes; the first byte says what it is:
//!
//! ```text
//! 'J' JSON                          ready / errors: {"ready": ...} or {"error": "..."}
//! 'D' u64 frame, u64 replay ms, u32 run_count, { u32 block, u32 offset, u32 length, bytes }*
//!                                   memory at a frame, as the runs that differ from the last
//!                                   'D' of the same job (the first one of a job is complete)
//! 'E' u64 job id, u8 reached_end    the job is done (reached_end: the replay ran out)
//! ```

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use supershuckie_core::emulator::{EmulatorCore, GameBoyAdvance, GameBoyColor, Model, NintendoDS};
use supershuckie_core::{std_timestamp_provider, SuperShuckieCore};
use supershuckie_replay_recorder::replay_file::playback::ReplayFilePlayer;
use supershuckie_replay_recorder::replay_file::ReplayConsoleType;
use supershuckie_replay_recorder::blake3_hash;

const DMG_BOOT: &[u8] = include_bytes!("A:/Programs/supershuckie/bootrom/dmg/dmg.bin");
const CGB_BOOT: &[u8] = include_bytes!("A:/Programs/supershuckie/bootrom/cgb/cgb_boot/cgb_boot_fast.bin");
const GBA_BIOS: &[u8] = include_bytes!("A:/Programs/supershuckie/bootrom/agb/gba_bios.bin");

/// Emulating this many frames forward is cheaper than loading a keyframe and catching up.
const MAX_EMULATED_GAP: u64 = 240;

fn open_player(path: &Path) -> Result<ReplayFilePlayer, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    // SAFETY: the replay is opened read-only and nothing writes it while it is read.
    let map = unsafe { memmap2::Mmap::map(&file) }.map_err(|e| format!("cannot map {}: {e}", path.display()))?;
    let source: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(map);
    ReplayFilePlayer::new_shared(source, true).map_err(|e| format!("cannot read {}: {e:?}", path.display()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn console_name(c: ReplayConsoleType) -> &'static str {
    match c {
        ReplayConsoleType::GameBoy => "GB",
        ReplayConsoleType::SuperGameBoy2 => "SGB2",
        ReplayConsoleType::GameBoyColor => "GBC",
        ReplayConsoleType::GameBoyAdvance => "GBA",
        ReplayConsoleType::NintendoDS => "NDS",
        _ => "unknown",
    }
}

fn info(player: &ReplayFilePlayer) -> String {
    let m = player.get_replay_metadata();
    let keyframes: Vec<String> = player.all_keyframes().keys().map(|k| k.to_string()).collect();
    let opt = |v: Option<(u64, u64)>| v.map(|(f, ms)| format!("[{f},{ms}]")).unwrap_or_else(|| "null".into());
    let crop_start = m.crop_start.map(|(f, ms)| (f, ms.0));
    let crop_end = m.crop_end.map(|(f, ms)| (f, ms.0));
    format!(
        "{{\"console\":{},\"rom_name\":{},\"rom_filename\":{},\"rom_checksum\":\"{}\",\"emulator_core\":{},\"total_frames\":{},\"total_ms\":{},\"crop_start\":{},\"crop_end\":{},\"timer_offset\":{},\"keyframes\":[{}]}}",
        json_str(console_name(m.console_type)),
        json_str(&m.rom_name),
        json_str(&m.rom_filename),
        hex(&m.rom_checksum),
        json_str(&m.emulator_core_name),
        player.get_total_frames(),
        player.get_total_milliseconds().0,
        opt(crop_start),
        opt(crop_end),
        m.timer_offset.map(|t| t.0.to_string()).unwrap_or_else(|| "null".into()),
        keyframes.join(","),
    )
}

/// The ROM with the replay's checksum under `dirs` (at most 3 folders deep): files named
/// like the replay's ROM first, then every GB/GBC/GBA/NDS file.
fn find_rom(player: &ReplayFilePlayer, dirs: &[PathBuf]) -> Option<PathBuf> {
    let want = player.get_replay_metadata().rom_checksum;
    let named = player.get_replay_metadata().rom_filename.to_lowercase();
    let exts = ["gb", "gbc", "gba", "nds"];
    let mut candidates = Vec::new();
    let mut stack: Vec<(PathBuf, u32)> = dirs.iter().map(|d| (d.clone(), 0)).collect();
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for e in entries.flatten() {
            let p = e.path();
            let Ok(ft) = e.file_type() else { continue };
            if ft.is_dir() {
                if depth < 3 {
                    stack.push((p, depth + 1));
                }
            } else if p.extension().and_then(|x| x.to_str()).map(|x| exts.contains(&x.to_ascii_lowercase().as_str())).unwrap_or(false) {
                candidates.push(p);
            }
        }
    }
    let is_named = |p: &PathBuf| p.file_name().and_then(|n| n.to_str()).map(|n| n.to_lowercase() == named).unwrap_or(false);
    candidates.sort_by_key(|p| !is_named(p));
    candidates.dedup();
    candidates.into_iter().find(|p| std::fs::read(p).map(|d| blake3_hash(&d) == want).unwrap_or(false))
}

struct Worker {
    core: SuperShuckieCore,
    keyframes: Vec<u64>,
    blocks: Vec<(u32, usize)>,
    last: Vec<Vec<u8>>,
    cur: Vec<Vec<u8>>,
    out: std::io::BufWriter<std::fs::File>,
    ended: bool,
}

impl Worker {
    fn send(&mut self, payload: &[u8]) {
        let _ = self.out.write_all(&(payload.len() as u32).to_le_bytes());
        let _ = self.out.write_all(payload);
    }

    fn send_json(&mut self, json: &str) {
        let mut p = Vec::with_capacity(json.len() + 1);
        p.push(b'J');
        p.extend_from_slice(json.as_bytes());
        self.send(&p);
        let _ = self.out.flush();
    }

    /// Read the blocks and send what changed since the last snapshot of this job.
    fn snapshot(&mut self) {
        for (i, (start, len)) in self.blocks.iter().enumerate() {
            let buf = &mut self.cur[i];
            buf.resize(*len, 0);
            if self.core.get_core().read_ram(*start, buf).is_err() {
                // An unmapped block reads as zeroes, like Poke-A-Byte's never-filled buffer.
                buf.fill(0);
            }
        }
        let (frame, ms) = self.core.replay_position();
        let mut p = Vec::with_capacity(64);
        p.push(b'D');
        p.extend_from_slice(&frame.to_le_bytes());
        p.extend_from_slice(&ms.0.to_le_bytes());
        let count_at = p.len();
        p.extend_from_slice(&0u32.to_le_bytes());
        let mut runs = 0u32;
        for i in 0..self.blocks.len() {
            let (a, b) = (&self.last[i], &self.cur[i]);
            let n = b.len();
            if a.len() != n {
                p.extend_from_slice(&(i as u32).to_le_bytes());
                p.extend_from_slice(&0u32.to_le_bytes());
                p.extend_from_slice(&(n as u32).to_le_bytes());
                p.extend_from_slice(b);
                runs += 1;
                continue;
            }
            let mut j = 0;
            while j < n {
                // Skip equal 8-byte chunks quickly.
                if j + 8 <= n && a[j..j + 8] == b[j..j + 8] {
                    j += 8;
                    continue;
                }
                if a[j] == b[j] {
                    j += 1;
                    continue;
                }
                let start = j;
                let mut end = j + 1;
                // Extend while differences are less than 16 bytes apart.
                let mut k = end;
                while k < n && k < end + 16 {
                    if a[k] != b[k] {
                        end = k + 1;
                    }
                    k += 1;
                }
                p.extend_from_slice(&(i as u32).to_le_bytes());
                p.extend_from_slice(&(start as u32).to_le_bytes());
                p.extend_from_slice(&((end - start) as u32).to_le_bytes());
                p.extend_from_slice(&b[start..end]);
                runs += 1;
                j = end;
            }
        }
        p[count_at..count_at + 4].copy_from_slice(&runs.to_le_bytes());
        // `last` becomes what was just sent; the next read overwrites `cur` whole.
        std::mem::swap(&mut self.last, &mut self.cur);
        self.send(&p);
    }

    fn load_keyframe(&mut self, frame: u64) -> Result<(), String> {
        self.core.go_to_replay_keyframe(frame)?;
        self.ended = false;
        Ok(())
    }

    /// Put the emulator at `frame` (emulated from the nearest keyframe when far away).
    /// `false` when the replay ends first.
    fn goto(&mut self, frame: u64) -> Result<bool, String> {
        let cur = self.core.total_frames();
        if frame < cur || frame > cur + MAX_EMULATED_GAP || self.ended {
            let kf = match self.keyframes.binary_search(&frame) {
                Ok(i) => self.keyframes[i],
                Err(0) => return Err(format!("no keyframe at or before frame {frame}")),
                Err(i) => self.keyframes[i - 1],
            };
            if !(kf <= cur && cur <= frame && !self.ended) {
                self.load_keyframe(kf)?;
            }
        }
        while self.core.total_frames() < frame {
            let before = self.core.total_frames();
            self.core.run_unlocked_hidden();
            if self.core.is_replay_stalled() || (self.core.total_frames() == before && self.core.is_replay_stalled()) {
                self.ended = true;
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn run_job(&mut self, id: u64, pieces: &str) -> Result<(), String> {
        for l in &mut self.last {
            l.clear();
        }
        let mut reached_end = false;
        'pieces: for piece in pieces.split(';').map(str::trim).filter(|p| !p.is_empty()) {
            let parts: Vec<&str> = piece.split_whitespace().collect();
            let num = |i: usize| -> Result<u64, String> {
                parts.get(i).ok_or_else(|| format!("bad piece '{piece}'"))?.parse::<u64>().map_err(|e| format!("bad piece '{piece}': {e}"))
            };
            match parts.first().copied() {
                Some("s") => {
                    let (from, to) = (num(1)?, num(2)?);
                    let lo = self.keyframes.partition_point(|&k| k < from);
                    let hi = self.keyframes.partition_point(|&k| k <= to);
                    for i in lo..hi {
                        let k = self.keyframes[i];
                        self.load_keyframe(k)?;
                        self.snapshot();
                    }
                }
                Some("d") => {
                    let (from, to, stride) = (num(1)?, num(2)?, num(3)?.max(1));
                    let mut f = from;
                    while f <= to {
                        if !self.goto(f)? {
                            reached_end = true;
                            break 'pieces;
                        }
                        self.snapshot();
                        f += stride;
                    }
                }
                _ => return Err(format!("bad piece '{piece}'")),
            }
        }
        let mut p = vec![b'E'];
        p.extend_from_slice(&id.to_le_bytes());
        p.push(reached_end as u8);
        self.send(&p);
        let _ = self.out.flush();
        Ok(())
    }
}

/// The protocol's stdout, with stdout itself pointed at stderr: the emulator cores (C/C++)
/// may print, and a stray byte in the protocol stream would desynchronise it.
#[cfg(windows)]
fn protocol_out() -> std::fs::File {
    use std::os::windows::io::FromRawHandle;
    unsafe extern "C" {
        fn _dup(fd: i32) -> i32;
        fn _dup2(src: i32, dst: i32) -> i32;
        fn _get_osfhandle(fd: i32) -> isize;
    }
    unsafe extern "system" {
        fn GetStdHandle(which: u32) -> isize;
        fn SetStdHandle(which: u32, handle: isize) -> i32;
    }
    const STD_OUTPUT_HANDLE: u32 = -11i32 as u32;
    const STD_ERROR_HANDLE: u32 = -12i32 as u32;
    unsafe {
        let fd = _dup(1);
        let handle = if fd >= 0 { _get_osfhandle(fd) } else { -1 };
        if handle == -1 {
            return std::fs::File::from_raw_handle(GetStdHandle(STD_OUTPUT_HANDLE) as _);
        }
        _dup2(2, 1);
        SetStdHandle(STD_OUTPUT_HANDLE, GetStdHandle(STD_ERROR_HANDLE));
        std::fs::File::from_raw_handle(handle as _)
    }
}

#[cfg(unix)]
fn protocol_out() -> std::fs::File {
    use std::os::unix::io::FromRawFd;
    unsafe extern "C" {
        fn dup(fd: i32) -> i32;
        fn dup2(src: i32, dst: i32) -> i32;
    }
    unsafe {
        let fd = dup(1);
        dup2(2, 1);
        std::fs::File::from_raw_fd(fd)
    }
}

fn make_core(rom: &[u8], player: &ReplayFilePlayer) -> Result<Box<dyn EmulatorCore>, String> {
    let m = player.get_replay_metadata();
    Ok(match m.console_type {
        ReplayConsoleType::GameBoy => Box::new(GameBoyColor::new_from_rom(rom, DMG_BOOT, None, Model::DmgB)),
        ReplayConsoleType::SuperGameBoy2 => Box::new(GameBoyColor::new_from_rom(rom, DMG_BOOT, None, Model::Sgb2)),
        ReplayConsoleType::GameBoyColor => Box::new(GameBoyColor::new_from_rom(rom, CGB_BOOT, None, Model::Cgb0)),
        ReplayConsoleType::GameBoyAdvance => {
            let bios: &[u8] = if m.bios_checksum == blake3_hash(GBA_BIOS) { GBA_BIOS } else { &[] };
            Box::new(GameBoyAdvance::new_from_rom(rom, None, bios, std_timestamp_provider()).map_err(|e| format!("mGBA rejected the ROM: {e}"))?)
        }
        ReplayConsoleType::NintendoDS => {
            Box::new(NintendoDS::new_from_rom(rom, None, std_timestamp_provider(), false).map_err(|e| format!("melonDS rejected the ROM: {e}"))?)
        }
        other => return Err(format!("unsupported console {other:?}")),
    })
}

fn main() {
    let mut replay = None;
    let mut rom = None;
    let mut want_info = false;
    let mut find_dirs = Vec::new();
    let mut allow_mismatch = false;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--replay" => replay = args.next().map(PathBuf::from),
            "--rom" => rom = args.next().map(PathBuf::from),
            "--info" => want_info = true,
            "--find-rom" => find_dirs.extend(args.next().map(PathBuf::from)),
            "--allow-mismatch" => allow_mismatch = true,
            other => {
                eprintln!("unknown argument {other}");
                std::process::exit(2);
            }
        }
    }
    let Some(replay) = replay else {
        eprintln!("usage: xpr-replay-worker --replay <file> (--info | --find-rom <dir>... | --rom <rom>)");
        std::process::exit(2);
    };
    let player = match open_player(&replay) {
        Ok(p) => p,
        Err(e) => {
            eprintln!("{e}");
            std::process::exit(1);
        }
    };
    if want_info {
        println!("{}", info(&player));
        return;
    }
    if !find_dirs.is_empty() {
        if let Some(p) = find_rom(&player, &find_dirs) {
            println!("{}", p.display());
        }
        return;
    }
    let Some(rom_path) = rom else {
        eprintln!("--rom is required to serve memory");
        std::process::exit(2);
    };

    let mut out = std::io::BufWriter::with_capacity(1 << 20, protocol_out());
    let fail = |out: &mut std::io::BufWriter<std::fs::File>, msg: String| -> ! {
        let json = format!("{{\"error\":{}}}", json_str(&msg));
        let mut p = vec![b'J'];
        p.extend_from_slice(json.as_bytes());
        let _ = out.write_all(&(p.len() as u32).to_le_bytes());
        let _ = out.write_all(&p);
        let _ = out.flush();
        std::process::exit(1);
    };
    let rom_bytes = match std::fs::read(&rom_path) {
        Ok(b) => b,
        Err(e) => fail(&mut out, format!("cannot read {}: {e}", rom_path.display())),
    };
    if !allow_mismatch && blake3_hash(&rom_bytes) != player.get_replay_metadata().rom_checksum {
        fail(&mut out, format!("{} is not the ROM this replay was recorded with", rom_path.display()));
    }
    let keyframes: Vec<u64> = player.all_keyframes().keys().copied().collect();
    let emu = match make_core(&rom_bytes, &player) {
        Ok(c) => c,
        Err(e) => fail(&mut out, e),
    };
    let mut core = SuperShuckieCore::new(emu, std_timestamp_provider());
    if let Err(e) = core.attach_replay_player(player, true) {
        fail(&mut out, format!("cannot attach the replay: {e:?}"));
    }

    let mut w = Worker { core, keyframes, blocks: Vec::new(), last: Vec::new(), cur: Vec::new(), out, ended: false };
    w.send_json("{\"ready\":true}");

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let line = line.trim();
        let (cmd, rest) = line.split_once(' ').unwrap_or((line, ""));
        match cmd {
            "" => {}
            "quit" => break,
            "blocks" => {
                let mut blocks = Vec::new();
                for b in rest.split(',').filter(|b| !b.is_empty()) {
                    let parsed = b.split_once(':').and_then(|(s, l)| Some((s.trim().parse::<u32>().ok()?, l.trim().parse::<usize>().ok()?)));
                    match parsed {
                        Some(x) => blocks.push(x),
                        None => {
                            w.send_json(&format!("{{\"error\":{}}}", json_str(&format!("bad block '{b}'"))));
                            continue;
                        }
                    }
                }
                w.last = blocks.iter().map(|_| Vec::new()).collect();
                w.cur = blocks.iter().map(|(_, l)| vec![0; *l]).collect();
                w.blocks = blocks;
                w.send_json("{\"blocks\":true}");
            }
            "job" => {
                let (id, pieces) = rest.split_once(' ').unwrap_or((rest, ""));
                let id = id.parse::<u64>().unwrap_or(0);
                if let Err(e) = w.run_job(id, pieces) {
                    w.send_json(&format!("{{\"error\":{},\"job\":{id}}}", json_str(&e)));
                }
            }
            other => w.send_json(&format!("{{\"error\":{}}}", json_str(&format!("unknown command '{other}'")))),
        }
        let _ = w.out.flush();
    }
}
