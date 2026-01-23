//! Antigravity provider implementation
//!
//! Uses local Antigravity language server status probe to read usage data.

use super::{
    ProviderFetcher,
    ProviderIdentity,
    ProviderStatus,
    RateWindow,
    StatusIndicator,
    UsageSnapshot,
};
use async_trait::async_trait;
use regex::Regex;

const PROCESS_NAME: &str = "language_server_macos";
const GET_USER_STATUS_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetUserStatus";
const COMMAND_MODEL_CONFIG_PATH: &str =
    "/exa.language_server_pb.LanguageServerService/GetCommandModelConfigs";
const UNLEASH_PATH: &str = "/exa.language_server_pb.LanguageServerService/GetUnleashData";

const DEFAULT_TIMEOUT_SECS: u64 = 8;

pub struct AntigravityProvider {
    client: reqwest::Client,
    timeout: std::time::Duration,
}

impl AntigravityProvider {
    pub fn new() -> Self {
        let timeout = std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS);
        let client = reqwest::Client::builder()
            .timeout(timeout)
            .danger_accept_invalid_certs(true)
            .build()
            .unwrap_or_default();

        Self { client, timeout }
    }

    async fn fetch_snapshot(&self) -> Result<AntigravityStatusSnapshot, AntigravityError> {
        let process = self.detect_process_info().await?;
        let ports = self.listening_ports(process.pid).await?;
        let connect_port = self.find_working_port(&ports, &process.csrf_token).await?;

        let context = RequestContext {
            https_port: connect_port,
            http_port: process.extension_port,
            csrf_token: process.csrf_token,
        };

        let payload = RequestPayload {
            path: GET_USER_STATUS_PATH,
            body: default_request_body(),
        };

        match self.make_request(payload, &context).await {
            Ok(data) => Self::parse_user_status_response(&data),
            Err(_) => {
                let payload = RequestPayload {
                    path: COMMAND_MODEL_CONFIG_PATH,
                    body: default_request_body(),
                };
                let data = self.make_request(payload, &context).await?;
                Self::parse_command_model_response(&data)
            }
        }
    }

    fn build_usage_snapshot(
        &self,
        snapshot: AntigravityStatusSnapshot,
    ) -> Result<UsageSnapshot, AntigravityError> {
        let ordered = select_models(&snapshot.model_quotas);
        let primary = ordered.get(0).map(to_rate_window);
        let secondary = ordered.get(1).map(to_rate_window);
        let tertiary = ordered.get(2).map(to_rate_window);

        if primary.is_none() {
            return Err(AntigravityError::Parse(
                "No quota models available".to_string(),
            ));
        }

        Ok(UsageSnapshot {
            primary: primary.map(|mut window| {
                window.label = Some("Claude".to_string());
                window
            }),
            secondary: secondary.map(|mut window| {
                window.label = Some("Gemini Pro".to_string());
                window
            }),
            tertiary: tertiary.map(|mut window| {
                window.label = Some("Gemini Flash".to_string());
                window
            }),
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: snapshot.account_email,
                name: None,
                plan: snapshot.account_plan,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        })
    }

    async fn detect_process_info(&self) -> Result<ProcessInfoResult, AntigravityError> {
        let output = tokio::process::Command::new("/bin/ps")
            .args(["-ax", "-o", "pid=,command="])
            .output()
            .await
            .map_err(|err| AntigravityError::Process(format!("ps failed: {}", err)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut saw_antigravity_process = false;
        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let mut parts = trimmed.splitn(2, ' ');
            let pid_str = parts.next().unwrap_or_default();
            let command = parts.next().unwrap_or_default().trim();
            let pid: i32 = match pid_str.parse() {
                Ok(value) => value,
                Err(_) => continue,
            };

            let lower = command.to_lowercase();
            if !lower.contains(PROCESS_NAME) {
                continue;
            }
            if !is_antigravity_command_line(&lower) {
                continue;
            }
            saw_antigravity_process = true;

            let csrf_token =
                extract_flag("--csrf_token", command).ok_or(AntigravityError::MissingCsrfToken)?;
            let extension_port = extract_flag("--extension_server_port", command)
                .and_then(|raw| raw.parse::<i32>().ok());

            return Ok(ProcessInfoResult {
                pid,
                extension_port,
                csrf_token,
            });
        }

        if saw_antigravity_process {
            Err(AntigravityError::MissingCsrfToken)
        } else {
            Err(AntigravityError::NotRunning)
        }
    }

    async fn listening_ports(&self, pid: i32) -> Result<Vec<i32>, AntigravityError> {
        let lsof = if std::path::Path::new("/usr/sbin/lsof").exists() {
            "/usr/sbin/lsof"
        } else if std::path::Path::new("/usr/bin/lsof").exists() {
            "/usr/bin/lsof"
        } else {
            return Err(AntigravityError::PortDetection(
                "lsof not available".to_string(),
            ));
        };

        let output = tokio::process::Command::new(lsof)
            .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-a", "-p", &pid.to_string()])
            .output()
            .await
            .map_err(|err| AntigravityError::PortDetection(format!("lsof failed: {}", err)))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        let ports = parse_listening_ports(&stdout);
        if ports.is_empty() {
            return Err(AntigravityError::PortDetection(
                "no listening ports found".to_string(),
            ));
        }
        Ok(ports)
    }

    async fn find_working_port(
        &self,
        ports: &[i32],
        csrf_token: &str,
    ) -> Result<i32, AntigravityError> {
        for port in ports {
            if self.test_port(*port, csrf_token).await {
                return Ok(*port);
            }
        }
        Err(AntigravityError::PortDetection(
            "no working API port found".to_string(),
        ))
    }

    async fn test_port(&self, port: i32, csrf_token: &str) -> bool {
        let context = RequestContext {
            https_port: port,
            http_port: None,
            csrf_token: csrf_token.to_string(),
        };
        let payload = RequestPayload {
            path: UNLEASH_PATH,
            body: unleash_request_body(),
        };
        self.make_request(payload, &context).await.is_ok()
    }

    async fn make_request(
        &self,
        payload: RequestPayload<'_>,
        context: &RequestContext,
    ) -> Result<Vec<u8>, AntigravityError> {
        let https_result = self
            .send_request("https", context.https_port, &payload, context)
            .await;

        match https_result {
            Ok(data) => Ok(data),
            Err(err) => {
                if let Some(http_port) = context.http_port {
                    if http_port != context.https_port {
                        return self
                            .send_request("http", http_port, &payload, context)
                            .await;
                    }
                }
                Err(err)
            }
        }
    }

    async fn send_request(
        &self,
        scheme: &str,
        port: i32,
        payload: &RequestPayload<'_>,
        context: &RequestContext,
    ) -> Result<Vec<u8>, AntigravityError> {
        let url = format!("{}://127.0.0.1:{}{}", scheme, port, payload.path);
        let body = serde_json::to_vec(&payload.body)
            .map_err(|err| AntigravityError::Api(format!("Serialize failed: {}", err)))?;

        let response = self
            .client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Content-Length", body.len())
            .header("Connect-Protocol-Version", "1")
            .header("X-Codeium-Csrf-Token", &context.csrf_token)
            .timeout(self.timeout)
            .body(body)
            .send()
            .await
            .map_err(|err| AntigravityError::Api(err.to_string()))?;

        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| AntigravityError::Api(err.to_string()))?;
        if !status.is_success() {
            let message = String::from_utf8_lossy(&bytes).to_string();
            return Err(AntigravityError::Api(format!(
                "HTTP {}: {}",
                status, message
            )));
        }
        Ok(bytes.to_vec())
    }

    fn parse_user_status_response(
        data: &[u8],
    ) -> Result<AntigravityStatusSnapshot, AntigravityError> {
        let response: UserStatusResponse =
            serde_json::from_slice(data).map_err(|err| AntigravityError::Parse(err.to_string()))?;
        if let Some(invalid) = invalid_code(response.code.as_ref()) {
            return Err(AntigravityError::Api(invalid));
        }
        let user_status = response
            .user_status
            .ok_or_else(|| AntigravityError::Parse("Missing userStatus".to_string()))?;
        let model_configs = user_status
            .cascade_model_config_data
            .and_then(|data| data.client_model_configs)
            .unwrap_or_default();
        let models = model_configs
            .into_iter()
            .filter_map(quota_from_config)
            .collect::<Vec<_>>();
        let plan = user_status
            .plan_status
            .and_then(|status| status.plan_info)
            .and_then(|info| info.preferred_name());

        Ok(AntigravityStatusSnapshot {
            model_quotas: models,
            account_email: user_status.email,
            account_plan: plan,
        })
    }

    fn parse_command_model_response(
        data: &[u8],
    ) -> Result<AntigravityStatusSnapshot, AntigravityError> {
        let response: CommandModelConfigResponse =
            serde_json::from_slice(data).map_err(|err| AntigravityError::Parse(err.to_string()))?;
        if let Some(invalid) = invalid_code(response.code.as_ref()) {
            return Err(AntigravityError::Api(invalid));
        }
        let model_configs = response.client_model_configs.unwrap_or_default();
        let models = model_configs
            .into_iter()
            .filter_map(quota_from_config)
            .collect::<Vec<_>>();
        Ok(AntigravityStatusSnapshot {
            model_quotas: models,
            account_email: None,
            account_plan: None,
        })
    }
}

