use anyhow::{Context, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration as StdDuration;
use tauri::AppHandle;
use url::Url;
use uuid::Uuid;

use crate::providers::{ProviderId, ProviderRegistry, UsageSnapshot};
use crate::storage::keyring::KeyringError;
use crate::storage::SecureStorage;

const APP_DIR_NAME: &str = "IncuBar";
const CONFIG_FILE_NAME: &str = "employer-reporting-config.json";
const STATE_FILE_NAME: &str = "employer-reporting-state.json";
const QUEUE_FILE_NAME: &str = "employer-reporting-queue.json";
const KEYCHAIN_GATEWAY_KEY: &str = "employer_reporting_gateway_key";
const DAILY_SEND_INTERVAL_HOURS: i64 = 24;
const SCHEMA_VERSION: u16 = 1;

static CLOSE_REPORT_QUEUED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerReportingConfig {
    pub enabled: bool,
    pub gateway_url: String,
    pub employee_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub employee_email: Option<String>,
    pub include_employee_email: bool,
}

impl Default for EmployerReportingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            gateway_url: String::new(),
            employee_id: String::new(),
            employee_email: None,
            include_employee_email: false,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerReportingStatus {
    pub enabled: bool,
    pub token_configured: bool,
    pub queued_reports: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_sent_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_send_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_daily_send_at: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmployerReportSendResult {
    pub sent: bool,
    pub queued: bool,
    pub reason: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sent_at: Option<String>,
    pub queued_reports: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct EmployerReportingState {
    install_id: Option<String>,
    last_sent_at: Option<String>,
    last_send_reason: Option<String>,
    last_error: Option<String>,
    last_daily_sent_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct QueuedReport {
    payload: GatewayReportPayload,
    queued_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewayReportPayload {
    schema_version: u16,
    report_id: String,
    report_ts: String,
    send_reason: String,
    app_install_id_hash: String,
    app_version: String,
    platform: String,
    employee_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    employee_email: Option<String>,
    enabled_providers: Vec<String>,
    enabled_provider_count: u8,
    rows: Vec<GatewayProviderRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GatewayProviderRow {
    provider_id: String,
    provider_enabled: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    provider_plan: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    primary_used_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_remaining_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_window_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    primary_resets_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_used_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_remaining_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_window_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    secondary_resets_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    tertiary_used_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tertiary_remaining_percent: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tertiary_window_minutes: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tertiary_resets_at: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    credits_remaining: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credits_total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credits_unit: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    credits_remaining_percent: Option<f64>,

    #[serde(skip_serializing_if = "Option::is_none")]
    cost_today_amount_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost_today_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost_month_amount_usd: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost_month_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cost_currency: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    snapshot_updated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fetch_error: Option<String>,
}

enum DeliveryError {
    Retryable(String),
    Permanent(String),
}

pub fn load_reporting_config() -> Result<EmployerReportingConfig> {
    Ok(read_json_or_default(&config_file_path()?)?)
}

pub fn save_reporting_config(config: EmployerReportingConfig) -> Result<EmployerReportingConfig> {
    let normalized = normalize_config(config)?;
    write_json_file(&config_file_path()?, &normalized)?;
    Ok(normalized)
}

pub fn store_gateway_key(api_key: &str) -> Result<()> {
    let key = api_key.trim();
    if key.is_empty() {
        return Err(anyhow::anyhow!("Gateway API key cannot be empty"));
    }
    let storage = SecureStorage::new();
    storage
        .store(KEYCHAIN_GATEWAY_KEY, key)
        .map_err(|err| anyhow::anyhow!("Failed to save gateway API key: {}", err))
}

pub fn clear_gateway_key() -> Result<()> {
    let storage = SecureStorage::new();
    match storage.delete(KEYCHAIN_GATEWAY_KEY) {
        Ok(()) => Ok(()),
        Err(KeyringError::NotFound) => Ok(()),
        Err(err) => Err(anyhow::anyhow!(
            "Failed to clear gateway API key: {}",
            err
        )),
    }
}

pub fn get_reporting_status() -> Result<EmployerReportingStatus> {
    let config = load_reporting_config()?;
    let state = load_reporting_state()?;
    let queue = load_report_queue()?;
    let token_configured = load_gateway_key().is_ok();

    let next_daily_send_at = state
        .last_daily_sent_at
        .as_ref()
        .and_then(|value| DateTime::parse_from_rfc3339(value).ok())
        .map(|value| {
            value
                .with_timezone(&Utc)
                .checked_add_signed(Duration::hours(DAILY_SEND_INTERVAL_HOURS))
                .unwrap_or_else(Utc::now)
                .to_rfc3339()
        });

    Ok(EmployerReportingStatus {
        enabled: config.enabled,
        token_configured,
        queued_reports: queue.len(),
        last_sent_at: state.last_sent_at,
        last_send_reason: state.last_send_reason,
        last_error: state.last_error,
        next_daily_send_at,
    })
}

pub async fn send_usage_report(
    app: &AppHandle,
    registry: &ProviderRegistry,
    reason: &str,
    force: bool,
) -> Result<EmployerReportSendResult> {
    let normalized_reason = normalize_reason(reason);
    let config = load_reporting_config()?;

    if !config.enabled {
        return Ok(EmployerReportSendResult {
            sent: false,
            queued: false,
            reason: normalized_reason,
            message: "Employer reporting is disabled".to_string(),
            sent_at: None,
            queued_reports: load_report_queue()?.len(),
        });
    }

    let config = match validate_active_config(config) {
        Ok(value) => value,
        Err(err) => {
            return Ok(EmployerReportSendResult {
                sent: false,
                queued: false,
                reason: normalized_reason,
                message: err.to_string(),
                sent_at: None,
                queued_reports: load_report_queue()?.len(),
            });
        }
    };

    let gateway_key = match load_gateway_key() {
        Ok(value) => value,
        Err(_) => {
            return Ok(EmployerReportSendResult {
                sent: false,
                queued: false,
                reason: normalized_reason,
                message: "Gateway API key is not configured".to_string(),
                sent_at: None,
                queued_reports: load_report_queue()?.len(),
            });
        }
    };

    let mut state = load_reporting_state()?;
    flush_queue(&config, &gateway_key, &mut state).await?;

    if !force && normalized_reason == "daily" && !daily_report_due(&state) {
        write_reporting_state(&state)?;
        let queue_size = load_report_queue()?.len();
        return Ok(EmployerReportSendResult {
            sent: false,
            queued: false,
            reason: normalized_reason,
            message: "Daily report already sent in the last 24 hours".to_string(),
            sent_at: None,
            queued_reports: queue_size,
        });
    }

    let payload = build_payload(app, registry, &config, &normalized_reason, &mut state).await?;

    let mut queued = false;
    let mut sent = false;
    let mut sent_at = None;
    let message = match deliver_payload(&config, &gateway_key, &payload).await {
        Ok(()) => {
            sent = true;
            let now = Utc::now();
            update_state_after_success(&mut state, &normalized_reason, now);
            sent_at = Some(now.to_rfc3339());
            "Usage report sent".to_string()
        }
        Err(DeliveryError::Retryable(err)) => {
            enqueue_report(payload)?;
            queued = true;
            state.last_error = Some(err.clone());
            format!("Report queued for retry: {}", err)
        }
        Err(DeliveryError::Permanent(err)) => {
            state.last_error = Some(err.clone());
            format!("Report rejected: {}", err)
        }
    };

    write_reporting_state(&state)?;
    let queue_size = load_report_queue()?.len();

    Ok(EmployerReportSendResult {
        sent,
        queued,
        reason: normalized_reason,
        message,
        sent_at,
        queued_reports: queue_size,
    })
}

pub async fn queue_close_report_once(app: &AppHandle, registry: &ProviderRegistry) -> Result<()> {
    if CLOSE_REPORT_QUEUED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    let config = load_reporting_config()?;
    if !config.enabled {
        return Ok(());
    }
    let config = match validate_active_config(config) {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };
    let gateway_key = match load_gateway_key() {
        Ok(value) => value,
        Err(_) => return Ok(()),
    };

    let mut state = load_reporting_state()?;
    let payload = build_payload(app, registry, &config, "close", &mut state).await?;
    enqueue_report(payload)?;
    flush_queue(&config, &gateway_key, &mut state).await?;
    write_reporting_state(&state)?;
    Ok(())
}

fn normalize_reason(reason: &str) -> String {
    match reason.trim().to_lowercase().as_str() {
        "open" => "open".to_string(),
        "close" => "close".to_string(),
        "daily" => "daily".to_string(),
        "retry" => "retry".to_string(),
        _ => "manual".to_string(),
    }
}

fn daily_report_due(state: &EmployerReportingState) -> bool {
    let Some(last_daily) = &state.last_daily_sent_at else {
        return true;
    };

    let Ok(parsed) = DateTime::parse_from_rfc3339(last_daily) else {
        return true;
    };

    let elapsed = Utc::now().signed_duration_since(parsed.with_timezone(&Utc));
    elapsed >= Duration::hours(DAILY_SEND_INTERVAL_HOURS)
}

fn normalize_config(config: EmployerReportingConfig) -> Result<EmployerReportingConfig> {
    let gateway_url = config.gateway_url.trim();
    let normalized_gateway_url = if gateway_url.is_empty() {
        String::new()
    } else {
        normalize_gateway_url(gateway_url)?
    };

    let employee_id = config.employee_id.trim().to_string();
    let employee_email = config
        .employee_email
        .and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });

    Ok(EmployerReportingConfig {
        enabled: config.enabled,
        gateway_url: normalized_gateway_url,
        employee_id,
        employee_email,
        include_employee_email: config.include_employee_email,
    })
}

fn validate_active_config(config: EmployerReportingConfig) -> Result<EmployerReportingConfig> {
    let normalized = normalize_config(config)?;
    if normalized.gateway_url.is_empty() {
        return Err(anyhow::anyhow!(
            "Gateway URL is required when employer reporting is enabled"
        ));
    }
    if normalized.employee_id.is_empty() {
        return Err(anyhow::anyhow!(
            "Employee ID is required when employer reporting is enabled"
        ));
    }
    Ok(normalized)
}

fn normalize_gateway_url(raw: &str) -> Result<String> {
    let parsed = Url::parse(raw).map_err(|err| anyhow::anyhow!("Invalid gateway URL: {}", err))?;
    if parsed.scheme() != "https" {
        return Err(anyhow::anyhow!(
            "Gateway URL must use HTTPS for secure reporting"
        ));
    }
    Ok(parsed.to_string())
}

fn provider_id_to_string(provider_id: ProviderId) -> String {
    serde_json::to_string(&provider_id)
        .unwrap_or_else(|_| "\"unknown\"".to_string())
        .trim_matches('"')
        .to_string()
}

fn remaining_percent(used: f64) -> f64 {
    (100.0 - used).clamp(0.0, 100.0)
}

fn window_minutes_to_u32(value: Option<i32>) -> Option<u32> {
    value.and_then(|minutes| if minutes > 0 { Some(minutes as u32) } else { None })
}

fn normalize_timestamp(value: Option<&str>) -> Option<String> {
    let raw = value?.trim();
    if raw.is_empty() {
        return None;
    }
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|date| date.with_timezone(&Utc).to_rfc3339())
}

fn build_provider_row(provider_id: ProviderId, enabled: bool, usage: Option<&UsageSnapshot>) -> GatewayProviderRow {
    let primary = usage.and_then(|snapshot| snapshot.primary.as_ref());
    let secondary = usage.and_then(|snapshot| snapshot.secondary.as_ref());
    let tertiary = usage.and_then(|snapshot| snapshot.tertiary.as_ref());
    let credits = usage.and_then(|snapshot| snapshot.credits.as_ref());
    let cost = usage.and_then(|snapshot| snapshot.cost.as_ref());
    let identity = usage.and_then(|snapshot| snapshot.identity.as_ref());

    let credits_remaining_percent = credits.and_then(|value| {
        value.total.and_then(|total| {
            if total > 0.0 {
                Some(((value.remaining / total) * 100.0).clamp(0.0, 100.0))
            } else {
                None
            }
        })
    });

    GatewayProviderRow {
        provider_id: provider_id_to_string(provider_id),
        provider_enabled: enabled,
        provider_plan: identity.and_then(|value| value.plan.clone()),

        primary_used_percent: primary.map(|value| value.used_percent),
        primary_remaining_percent: primary.map(|value| remaining_percent(value.used_percent)),
        primary_window_minutes: window_minutes_to_u32(primary.and_then(|value| value.window_minutes)),
        primary_resets_at: normalize_timestamp(primary.and_then(|value| value.resets_at.as_deref())),

        secondary_used_percent: secondary.map(|value| value.used_percent),
        secondary_remaining_percent: secondary.map(|value| remaining_percent(value.used_percent)),
        secondary_window_minutes: window_minutes_to_u32(secondary.and_then(|value| value.window_minutes)),
        secondary_resets_at: normalize_timestamp(secondary.and_then(|value| value.resets_at.as_deref())),

        tertiary_used_percent: tertiary.map(|value| value.used_percent),
        tertiary_remaining_percent: tertiary.map(|value| remaining_percent(value.used_percent)),
        tertiary_window_minutes: window_minutes_to_u32(tertiary.and_then(|value| value.window_minutes)),
        tertiary_resets_at: normalize_timestamp(tertiary.and_then(|value| value.resets_at.as_deref())),

        credits_remaining: credits.map(|value| value.remaining),
        credits_total: credits.and_then(|value| value.total),
        credits_unit: credits.map(|value| value.unit.clone()),
        credits_remaining_percent,

        cost_today_amount_usd: cost.map(|value| value.today_amount),
        cost_today_tokens: cost.map(|value| value.today_tokens),
        cost_month_amount_usd: cost.map(|value| value.month_amount),
        cost_month_tokens: cost.map(|value| value.month_tokens),
        cost_currency: cost.map(|value| value.currency.clone()),

        snapshot_updated_at: usage.and_then(|snapshot| normalize_timestamp(Some(&snapshot.updated_at))),
        fetch_error: usage.and_then(|snapshot| snapshot.error.clone()),
    }
}

async fn build_payload(
    _app: &AppHandle,
    registry: &ProviderRegistry,
    config: &EmployerReportingConfig,
    reason: &str,
    state: &mut EmployerReportingState,
) -> Result<GatewayReportPayload> {
    let install_id = state
        .install_id
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    state.install_id = Some(install_id.clone());

    let enabled_providers = registry
        .get_enabled_providers()
        .await
        .into_iter()
        .map(provider_id_to_string)
        .collect::<Vec<_>>();

    let enabled_provider_count = enabled_providers.len().min(u8::MAX as usize) as u8;

    let usage_map = registry.get_all_cached_usage().await;
    let enabled_set = registry
        .get_enabled_providers()
        .await
        .into_iter()
        .collect::<std::collections::HashSet<_>>();

    let rows = ProviderId::all()
        .into_iter()
        .map(|provider_id| {
            let usage = usage_map.get(&provider_id);
            build_provider_row(provider_id, enabled_set.contains(&provider_id), usage)
        })
        .collect::<Vec<_>>();

    Ok(GatewayReportPayload {
        schema_version: SCHEMA_VERSION,
        report_id: Uuid::new_v4().to_string(),
        report_ts: Utc::now().to_rfc3339(),
        send_reason: reason.to_string(),
        app_install_id_hash: install_id,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        platform: std::env::consts::OS.to_string(),
        employee_id: config.employee_id.clone(),
        employee_email: if config.include_employee_email {
            config.employee_email.clone()
        } else {
            None
        },
        enabled_providers,
        enabled_provider_count,
        rows,
    })
}

async fn flush_queue(
    config: &EmployerReportingConfig,
    gateway_key: &str,
    state: &mut EmployerReportingState,
) -> Result<()> {
    let mut queue = load_report_queue()?;
    if queue.is_empty() {
        return Ok(());
    }

    while !queue.is_empty() {
        let queued = queue[0].clone();
        match deliver_payload(config, gateway_key, &queued.payload).await {
            Ok(()) => {
                update_state_after_success(state, &queued.payload.send_reason, Utc::now());
                queue.remove(0);
            }
            Err(DeliveryError::Permanent(err)) => {
                state.last_error = Some(err);
                queue.remove(0);
            }
            Err(DeliveryError::Retryable(err)) => {
                state.last_error = Some(err);
                break;
            }
        }
    }

    write_report_queue(&queue)?;
    Ok(())
}

fn update_state_after_success(state: &mut EmployerReportingState, reason: &str, now: DateTime<Utc>) {
    let now_iso = now.to_rfc3339();
    state.last_sent_at = Some(now_iso.clone());
    state.last_send_reason = Some(reason.to_string());
    if reason == "daily" {
        state.last_daily_sent_at = Some(now_iso);
    }
    state.last_error = None;
}

async fn deliver_payload(
    config: &EmployerReportingConfig,
    gateway_key: &str,
    payload: &GatewayReportPayload,
) -> std::result::Result<(), DeliveryError> {
    let client = reqwest::Client::builder()
        .timeout(StdDuration::from_secs(12))
        .build()
        .map_err(|err| DeliveryError::Retryable(format!("Failed to create HTTP client: {}", err)))?;

    let response = client
        .post(&config.gateway_url)
        .bearer_auth(gateway_key)
        .header("Content-Type", "application/json")
        .json(payload)
        .send()
        .await
        .map_err(|err| DeliveryError::Retryable(format!("Gateway request failed: {}", err)))?;

    if response.status().is_success() {
        return Ok(());
    }

    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = format!("Gateway responded with {}: {}", status, body);

    if status.as_u16() == 408
        || status.as_u16() == 409
        || status.as_u16() == 425
        || status.as_u16() == 429
        || status.is_server_error()
    {
        Err(DeliveryError::Retryable(message))
    } else {
        Err(DeliveryError::Permanent(message))
    }
}

fn enqueue_report(payload: GatewayReportPayload) -> Result<()> {
    let mut queue = load_report_queue()?;
    queue.push(QueuedReport {
        payload,
        queued_at: Utc::now().to_rfc3339(),
    });
    write_report_queue(&queue)
}

fn load_gateway_key() -> std::result::Result<String, KeyringError> {
    let storage = SecureStorage::new();
    storage.get(KEYCHAIN_GATEWAY_KEY)
}

fn app_data_dir() -> Result<PathBuf> {
    let dir = dirs::data_dir().context("Could not determine data directory")?;
    let app_dir = dir.join(APP_DIR_NAME);
    fs::create_dir_all(&app_dir).context("Failed to create app data directory")?;
    Ok(app_dir)
}

fn config_file_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join(CONFIG_FILE_NAME))
}

fn reporting_state_file_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join(STATE_FILE_NAME))
}

