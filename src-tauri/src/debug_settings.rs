use std::fs::OpenOptions;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use once_cell::sync::Lazy;
use tracing_subscriber::fmt::MakeWriter;

static FILE_LOGGING_ENABLED: AtomicBool = AtomicBool::new(false);
static KEEP_CLI_SESSIONS_ALIVE: AtomicBool = AtomicBool::new(false);
static RANDOM_BLINK_ENABLED: AtomicBool = AtomicBool::new(false);

static DEBUG_LOG_FILE: Lazy<Arc<Mutex<std::fs::File>>> = Lazy::new(|| {
    let file = open_debug_log_file().unwrap_or_else(|_| open_fallback_log_file());
    Arc::new(Mutex::new(file))
});

pub fn set_file_logging(enabled: bool) {
    FILE_LOGGING_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn file_logging_enabled() -> bool {
    FILE_LOGGING_ENABLED.load(Ordering::Relaxed)
}

pub fn set_keep_cli_sessions_alive(enabled: bool) {
    KEEP_CLI_SESSIONS_ALIVE.store(enabled, Ordering::Relaxed);
}

pub fn keep_cli_sessions_alive() -> bool {
    KEEP_CLI_SESSIONS_ALIVE.load(Ordering::Relaxed)
}

pub fn set_random_blink(enabled: bool) {
    RANDOM_BLINK_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn random_blink_enabled() -> bool {
    RANDOM_BLINK_ENABLED.load(Ordering::Relaxed)
}

pub fn file_writer() -> DebugFileWriter {
    DebugFileWriter {
        file: DEBUG_LOG_FILE.clone(),
    }
}

fn open_debug_log_file() -> io::Result<std::fs::File> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Missing data directory"))?;
    let log_dir = data_dir.join("IncuBar");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("incubar-debug.log");
    OpenOptions::new().create(true).append(true).open(log_path)
}

fn open_fallback_log_file() -> std::fs::File {
    let mut path = PathBuf::from(std::env::temp_dir());
    path.push("incubar-debug.log");
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .unwrap_or_else(|_| std::fs::File::create("incubar-debug.log").unwrap())
}

#[derive(Clone)]
pub struct DebugFileWriter {
    file: Arc<Mutex<std::fs::File>>,
}

impl<'a> MakeWriter<'a> for DebugFileWriter {
    type Writer = DebugFileWriterGuard;

    fn make_writer(&'a self) -> Self::Writer {
        DebugFileWriterGuard {
            file: self.file.clone(),
        }
    }
}

pub struct DebugFileWriterGuard {
    file: Arc<Mutex<std::fs::File>>,
}

impl Write for DebugFileWriterGuard {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if !FILE_LOGGING_ENABLED.load(Ordering::Relaxed) {
            return Ok(buf.len());
        }

        match self.file.lock() {
            Ok(mut file) => file.write(buf),
            Err(_) => Ok(buf.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        if !FILE_LOGGING_ENABLED.load(Ordering::Relaxed) {
            return Ok(());
        }
        match self.file.lock() {
            Ok(mut file) => file.flush(),
            Err(_) => Ok(()),
        }
    }
}
