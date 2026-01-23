//! Kiro provider implementation
//!
//! Uses the Kiro CLI status probe to read usage data.

use super::{ProviderFetcher, ProviderIdentity, ProviderStatus, RateWindow, StatusIndicator, UsageSnapshot};
use async_trait::async_trait;
use chrono::Datelike;
use regex::Regex;
use tokio::io::AsyncReadExt;

const CLI_NAME: &str = "kiro-cli";
const STATUS_FEED_URL: &str = "https://status.aws.amazon.com/rss/all.rss";
const DEFAULT_TIMEOUT_SECS: u64 = 20;
const DEFAULT_IDLE_TIMEOUT_SECS: u64 = 10;

pub struct KiroProvider {
    client: reqwest::Client,
    timeout: std::time::Duration,
    idle_timeout: std::time::Duration,
}

impl KiroProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            client,
            timeout: std::time::Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            idle_timeout: std::time::Duration::from_secs(DEFAULT_IDLE_TIMEOUT_SECS),
        }
    }

    fn parse_output(&self, output: &str) -> Result<KiroUsageSnapshot, KiroError> {
        let stripped = strip_ansi(output);
        let trimmed = stripped.trim();
        if trimmed.is_empty() {
            return Err(KiroError::Parse("Empty output from kiro-cli".to_string()));
        }

        let lowered = stripped.to_lowercase();
        if lowered.contains("could not retrieve usage information") {
            return Err(KiroError::Parse(
                "Kiro CLI could not retrieve usage information".to_string(),
            ));
        }
        if is_not_logged_in(&lowered) {
            return Err(KiroError::NotLoggedIn);
        }

        let plan_name = parse_plan_name(&stripped).unwrap_or_else(|| "Kiro".to_string());
        let resets_at = parse_reset_date(&stripped);

        let (mut credits_percent, percent_matched) = parse_percent(&stripped);
        let (mut credits_used, mut credits_total, credits_matched) = parse_credits(&stripped);
        if !percent_matched && credits_matched && credits_total > 0.0 {
            credits_percent = (credits_used / credits_total) * 100.0;
        }

        let (bonus_used, bonus_total) = parse_bonus_credits(&stripped);
        let bonus_expiry_days = parse_bonus_expiry_days(&stripped);

        if !percent_matched && !credits_matched {
            return Err(KiroError::Parse(
                "No recognizable usage patterns found. Kiro CLI output format may have changed."
                    .to_string(),
            ));
        }

        if !credits_matched {
            credits_used = 0.0;
            credits_total = 50.0;
        }

        Ok(KiroUsageSnapshot {
            plan_name,
            credits_used,
            credits_total,
            credits_percent,
            bonus_credits_used: bonus_used,
            bonus_credits_total: bonus_total,
            bonus_expiry_days,
            resets_at,
            updated_at: chrono::Utc::now(),
        })
    }

    async fn ensure_logged_in(&self) -> Result<(), KiroError> {
        let result = run_command(
            &["whoami"],
            std::time::Duration::from_secs(5),
            std::time::Duration::from_secs(2),
        )
        .await?;
        validate_whoami_output(&result.stdout, &result.stderr, result.status)?;
        Ok(())
    }

    async fn run_usage_command(&self) -> Result<String, KiroError> {
        let result = run_command(
            &["chat", "--no-interactive", "/usage"],
            self.timeout,
            self.idle_timeout,
        )
        .await?;

        let trimmed_stdout = result.stdout.trim();
        let trimmed_stderr = result.stderr.trim();
        let combined = if trimmed_stderr.is_empty() {
            trimmed_stdout
        } else {
            trimmed_stderr
        };
        let lowered = strip_ansi(combined).to_lowercase();

        if is_not_logged_in(&lowered) {
            return Err(KiroError::NotLoggedIn);
        }

        if result.terminated_for_idle && !usage_output_complete(combined) {
            return Err(KiroError::Timeout);
        }

        if !trimmed_stdout.is_empty() {
            return Ok(result.stdout);
        }

        if !trimmed_stderr.is_empty() {
            return Ok(result.stderr);
        }

        if result.status != 0 {
            let message = if combined.is_empty() {
                format!("Kiro CLI failed with status {}.", result.status)
            } else {
                combined.to_string()
            };
            return Err(KiroError::CliFailed(message));
        }

        Ok(result.stdout)
    }

    async fn fetch_status_snapshot(&self) -> Result<ProviderStatus, KiroError> {
        let response = self
            .client
            .get(STATUS_FEED_URL)
            .header("Accept", "application/rss+xml, text/xml;q=0.9, */*;q=0.8")
            .send()
            .await
            .map_err(|err| KiroError::Status(err.to_string()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|err| KiroError::Status(err.to_string()))?;
        if !status.is_success() {
            return Err(KiroError::Status(format!("HTTP {}", status)));
        }
        parse_aws_status_feed(&body)
    }
}

