// Provider identifiers
export type ProviderId = 
  | 'claude'
  | 'codex'
  | 'cursor'
  | 'copilot'
  | 'gemini'
  | 'antigravity'
  | 'factory'
  | 'zai'
  | 'minimax'
  | 'kimi'
  | 'kimi_k2'
  | 'kiro'
  | 'vertexai'
  | 'augment'
  | 'amp'
  | 'jetbrains'
  | 'opencode'
  | 'synthetic';

// Rate window represents a usage period (session, weekly, etc.)
export interface RateWindow {
  usedPercent: number;        // 0-100
  windowMinutes?: number;     // Duration of the window
  resetsAt?: string;          // ISO date string
  resetDescription?: string;  // "Resets in 4h"
  label?: string;             // "Session", "Weekly", etc.
}

// Cost information
export interface CostSnapshot {
  todayAmount: number;
  todayTokens: number;
  monthAmount: number;
  monthTokens: number;
  currency: string;
}

// Provider identity info
export interface ProviderIdentity {
  email?: string;
  name?: string;
  plan?: string;
  organization?: string;
}

// Full usage snapshot for a provider
export interface UsageSnapshot {
  primary?: RateWindow;       // Session/5h window
  secondary?: RateWindow;     // Weekly window
  tertiary?: RateWindow;      // Extra (e.g., Opus for Claude)
  credits?: {
    remaining: number;
    total?: number;
    unit: string;             // "tokens", "credits", etc.
  };
  cost?: CostSnapshot;
  identity?: ProviderIdentity;
  updatedAt: string;          // ISO date string
  error?: string;             // Error message if fetch failed
}

// Provider state in the store
export interface ProviderState {
  id: ProviderId;
  name: string;
  enabled: boolean;
  usage?: UsageSnapshot;
  isLoading: boolean;
  lastError?: string;
}

// Provider metadata (static info)
export interface ProviderMetadata {
  id: ProviderId;
  name: string;
  icon: string;               // Lucide icon name
  accentColor: string;        // Tailwind color class
  authMethod: 'oauth' | 'cookies' | 'api_key' | 'cli' | 'local_config';
  sessionLabel: string;
  weeklyLabel: string;
  opusLabel?: string | null;
  supportsOpus: boolean;
  supportsCredits: boolean;
  implemented: boolean;
  statusPageUrl?: string;
}

export type CookieSource = 'chrome' | 'safari' | 'firefox' | 'arc' | 'edge' | 'brave' | 'opera';

export type MenuBarDisplayMode = 'session' | 'weekly' | 'pace';

// Settings
export interface AppSettings {
  refreshIntervalSeconds: number;
  enabledProviders: ProviderId[];
  providerOrder: ProviderId[];
  displayMode: 'merged' | 'separate';
  menuBarDisplayMode: MenuBarDisplayMode;
  showNotifications: boolean;
  launchAtLogin: boolean;
  showCredits: boolean;
  showCost: boolean;
  cookieSources: Partial<Record<ProviderId, CookieSource>>;
}

// Event payloads from Rust
export interface UsageUpdateEvent {
  providerId: ProviderId;
  usage: UsageSnapshot;
}

export interface RefreshingEvent {
  providerId: ProviderId;
  isRefreshing: boolean;
}
