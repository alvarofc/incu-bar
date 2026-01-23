//! System tray and popup window management

use anyhow::Result;
use once_cell::sync::Lazy;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::f64::consts::PI;
use std::sync::RwLock;
use tauri::{
    image::Image,
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder,
};
use tauri_plugin_positioner::{Position, WindowExt};
use url::Url;
use chrono::{DateTime, Utc};

use crate::providers::{ProviderId, UsageSnapshot};

const TRAY_ICON_ID: &str = "main";
const ICON_SIZE: u32 = 32;
const RING_THICKNESS: f64 = 3.0;
const RING_GAP: f64 = 1.5;
const MAX_RINGS: usize = 3;
const STALE_THRESHOLD_SECS: i64 = 600;

static TRAY_USAGE_STATE: Lazy<RwLock<TrayUsageState>> = Lazy::new(|| {
    RwLock::new(TrayUsageState {
        provider_usage: HashMap::new(),
    })
});

#[derive(Default)]
struct TrayUsageState {
    provider_usage: HashMap<ProviderId, UsageSnapshot>,
}

#[cfg(test)]
fn reset_tray_usage_state() {
    if let Ok(mut state) = TRAY_USAGE_STATE.write() {
        state.provider_usage.clear();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayStatus {
    Ok,
    Inactive,
    Stale,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct UsageRing {
    percent: f64,
    color: [u8; 4],
    provider_id: ProviderId,
}

#[derive(Clone)]
struct TrayRenderState {
    usage_rings: Vec<UsageRing>,
    status: TrayStatus,
    primary_provider: Option<ProviderId>,
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

impl Canvas {
    fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![0; (width * height * 4) as usize],
        }
    }

    fn set_pixel(&mut self, x: i32, y: i32, color: [u8; 4]) {
        if x < 0 || y < 0 {
            return;
        }
        let x = x as u32;
        let y = y as u32;
        if x >= self.width || y >= self.height {
            return;
        }
        let idx = ((y * self.width + x) * 4) as usize;
        self.pixels[idx..idx + 4].copy_from_slice(&color);
    }

    fn draw_filled_circle(&mut self, center_x: f64, center_y: f64, radius: f64, color: [u8; 4]) {
        let radius_sq = radius * radius;
        let min_x = (center_x - radius).floor() as i32;
        let max_x = (center_x + radius).ceil() as i32;
        let min_y = (center_y - radius).floor() as i32;
        let max_y = (center_y + radius).ceil() as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x as f64 + 0.5) - center_x;
                let dy = (y as f64 + 0.5) - center_y;
                if (dx * dx + dy * dy) <= radius_sq {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }

    fn draw_square(&mut self, center_x: i32, center_y: i32, size: i32, color: [u8; 4]) {
        let half = size / 2;
        for y in (center_y - half)..=(center_y + half) {
            for x in (center_x - half)..=(center_x + half) {
                self.set_pixel(x, y, color);
            }
        }
    }

    fn draw_ring(
        &mut self,
        center_x: f64,
        center_y: f64,
        outer_radius: f64,
        thickness: f64,
        track_color: [u8; 4],
        fill_color: Option<[u8; 4]>,
        fill_fraction: Option<f64>,
    ) {
        let inner_radius = (outer_radius - thickness).max(0.0);
        let outer_sq = outer_radius * outer_radius;
        let inner_sq = inner_radius * inner_radius;
        let min_x = (center_x - outer_radius).floor() as i32;
        let max_x = (center_x + outer_radius).ceil() as i32;
        let min_y = (center_y - outer_radius).floor() as i32;
        let max_y = (center_y + outer_radius).ceil() as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x as f64 + 0.5) - center_x;
                let dy = (y as f64 + 0.5) - center_y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > outer_sq || dist_sq < inner_sq {
                    continue;
                }

                let mut color = track_color;
                if let (Some(fill_color), Some(fill_fraction)) = (fill_color, fill_fraction) {
                    let mut angle = dy.atan2(dx) + (PI / 2.0);
                    if angle < 0.0 {
                        angle += 2.0 * PI;
                    }
                    let sweep = angle / (2.0 * PI);
                    if sweep <= fill_fraction {
                        color = fill_color;
                    }
                }
                self.set_pixel(x, y, color);
            }
        }
    }
}

pub fn handle_usage_update(
    app: &AppHandle,
    provider_id: ProviderId,
    usage: UsageSnapshot,
) -> Result<()> {
    if let Ok(mut state) = TRAY_USAGE_STATE.write() {
        state.provider_usage.insert(provider_id, usage);
    } else {
        tracing::warn!("Tray usage state lock poisoned");
    }
    update_tray_icon(app)
}

