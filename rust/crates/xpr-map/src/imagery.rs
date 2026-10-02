//! The pictures of an image-world pack (gen 4/5), which are too big to embed
//! in the binary and so ship separately (GEN45_REQUIREMENTS §4.12): the
//! world as a 512 px webp tile pyramid (`world/<L>/<x>_<y>.webp`) and one
//! lossless webp per interior (`interiors/<name>.webp`).
//!
//! They are read from a directory laid out like that (a development checkout
//! of pokemap's `public/data/<game>/`, or `map_data/<game>/`) or straight out
//! of the pack's imagery zip, which pokemap writes with stored entries so
//! nothing needs extracting.

use std::collections::{HashMap, VecDeque};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use sha2::{Digest, Sha256};

use crate::lod::Pixmap;
use crate::model::ImageryInfo;

/// Read access to the imagery's files by their path inside the artifact.
pub trait ImageFiles: Send + Sync {
    fn read(&self, rel: &str) -> Option<Vec<u8>>;
    /// Where the files come from, for the status line.
    fn describe(&self) -> String;
}

/// `<root>/world/…` and `<root>/interiors/…` on disk.
pub struct DirFiles {
    root: PathBuf,
}

impl DirFiles {
    /// A directory holding the imagery (it has a `world` or an `interiors` folder).
    pub fn open(root: &Path) -> Option<DirFiles> {
        (root.join("world").is_dir() || root.join("interiors").is_dir()).then(|| DirFiles { root: root.to_path_buf() })
    }
}

impl ImageFiles for DirFiles {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        std::fs::read(self.root.join(rel)).ok()
    }
    fn describe(&self) -> String {
        self.root.display().to_string()
    }
}

/// Where a zip's bytes are.
enum ZipData {
    File(Mutex<File>),
    /// embedded in the executable
    Static(&'static [u8]),
}

struct ZipEntry {
    /// offset of the local file header
    header: u64,
    method: u16,
    csize: u64,
    usize: u64,
}

/// A zip archive read in place (stored or deflated entries).
pub struct ZipFiles {
    path: PathBuf,
    entries: HashMap<String, ZipEntry>,
    data: ZipData,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}
fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

impl ZipFiles {
    pub fn open(path: &Path) -> Result<ZipFiles, String> {
        let mut file = File::open(path).map_err(|e| format!("{}: {}", path.display(), e))?;
        let len = file.metadata().map_err(|e| e.to_string())?.len();
        // the end-of-central-directory record is in the last 64 KiB + 22 bytes
        let tail_len = len.min(65_557);
        file.seek(SeekFrom::Start(len - tail_len)).map_err(|e| e.to_string())?;
        let mut tail = vec![0u8; tail_len as usize];
        file.read_exact(&mut tail).map_err(|e| e.to_string())?;
        let (cd_offset, cd_size, count) = Self::eocd(&tail, len)?;
        file.seek(SeekFrom::Start(cd_offset)).map_err(|e| e.to_string())?;
        let mut cd = vec![0u8; cd_size as usize];
        file.read_exact(&mut cd).map_err(|e| e.to_string())?;
        let entries = Self::central_directory(&cd, count)?;
        Ok(ZipFiles { path: path.to_path_buf(), entries, data: ZipData::File(Mutex::new(file)) })
    }

    /// A zip embedded in the executable.
    pub fn from_static(name: &str, bytes: &'static [u8]) -> Result<ZipFiles, String> {
        let len = bytes.len() as u64;
        let tail = &bytes[bytes.len() - (bytes.len().min(65_557))..];
        let (cd_offset, cd_size, count) = Self::eocd(tail, len)?;
        let entries = Self::central_directory(&bytes[cd_offset as usize..(cd_offset + cd_size) as usize], count)?;
        Ok(ZipFiles { path: PathBuf::from(name), entries, data: ZipData::Static(bytes) })
    }

