//! System tray and popup window management

use anyhow::Result;
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
use rand::Rng;
use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::f64::consts::PI;
use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};
use std::sync::{Mutex, RwLock, RwLockReadGuard, RwLockWriteGuard};
use tauri::{
    image::Image,
    menu::{Menu, MenuItemBuilder},
    tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Theme, WebviewUrl, WebviewWindowBuilder, WindowEvent,
};
use tauri_plugin_positioner::{Position, WindowExt};
use tokio::sync::mpsc;
use tokio::time::{self, Instant, MissedTickBehavior};
use url::Url;

use crate::debug_settings;
use crate::providers::{ProviderId, UsageSnapshot};

const TRAY_ICON_ID: &str = "main";
const TRAY_REFRESH_MENU_ID: &str = "tray-refresh";
const TRAY_TOOLTIP_BASE: &str = "IncuBar - AI Usage Tracker";
const ICON_SIZE: u32 = 32;
const RING_THICKNESS: f64 = 3.0;
const RING_GAP: f64 = 1.5;
const MAX_RINGS: usize = 3;
// Keep in sync with `src/lib/staleness.ts` DEFAULT_STALE_AFTER_MS (10 minutes).
const STALE_THRESHOLD_SECS: i64 = 600;
const LOADING_ANIMATION_TICK_MS: u64 = 250;
const BLINKING_ANIMATION_TICK_MS: u64 = 500;
const RANDOM_BLINK_INTERVAL_MS: u64 = 4200;
const RANDOM_BLINK_VARIANCE_MS: u64 = 1600;

static TRAY_USAGE_STATE: Lazy<RwLock<TrayUsageState>> =
    Lazy::new(|| RwLock::new(TrayUsageState::default()));

static TRAY_DISPLAY_TEXT_STATE: Lazy<RwLock<TrayDisplayTextState>> =
    Lazy::new(|| RwLock::new(TrayDisplayTextState::default()));

static TRAY_ANIMATION_CONTROL: Lazy<Mutex<Option<mpsc::UnboundedSender<AnimationCommand>>>> =
    Lazy::new(|| Mutex::new(None));

static TRAY_HANDLE: Lazy<Mutex<Option<TrayIcon>>> = Lazy::new(|| Mutex::new(None));
#[allow(dead_code)] // Used only in release builds
static TRAY_ICON_TEMPLATE: Lazy<Image<'static>> = Lazy::new(|| {
    let bytes = include_bytes!("../../icons/32x32.png");
    Image::from_bytes(bytes)
        .map(|image| image.to_owned())
        .unwrap_or_else(|err| {
            tracing::warn!("Failed to load tray icon bytes: {err}");
            render_tray_icon(TrayRenderState {
                usage_rings: Vec::new(),
                status: TrayStatus::Disabled,
                primary_provider: None,
                animation_phase: 0,
                blink_enabled: false,
                theme: Theme::Light,
            })
        })
});

enum AnimationCommand {
    Wake(AppHandle),
}

/// Shutdown signal for the random blinking thread
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Request shutdown of background threads (call on app exit)
pub fn request_shutdown() {
    SHUTDOWN_REQUESTED.store(true, AtomicOrdering::SeqCst);
}

fn write_tray_usage_state() -> RwLockWriteGuard<'static, TrayUsageState> {
    TRAY_USAGE_STATE.write().unwrap_or_else(|poisoned| {
        tracing::warn!("Tray usage state lock poisoned; recovering");
        TRAY_USAGE_STATE.clear_poison();
        poisoned.into_inner()
    })
}

fn read_tray_usage_state() -> RwLockReadGuard<'static, TrayUsageState> {
    TRAY_USAGE_STATE.read().unwrap_or_else(|poisoned| {
        tracing::warn!("Tray usage state lock poisoned; recovering");
        TRAY_USAGE_STATE.clear_poison();
        poisoned.into_inner()
    })
}

fn write_tray_display_text_state() -> RwLockWriteGuard<'static, TrayDisplayTextState> {
    TRAY_DISPLAY_TEXT_STATE.write().unwrap_or_else(|poisoned| {
        tracing::warn!("Tray display text state lock poisoned; recovering");
        TRAY_DISPLAY_TEXT_STATE.clear_poison();
        poisoned.into_inner()
    })
}

fn read_tray_display_text_state() -> RwLockReadGuard<'static, TrayDisplayTextState> {
    TRAY_DISPLAY_TEXT_STATE.read().unwrap_or_else(|poisoned| {
        tracing::warn!("Tray display text state lock poisoned; recovering");
        TRAY_DISPLAY_TEXT_STATE.clear_poison();
        poisoned.into_inner()
    })
}

fn animation_tick_ms(blink_enabled: bool) -> u64 {
    if blink_enabled {
        BLINKING_ANIMATION_TICK_MS
    } else {
        LOADING_ANIMATION_TICK_MS
    }
}

fn animation_tick_duration(blink_enabled: bool) -> std::time::Duration {
    std::time::Duration::from_millis(animation_tick_ms(blink_enabled))
}

fn animation_interval(blink_enabled: bool) -> time::Interval {
    let duration = animation_tick_duration(blink_enabled);
    let start = Instant::now() + duration;
    let mut interval = time::interval_at(start, duration);
    interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    interval
}

fn animation_should_continue(state: &TrayUsageState) -> bool {
    state.loading_count > 0 || state.blinking
}

fn advance_animation_phase() -> bool {
    let mut guard = write_tray_usage_state();
    guard.animation_phase = guard.animation_phase.wrapping_add(1);
    animation_should_continue(&guard)
}