#[async_trait]
impl ProviderFetcher for AntigravityProvider {
    fn name(&self) -> &'static str {
        "Antigravity"
    }

    fn description(&self) -> &'static str {
        "Antigravity status probe"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Antigravity usage");
        let snapshot = self.fetch_snapshot().await?;
        Ok(self.build_usage_snapshot(snapshot)?)
    }

    async fn fetch_status(&self) -> Result<ProviderStatus, anyhow::Error> {
        let response = self
            .client
            .get("https://www.google.com/appsstatus/json/en")
            .header("Accept", "application/json")
            .timeout(self.timeout)
            .send()
            .await
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        if !status.is_success() {
            return Err(anyhow::anyhow!(format!("HTTP {}", status)));
        }
        parse_google_workspace_status(&bytes, "npdyhgECDJ6tB66MxXyo")
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    }
}

#[derive(Debug)]
struct ProcessInfoResult {
    pid: i32,
    extension_port: Option<i32>,
    csrf_token: String,
}

#[derive(Debug)]
struct RequestContext {
    https_port: i32,
    http_port: Option<i32>,
    csrf_token: String,
}

#[derive(Debug)]
struct RequestPayload<'a> {
    path: &'a str,
    body: serde_json::Value,
}

#[derive(Debug, Clone)]
struct AntigravityStatusSnapshot {
    model_quotas: Vec<AntigravityModelQuota>,
    account_email: Option<String>,
    account_plan: Option<String>,
}

