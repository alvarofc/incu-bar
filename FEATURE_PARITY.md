# IncuBar Feature Parity Checklist (CodexBar)

This document tracks feature parity between IncuBar (Tauri port) and the original CodexBar macOS Swift app.

**Legend:** Done | Partial | Not Started | N/A

---

## Current Implementation Status

### Providers Implemented

| Provider | Auth | Usage Fetch | Status |
|----------|------|-------------|--------|
| Cursor | Browser cookies | Done | Done |
| Copilot | GitHub Device Flow | Done | Done |
| Claude | CLI OAuth (~/.claude/) | Done | Done |
| Codex | CLI OAuth (~/.codex/) | Done | Done |
| Gemini | CLI OAuth (~/.gemini/) | Done | Done |
| z.ai | API Token (env var) | Done | Done |
| Kimi K2 | API Key (env var) | Done | Done |
| Synthetic | API Key (env var) | Done | Done |
| Factory | Browser cookies | Not Started | Not Started |
| Augment | Browser cookies | Not Started | Not Started |
| Kiro | Browser cookies | Not Started | Not Started |
| Kimi | Browser JWT | Not Started | Not Started |
| MiniMax | Browser cookies | Not Started | Not Started |
| Amp | Browser cookies | Not Started | Not Started |
| JetBrains | Local log parsing | Not Started | Not Started |
| OpenCode | Browser cookies | Not Started | Not Started |
| Vertex | Google OAuth | Not Started | Not Started |
| Antigravity | Status probe | Not Started | Not Started |

---

## Provider API Reference

### 1. Codex (OpenAI)

**Auth Methods:**
- OAuth: `~/.codex/auth.json` contains `{ "access_token": "...", "refresh_token": "..." }`
- CLI: `/status` command via PTY
- Web Dashboard: WKWebView scraping of ChatGPT

**API Endpoints:**
```
GET https://chatgpt.com/backend-api/wham/usage
Headers: Authorization: Bearer <token>
         Cookie: <session cookies>

Response:
{
  "plan_id": "chatgptplusplan",
  "rate_limits": [{
    "window_seconds": 10800,
    "requests_used": 45,
    "requests_limit": 80
  }]
}
```

**Cookie Domain:** `chatgpt.com`

---

### 2. Claude (Anthropic)

**Auth Methods:**
- OAuth: `~/.claude/.credentials.json` contains `{ "access_token": "...", "expires_at": "..." }`
- Web: Browser cookies from claude.ai
- CLI: `/status` and `/usage` commands via PTY

**API Endpoints:**

OAuth Usage:
```
GET https://api.anthropic.com/api/oauth/usage
Headers: Authorization: Bearer <access_token>
         anthropic-beta: prompt-caching-2024-07-31

Response:
{
  "usage": {
    "fast_rate_limit_used_percent": 45.5,
    "slow_rate_limit_used_percent": 12.0,
    "slow_rate_limit_resets_at": "2025-01-23T00:00:00Z"
  }
}
```

Web API:
```
GET https://claude.ai/api/organizations
GET https://claude.ai/api/organizations/<org_id>/usage
GET https://claude.ai/api/organizations/<org_id>/account
Headers: Cookie: <session cookies>
```

**Cookie Domain:** `claude.ai`

---

### 3. Cursor

**Auth Methods:**
- Browser cookies from cursor.com / cursor.sh

**API Endpoints:**
```
GET https://cursor.com/api/usage-summary
Headers: Cookie: <session cookies>

Response:
{
  "planType": "pro",
  "usageLimit": 500,
  "usageUsed": 245,
  "resetDate": "2025-02-01T00:00:00Z"
}

GET https://cursor.com/api/auth/me
Response:
{
  "email": "user@example.com",
  "name": "User Name"
}
```

**Cookie Domain:** `cursor.com`, `cursor.sh`

---

### 4. GitHub Copilot

**Auth Methods:**
- GitHub Device Flow OAuth

**Device Flow:**
```
POST https://github.com/login/device/code
Body: client_id=<client_id>&scope=copilot

Response:
{
  "device_code": "...",
  "user_code": "XXXX-XXXX",
  "verification_uri": "https://github.com/login/device",
  "expires_in": 899,
  "interval": 5
}

POST https://github.com/login/oauth/access_token
Body: client_id=<client_id>&device_code=<device_code>&grant_type=urn:ietf:params:oauth:grant-type:device_code

Response:
{
  "access_token": "gho_...",
  "token_type": "bearer",
  "scope": "copilot"
}
```

