//! OpenCode provider implementation
//!
//! Uses cookie-based authentication via browser cookie import.
//! Endpoint: https://opencode.ai/_server

use async_trait::async_trait;
use serde_json::Value;
use super::{ProviderFetcher, UsageSnapshot, RateWindow, ProviderIdentity};

const BASE_URL: &str = "https://opencode.ai";
const SERVER_URL: &str = "https://opencode.ai/_server";
const WORKSPACES_SERVER_ID: &str =
    "def39973159c7f0483d8793a822b8dbb10d067e12c65455fcb4608459ba0234f";
const SUBSCRIPTION_SERVER_ID: &str =
    "7abeebee372f304e050aaaf92be863f4a86490e382f8c79db68fd94040d691b4";

const USER_AGENT: &str =
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) \
     Chrome/143.0.0.0 Safari/537.36";

const PERCENT_KEYS: &[&str] = &[
    "usagePercent",
    "usedPercent",
    "percentUsed",
    "percent",
    "usage_percent",
    "used_percent",
    "utilization",
    "utilizationPercent",
    "utilization_percent",
    "usage",
];

const RESET_IN_KEYS: &[&str] = &[
    "resetInSec",
    "resetInSeconds",
    "resetSeconds",
    "reset_sec",
    "reset_in_sec",
    "resetsInSec",
    "resetsInSeconds",
    "resetIn",
    "resetSec",
];

const RESET_AT_KEYS: &[&str] = &[
    "resetAt",
    "resetsAt",
    "reset_at",
    "resets_at",
    "nextReset",
    "next_reset",
    "renewAt",
    "renew_at",
];

pub struct OpencodeProvider {
    client: reqwest::Client,
}

