use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Datelike, Duration, Local, NaiveDate, Utc};
use once_cell::sync::Lazy;
use regex::Regex;
use serde_json::Value;

use super::{CostSnapshot, ProviderId};

#[derive(Clone, Copy, Debug, Default)]
struct DayTotals {
    input: i64,
    output: i64,
    cache_read: i64,
    cache_create: i64,
    cost: f64,
    cost_seen: bool,
}

#[derive(Clone, Copy)]
struct CodexPricing {
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_read_input_cost_per_token: f64,
}

#[derive(Clone, Copy)]
struct ClaudePricing {
    input_cost_per_token: f64,
    output_cost_per_token: f64,
    cache_creation_input_cost_per_token: f64,
    cache_read_input_cost_per_token: f64,
    threshold_tokens: Option<i64>,
    input_cost_per_token_above_threshold: Option<f64>,
    output_cost_per_token_above_threshold: Option<f64>,
    cache_creation_input_cost_per_token_above_threshold: Option<f64>,
    cache_read_input_cost_per_token_above_threshold: Option<f64>,
}

static CODEX_PRICING: Lazy<HashMap<&'static str, CodexPricing>> = Lazy::new(|| {
    let mut pricing = HashMap::new();
    pricing.insert(
        "gpt-5",
        CodexPricing {
            input_cost_per_token: 1.25e-6,
            output_cost_per_token: 1e-5,
            cache_read_input_cost_per_token: 1.25e-7,
        },
    );
    pricing.insert(
        "gpt-5-codex",
        CodexPricing {
            input_cost_per_token: 1.25e-6,
            output_cost_per_token: 1e-5,
            cache_read_input_cost_per_token: 1.25e-7,
        },
    );
    pricing.insert(
        "gpt-5.1",
        CodexPricing {
            input_cost_per_token: 1.25e-6,
            output_cost_per_token: 1e-5,
            cache_read_input_cost_per_token: 1.25e-7,
        },
    );
    pricing.insert(
        "gpt-5.2",
        CodexPricing {
            input_cost_per_token: 1.75e-6,
            output_cost_per_token: 1.4e-5,
            cache_read_input_cost_per_token: 1.75e-7,
        },
    );
    pricing.insert(
        "gpt-5.2-codex",
        CodexPricing {
            input_cost_per_token: 1.75e-6,
            output_cost_per_token: 1.4e-5,
            cache_read_input_cost_per_token: 1.75e-7,
        },
    );
    pricing
});

