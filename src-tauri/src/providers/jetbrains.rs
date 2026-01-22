//! JetBrains AI provider implementation
//!
//! Reads local IDE log files to extract monthly AI credits usage.

use async_trait::async_trait;
use chrono::Datelike;
use glob::glob;
use std::path::{Path, PathBuf};

use super::{Credits, ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};

const LOG_FILENAME_PREFIX: &str = "idea.log";

#[derive(Debug, Clone, Copy)]
struct CreditsUsage {
    remaining: f64,
    total: Option<f64>,
}

pub struct JetbrainsProvider;

impl JetbrainsProvider {
    pub fn new() -> Self {
        Self
    }

    async fn fetch_from_logs(&self) -> Result<CreditsUsage, anyhow::Error> {
        let log_paths = self.collect_log_paths()?;

        if log_paths.is_empty() {
            return Err(anyhow::anyhow!(
                "No JetBrains IDE logs found. Open a JetBrains IDE with AI Assistant enabled."
            ));
        }

        let mut sorted_logs = log_paths;
        sorted_logs.sort_by_key(|path| {
            std::fs::metadata(path)
                .and_then(|meta| meta.modified())
                .ok()
        });
        sorted_logs.reverse();

        for log_path in sorted_logs {
            if let Ok(content) = tokio::fs::read_to_string(&log_path).await {
                if let Some(usage) = self.parse_usage_from_log(&content) {
                    return Ok(usage);
                }
            }
        }

        Err(anyhow::anyhow!(
            "No AI credits usage found in JetBrains logs"
        ))
    }

    fn collect_log_paths(&self) -> Result<Vec<PathBuf>, anyhow::Error> {
        let mut log_paths = Vec::new();

        if let Ok(path) = std::env::var("JETBRAINS_IDE_LOG_PATH") {
            let custom_path = PathBuf::from(path);
            if custom_path.is_file() {
                log_paths.push(custom_path);
                return Ok(log_paths);
            }
            if custom_path.is_dir() {
                log_paths.extend(self.scan_log_directory(&custom_path)?);
                return Ok(log_paths);
            }
        }

        let base_paths = self.base_paths()?;
        for base in base_paths {
            if base.is_dir() {
                log_paths.extend(self.scan_log_directory(&base)?);
            }
        }

        Ok(log_paths)
    }

    fn base_paths(&self) -> Result<Vec<PathBuf>, anyhow::Error> {
        if let Ok(path) = std::env::var("JETBRAINS_IDE_BASE_PATH") {
            let separator = if cfg!(target_os = "windows") { ';' } else { ':' };
            let paths = path
                .split(separator)
                .filter(|entry| !entry.trim().is_empty())
                .map(|entry| PathBuf::from(entry.trim()))
                .collect::<Vec<_>>();
            if !paths.is_empty() {
                return Ok(paths);
            }
        }

        if cfg!(target_os = "macos") {
            if let Some(home) = dirs::home_dir() {
                return Ok(vec![home.join("Library/Application Support/JetBrains")]);
            }
        } else if cfg!(target_os = "windows") {
            if let Ok(appdata) = std::env::var("APPDATA") {
                return Ok(vec![PathBuf::from(appdata).join("JetBrains")]);
            }
        } else if let Some(home) = dirs::home_dir() {
            return Ok(vec![home.join(".config").join("JetBrains")]);
        }

        Ok(Vec::new())
    }

    fn scan_log_directory(&self, base: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
        let mut logs = Vec::new();
        let entries = std::fs::read_dir(base).map_err(|err| {
            anyhow::anyhow!("Failed to read JetBrains directory {:?}: {}", base, err)
        })?;

        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }

            logs.extend(self.collect_logs_in_ide_dir(&path)?);
        }

        Ok(logs)
    }

    fn collect_logs_in_ide_dir(&self, ide_dir: &Path) -> Result<Vec<PathBuf>, anyhow::Error> {
        let mut logs = Vec::new();
        for log_dir in ["log", "system/log"] {
            let pattern = ide_dir
                .join(log_dir)
                .join(format!("{}*", LOG_FILENAME_PREFIX));
            let pattern = pattern.to_string_lossy().to_string();
            for entry in glob(&pattern)? {
                if let Ok(path) = entry {
                    if path.is_file() {
                        logs.push(path);
                    }
                }
            }
        }
        Ok(logs)
    }

    fn parse_usage_from_log(&self, content: &str) -> Option<CreditsUsage> {
        for line in content.lines().rev() {
            if let Some(usage) = self.parse_credits_line(line) {
                return Some(usage);
            }
        }
        None
    }

    fn parse_credits_line(&self, line: &str) -> Option<CreditsUsage> {
        let lower = line.to_lowercase();
        if !lower.contains("credit") || (!lower.contains("ai") && !lower.contains("assistant")) {
            return None;
        }

        let numbers = extract_numbers(line);
        if numbers.is_empty() {
            return None;
        }

        let has_remaining = lower.contains("remaining") || lower.contains("left") || lower.contains("available");
        let has_used = lower.contains("used") || lower.contains("spent") || lower.contains("consumed");
        let has_total = lower.contains("total") || lower.contains("limit") || lower.contains("quota");
        let has_separator = lower.contains('/') || lower.contains("of");

        if has_remaining {
            let remaining = numbers[0];
            let total = numbers.get(1).copied();
            return Some(self.build_usage_from_remaining(remaining, total));
        }

        if has_used || has_separator {
            if numbers.len() >= 2 {
                let used = numbers[0];
                let total = numbers[1];
                return Some(self.build_usage_from_used(used, Some(total)));
            }
            if numbers.len() == 1 {
                return Some(self.build_usage_from_used(numbers[0], None));
            }
        }

        if numbers.len() >= 2 {
            return Some(self.build_usage_from_remaining(numbers[0], Some(numbers[1])));
        }

        None
    }

    fn build_usage_from_remaining(&self, remaining: f64, total: Option<f64>) -> CreditsUsage {
        CreditsUsage {
            remaining: remaining.max(0.0),
            total: total.filter(|value| *value > 0.0),
        }
    }

    fn build_usage_from_used(&self, used: f64, total: Option<f64>) -> CreditsUsage {
        let total_value = total.filter(|value| *value > 0.0);
        let remaining = total_value
            .map(|value| (value - used).max(0.0))
            .unwrap_or(0.0);
        CreditsUsage {
            remaining,
            total: total_value,
        }
    }

    fn build_snapshot(&self, usage: CreditsUsage) -> UsageSnapshot {
        let total = usage.total;
        let used_percent = total
            .filter(|value| *value > 0.0)
            .map(|value| ((value - usage.remaining) / value) * 100.0)
            .unwrap_or(0.0)
            .clamp(0.0, 100.0);

        let reset_at = self.month_end(chrono::Utc::now());
        let resets_at = Some(reset_at.to_rfc3339());
        let reset_description = Some(self.format_reset_time(&reset_at));

        UsageSnapshot {
            primary: Some(RateWindow {
                used_percent,
                window_minutes: None,
                resets_at,
                reset_description,
                label: Some("Monthly".to_string()),
            }),
            secondary: None,
            tertiary: None,
            credits: Some(Credits {
                remaining: usage.remaining,
                total,
                unit: "credits".to_string(),
            }),
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: None,
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn month_end(&self, now: chrono::DateTime<chrono::Utc>) -> chrono::DateTime<chrono::Utc> {
        let year = now.year();
        let month = now.month();
        let (next_year, next_month) = if month == 12 {
            (year + 1, 1)
        } else {
            (year, month + 1)
        };

        let first_next_month = chrono::NaiveDate::from_ymd_opt(next_year, next_month, 1)
            .unwrap_or_else(|| chrono::NaiveDate::from_ymd_opt(year, month, 1).unwrap());
        let first_next_month = first_next_month.and_hms_opt(0, 0, 0).unwrap();
        let end_of_month = first_next_month - chrono::Duration::seconds(1);
        chrono::DateTime::<chrono::Utc>::from_utc(end_of_month, chrono::Utc)
    }

    fn format_reset_time(&self, reset_date: &chrono::DateTime<chrono::Utc>) -> String {
        let now = chrono::Utc::now();
        let duration = reset_date.signed_duration_since(now);
        if duration.num_days() < 1 {
            let hours = duration.num_hours().max(1);
            format!("Resets in {}h", hours)
        } else {
            format!("Resets in {} days", duration.num_days())
        }
    }
}

fn extract_numbers(text: &str) -> Vec<f64> {
    let mut numbers = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            current.push(ch);
        } else if !current.is_empty() {
            if let Ok(value) = current.parse::<f64>() {
                numbers.push(value);
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(value) = current.parse::<f64>() {
            numbers.push(value);
        }
    }

    numbers
}

#[async_trait]
impl ProviderFetcher for JetbrainsProvider {
    fn name(&self) -> &'static str {
        "JetBrains AI"
    }

    fn description(&self) -> &'static str {
        "JetBrains AI Assistant"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching JetBrains AI usage");
        let usage = self.fetch_from_logs().await?;
        Ok(self.build_snapshot(usage))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_remaining_credits_line() {
        let provider = JetbrainsProvider::new();
        let usage = provider
            .parse_credits_line("AI credits remaining: 120 of 500")
            .expect("usage");

        assert_eq!(usage.remaining, 120.0);
        assert_eq!(usage.total, Some(500.0));
    }

    #[test]
    fn parses_used_credits_line() {
        let provider = JetbrainsProvider::new();
        let usage = provider
            .parse_credits_line("AI credits used 380 / 500")
            .expect("usage");

        assert_eq!(usage.remaining, 120.0);
        assert_eq!(usage.total, Some(500.0));
    }

    #[test]
    fn parses_slash_credits_line() {
        let provider = JetbrainsProvider::new();
        let usage = provider
            .parse_credits_line("AI Credits 380/500")
            .expect("usage");

        assert_eq!(usage.remaining, 120.0);
        assert_eq!(usage.total, Some(500.0));
    }

    #[test]
    fn parses_remaining_only_line() {
        let provider = JetbrainsProvider::new();
        let usage = provider
            .parse_credits_line("AI credits remaining 75")
            .expect("usage");

        assert_eq!(usage.remaining, 75.0);
        assert_eq!(usage.total, None);
    }

    #[test]
    fn ignores_lines_without_ai_context() {
        let provider = JetbrainsProvider::new();
        assert!(provider.parse_credits_line("Credits remaining: 100").is_none());
    }

    #[test]
    fn computes_month_end() {
        let provider = JetbrainsProvider::new();
        let date = chrono::DateTime::parse_from_rfc3339("2025-01-15T10:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let month_end = provider.month_end(date);
        assert_eq!(month_end.to_rfc3339(), "2025-01-31T23:59:59+00:00");
    }
}
