import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import type {
  ProviderId,
  AppSettings,
  CookieSource,
  MenuBarDisplayMode,
  UsageBarDisplayMode,
  UpdateChannel,
} from '../lib/types';
import { DEFAULT_COOKIE_SOURCE } from '../lib/cookieSources';
import { DEFAULT_SETTINGS } from '../lib/providers';
import { getDefaultUpdateChannelForVersion } from '../lib/updateChannel';

const SETTINGS_STORAGE_KEY = 'incubar-settings';
const LEGACY_SETTINGS_STORAGE_KEYS = ['codexbar-settings'];
const LEGACY_SETTINGS_KEYS = {
  refreshFrequency: 'refreshFrequency',
  launchAtLogin: 'launchAtLogin',
  statusChecksEnabled: 'statusChecksEnabled',
  sessionQuotaNotificationsEnabled: 'sessionQuotaNotificationsEnabled',
  usageBarsShowUsed: 'usageBarsShowUsed',
  resetTimesShowAbsolute: 'resetTimesShowAbsolute',
  mergeIcons: 'mergeIcons',
  switcherShowsIcons: 'switcherShowsIcons',
  showOptionalCreditsAndExtraUsage: 'showOptionalCreditsAndExtraUsage',
  tokenCostUsageEnabled: 'tokenCostUsageEnabled',
  providerOrder: 'providerOrder',
  providerToggles: 'providerToggles',
  menuBarShowsHighestUsage: 'menuBarShowsHighestUsage',
};

const LEGACY_REFRESH_FREQUENCY_TO_SECONDS: Record<string, number> = {
  manual: 0,
  oneMinute: 60,
  twoMinutes: 120,
  fiveMinutes: 300,
  fifteenMinutes: 900,
  thirtyMinutes: 1800,
};

const migrateLegacySettingsStorage = () => {
  if (typeof localStorage === 'undefined') {
    return;
  }

  const currentValue = localStorage.getItem(SETTINGS_STORAGE_KEY);

  LEGACY_SETTINGS_STORAGE_KEYS.forEach((legacyKey) => {
    const legacyValue = localStorage.getItem(legacyKey);
    if (legacyValue === null) {
      return;
    }

    if (currentValue === null) {
      localStorage.setItem(SETTINGS_STORAGE_KEY, legacyValue);
    }

    localStorage.removeItem(legacyKey);
  });
};

const parseLegacyRefreshInterval = (raw: unknown) => {
  if (typeof raw === 'number') {
    return raw;
  }
  if (typeof raw !== 'string') {
    return undefined;
  }
  if (raw === 'manual') {
    return 0;
  }
  if (Object.prototype.hasOwnProperty.call(LEGACY_REFRESH_FREQUENCY_TO_SECONDS, raw)) {
    return LEGACY_REFRESH_FREQUENCY_TO_SECONDS[raw];
  }
  const parsed = Number(raw);
  return Number.isFinite(parsed) ? parsed : undefined;
};

