//! Port of `utils/custom_logging.py`: a log file that rolls over on every
//! start (20 backups kept), tolerant of a locked file, plus stderr output.

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use log::{Level, LevelFilter, Log, Metadata, Record};

pub const LOG_FILE_NAME: &str = "pkmn_router_logs.log";
pub const BACKUP_COUNT: usize = 20;

struct FileLogger {
    file: Mutex<Option<File>>,
    level: LevelFilter,
}

impl Log for FileLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        if !self.enabled(record.metadata()) {
            return;
        }
        // '%(asctime)s %(levelname)-8s [%(filename)s:%(lineno)d] %(message)s'
        let now = chrono::Local::now();
        let level = match record.level() {
            Level::Error => "ERROR",
            Level::Warn => "WARNING",
            Level::Info => "INFO",
            Level::Debug => "DEBUG",
            Level::Trace => "TRACE",
        };
        let file = record
            .file()
            .map(|f| {
                Path::new(f)
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| f.to_string())
            })
            .unwrap_or_default();
        let line = format!(
            "{} {:<8} [{}:{}] {}\n",
            now.format("%Y-%m-%d %H:%M:%S,%3f"),
            level,
            file,
            record.line().unwrap_or(0),
            record.args()
        );
        let _ = std::io::stderr().write_all(line.as_bytes());
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = f.write_all(line.as_bytes());
            }
        }
    }

    fn flush(&self) {
        if let Ok(mut guard) = self.file.lock() {
            if let Some(f) = guard.as_mut() {
                let _ = f.flush();
            }
        }
    }
}

/// `RotatingFileHandler.doRollover` with `backupCount=20` and no size limit:
/// `.log.19 -> .log.20`, ..., `.log -> .log.1`.
fn rollover(base_log_dir: &Path) {
    let final_path = base_log_dir.join(LOG_FILE_NAME);
    if !final_path.exists() {
        return;
    }
    for i in (1..BACKUP_COUNT).rev() {
        let sfn = base_log_dir.join(format!("{}.{}", LOG_FILE_NAME, i));
        let dfn = base_log_dir.join(format!("{}.{}", LOG_FILE_NAME, i + 1));
        if sfn.exists() {
            if dfn.exists() {
                let _ = std::fs::remove_file(&dfn);
            }
            let _ = std::fs::rename(&sfn, &dfn);
        }
    }
    let dfn = base_log_dir.join(format!("{}.1", LOG_FILE_NAME));
    if dfn.exists() {
        let _ = std::fs::remove_file(&dfn);
    }
    // On Windows this fails when another instance has the file open; the
    // Python app just keeps appending to the existing file in that case.
    let _ = std::fs::rename(&final_path, &dfn);
}

/// `config_logging`
pub fn config_logging(base_log_dir: &Path) {
    if !base_log_dir.exists() {
        let _ = std::fs::create_dir_all(base_log_dir);
    }
    rollover(base_log_dir);
    let final_path = base_log_dir.join(LOG_FILE_NAME);
    let file = OpenOptions::new().create(true).append(true).open(&final_path).ok();
    let logger = FileLogger {
        file: Mutex::new(file),
        level: LevelFilter::Info,
    };
    if log::set_boxed_logger(Box::new(logger)).is_ok() {
        log::set_max_level(LevelFilter::Info);
    }
    log::info!("Logging configured to output to: {}", base_log_dir.display());
}