fn start_tray_animation_thread(app: &AppHandle) {
    let mut guard = TRAY_ANIMATION_CONTROL.lock().unwrap();
    if let Some(sender) = guard.as_ref() {
        let _ = sender.send(AnimationCommand::Wake(app.clone()));
        return;
    }

    let (sender, mut receiver) = mpsc::unbounded_channel();
    *guard = Some(sender.clone());
    let app_handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut active = false;
        let mut app_handle = app_handle;
        let mut interval = animation_interval(false);
        let mut interval_blink_enabled = false;

        loop {
            if !active {
                match receiver.recv().await {
                    Some(AnimationCommand::Wake(app)) => {
                        app_handle = app;
                        active = true;
                    }
                    None => break,
                }
                continue;
            }

            let (blink_enabled, should_continue) = {
                let state = read_tray_usage_state();
                (state.blinking, animation_should_continue(&state))
            };
            if !should_continue {
                active = false;
                continue;
            }

            if blink_enabled != interval_blink_enabled {
                interval = animation_interval(blink_enabled);
                interval_blink_enabled = blink_enabled;
            }

            tokio::select! {
                Some(AnimationCommand::Wake(app)) = receiver.recv() => {
                    app_handle = app;
                }
                _ = interval.tick() => {
                    let should_continue = advance_animation_phase();
                    if should_continue {
                        let _ = update_tray_icon_with_animation(&app_handle, true);
                    } else {
                        active = false;
                    }
                }
            }
        }
    });

    let _ = sender.send(AnimationCommand::Wake(app.clone()));
}

struct TrayUsageState {
    provider_usage: HashMap<ProviderId, UsageSnapshot>,
    disabled_providers: HashSet<ProviderId>,
    loading_count: usize,
    animation_phase: u8,
    blinking: bool,
    theme: Theme,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayDisplayTextMode {
    Percent,
    Pace,
    Both,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TrayPercentWindowMode {
    Session,
    Weekly,
    Highest,
}

#[derive(Clone, Copy)]
struct TrayDisplayTextState {
    enabled: bool,
    mode: TrayDisplayTextMode,
    percent_window_mode: TrayPercentWindowMode,
    show_used: bool,
}

impl Default for TrayUsageState {
    fn default() -> Self {
        Self {
            provider_usage: HashMap::new(),
            disabled_providers: HashSet::new(),
            loading_count: 0,
            animation_phase: 0,
            blinking: false,
            theme: Theme::Light,
        }
    }
}

impl Default for TrayDisplayTextState {
    fn default() -> Self {
        Self {
            enabled: false,
            mode: TrayDisplayTextMode::Percent,
            percent_window_mode: TrayPercentWindowMode::Session,
            show_used: true,
        }
    }
}

#[cfg(test)]
fn reset_tray_usage_state() {
    *write_tray_usage_state() = TrayUsageState::default();
    *write_tray_display_text_state() = TrayDisplayTextState::default();
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TrayStatus {
    Ok,
    Loading,
    Disabled,
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
    animation_phase: u8,
    blink_enabled: bool,
    theme: Theme,
}

impl TrayRenderState {
    fn needs_animation(&self) -> bool {
        matches!(self.status, TrayStatus::Loading) || self.blink_enabled
    }
}

fn sort_usage_rings(rings: &mut Vec<UsageRing>) {
    rings.retain(|ring| ring.percent.is_finite());
    rings.sort_by(|a, b| b.percent.partial_cmp(&a.percent).unwrap_or(Ordering::Equal));
}

struct Canvas {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy)]
struct TrayPalette {
    track: [u8; 4],
    track_disabled: [u8; 4],
    generic_ring: [u8; 4],
    loading_spinner: [u8; 4],
    badge_disabled: [u8; 4],
    badge_stale: [u8; 4],
    badge_error: [u8; 4],
    usage_good: [u8; 4],
    usage_warn: [u8; 4],
    usage_critical: [u8; 4],
    codex_face: [u8; 4],
    codex_eye: [u8; 4],
    codex_mouth: [u8; 4],
    claude_body: [u8; 4],
    claude_claw: [u8; 4],
    claude_eye: [u8; 4],
    gemini: [u8; 4],
    factory_gear: [u8; 4],
    factory_tooth: [u8; 4],
}

fn palette_for_theme(theme: Theme) -> TrayPalette {
    match theme {
        Theme::Dark => TrayPalette {
            track: [224, 224, 224, 220],
            track_disabled: [178, 178, 178, 180],
            generic_ring: [238, 238, 238, 255],
            loading_spinner: [114, 186, 255, 255],
            badge_disabled: [185, 185, 185, 255],
            badge_stale: [248, 193, 102, 255],
            badge_error: [242, 124, 112, 255],
            usage_good: [96, 200, 130, 255],
            usage_warn: [248, 193, 102, 255],
            usage_critical: [242, 124, 112, 255],
            codex_face: [245, 216, 156, 255],
            codex_eye: [30, 30, 30, 255],
            codex_mouth: [110, 72, 60, 255],
            claude_body: [244, 146, 110, 255],
            claude_claw: [224, 122, 90, 255],
            claude_eye: [30, 30, 30, 255],
            gemini: [140, 210, 255, 255],
            factory_gear: [200, 200, 200, 255],
            factory_tooth: [225, 225, 225, 255],
        },
        Theme::Light => TrayPalette {
            track: [190, 190, 190, 220],
            track_disabled: [165, 165, 165, 180],
            generic_ring: [80, 80, 80, 255],
            loading_spinner: [86, 157, 226, 255],
            badge_disabled: [120, 120, 120, 255],
            badge_stale: [234, 167, 77, 255],
            badge_error: [220, 70, 60, 255],
            usage_good: [73, 177, 108, 255],
            usage_warn: [234, 167, 77, 255],
            usage_critical: [220, 85, 73, 255],
            codex_face: [232, 198, 132, 255],
            codex_eye: [42, 42, 42, 255],
            codex_mouth: [90, 60, 50, 255],
            claude_body: [230, 120, 88, 255],
            claude_claw: [200, 92, 68, 255],
            claude_eye: [36, 36, 36, 255],
            gemini: [118, 191, 246, 255],
            factory_gear: [140, 140, 140, 255],
            factory_tooth: [165, 165, 165, 255],
        },
        _ => TrayPalette {
            track: [190, 190, 190, 220],
            track_disabled: [165, 165, 165, 180],
            generic_ring: [80, 80, 80, 255],
            loading_spinner: [86, 157, 226, 255],
            badge_disabled: [120, 120, 120, 255],
            badge_stale: [234, 167, 77, 255],
            badge_error: [220, 70, 60, 255],
            usage_good: [73, 177, 108, 255],
            usage_warn: [234, 167, 77, 255],
            usage_critical: [220, 85, 73, 255],
            codex_face: [232, 198, 132, 255],
            codex_eye: [42, 42, 42, 255],
            codex_mouth: [90, 60, 50, 255],
            claude_body: [230, 120, 88, 255],
            claude_claw: [200, 92, 68, 255],
            claude_eye: [36, 36, 36, 255],
            gemini: [118, 191, 246, 255],
            factory_gear: [140, 140, 140, 255],
            factory_tooth: [165, 165, 165, 255],
        },
    }
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

    fn draw_arc(
        &mut self,
        center_x: f64,
        center_y: f64,
        outer_radius: f64,
        thickness: f64,
        start_fraction: f64,
        sweep_fraction: f64,
        color: [u8; 4],
    ) {
        let inner_radius = (outer_radius - thickness).max(0.0);
        let outer_sq = outer_radius * outer_radius;
        let inner_sq = inner_radius * inner_radius;
        let min_x = (center_x - outer_radius).floor() as i32;
        let max_x = (center_x + outer_radius).ceil() as i32;
        let min_y = (center_y - outer_radius).floor() as i32;
        let max_y = (center_y + outer_radius).ceil() as i32;
        let start = start_fraction.rem_euclid(1.0);
        let sweep = sweep_fraction.clamp(0.0, 1.0);
        let end = (start + sweep).rem_euclid(1.0);

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let dx = (x as f64 + 0.5) - center_x;
                let dy = (y as f64 + 0.5) - center_y;
                let dist_sq = dx * dx + dy * dy;
                if dist_sq > outer_sq || dist_sq < inner_sq {
                    continue;
                }

                let mut angle = dy.atan2(dx) + (PI / 2.0);
                if angle < 0.0 {
                    angle += 2.0 * PI;
                }
                let fraction = angle / (2.0 * PI);

                let in_arc = if sweep >= 1.0 {
                    true
                } else if start <= end {
                    fraction >= start && fraction <= end
                } else {
                    fraction >= start || fraction <= end
                };

                if in_arc {
                    self.set_pixel(x, y, color);
                }
            }
        }
    }
}

