use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const APP_DIR_NAME: &str = "IncuBar";
const INSTALL_ORIGIN_FILENAME: &str = "install-origin.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstallOriginRecord {
    origin: String,
    recorded_at: String,
}

fn install_origin_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("Could not determine data directory")?;
    Ok(data_dir.join(APP_DIR_NAME).join(INSTALL_ORIGIN_FILENAME))
}

fn load_install_origin(path: &PathBuf) -> Option<InstallOriginRecord> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

fn detect_install_origin() -> String {
    if let Ok(origin) = std::env::var("INCUBAR_INSTALL_ORIGIN") {
        let trimmed = origin.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }

    if cfg!(debug_assertions) {
        return "Development".to_string();
    }

    let Ok(exe_path) = std::env::current_exe() else {
        return "Unknown".to_string();
    };
    let exe_path = exe_path.to_string_lossy();

    if exe_path.contains(".app/Contents/MacOS/") {
        return "macOS App Bundle".to_string();
    }

    if exe_path.contains("Program Files") || exe_path.contains("Program Files (x86)") {
        return "Windows Installer".to_string();
    }

    if exe_path.contains("/usr/local/")
        || exe_path.contains("/opt/")
        || exe_path.contains("/usr/bin/")
    {
        return "System Install".to_string();
    }

    "Unknown".to_string()
}

pub fn read_or_record_install_origin() -> Result<String> {
    let path = install_origin_path()?;
    if let Some(record) = load_install_origin(&path) {
        return Ok(record.origin);
    }

    let origin = detect_install_origin();
    let record = InstallOriginRecord {
        origin: origin.clone(),
        recorded_at: Utc::now().to_rfc3339(),
    };

    if let Some(parent) = path.parent() {
        if let Err(err) = fs::create_dir_all(parent) {
            tracing::warn!("Failed to create install origin directory: {}", err);
            return Ok(origin);
        }
    }

    if let Ok(payload) = serde_json::to_string_pretty(&record) {
        if let Err(err) = fs::write(&path, payload) {
            tracing::warn!("Failed to write install origin: {}", err);
        }
    }

    Ok(origin)
}
