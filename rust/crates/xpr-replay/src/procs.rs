//! The two helper processes: `xpr-replay-worker` (the replay's memory) and
//! `pab-host` (Poke-A-Byte's mapper engine). Both answer with a u32 (LE)
//! length and that many bytes.

use std::collections::HashMap;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;

use serde_json::Value;

use crate::Change;

fn hidden(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NO_WINDOW: no console flashing up from the GUI app; BELOW_NORMAL_PRIORITY_CLASS:
        // an import gives way to whatever else the machine is doing
        cmd.creation_flags(0x0800_0000 | 0x0000_4000);
    }
    cmd
}

fn read_frame(r: &mut impl Read, what: &str) -> Result<Vec<u8>, String> {
    let mut len = [0u8; 4];
    r.read_exact(&mut len).map_err(|e| format!("{what} stopped answering: {e}"))?;
    let mut buf = vec![0u8; u32::from_le_bytes(len) as usize];
    r.read_exact(&mut buf).map_err(|e| format!("{what} stopped answering: {e}"))?;
    Ok(buf)
}

fn json_error(v: &Value) -> Option<String> {
    v.get("error").and_then(|e| e.as_str()).map(|s| s.to_string())
}

/// One answer of the worker.
pub enum WorkerMsg {
    /// memory at `frame`: the runs (u32 run count first) as pab-host's `S` command takes them
    Snapshot { frame: u64, ms: u64, runs: Vec<u8> },
    /// a job finished (`ended`: the replay ran out first)
    End { ended: bool },
}

pub struct WorkerProc {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl WorkerProc {
    pub fn start(exe: &Path, replay: &Path, rom: &Path) -> Result<WorkerProc, String> {
        let mut child = hidden(Command::new(exe).arg("--replay").arg(replay).arg("--rom").arg(rom))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
        let stdin = BufWriter::new(child.stdin.take().unwrap());
        let stdout = BufReader::with_capacity(1 << 20, child.stdout.take().unwrap());
        let mut w = WorkerProc { child, stdin, stdout };
        let hello = w.json()?;
        if let Some(e) = json_error(&hello) {
            return Err(e);
        }
        Ok(w)
    }

    fn json(&mut self) -> Result<Value, String> {
        let buf = read_frame(&mut self.stdout, "the replay worker")?;
        if buf.first() != Some(&b'J') {
            return Err("the replay worker sent something unexpected".into());
        }
        serde_json::from_slice(&buf[1..]).map_err(|e| format!("the replay worker sent bad JSON: {e}"))
    }

    fn command(&mut self, line: &str) -> Result<(), String> {
        writeln!(self.stdin, "{line}").and_then(|_| self.stdin.flush()).map_err(|e| format!("the replay worker stopped: {e}"))
    }

    pub fn set_blocks(&mut self, blocks: &[(u32, u32)]) -> Result<(), String> {
        let list: Vec<String> = blocks.iter().map(|(s, l)| format!("{s}:{l}")).collect();
        self.command(&format!("blocks {}", list.join(",")))?;
        let v = self.json()?;
        json_error(&v).map_or(Ok(()), Err)
    }

    pub fn start_job(&mut self, id: u64, pieces: &str) -> Result<(), String> {
        self.command(&format!("job {id} {pieces}"))
    }