impl OpencodeProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_with_cookies(&self, cookie_header: &str) -> Result<UsageSnapshot, OpencodeError> {
        let now = chrono::Utc::now();
        let workspace_id = self.fetch_workspace_id(cookie_header).await?;
        let subscription_text = self
            .fetch_subscription_info(&workspace_id, cookie_header, now)
            .await?;
        let snapshot = self.parse_subscription(&subscription_text, now)?;
        Ok(snapshot.to_usage_snapshot())
    }

    async fn fetch_workspace_id(&self, cookie_header: &str) -> Result<String, OpencodeError> {
        let text = self
            .fetch_server_text(
                ServerRequest {
                    server_id: WORKSPACES_SERVER_ID,
                    args: None,
                    method: "GET",
                    referer: BASE_URL.to_string(),
                },
                cookie_header,
            )
            .await?;

        if looks_signed_out(&text) {
            return Err(OpencodeError::InvalidCredentials);
        }

        let mut ids = parse_workspace_ids(&text);
        if ids.is_empty() {
            ids = parse_workspace_ids_from_json(&text);
        }

        if ids.is_empty() {
            let fallback = self
                .fetch_server_text(
                    ServerRequest {
                        server_id: WORKSPACES_SERVER_ID,
                        args: Some(Value::Array(Vec::new())),
                        method: "POST",
                        referer: BASE_URL.to_string(),
                    },
                    cookie_header,
                )
                .await?;

            if looks_signed_out(&fallback) {
                return Err(OpencodeError::InvalidCredentials);
            }

            ids = parse_workspace_ids(&fallback);
            if ids.is_empty() {
                ids = parse_workspace_ids_from_json(&fallback);
            }

            if ids.is_empty() {
                return Err(OpencodeError::Parse("Missing workspace id".to_string()));
            }

            return Ok(ids[0].clone());
        }

        Ok(ids[0].clone())
    }

    async fn fetch_subscription_info(
        &self,
        workspace_id: &str,
        cookie_header: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<String, OpencodeError> {
        let referer = format!("https://opencode.ai/workspace/{}/billing", workspace_id);
        let text = self
            .fetch_server_text(
                ServerRequest {
                    server_id: SUBSCRIPTION_SERVER_ID,
                    args: Some(Value::Array(vec![Value::String(workspace_id.to_string())])),
                    method: "GET",
                    referer: referer.clone(),
                },
                cookie_header,
            )
            .await?;

        if looks_signed_out(&text) {
            return Err(OpencodeError::InvalidCredentials);
        }

        let has_usage = self.parse_subscription_json(&text, now).is_some()
            || extract_double(
                r#"rollingUsage[^}]*?usagePercent\s*:\s*([0-9]+(?:\.[0-9]+)?)"#,
                &text,
            )
            .is_some();

        if !has_usage {
            let fallback = self
                .fetch_server_text(
                    ServerRequest {
                        server_id: SUBSCRIPTION_SERVER_ID,
                        args: Some(Value::Array(vec![Value::String(workspace_id.to_string())])),
                        method: "POST",
                        referer,
                    },
                    cookie_header,
                )
                .await?;

            if looks_signed_out(&fallback) {
                return Err(OpencodeError::InvalidCredentials);
            }

            return Ok(fallback);
        }

        Ok(text)
    }

    async fn fetch_server_text(
        &self,
        server_request: ServerRequest,
        cookie_header: &str,
    ) -> Result<String, OpencodeError> {
        let url = server_request.url();
        let mut request = match server_request.method {
            "POST" => self.client.post(url),
            _ => self.client.get(url),
        };

        request = request
            .header("Cookie", cookie_header)
            .header("X-Server-Id", server_request.server_id)
            .header("X-Server-Instance", format!("server-fn:{}", uuid::Uuid::new_v4()))
            .header("User-Agent", USER_AGENT)
            .header("Origin", BASE_URL)
            .header("Referer", server_request.referer)
            .header("Accept", "text/javascript, application/json;q=0.9, */*;q=0.8");

        if server_request.method == "POST" {
            if let Some(args) = server_request.args {
                request = request.json(&args);
            }
        }

        let response = request.send().await.map_err(|err| OpencodeError::Api(err.to_string()))?;
        let status = response.status();
        let text = response
            .text()
            .await
            .map_err(|err| OpencodeError::Api(err.to_string()))?;

        if !status.is_success() {
            if looks_signed_out(&text) || status.as_u16() == 401 || status.as_u16() == 403 {
                return Err(OpencodeError::InvalidCredentials);
            }

            if let Some(message) = extract_server_error_message(&text) {
                return Err(OpencodeError::Api(format!("HTTP {}: {}", status, message)));
            }

            return Err(OpencodeError::Api(format!("HTTP {}", status)));
        }

        Ok(text)
    }

    fn parse_subscription(
        &self,
        text: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Result<OpenCodeUsageSnapshot, OpencodeError> {
        if let Some(snapshot) = self.parse_subscription_json(text, now) {
            return Ok(snapshot);
        }

        let rolling_percent = extract_double(
            r#"rollingUsage[^}]*?usagePercent\s*:\s*([0-9]+(?:\.[0-9]+)?)"#,
            text,
        );
        let rolling_reset = extract_int(r#"rollingUsage[^}]*?resetInSec\s*:\s*([0-9]+)"#, text);
        let weekly_percent = extract_double(
            r#"weeklyUsage[^}]*?usagePercent\s*:\s*([0-9]+(?:\.[0-9]+)?)"#,
            text,
        );
        let weekly_reset = extract_int(r#"weeklyUsage[^}]*?resetInSec\s*:\s*([0-9]+)"#, text);

        match (rolling_percent, rolling_reset, weekly_percent, weekly_reset) {
            (Some(rolling_percent), Some(rolling_reset), Some(weekly_percent), Some(weekly_reset)) => {
                Ok(OpenCodeUsageSnapshot {
                    rolling_usage_percent: rolling_percent,
                    weekly_usage_percent: weekly_percent,
                    rolling_reset_in_sec: rolling_reset,
                    weekly_reset_in_sec: weekly_reset,
                    updated_at: now,
                })
            }
            _ => Err(OpencodeError::Parse("Missing usage fields".to_string())),
        }
    }

    fn parse_subscription_json(
        &self,
        text: &str,
        now: chrono::DateTime<chrono::Utc>,
    ) -> Option<OpenCodeUsageSnapshot> {
        let data: Value = serde_json::from_str(text).ok()?;
        parse_usage_json(&data, now).or_else(|| parse_usage_from_candidates(&data, now))
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;
        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: OpencodeSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }
        Err(anyhow::anyhow!("No stored OpenCode session found"))
    }

    async fn store_session(&self, cookie_header: &str) -> Result<(), anyhow::Error> {
        let session_path = self.get_session_path()?;
        if let Some(parent) = session_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        let content = serde_json::json!({
            "cookieHeader": cookie_header,
            "savedAt": chrono::Utc::now().to_rfc3339(),
        });
        tokio::fs::write(&session_path, serde_json::to_string_pretty(&content)?).await?;
        Ok(())
    }

    async fn clear_session(&self) {
        if let Ok(path) = self.get_session_path() {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    fn get_session_path(&self) -> Result<std::path::PathBuf, anyhow::Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("opencode-session.json"))
    }
}

#[async_trait]
impl ProviderFetcher for OpencodeProvider {
    fn name(&self) -> &'static str {
        "OpenCode"
    }

    fn description(&self) -> &'static str {
        "OpenCode"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching OpenCode usage");

        if let Ok(cookies) = self.load_stored_cookies().await {
            match self.fetch_with_cookies(&cookies).await {
                Ok(usage) => return Ok(usage),
                Err(err) => {
                    tracing::debug!("OpenCode fetch with stored cookies failed: {}", err);
                    if matches!(err, OpencodeError::InvalidCredentials) {
                        self.clear_session().await;
                    }
                }
            }
        }

        match crate::browser_cookies::import_opencode_cookies_from_browser().await {
            Ok(result) => {
                if !cookie_header_has_auth(&result.cookie_header) {
                    return Err(anyhow::anyhow!(OpencodeError::InvalidCredentials.to_string()));
                }
                if let Err(err) = self.store_session(&result.cookie_header).await {
                    tracing::debug!("Failed to store OpenCode session: {}", err);
                }
                self.fetch_with_cookies(&result.cookie_header)
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))
            }
            Err(err) => Err(anyhow::anyhow!("Not authenticated: {}", err)),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OpencodeSession {
    cookie_header: String,
}

#[derive(Debug, Clone)]
struct OpenCodeUsageSnapshot {
    rolling_usage_percent: f64,
    weekly_usage_percent: f64,
    rolling_reset_in_sec: i64,
    weekly_reset_in_sec: i64,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl OpenCodeUsageSnapshot {
    fn to_usage_snapshot(&self) -> UsageSnapshot {
        let rolling_reset = self.updated_at + chrono::Duration::seconds(self.rolling_reset_in_sec);
        let weekly_reset = self.updated_at + chrono::Duration::seconds(self.weekly_reset_in_sec);

        UsageSnapshot {
            primary: Some(RateWindow {
                used_percent: self.rolling_usage_percent,
                window_minutes: Some(5 * 60),
                resets_at: Some(rolling_reset.to_rfc3339()),
                reset_description: None,
                label: Some("5-hour".to_string()),
            }),
            secondary: Some(RateWindow {
                used_percent: self.weekly_usage_percent,
                window_minutes: Some(7 * 24 * 60),
                resets_at: Some(weekly_reset.to_rfc3339()),
                reset_description: None,
                label: Some("Weekly".to_string()),
            }),
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: None,
                organization: None,
            }),
            updated_at: self.updated_at.to_rfc3339(),
            error: None,
        }
    }
}