#[async_trait]
impl ProviderFetcher for KiroProvider {
    fn name(&self) -> &'static str {
        "Kiro"
    }

    fn description(&self) -> &'static str {
        "Kiro status probe"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Kiro usage via CLI");
        self.ensure_logged_in()
            .await
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let output = self
            .run_usage_command()
            .await
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        let snapshot = self
            .parse_output(&output)
            .map_err(|err| anyhow::anyhow!(err.to_string()))?;
        Ok(snapshot.to_usage_snapshot())
    }

    async fn fetch_status(&self) -> Result<ProviderStatus, anyhow::Error> {
        self.fetch_status_snapshot()
            .await
            .map_err(|err| anyhow::anyhow!(err.to_string()))
    }
}

#[derive(Debug, Clone)]
struct KiroUsageSnapshot {
    plan_name: String,
    credits_used: f64,
    credits_total: f64,
    credits_percent: f64,
    bonus_credits_used: Option<f64>,
    bonus_credits_total: Option<f64>,
    bonus_expiry_days: Option<i64>,
    resets_at: Option<String>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl KiroUsageSnapshot {
    fn to_usage_snapshot(&self) -> UsageSnapshot {
        let primary = RateWindow {
            used_percent: self.credits_percent.clamp(0.0, 100.0),
            window_minutes: None,
            resets_at: self.resets_at.clone(),
            reset_description: None,
            label: Some("Credits".to_string()),
        };

        let secondary = match (self.bonus_credits_used, self.bonus_credits_total) {
            (Some(used), Some(total)) if total > 0.0 => {
                let bonus_percent = ((used / total) * 100.0).clamp(0.0, 100.0);
                let expires_at = self
                    .bonus_expiry_days
                    .and_then(|days| {
                        chrono::Utc::now().checked_add_signed(chrono::Duration::days(days))
                    })
                    .map(|value| value.to_rfc3339());
                Some(RateWindow {
                    used_percent: bonus_percent,
                    window_minutes: None,
                    resets_at: expires_at,
                    reset_description: self
                        .bonus_expiry_days
                        .map(|days| format!("expires in {}d", days)),
                    label: Some("Bonus".to_string()),
                })
            }
            _ => None,
        };

        UsageSnapshot {
            primary: Some(primary),
            secondary,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: Some(self.plan_name.clone()),
                organization: Some(self.plan_name.clone()),
            }),
            updated_at: self.updated_at.to_rfc3339(),
            error: None,
        }
    }
}

#[derive(Debug, Clone)]
struct CommandResult {
    stdout: String,
    stderr: String,
    status: i32,
    terminated_for_idle: bool,
}

#[derive(thiserror::Error, Debug)]
enum KiroError {
    #[error("Kiro CLI not found. Install it from https://kiro.dev")]
    CliNotFound,
    #[error("Not logged in to Kiro. Run 'kiro-cli login' first.")]
    NotLoggedIn,
    #[error("Kiro CLI failed: {0}")]
    CliFailed(String),
    #[error("Failed to parse Kiro usage: {0}")]
    Parse(String),
    #[error("Kiro CLI timed out.")]
    Timeout,
    #[error("Kiro status probe failed: {0}")]
    Status(String),
}

fn is_not_logged_in(lowered: &str) -> bool {
    lowered.contains("not logged in")
        || lowered.contains("login required")
        || lowered.contains("failed to initialize auth portal")
        || lowered.contains("kiro-cli login")
        || lowered.contains("oauth error")
}

