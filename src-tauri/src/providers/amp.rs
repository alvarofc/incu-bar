//!
//! Amp provider implementation
//!
//! Uses cookie-based authentication via browser cookie import.
//! Endpoint: https://ampcode.com/settings

use async_trait::async_trait;
use super::{ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};

const SETTINGS_URL: &str = "https://ampcode.com/settings";

pub struct AmpProvider {
    client: reqwest::Client,
}

impl AmpProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_with_cookies(&self, cookie_header: &str) -> Result<UsageSnapshot, AmpError> {
        let response = self.client
            .get(SETTINGS_URL)
            .header("Cookie", cookie_header)
            .header("Accept", "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8")
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Origin", "https://ampcode.com")
            .header("Referer", SETTINGS_URL)
            .header("User-Agent", "IncuBar/1.0")
            .send()
            .await
            .map_err(|err| AmpError::Api(err.to_string()))?;

        match response.status().as_u16() {
            200 => {}
            401 | 403 => return Err(AmpError::SessionExpired),
            status => return Err(AmpError::Api(format!("HTTP {}", status))),
        }

        let html = response.text().await.map_err(|err| AmpError::Api(err.to_string()))?;
        self.parse_usage_response(&html)
    }

    fn parse_usage_response(&self, html: &str) -> Result<UsageSnapshot, AmpError> {
        let usage = self.parse_free_tier_usage(html).ok_or_else(|| {
            if self.looks_signed_out(html) {
                AmpError::NotLoggedIn
            } else {
                AmpError::Parse("Missing Amp Free usage data".to_string())
            }
        })?;

        Ok(self.build_snapshot(&usage))
    }

    fn build_snapshot(&self, usage: &FreeTierUsage) -> UsageSnapshot {
        let quota = usage.quota.max(0.0);
        let used = usage.used.max(0.0);
        let used_percent = if quota > 0.0 { (used / quota) * 100.0 } else { 0.0 };

        let window_minutes = usage
            .window_hours
            .filter(|hours| *hours > 0.0)
            .map(|hours| (hours * 60.0).round() as i32);

        let resets_at = if quota > 0.0 && usage.hourly_replenishment > 0.0 {
            let hours_to_full = used / usage.hourly_replenishment;
            let seconds = (hours_to_full * 3600.0).max(0.0) as i64;
            let reset_at = chrono::Utc::now() + chrono::Duration::seconds(seconds);
            Some(reset_at.to_rfc3339())
        } else {
            None
        };

        UsageSnapshot {
            primary: Some(RateWindow {
                used_percent: used_percent.clamp(0.0, 100.0),
                window_minutes,
                resets_at,
                reset_description: None,
                label: Some("Amp Free".to_string()),
            }),
            secondary: None,
            tertiary: None,
            credits: None,
            cost: None,
            identity: Some(ProviderIdentity {
                email: None,
                name: None,
                plan: Some("Amp Free".to_string()),
                organization: None,
            }),
            updated_at: chrono::Utc::now().to_rfc3339(),
            error: None,
        }
    }

    fn parse_free_tier_usage(&self, html: &str) -> Option<FreeTierUsage> {
        for token in ["freeTierUsage", "getFreeTierUsage"] {
            if let Some(object) = self.extract_object(token, html) {
                if let Some(usage) = self.parse_free_tier_usage_object(&object) {
                    return Some(usage);
                }
            }
        }
        None
    }

    fn parse_free_tier_usage_object(&self, object: &str) -> Option<FreeTierUsage> {
        let quota = self.number_for_key("quota", object)?;
        let used = self.number_for_key("used", object)?;
        let hourly_replenishment = self.number_for_key("hourlyReplenishment", object)?;
        let window_hours = self.number_for_key("windowHours", object);

        Some(FreeTierUsage {
            quota,
            used,
            hourly_replenishment,
            window_hours,
        })
    }

    fn extract_object(&self, token: &str, text: &str) -> Option<String> {
        let token_index = text.find(token)?;
        let offset = token_index + token.len();
        let brace_offset = text[offset..].find('{')?;
        let start = offset + brace_offset;

        let mut depth = 0i32;
        let mut in_string = false;
        let mut is_escaped = false;

        for (index, ch) in text[start..].char_indices() {
            let absolute = start + index;
            if in_string {
                if is_escaped {
                    is_escaped = false;
                } else if ch == '\\' {
                    is_escaped = true;
                } else if ch == '"' {
                    in_string = false;
                }
            } else {
                if ch == '"' {
                    in_string = true;
                } else if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        let end = absolute + ch.len_utf8();
                        return Some(text[start..end].to_string());
                    }
                }
            }
        }

        None
    }

    fn number_for_key(&self, key: &str, text: &str) -> Option<f64> {
        let mut start = 0usize;
        let key_len = key.len();

        while let Some(pos) = text[start..].find(key) {
            let index = start + pos;
            if !self.is_word_boundary(text, index, key_len) {
                start = index + key_len;
                continue;
            }

            let mut cursor = index + key_len;
            while cursor < text.len() && text.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            if cursor >= text.len() || text.as_bytes()[cursor] != b':' {
                start = index + key_len;
                continue;
            }

            cursor += 1;
            while cursor < text.len() && text.as_bytes()[cursor].is_ascii_whitespace() {
                cursor += 1;
            }

            let number_start = cursor;
            let mut saw_digit = false;
            while cursor < text.len() {
                let byte = text.as_bytes()[cursor];
                if byte.is_ascii_digit() {
                    saw_digit = true;
                    cursor += 1;
                } else if byte == b'.' {
                    cursor += 1;
                } else {
                    break;
                }
            }

            if saw_digit {
                let slice = &text[number_start..cursor];
                if let Ok(value) = slice.parse::<f64>() {
                    return Some(value);
                }
            }

            start = index + key_len;
        }

        None
    }

    fn is_word_boundary(&self, text: &str, index: usize, len: usize) -> bool {
        let before = index.checked_sub(1).and_then(|idx| text.as_bytes().get(idx));
        let after = text.as_bytes().get(index + len);

        let before_ok = before.map(|b| !self.is_word_char(*b)).unwrap_or(true);
        let after_ok = after.map(|b| !self.is_word_char(*b)).unwrap_or(true);

        before_ok && after_ok
    }

    fn is_word_char(&self, byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'$'
    }

    fn looks_signed_out(&self, html: &str) -> bool {
        let lower = html.to_lowercase();
        lower.contains("sign in")
            || lower.contains("log in")
            || lower.contains("login")
            || lower.contains("/login")
            || lower.contains("ampcode.com/login")
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;
        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: AmpSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }
        Err(anyhow::anyhow!("No stored Amp session found"))
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
        Ok(data_dir.join("IncuBar").join("amp-session.json"))
    }

    fn extract_session_cookie(&self, cookie_header: &str) -> Result<String, AmpError> {
        let mut parts = Vec::new();

        for part in cookie_header.split(';') {
            let trimmed = part.trim();
            if let Some((name, value)) = trimmed.split_once('=') {
                if name.trim() == "session" {
                    let value = value.trim();
                    if !value.is_empty() {
                        parts.push(format!("session={}", value));
                    }
                }
            }
        }

        if parts.is_empty() {
            Err(AmpError::NoSessionCookie)
        } else {
            Ok(parts.join("; "))
        }
    }
}