static CLAUDE_PRICING: Lazy<HashMap<&'static str, ClaudePricing>> = Lazy::new(|| {
    let mut pricing = HashMap::new();
    pricing.insert(
        "claude-haiku-4-5-20251001",
        ClaudePricing {
            input_cost_per_token: 1e-6,
            output_cost_per_token: 5e-6,
            cache_creation_input_cost_per_token: 1.25e-6,
            cache_read_input_cost_per_token: 1e-7,
            threshold_tokens: None,
            input_cost_per_token_above_threshold: None,
            output_cost_per_token_above_threshold: None,
            cache_creation_input_cost_per_token_above_threshold: None,
            cache_read_input_cost_per_token_above_threshold: None,
        },
    );
    pricing.insert(
        "claude-opus-4-5-20251101",
        ClaudePricing {
            input_cost_per_token: 5e-6,
            output_cost_per_token: 2.5e-5,
            cache_creation_input_cost_per_token: 6.25e-6,
            cache_read_input_cost_per_token: 5e-7,
            threshold_tokens: None,
            input_cost_per_token_above_threshold: None,
            output_cost_per_token_above_threshold: None,
            cache_creation_input_cost_per_token_above_threshold: None,
            cache_read_input_cost_per_token_above_threshold: None,
        },
    );
    pricing.insert(
        "claude-sonnet-4-5",
        ClaudePricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 1.5e-5,
            cache_creation_input_cost_per_token: 3.75e-6,
            cache_read_input_cost_per_token: 3e-7,
            threshold_tokens: Some(200_000),
            input_cost_per_token_above_threshold: Some(6e-6),
            output_cost_per_token_above_threshold: Some(2.25e-5),
            cache_creation_input_cost_per_token_above_threshold: Some(7.5e-6),
            cache_read_input_cost_per_token_above_threshold: Some(6e-7),
        },
    );
    pricing.insert(
        "claude-sonnet-4-5-20250929",
        ClaudePricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 1.5e-5,
            cache_creation_input_cost_per_token: 3.75e-6,
            cache_read_input_cost_per_token: 3e-7,
            threshold_tokens: Some(200_000),
            input_cost_per_token_above_threshold: Some(6e-6),
            output_cost_per_token_above_threshold: Some(2.25e-5),
            cache_creation_input_cost_per_token_above_threshold: Some(7.5e-6),
            cache_read_input_cost_per_token_above_threshold: Some(6e-7),
        },
    );
    pricing.insert(
        "claude-opus-4-20250514",
        ClaudePricing {
            input_cost_per_token: 1.5e-5,
            output_cost_per_token: 7.5e-5,
            cache_creation_input_cost_per_token: 1.875e-5,
            cache_read_input_cost_per_token: 1.5e-6,
            threshold_tokens: None,
            input_cost_per_token_above_threshold: None,
            output_cost_per_token_above_threshold: None,
            cache_creation_input_cost_per_token_above_threshold: None,
            cache_read_input_cost_per_token_above_threshold: None,
        },
    );
    pricing.insert(
        "claude-opus-4-1",
        ClaudePricing {
            input_cost_per_token: 1.5e-5,
            output_cost_per_token: 7.5e-5,
            cache_creation_input_cost_per_token: 1.875e-5,
            cache_read_input_cost_per_token: 1.5e-6,
            threshold_tokens: None,
            input_cost_per_token_above_threshold: None,
            output_cost_per_token_above_threshold: None,
            cache_creation_input_cost_per_token_above_threshold: None,
            cache_read_input_cost_per_token_above_threshold: None,
        },
    );
    pricing.insert(
        "claude-sonnet-4-20250514",
        ClaudePricing {
            input_cost_per_token: 3e-6,
            output_cost_per_token: 1.5e-5,
            cache_creation_input_cost_per_token: 3.75e-6,
            cache_read_input_cost_per_token: 3e-7,
            threshold_tokens: Some(200_000),
            input_cost_per_token_above_threshold: Some(6e-6),
            output_cost_per_token_above_threshold: Some(2.25e-5),
            cache_creation_input_cost_per_token_above_threshold: Some(7.5e-6),
            cache_read_input_cost_per_token_above_threshold: Some(6e-7),
        },
    );
    pricing
});

static CLAUDE_VERSION_SUFFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"-v\d+:\d+$").unwrap_or_else(|_| Regex::new("$").unwrap()));
static CLAUDE_BASE_SUFFIX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"-\d{8}$").unwrap_or_else(|_| Regex::new("$").unwrap()));

#[derive(Clone)]
pub(crate) struct ScanOptions {
    codex_sessions_root: Option<PathBuf>,
    claude_projects_roots: Option<Vec<PathBuf>>,
    now: DateTime<Local>,
}

impl ScanOptions {
    pub(crate) fn default_for_runtime() -> Self {
        Self {
            codex_sessions_root: None,
            claude_projects_roots: None,
            now: Local::now(),
        }
    }
}

pub(crate) async fn load_cost_snapshot(provider: ProviderId) -> Option<CostSnapshot> {
    let options = ScanOptions::default_for_runtime();
    tokio::task::spawn_blocking(move || scan_cost_snapshot(provider, &options))
        .await
        .ok()
        .flatten()
}