const normalizeLegacyProviderId = (raw: string): ProviderId =>
  raw === 'kimik2' ? 'kimi_k2' : (raw as ProviderId);

  const mergeLegacySettingsDefaults = (settings: AppSettings, stored?: Record<string, unknown>) => {
  if (!stored) {
    return settings;
  }

  const refreshRaw = stored[LEGACY_SETTINGS_KEYS.refreshFrequency];
  const refreshIntervalSeconds = parseLegacyRefreshInterval(refreshRaw);

  const launchAtLogin = stored[LEGACY_SETTINGS_KEYS.launchAtLogin];
  const statusChecksEnabled = stored[LEGACY_SETTINGS_KEYS.statusChecksEnabled];
  const sessionQuotaNotificationsEnabled = stored[LEGACY_SETTINGS_KEYS.sessionQuotaNotificationsEnabled];
  const usageBarsShowUsed = stored[LEGACY_SETTINGS_KEYS.usageBarsShowUsed];
  const resetTimesShowAbsolute = stored[LEGACY_SETTINGS_KEYS.resetTimesShowAbsolute];
  const mergeIcons = stored[LEGACY_SETTINGS_KEYS.mergeIcons];
  const showOptionalCreditsAndExtraUsage = stored[LEGACY_SETTINGS_KEYS.showOptionalCreditsAndExtraUsage];
  const tokenCostUsageEnabled = stored[LEGACY_SETTINGS_KEYS.tokenCostUsageEnabled];
  const providerOrder = stored[LEGACY_SETTINGS_KEYS.providerOrder];
  const providerToggles = stored[LEGACY_SETTINGS_KEYS.providerToggles];
  const menuBarShowsHighestUsage = stored[LEGACY_SETTINGS_KEYS.menuBarShowsHighestUsage];

  const updates: Partial<AppSettings> = {};

  if (typeof refreshIntervalSeconds === 'number') {
    updates.refreshIntervalSeconds = refreshIntervalSeconds;
  }

  if (typeof launchAtLogin === 'boolean') {
    updates.launchAtLogin = launchAtLogin;
  }

  if (typeof statusChecksEnabled === 'boolean') {
    updates.pollProviderStatus = statusChecksEnabled;
  }

  if (typeof sessionQuotaNotificationsEnabled === 'boolean') {
    updates.notifySessionUsage = sessionQuotaNotificationsEnabled;
  }

  if (typeof usageBarsShowUsed === 'boolean') {
    updates.usageBarDisplayMode = usageBarsShowUsed ? 'used' : 'remaining';
  }

  if (typeof resetTimesShowAbsolute === 'boolean') {
    updates.resetTimeDisplayMode = resetTimesShowAbsolute ? 'absolute' : 'relative';
  }

  if (typeof mergeIcons === 'boolean') {
    updates.displayMode = mergeIcons ? 'merged' : 'separate';
  }

  if (typeof showOptionalCreditsAndExtraUsage === 'boolean') {
    updates.showExtraUsage = showOptionalCreditsAndExtraUsage;
  }

  if (typeof tokenCostUsageEnabled === 'boolean') {
    updates.showCost = tokenCostUsageEnabled;
  }

  if (Array.isArray(providerOrder)) {
    const order = providerOrder
      .filter((id) => typeof id === 'string')
      .map((id) => normalizeLegacyProviderId(id as string));
    if (order.length) {
      updates.providerOrder = order;
    }
  }

  if (providerToggles && typeof providerToggles === 'object') {
    const toggles = providerToggles as Record<string, unknown>;
    const enabledProviders = Object.entries(toggles)
      .filter(([, enabled]) => enabled === true)
      .map(([id]) => normalizeLegacyProviderId(id));
    if (enabledProviders.length) {
      updates.enabledProviders = enabledProviders;
    }
  }

  if (menuBarShowsHighestUsage === true) {
    updates.menuBarDisplayMode = 'highest';
  }

  return {
    ...settings,
    ...updates,
  };
};

migrateLegacySettingsStorage();