    pub fn next(&mut self) -> Result<WorkerMsg, String> {
        let buf = read_frame(&mut self.stdout, "the replay worker")?;
        match buf.first() {
            Some(b'D') if buf.len() >= 21 => {
                let frame = u64::from_le_bytes(buf[1..9].try_into().unwrap());
                let ms = u64::from_le_bytes(buf[9..17].try_into().unwrap());
                Ok(WorkerMsg::Snapshot { frame, ms, runs: buf[17..].to_vec() })
            }
            Some(b'E') if buf.len() >= 10 => Ok(WorkerMsg::End { ended: buf[9] != 0 }),
            Some(b'J') => {
                let v: Value = serde_json::from_slice(&buf[1..]).unwrap_or(Value::Null);
                Err(json_error(&v).unwrap_or_else(|| "the replay worker sent something unexpected".into()))
            }
            _ => Err("the replay worker sent something unexpected".into()),
        }
    }
}

impl Drop for WorkerProc {
    fn drop(&mut self) {
        let _ = self.command("quit");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// `xpr-replay-worker --info`.
pub fn replay_info(exe: &Path, replay: &Path) -> Result<Value, String> {
    let out = hidden(Command::new(exe).arg("--replay").arg(replay).arg("--info"))
        .output()
        .map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
    if !out.status.success() {
        return Err(format!("cannot read the replay: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    serde_json::from_slice(&out.stdout).map_err(|e| format!("bad replay info: {e}"))
}

/// `xpr-replay-worker --find-rom`: the ROM the replay was recorded with.
pub fn find_rom(exe: &Path, replay: &Path, dirs: &[std::path::PathBuf]) -> Result<Option<std::path::PathBuf>, String> {
    let mut cmd = Command::new(exe);
    cmd.arg("--replay").arg(replay);
    for d in dirs {
        cmd.arg("--find-rom").arg(d);
    }
    let out = hidden(&mut cmd).output().map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
    if !out.status.success() {
        return Err(format!("cannot read the replay: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Ok(if s.is_empty() { None } else { Some(s.into()) })
}

pub struct PabProc {
    child: Child,
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    pub blocks: Vec<(u32, u32)>,
    pub game_name: String,
    paths: HashMap<String, Arc<str>>,
}

impl PabProc {
    pub fn start(exe: &Path, mappers: &Path, mapper: &str) -> Result<PabProc, String> {
        let mut child = hidden(Command::new(exe).arg("--mappers").arg(mappers).arg("--mapper").arg(mapper).arg("--compact"))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|e| format!("cannot start {}: {e}", exe.display()))?;
        let stdin = BufWriter::with_capacity(1 << 20, child.stdin.take().unwrap());
        let stdout = BufReader::with_capacity(1 << 20, child.stdout.take().unwrap());
        let mut p = PabProc { child, stdin, stdout, blocks: Vec::new(), game_name: String::new(), paths: HashMap::new() };
        let hello = p.json()?;
        if let Some(e) = json_error(&hello) {
            return Err(e);
        }
        p.blocks = hello
            .get("blocks")
            .and_then(|b| b.as_array())
            .ok_or("pab-host did not list its memory blocks")?
            .iter()
            .filter_map(|b| Some((b.get(0)?.as_u64()? as u32, b.get(1)?.as_u64()? as u32)))
            .collect();
        p.game_name = hello.get("gameName").and_then(|g| g.as_str()).unwrap_or("").to_string();
        Ok(p)
    }

    fn raw(&mut self) -> Result<Vec<u8>, String> {
        read_frame(&mut self.stdout, "pab-host")
    }

    fn json(&mut self) -> Result<Value, String> {
        let buf = self.raw()?;
        serde_json::from_slice(&buf).map_err(|e| format!("pab-host sent bad JSON: {e}"))
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.stdin.write_all(bytes).and_then(|_| self.stdin.flush()).map_err(|e| format!("pab-host stopped: {e}"))
    }

    /// Patch the memory with `runs` (the worker's snapshot body) and read:
    /// the properties whose value or address changed.
    pub fn step(&mut self, runs: &[u8]) -> Result<Vec<Change>, String> {
        self.stdin.write_all(b"S").map_err(|e| format!("pab-host stopped: {e}"))?;
        self.send(runs)?;
        let items = match self.json()? {
            Value::Array(a) => a,
            other => return Err(json_error(&other).unwrap_or_else(|| "pab-host sent something unexpected".into())),
        };
        let mut out = Vec::with_capacity(items.len());
        for mut item in items {
            let Some(path) = item.get("path").and_then(|p| p.as_str()) else { continue };
            let path = match self.paths.get(path) {
                Some(p) => p.clone(),
                None => {
                    let p: Arc<str> = Arc::from(path);
                    self.paths.insert(path.to_string(), p.clone());
                    p
                }
            };
            let fields = item.get("fieldsChanged").and_then(|f| f.as_array()).map(|f| Change::fields_from(f)).unwrap_or(crate::change::VALUE);
            let address = item.get("address").and_then(|a| a.as_u64());
            let value = item.get_mut("value").map(Value::take).unwrap_or(Value::Null);
            out.push(Change { path, address, value, fields });
        }
        Ok(out)
    }

    /// `GET /mapper` now.
    pub fn mapper(&mut self) -> Result<Value, String> {
        self.send(b"M")?;
        let v = self.json()?;
        match json_error(&v) {
            Some(e) => Err(e),
            None => Ok(v),
        }
    }

    /// The script's plain-data state (`X`), to seed another instance with.
    pub fn export_state(&mut self) -> Result<Vec<u8>, String> {
        self.send(b"X")?;
        let buf = self.raw()?;
        if buf.starts_with(b"{\"error\"") {
            let v: Value = serde_json::from_slice(&buf).unwrap_or(Value::Null);
            return Err(json_error(&v).unwrap_or_else(|| "pab-host could not export its state".into()));
        }
        Ok(buf)
    }

    /// Seed this instance with another's exported state (`I`).
    pub fn import_state(&mut self, state: &[u8]) -> Result<(), String> {
        let mut msg = Vec::with_capacity(state.len() + 5);
        msg.push(b'I');
        msg.extend_from_slice(&(state.len() as u32).to_le_bytes());
        msg.extend_from_slice(state);
        self.send(&msg)?;
        let v = self.json()?;
        json_error(&v).map_or(Ok(()), Err)
    }

    pub fn reset(&mut self) -> Result<(), String> {
        self.send(b"R")?;
        let v = self.json()?;
        json_error(&v).map_or(Ok(()), Err)
    }
}

impl Drop for PabProc {
    fn drop(&mut self) {
        let _ = self.send(b"Q");
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