#[derive(Debug, Clone)]
struct AntigravityModelQuota {
    label: String,
    model_id: String,
    remaining_fraction: Option<f64>,
    reset_time: Option<String>,
}

#[derive(thiserror::Error, Debug)]
enum AntigravityError {
    #[error("Antigravity language server not detected. Launch Antigravity and retry.")]
    NotRunning,
    #[error("Antigravity CSRF token not found. Restart Antigravity and retry.")]
    MissingCsrfToken,
    #[error("Antigravity port detection failed: {0}")]
    PortDetection(String),
    #[error("Antigravity API error: {0}")]
    Api(String),
    #[error("Antigravity parse error: {0}")]
    Parse(String),
    #[error("Antigravity process error: {0}")]
    Process(String),
}

fn default_request_body() -> serde_json::Value {
    serde_json::json!({
        "metadata": {
            "ideName": "antigravity",
            "extensionName": "antigravity",
            "ideVersion": "unknown",
            "locale": "en"
        }
    })
}

fn unleash_request_body() -> serde_json::Value {
    serde_json::json!({
        "context": {
            "properties": {
                "devMode": "false",
                "extensionVersion": "unknown",
                "hasAnthropicModelAccess": "true",
                "ide": "antigravity",
                "ideVersion": "unknown",
                "installationId": "incubar",
                "language": "UNSPECIFIED",
                "os": "macos",
                "requestedModelId": "MODEL_UNSPECIFIED"
            }
        }
    })
}