pub(crate) fn scan_cost_snapshot(
    provider: ProviderId,
    options: &ScanOptions,
) -> Option<CostSnapshot> {
    let (since_key, until_key) = day_key_range(options.now);
    let mut totals = HashMap::new();

    match provider {
        ProviderId::Codex => scan_codex(&mut totals, &since_key, &until_key, options),
        ProviderId::Claude => scan_claude(&mut totals, &since_key, &until_key, options),
        _ => {}
    }

    build_cost_snapshot(provider, &totals, &since_key, &until_key)
}

fn scan_codex(
    totals: &mut HashMap<String, DayTotals>,
    since_key: &str,
    until_key: &str,
    options: &ScanOptions,
) {
    let roots = codex_session_roots(options);
    for root in roots {
        for file in collect_jsonl_files(&root) {
            scan_codex_file(&file, totals, since_key, until_key);
        }
    }
}

fn scan_claude(
    totals: &mut HashMap<String, DayTotals>,
    since_key: &str,
    until_key: &str,
    options: &ScanOptions,
) {
    let roots = claude_project_roots(options);
    for root in roots {
        for file in collect_jsonl_files(&root) {
            scan_claude_file(&file, totals, since_key, until_key);
        }
    }
}

fn codex_session_roots(options: &ScanOptions) -> Vec<PathBuf> {
    let base = if let Some(root) = &options.codex_sessions_root {
        root.clone()
    } else if let Ok(env) = std::env::var("CODEX_HOME") {
        let trimmed = env.trim();
        if !trimmed.is_empty() {
            PathBuf::from(trimmed).join("sessions")
        } else {
            default_codex_sessions_root()
        }
    } else {
        default_codex_sessions_root()
    };

    let mut roots = vec![base.clone()];
    if let Some(archived) = codex_archived_root(&base) {
        roots.push(archived);
    }
    roots
}

fn default_codex_sessions_root() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/"))
        .join(".codex")
        .join("sessions")
}

fn codex_archived_root(root: &Path) -> Option<PathBuf> {
    if root.file_name().and_then(|name| name.to_str()) == Some("sessions") {
        return Some(root.parent()?.join("archived_sessions"));
    }
    None
}

fn claude_project_roots(options: &ScanOptions) -> Vec<PathBuf> {
    if let Some(roots) = &options.claude_projects_roots {
        return roots.clone();
    }

    if let Ok(env) = std::env::var("CLAUDE_CONFIG_DIR") {
        let mut roots = Vec::new();
        for part in env.split(',') {
            let trimmed = part.trim();
            if trimmed.is_empty() {
                continue;
            }
            let path = PathBuf::from(trimmed);
            if path.file_name().and_then(|name| name.to_str()) == Some("projects") {
                roots.push(path);
            } else {
                roots.push(path.join("projects"));
            }
        }
        if !roots.is_empty() {
            return roots;
        }
    }

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    vec![
        home.join(".config").join("claude").join("projects"),
        home.join(".claude").join("projects"),
    ]
}

fn collect_jsonl_files(root: &Path) -> Vec<PathBuf> {
    if !root.exists() {
        return Vec::new();
    }
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    if name.starts_with('.') {
                        continue;
                    }
                }
                stack.push(path);
            } else if path.extension().and_then(|ext| ext.to_str()) == Some("jsonl") {
                files.push(path);
            }
        }
    }

    files
}

