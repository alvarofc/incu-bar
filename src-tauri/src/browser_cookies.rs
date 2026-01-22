//! Browser cookie extraction module
//!
//! Extracts cookies from system browsers (Chrome, Firefox, Safari, etc.) for use with
//! providers that require cookie-based authentication.
//!
//! ## Permissions Required
//! 
//! ### Chrome/Chromium browsers (macOS)
//! - Keychain access: The app will prompt for keychain access to decrypt cookies
//! - If denied, cookie import will fail with a keychain error
//!
//! ### Firefox
//! - No special permissions required on most systems
//! - Firefox must be closed or cookies may be locked
//!
//! ### Safari (macOS)
//! - Full Disk Access: Required to read Safari's Cookies.binarycookies file
//! - Grant in System Settings > Privacy & Security > Full Disk Access

use anyhow::Result;
use decrypt_cookies::prelude::*;
use decrypt_cookies::chromium::{ChromiumCookie, GetCookies};

// Firefox support
use decrypt_cookies::firefox::{builder::FirefoxBuilder, GetCookies as FirefoxGetCookies, MozCookie};

// Safari support (macOS only) - SafariCookie is re-exported from prelude
#[cfg(target_os = "macos")]
use decrypt_cookies::safari::SafariBuilder;

/// Domains to extract cookies for Cursor
const CURSOR_DOMAINS: &[&str] = &["cursor.com", "cursor.sh", "workos.com"];
const FACTORY_DOMAINS: &[&str] = &["factory.ai", "app.factory.ai"];
const AUGMENT_DOMAINS: &[&str] = &["augmentcode.com", "app.augmentcode.com"];
const KIMI_DOMAINS: &[&str] = &["kimi.moonshot.cn", "kimi.com"];
const MINIMAX_DOMAINS: &[&str] = &["minimax.chat", "platform.minimax.io"];
const AMP_DOMAINS: &[&str] = &["ampcode.com", "www.ampcode.com"];

/// Result of a browser cookie import
#[derive(Debug)]
pub struct BrowserCookieResult {
    pub browser_name: String,
    pub cookie_header: String,
    pub cookie_count: usize,
}

/// Import Cursor cookies from system browsers
/// 
/// Tries Chrome first (most common), then other Chromium browsers.
/// Returns the first successful result.
pub async fn import_cursor_cookies_from_browser() -> Result<BrowserCookieResult> {
    import_cookies_for_domains(CURSOR_DOMAINS).await
}

/// Import Factory (Droid) cookies from system browsers
pub async fn import_factory_cookies_from_browser() -> Result<BrowserCookieResult> {
    import_cookies_for_domains(FACTORY_DOMAINS).await
}

/// Import Augment cookies from system browsers
pub async fn import_augment_cookies_from_browser() -> Result<BrowserCookieResult> {
    import_cookies_for_domains(AUGMENT_DOMAINS).await
}

/// Import Kimi cookies from system browsers
pub async fn import_kimi_cookies_from_browser() -> Result<BrowserCookieResult> {
    import_cookies_for_domains(KIMI_DOMAINS).await
}

/// Import MiniMax cookies from system browsers
pub async fn import_minimax_cookies_from_browser() -> Result<BrowserCookieResult> {
    import_cookies_for_domains(MINIMAX_DOMAINS).await
}

/// Import Amp cookies from system browsers
pub async fn import_amp_cookies_from_browser() -> Result<BrowserCookieResult> {
    import_cookies_for_domains(AMP_DOMAINS).await
}

/// Import cookies for specified domains from system browsers
pub async fn import_cookies_for_domains(domains: &[&str]) -> Result<BrowserCookieResult> {
    // Try Firefox first (common for power users)
    if let Ok(result) = try_firefox_cookies(domains).await {
        return Ok(result);
    }
    
    // Try Chrome (most common browser)
    if let Ok(result) = try_chrome_cookies(domains).await {
        return Ok(result);
    }
    
    // Try Safari (macOS only) - second preference for Mac users
    #[cfg(target_os = "macos")]
    if let Ok(result) = try_safari_cookies(domains).await {
        return Ok(result);
    }
    
    // Try Edge
    if let Ok(result) = try_edge_cookies(domains).await {
        return Ok(result);
    }
    
    // Try Brave
    if let Ok(result) = try_brave_cookies(domains).await {
        return Ok(result);
    }
    
    // Try Chromium
    if let Ok(result) = try_chromium_cookies(domains).await {
        return Ok(result);
    }
    
    Err(anyhow::anyhow!(
        "Could not find cookies in any browser for domains: {:?}. \
        Make sure you're logged in, then try again. \
        For Chrome: allow keychain access when prompted. \
        For Safari: enable Full Disk Access in System Settings.",
        domains
    ))
}

