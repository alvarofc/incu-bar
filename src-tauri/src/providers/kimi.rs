//! Kimi provider implementation
//!
//! Uses JWT cookie authentication via browser cookie import.
//! Endpoint: https://kimi.com/apiv2/grpc/kimi_api.BillingService/GetUsages

use super::{ProviderFetcher, ProviderIdentity, RateWindow, UsageSnapshot};
use async_trait::async_trait;

const USAGE_URL: &str = "https://kimi.com/apiv2/grpc/kimi_api.BillingService/GetUsages";

pub struct KimiProvider {
    client: reqwest::Client,
}

impl KimiProvider {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_default();

        Self { client }
    }

    async fn fetch_with_cookies(
        &self,
        cookie_header: &str,
    ) -> Result<UsageSnapshot, anyhow::Error> {
        let response = self
            .client
            .post(USAGE_URL)
            .header("Cookie", cookie_header)
            .header("Content-Type", "application/proto")
            .header("Accept", "application/proto")
            .header("User-Agent", "IncuBar/1.0")
            .body(Vec::new())
            .send()
            .await?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "Kimi API returned status: {}",
                response.status()
            ));
        }

        let bytes = response.bytes().await?;
        let usages = parse_usage_response(&bytes)?;
        if usages.is_empty() {
            return Err(anyhow::anyhow!("Kimi usage response missing usage entries"));
        }

        Ok(self.build_snapshot(&usages))
    }

    fn build_snapshot(&self, usages: &[UsageEntry]) -> UsageSnapshot {
        let (primary_entry, secondary_entry) = self.pick_usage_entries(usages);

        let primary = primary_entry.and_then(|entry| self.rate_window_from_entry(entry));
        let secondary = secondary_entry.and_then(|entry| self.rate_window_from_entry(entry));

        UsageSnapshot {
            primary,
            secondary,
            tertiary: None,
            credits: None,
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

    fn pick_usage_entries<'a>(
        &self,
        usages: &'a [UsageEntry],
    ) -> (Option<&'a UsageEntry>, Option<&'a UsageEntry>) {
        if usages.is_empty() {
            return (None, None);
        }

        let primary_index = usages
            .iter()
            .position(|entry| {
                entry
                    .usage_type
                    .as_deref()
                    .map(|t| t.eq_ignore_ascii_case("tokens"))
                    .unwrap_or(false)
            })
            .unwrap_or(0);

        let primary = usages.get(primary_index);
        let secondary = usages
            .iter()
            .enumerate()
            .find(|(idx, _)| *idx != primary_index)
            .map(|(_, entry)| entry);

        (primary, secondary)
    }

    fn rate_window_from_entry(&self, entry: &UsageEntry) -> Option<RateWindow> {
        let used = entry.used? as f64;
        let limit = entry.limit? as f64;
        let used_percent = if limit > 0.0 && limit.is_finite() {
            (used / limit) * 100.0
        } else {
            0.0
        };
        let label = entry
            .usage_type
            .as_deref()
            .and_then(|value| self.format_label(value));
        let reset_description = if limit > 0.0 && limit.is_finite() {
            let prefix = label.clone().unwrap_or_else(|| "Usage".to_string());
            Some(format!("{} {:.0}/{:.0}", prefix, used, limit))
        } else {
            None
        };

        Some(RateWindow {
            used_percent: used_percent.clamp(0.0, 100.0),
            window_minutes: None,
            resets_at: None,
            reset_description,
            label,
        })
    }

    fn format_label(&self, label: &str) -> Option<String> {
        let trimmed = label.trim();
        if trimmed.is_empty() {
            return None;
        }

        let mut output = String::new();
        for (index, part) in trimmed.split('_').enumerate() {
            if part.is_empty() {
                continue;
            }
            let mut chars = part.chars();
            if let Some(first) = chars.next() {
                if index > 0 {
                    output.push(' ');
                }
                output.extend(first.to_uppercase());
                output.push_str(chars.as_str());
            }
        }

        if output.is_empty() {
            None
        } else {
            Some(output)
        }
    }

    async fn load_stored_cookies(&self) -> Result<String, anyhow::Error> {
        let session_path = self.get_session_path()?;
        if session_path.exists() {
            let content = tokio::fs::read_to_string(&session_path).await?;
            let session: KimiSession = serde_json::from_str(&content)?;
            return Ok(session.cookie_header);
        }
        Err(anyhow::anyhow!("No stored Kimi session found"))
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

    fn get_session_path(&self) -> Result<std::path::PathBuf, anyhow::Error> {
        let data_dir = dirs::data_dir()
            .ok_or_else(|| anyhow::anyhow!("Could not determine data directory"))?;
        Ok(data_dir.join("IncuBar").join("kimi-session.json"))
    }
}

#[async_trait]
impl ProviderFetcher for KimiProvider {
    fn name(&self) -> &'static str {
        "Kimi"
    }

    fn description(&self) -> &'static str {
        "Kimi (Moonshot AI)"
    }

    async fn fetch(&self) -> Result<UsageSnapshot, anyhow::Error> {
        tracing::debug!("Fetching Kimi usage");

        if let Ok(cookies) = self.load_stored_cookies().await {
            if let Ok(usage) = self.fetch_with_cookies(&cookies).await {
                return Ok(usage);
            }
        }

        match crate::browser_cookies::import_kimi_cookies_from_browser().await {
            Ok(result) => {
                if let Err(err) = self.store_session(&result.cookie_header).await {
                    tracing::debug!("Failed to store Kimi session: {}", err);
                }
                self.fetch_with_cookies(&result.cookie_header).await
            }
            Err(err) => Err(anyhow::anyhow!("Not authenticated: {}", err)),
        }
    }
}