fn update_tray_icon(app: &AppHandle) -> Result<()> {
    let tray = match app.tray_by_id(TRAY_ICON_ID) {
        Some(tray) => tray,
        None => {
            tracing::warn!("Tray icon not found for updates");
            return Ok(());
        }
    };

    let state = compute_render_state();
    let icon = render_tray_icon(state);
    tray.set_icon(Some(icon))?;
    tray.set_icon_as_template(false)?;
    Ok(())
}

fn compute_render_state() -> TrayRenderState {
    let mut rings: Vec<UsageRing> = Vec::new();
    let mut has_error = false;
    let mut has_stale = false;

    if let Ok(state) = TRAY_USAGE_STATE.read() {
        for (provider_id, usage) in state.provider_usage.iter() {
            if usage.error.is_some() {
                has_error = true;
            }
            if is_snapshot_stale(usage) {
                has_stale = true;
            }
            if let Some(percent) = usage_percent_from_snapshot(usage) {
                rings.push(UsageRing {
                    percent,
                    color: usage_color(percent),
                    provider_id: *provider_id,
                });
            }
        }
    }

    rings.sort_by(|a, b| {
        b.percent
            .partial_cmp(&a.percent)
            .unwrap_or(Ordering::Equal)
    });
    rings.truncate(MAX_RINGS);
    let primary_provider = rings.first().map(|ring| ring.provider_id);

    let status = if has_error {
        TrayStatus::Error
    } else if rings.is_empty() {
        TrayStatus::Inactive
    } else if has_stale {
        TrayStatus::Stale
    } else {
        TrayStatus::Ok
    };

    TrayRenderState {
        usage_rings: rings,
        status,
        primary_provider,
    }
}

fn is_snapshot_stale(usage: &UsageSnapshot) -> bool {
    let parsed = DateTime::parse_from_rfc3339(&usage.updated_at);
    let Some(timestamp) = parsed.ok() else {
        return false;
    };
    let age = Utc::now().signed_duration_since(timestamp.with_timezone(&Utc));
    age.num_seconds() > STALE_THRESHOLD_SECS
}

fn usage_percent_from_snapshot(usage: &UsageSnapshot) -> Option<f64> {
    let mut best: Option<f64> = None;
    for window in [&usage.primary, &usage.secondary, &usage.tertiary] {
        if let Some(window) = window {
            let percent = window.used_percent;
            if percent.is_finite() {
                let clamped = percent.clamp(0.0, 100.0);
                best = Some(best.map_or(clamped, |current| current.max(clamped)));
            }
        }
    }
    best
}

fn render_tray_icon(state: TrayRenderState) -> Image<'static> {
    let mut canvas = Canvas::new(ICON_SIZE, ICON_SIZE);
    let center = (ICON_SIZE as f64 / 2.0, ICON_SIZE as f64 / 2.0);
    let outer_radius = (ICON_SIZE as f64 / 2.0) - 1.0;
    let track_color = [190, 190, 190, 220];

    if state.usage_rings.is_empty() {
        canvas.draw_ring(
            center.0,
            center.1,
            outer_radius,
            RING_THICKNESS,
            track_color,
            None,
            None,
        );
    } else {
        for (index, ring) in state.usage_rings.iter().enumerate() {
            let ring_outer = outer_radius - (index as f64 * (RING_THICKNESS + RING_GAP));
            if ring_outer <= RING_THICKNESS {
                continue;
            }
            let fill_fraction = (ring.percent / 100.0).clamp(0.0, 1.0);
            canvas.draw_ring(
                center.0,
                center.1,
                ring_outer,
                RING_THICKNESS,
                track_color,
                Some(ring.color),
                Some(fill_fraction),
            );
        }
    }
    draw_provider_icon(&mut canvas, state.primary_provider, center);

    let badge_color = match state.status {
        TrayStatus::Ok => None,
        TrayStatus::Inactive => Some([120, 120, 120, 255]),
        TrayStatus::Stale => Some([234, 167, 77, 255]),
        TrayStatus::Error => Some([220, 70, 60, 255]),
    };

    if let Some(color) = badge_color {
        canvas.draw_filled_circle(ICON_SIZE as f64 - 6.0, 6.0, 3.0, color);
    }

    Image::new_owned(canvas.pixels, ICON_SIZE, ICON_SIZE)
}