#[derive(Debug)]
struct ServerRequest {
    server_id: &'static str,
    args: Option<Value>,
    method: &'static str,
    referer: String,
}

impl ServerRequest {
    fn url(&self) -> String {
        if self.method == "GET" {
            let mut url = url::Url::parse(SERVER_URL).unwrap_or_else(|_| url::Url::parse(BASE_URL).unwrap());
            url.query_pairs_mut().append_pair("id", self.server_id);
            if let Some(args) = &self.args {
                if let Ok(encoded) = serde_json::to_string(args) {
                    if !encoded.is_empty() && encoded != "[]" {
                        url.query_pairs_mut().append_pair("args", &encoded);
                    }
                }
            }
            url.to_string()
        } else {
            SERVER_URL.to_string()
        }
    }
}

fn looks_signed_out(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("login") || lower.contains("sign in") || lower.contains("auth/authorize")
}

fn cookie_header_has_auth(cookie_header: &str) -> bool {
    cookie_header.split(';').any(|part| {
        let trimmed = part.trim();
        trimmed.starts_with("auth=") || trimmed.starts_with("__Host-auth=")
    })
}

fn parse_workspace_ids(text: &str) -> Vec<String> {
    let regex = regex::Regex::new(r#"id\s*:\s*\"(wrk_[^\"]+)\""#).ok();
    if let Some(regex) = regex {
        regex
            .captures_iter(text)
            .filter_map(|cap| cap.get(1).map(|m| m.as_str().to_string()))
            .collect()
    } else {
        Vec::new()
    }
}

fn parse_workspace_ids_from_json(text: &str) -> Vec<String> {
    let value: Value = match serde_json::from_str(text) {
        Ok(value) => value,
        Err(_) => return Vec::new(),
    };
    let mut ids = Vec::new();
    collect_workspace_ids(&value, &mut ids);
    ids
}

fn collect_workspace_ids(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for val in map.values() {
                collect_workspace_ids(val, out);
            }
        }
        Value::Array(items) => {
            for item in items {
                collect_workspace_ids(item, out);
            }
        }
        Value::String(s) => {
            if s.starts_with("wrk_") && !out.contains(s) {
                out.push(s.to_string());
            }
        }
        _ => {}
    }
}