#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct KimiSession {
    cookie_header: String,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct UsageEntry {
    usage_type: Option<String>,
    used: Option<u64>,
    limit: Option<u64>,
}

fn parse_usage_response(bytes: &[u8]) -> Result<Vec<UsageEntry>, anyhow::Error> {
    let mut usages = Vec::new();
    let mut index = 0;

    while index < bytes.len() {
        let (tag, tag_len) = decode_varint(bytes, index)?;
        index += tag_len;
        let field_number = tag >> 3;
        let wire_type = (tag & 0x7) as u8;

        match (field_number, wire_type) {
            (1, 2) => {
                let (length, len_len) = decode_varint(bytes, index)?;
                index += len_len;
                let end = index + length as usize;
                if end > bytes.len() {
                    return Err(anyhow::anyhow!("Kimi usage response truncated"));
                }
                let entry_bytes = &bytes[index..end];
                index = end;
                if let Ok(entry) = parse_usage_entry(entry_bytes) {
                    usages.push(entry);
                }
            }
            _ => {
                index = skip_field(bytes, index, wire_type)?;
            }
        }
    }

    Ok(usages)
}

fn parse_usage_entry(bytes: &[u8]) -> Result<UsageEntry, anyhow::Error> {
    let mut entry = UsageEntry::default();
    let mut index = 0;

    while index < bytes.len() {
        let (tag, tag_len) = decode_varint(bytes, index)?;
        index += tag_len;
        let field_number = tag >> 3;
        let wire_type = (tag & 0x7) as u8;

        match (field_number, wire_type) {
            (1, 2) => {
                let (length, len_len) = decode_varint(bytes, index)?;
                index += len_len;
                let end = index + length as usize;
                if end > bytes.len() {
                    return Err(anyhow::anyhow!("Kimi usage entry truncated"));
                }
                let value = std::str::from_utf8(&bytes[index..end]).ok();
                entry.usage_type = value.map(|s| s.to_string());
                index = end;
            }
            (2, 0) => {
                let (value, consumed) = decode_varint(bytes, index)?;
                entry.used = Some(value);
                index += consumed;
            }
            (3, 0) => {
                let (value, consumed) = decode_varint(bytes, index)?;
                entry.limit = Some(value);
                index += consumed;
            }
            _ => {
                index = skip_field(bytes, index, wire_type)?;
            }
        }
    }

    Ok(entry)
}

fn decode_varint(bytes: &[u8], start: usize) -> Result<(u64, usize), anyhow::Error> {
    let mut result = 0u64;
    let mut shift = 0u32;
    let mut index = start;

    while index < bytes.len() {
        let byte = bytes[index];
        result |= ((byte & 0x7F) as u64) << shift;
        index += 1;
        if byte & 0x80 == 0 {
            return Ok((result, index - start));
        }
        shift += 7;
        if shift >= 64 {
            break;
        }
    }

    Err(anyhow::anyhow!("Invalid protobuf varint"))
}

fn skip_field(bytes: &[u8], start: usize, wire_type: u8) -> Result<usize, anyhow::Error> {
    match wire_type {
        0 => {
            let (_, consumed) = decode_varint(bytes, start)?;
            Ok(start + consumed)
        }
        1 => Ok(start + 8),
        2 => {
            let (length, len_len) = decode_varint(bytes, start)?;
            Ok(start + len_len + length as usize)
        }
        5 => Ok(start + 4),
        _ => Err(anyhow::anyhow!("Unsupported protobuf wire type")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn encode_varint(mut value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut byte = (value & 0x7F) as u8;
            value >>= 7;
            if value != 0 {
                byte |= 0x80;
            }
            out.push(byte);
            if value == 0 {
                break;
            }
        }
        out
    }

    fn encode_length_delimited(field_number: u64, data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(encode_varint((field_number << 3) | 2));
        out.extend(encode_varint(data.len() as u64));
        out.extend(data);
        out
    }

    fn encode_varint_field(field_number: u64, value: u64) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(encode_varint((field_number << 3) | 0));
        out.extend(encode_varint(value));
        out
    }

    #[test]
    fn parses_usage_response_with_tokens() {
        let mut usage = Vec::new();
        usage.extend(encode_length_delimited(1, b"tokens"));
        usage.extend(encode_varint_field(2, 50000));
        usage.extend(encode_varint_field(3, 100000));

        let mut response = Vec::new();
        response.extend(encode_length_delimited(1, &usage));

        let parsed = parse_usage_response(&response).expect("parse response");
        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].usage_type.as_deref(), Some("tokens"));
        assert_eq!(parsed[0].used, Some(50000));
        assert_eq!(parsed[0].limit, Some(100000));
    }

    #[test]
    fn builds_snapshot_from_usage_entries() {
        let provider = KimiProvider::new();
        let usages = vec![UsageEntry {
            usage_type: Some("tokens".to_string()),
            used: Some(25000),
            limit: Some(100000),
        }];

        let snapshot = provider.build_snapshot(&usages);
        let primary = snapshot.primary.expect("primary");

        assert!((primary.used_percent - 25.0).abs() < 0.01);
        assert_eq!(primary.label.as_deref(), Some("Tokens"));
        assert_eq!(
            primary.reset_description.as_deref(),
            Some("Tokens 25000/100000")
        );
    }


}