#[async_trait]
impl ProviderFetcher for AmpProvider {
    fn name(&self) -> &'static str {
        "Amp"
    }

    fn description(&self) -> &'static str {
        "Amp"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Amp usage");

        if let Ok(cookies) = self.load_stored_cookies().await {
            match self.fetch_with_cookies(&cookies).await {
                Ok(usage) => return Ok(usage),
                Err(err) => {
                    tracing::debug!("Amp fetch with stored cookies failed: {}", err);
                    if matches!(err, AmpError::SessionExpired | AmpError::NotLoggedIn) {
                        self.clear_session().await;
                    }
                }
            }
        }

        match crate::browser_cookies::import_amp_cookies_from_browser().await {
            Ok(result) => {
                let session_cookie = self.extract_session_cookie(&result.cookie_header)?;
                if let Err(err) = self.store_session(&session_cookie).await {
                    tracing::debug!("Failed to store Amp session: {}", err);
                }
                self.fetch_with_cookies(&session_cookie)
                    .await
                    .map_err(|err| anyhow::anyhow!(err.to_string()))
            }
            Err(err) => Err(anyhow::anyhow!("Not authenticated: {}", err)),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct AmpSession {
    cookie_header: String,
}

#[derive(Debug)]
struct FreeTierUsage {
    quota: f64,
    used: f64,
    hourly_replenishment: f64,
    window_hours: Option<f64>,
}

#[derive(thiserror::Error, Debug)]
enum AmpError {
    #[error("Amp session expired")]
    SessionExpired,
    #[error("Not logged in to Amp")]
    NotLoggedIn,
    #[error("Amp API error: {0}")]
    Api(String),
    #[error("Failed to parse Amp response: {0}")]
    Parse(String),
    #[error("No Amp session cookie found")]
    NoSessionCookie,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_usage_from_html() {
        let provider = AmpProvider::new();
        let html = r#"
            <script>
                window.__AMP__ = {
                    freeTierUsage: {
                        quota: 100,
                        used: 25,
                        hourlyReplenishment: 5,
                        windowHours: 12
                    }
                };
            </script>
        "#;

        let snapshot = provider.parse_usage_response(html).expect("snapshot");
        let primary = snapshot.primary.expect("primary");

        assert!((primary.used_percent - 25.0).abs() < 0.01);
        assert_eq!(primary.window_minutes, Some(720));
        assert_eq!(primary.label.as_deref(), Some("Amp Free"));
        assert!(snapshot.identity.is_some());
    }

    #[test]
    fn errors_when_signed_out() {
        let provider = AmpProvider::new();
        let html = "<html>Please sign in to continue</html>";
        let result = provider.parse_usage_response(html);

        assert!(matches!(result, Err(AmpError::NotLoggedIn)));
    }

    #[test]
    fn extracts_session_cookie() {
        let provider = AmpProvider::new();
        let header = "foo=bar; session=abc123; other=value";
        let extracted = provider.extract_session_cookie(header).expect("session");

        assert_eq!(extracted, "session=abc123");
    }
}