pub fn handle_usage_update(
    app: &AppHandle,
    provider_id: ProviderId,
    usage: UsageSnapshot,
) -> Result<()> {
    {
        let mut state = write_tray_usage_state();
        state.provider_usage.insert(provider_id, usage);
    }
    update_tray_icon(app)
}

pub fn set_loading_state(app: &AppHandle, is_loading: bool) -> Result<()> {
    {
        let mut state = write_tray_usage_state();
        if is_loading {
            state.loading_count = state.loading_count.saturating_add(1);
        } else if state.loading_count > 0 {
            state.loading_count -= 1;
        }
    }
    update_tray_icon(app)
}

pub fn set_blinking_state(app: &AppHandle, enabled: bool) -> Result<()> {
    {
        let mut state = write_tray_usage_state();
        state.blinking = enabled;
    }
    update_tray_icon(app)
}

pub fn set_tray_theme(app: &AppHandle, theme: Theme) -> Result<()> {
    {
        let mut state = write_tray_usage_state();
        state.theme = theme;
    }
    update_tray_icon(app)
}

pub fn set_display_text(
    app: &AppHandle,
    enabled: bool,
    mode: TrayDisplayTextMode,
    percent_window_mode: TrayPercentWindowMode,
    show_used: bool,
) -> Result<()> {
    {
        let mut state = write_tray_display_text_state();
        state.enabled = enabled;
        state.mode = mode;
        state.percent_window_mode = percent_window_mode;
        state.show_used = show_used;
    }
    update_tray_icon(app)
}

pub fn set_display_text_for_provider(
    app: &AppHandle,
    display_mode: &str,
    text_enabled: bool,
    text_mode: &str,
    show_used: bool,
) -> Result<()> {
    let percent_window_mode = match display_mode {
        "session" => TrayPercentWindowMode::Session,
        "weekly" => TrayPercentWindowMode::Weekly,
        "highest" => TrayPercentWindowMode::Highest,
        _ => TrayPercentWindowMode::Session,
    };
    let text_mode = match text_mode {
        "pace" => TrayDisplayTextMode::Pace,
        "both" => TrayDisplayTextMode::Both,
        _ => TrayDisplayTextMode::Percent,
    };
    set_display_text(app, text_enabled, text_mode, percent_window_mode, show_used)
}

pub fn set_provider_disabled(
    app: &AppHandle,
    provider_id: ProviderId,
    disabled: bool,
) -> Result<()> {
    {
        let mut state = write_tray_usage_state();
        if disabled {
            state.disabled_providers.insert(provider_id);
        } else {
            state.disabled_providers.remove(&provider_id);
        }
    }
    update_tray_icon(app)
}

fn update_tray_icon(app: &AppHandle) -> Result<()> {
    update_tray_icon_with_animation(app, false)
}

fn should_start_animation_thread(state: &TrayRenderState, from_animation_thread: bool) -> bool {
    state.needs_animation() && !from_animation_thread
}

