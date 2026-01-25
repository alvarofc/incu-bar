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
static REDACT_PERSONAL_INFO: AtomicBool = AtomicBool::new(false);

static DEBUG_LOG_FILE: Lazy<Arc<Mutex<Box<dyn Write + Send>>>> = Lazy::new(|| {
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

pub fn set_redact_personal_info(enabled: bool) {
    REDACT_PERSONAL_INFO.store(enabled, Ordering::Relaxed);
}

pub fn redact_personal_info_enabled() -> bool {
    REDACT_PERSONAL_INFO.load(Ordering::Relaxed)
}

pub fn redact_value(value: &str) -> String {
    if redact_personal_info_enabled() {
        "[redacted]".to_string()
    } else {
        value.to_string()
    }
}

pub fn redact_option(value: Option<&str>) -> String {
    match value {
        Some(raw) => redact_value(raw),
        None => "None".to_string(),
    }
}

pub fn file_writer() -> DebugFileWriter {
    DebugFileWriter {
        file: DEBUG_LOG_FILE.clone(),
    }
}

fn open_debug_log_file() -> io::Result<Box<dyn Write + Send>> {
    let data_dir = dirs::data_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "Missing data directory"))?;
    let log_dir = data_dir.join("IncuBar");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join("incubar-debug.log");
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)
        .map(|file| Box::new(file) as Box<dyn Write + Send>)
}

fn open_fallback_log_file() -> Box<dyn Write + Send> {
    open_fallback_log_file_with_paths(std::env::temp_dir(), PathBuf::from("."))
}

fn open_fallback_log_file_with_paths(
    temp_dir: PathBuf,
    current_dir: PathBuf,
) -> Box<dyn Write + Send> {
    let temp_path = temp_dir.join("incubar-debug.log");
    match OpenOptions::new().create(true).append(true).open(temp_path) {
        Ok(file) => Box::new(file),
        Err(_) => {
            let current_path = current_dir.join("incubar-debug.log");
            match std::fs::File::create(current_path) {
                Ok(file) => Box::new(file),
                Err(_) => Box::new(io::sink()),
            }
        }
    }
}

#[derive(Clone)]
pub struct DebugFileWriter {
    file: Arc<Mutex<Box<dyn Write + Send>>>,
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
    file: Arc<Mutex<Box<dyn Write + Send>>>,
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn fallback_creates_log_in_temp_dir() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let current_dir = tempfile::tempdir().expect("current dir");
        let log_path = temp_dir.path().join("incubar-debug.log");

        assert!(!log_path.exists());
        let _writer = open_fallback_log_file_with_paths(
            temp_dir.path().to_path_buf(),
            current_dir.path().to_path_buf(),
        );
        assert!(log_path.exists());
    }

    #[test]
    fn fallback_uses_sink_when_all_fail() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let current_dir = tempfile::tempdir().expect("current dir");
        let missing_temp = temp_dir.path().join("missing-temp");
        let missing_current = current_dir.path().join("missing-current");
        let mut writer =
            open_fallback_log_file_with_paths(missing_temp.clone(), missing_current.clone());

        let bytes = writer.write(b"hello").expect("write");
        assert_eq!(bytes, 5);
        assert!(!missing_temp.join("incubar-debug.log").exists());
        assert!(!missing_current.join("incubar-debug.log").exists());
    }
}