interface SettingsStore extends AppSettings {
  // Actions
  setRefreshInterval: (seconds: number) => void;
  toggleProvider: (id: ProviderId) => void;
  enableProvider: (id: ProviderId) => void;
  disableProvider: (id: ProviderId) => void;
  setProviderOrder: (order: ProviderId[]) => void;
  setDisplayMode: (mode: 'merged' | 'separate') => void;
  setMenuBarDisplayMode: (mode: MenuBarDisplayMode) => void;
  setUsageBarDisplayMode: (mode: UsageBarDisplayMode) => void;
  setResetTimeDisplayMode: (mode: 'relative' | 'absolute') => void;
  setAutoUpdateEnabled: (enabled: boolean) => void;
  setUpdateChannel: (channel: UpdateChannel) => void;
  setShowNotifications: (show: boolean) => void;
  setNotifySessionUsage: (enabled: boolean) => void;
  setNotifyCreditsLow: (enabled: boolean) => void;
  setNotifyRefreshFailure: (enabled: boolean) => void;
  setNotifyStaleUsage: (enabled: boolean) => void;
  setLaunchAtLogin: (launch: boolean) => void;
  setShowCredits: (show: boolean) => void;
  setShowCost: (show: boolean) => void;
  setShowExtraUsage: (show: boolean) => void;
  setStoreUsageHistory: (enabled: boolean) => void;
  setPollProviderStatus: (enabled: boolean) => void;
  setCookieSource: (providerId: ProviderId, source: CookieSource) => void;
  getCookieSource: (providerId: ProviderId) => CookieSource;
  resetToDefaults: () => void;
  setDebugFileLogging: (enabled: boolean) => void;
  setDebugKeepCliSessionsAlive: (enabled: boolean) => void;
  setDebugRandomBlink: (enabled: boolean) => void;
  setInstallOrigin: (origin: string | null) => void;
  // Initialization
  initAutostart: () => Promise<void>;
  setCrashRecoveryAt: (timestamp: string) => void;
  syncProviderEnabled: (id: ProviderId, enabled: boolean) => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set, get) => ({
      ...DEFAULT_SETTINGS,

      setRefreshInterval: (seconds) => set({ refreshIntervalSeconds: seconds }),

      toggleProvider: (id) =>
        set((state) => ({
          enabledProviders: state.enabledProviders.includes(id)
            ? state.enabledProviders.filter((p) => p !== id)
            : [...state.enabledProviders, id],
        })),

      enableProvider: (id) =>
        set((state) => ({
          enabledProviders: state.enabledProviders.includes(id)
            ? state.enabledProviders
            : [...state.enabledProviders, id],
        })),

      disableProvider: (id) =>
        set((state) => ({
          enabledProviders: state.enabledProviders.filter((p) => p !== id),
        })),

      setProviderOrder: (order) => set({ providerOrder: order }),

      setDisplayMode: (mode) => set({ displayMode: mode }),

      setMenuBarDisplayMode: (mode) => set({ menuBarDisplayMode: mode }),

      setUsageBarDisplayMode: (mode) => set({ usageBarDisplayMode: mode }),

      setResetTimeDisplayMode: (mode) => set({ resetTimeDisplayMode: mode }),

      setAutoUpdateEnabled: (enabled) => set({ autoUpdateEnabled: enabled }),

      setUpdateChannel: (channel) => set({ updateChannel: channel }),

      setShowNotifications: (show) => set({ showNotifications: show }),

      setNotifySessionUsage: (enabled) => set({ notifySessionUsage: enabled }),

      setNotifyCreditsLow: (enabled) => set({ notifyCreditsLow: enabled }),

      setNotifyRefreshFailure: (enabled) => set({ notifyRefreshFailure: enabled }),

      setNotifyStaleUsage: (enabled) => set({ notifyStaleUsage: enabled }),

      setLaunchAtLogin: async (launch) => {
        try {
          await invoke('set_autostart_enabled', { enabled: launch });
          set({ launchAtLogin: launch });
        } catch (e) {
          console.error('Failed to set autostart:', e);
        }
      },

      setShowCredits: (show) => set({ showCredits: show }),

      setShowCost: (show) => set({ showCost: show }),

      setShowExtraUsage: (show) => set({ showExtraUsage: show }),

      setStoreUsageHistory: (enabled) => set({ storeUsageHistory: enabled }),

      setPollProviderStatus: (enabled) => set({ pollProviderStatus: enabled }),

      setDebugFileLogging: (enabled) => set({ debugFileLogging: enabled }),

      setDebugKeepCliSessionsAlive: (enabled) =>
        set({ debugKeepCliSessionsAlive: enabled }),

      setDebugRandomBlink: (enabled) => set({ debugRandomBlink: enabled }),

      setInstallOrigin: (origin) => set({ installOrigin: origin ?? undefined }),

      setCookieSource: (providerId, source) =>
        set((state) => ({
          cookieSources: {
            ...state.cookieSources,
            [providerId]: source,
          },
        })),

      getCookieSource: (providerId) =>
        get().cookieSources[providerId] ?? DEFAULT_COOKIE_SOURCE,

      resetToDefaults: () => set(DEFAULT_SETTINGS),

      setCrashRecoveryAt: (timestamp) => set({ crashRecoveryAt: timestamp }),

      syncProviderEnabled: async (id, enabled) => {
        try {
          await invoke('set_provider_enabled', { providerId: id, enabled });
        } catch (e) {
          console.error('Failed to sync provider enabled state:', e);
        }
      },

      initAutostart: async () => {
        try {
          const enabled = await invoke<boolean>('get_autostart_enabled');
          set({ launchAtLogin: enabled });
        } catch (e) {
          console.error('Failed to get autostart status:', e);
        }
      },
    }),
    {
      name: SETTINGS_STORAGE_KEY,
      storage: createJSONStorage(() => localStorage),
      merge: (persistedState, currentState) => {
        const stored = persistedState as Partial<AppSettings> & {
          legacyDefaults?: Record<string, unknown>;
        };
        const baseState = currentState as SettingsStore;
        const mergedSettings = stored?.legacyDefaults
          ? mergeLegacySettingsDefaults({ ...baseState, ...(stored ?? {}) }, stored.legacyDefaults)
          : { ...baseState, ...(stored ?? {}) };
        const currentVersion = import.meta.env.PACKAGE_VERSION ?? '';
        const defaultChannel = getDefaultUpdateChannelForVersion(currentVersion);
        const updateChannel = stored?.updateChannel ?? defaultChannel;
        return {
          ...baseState,
          ...mergedSettings,
          updateChannel,
        };
      },
      onRehydrateStorage: () => () => {
        if (typeof localStorage === 'undefined') {
          return;
        }
        const legacySettingsRaw = localStorage.getItem('settings-store');
        if (!legacySettingsRaw) {
          return;
        }
        try {
          const legacySettings = JSON.parse(legacySettingsRaw) as Record<string, unknown>;
          if (legacySettings?.legacyDefaults || legacySettings?.legacyConfig) {
            return;
          }
          const merged = {
            ...legacySettings,
            legacyDefaults: legacySettings,
          };
          localStorage.setItem('settings-store', JSON.stringify(merged));
        } catch (error) {
          console.warn('Failed to migrate legacy settings defaults', error);
        }
      },
    }
  )
);

// Selectors
export const useEnabledProviderIds = () =>
  useSettingsStore((state) => state.enabledProviders);

export const useRefreshInterval = () =>
  useSettingsStore((state) => state.refreshIntervalSeconds);

export const useCookieSource = (providerId: ProviderId) =>
  useSettingsStore(
    (state) => state.cookieSources[providerId] ?? DEFAULT_COOKIE_SOURCE
  );
