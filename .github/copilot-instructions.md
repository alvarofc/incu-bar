# GitHub Copilot Instructions for IncuBar

## Project Overview

IncuBar is an AI usage tracker for Claude, Codex, Cursor, and other AI assistants. It's a cross-platform desktop application built with Tauri, providing a native shell with a React + TypeScript UI. This project is a port of the original CodexBar macOS Swift app, with feature parity being tracked in `FEATURE_PARITY.md`.

## Technology Stack

### Frontend
- **Framework**: React 19 with TypeScript
- **Build Tool**: Vite
- **Styling**: TailwindCSS v4
- **State Management**: Zustand
- **UI Icons**: Lucide React
- **Date Handling**: date-fns

### Backend
- **Framework**: Tauri v2
- **Language**: Rust
- **Plugins**: 
  - `@tauri-apps/plugin-fs` - File system access
  - `@tauri-apps/plugin-http` - HTTP requests
  - `@tauri-apps/plugin-notification` - System notifications
  - `@tauri-apps/plugin-store` - Persistent storage
  - `@tauri-apps/plugin-updater` - Auto-updates

### Package Manager
- **Bun**: Used for all dependency management and scripts

### Supported Platforms
- macOS (arm64 + x64)
- Windows (x64)
- Linux (x64)

## Architecture

### Frontend Structure
- `src/App.tsx` - Main application component with event listeners and state management
- `src/components/` - React components (PopupWindow, SettingsPanel, ProviderTabs, etc.)
- `src/stores/` - Zustand state stores (usageStore, settingsStore)
- `src/lib/` - Utility libraries (providers, notifications, staleness tracking, etc.)
- `src/styles/` - Global CSS styles

### Backend Structure
- `src-tauri/src/main.rs` - Application entry point
- `src-tauri/src/lib.rs` - Library setup
- `src-tauri/src/commands/` - Tauri commands exposed to frontend
- `src-tauri/src/providers/` - Provider-specific implementations
- `src-tauri/src/tray/` - System tray functionality
- `src-tauri/src/storage/` - Data persistence and security
- `src-tauri/src/browser_cookies.rs` - Browser cookie extraction for auth

## Development Workflow

### Setup
```bash
bun install
```

### Development Server
**IMPORTANT**: The Tauri development server is often already running. Do NOT attempt to start it again with `bun run tauri dev` if it's already running. The app will hot-reload automatically when you make changes.

To start if needed:
```bash
bun run tauri dev
```

### Building
```bash
bun run tauri build
```

For macOS with DMG:
```bash
bun run tauri:build
```

### Type Checking
```bash
bun run lint
```

## Testing

### Test Suite
The project uses a custom Node.js-based test suite. Tests are located in the `tests/` directory and are plain `.cjs` files. All tests are run sequentially via the `test` script in `package.json`.

Run all tests:
```bash
bun run test
```

### Test Coverage Areas
- Feature parity with CodexBar
- Cookie source extraction
- Provider settings pane functionality
- Menu bar display options
- Usage tracking and notifications
- Settings persistence and migration
- Crash recovery
- Auto-update functionality
- Provider authentication flows

### Writing Tests
- Tests should validate behavior matches CodexBar spec
- Use CommonJS format (`.cjs` extension)
- Tests are assertion-based and throw errors on failure
- Each test file should be self-contained

## Release Process

### Automatic Releases
Pushes to `main` trigger the release workflow (`.github/workflows/release-on-main.yml`), which:
1. Determines the next version based on commit messages:
   - `breaking:` or `BREAKING CHANGE:` → major version bump
   - `feat:` → minor version bump
   - Other non-docs/chore/test commits → patch version bump
2. Skips releasing when commits since the last tag are only `docs:`, `chore:`, or `test:`
3. Builds installers for all supported platforms
4. Creates a GitHub Release with the version tag

### Manual Releases
Create a GitHub Release with a `vX.Y.Z` tag to trigger the release workflow.

## Code Style and Best Practices

### TypeScript
- Use strict TypeScript with no implicit any
- Prefer functional components with hooks
- Use type imports: `import type { TypeName } from './module'`
- Define interfaces for all data structures
- Use Zod for runtime type validation where needed

### React
- Use hooks (useState, useEffect, useMemo, useCallback)
- Store global state in Zustand stores
- Keep components focused and composable
- Handle cleanup in useEffect return functions
- Listen to Tauri events properly (see App.tsx for patterns)

### Rust
- Follow standard Rust conventions
- Use `#[tauri::command]` for functions exposed to frontend
- Handle errors properly with Result types
- Keep sensitive data operations in the backend
- Use async/await for I/O operations

### State Management
- Use `useUsageStore` for usage data and provider status
- Use `useSettingsStore` for user preferences and settings
- Stores should persist to disk automatically via Tauri plugin-store
- Initialize stores with defaults and handle hydration

## Important Notes and Constraints

### Feature Parity
- **CRITICAL**: This project maintains feature parity with CodexBar
- Reference `FEATURE_PARITY.md` for the complete parity checklist
- New features must be added to the parity document before implementation
- A feature is "Done" only when behavior matches CodexBar in menu bar, popup, and notifications

### Development Server
- The Tauri dev server is typically already running
- Do NOT restart it unnecessarily
- Changes hot-reload automatically
- Check `AGENTS.md` for current server status notes

### Security
- Credentials are stored in system keyring via `src-tauri/src/storage/keyring.rs`
- Browser cookies are extracted securely
- Never commit secrets or credentials
- Use secure deletion for sensitive data

### Provider Support
- Each provider has its own implementation in `src-tauri/src/providers/`
- Providers must return normalized fields: plan/tier, used, limit, reset
- Auth methods must match CodexBar's fallback strategies
- Errors surface in the popup and log in debug mode

### Notifications
- Follow CodexBar rules for thresholds, failures, and resets
- Notification types: usage, low credits, refresh failure, stale data
- Respect user preferences from settings
- See `src/lib/notifications.ts` for implementation

### Tray Icon
- States: normal, loading, error, disabled, stale
- Animations: blink, wiggle, tilt
- Must match CodexBar behavior exactly

## Common Tasks

### Adding a New Provider
1. Add entry to `FEATURE_PARITY.md`
2. Create provider implementation in `src-tauri/src/providers/`
3. Add provider definition to `src/lib/providers.ts`
4. Add icon to `src/components/ProviderIcons.tsx`
5. Update types in `src/lib/types.ts`
6. Write parity tests
7. Test auth flow, usage tracking, and settings

### Updating Dependencies
- Use `bun install <package>` for JS/TS dependencies
- Update Cargo.toml for Rust dependencies
- Run tests after updates
- Check for breaking changes

### Debugging
- Frontend: Use browser DevTools (right-click in dev mode)
- Backend: Check Rust console output
- Enable debug mode in settings for verbose logging
- Check system logs for crash reports

## Resources

- Original CodexBar: https://github.com/steipete/CodexBar
- Tauri Documentation: https://tauri.app/
- React Documentation: https://react.dev/
- Bun Documentation: https://bun.sh/docs

## Additional Context

### Project Files
- `AGENTS.md` - Development notes and server status
- `FEATURE_PARITY.md` - Complete feature parity tracking with CodexBar
- `README.md` - User-facing documentation
- `package.json` - Build scripts and dependencies
- `tauri.conf.json` - Tauri configuration
- `vite.config.ts` - Vite build configuration

### Key Directories
- `.github/workflows/` - CI/CD workflows
- `src/` - Frontend TypeScript/React code
- `src-tauri/` - Backend Rust code
- `tests/` - Test suite
- `public/` - Static assets
- `scripts/` - Build and release scripts