fn update_tray_icon_with_animation(app: &AppHandle, from_animation_thread: bool) -> Result<()> {
    let tray = {
        let guard = TRAY_HANDLE.lock().unwrap();
        guard.clone()
    };

    let Some(tray) = tray else {
        tracing::warn!("Tray icon not found for updates");
        return Ok(());
    };

    let state = compute_render_state();
    // In dev mode, always render dynamic icon to show purple dev colors
    // In production, only render dynamic icon when loading (for animation)
    #[cfg(debug_assertions)]
    let icon = render_tray_icon(state.clone());
    #[cfg(not(debug_assertions))]
    let icon = if matches!(state.status, TrayStatus::Loading) {
        render_tray_icon(state.clone())
    } else {
        TRAY_ICON_TEMPLATE.clone()
    };
    tray.set_icon(Some(icon))?;
    // In dev mode, disable template mode to show colored icon
    #[cfg(debug_assertions)]
    tray.set_icon_as_template(false)?;
    #[cfg(not(debug_assertions))]
    tray.set_icon_as_template(true)?;
    tray.set_tooltip(Some(build_tray_tooltip()))?;
    tray.set_title(build_tray_title(&state))?;

    if should_start_animation_thread(&state, from_animation_thread) {
        start_tray_animation_thread(app);
    }

    Ok(())
}

fn compute_render_state() -> TrayRenderState {
    let mut rings: Vec<UsageRing> = Vec::new();
    let mut has_error = false;
    let mut has_stale = false;
    let state = read_tray_usage_state();
    let loading_count = state.loading_count;
    let animation_phase = state.animation_phase;
    let blinking = state.blinking;
    let has_disabled = !state.disabled_providers.is_empty();
    let theme = state.theme;
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
                color: usage_color(percent, theme),
                provider_id: *provider_id,
            });
        }
    }

    sort_usage_rings(&mut rings);
    rings.truncate(MAX_RINGS);
    let primary_provider = rings.first().map(|ring| ring.provider_id);

    let status = if loading_count > 0 {
        TrayStatus::Loading
    } else if has_error {
        TrayStatus::Error
    } else if has_stale {
        TrayStatus::Stale
    } else if has_disabled {
        TrayStatus::Disabled
    } else if rings.is_empty() {
        TrayStatus::Disabled
    } else {
        TrayStatus::Ok
    };

    TrayRenderState {
        usage_rings: rings,
        status,
        primary_provider,
        animation_phase,
        blink_enabled: blinking,
        theme,
    }
}

fn build_tray_title(state: &TrayRenderState) -> Option<String> {
    let display_state = read_tray_display_text_state();
    if !display_state.enabled {
        return None;
    }
    format_tray_display_text(state, *display_state)
}

fn format_tray_display_text(
    state: &TrayRenderState,
    display_state: TrayDisplayTextState,
) -> Option<String> {
    let primary_provider = state.primary_provider?;
    let usage_state = read_tray_usage_state();
    let usage = usage_state.provider_usage.get(&primary_provider)?;
    let percent_text = resolve_percent_window(display_state, usage).and_then(|value| {
        if !value.is_finite() {
            return None;
        }
        let clamped = value.clamp(0.0, 100.0);
        let shown = if display_state.show_used {
            clamped
        } else {
            (100.0 - clamped).clamp(0.0, 100.0)
        };
        Some(format!("{:.0}%", shown))
    });

    let pace_text = format_tray_pace_text(primary_provider, usage);

    match display_state.mode {
        TrayDisplayTextMode::Percent => percent_text,
        TrayDisplayTextMode::Pace => pace_text,
        TrayDisplayTextMode::Both => {
            let percent_text = percent_text?;
            let pace_text = pace_text?;
            Some(format!("{} · {}", percent_text, pace_text))
        }
    }
}

fn resolve_percent_window(
    display_state: TrayDisplayTextState,
    usage: &UsageSnapshot,
) -> Option<f64> {
    match display_state.percent_window_mode {
        TrayPercentWindowMode::Session => usage.primary.as_ref().map(|window| window.used_percent),
        TrayPercentWindowMode::Weekly => usage.secondary.as_ref().map(|window| window.used_percent),
        TrayPercentWindowMode::Highest => {
            let mut best: Option<f64> = None;
            for window in [&usage.primary, &usage.secondary, &usage.tertiary] {
                if let Some(window) = window.as_ref() {
                    let percent = window.used_percent;
                    if percent.is_finite() {
                        best = Some(best.map_or(percent, |current| current.max(percent)));
                    }
                }
            }
            best
        }
    }
}

fn format_tray_pace_text(provider_id: ProviderId, usage: &UsageSnapshot) -> Option<String> {
    if !matches!(provider_id, ProviderId::Codex | ProviderId::Claude) {
        return None;
    }
    let window = usage.secondary.as_ref()?;
    let resets_at = window.resets_at.as_ref()?;
    let window_minutes = window.window_minutes?;
    if window_minutes <= 0 {
        return None;
    }
    let parsed = DateTime::parse_from_rfc3339(resets_at).ok()?;
    let now = Utc::now();
    let resets_at = parsed.with_timezone(&Utc);
    let duration_minutes = window_minutes as f64;
    if duration_minutes <= 0.0 {
        return None;
    }
    let duration_seconds = duration_minutes * 60.0;
    let time_until_reset = (resets_at - now).num_seconds() as f64;
    if time_until_reset <= 0.0 || time_until_reset > duration_seconds {
        return None;
    }
    let elapsed = (duration_seconds - time_until_reset).clamp(0.0, duration_seconds);
    let expected_used_percent = (elapsed / duration_seconds) * 100.0;
    if expected_used_percent < 3.0 {
        return None;
    }
    let actual_used_percent = window.used_percent.clamp(0.0, 100.0);
    let remaining_percent = 100.0 - actual_used_percent;
    if remaining_percent <= 0.0 {
        return None;
    }
    if elapsed == 0.0 && actual_used_percent > 0.0 {
        return None;
    }
    let delta = actual_used_percent - expected_used_percent;
    let sign = if delta >= 0.0 { "+" } else { "-" };
    let delta_value = delta.abs().round();
    Some(format!("{}{}%", sign, delta_value))
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
    let palette = palette_for_theme(state.theme);
    let track_color = if matches!(state.status, TrayStatus::Disabled) {
        palette.track_disabled
    } else {
        palette.track
    };

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

    if !matches!(state.status, TrayStatus::Disabled) {
        draw_provider_icon(&mut canvas, state.primary_provider, center, palette);
    } else {
        draw_generic_ring(&mut canvas, center, palette);
    }

    if matches!(state.status, TrayStatus::Loading) {
        let phase = (state.animation_phase % 4) as f64;
        let start_fraction = (phase * 0.25) % 1.0;
        canvas.draw_arc(
            center.0,
            center.1,
            outer_radius - 0.5,
            RING_THICKNESS,
            start_fraction,
            0.25,
            palette.loading_spinner,
        );
    }

    let blink_off = state.blink_enabled && state.animation_phase % 2 == 1;
    let badge_color = if blink_off {
        None
    } else {
        match state.status {
            TrayStatus::Ok => None,
            TrayStatus::Loading => Some(palette.loading_spinner),
            TrayStatus::Disabled => Some(palette.badge_disabled),
            TrayStatus::Stale => Some(palette.badge_stale),
            TrayStatus::Error => Some(palette.badge_error),
        }
    };

    if let Some(color) = badge_color {
        canvas.draw_filled_circle(ICON_SIZE as f64 - 6.0, 6.0, 3.0, color);
    }

    // In dev mode, always draw a purple DEV indicator badge in the bottom-left
    #[cfg(debug_assertions)]
    {
        let dev_badge_color: [u8; 4] = [180, 80, 220, 255]; // Bright purple
        canvas.draw_filled_circle(6.0, ICON_SIZE as f64 - 6.0, 4.0, dev_badge_color);
    }

    Image::new_owned(canvas.pixels, ICON_SIZE, ICON_SIZE)
}