fn usage_output_complete(output: &str) -> bool {
    let lowered = strip_ansi(output).to_lowercase();
    lowered.contains("covered in plan")
        || lowered.contains("resets on")
        || lowered.contains("bonus credits")
}

fn validate_whoami_output(stdout: &str, stderr: &str, status: i32) -> Result<(), KiroError> {
    let trimmed_stdout = stdout.trim();
    let trimmed_stderr = stderr.trim();
    let combined = if trimmed_stderr.is_empty() {
        trimmed_stdout
    } else {
        trimmed_stderr
    };
    let lowered = combined.to_lowercase();

    if lowered.contains("not logged in") || lowered.contains("login required") {
        return Err(KiroError::NotLoggedIn);
    }

    if status != 0 {
        let message = if combined.is_empty() {
            format!("Kiro CLI failed with status {}.", status)
        } else {
            combined.to_string()
        };
        return Err(KiroError::CliFailed(message));
    }

    if combined.is_empty() {
        return Err(KiroError::CliFailed(
            "Kiro CLI whoami returned no output.".to_string(),
        ));
    }

    Ok(())
}

fn parse_plan_name(text: &str) -> Option<String> {
    let regex = Regex::new(r"\|\s*(KIRO\s+\w+)").ok()?;
    regex
        .captures(text)
        .and_then(|cap| cap.get(1).map(|m| m.as_str().trim().to_string()))
}

fn parse_reset_date(text: &str) -> Option<String> {
    let regex = Regex::new(r"resets on (\d{2}/\d{2})").ok()?;
    let capture = regex.captures(text)?;
    let date_str = capture.get(1)?.as_str();
    parse_reset_date_string(date_str)
}

fn parse_reset_date_string(date_str: &str) -> Option<String> {
    let parts: Vec<&str> = date_str.split('/').collect();
    if parts.len() != 2 {
        return None;
    }
    let month: u32 = parts[0].parse().ok()?;
    let day: u32 = parts[1].parse().ok()?;
    let now = chrono::Utc::now();
    let year = now.year();
    let candidate = chrono::NaiveDate::from_ymd_opt(year, month, day)?
        .and_hms_opt(0, 0, 0)?
        .and_utc();
    let reset = if candidate > now {
        candidate
    } else {
        chrono::NaiveDate::from_ymd_opt(year + 1, month, day)?
            .and_hms_opt(0, 0, 0)?
            .and_utc()
    };
    Some(reset.to_rfc3339())
}

fn parse_percent(text: &str) -> (f64, bool) {
    let regex = Regex::new(r"█+\s*(\d+)%").ok();
    if let Some(regex) = regex {
        if let Some(cap) = regex.captures(text) {
            if let Some(value) = cap.get(1) {
                if let Ok(percent) = value.as_str().parse::<f64>() {
                    return (percent, true);
                }
            }
        }
    }
    (0.0, false)
}

fn parse_credits(text: &str) -> (f64, f64, bool) {
    let regex = Regex::new(r"\((\d+\.?\d*)\s+of\s+(\d+)\s+covered").ok();
    if let Some(regex) = regex {
        if let Some(cap) = regex.captures(text) {
            let used = cap
                .get(1)
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(0.0);
            let total = cap
                .get(2)
                .and_then(|m| m.as_str().parse::<f64>().ok())
                .unwrap_or(50.0);
            return (used, total, true);
        }
    }
    (0.0, 50.0, false)
}

fn parse_bonus_credits(text: &str) -> (Option<f64>, Option<f64>) {
    let regex = match Regex::new(r"Bonus credits:\s*(\d+\.?\d*)/(\d+)") {
        Ok(value) => value,
        Err(_) => return (None, None),
    };
    let caps = match regex.captures(text) {
        Some(value) => value,
        None => return (None, None),
    };
    let used = caps.get(1).and_then(|m| m.as_str().parse::<f64>().ok());
    let total = caps.get(2).and_then(|m| m.as_str().parse::<f64>().ok());
    (used, total)
}