fn draw_provider_icon(canvas: &mut Canvas, provider: Option<ProviderId>, center: (f64, f64)) {
    match provider {
        Some(ProviderId::Codex) => draw_codex_face(canvas, center),
        Some(ProviderId::Claude) => draw_claude_crab(canvas, center),
        Some(ProviderId::Gemini) => draw_gemini_sparkle(canvas, center),
        Some(ProviderId::Factory) => draw_factory_gear(canvas, center),
        _ => draw_generic_ring(canvas, center),
    }
}

fn draw_codex_face(canvas: &mut Canvas, center: (f64, f64)) {
    let face_color = [232, 198, 132, 255];
    let eye_color = [42, 42, 42, 255];
    let mouth_color = [90, 60, 50, 255];
    canvas.draw_filled_circle(center.0, center.1, 6.0, face_color);
    canvas.set_pixel(center.0 as i32 - 2, center.1 as i32 - 1, eye_color);
    canvas.set_pixel(center.0 as i32 + 2, center.1 as i32 - 1, eye_color);
    for x in (center.0 as i32 - 2)..=(center.0 as i32 + 2) {
        canvas.set_pixel(x, center.1 as i32 + 2, mouth_color);
    }
}

fn draw_claude_crab(canvas: &mut Canvas, center: (f64, f64)) {
    let body_color = [230, 120, 88, 255];
    let claw_color = [200, 92, 68, 255];
    let eye_color = [36, 36, 36, 255];
    canvas.draw_filled_circle(center.0, center.1 + 1.0, 4.5, body_color);
    canvas.draw_filled_circle(center.0 - 5.0, center.1 - 2.0, 2.0, claw_color);
    canvas.draw_filled_circle(center.0 + 5.0, center.1 - 2.0, 2.0, claw_color);
    canvas.set_pixel(center.0 as i32 - 1, center.1 as i32 - 1, eye_color);
    canvas.set_pixel(center.0 as i32 + 1, center.1 as i32 - 1, eye_color);
}

fn draw_gemini_sparkle(canvas: &mut Canvas, center: (f64, f64)) {
    let sparkle_color = [118, 191, 246, 255];
    let cx = center.0 as i32;
    let cy = center.1 as i32;
    for offset in -4_i32..=4 {
        if offset.abs() <= 3 {
            canvas.set_pixel(cx + offset, cy, sparkle_color);
            canvas.set_pixel(cx, cy + offset, sparkle_color);
        }
    }
    canvas.set_pixel(cx - 2, cy - 2, sparkle_color);
    canvas.set_pixel(cx + 2, cy - 2, sparkle_color);
    canvas.set_pixel(cx - 2, cy + 2, sparkle_color);
    canvas.set_pixel(cx + 2, cy + 2, sparkle_color);
}

fn draw_factory_gear(canvas: &mut Canvas, center: (f64, f64)) {
    let gear_color = [140, 140, 140, 255];
    let tooth_color = [165, 165, 165, 255];
    canvas.draw_ring(center.0, center.1, 6.0, 2.0, gear_color, None, None);
    canvas.draw_square(center.0 as i32, center.1 as i32 - 7, 2, tooth_color);
    canvas.draw_square(center.0 as i32, center.1 as i32 + 7, 2, tooth_color);
    canvas.draw_square(center.0 as i32 - 7, center.1 as i32, 2, tooth_color);
    canvas.draw_square(center.0 as i32 + 7, center.1 as i32, 2, tooth_color);
}

fn draw_generic_ring(canvas: &mut Canvas, center: (f64, f64)) {
    let ring_color = [80, 80, 80, 255];
    canvas.draw_ring(center.0, center.1, 6.0, 2.0, ring_color, None, None);
}

fn usage_color(percent: f64) -> [u8; 4] {
    if percent < 50.0 {
        [73, 177, 108, 255]
    } else if percent < 80.0 {
        [234, 167, 77, 255]
    } else {
        [220, 85, 73, 255]
    }
}