fn report_queue_file_path() -> Result<PathBuf> {
    Ok(app_data_dir()?.join(QUEUE_FILE_NAME))
}

fn load_reporting_state() -> Result<EmployerReportingState> {
    read_json_or_default(&reporting_state_file_path()?)
}

fn write_reporting_state(state: &EmployerReportingState) -> Result<()> {
    write_json_file(&reporting_state_file_path()?, state)
}

fn load_report_queue() -> Result<Vec<QueuedReport>> {
    read_json_or_default(&report_queue_file_path()?)
}

fn write_report_queue(queue: &[QueuedReport]) -> Result<()> {
    write_json_file(&report_queue_file_path()?, queue)
}

fn read_json_or_default<T>(path: &Path) -> Result<T>
where
    T: for<'de> Deserialize<'de> + Default,
{
    if !path.exists() {
        return Ok(T::default());
    }

    let contents = fs::read_to_string(path)
        .with_context(|| format!("Failed to read {}", path.to_string_lossy()))?;
    if contents.trim().is_empty() {
        return Ok(T::default());
    }

    serde_json::from_str(&contents)
        .with_context(|| format!("Failed to parse {}", path.to_string_lossy()))
}

fn write_json_file<T>(path: &Path, value: &T) -> Result<()>
where
    T: Serialize + ?Sized,
{
    let json = serde_json::to_string_pretty(value)
        .with_context(|| format!("Failed to serialize {}", path.to_string_lossy()))?;
    fs::write(path, json).with_context(|| format!("Failed to write {}", path.to_string_lossy()))
}

#[cfg(test)]
mod tests {
    use super::{daily_report_due, normalize_gateway_url, EmployerReportingState};
    use chrono::{Duration, Utc};

    #[test]
    fn normalize_gateway_url_rejects_non_https() {
        let err = normalize_gateway_url("http://example.com").expect_err("should fail");
        assert!(err.to_string().contains("HTTPS"));
    }

    #[test]
    fn normalize_gateway_url_accepts_https() {
        let normalized = normalize_gateway_url("https://gateway.example.com/v1/usage-report")
            .expect("should normalize");
        assert!(normalized.starts_with("https://gateway.example.com"));
    }

    #[test]
    fn daily_report_due_after_24_hours() {
        let mut state = EmployerReportingState::default();
        state.last_daily_sent_at = Some((Utc::now() - Duration::hours(25)).to_rfc3339());
        assert!(daily_report_due(&state));
    }

    #[test]
    fn daily_report_not_due_within_24_hours() {
        let mut state = EmployerReportingState::default();
        state.last_daily_sent_at = Some((Utc::now() - Duration::hours(2)).to_rfc3339());
        assert!(!daily_report_due(&state));
    }
}
