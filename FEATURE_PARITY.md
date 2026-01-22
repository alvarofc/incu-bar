# IncuBar Feature Parity Checklist (CodexBar)

This is a thorough, structured list of features to reach parity with the original CodexBar macOS menu bar app. It is organized by capability area so gaps can be mapped to ownership and milestones.

## 1. Core app shell and lifecycle
- Menu bar only app behavior (no dock icon by default, activate on click)
- Single instance enforcement
- Startup behavior on login
- Graceful shutdown and relaunch behavior
- Background refresh loop with pause when system sleeps
- Resume refresh loop on wake
- Safe handling of app updates while running
- Crash recovery and state restoration
- Local logging with debug/trace levels

## 2. Tray/menu bar behavior
- Tray icon presence on app start
- Left click opens popup window anchored to tray
- Click outside dismisses popup
- Escape key closes popup
- Optional right click menu (if present in CodexBar)
- Tray icon reflects state (fresh, stale, error, disabled)
- Dynamic tray icon rendering based on usage percent
- High DPI icons and platform-specific assets
- Adaptive icon color for dark/light menu bar
- Tooltip showing quick status summary

## 3. Popup UI and navigation
- Main popup layout matches CodexBar hierarchy
- Provider tabs (only when 2+ providers active)
- Provider card for single provider state
- Empty state onboarding for zero providers
- Loading states with spinners
- Error states with contextual guidance
- Last updated timestamp shown per provider
- Manual refresh control for all providers
- Settings entry point
- Keyboard navigation for tabs and controls

## 4. Provider coverage and parity
- Claude
- Codex
- Cursor
- GitHub Copilot
- Gemini
- Kiro
- JetBrains AI
- Vertex AI
- Factory
- Amp (Sourcegraph)
- Augment
- z.ai
- MiniMax
- Kimi
- Kimi K2
- Antigravity
- OpenCode
- Any other CodexBar-supported providers

## 5. Provider authentication flows
- OAuth for providers that use OAuth (system browser login)
- Device code flow where required (Copilot)
- CLI config parsing for providers with local CLI auth
- Cookie import for web-based providers
- Browser-specific cookie import with Chrome default
- Manual cookie input fallback
- Local config file detection (JetBrains, Antigravity, etc)
- API key entry flow where required
- Secure credential storage (keychain/keyring)
- Credential reset and re-auth flow
- Status display for connected/disconnected
- Error reporting for auth failures

## 6. Usage metrics and calculations
- Primary usage window (current plan cycle)
- Secondary usage window (weekly/monthly)
- Tertiary usage window where available
- Support for window minutes and reset timestamps
- Reset description string for each window
- Credits remaining/total and units
- Cost tracking (today and monthly)
- Tokens remaining and display formatting
- Percent calculations based on provider rules
- Correct mapping of provider fields to UI
- Strict separation of identity/plan data between providers
- Provider-specific parsing of response formats

## 7. Identity and account details
- Display plan name when available
- Display email/organization when available
- Display provider-specific account name
- Hide identity fields when unauthenticated
- Avoid cross-provider identity leakage

## 8. Refresh behavior and caching
- Manual refresh per provider
- Refresh all providers
- Auto refresh interval setting
- Backoff on repeated failures
- Cache last successful usage result
- Refresh triggered on popup open
- Persisted last refresh timestamp
- Refresh loop respects enabled providers
- Rate limit error handling

## 9. Settings and preferences
- Provider enable/disable toggles
- Provider order and ordering UI
- Display mode (merged vs per-provider)
- Refresh interval options
- Show credits toggle
- Show cost toggle
- Notification toggle
- Launch at login toggle
- Reset settings to defaults
- Version display
- Persisted settings storage

## 10. Notifications and alerts
- Usage threshold alerts
- Credits running low notifications
- Failed refresh alerts (optional)
- Success notifications (optional)
- Respect OS notification permission
- Notification throttling

## 11. Data storage and security
- Encrypted storage for tokens/cookies
- Secure delete on disconnect
- Minimal local caching of usage
- No sensitive data in logs by default
- Clear separation between settings and secrets

## 12. Parsing and provider adapters
- CLI log parsing (Claude, Codex, Kiro)
- JSON API responses parsing
- HTML or web response parsing where needed
- Robust error handling for malformed responses
- Schema drift handling and fallbacks
- Provider-specific rate window labels
- Provider-specific plan naming

## 13. Icon rendering and branding
- Provider icons for all supported services
- Accurate icon colors and shapes
- Tray icon renderer with usage ring segments
- Multiple segment rendering for tiers
- Disabled/unavailable state visuals
- High contrast for low usage remaining

## 14. Accessibility and UI polish
- Keyboard navigable controls
- Focus-visible styles for all interactive elements
- ARIA labels for icon-only buttons
- Live regions for async status
- Text truncation for long provider names/emails
- Reduced motion support

## 15. Localization and formatting
- Locale-aware number formatting
- Currency formatting with Intl
- Date/time formatting with Intl
- Units formatting with proper spacing
- Support for 12/24h preferences if applicable

## 16. Error handling and resilience
- Clear user-facing error messages
- Network errors with retry guidance
- Auth errors with next steps
- Provider-specific error messages
- Guard against partial data
- Fail gracefully when a provider is missing

## 17. Packaging and release flow
- App packaging pipeline
- Signing and notarization (macOS)
- Appcast generation (if using Sparkle)
- Versioning alignment with CodexBar
- Release notes generation
- Update feed validation

## 18. Diagnostics and support
- Internal debug panel (if present in CodexBar)
- Export logs for support
- Hidden developer toggles
- Version and build metadata display

## 19. Tests and fixtures
- Unit tests for usage parsing
- Fixtures for provider responses
- Tests for error handling
- Snapshot tests for icon rendering (if used)
- Regression tests for major providers

## 20. Cross-platform specifics (Tauri parity)
- Windows tray and notification parity
- Linux tray behavior parity
- Platform-specific file paths for credentials
- Browser cookie access on Windows/Linux
- Window sizing and DPI scaling
- Auto-start behavior per OS

## 21. Parity verification checklist
- Compare CodexBar UI screens 1:1
- Confirm all providers match data fields
- Compare refresh timing with original
- Validate cost and credits calculations
- Confirm settings default values
- Validate popup size and padding
- Confirm onboarding flow matches original intent