**Usage API:**
```
GET https://api.github.com/copilot_internal/user
Headers: Authorization: Bearer <access_token>

Response:
{
  "quota": {
    "monthly_limit": 2000,
    "monthly_used": 450
  }
}
```

---

### 5. Gemini

**Auth Methods:**
- CLI OAuth: `~/.gemini/oauth_creds.json` contains `{ "access_token": "...", "refresh_token": "...", "expiry_date": ... }`
- Settings: `~/.gemini/settings.json` contains auth type (oauth-personal, api-key, vertex-ai)

**API Endpoints:**

Load Code Assist (get project ID and tier):
```
POST https://cloudcode-pa.googleapis.com/v1internal:loadCodeAssist
Headers: Authorization: Bearer <access_token>
Body: {"metadata":{"ideType":"GEMINI_CLI","pluginType":"GEMINI"}}

Response:
{
  "cloudaicompanionProject": "project-id-123",
  "currentTier": { "id": "standard-tier" }
}
```

Retrieve Quota:
```
POST https://cloudcode-pa.googleapis.com/v1internal:retrieveUserQuota
Headers: Authorization: Bearer <access_token>
Body: {"project": "project-id-123"}

Response:
{
  "buckets": [{
    "modelId": "gemini-2.5-pro",
    "remainingFraction": 0.85,
    "resetTime": "2025-01-23T00:00:00Z"
  }, {
    "modelId": "gemini-2.5-flash",
    "remainingFraction": 0.92,
    "resetTime": "2025-01-23T00:00:00Z"
  }]
}
```

Token Refresh:
```
POST https://oauth2.googleapis.com/token
Body: client_id=<gemini_cli_client_id>&client_secret=<secret>&refresh_token=<token>&grant_type=refresh_token
```

**OAuth Client ID:** `REDACTED_GEMINI_OAUTH_CLIENT_ID`

---

### 6. Augment

**Auth Methods:**
- Browser cookies from augmentcode.com
- CLI OAuth

**API Endpoints:**
```
GET https://app.augmentcode.com/api/credits
Headers: Cookie: <session cookies>

Response:
{
  "credits_remaining": 1500,
  "credits_total": 2000,
  "resets_at": "2025-02-01T00:00:00Z"
}
```

**Cookie Domain:** `augmentcode.com`, `augment.co`

---

### 7. Kimi (Moonshot AI)

**Auth Methods:**
- JWT cookie from kimi.moonshot.cn

**API Endpoints:**
```
POST https://kimi.com/apiv2/grpc/kimi_api.BillingService/GetUsages
Headers: Cookie: <session cookies>
         Content-Type: application/proto

Response (decoded):
{
  "usages": [{
    "type": "tokens",
    "used": 50000,
    "limit": 100000
  }]
}
```

**Cookie Domain:** `kimi.moonshot.cn`, `kimi.com`

---

### 8. Kimi K2

**Auth Methods:**
- API key via `KIMI_K2_API_KEY`, `KIMI_API_KEY`, or `KIMI_KEY` environment variable

**API Endpoints:**
```
GET https://kimi-k2.ai/api/user/credits
Headers: Authorization: Bearer <api_key>

Response (flexible parsing supports multiple formats):
{
  "credits_remaining": 1500,
  "credits_total": 2000
}
// or
{
  "data": {
    "remaining": 1500,
    "total": 2000
  }
}
```

---

### 8b. Synthetic

**Auth Methods:**
- API key via `SYNTHETIC_API_KEY` environment variable

**API Endpoints:**
```
GET https://api.synthetic.new/v2/quotas
Headers: Authorization: Bearer <api_key>

Response:
{
  "quotas": [{
    "used": 500,
    "limit": 2000,
    "resets_at": "2025-02-01T00:00:00Z"
  }]
}
// or flat format:
{
  "used": 500,
  "limit": 2000
}
```

---

### 9. MiniMax

**Auth Methods:**
- Browser cookies from platform.minimax.io
- API token (alternative)

**API Endpoints:**
```
GET https://platform.minimax.io/platform/api/subscription/coding_plan/remains
Headers: Cookie: <session cookies>

Response:
{
  "data": {
    "remaining_credits": 500,
    "total_credits": 1000
  }
}
```

**Cookie Domain:** `minimax.chat`, `platform.minimax.io`

---

### 10. z.ai

**Auth Methods:**
- API token via `Z_AI_API_KEY` environment variable
- Region selection: Global (default) or BigModel CN