fn extract_double(pattern: &str, text: &str) -> Option<f64> {
    let regex = regex::Regex::new(pattern).ok()?;
    let capture = regex.captures(text)?;
    capture.get(1)?.as_str().parse::<f64>().ok()
}

fn extract_int(pattern: &str, text: &str) -> Option<i64> {
    let regex = regex::Regex::new(pattern).ok()?;
    let capture = regex.captures(text)?;
    capture.get(1)?.as_str().parse::<i64>().ok()
}

fn extract_server_error_message(text: &str) -> Option<String> {
    let value: Value = serde_json::from_str(text).ok()?;
    if let Value::Object(map) = value {
        if let Some(Value::String(message)) = map.get("message") {
            if !message.is_empty() {
                return Some(message.clone());
            }
        }
        if let Some(Value::String(message)) = map.get("error") {
            if !message.is_empty() {
                return Some(message.clone());
            }
        }
    }
    None
}

fn parse_usage_json(value: &Value, now: chrono::DateTime<chrono::Utc>) -> Option<OpenCodeUsageSnapshot> {
    let dict = value.as_object()?;
    if let Some(snapshot) = parse_usage_dictionary(dict, now) {
        return Some(snapshot);
    }

    for key in ["data", "result", "usage", "billing", "payload"] {
        if let Some(Value::Object(obj)) = dict.get(key) {
            if let Some(snapshot) = parse_usage_dictionary(obj, now) {
                return Some(snapshot);
            }
        }
    }

    parse_usage_nested(dict, now, 0)
}