async fn try_firefox_cookies(domains: &[&str]) -> Result<BrowserCookieResult> {
    tracing::info!("Attempting to import cookies from Firefox...");
    
    let firefox = FirefoxBuilder::<Firefox>::new()
        .build_cookie()
        .await
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("locked") || err_str.contains("busy") {
                anyhow::anyhow!("Firefox database is locked. Please close Firefox and try again.")
            } else if err_str.contains("No such file") || err_str.contains("not found") {
                anyhow::anyhow!("Firefox profile not found. Make sure Firefox is installed and has been used.")
            } else {
                anyhow::anyhow!("Firefox not available: {}", e)
            }
        })?;
    
    let all_cookies: Vec<MozCookie> = firefox
        .cookies_all()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read Firefox cookies: {}", e))?;
    
    extract_firefox_cookies("Firefox", all_cookies, domains)
}

async fn try_chrome_cookies(domains: &[&str]) -> Result<BrowserCookieResult> {
    tracing::info!("Attempting to import cookies from Chrome...");
    
    let chromium = ChromiumBuilder::<Chrome>::new()
        .build()
        .await
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("keychain") || err_str.contains("Keychain") {
                anyhow::anyhow!("Chrome requires keychain access. Please allow when prompted, or check System Settings > Privacy > Keychain Access.")
            } else {
                anyhow::anyhow!("Chrome not available: {}", e)
            }
        })?;
    
    let all_cookies: Vec<ChromiumCookie> = chromium
        .cookies_all()
        .await
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("keychain") || err_str.contains("Keychain") {
                anyhow::anyhow!("Failed to decrypt Chrome cookies. Please allow keychain access when prompted.")
            } else {
                anyhow::anyhow!("Failed to read Chrome cookies: {}", e)
            }
        })?;
    
    extract_chromium_cookies("Chrome", all_cookies, domains)
}

async fn try_edge_cookies(domains: &[&str]) -> Result<BrowserCookieResult> {
    tracing::info!("Attempting to import cookies from Edge...");
    
    let chromium = ChromiumBuilder::<Edge>::new()
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("Edge not available: {}", e))?;
    
    let all_cookies: Vec<ChromiumCookie> = chromium
        .cookies_all()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read Edge cookies: {}", e))?;
    
    extract_chromium_cookies("Edge", all_cookies, domains)
}

async fn try_brave_cookies(domains: &[&str]) -> Result<BrowserCookieResult> {
    tracing::info!("Attempting to import cookies from Brave...");
    
    let chromium = ChromiumBuilder::<Brave>::new()
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("Brave not available: {}", e))?;
    
    let all_cookies: Vec<ChromiumCookie> = chromium
        .cookies_all()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read Brave cookies: {}", e))?;
    
    extract_chromium_cookies("Brave", all_cookies, domains)
}

async fn try_chromium_cookies(domains: &[&str]) -> Result<BrowserCookieResult> {
    tracing::info!("Attempting to import cookies from Chromium...");
    
    let chromium = ChromiumBuilder::<Chromium>::new()
        .build()
        .await
        .map_err(|e| anyhow::anyhow!("Chromium not available: {}", e))?;
    
    let all_cookies: Vec<ChromiumCookie> = chromium
        .cookies_all()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read Chromium cookies: {}", e))?;
    
    extract_chromium_cookies("Chromium", all_cookies, domains)
}

/// Extract cookies matching the given domains from Chromium-based browsers
fn extract_chromium_cookies(
    browser_name: &str,
    cookies: Vec<ChromiumCookie>,
    domains: &[&str],
) -> Result<BrowserCookieResult> {
    let mut cookie_parts: Vec<String> = Vec::new();
    
    for cookie in cookies {
        let cookie_domain = cookie.host_key.to_lowercase();
        for &target_domain in domains {
            if domain_matches(&cookie_domain, target_domain) {
                // decrypted_value is Option<String>, use unwrap_or_default
                let value = cookie.decrypted_value.unwrap_or_default();
                if !value.is_empty() {
                    let part = format!("{}={}", cookie.name, value);
                    // Avoid duplicate cookie names
                    if !cookie_parts.iter().any(|p| p.starts_with(&format!("{}=", cookie.name))) {
                        cookie_parts.push(part);
                    }
                }
                break;
            }
        }
    }
    
    if cookie_parts.is_empty() {
        return Err(anyhow::anyhow!(
            "No cookies found in {} for domains: {:?}. Make sure you're logged in.",
            browser_name,
            domains
        ));
    }
    
    let cookie_header = cookie_parts.join("; ");
    let cookie_count = cookie_parts.len();
    
    tracing::info!(
        "Found {} cookies in {} for target domains",
        cookie_count,
        browser_name
    );
    
    Ok(BrowserCookieResult {
        browser_name: browser_name.to_string(),
        cookie_header,
        cookie_count,
    })
}