**API Endpoints:**
```
GET https://api.z.ai/api/monitor/usage/quota/limit
Headers: Authorization: Bearer <api_token>

Response:
{
  "code": 0,
  "msg": "ok",
  "data": {
    "token": {
      "used": 50000,
      "limit": 200000
    },
    "time": {
      "used": 120,
      "limit": 600
    }
  }
}
```

**CN Region Endpoint:** `https://bigmodel.cn/api/monitor/usage/quota/limit`

---

### 11. Factory (Droid)

**Auth Methods:**
- Browser cookies from factory.ai

**API Endpoints:**
```
GET https://app.factory.ai/api/usage
Headers: Cookie: <session cookies>
```

**Cookie Domain:** `factory.ai`, `app.factory.ai`

---

### 12. Amp (Sourcegraph)

**Auth Methods:**
- Browser cookies from ampcode.com

**API Endpoints:**
```
GET https://ampcode.com/api/usage
Headers: Cookie: <session cookies>
```

**Cookie Domain:** `ampcode.com`

---

### 13. JetBrains AI

**Auth Methods:**
- Local IDE log parsing (no API)

**Credential Locations:**
- macOS: `~/Library/Application Support/JetBrains/<IDE>/`
- Windows: `%APPDATA%\JetBrains\<IDE>\`
- Linux: `~/.config/JetBrains/<IDE>/`

Reads usage from IDE internal logs.

---

### 14. OpenCode

**Auth Methods:**
- Browser cookies from opencode.ai

**API Endpoints:**
```
GET https://opencode.ai/_server (server functions)
Headers: Cookie: <session cookies>
```

**Cookie Domain:** `opencode.ai`

---

### 15. Vertex AI

**Auth Methods:**
- Google OAuth with Cloud Monitoring scope

**API Endpoints:**
```
GET https://monitoring.googleapis.com/v3/projects/<project>/timeSeries
Headers: Authorization: Bearer <google_oauth_token>
```

---

### 16. Antigravity

**Auth Methods:**
- Status probe only (no auth required)

Monitors Google Workspace status page.

---

### 17. Kiro (AWS)

**Auth Methods:**
- Status probe only (no auth required)

Status page monitoring.

---

## Settings Reference

### CodexBar Settings (from SettingsStoreState.swift)

```typescript
interface Settings {
  // Refresh
  refreshFrequency: 'manual' | 60 | 120 | 300 | 900 | 1800; // seconds
  
  // General
  launchAtLogin: boolean;
  statusChecksEnabled: boolean;
  sessionQuotaNotificationsEnabled: boolean;
  
  // Display
  menuBarDisplayMode: 'session' | 'weekly' | 'pace';
  menuBarShowsBrandIconWithPercent: boolean;
  menuBarShowsHighestUsage: boolean;
  mergeIcons: boolean;
  switcherShowsIcons: boolean;
  usageBarsShowUsed: boolean; // vs remaining
  resetTimesShowAbsolute: boolean; // vs relative
  
  // Advanced
  costUsageEnabled: boolean;
  hidePersonalInfo: boolean;
  showOptionalCreditsAndExtraUsage: boolean;
  showAllTokenAccountsInMenu: boolean;
  
  // Debug
  debugMenuEnabled: boolean;
  debugFileLoggingEnabled: boolean;
  debugKeepCLISessionsAlive: boolean;
  randomBlinkEnabled: boolean;
  
  // Provider-specific
  claudeWebExtrasEnabled: boolean;
  openAIWebAccessEnabled: boolean;
  jetbrainsIDEBasePath: string;
  