fn parse_usage_dictionary(
    dict: &serde_json::Map<String, Value>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<OpenCodeUsageSnapshot> {
    if let Some(Value::Object(usage)) = dict.get("usage") {
        if let Some(snapshot) = parse_usage_dictionary(usage, now) {
            return Some(snapshot);
        }
    }

    let rolling_keys = ["rollingUsage", "rolling", "rolling_usage", "rollingWindow", "rolling_window"];
    let weekly_keys = ["weeklyUsage", "weekly", "weekly_usage", "weeklyWindow", "weekly_window"];

    let rolling = rolling_keys
        .iter()
        .filter_map(|key| dict.get(*key).and_then(|value| value.as_object()))
        .next();
    let weekly = weekly_keys
        .iter()
        .filter_map(|key| dict.get(*key).and_then(|value| value.as_object()))
        .next();

    if let (Some(rolling), Some(weekly)) = (rolling, weekly) {
        return build_snapshot(rolling, weekly, now);
    }

    None
}

fn parse_usage_nested(
    dict: &serde_json::Map<String, Value>,
    now: chrono::DateTime<chrono::Utc>,
    depth: usize,
) -> Option<OpenCodeUsageSnapshot> {
    if depth > 3 {
        return None;
    }

    let mut rolling: Option<&serde_json::Map<String, Value>> = None;
    let mut weekly: Option<&serde_json::Map<String, Value>> = None;

    for (key, value) in dict.iter() {
        if let Some(sub) = value.as_object() {
            let lower = key.to_lowercase();
            if lower.contains("rolling") {
                rolling = Some(sub);
            } else if lower.contains("weekly") || lower.contains("week") {
                weekly = Some(sub);
            }
        }
    }

    if let (Some(rolling), Some(weekly)) = (rolling, weekly) {
        if let Some(snapshot) = build_snapshot(rolling, weekly, now) {
            return Some(snapshot);
        }
    }

    for value in dict.values() {
        if let Some(sub) = value.as_object() {
            if let Some(snapshot) = parse_usage_nested(sub, now, depth + 1) {
                return Some(snapshot);
            }
        }
    }

    None
}

fn parse_usage_from_candidates(value: &Value, now: chrono::DateTime<chrono::Utc>) -> Option<OpenCodeUsageSnapshot> {
    let mut candidates = Vec::new();
    collect_window_candidates(value, now, &mut Vec::new(), &mut candidates);
    if candidates.is_empty() {
        return None;
    }

    let rolling_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.path_lower.contains("rolling")
                || candidate.path_lower.contains("hour")
                || candidate.path_lower.contains("5h")
                || candidate.path_lower.contains("5-hour")
        })
        .cloned()
        .collect();
    let weekly_candidates: Vec<_> = candidates
        .iter()
        .filter(|candidate| {
            candidate.path_lower.contains("weekly") || candidate.path_lower.contains("week")
        })
        .cloned()
        .collect();

    let rolling = pick_candidate(&rolling_candidates, &candidates, true, None)?;
    let weekly = pick_candidate(&weekly_candidates, &candidates, false, Some(rolling.id))?;

    Some(OpenCodeUsageSnapshot {
        rolling_usage_percent: rolling.percent,
        weekly_usage_percent: weekly.percent,
        rolling_reset_in_sec: rolling.reset_in_sec,
        weekly_reset_in_sec: weekly.reset_in_sec,
        updated_at: now,
    })
}

#[derive(Clone)]
struct WindowCandidate {
    id: uuid::Uuid,
    percent: f64,
    reset_in_sec: i64,
    path_lower: String,
}

fn collect_window_candidates(
    value: &Value,
    now: chrono::DateTime<chrono::Utc>,
    path: &mut Vec<String>,
    out: &mut Vec<WindowCandidate>,
) {
    match value {
        Value::Object(map) => {
            if let Some(window) = parse_window(map, now) {
                let path_lower = path.join(".").to_lowercase();
                out.push(WindowCandidate {
                    id: uuid::Uuid::new_v4(),
                    percent: window.0,
                    reset_in_sec: window.1,
                    path_lower,
                });
            }
            for (key, val) in map {
                path.push(key.clone());
                collect_window_candidates(val, now, path, out);
                path.pop();
            }
        }
        Value::Array(items) => {
            for (index, val) in items.iter().enumerate() {
                path.push(format!("[{}]", index));
                collect_window_candidates(val, now, path, out);
                path.pop();
            }
        }
        _ => {}
    }
}

fn pick_candidate(
    preferred: &[WindowCandidate],
    fallback: &[WindowCandidate],
    pick_shorter: bool,
    excluding: Option<uuid::Uuid>,
) -> Option<WindowCandidate> {
    let filtered = preferred
        .iter()
        .filter(|candidate| Some(candidate.id) != excluding)
        .cloned()
        .collect::<Vec<_>>();
    if let Some(picked) = pick_candidate_from(&filtered, pick_shorter) {
        return Some(picked);
    }
    let fallback_filtered = fallback
        .iter()
        .filter(|candidate| Some(candidate.id) != excluding)
        .cloned()
        .collect::<Vec<_>>();
    pick_candidate_from(&fallback_filtered, pick_shorter)
}

fn pick_candidate_from(candidates: &[WindowCandidate], pick_shorter: bool) -> Option<WindowCandidate> {
    if candidates.is_empty() {
        return None;
    }
    let mut best = candidates[0].clone();
    for candidate in candidates.iter().skip(1) {
        let replace = if pick_shorter {
            if candidate.reset_in_sec == best.reset_in_sec {
                candidate.percent > best.percent
            } else {
                candidate.reset_in_sec < best.reset_in_sec
            }
        } else if candidate.reset_in_sec == best.reset_in_sec {
            candidate.percent > best.percent
        } else {
            candidate.reset_in_sec > best.reset_in_sec
        };
        if replace {
            best = candidate.clone();
        }
    }
    Some(best)
}