/// Check if a cookie domain matches a target domain
fn domain_matches(cookie_domain: &str, target_domain: &str) -> bool {
    let cookie_domain = cookie_domain.trim_start_matches('.');
    cookie_domain == target_domain || cookie_domain.ends_with(&format!(".{}", target_domain))
}

// ============== Firefox Support ==============

/// Extract cookies matching the given domains from Firefox
fn extract_firefox_cookies(
    browser_name: &str,
    cookies: Vec<MozCookie>,
    domains: &[&str],
) -> Result<BrowserCookieResult> {
    let mut cookie_parts: Vec<String> = Vec::new();
    
    for cookie in cookies {
        let cookie_domain = cookie.host.to_lowercase();
        for &target_domain in domains {
            if domain_matches(&cookie_domain, target_domain) {
                if !cookie.value.is_empty() {
                    let part = format!("{}={}", cookie.name, cookie.value);
                    // Avoid duplicate cookie names
                    if !cookie_parts.iter().any(|p| p.starts_with(&format!("{}=", cookie.name))) {
                        cookie_parts.push(part);
                    }
                }
                break;
            }
        }
    }
    
    if cookie_parts.is_empty() {
        return Err(anyhow::anyhow!(
            "No cookies found in {} for domains: {:?}. Make sure you're logged in.",
            browser_name,
            domains
        ));
    }
    
    let cookie_header = cookie_parts.join("; ");
    let cookie_count = cookie_parts.len();
    
    tracing::info!(
        "Found {} cookies in {} for target domains",
        cookie_count,
        browser_name
    );
    
    Ok(BrowserCookieResult {
        browser_name: browser_name.to_string(),
        cookie_header,
        cookie_count,
    })
}

// ============== Safari Support (macOS only) ==============

#[cfg(target_os = "macos")]
async fn try_safari_cookies(domains: &[&str]) -> Result<BrowserCookieResult> {
    tracing::info!("Attempting to import cookies from Safari...");
    
    let safari = SafariBuilder::new()
        .build()
        .await
        .map_err(|e| {
            let err_str = e.to_string();
            if err_str.contains("permission") || err_str.contains("denied") || err_str.contains("Operation not permitted") {
                anyhow::anyhow!("Safari requires Full Disk Access. Grant permission in System Settings > Privacy & Security > Full Disk Access, then restart the app.")
            } else if err_str.contains("No such file") || err_str.contains("not found") {
                anyhow::anyhow!("Safari cookies file not found. Make sure Safari has been used at least once.")
            } else {
                anyhow::anyhow!("Safari not available: {}", e)
            }
        })?;
    
    // SafariCookie is from the prelude
    let all_cookies = safari.cookies_all();
    
    extract_safari_cookies("Safari", all_cookies, domains)
}

#[cfg(target_os = "macos")]
fn extract_safari_cookies(
    browser_name: &str,
    cookies: &[SafariCookie],
    domains: &[&str],
) -> Result<BrowserCookieResult> {
    let mut cookie_parts: Vec<String> = Vec::new();
    
    for cookie in cookies {
        let cookie_domain = cookie.domain.to_lowercase();
        for &target_domain in domains {
            if domain_matches(&cookie_domain, target_domain) {
                if !cookie.value.is_empty() {
                    let part = format!("{}={}", cookie.name, cookie.value);
                    // Avoid duplicate cookie names
                    if !cookie_parts.iter().any(|p| p.starts_with(&format!("{}=", cookie.name))) {
                        cookie_parts.push(part);
                    }
                }
                break;
            }
        }
    }
    
    if cookie_parts.is_empty() {
        return Err(anyhow::anyhow!(
            "No cookies found in {} for domains: {:?}. Make sure you're logged in.",
            browser_name,
            domains
        ));
    }
    
    let cookie_header = cookie_parts.join("; ");
    let cookie_count = cookie_parts.len();
    
    tracing::info!(
        "Found {} cookies in {} for target domains",
        cookie_count,
        browser_name
    );
    
    Ok(BrowserCookieResult {
        browser_name: browser_name.to_string(),
        cookie_header,
        cookie_count,
    })
}