fn scan_codex_file(
    path: &Path,
    totals: &mut HashMap<String, DayTotals>,
    since_key: &str,
    until_key: &str,
) {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return,
    };
    let reader = BufReader::new(file);
    let mut current_model: Option<String> = None;
    let mut previous_totals: Option<(i64, i64, i64)> = None;

    for line in reader.lines().flatten() {
        if !line.contains("\"type\"") {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        let ty = value.get("type").and_then(Value::as_str);
        match ty {
            Some("turn_context") => {
                if let Some(payload) = value.get("payload") {
                    if let Some(model) = payload.get("model").and_then(Value::as_str) {
                        current_model = Some(model.to_string());
                    } else if let Some(info) = payload.get("info") {
                        if let Some(model) = info.get("model").and_then(Value::as_str) {
                            current_model = Some(model.to_string());
                        }
                    }
                }
            }
            Some("event_msg") => {
                let payload = match value.get("payload") {
                    Some(payload) => payload,
                    None => continue,
                };
                if payload.get("type").and_then(Value::as_str) != Some("token_count") {
                    continue;
                }
                let info = payload.get("info");
                let model = info
                    .and_then(|info| info.get("model").and_then(Value::as_str))
                    .or_else(|| {
                        info.and_then(|info| info.get("model_name").and_then(Value::as_str))
                    })
                    .or_else(|| payload.get("model").and_then(Value::as_str))
                    .or_else(|| value.get("model").and_then(Value::as_str))
                    .map(|model| model.to_string())
                    .or_else(|| current_model.clone())
                    .unwrap_or_else(|| "gpt-5".to_string());

                let timestamp = value.get("timestamp");
                let day_key = match day_key_from_timestamp(timestamp) {
                    Some(day) => day,
                    None => continue,
                };
                if day_key.as_str() < since_key || day_key.as_str() > until_key {
                    continue;
                }

                let delta_input;
                let delta_cached;
                let delta_output;

                if let Some(total) = info.and_then(|info| info.get("total_token_usage")) {
                    let input = value_to_i64(total.get("input_tokens"));
                    let cached = value_to_i64(
                        total
                            .get("cached_input_tokens")
                            .or_else(|| total.get("cache_read_input_tokens")),
                    );
                    let output = value_to_i64(total.get("output_tokens"));
                    if let Some((prev_input, prev_cached, prev_output)) = previous_totals {
                        delta_input = (input - prev_input).max(0);
                        delta_cached = (cached - prev_cached).max(0);
                        delta_output = (output - prev_output).max(0);
                    } else {
                        delta_input = input.max(0);
                        delta_cached = cached.max(0);
                        delta_output = output.max(0);
                    }
                    previous_totals = Some((input, cached, output));
                } else if let Some(last) = info.and_then(|info| info.get("last_token_usage")) {
                    delta_input = value_to_i64(last.get("input_tokens")).max(0);
                    delta_cached = value_to_i64(
                        last.get("cached_input_tokens")
                            .or_else(|| last.get("cache_read_input_tokens")),
                    )
                    .max(0);
                    delta_output = value_to_i64(last.get("output_tokens")).max(0);
                } else {
                    continue;
                }

                if delta_input == 0 && delta_cached == 0 && delta_output == 0 {
                    continue;
                }
                let cached_clamped = delta_cached.min(delta_input.max(0));
                let cost = codex_cost_usd(&model, delta_input, cached_clamped, delta_output);

                let entry = totals.entry(day_key).or_default();
                entry.input += delta_input;
                entry.output += delta_output;
                entry.cache_read += cached_clamped;
                if let Some(cost) = cost {
                    entry.cost += cost;
                    entry.cost_seen = true;
                }
            }
            _ => {}
        }
    }
}

fn day_key_from_timestamp(value: Option<&Value>) -> Option<String> {
    let value = value?;
    let parsed = match value {
        Value::String(text) => parse_timestamp(text),
        Value::Number(number) => number
            .as_i64()
            .and_then(|ts| DateTime::<Utc>::from_timestamp(ts, 0))
            .map(|dt| dt.with_timezone(&Local)),
        _ => None,
    }?;
    Some(day_key_from_date(parsed.date_naive()))
}

fn parse_timestamp(text: &str) -> Option<DateTime<Local>> {
    if let Ok(dt) = DateTime::parse_from_rfc3339(text) {
        return Some(dt.with_timezone(&Local));
    }
    None
}