/// Set up the system tray icon
pub fn setup_tray(app: &AppHandle) -> Result<()> {
    let _tray = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .tooltip("IncuBar - AI Usage Tracker")
        .icon(render_tray_icon(TrayRenderState {
            usage_rings: Vec::new(),
            status: TrayStatus::Inactive,
            primary_provider: None,
        }))
        .icon_as_template(false)
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

    update_tray_icon(app)?;
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

    let _window =
        WebviewWindowBuilder::new(app, "cursor-login", WebviewUrl::External(url.parse()?))
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
                tracing::debug!(
                    "Cursor page loaded: {} (event: {:?})",
                    url_str,
                    payload.event()
                );

                // Only trigger on Finished events
                if !matches!(payload.event(), tauri::webview::PageLoadEvent::Finished) {
                    return;
                }

                // Check if we're on the settings or dashboard after seeing the auth page
                // OR if we're on settings/dashboard and it's a redirect (user was already logged in)
                let on_logged_in_page = url_str.contains("cursor.com/settings")
                    || url_str.contains("cursor.com/dashboard");
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
                        tracing::info!(
                            "User already logged in to Cursor, triggering cookie extraction"
                        );
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
                        tracing::debug!(
                            "    - {} = {}...",
                            cookie.name(),
                            &cookie.value().chars().take(10).collect::<String>()
                        );
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
                tracing::info!(
                    "    - [{}] {} = {}...",
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

    tracing::info!(
        "=== Cookie extraction complete: {} total cookies ===",
        all_cookies.len()
    );

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
        name.contains("session")
            || name.contains("Session")
            || name.contains("auth")
            || name.contains("Auth")
            || name.contains("WorkOS")
            || name.contains("token")
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
                tracing::warn!(
                    "Failed to position at TrayCenter: {}, trying TrayBottomCenter",
                    e
                );
                // Fallback to TrayBottomCenter if TrayCenter fails
                if let Err(e2) = window
                    .as_ref()
                    .window()
                    .move_window(Position::TrayBottomCenter)
                {
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

#[cfg(test)]
mod tests {
    use super::{
        compute_render_state, render_tray_icon, reset_tray_usage_state, TrayRenderState,
        TrayStatus, UsageRing, ICON_SIZE, STALE_THRESHOLD_SECS, TRAY_USAGE_STATE,
    };
    use crate::providers::{ProviderId, RateWindow, UsageSnapshot};
    use std::collections::HashMap;

    fn sample_usage(percent: f64) -> UsageSnapshot {
        sample_usage_with_time(percent, &chrono::Utc::now().to_rfc3339())
    }

    fn sample_usage_with_time(percent: f64, updated_at: &str) -> UsageSnapshot {
        UsageSnapshot {
            primary: Some(RateWindow {
                used_percent: percent,
                window_minutes: None,
                resets_at: None,
                reset_description: None,
                label: None,
            }),
            secondary: None,
            tertiary: None,
            credits: None,
            cost: None,
            identity: None,
            updated_at: updated_at.to_string(),
            error: None,
        }
    }

    #[test]
    fn render_tray_icon_generates_expected_size() {
        reset_tray_usage_state();
        let icon = render_tray_icon(TrayRenderState {
            usage_rings: vec![UsageRing {
                percent: 42.0,
                color: [73, 177, 108, 255],
                provider_id: ProviderId::Claude,
            }],
            status: TrayStatus::Ok,
            primary_provider: Some(ProviderId::Claude),
        });
        assert_eq!(icon.width(), ICON_SIZE);
        assert_eq!(icon.height(), ICON_SIZE);
        assert_eq!(icon.rgba().len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn compute_render_state_uses_max_usage_and_error() {
        reset_tray_usage_state();
        let mut guard = TRAY_USAGE_STATE.write().unwrap();
        guard.provider_usage = HashMap::from([
            (ProviderId::Claude, sample_usage(33.0)),
            (ProviderId::Codex, sample_usage(81.0)),
        ]);
        drop(guard);

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Ok);
        assert_eq!(state.usage_rings.len(), 2);
        assert_eq!(state.usage_rings.first().map(|ring| ring.percent), Some(81.0));
        assert_eq!(state.primary_provider, Some(ProviderId::Codex));
    }

    #[test]
    fn compute_render_state_marks_error_status() {
        reset_tray_usage_state();
        let mut guard = TRAY_USAGE_STATE.write().unwrap();
        let mut usage = sample_usage(10.0);
        usage.error = Some("failed".to_string());
        guard.provider_usage = HashMap::from([(ProviderId::Claude, usage)]);
        drop(guard);

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Error);
    }

    #[test]
    fn compute_render_state_marks_stale_status() {
        reset_tray_usage_state();
        let stale_time = chrono::Utc::now()
            .checked_sub_signed(chrono::Duration::seconds(STALE_THRESHOLD_SECS + 5))
            .unwrap()
            .to_rfc3339();
        let mut guard = TRAY_USAGE_STATE.write().unwrap();
        guard.provider_usage = HashMap::from([(
            ProviderId::Claude,
            sample_usage_with_time(55.0, &stale_time),
        )]);
        drop(guard);

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Stale);
    }
}