    /// (central directory offset, size, entry count) from the end of the archive.
    fn eocd(tail: &[u8], len: u64) -> Result<(u64, u64, usize), String> {
        let eocd = (0..tail.len().saturating_sub(21)).rev().find(|&i| u32_at(tail, i) == 0x0605_4b50).ok_or("not a zip file")?;
        let count = u16_at(tail, eocd + 10) as usize;
        let cd_size = u32_at(tail, eocd + 12) as u64;
        let cd_offset = u32_at(tail, eocd + 16) as u64;
        if cd_offset + cd_size > len {
            return Err("zip central directory is out of range".into());
        }
        Ok((cd_offset, cd_size, count))
    }

    fn central_directory(cd: &[u8], count: usize) -> Result<HashMap<String, ZipEntry>, String> {
        let mut entries = HashMap::with_capacity(count);
        let mut i = 0usize;
        for _ in 0..count {
            if i + 46 > cd.len() || u32_at(cd, i) != 0x0201_4b50 {
                return Err("bad zip central directory".into());
            }
            let method = u16_at(cd, i + 10);
            let csize = u32_at(cd, i + 20) as u64;
            let usize_ = u32_at(cd, i + 24) as u64;
            let nlen = u16_at(cd, i + 28) as usize;
            let xlen = u16_at(cd, i + 30) as usize;
            let clen = u16_at(cd, i + 32) as usize;
            let header = u32_at(cd, i + 42) as u64;
            let name = String::from_utf8_lossy(&cd[i + 46..i + 46 + nlen]).replace('\\', "/");
            entries.insert(name, ZipEntry { header, method, csize, usize: usize_ });
            i += 46 + nlen + xlen + clen;
        }
        Ok(entries)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

impl ImageFiles for ZipFiles {
    fn read(&self, rel: &str) -> Option<Vec<u8>> {
        let e = self.entries.get(rel)?;
        let data = match &self.data {
            ZipData::File(file) => {
                let mut f = file.lock().ok()?;
                f.seek(SeekFrom::Start(e.header)).ok()?;
                let mut lh = [0u8; 30];
                f.read_exact(&mut lh).ok()?;
                if u32_at(&lh, 0) != 0x0403_4b50 {
                    return None;
                }
                let skip = u16_at(&lh, 26) as i64 + u16_at(&lh, 28) as i64;
                f.seek(SeekFrom::Current(skip)).ok()?;
                let mut data = vec![0u8; e.csize as usize];
                f.read_exact(&mut data).ok()?;
                data
            }
            ZipData::Static(bytes) => {
                let h = e.header as usize;
                let lh = bytes.get(h..h + 30)?;
                if u32_at(lh, 0) != 0x0403_4b50 {
                    return None;
                }
                let start = h + 30 + u16_at(lh, 26) as usize + u16_at(lh, 28) as usize;
                bytes.get(start..start + e.csize as usize)?.to_vec()
            }
        };
        match e.method {
            0 => Some(data),
            8 => {
                let mut out = Vec::with_capacity(e.usize as usize);
                flate2::read::DeflateDecoder::new(&data[..]).read_to_end(&mut out).ok()?;
                Some(out)
            }
            _ => None,
        }
    }
    fn describe(&self) -> String {
        self.path.display().to_string()
    }
}

/// Imagery zips embedded in the executable, by game. The app's `main`
/// registers them (`xpr-app/build.rs` generates the calls); the library and
/// its tests have none.
static EMBEDDED: std::sync::OnceLock<Mutex<HashMap<String, &'static [u8]>>> = std::sync::OnceLock::new();

pub fn register_embedded(game: &str, zip: &'static [u8]) {
    EMBEDDED.get_or_init(Default::default).lock().unwrap().insert(game.to_string(), zip);
}

/// The embedded imagery zip of a game, if this executable has one.
pub fn embedded(game: &str) -> Option<&'static [u8]> {
    EMBEDDED.get()?.lock().ok()?.get(game).copied()
}

/// Check a zip against the pack manifest: its size, then its sha256.
pub fn verify_zip(path: &Path, info: &ImageryInfo) -> Result<(), String> {
    let len = std::fs::metadata(path).map_err(|e| e.to_string())?.len();
    if len != info.bytes {
        return Err(format!("the file is {} bytes; this map pack expects {} bytes", len, info.bytes));
    }
    let mut f = File::open(path).map_err(|e| e.to_string())?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 1 << 20];
    loop {
        let n = f.read(&mut buf).map_err(|e| e.to_string())?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    let got: String = h.finalize().iter().map(|b| format!("{:02x}", b)).collect();
    if got != info.sha256 {
        return Err("the file's checksum does not match this map pack (a different version?)".into());
    }
    Ok(())
}

/// Decode a webp / png into RGBA.
pub fn decode_rgba(bytes: &[u8]) -> Option<Pixmap> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    Some(Pixmap { w: w as usize, h: h as usize, data: img.into_raw() })
}

/// Halve an RGBA image (box filter over premultiplied colour, odd edges
/// averaged over what exists), for interior mip levels.
pub fn half(src: &Pixmap) -> Pixmap {
    let w = src.w.div_ceil(2).max(1);
    let h = src.h.div_ceil(2).max(1);
    let mut out = Pixmap::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let mut acc = [0u32; 4];
            let mut n = 0u32;
            for dy in 0..2 {
                for dx in 0..2 {
                    let (sx, sy) = (x * 2 + dx, y * 2 + dy);
                    if sx >= src.w || sy >= src.h {
                        continue;
                    }
                    let p = src.get(sx, sy);
                    let a = p[3] as u32;
                    acc[0] += p[0] as u32 * a;
                    acc[1] += p[1] as u32 * a;
                    acc[2] += p[2] as u32 * a;
                    acc[3] += a;
                    n += 1;
                }
            }
            if acc[3] == 0 {
                continue;
            }
            out.put(x, y, [(acc[0] / acc[3]) as u8, (acc[1] / acc[3]) as u8, (acc[2] / acc[3]) as u8, (acc[3] / n.max(1)) as u8]);
        }
    }
    out
}