fn parse_bonus_expiry_days(text: &str) -> Option<i64> {
    let regex = match Regex::new(r"expires in (\d+) days?") {
        Ok(value) => value,
        Err(_) => return None,
    };
    let caps = match regex.captures(text) {
        Some(value) => value,
        None => return None,
    };
    caps.get(1)?.as_str().parse::<i64>().ok()
}

fn strip_ansi(text: &str) -> String {
    let regex = Regex::new(r"\x1B\[[0-9;?]*[A-Za-z]|\x1B\].*?\x07").ok();
    if let Some(regex) = regex {
        regex.replace_all(text, "").to_string()
    } else {
        text.to_string()
    }
}

fn parse_aws_status_feed(feed: &str) -> Result<ProviderStatus, KiroError> {
    let item_regex =
        Regex::new(r"(?s)<item>(.*?)</item>").map_err(|err| KiroError::Status(err.to_string()))?;
    let item = match item_regex.captures(feed).and_then(|cap| cap.get(1)) {
        Some(value) => value.as_str(),
        None => {
            return Ok(ProviderStatus::none());
        }
    };

    let title = extract_tag_value(item, "title");
    let description = extract_tag_value(item, "description").and_then(|value| html_summary(&value));
    let pub_date = extract_tag_value(item, "pubDate").and_then(|value| parse_pub_date(&value));

    let indicator = aws_indicator(title.as_deref().unwrap_or(""));
    Ok(ProviderStatus {
        indicator,
        description,
        updated_at: pub_date,
    })
}

fn extract_tag_value(text: &str, tag: &str) -> Option<String> {
    let pattern = format!(r"(?s)<{tag}>(.*?)</{tag}>");
    let regex = Regex::new(&pattern).ok()?;
    let capture = regex.captures(text)?;
    let raw = capture.get(1)?.as_str().trim();
    let cleaned = raw
        .trim_start_matches("<![CDATA[")
        .trim_end_matches("]]>")
        .trim();
    if cleaned.is_empty() {
        None
    } else {
        Some(cleaned.to_string())
    }
}

fn html_summary(text: &str) -> Option<String> {
    let normalized = text
        .replace("<br />", "\n")
        .replace("<br/>", "\n")
        .replace("<br>", "\n");
    let tag_regex = Regex::new(r"<[^>]+>").ok();
    let stripped = if let Some(regex) = tag_regex {
        regex.replace_all(&normalized, "").to_string()
    } else {
        normalized
    };
    for line in stripped.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        return Some(trimmed.to_string());
    }
    None
}

fn parse_pub_date(value: &str) -> Option<String> {
    chrono::DateTime::parse_from_rfc2822(value)
        .ok()
        .map(|dt| dt.with_timezone(&chrono::Utc).to_rfc3339())
}

fn aws_indicator(text: &str) -> StatusIndicator {
    let lower = text.to_lowercase();
    if lower.contains("service outage") || lower.contains("service disruption") {
        return if lower.contains("outage") {
            StatusIndicator::Critical
        } else {
            StatusIndicator::Major
        };
    }
    if lower.contains("performance") || lower.contains("informational") {
        return StatusIndicator::Minor;
    }
    if lower.contains("maintenance") || lower.contains("scheduled") {
        return StatusIndicator::Maintenance;
    }
    if lower.contains("resolved") || lower.contains("operating normally") {
        return StatusIndicator::None;
    }
    StatusIndicator::Unknown
}

