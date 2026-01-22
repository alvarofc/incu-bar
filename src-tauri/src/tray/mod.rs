//! System tray and popup window management

use anyhow::Result;
use tauri::{
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};
use url::Url;

/// Set up the system tray icon
pub fn setup_tray(app: &AppHandle) -> Result<()> {
    let _tray = TrayIconBuilder::new()
        .tooltip("IncuBar - AI Usage Tracker")
        .icon(app.default_window_icon().unwrap().clone())
        .on_tray_icon_event(|tray, event| {
            // Forward tray events to the positioner plugin
            tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
            
            match event {
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    tracing::info!("Tray icon clicked");
                    let app = tray.app_handle();
                    if let Err(e) = toggle_popup(app) {
                        tracing::error!("Failed to toggle popup: {}", e);
                    }
                }
                _ => {}
            }
        })
        .build(app)?;

    tracing::info!("Tray icon created");
    Ok(())
}

/// Create the popup window (hidden by default)
pub fn create_popup_window(app: &AppHandle) -> Result<()> {
    let window = WebviewWindowBuilder::new(app, "popup", WebviewUrl::App("index.html".into()))
        .title("IncuBar")
        .inner_size(320.0, 420.0)
        .resizable(false)
        .visible(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true)
        .build()?;

    // Open devtools in debug mode
    #[cfg(debug_assertions)]
    window.open_devtools();

    tracing::info!("Popup window created");
    Ok(())
}

/// Create a Cursor login window
/// Opens cursor.com/settings for user to login and automatically extracts cookies
pub fn create_cursor_login_window(app: &AppHandle) -> Result<()> {
    // Check if window already exists
    if let Some(existing) = app.get_webview_window("cursor-login") {
        existing.show()?;
        existing.set_focus()?;
        return Ok(());
    }

    let url = "https://www.cursor.com/settings";

    // Clone app handle for use in the callbacks
    let app_handle = app.clone();
    
    // Track if we've seen the login page (to avoid triggering on initial load)
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let seen_auth_page = Arc::new(AtomicBool::new(false));
    let seen_auth_page_clone = seen_auth_page.clone();
    
    // NOTE: We removed data_store_identifier to test if it was blocking cookie access
    // Previously: data_store_id = [0x43, 0x75, ...]; // "CursorLogin" 
    // The separate data store might prevent cookies_for_url() from accessing cookies
    
    let _window = WebviewWindowBuilder::new(app, "cursor-login", WebviewUrl::External(url.parse()?))
        .title("Cursor Login - Login will be detected automatically")
        .inner_size(500.0, 700.0)
        .resizable(true)
        .visible(true)
        .decorations(true)
        .always_on_top(false)
        .focused(true)
        // REMOVED: data_store_identifier to allow cookie access from main process
        // Track navigation to detect auth pages
        .on_navigation(move |nav_url: &Url| {
            let url_str = nav_url.as_str();
            tracing::debug!("Cursor login navigating to: {}", url_str);
            
            // If we see the authenticator page, user needs to login
            if url_str.contains("authenticator.cursor.sh") || url_str.contains("/auth/") {
                seen_auth_page.store(true, Ordering::SeqCst);
                tracing::info!("User redirected to Cursor login page");
            }
            
            true // Allow all navigation
        })
        // Detect when page finishes loading
        .on_page_load(move |_window, payload| {
            let url_str = payload.url().as_str();
            tracing::debug!("Cursor page loaded: {} (event: {:?})", url_str, payload.event());
            
            // Only trigger on Finished events
            if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                return;
            }
            
            // Check if we're on the settings or dashboard after seeing the auth page
            // OR if we're on settings/dashboard and it's a redirect (user was already logged in)
            let on_logged_in_page = url_str.contains("cursor.com/settings") || url_str.contains("cursor.com/dashboard");
            let saw_auth = seen_auth_page_clone.load(Ordering::SeqCst);
            
            if on_logged_in_page {
                if saw_auth {
                    tracing::info!("User completed Cursor login, triggering cookie extraction");
                    // User went through auth flow and landed on settings/dashboard
                    let app_for_emit = app_handle.clone();
                    std::thread::spawn(move || {
                        // Wait a moment for cookies to be fully set
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        let _ = app_for_emit.emit("cursor-login-detected", ());
                    });
                } else {
                    // User was already logged in, still extract cookies
                    tracing::info!("User already logged in to Cursor, triggering cookie extraction");
                    let app_for_emit = app_handle.clone();
                    // Mark as seen so we don't trigger again
                    seen_auth_page_clone.store(true, Ordering::SeqCst);
                    std::thread::spawn(move || {
                        std::thread::sleep(std::time::Duration::from_secs(1));
                        let _ = app_for_emit.emit("cursor-login-detected", ());
                    });
                }
            }
        })
        .build()?;

    tracing::info!("Cursor login window created with auto-cookie detection");
    Ok(())
}

/// Close the Cursor login window
pub fn close_cursor_login_window(app: &AppHandle) -> Result<()> {
    if let Some(window) = app.get_webview_window("cursor-login") {
        window.close()?;
    }
    Ok(())
}