fn scan_claude_file(
    path: &Path,
    totals: &mut HashMap<String, DayTotals>,
    since_key: &str,
    until_key: &str,
) {
    let file = match fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return,
    };
    let reader = BufReader::new(file);
    let mut seen_keys = HashSet::new();

    for line in reader.lines().flatten() {
        if !line.contains("\"type\":\"assistant\"") || !line.contains("\"usage\"") {
            continue;
        }
        let value: Value = match serde_json::from_str(&line) {
            Ok(value) => value,
            Err(_) => continue,
        };
        if value.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }
        let timestamp = value.get("timestamp");
        let day_key = match day_key_from_timestamp(timestamp) {
            Some(day) => day,
            None => continue,
        };
        if day_key.as_str() < since_key || day_key.as_str() > until_key {
            continue;
        }

        let message = match value.get("message") {
            Some(message) => message,
            None => continue,
        };
        let model = match message.get("model").and_then(Value::as_str) {
            Some(model) => model,
            None => continue,
        };
        let usage = match message.get("usage") {
            Some(usage) => usage,
            None => continue,
        };

        if let (Some(message_id), Some(request_id)) = (
            message.get("id").and_then(Value::as_str),
            value.get("requestId").and_then(Value::as_str),
        ) {
            let key = format!("{}:{}", message_id, request_id);
            if seen_keys.contains(&key) {
                continue;
            }
            seen_keys.insert(key);
        }

        let input = value_to_i64(usage.get("input_tokens")).max(0);
        let cache_create = value_to_i64(usage.get("cache_creation_input_tokens")).max(0);
        let cache_read = value_to_i64(usage.get("cache_read_input_tokens")).max(0);
        let output = value_to_i64(usage.get("output_tokens")).max(0);

        if input == 0 && cache_create == 0 && cache_read == 0 && output == 0 {
            continue;
        }

        let cost = claude_cost_usd(model, input, cache_read, cache_create, output);
        let entry = totals.entry(day_key).or_default();
        entry.input += input;
        entry.output += output;
        entry.cache_read += cache_read;
        entry.cache_create += cache_create;
        if let Some(cost) = cost {
            entry.cost += cost;
            entry.cost_seen = true;
        }
    }
}

fn build_cost_snapshot(
    provider: ProviderId,
    totals: &HashMap<String, DayTotals>,
    since_key: &str,
    until_key: &str,
) -> Option<CostSnapshot> {
    let mut day_keys: Vec<&String> = totals
        .keys()
        .filter(|key| key.as_str() >= since_key && key.as_str() <= until_key)
        .collect();
    if day_keys.is_empty() {
        return None;
    }
    day_keys.sort();
    let latest_key = *day_keys.last()?;
    let latest = totals.get(latest_key)?;

    let mut month_tokens: i64 = 0;
    let mut month_cost: f64 = 0.0;
    let mut cost_seen = false;

    for key in day_keys {
        if let Some(day) = totals.get(key) {
            month_tokens += day_token_total(provider, day);
            if day.cost_seen {
                month_cost += day.cost;
                cost_seen = true;
            }
        }
    }

    let today_tokens = day_token_total(provider, latest);
    let today_cost = if latest.cost_seen { latest.cost } else { 0.0 };

    if month_tokens <= 0 && !cost_seen {
        return None;
    }

    Some(CostSnapshot {
        today_amount: today_cost,
        today_tokens: today_tokens.max(0) as u64,
        month_amount: month_cost,
        month_tokens: month_tokens.max(0) as u64,
        currency: "$".to_string(),
    })
}

fn day_token_total(provider: ProviderId, day: &DayTotals) -> i64 {
    match provider {
        ProviderId::Codex => day.input + day.output,
        ProviderId::Claude => day.input + day.output + day.cache_read + day.cache_create,
        _ => day.input + day.output,
    }
}

fn day_key_range(now: DateTime<Local>) -> (String, String) {
    let until_date = now.date_naive();
    let since_date = until_date - Duration::days(29);
    (day_key_from_date(since_date), day_key_from_date(until_date))
}

