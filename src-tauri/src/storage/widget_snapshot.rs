use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::providers::{ProviderId, UsageSnapshot};

const APP_DIR_NAME: &str = "IncuBar";
const WIDGET_SNAPSHOT_FILENAME: &str = "widget-snapshot.json";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WidgetSnapshot {
    updated_at: String,
    providers: HashMap<ProviderId, UsageSnapshot>,
}

impl WidgetSnapshot {
    fn new() -> Self {
        Self {
            updated_at: Utc::now().to_rfc3339(),
            providers: HashMap::new(),
        }
    }
}

fn snapshot_path() -> Result<PathBuf> {
    let data_dir = dirs::data_dir().context("Could not determine data directory")?;
    Ok(data_dir.join(APP_DIR_NAME).join(WIDGET_SNAPSHOT_FILENAME))
}

fn load_snapshot(path: &PathBuf) -> Option<WidgetSnapshot> {
    let contents = fs::read_to_string(path).ok()?;
    serde_json::from_str(&contents).ok()
}

pub fn write_widget_snapshot(provider_id: ProviderId, usage: &UsageSnapshot) -> Result<()> {
    let path = snapshot_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).context("Failed to create widget snapshot directory")?;
    }

    let mut snapshot = load_snapshot(&path).unwrap_or_else(WidgetSnapshot::new);
    snapshot.updated_at = Utc::now().to_rfc3339();
    snapshot.providers.insert(provider_id, usage.clone());

    let payload =
        serde_json::to_string_pretty(&snapshot).context("Failed to serialize widget snapshot")?;
    fs::write(&path, payload).context("Failed to write widget snapshot")?;
    Ok(())
}