async fn run_command(
    arguments: &[&str],
    timeout: std::time::Duration,
    idle_timeout: std::time::Duration,
) -> Result<CommandResult, KiroError> {
    let binary = which_cli().ok_or(KiroError::CliNotFound)?;
    let mut command = tokio::process::Command::new(binary);
    command.args(arguments);
    command.stdin(std::process::Stdio::null());
    command.stdout(std::process::Stdio::piped());
    command.stderr(std::process::Stdio::piped());
    command.env("TERM", "xterm-256color");

    let mut child = command
        .spawn()
        .map_err(|err| KiroError::CliFailed(err.to_string()))?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| KiroError::CliFailed("Missing stdout".to_string()))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| KiroError::CliFailed("Missing stderr".to_string()))?;

    let (stdout_sender, mut stdout_receiver) = tokio::sync::mpsc::unbounded_channel();
    let (stderr_sender, mut stderr_receiver) = tokio::sync::mpsc::unbounded_channel();

    let stdout_task = tokio::spawn(async move { read_stream(stdout, stdout_sender).await });

    let stderr_task = tokio::spawn(async move { read_stream(stderr, stderr_sender).await });

    let start = std::time::Instant::now();
    let mut last_activity = start;
    let mut saw_activity = false;
    let mut terminated_for_idle = false;

    loop {
        if start.elapsed() >= timeout {
            let _ = child.kill().await;
            return Err(KiroError::Timeout);
        }

        let status = child
            .try_wait()
            .map_err(|err| KiroError::CliFailed(err.to_string()))?;
        if let Some(status) = status {
            let stdout_bytes = stdout_task.await.unwrap_or_default();
            let stderr_bytes = stderr_task.await.unwrap_or_default();
            return Ok(CommandResult {
                stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                status: status.code().unwrap_or(-1),
                terminated_for_idle,
            });
        }

        let mut had_activity = false;
        while stdout_receiver.try_recv().is_ok() {
            had_activity = true;
        }
        while stderr_receiver.try_recv().is_ok() {
            had_activity = true;
        }
        if had_activity {
            last_activity = std::time::Instant::now();
            saw_activity = true;
        }

        if last_activity.elapsed() >= idle_timeout && saw_activity {
            terminated_for_idle = true;
            let _ = child.kill().await;
            let _ = child.wait().await;
            let stdout_bytes = stdout_task.await.unwrap_or_default();
            let stderr_bytes = stderr_task.await.unwrap_or_default();
            return Ok(CommandResult {
                stdout: String::from_utf8_lossy(&stdout_bytes).to_string(),
                stderr: String::from_utf8_lossy(&stderr_bytes).to_string(),
                status: -1,
                terminated_for_idle,
            });
        }

        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
}

async fn read_stream<S>(mut stream: S, sender: tokio::sync::mpsc::UnboundedSender<()>) -> Vec<u8>
where
    S: tokio::io::AsyncRead + Unpin,
{
    let mut buffer = Vec::new();
    let mut temp = [0u8; 1024];
    loop {
        match stream.read(&mut temp).await {
            Ok(0) => break,
            Ok(n) => {
                buffer.extend_from_slice(&temp[..n]);
                let _ = sender.send(());
            }
            Err(_) => break,
        }
    }
    buffer
}