fn build_snapshot(
    rolling: &serde_json::Map<String, Value>,
    weekly: &serde_json::Map<String, Value>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<OpenCodeUsageSnapshot> {
    let rolling_window = parse_window(rolling, now)?;
    let weekly_window = parse_window(weekly, now)?;
    Some(OpenCodeUsageSnapshot {
        rolling_usage_percent: rolling_window.0,
        weekly_usage_percent: weekly_window.0,
        rolling_reset_in_sec: rolling_window.1,
        weekly_reset_in_sec: weekly_window.1,
        updated_at: now,
    })
}

fn parse_window(
    dict: &serde_json::Map<String, Value>,
    now: chrono::DateTime<chrono::Utc>,
) -> Option<(f64, i64)> {
    let mut percent = double_value_from_map(dict, PERCENT_KEYS);

    if percent.is_none() {
        let used = double_value_from_map(dict, &["used", "usage", "consumed", "count", "usedTokens"]);
        let limit = double_value_from_map(dict, &["limit", "total", "quota", "max", "cap", "tokenLimit"]);
        if let (Some(used), Some(limit)) = (used, limit) {
            if limit > 0.0 {
                percent = Some((used / limit) * 100.0);
            }
        }
    }

    let mut resolved_percent = percent?;
    if resolved_percent <= 1.0 && resolved_percent >= 0.0 {
        resolved_percent *= 100.0;
    }
    resolved_percent = resolved_percent.clamp(0.0, 100.0);

    let mut reset_in_sec = int_value_from_map(dict, RESET_IN_KEYS).map(|value| value as i64);
    if reset_in_sec.is_none() {
        let reset_value = value_from_map(dict, RESET_AT_KEYS);
        if let Some(reset_at) = date_value(reset_value) {
            reset_in_sec = Some((reset_at - now).num_seconds().max(0));
        }
    }

    Some((resolved_percent, reset_in_sec.unwrap_or(0).max(0)))
}

fn double_value_from_map(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<f64> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(number) = double_value(value) {
                return Some(number);
            }
        }
    }
    None
}

fn int_value_from_map(map: &serde_json::Map<String, Value>, keys: &[&str]) -> Option<i64> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            if let Some(number) = int_value(value) {
                return Some(number);
            }
        }
    }
    None
}

fn value_from_map<'a>(map: &'a serde_json::Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    for key in keys {
        if let Some(value) = map.get(*key) {
            return Some(value);
        }
    }
    None
}

fn double_value(value: &Value) -> Option<f64> {
    match value {
        Value::Number(num) => num.as_f64(),
        Value::String(s) => s.trim().parse::<f64>().ok(),
        _ => None,
    }
}

fn int_value(value: &Value) -> Option<i64> {
    match value {
        Value::Number(num) => num.as_i64().or_else(|| num.as_f64().map(|n| n as i64)),
        Value::String(s) => s.trim().parse::<i64>().ok(),
        _ => None,
    }
}

fn date_value(value: Option<&Value>) -> Option<chrono::DateTime<chrono::Utc>> {
    let value = value?;
    if let Some(number) = double_value(value) {
        if number > 1_000_000_000_000.0 {
            return chrono::DateTime::from_timestamp((number / 1000.0) as i64, 0);
        }
        if number > 1_000_000_000.0 {
            return chrono::DateTime::from_timestamp(number as i64, 0);
        }
    }

    if let Value::String(text) = value {
        if let Ok(number) = text.trim().parse::<f64>() {
            return date_value(Some(&Value::Number(serde_json::Number::from_f64(number)?)));
        }
        if let Ok(parsed) = chrono::DateTime::parse_from_rfc3339(text) {
            return Some(parsed.with_timezone(&chrono::Utc));
        }
    }

    None
}