fn select_models(models: &[AntigravityModelQuota]) -> Vec<AntigravityModelQuota> {
    let mut ordered = Vec::new();
    if let Some(claude) = models
        .iter()
        .find(|model| is_claude_without_thinking(&model.label))
    {
        ordered.push(claude.clone());
    }
    if let Some(pro) = models.iter().find(|model| is_gemini_pro_low(&model.label)) {
        if !ordered.iter().any(|model| model.label == pro.label) {
            ordered.push(pro.clone());
        }
    }
    if let Some(flash) = models.iter().find(|model| is_gemini_flash(&model.label)) {
        if !ordered.iter().any(|model| model.label == flash.label) {
            ordered.push(flash.clone());
        }
    }
    if ordered.is_empty() {
        let mut sorted = models.to_vec();
        sorted.sort_by(|a, b| {
            remaining_percent(a)
                .partial_cmp(&remaining_percent(b))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        ordered.extend(sorted);
    }
    ordered
}

fn remaining_percent(quota: &AntigravityModelQuota) -> f64 {
    quota.remaining_fraction.unwrap_or(0.0).clamp(0.0, 1.0) * 100.0
}

fn to_rate_window(quota: &AntigravityModelQuota) -> RateWindow {
    RateWindow {
        used_percent: (100.0 - remaining_percent(quota)).clamp(0.0, 100.0),
        window_minutes: None,
        resets_at: quota.reset_time.clone(),
        reset_description: None,
        label: Some(quota.label.clone()),
    }
}

fn is_claude_without_thinking(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("claude") && !lower.contains("thinking")
}

fn is_gemini_pro_low(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("pro") && lower.contains("low")
}

fn is_gemini_flash(label: &str) -> bool {
    let lower = label.to_lowercase();
    lower.contains("gemini") && lower.contains("flash")
}

fn is_antigravity_command_line(command: &str) -> bool {
    command.contains("--app_data_dir") && command.contains("antigravity")
        || command.contains("/antigravity/")
        || command.contains("\\antigravity\\")
}

fn extract_flag(flag: &str, command: &str) -> Option<String> {
    let pattern = format!("{}[=\\s]+([^\\s]+)", regex::escape(flag));
    let regex = Regex::new(&pattern).ok()?;
    let captures = regex.captures(command)?;
    captures.get(1).map(|m| m.as_str().to_string())
}

fn parse_listening_ports(output: &str) -> Vec<i32> {
    let regex = Regex::new(r":(\d+)\s+\(LISTEN\)").ok();
    let mut ports = std::collections::HashSet::new();
    if let Some(regex) = regex {
        for capture in regex.captures_iter(output) {
            if let Some(port) = capture.get(1).and_then(|m| m.as_str().parse::<i32>().ok()) {
                ports.insert(port);
            }
        }
    }
    let mut ports: Vec<i32> = ports.into_iter().collect();
    ports.sort();
    ports
}

fn quota_from_config(config: ModelConfig) -> Option<AntigravityModelQuota> {
    let quota = config.quota_info?;
    Some(AntigravityModelQuota {
        label: config.label,
        model_id: config.model_or_alias.model,
        remaining_fraction: quota.remaining_fraction,
        reset_time: quota.reset_time,
    })
}

fn parse_google_workspace_status(
    data: &[u8],
    product_id: &str,
) -> Result<ProviderStatus, AntigravityError> {
    let mut incident_list: Vec<GoogleWorkspaceIncident> =
        serde_json::from_slice(data).map_err(|err| AntigravityError::Parse(err.to_string()))?;

    incident_list.retain(|incident| incident.is_active());
    let relevant: Vec<GoogleWorkspaceIncident> = incident_list
        .into_iter()
        .filter(|incident| incident.is_relevant(product_id))
        .collect();

    if relevant.is_empty() {
        return Ok(ProviderStatus::none());
    }

    let mut best = None;
    for incident in relevant {
        let update = incident.most_recent_update.clone().or_else(|| {
            incident
                .updates
                .as_ref()
                .and_then(|updates| updates.last().cloned())
        });
        let indicator = workspace_indicator(
            update.as_ref().and_then(|value| value.status.as_deref()),
            incident.severity.as_deref(),
            incident.status_impact.as_deref(),
        );
        let candidate = (indicator, incident, update);
        match &best {
            Some((current_indicator, _, _)) => {
                if indicator_rank(&candidate.0) > indicator_rank(current_indicator) {
                    best = Some(candidate);
                }
            }
            None => best = Some(candidate),
        }
    }

    let (indicator, incident, update) = best.expect("best incident");
    let description = workspace_summary(
        update
            .as_ref()
            .and_then(|value| value.text.as_deref())
            .or(incident.external_desc.as_deref()),
    );
    let updated_at = update
        .as_ref()
        .and_then(|value| value.when.clone())
        .or(incident.modified)
        .or(incident.begin);

    Ok(ProviderStatus {
        indicator,
        description,
        updated_at,
    })
}

fn indicator_rank(indicator: &StatusIndicator) -> i32 {
    match indicator {
        StatusIndicator::None => 0,
        StatusIndicator::Maintenance => 1,
        StatusIndicator::Minor => 2,
        StatusIndicator::Major => 3,
        StatusIndicator::Critical => 4,
        StatusIndicator::Unknown => 1,
    }
}

fn workspace_indicator(
    status: Option<&str>,
    severity: Option<&str>,
    impact: Option<&str>,
) -> StatusIndicator {
    match status.map(|value| value.to_uppercase()) {
        Some(value) if value == "AVAILABLE" => StatusIndicator::None,
        Some(value) if value == "SERVICE_INFORMATION" => StatusIndicator::Minor,
        Some(value) if value == "SERVICE_DISRUPTION" => StatusIndicator::Major,
        Some(value) if value == "SERVICE_OUTAGE" => StatusIndicator::Critical,
        Some(value) if value == "SERVICE_MAINTENANCE" || value == "SCHEDULED_MAINTENANCE" => {
            StatusIndicator::Maintenance
        }
        _ => match severity.map(|value| value.to_lowercase()) {
            Some(value) if value == "low" => StatusIndicator::Minor,
            Some(value) if value == "medium" => StatusIndicator::Major,
            Some(value) if value == "high" => StatusIndicator::Critical,
            _ => match impact.map(|value| value.to_uppercase()) {
                Some(value) if value == "AVAILABLE" => StatusIndicator::None,
                Some(value) if value == "SERVICE_INFORMATION" => StatusIndicator::Minor,
                Some(value) if value == "SERVICE_DISRUPTION" => StatusIndicator::Major,
                Some(value) if value == "SERVICE_OUTAGE" => StatusIndicator::Critical,
                Some(value)
                    if value == "SERVICE_MAINTENANCE" || value == "SCHEDULED_MAINTENANCE" =>
                {
                    StatusIndicator::Maintenance
                }
                _ => StatusIndicator::Minor,
            },
        },
    }
}

fn workspace_summary(text: Option<&str>) -> Option<String> {
    let text = text?;
    let normalized = text.replace("\r\n", "\n").replace("\r", "\n");
    for raw_line in normalized.split('\n') {
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let lower = trimmed.to_lowercase();
        if lower.starts_with("**summary")
            || lower.starts_with("**description")
            || lower == "summary"
        {
            continue;
        }
        let mut cleaned = trimmed.replace("**", "");
        cleaned = strip_markdown_links(&cleaned);
        if cleaned.starts_with("- ") {
            cleaned = cleaned[2..].to_string();
        }
        let cleaned = cleaned.trim();
        if !cleaned.is_empty() {
            return Some(cleaned.to_string());
        }
    }
    None
}

fn strip_markdown_links(text: &str) -> String {
    Regex::new(r"\[([^\]]+)\]\([^\)]+\)")
        .map(|regex| regex.replace_all(text, "$1").to_string())
        .unwrap_or_else(|_| text.to_string())
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct GoogleWorkspaceIncident {
    begin: Option<String>,
    end: Option<String>,
    modified: Option<String>,
    external_desc: Option<String>,
    status_impact: Option<String>,
    severity: Option<String>,
    affected_products: Option<Vec<GoogleWorkspaceProduct>>,
    currently_affected_products: Option<Vec<GoogleWorkspaceProduct>>,
    most_recent_update: Option<GoogleWorkspaceUpdate>,
    updates: Option<Vec<GoogleWorkspaceUpdate>>,
}

impl GoogleWorkspaceIncident {
    fn is_active(&self) -> bool {
        self.end.is_none()
    }

    fn is_relevant(&self, product_id: &str) -> bool {
        if let Some(current) = &self.currently_affected_products {
            return current.iter().any(|product| product.id == product_id);
        }
        self.affected_products
            .as_ref()
            .map(|products| products.iter().any(|product| product.id == product_id))
            .unwrap_or(false)
    }
}

#[derive(Debug, serde::Deserialize, Clone)]
struct GoogleWorkspaceProduct {
    title: Option<String>,
    id: String,
}

#[derive(Debug, serde::Deserialize, Clone)]
#[serde(rename_all = "snake_case")]
struct GoogleWorkspaceUpdate {
    when: Option<String>,
    status: Option<String>,
    text: Option<String>,
}

fn invalid_code(code: Option<&CodeValue>) -> Option<String> {
    let code = code?;
    if code.is_ok() {
        None
    } else {
        Some(code.raw_value())
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserStatusResponse {
    code: Option<CodeValue>,
    message: Option<String>,
    user_status: Option<UserStatus>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct CommandModelConfigResponse {
    code: Option<CodeValue>,
    message: Option<String>,
    client_model_configs: Option<Vec<ModelConfig>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct UserStatus {
    email: Option<String>,
    plan_status: Option<PlanStatus>,
    cascade_model_config_data: Option<ModelConfigData>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanStatus {
    plan_info: Option<PlanInfo>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlanInfo {
    plan_name: Option<String>,
    plan_display_name: Option<String>,
    display_name: Option<String>,
    product_name: Option<String>,
    plan_short_name: Option<String>,
}

impl PlanInfo {
    fn preferred_name(self) -> Option<String> {
        let candidates = [
            self.plan_display_name,
            self.display_name,
            self.product_name,
            self.plan_name,
            self.plan_short_name,
        ];
        for candidate in candidates {
            if let Some(value) = candidate {
                let trimmed = value.trim();
                if !trimmed.is_empty() {
                    return Some(trimmed.to_string());
                }
            }
        }
        None
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelConfigData {
    client_model_configs: Option<Vec<ModelConfig>>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelConfig {
    label: String,
    model_or_alias: ModelAlias,
    quota_info: Option<QuotaInfo>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelAlias {
    model: String,
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct QuotaInfo {
    remaining_fraction: Option<f64>,
    reset_time: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
#[serde(untagged)]
enum CodeValue {
    Int(i64),
    String(String),
}

impl CodeValue {
    fn is_ok(&self) -> bool {
        match self {
            CodeValue::Int(value) => *value == 0,
            CodeValue::String(value) => {
                let lower = value.to_lowercase();
                lower == "ok" || lower == "success" || value == "0"
            }
        }
    }

    fn raw_value(&self) -> String {
        match self {
            CodeValue::Int(value) => value.to_string(),
            CodeValue::String(value) => value.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_user_status_response() {
        let json = r#"
        {
          "code": 0,
          "userStatus": {
            "email": "test@example.com",
            "planStatus": {
              "planInfo": {
                "planName": "Pro"
              }
            },
            "cascadeModelConfigData": {
              "clientModelConfigs": [
                {
                  "label": "Claude 3.5 Sonnet",
                  "modelOrAlias": { "model": "claude-3-5-sonnet" },
                  "quotaInfo": { "remainingFraction": 0.5, "resetTime": "2025-12-24T10:00:00Z" }
                },
                {
                  "label": "Gemini Pro Low",
                  "modelOrAlias": { "model": "gemini-pro-low" },
                  "quotaInfo": { "remainingFraction": 0.8, "resetTime": "2025-12-24T11:00:00Z" }
                },
                {
                  "label": "Gemini Flash",
                  "modelOrAlias": { "model": "gemini-flash" },
                  "quotaInfo": { "remainingFraction": 0.2, "resetTime": "2025-12-24T12:00:00Z" }
                }
              ]
            }
          }
        }
        "#;

        let snapshot =
            AntigravityProvider::parse_user_status_response(json.as_bytes()).expect("snapshot");
        assert_eq!(snapshot.account_email.as_deref(), Some("test@example.com"));
        assert_eq!(snapshot.account_plan.as_deref(), Some("Pro"));
        assert_eq!(snapshot.model_quotas.len(), 3);

        let provider = AntigravityProvider::new();
        let usage = provider.build_usage_snapshot(snapshot).expect("usage");
        let primary = usage.primary.expect("primary");
        let secondary = usage.secondary.expect("secondary");
        let tertiary = usage.tertiary.expect("tertiary");

        assert!((primary.used_percent - 50.0).abs() < 0.1);
        assert!((secondary.used_percent - 20.0).abs() < 0.1);
        assert!((tertiary.used_percent - 80.0).abs() < 0.1);
        assert_eq!(primary.label.as_deref(), Some("Claude"));
        assert_eq!(secondary.label.as_deref(), Some("Gemini Pro"));
        assert_eq!(tertiary.label.as_deref(), Some("Gemini Flash"));
    }

    #[test]
    fn parses_command_model_response() {
        let json = r#"
        {
          "code": "ok",
          "clientModelConfigs": [
            {
              "label": "Gemini Flash",
              "modelOrAlias": { "model": "gemini-flash" },
              "quotaInfo": { "remainingFraction": 0.25 }
            }
          ]
        }
        "#;

        let snapshot =
            AntigravityProvider::parse_command_model_response(json.as_bytes()).expect("snapshot");
        assert_eq!(snapshot.model_quotas.len(), 1);
        assert_eq!(snapshot.model_quotas[0].label, "Gemini Flash");
    }

    #[test]
    fn orders_models_with_fallback() {
        let models = vec![
            AntigravityModelQuota {
                label: "Other".to_string(),
                model_id: "other".to_string(),
                remaining_fraction: Some(0.2),
                reset_time: None,
            },
            AntigravityModelQuota {
                label: "Claude Opus".to_string(),
                model_id: "claude-opus".to_string(),
                remaining_fraction: Some(0.9),
                reset_time: None,
            },
        ];

        let ordered = select_models(&models);
        assert_eq!(ordered.len(), 1);
        assert_eq!(ordered[0].label, "Claude Opus");
    }

    #[test]
    fn parses_google_workspace_incident() {
        let data = br#"
        [
          {
            "id": "inc-1",
            "begin": "2025-12-02T09:00:00+00:00",
            "end": null,
            "affected_products": [
              {"title": "Gemini", "id": "npdyhgECDJ6tB66MxXyo"}
            ],
            "most_recent_update": {
              "when": "2025-12-02T10:00:00+00:00",
              "status": "SERVICE_OUTAGE",
              "text": "**Summary**\nGemini API error.\n"
            }
          }
        ]
        "#;

        let status = parse_google_workspace_status(data, "npdyhgECDJ6tB66MxXyo").expect("status");
        assert_eq!(status.indicator, StatusIndicator::Critical);
        assert_eq!(status.description.as_deref(), Some("Gemini API error."));
    }
}