fn draw_provider_icon(
    canvas: &mut Canvas,
    provider: Option<ProviderId>,
    center: (f64, f64),
    palette: TrayPalette,
) {
    match provider {
        Some(ProviderId::Codex) => draw_codex_face(canvas, center, palette),
        Some(ProviderId::Claude) => draw_claude_crab(canvas, center, palette),
        Some(ProviderId::Gemini) => draw_gemini_sparkle(canvas, center, palette),
        Some(ProviderId::Factory) => draw_factory_gear(canvas, center, palette),
        _ => draw_generic_ring(canvas, center, palette),
    }
}

fn draw_codex_face(canvas: &mut Canvas, center: (f64, f64), palette: TrayPalette) {
    let face_color = palette.codex_face;
    let eye_color = palette.codex_eye;
    let mouth_color = palette.codex_mouth;
    canvas.draw_filled_circle(center.0, center.1, 6.0, face_color);
    canvas.set_pixel(center.0 as i32 - 2, center.1 as i32 - 1, eye_color);
    canvas.set_pixel(center.0 as i32 + 2, center.1 as i32 - 1, eye_color);
    for x in (center.0 as i32 - 2)..=(center.0 as i32 + 2) {
        canvas.set_pixel(x, center.1 as i32 + 2, mouth_color);
    }
}

fn draw_claude_crab(canvas: &mut Canvas, center: (f64, f64), palette: TrayPalette) {
    let body_color = palette.claude_body;
    let claw_color = palette.claude_claw;
    let eye_color = palette.claude_eye;
    canvas.draw_filled_circle(center.0, center.1 + 1.0, 4.5, body_color);
    canvas.draw_filled_circle(center.0 - 5.0, center.1 - 2.0, 2.0, claw_color);
    canvas.draw_filled_circle(center.0 + 5.0, center.1 - 2.0, 2.0, claw_color);
    canvas.set_pixel(center.0 as i32 - 1, center.1 as i32 - 1, eye_color);
    canvas.set_pixel(center.0 as i32 + 1, center.1 as i32 - 1, eye_color);
}

fn draw_gemini_sparkle(canvas: &mut Canvas, center: (f64, f64), palette: TrayPalette) {
    let sparkle_color = palette.gemini;
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

fn draw_factory_gear(canvas: &mut Canvas, center: (f64, f64), palette: TrayPalette) {
    let gear_color = palette.factory_gear;
    let tooth_color = palette.factory_tooth;
    canvas.draw_ring(center.0, center.1, 6.0, 2.0, gear_color, None, None);
    canvas.draw_square(center.0 as i32, center.1 as i32 - 7, 2, tooth_color);
    canvas.draw_square(center.0 as i32, center.1 as i32 + 7, 2, tooth_color);
    canvas.draw_square(center.0 as i32 - 7, center.1 as i32, 2, tooth_color);
    canvas.draw_square(center.0 as i32 + 7, center.1 as i32, 2, tooth_color);
}

fn draw_generic_ring(canvas: &mut Canvas, center: (f64, f64), palette: TrayPalette) {
    let ring_color = palette.generic_ring;
    canvas.draw_ring(center.0, center.1, 6.0, 2.0, ring_color, None, None);
}

fn usage_color(percent: f64, theme: Theme) -> [u8; 4] {
    let palette = palette_for_theme(theme);
    if percent < 50.0 {
        palette.usage_good
    } else if percent < 80.0 {
        palette.usage_warn
    } else {
        palette.usage_critical
    }
}

fn provider_display_name(provider_id: ProviderId) -> &'static str {
    match provider_id {
        ProviderId::Claude => "Claude",
        ProviderId::Codex => "Codex",
        ProviderId::Cursor => "Cursor",
        ProviderId::Copilot => "Copilot",
        ProviderId::Gemini => "Gemini",
        ProviderId::Antigravity => "Antigravity",
        ProviderId::Factory => "Droid",
        ProviderId::Zai => "z.ai",
        ProviderId::Minimax => "MiniMax",
        ProviderId::Kimi => "Kimi",
        ProviderId::KimiK2 => "Kimi K2",
        ProviderId::Kiro => "Kiro",
        ProviderId::Vertex => "Vertex AI",
        ProviderId::Augment => "Augment",
        ProviderId::Amp => "Amp",
        ProviderId::Jetbrains => "JetBrains AI",
        ProviderId::Opencode => "OpenCode",
        ProviderId::Synthetic => "Synthetic",
    }
}