fn day_key_from_date(date: NaiveDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

fn value_to_i64(value: Option<&Value>) -> i64 {
    match value {
        Some(Value::Number(num)) => num
            .as_i64()
            .or_else(|| num.as_f64().map(|n| n as i64))
            .unwrap_or(0),
        Some(Value::String(text)) => text.trim().parse::<i64>().unwrap_or(0),
        _ => 0,
    }
}

fn codex_cost_usd(
    model: &str,
    input_tokens: i64,
    cached_input_tokens: i64,
    output_tokens: i64,
) -> Option<f64> {
    let normalized = normalize_codex_model(model);
    let pricing = CODEX_PRICING.get(normalized.as_str())?;
    let cached = cached_input_tokens.max(0).min(input_tokens.max(0));
    let non_cached = input_tokens.max(0) - cached;
    Some(
        (non_cached as f64) * pricing.input_cost_per_token
            + (cached as f64) * pricing.cache_read_input_cost_per_token
            + (output_tokens.max(0) as f64) * pricing.output_cost_per_token,
    )
}

fn claude_cost_usd(
    model: &str,
    input_tokens: i64,
    cache_read_tokens: i64,
    cache_creation_tokens: i64,
    output_tokens: i64,
) -> Option<f64> {
    let normalized = normalize_claude_model(model);
    let pricing = CLAUDE_PRICING.get(normalized.as_str())?;

    fn tiered(tokens: i64, base: f64, above: Option<f64>, threshold: Option<i64>) -> f64 {
        if let (Some(threshold), Some(above)) = (threshold, above) {
            let below = tokens.min(threshold).max(0);
            let over = (tokens - threshold).max(0);
            (below as f64) * base + (over as f64) * above
        } else {
            (tokens.max(0) as f64) * base
        }
    }

    Some(
        tiered(
            input_tokens,
            pricing.input_cost_per_token,
            pricing.input_cost_per_token_above_threshold,
            pricing.threshold_tokens,
        ) + tiered(
            cache_read_tokens,
            pricing.cache_read_input_cost_per_token,
            pricing.cache_read_input_cost_per_token_above_threshold,
            pricing.threshold_tokens,
        ) + tiered(
            cache_creation_tokens,
            pricing.cache_creation_input_cost_per_token,
            pricing.cache_creation_input_cost_per_token_above_threshold,
            pricing.threshold_tokens,
        ) + tiered(
            output_tokens,
            pricing.output_cost_per_token,
            pricing.output_cost_per_token_above_threshold,
            pricing.threshold_tokens,
        ),
    )
}

fn normalize_codex_model(raw: &str) -> String {
    let mut trimmed = raw.trim().to_string();
    if let Some(stripped) = trimmed.strip_prefix("openai/") {
        trimmed = stripped.to_string();
    }
    if let Some(range) = trimmed.find("-codex") {
        let base = &trimmed[..range];
        if CODEX_PRICING.contains_key(base) {
            return base.to_string();
        }
    }
    trimmed
}

fn normalize_claude_model(raw: &str) -> String {
    let mut trimmed = raw.trim().to_string();
    if let Some(stripped) = trimmed.strip_prefix("anthropic.") {
        trimmed = stripped.to_string();
    }
    if trimmed.contains("claude-") {
        if let Some(idx) = trimmed.rfind('.') {
            let tail = &trimmed[idx + 1..];
            if tail.starts_with("claude-") {
                trimmed = tail.to_string();
            }
        }
    }
    if CLAUDE_VERSION_SUFFIX.is_match(&trimmed) {
        trimmed = CLAUDE_VERSION_SUFFIX.replace(&trimmed, "").to_string();
    }
    if CLAUDE_BASE_SUFFIX.is_match(&trimmed) {
        let base = CLAUDE_BASE_SUFFIX.replace(&trimmed, "").to_string();
        if CLAUDE_PRICING.contains_key(base.as_str()) {
            return base;
        }
    }
    trimmed
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs::File;
    use std::io::Write;

    fn write_jsonl(path: &Path, lines: &[String]) {
        let mut file = File::create(path).expect("create temp file");
        for line in lines {
            writeln!(file, "{}", line).expect("write line");
        }
    }

    #[test]
    fn scans_codex_cost_usage_last_30_days() {
        let temp = tempfile::tempdir().expect("temp dir");
        let sessions = temp.path().join("sessions");
        fs::create_dir_all(&sessions).expect("create sessions dir");
        let log_path = sessions.join("usage.jsonl");

        let lines = vec![
            serde_json::json!({
                "type": "turn_context",
                "timestamp": "2025-01-05T10:00:00Z",
                "payload": {"model": "gpt-5"}
            })
            .to_string(),
            serde_json::json!({
                "type": "event_msg",
                "timestamp": "2025-01-05T10:00:01Z",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "total_token_usage": {
                            "input_tokens": 1000,
                            "cached_input_tokens": 200,
                            "output_tokens": 500
                        }
                    }
                }
            })
            .to_string(),
            serde_json::json!({
                "type": "event_msg",
                "timestamp": "2025-01-05T10:00:02Z",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "total_token_usage": {
                            "input_tokens": 1500,
                            "cached_input_tokens": 300,
                            "output_tokens": 700
                        }
                    }
                }
            })
            .to_string(),
            serde_json::json!({
                "type": "event_msg",
                "timestamp": "2024-11-01T10:00:00Z",
                "payload": {
                    "type": "token_count",
                    "info": {
                        "total_token_usage": {
                            "input_tokens": 2000,
                            "cached_input_tokens": 200,
                            "output_tokens": 800
                        }
                    }
                }
            })
            .to_string(),
        ];
        write_jsonl(&log_path, &lines);

        let options = ScanOptions {
            codex_sessions_root: Some(sessions),
            claude_projects_roots: None,
            now: Local.with_ymd_and_hms(2025, 1, 20, 0, 0, 0).unwrap(),
        };

        let snapshot = scan_cost_snapshot(ProviderId::Codex, &options).expect("snapshot");
        assert_eq!(snapshot.today_tokens, 2200);
        assert_eq!(snapshot.month_tokens, 2200);

        let first_cost = codex_cost_usd("gpt-5", 1000, 200, 500).unwrap();
        let second_cost = codex_cost_usd("gpt-5", 500, 100, 200).unwrap();
        let expected = first_cost + second_cost;
        let delta = (snapshot.today_amount - expected).abs();
        assert!(delta < 1e-9, "cost delta {delta}");
    }

    #[test]
    fn scans_claude_cost_usage_with_dedupe() {
        let temp = tempfile::tempdir().expect("temp dir");
        let projects = temp.path().join("projects");
        fs::create_dir_all(&projects).expect("create projects dir");
        let log_path = projects.join("claude.jsonl");

        let base_line = serde_json::json!({
            "type": "assistant",
            "timestamp": "2025-01-10T12:00:00Z",
            "requestId": "req_1",
            "message": {
                "id": "msg_1",
                "model": "claude-sonnet-4-5",
                "usage": {
                    "input_tokens": 1000,
                    "cache_creation_input_tokens": 100,
                    "cache_read_input_tokens": 50,
                    "output_tokens": 200
                }
            }
        })
        .to_string();

        let lines = vec![base_line.clone(), base_line];
        write_jsonl(&log_path, &lines);

        let options = ScanOptions {
            codex_sessions_root: None,
            claude_projects_roots: Some(vec![projects]),
            now: Local.with_ymd_and_hms(2025, 1, 25, 0, 0, 0).unwrap(),
        };

        let snapshot = scan_cost_snapshot(ProviderId::Claude, &options).expect("snapshot");
        assert_eq!(snapshot.today_tokens, 1350);
        assert_eq!(snapshot.month_tokens, 1350);

        let expected = claude_cost_usd("claude-sonnet-4-5", 1000, 50, 100, 200).unwrap();
        let delta = (snapshot.today_amount - expected).abs();
        assert!(delta < 1e-9, "cost delta {delta}");
    }
}