/// Extract cookies from the Cursor login window
/// Returns the cookies as a Cookie header string
pub async fn extract_cursor_cookies(app: &AppHandle) -> Result<Option<String>> {
    let window = match app.get_webview_window("cursor-login") {
        Some(w) => w,
        None => {
            tracing::warn!("Cursor login window not found");
            return Ok(None);
        }
    };

    tracing::info!("=== Starting Cursor cookie extraction ===");
    
    // Method 1: Try JavaScript document.cookie first (non-HttpOnly cookies only)
    // Note: Tauri's eval() doesn't return values directly, would need IPC for that
    tracing::info!("Method 1: JavaScript approach skipped (eval doesn't return values)");
    // For future: could use invoke handler with window.emit() to pass cookies back

    // Method 2: Try multiple URL patterns for cursor.com cookies
    tracing::info!("Method 2: Trying cookies_for_url() with various URLs...");
    let urls_to_try = [
        "https://www.cursor.com/",
        "https://cursor.com/",
        "https://www.cursor.com/settings",
        "https://cursor.com/settings",
        "https://authenticator.cursor.sh/",
        "https://www.cursor.sh/",
        "https://cursor.sh/",
    ];
    
    let mut all_cookies: Vec<cookie::Cookie<'static>> = Vec::new();
    
    for url_str in urls_to_try {
        if let Ok(url) = Url::parse(url_str) {
            match window.cookies_for_url(url) {
                Ok(cookies) => {
                    tracing::info!("  {} => {} cookies", url_str, cookies.len());
                    for cookie in &cookies {
                        tracing::debug!("    - {} = {}...", cookie.name(), 
                            &cookie.value().chars().take(10).collect::<String>());
                    }
                    for cookie in cookies {
                        let name = cookie.name().to_string();
                        if !all_cookies.iter().any(|c| c.name() == name) {
                            all_cookies.push(cookie);
                        }
                    }
                }
                Err(e) => {
                    tracing::info!("  {} => ERROR: {}", url_str, e);
                }
            }
        }
    }
    
    // Method 3: Try getting ALL cookies from the webview
    tracing::info!("Method 3: Trying cookies() to get all webview cookies...");
    match window.cookies() {
        Ok(cookies) => {
            tracing::info!("  Total cookies in webview store: {}", cookies.len());
            for cookie in &cookies {
                let domain = cookie.domain().unwrap_or("(no domain)");
                tracing::info!("    - [{}] {} = {}...", 
                    domain,
                    cookie.name(), 
                    &cookie.value().chars().take(10).collect::<String>()
                );
            }
            // Add any cursor-related cookies we don't already have
            for cookie in cookies {
                let domain = cookie.domain().unwrap_or("");
                if domain.contains("cursor") {
                    let name = cookie.name().to_string();
                    if !all_cookies.iter().any(|c| c.name() == name) {
                        all_cookies.push(cookie);
                    }
                }
            }
        }
        Err(e) => {
            tracing::warn!("  cookies() failed: {}", e);
        }
    }
    
    tracing::info!("=== Cookie extraction complete: {} total cookies ===", all_cookies.len());
    
    if all_cookies.is_empty() {
        tracing::warn!("No cookies found for cursor.com after trying all methods");
        tracing::info!("This may be a Tauri limitation - consider using manual cookie paste");
        return Ok(None);
    }
    
    // Build a cookie header string from all cookies
    let cookie_header: String = all_cookies
        .iter()
        .map(|c| format!("{}={}", c.name(), c.value()))
        .collect::<Vec<_>>()
        .join("; ");
    
    // Check if we have the essential session cookies
    let has_session = all_cookies.iter().any(|c| {
        let name = c.name();
        name.contains("session") || name.contains("Session") || 
        name.contains("auth") || name.contains("Auth") ||
        name.contains("WorkOS") || name.contains("token")
    });
    
    if has_session {
        tracing::info!("Found session cookies for Cursor");
        Ok(Some(cookie_header))
    } else {
        tracing::debug!("No session cookies found, but returning available cookies");
        if !cookie_header.is_empty() {
            Ok(Some(cookie_header))
        } else {
            Ok(None)
        }
    }
}

/// Toggle the popup window visibility, positioning near the tray icon
fn toggle_popup(app: &AppHandle) -> Result<()> {
    tracing::debug!("toggle_popup called");
    if let Some(window) = app.get_webview_window("popup") {
        let is_visible = window.is_visible().unwrap_or(false);
        tracing::debug!("Popup window found, is_visible: {}", is_visible);
        
        if is_visible {
            tracing::info!("Hiding popup");
            window.hide()?;
        } else {
            // Use the positioner plugin to position at tray center
            // This handles multi-monitor setups correctly
            if let Err(e) = window.as_ref().window().move_window(Position::TrayCenter) {
                tracing::warn!("Failed to position at TrayCenter: {}, trying TrayBottomCenter", e);
                // Fallback to TrayBottomCenter if TrayCenter fails
                if let Err(e2) = window.as_ref().window().move_window(Position::TrayBottomCenter) {
                    tracing::error!("Failed to position popup: {}", e2);
                }
            }
            
            tracing::info!("Showing popup at TrayCenter");
            window.show()?;
            window.set_focus()?;
        }
    } else {
        tracing::warn!("Popup window not found!");
    }

    Ok(())
}