fn build_tray_tooltip() -> String {
    let state = read_tray_usage_state();
    format_tray_tooltip(&state)
}

fn format_tray_tooltip(state: &TrayUsageState) -> String {
    let mut entries: Vec<(f64, ProviderId)> = Vec::new();
    let mut error_entries: Vec<ProviderId> = Vec::new();

    for (provider_id, usage) in state.provider_usage.iter() {
        if usage.error.is_some() {
            error_entries.push(*provider_id);
            continue;
        }
        if let Some(percent) = usage_percent_from_snapshot(usage) {
            entries.push((percent, *provider_id));
        }
    }

    entries.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(Ordering::Equal));

    let mut summary_parts: Vec<String> = Vec::new();
    for (percent, provider_id) in entries.iter().take(MAX_RINGS) {
        summary_parts.push(format!(
            "{} {:.0}%",
            provider_display_name(*provider_id),
            percent
        ));
    }

    for provider_id in error_entries {
        summary_parts.push(format!("{} error", provider_display_name(provider_id)));
    }

    if summary_parts.is_empty() {
        TRAY_TOOLTIP_BASE.to_string()
    } else {
        format!("{} - {}", TRAY_TOOLTIP_BASE, summary_parts.join(" • "))
    }
}

/// Set up the system tray icon
pub fn setup_tray(app: &AppHandle) -> Result<()> {
    tracing::info!("Setting up tray icon...");
    tracing::info!("Building tray icon with initial state...");
    
    // In dev mode, render initial icon with purple dev colors
    #[cfg(debug_assertions)]
    let initial_icon = render_tray_icon(TrayRenderState {
        usage_rings: Vec::new(),
        status: TrayStatus::Ok,
        primary_provider: None,
        animation_phase: 0,
        blink_enabled: false,
        theme: Theme::Dark,
    });
    #[cfg(not(debug_assertions))]
    let initial_icon = TRAY_ICON_TEMPLATE.clone();
    
    tracing::info!(
        "Loaded tray icon: {}x{}",
        initial_icon.width(),
        initial_icon.height()
    );
    let refresh_item = MenuItemBuilder::new("Refresh")
        .id(TRAY_REFRESH_MENU_ID)
        .build(app)?;
    let tray_menu = Menu::with_items(app, &[&refresh_item])?;

    // In dev mode, disable template mode to show colored icon
    #[cfg(debug_assertions)]
    let use_template = false;
    #[cfg(not(debug_assertions))]
    let use_template = true;

    let tray = TrayIconBuilder::with_id(TRAY_ICON_ID)
        .tooltip(TRAY_TOOLTIP_BASE)
        .icon(initial_icon)
        .icon_as_template(use_template)
        .menu(&tray_menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| {
            if event.id().as_ref() == TRAY_REFRESH_MENU_ID {
                let _ = app.emit("refresh-requested", ());
            }
        })
        .on_tray_icon_event(|tray, event| {
            match event {
                TrayIconEvent::DoubleClick {
                    button: MouseButton::Left,
                    ..
                } => {
                    tracing::info!("Tray icon double-clicked - opening settings");
                    let app = tray.app_handle().clone();
                    std::thread::spawn(move || {
                        if let Err(e) = create_settings_window(&app) {
                            tracing::error!("Failed to open settings window: {}", e);
                        }
                    });
                }
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                } => {
                    // Forward click events to the positioner plugin for tray rect tracking
                    // Only forward on actual clicks to avoid panics during initialization
                    tauri_plugin_positioner::on_tray_event(tray.app_handle(), &event);
                    
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

    tracing::info!("Tray icon build complete");

    let mut tray_guard = TRAY_HANDLE.lock().unwrap();
    *tray_guard = Some(tray);

    update_tray_icon(app)?;
    start_random_blinking_loop(app);
    tracing::info!("Tray icon created");
    Ok(())
}

fn start_random_blinking_loop(app: &AppHandle) {
    let app_handle = app.clone();
    std::thread::spawn(move || loop {
        // Check shutdown signal
        if SHUTDOWN_REQUESTED.load(AtomicOrdering::SeqCst) {
            tracing::debug!("Random blinking thread shutting down");
            break;
        }

        let mut rng = rand::thread_rng();
        let jitter = rng.gen_range(0..=RANDOM_BLINK_VARIANCE_MS);
        std::thread::sleep(std::time::Duration::from_millis(
            RANDOM_BLINK_INTERVAL_MS + jitter,
        ));

        // Check shutdown signal after sleep
        if SHUTDOWN_REQUESTED.load(AtomicOrdering::SeqCst) {
            tracing::debug!("Random blinking thread shutting down");
            break;
        }

        if !debug_settings::random_blink_enabled() {
            continue;
        }

        let mut state = write_tray_usage_state();
        if state.loading_count > 0 {
            continue;
        }
        state.blinking = true;

        let _ = update_tray_icon(&app_handle);

        std::thread::sleep(std::time::Duration::from_millis(900));

        let mut state = write_tray_usage_state();
        if state.loading_count == 0 {
            state.blinking = false;
        }

        let _ = update_tray_icon(&app_handle);
    });
}

/// Create the popup window (hidden by default)
pub fn create_popup_window(app: &AppHandle) -> Result<()> {
    tracing::info!("Creating popup window...");
    eprintln!("About to build WebviewWindow...");
    
    // In dev mode, use the external URL; in production, use the app URL
    #[cfg(debug_assertions)]
    let url = WebviewUrl::External("http://localhost:1420".parse().unwrap());
    #[cfg(not(debug_assertions))]
    let url = WebviewUrl::App("index.html".into());
    
    let builder = WebviewWindowBuilder::new(app, "popup", url)
        .title("IncuBar")
        .inner_size(320.0, 420.0)
        .resizable(false)
        .visible(false)
        .decorations(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .focused(true);
    
    eprintln!("Builder created, now calling build()...");
    
    let window = match builder.build()
    {
        Ok(w) => {
            eprintln!("Window build succeeded!");
            w
        },
        Err(e) => {
            eprintln!("Window build FAILED: {:?}", e);
            tracing::error!("Failed to create popup window: {:?}", e);
            return Err(e.into());
        }
    };

    if let Ok(theme) = window.theme() {
        let _ = set_tray_theme(app, theme);
    }

    let app_handle = app.clone();
    let window_clone = window.clone();
    window.on_window_event(move |event| {
        match event {
            WindowEvent::ThemeChanged(theme) => {
                let _ = set_tray_theme(&app_handle, *theme);
            }
            WindowEvent::Focused(false) => {
                // Hide popup when clicking outside (losing focus)
                tracing::debug!("Popup lost focus, hiding");
                let _ = window_clone.hide();
            }
            _ => {}
        }
    });

    // Open devtools in debug mode
    #[cfg(debug_assertions)]
    window.open_devtools();

    tracing::info!("Popup window created");
    Ok(())
}

/// Create or focus the settings window
pub fn create_settings_window(app: &AppHandle) -> Result<()> {
    if let Some(existing) = app.get_webview_window("settings") {
        existing.show()?;
        existing.set_focus()?;
        return Ok(());
    }

    #[cfg(debug_assertions)]
    let url = WebviewUrl::External("http://localhost:1420?view=settings".parse().unwrap());
    #[cfg(not(debug_assertions))]
    let url = WebviewUrl::App("index.html?view=settings".into());

    let window = WebviewWindowBuilder::new(app, "settings", url)
        .title("IncuBar Settings")
        .inner_size(920.0, 720.0)
        .min_inner_size(760.0, 600.0)
        .resizable(true)
        .visible(true)
        .decorations(true)
        .always_on_top(false)
        .skip_taskbar(false)
        .focused(true)
        .center()
        .build()?;

    window.show()?;
    window.set_focus()?;
    tracing::info!("Settings window created");

    if let Ok(theme) = window.theme() {
        let _ = set_tray_theme(app, theme);
    }

    let app_handle = app.clone();
    window.on_window_event(move |event| {
        if let WindowEvent::ThemeChanged(theme) = event {
            let _ = set_tray_theme(&app_handle, *theme);
        }
    });

    #[cfg(debug_assertions)]
    window.open_devtools();

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
            // Use the positioner plugin to position at tray center.
            // Guard against missing tray icon to avoid plugin panic.
            // The positioner plugin can panic if the tray rect is not available yet,
            // so we use catch_unwind to handle this gracefully.
            if app.tray_by_id(TRAY_ICON_ID).is_some() {
                let win_ref = window.as_ref().window().clone();
                let position_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    win_ref.move_window(Position::TrayCenter)
                }));
                
                match position_result {
                    Ok(Ok(())) => {}
                    Ok(Err(e)) => {
                        tracing::warn!(
                            "Failed to position at TrayCenter: {}, trying TrayBottomCenter",
                            e
                        );
                        // Fallback to TrayBottomCenter if TrayCenter fails
                        let win_ref = window.as_ref().window().clone();
                        let fallback_result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                            win_ref.move_window(Position::TrayBottomCenter)
                        }));
                        if let Ok(Err(e2)) = fallback_result {
                            tracing::error!("Failed to position popup: {}", e2);
                        }
                    }
                    Err(_) => {
                        tracing::warn!("Positioner panicked (tray rect not available); showing popup at current position");
                    }
                }
            } else {
                tracing::warn!("Tray icon not ready; skipping positioner move");
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
        advance_animation_phase, animation_interval, animation_should_continue, animation_tick_ms,
        compute_render_state, format_tray_tooltip, palette_for_theme, read_tray_usage_state,
        render_tray_icon, reset_tray_usage_state, should_start_animation_thread,
        sort_usage_rings, write_tray_usage_state, TrayRenderState, TrayStatus, UsageRing,
        BLINKING_ANIMATION_TICK_MS, ICON_SIZE, LOADING_ANIMATION_TICK_MS, STALE_THRESHOLD_SECS,
    };
    use crate::providers::{ProviderId, RateWindow, UsageSnapshot};
    use std::collections::HashMap;
    use std::time::Duration;
    use tauri::Theme;
    use tokio::time;

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
            animation_phase: 0,
            blink_enabled: false,
            theme: Theme::Light,
        });
        assert_eq!(icon.width(), ICON_SIZE);
        assert_eq!(icon.height(), ICON_SIZE);
        assert_eq!(icon.rgba().len(), (ICON_SIZE * ICON_SIZE * 4) as usize);
    }

    #[test]
    fn compute_render_state_uses_max_usage_and_error() {
        reset_tray_usage_state();
        let mut guard = write_tray_usage_state();
        guard.provider_usage = HashMap::from([
            (ProviderId::Claude, sample_usage(33.0)),
            (ProviderId::Codex, sample_usage(81.0)),
        ]);
        drop(guard);

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Ok);
        assert_eq!(state.animation_phase, 0);
        assert!(!state.blink_enabled);
        assert_eq!(state.usage_rings.len(), 2);
        assert_eq!(
            state.usage_rings.first().map(|ring| ring.percent),
            Some(81.0)
        );
        assert_eq!(state.primary_provider, Some(ProviderId::Codex));
    }

    #[test]
    fn sort_usage_rings_filters_nan_values() {
        let mut rings = vec![
            UsageRing {
                percent: f64::NAN,
                color: [0, 0, 0, 0],
                provider_id: ProviderId::Claude,
            },
            UsageRing {
                percent: 42.0,
                color: [0, 0, 0, 0],
                provider_id: ProviderId::Codex,
            },
        ];

        sort_usage_rings(&mut rings);

        assert_eq!(rings.len(), 1);
        assert_eq!(rings[0].percent, 42.0);
        assert_eq!(rings[0].provider_id, ProviderId::Codex);
    }

    #[test]
    fn compute_render_state_marks_error_status() {
        reset_tray_usage_state();
        let mut guard = write_tray_usage_state();
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
        let mut guard = write_tray_usage_state();
        guard.provider_usage = HashMap::from([(
            ProviderId::Claude,
            sample_usage_with_time(55.0, &stale_time),
        )]);
        drop(guard);

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Stale);
    }

    #[test]
    fn compute_render_state_marks_loading_status() {
        reset_tray_usage_state();
        let mut guard = write_tray_usage_state();
        guard.loading_count = 1;
        drop(guard);

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Loading);
    }

    #[test]
    fn compute_render_state_marks_disabled_status() {
        reset_tray_usage_state();

        let state = compute_render_state();
        assert_eq!(state.status, TrayStatus::Disabled);
    }

    #[test]
    fn render_tray_icon_blinks_when_enabled() {
        reset_tray_usage_state();
        let steady_icon = render_tray_icon(TrayRenderState {
            usage_rings: Vec::new(),
            status: TrayStatus::Disabled,
            primary_provider: None,
            animation_phase: 0,
            blink_enabled: false,
            theme: Theme::Light,
        });
        let blinking_icon = render_tray_icon(TrayRenderState {
            usage_rings: Vec::new(),
            status: TrayStatus::Disabled,
            primary_provider: None,
            animation_phase: 1,
            blink_enabled: true,
            theme: Theme::Light,
        });

        let idx = ((6 * ICON_SIZE + (ICON_SIZE - 6)) * 4) as usize;
        let steady_alpha = steady_icon.rgba()[idx + 3];
        let blinking_alpha = blinking_icon.rgba()[idx + 3];
        assert_eq!(steady_alpha, 255);
        assert_eq!(blinking_alpha, 180);
    }

    #[test]
    fn render_tray_icon_uses_dark_palette() {
        reset_tray_usage_state();
        let icon = render_tray_icon(TrayRenderState {
            usage_rings: Vec::new(),
            status: TrayStatus::Disabled,
            primary_provider: None,
            animation_phase: 0,
            blink_enabled: false,
            theme: Theme::Dark,
        });
        let palette = palette_for_theme(Theme::Dark);
        let center = (ICON_SIZE / 2) as usize;
        let idx = ((center - 6) * ICON_SIZE as usize + center) * 4;
        let pixel = &icon.rgba()[idx..idx + 4];
        assert_eq!(pixel, palette.generic_ring);
    }

    #[test]
    fn format_tray_tooltip_includes_top_usage_and_errors() {
        reset_tray_usage_state();
        let mut guard = write_tray_usage_state();
        guard.provider_usage = HashMap::from([
            (ProviderId::Codex, sample_usage(72.0)),
            (ProviderId::Claude, sample_usage(12.0)),
        ]);
        let mut error_usage = sample_usage(40.0);
        error_usage.error = Some("broken".to_string());
        guard.provider_usage.insert(ProviderId::Cursor, error_usage);
        drop(guard);

        let state = read_tray_usage_state();
        let tooltip = format_tray_tooltip(&state);
        assert!(tooltip.starts_with("IncuBar - AI Usage Tracker - "));
        assert!(tooltip.contains("Codex 72%"));
        assert!(tooltip.contains("Claude 12%"));
        assert!(tooltip.contains("Cursor error"));
    }

    #[test]
    fn animation_tick_ms_matches_expected_intervals() {
        assert_eq!(animation_tick_ms(false), LOADING_ANIMATION_TICK_MS);
        assert_eq!(animation_tick_ms(true), BLINKING_ANIMATION_TICK_MS);
    }

    #[tokio::test]
    async fn animation_interval_waits_for_first_tick() {
        let mut interval = animation_interval(false);
        let timeout_result = time::timeout(Duration::from_millis(5), interval.tick()).await;
        assert!(timeout_result.is_err());
        interval.tick().await;
    }

    #[test]
    fn animation_should_continue_checks_loading_or_blinking() {
        reset_tray_usage_state();
        let guard = read_tray_usage_state();
        assert!(!animation_should_continue(&guard));
        drop(guard);

        let mut guard = write_tray_usage_state();
        guard.loading_count = 1;
        drop(guard);
        let guard = read_tray_usage_state();
        assert!(animation_should_continue(&guard));
        drop(guard);

        let mut guard = write_tray_usage_state();
        guard.loading_count = 0;
        guard.blinking = true;
        drop(guard);
        let guard = read_tray_usage_state();
        assert!(animation_should_continue(&guard));
    }

    #[test]
    fn advance_animation_phase_updates_phase_and_continuation() {
        reset_tray_usage_state();
        let mut guard = write_tray_usage_state();
        guard.loading_count = 1;
        guard.animation_phase = 0;
        drop(guard);

        assert!(advance_animation_phase());
        let guard = read_tray_usage_state();
        assert_eq!(guard.animation_phase, 1);
    }

    #[test]
    fn should_start_animation_thread_ignores_animation_updates() {
        reset_tray_usage_state();
        let state = TrayRenderState {
            usage_rings: Vec::new(),
            status: TrayStatus::Loading,
            primary_provider: None,
            animation_phase: 0,
            blink_enabled: false,
            theme: Theme::Light,
        };

        assert!(should_start_animation_thread(&state, false));
        assert!(!should_start_animation_thread(&state, true));
    }

    #[test]
    fn poisoned_tray_state_recovers_on_read() {
        reset_tray_usage_state();
        let _ = std::panic::catch_unwind(|| {
            let _guard = write_tray_usage_state();
            panic!("poison state");
        });
        let guard = read_tray_usage_state();
        assert!(guard.provider_usage.is_empty());
    }
}