fn which_cli() -> Option<String> {
    if let Ok(output) = std::process::Command::new("which").arg(CLI_NAME).output() {
        if output.status.success() {
            let value = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !value.is_empty() {
                return Some(value);
            }
        }
    }

    let candidates = [
        "/usr/local/bin/kiro-cli",
        "/opt/homebrew/bin/kiro-cli",
        &format!(
            "{}/.local/bin/kiro-cli",
            std::env::var("HOME").unwrap_or_default()
        ),
    ];
    candidates
        .iter()
        .map(|path| path.to_string())
        .find(|path| std::path::Path::new(path).exists())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_usage_output() {
        let provider = KiroProvider::new();
        let output = r#"
        | KIRO FREE                                          |
        ████████████████████████████████████████████████████ 25%
        (12.50 of 50 covered in plan), resets on 01/15
        "#;

        let snapshot = provider.parse_output(output).expect("snapshot");
        assert_eq!(snapshot.plan_name, "KIRO FREE");
        assert_eq!(snapshot.credits_percent, 25.0);
        assert_eq!(snapshot.credits_used, 12.50);
        assert_eq!(snapshot.credits_total, 50.0);
        assert!(snapshot.bonus_credits_used.is_none());
        assert!(snapshot.resets_at.is_some());
    }

    #[test]
    fn parses_output_with_bonus_credits() {
        let provider = KiroProvider::new();
        let output = r#"
        | KIRO PRO                                           |
        ████████████████████████████████████████████████████ 80%
        (40.00 of 50 covered in plan), resets on 02/01
        Bonus credits: 5.00/10 credits used, expires in 7 days
        "#;

        let snapshot = provider.parse_output(output).expect("snapshot");
        assert_eq!(snapshot.plan_name, "KIRO PRO");
        assert_eq!(snapshot.credits_percent, 80.0);
        assert_eq!(snapshot.credits_used, 40.0);
        assert_eq!(snapshot.credits_total, 50.0);
        assert_eq!(snapshot.bonus_credits_used, Some(5.0));
        assert_eq!(snapshot.bonus_credits_total, Some(10.0));
        assert_eq!(snapshot.bonus_expiry_days, Some(7));
    }

    #[test]
    fn parses_output_without_percent() {
        let provider = KiroProvider::new();
        let output = r#"
        | KIRO FREE                                          |
        (12.50 of 50 covered in plan), resets on 01/15
        "#;

        let snapshot = provider.parse_output(output).expect("snapshot");
        assert_eq!(snapshot.credits_percent, 25.0);
    }

    #[test]
    fn parses_output_with_ansi_codes() {
        let provider = KiroProvider::new();
        let output =
            "\u{001B}[32m| KIRO FREE                                          |\u{001B}[0m\n\
            \u{001B}[38;5;11m████████████████████████████████████████████████████\u{001B}[0m 50%\n\
            (25.00 of 50 covered in plan), resets on 03/15";

        let snapshot = provider.parse_output(output).expect("snapshot");
        assert_eq!(snapshot.plan_name, "KIRO FREE");
        assert_eq!(snapshot.credits_percent, 50.0);
        assert_eq!(snapshot.credits_used, 25.0);
        assert_eq!(snapshot.credits_total, 50.0);
    }

    #[test]
    fn rejects_output_missing_usage() {
        let provider = KiroProvider::new();
        let output = "| KIRO FREE |";
        let result = provider.parse_output(output);
        assert!(matches!(result, Err(KiroError::Parse(_))));
    }

    #[test]
    fn builds_usage_snapshot_with_bonus() {
        let snapshot = KiroUsageSnapshot {
            plan_name: "KIRO PRO".to_string(),
            credits_used: 25.0,
            credits_total: 100.0,
            credits_percent: 25.0,
            bonus_credits_used: Some(5.0),
            bonus_credits_total: Some(20.0),
            bonus_expiry_days: Some(14),
            resets_at: None,
            updated_at: chrono::Utc::now(),
        };

        let usage = snapshot.to_usage_snapshot();
        assert_eq!(usage.primary.as_ref().unwrap().used_percent, 25.0);
        assert_eq!(usage.secondary.as_ref().unwrap().used_percent, 25.0);
        assert_eq!(
            usage.identity.as_ref().and_then(|id| id.plan.as_deref()),
            Some("KIRO PRO")
        );
    }

    #[test]
    fn whoami_not_logged_in() {
        let result = validate_whoami_output("Not logged in", "", 1);
        assert!(matches!(result, Err(KiroError::NotLoggedIn)));
    }

    #[test]
    fn whoami_success() {
        let result = validate_whoami_output("user@example.com", "", 0);
        assert!(result.is_ok());
    }

    #[test]
    fn parses_status_feed_with_incident() {
        let feed = r#"
        <rss version="2.0">
          <channel>
            <item>
              <title><![CDATA[Service Disruption - Example Service]]></title>
              <description><![CDATA[We are investigating elevated errors.]]></description>
              <pubDate>Thu, 22 Jan 2026 14:27:31 PST</pubDate>
            </item>
          </channel>
        </rss>
        "#;

        let status = parse_aws_status_feed(feed).expect("status");
        assert_eq!(status.indicator, StatusIndicator::Major);
        assert_eq!(
            status.description.as_deref(),
            Some("We are investigating elevated errors.")
        );
        assert!(status.updated_at.is_some());
    }

    #[test]
    fn parses_status_feed_without_items() {
        let feed = r#"<rss version="2.0"><channel></channel></rss>"#;
        let status = parse_aws_status_feed(feed).expect("status");
        assert_eq!(status.indicator, StatusIndicator::None);
        assert!(status.description.is_none());
    }
}