const INTERIOR_CACHE: usize = 24;

/// Decoded interiors by path, and their insertion order (oldest first).
type InteriorCache = (HashMap<String, Arc<Vec<Pixmap>>>, VecDeque<String>);

/// An opened imagery set with a small cache of decoded interiors.
pub struct Imagery {
    files: Box<dyn ImageFiles>,
    /// interior path -> mip chain (level 0 = native)
    interiors: Mutex<InteriorCache>,
}

impl Imagery {
    pub fn new(files: Box<dyn ImageFiles>) -> Imagery {
        Imagery { files, interiors: Mutex::new((HashMap::new(), VecDeque::new())) }
    }

    pub fn describe(&self) -> String {
        self.files.describe()
    }

    /// A world pyramid tile, decoded; `None` where the pyramid has no tile
    /// (nothing is drawn there).
    pub fn world_tile(&self, level: u8, x: u32, y: u32) -> Option<Pixmap> {
        let bytes = self.files.read(&format!("world/{}/{}_{}.webp", level, x, y))?;
        decode_rgba(&bytes)
    }

    /// An interior image and its halvings, decoded once and cached.
    pub fn interior(&self, path: &str, levels: u8) -> Option<Arc<Vec<Pixmap>>> {
        {
            let cache = self.interiors.lock().ok()?;
            if let Some(m) = cache.0.get(path) {
                return Some(m.clone());
            }
        }
        let base = decode_rgba(&self.files.read(path)?)?;
        let mut mips = vec![base];
        while mips.len() <= levels as usize {
            let next = half(mips.last().unwrap());
            mips.push(next);
        }
        let mips = Arc::new(mips);
        let mut cache = self.interiors.lock().ok()?;
        cache.0.insert(path.to_string(), mips.clone());
        cache.1.push_back(path.to_string());
        while cache.1.len() > INTERIOR_CACHE {
            if let Some(old) = cache.1.pop_front() {
                cache.0.remove(&old);
            }
        }
        Some(mips)
    }
}