  // Per-provider settings
  providers: {
    [providerId: string]: {
      enabled: boolean;
      cookieSource: 'chrome' | 'safari' | 'firefox' | 'arc' | 'edge' | 'brave' | 'opera';
      manualCookieHeader?: string;
    };
  };
}
```

---

## Icon Rendering Reference

### Tray Icon Specs
- Base size: 18x18pt at 2x scale (36x36px actual)
- Dynamic rendering based on usage percentage
- Provider-specific styles:
  - **Codex**: Face with eyes, optional hat
  - **Claude**: Crab with arms/legs
  - **Gemini**: Sparkle stars
  - **Factory**: Gear teeth
  - **Generic**: Progress ring

### States
- Normal (usage ring shows percent)
- Loading (spinning animation)
- Error (red indicator)
- Disabled (dimmed)
- Stale data (warning indicator)

### Animations
- Blink: Eyes open/close cycle
- Wiggle: Horizontal oscillation
- Tilt: Rotation oscillation

---

## Implementation Priority

### Phase 1: Core Providers - COMPLETE

1. **Cursor** - Done (browser cookies)
2. **Copilot** - Done (GitHub Device Flow)
3. **Claude OAuth** - Done (CLI OAuth from ~/.claude/)
4. **Codex OAuth** - Done (CLI OAuth from ~/.codex/)

### Phase 2: API Token Providers - COMPLETE

5. **z.ai** - Done (Z_AI_API_KEY env var)
6. **Kimi K2** - Done (KIMI_K2_API_KEY env var)
7. **Gemini** - Done (CLI OAuth from ~/.gemini/)
8. **Synthetic** - Done (SYNTHETIC_API_KEY env var)

### Phase 3: Cookie-Based Providers - IN PROGRESS

9. **Factory** - Not Started (browser cookies)
10. **Augment** - Not Started (browser cookies + keepalive)
11. **Kimi** - Not Started (JWT browser cookie)
12. **MiniMax** - Not Started (browser cookies or API)
13. **Amp** - Not Started (browser cookies)
14. **OpenCode** - Not Started (browser cookies)
15. **Kiro** - Not Started (browser cookies)

### Phase 4: Advanced Providers

16. **Claude CLI** - PTY session (optional enhancement)
17. **Codex CLI** - PTY session + dashboard scraping (optional)
18. **JetBrains** - Local log parsing
19. **Vertex AI** - Google OAuth

### Phase 5: Status-Only Providers

20. **Antigravity** - Status page monitoring

---

## Feature Checklist by Area

### 1. Core App Shell - Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Menu bar only (no dock) | Done | Tauri config |
| Single instance | Done | Tauri default |
| Launch at login | Done | tauri-plugin-autostart |
| Background refresh | Done | Rust async loop |
| Sleep/wake handling | Not Started | Need system events |
| Crash recovery | Not Started | |
| Debug logging | Done | tracing crate |

### 2. Tray Behavior - Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Tray icon present | Done | |
| Left click popup | Done | |
| Click outside dismiss | Done | |
| Escape closes | Done | |
| Dynamic icon | Not Started | Need canvas rendering |
| Dark/light adaptive | Not Started | |
| Tooltip | Not Started | |

### 3. Popup UI - Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Provider tabs | Done | |
| Single provider card | Done | |
| Empty onboarding | Done | |
| Loading states | Done | |
| Error states | Done | |
| Last updated | Done | |
| Manual refresh | Done | |
| Settings button | Done | |

### 4. Settings - Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Provider toggles | Done | |
| Provider order | Not Started | |
| Refresh interval | Done | |
| Show credits | Done | |
| Show cost | Done | |
| Notifications | Partial | Toggle exists, not wired |
| Launch at login | Partial | Toggle exists, not wired |
| Reset defaults | Done | |
| Version display | Done | |

### 5. Notifications - Not Started

| Feature | Status | Notes |
|---------|--------|-------|
| Usage threshold | Not Started | |
| Low credits | Not Started | |
| Refresh failure | Not Started | |

### 6. Storage - Partial

| Feature | Status | Notes |
|---------|--------|-------|
| Keychain tokens | Done | keyring crate |
| Cookie storage | Done | |
| Settings persist | Done | tauri-plugin-store |
| Secure delete | Not Started | |

---

## Rust Crates Needed

```toml
# Already using
reqwest = "0.12"           # HTTP client
serde = "1.0"              # Serialization
serde_json = "1.0"         # JSON
tokio = "1.0"              # Async runtime
anyhow = "1.0"             # Error handling
chrono = "0.4"             # Date/time
tracing = "0.1"            # Logging
keyring = "3.0"            # Keychain access
decrypt-cookies = "0.10"   # Browser cookie extraction
base64 = "0.22"            # JWT token decoding (Gemini)
dirs = "6"                 # Home directory
async-trait = "0.1"        # Async trait support
tauri-plugin-autostart = "2" # Launch at login

# Need to add (for remaining providers)
portable-pty = "0.8"       # PTY for CLI sessions (optional)
```

---

## Next Steps

1. ~~Complete Claude OAuth usage parsing~~ - Done
2. ~~Complete Codex OAuth usage parsing~~ - Done
3. ~~Add API token providers (z.ai, Kimi K2)~~ - Done
4. ~~Add Gemini provider~~ - Done
5. ~~Add Synthetic provider~~ - Done
6. ~~Add launch at login~~ - Done
7. Implement cookie-based providers (Factory, Augment, Kimi, MiniMax, Amp)
8. Implement dynamic tray icon rendering
9. Add notification support