#[derive(thiserror::Error, Debug)]
enum OpencodeError {
    #[error("OpenCode session expired")]
    InvalidCredentials,
    #[error("OpenCode API error: {0}")]
    Api(String),
    #[error("OpenCode parse error: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workspace_ids() {
        let text = ";0x00000089;((self.$R=self.$R||{})[\"codexbar\"]=[],";
        let text = format!(
            "{}($R=>$R[0]=[$R[1]={{id:\"wrk_01K6AR1ZET89H8NB691FQ2C2VB\",name:\"Default\",slug:null}}])",
            text
        );
        let ids = parse_workspace_ids(&text);
        assert_eq!(ids, vec!["wrk_01K6AR1ZET89H8NB691FQ2C2VB".to_string()]);
    }

    #[test]
    fn parses_subscription_usage() {
        let provider = OpencodeProvider::new();
        let text = "$R[16]($R[30],$R[41]={rollingUsage:$R[42]={status:\"ok\",resetInSec:5944,usagePercent:17},";
        let text = format!(
            "{}weeklyUsage:$R[43]={{status:\"ok\",resetInSec:278201,usagePercent:75}}}});",
            text
        );
        let now = chrono::DateTime::from_timestamp(0, 0).unwrap();
        let snapshot = provider.parse_subscription(&text, now).expect("snapshot");

        assert_eq!(snapshot.rolling_usage_percent, 17.0);
        assert_eq!(snapshot.weekly_usage_percent, 75.0);
        assert_eq!(snapshot.rolling_reset_in_sec, 5944);
        assert_eq!(snapshot.weekly_reset_in_sec, 278_201);
    }

    #[test]
    fn parses_subscription_from_json_with_reset_at() {
        let provider = OpencodeProvider::new();
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let reset_at = now + chrono::Duration::seconds(3600);
        let payload = serde_json::json!({
            "usage": {
                "rollingUsage": {
                    "usagePercent": 0.25,
                    "resetAt": reset_at.to_rfc3339(),
                },
                "weeklyUsage": {
                    "usagePercent": 75,
                    "resetInSec": 7200,
                }
            }
        });
        let text = payload.to_string();

        let snapshot = provider.parse_subscription(&text, now).expect("snapshot");
        assert_eq!(snapshot.rolling_usage_percent, 25.0);
        assert_eq!(snapshot.weekly_usage_percent, 75.0);
        assert_eq!(snapshot.rolling_reset_in_sec, 3600);
        assert_eq!(snapshot.weekly_reset_in_sec, 7200);
    }

    #[test]
    fn parses_subscription_from_candidate_windows() {
        let provider = OpencodeProvider::new();
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let payload = serde_json::json!({
            "windows": {
                "primaryWindow": {
                    "percent": 0.1,
                    "resetInSec": 300,
                },
                "secondaryWindow": {
                    "percent": 0.5,
                    "resetInSec": 1200,
                }
            }
        });
        let text = payload.to_string();

        let snapshot = provider.parse_subscription(&text, now).expect("snapshot");
        assert_eq!(snapshot.rolling_usage_percent, 10.0);
        assert_eq!(snapshot.weekly_usage_percent, 50.0);
        assert_eq!(snapshot.rolling_reset_in_sec, 300);
        assert_eq!(snapshot.weekly_reset_in_sec, 1200);
    }

    #[test]
    fn computes_usage_percent_from_totals() {
        let provider = OpencodeProvider::new();
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let payload = serde_json::json!({
            "rollingUsage": {
                "used": 25,
                "limit": 100,
                "resetInSec": 600,
            },
            "weeklyUsage": {
                "used": 50,
                "limit": 200,
                "resetInSec": 3600,
            }
        });
        let text = payload.to_string();

        let snapshot = provider.parse_subscription(&text, now).expect("snapshot");
        assert_eq!(snapshot.rolling_usage_percent, 25.0);
        assert_eq!(snapshot.weekly_usage_percent, 25.0);
    }

    #[test]
    fn parse_subscription_throws_when_fields_missing() {
        let provider = OpencodeProvider::new();
        let now = chrono::DateTime::from_timestamp(1_700_000_000, 0).unwrap();
        let text = "{\"ok\":true}";
        let result = provider.parse_subscription(text, now);
        assert!(matches!(result, Err(OpencodeError::Parse(_))));
    }
}
