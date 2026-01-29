import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import type {
  ProviderId,
  AppSettings,
  CookieSource,
  MenuBarDisplayMode,
  MenuBarDisplayTextMode,
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
  menuBarShowsBrandIconWithPercent: 'menuBarShowsBrandIconWithPercent',
  menuBarDisplayMode: 'menuBarDisplayMode',
  mergeIcons: 'mergeIcons',
  switcherShowsIcons: 'switcherShowsIcons',
  showOptionalCreditsAndExtraUsage: 'showOptionalCreditsAndExtraUsage',
  tokenCostUsageEnabled: 'tokenCostUsageEnabled',
  providerOrder: 'providerOrder',
  providerToggles: 'providerToggles',
  menuBarShowsHighestUsage: 'menuBarShowsHighestUsage',
  debugMenuEnabled: 'debugMenuEnabled',
  showAllTokenAccountsInMenu: 'showAllTokenAccountsInMenu',
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

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value);

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
  const menuBarShowsBrandIconWithPercent =
    stored[LEGACY_SETTINGS_KEYS.menuBarShowsBrandIconWithPercent];
  const menuBarDisplayModeRaw = stored[LEGACY_SETTINGS_KEYS.menuBarDisplayMode];
  const mergeIcons = stored[LEGACY_SETTINGS_KEYS.mergeIcons];
  const showOptionalCreditsAndExtraUsage = stored[LEGACY_SETTINGS_KEYS.showOptionalCreditsAndExtraUsage];
  const tokenCostUsageEnabled = stored[LEGACY_SETTINGS_KEYS.tokenCostUsageEnabled];
  const switcherShowsIcons = stored[LEGACY_SETTINGS_KEYS.switcherShowsIcons];
  const providerOrder = stored[LEGACY_SETTINGS_KEYS.providerOrder];
  const providerToggles = stored[LEGACY_SETTINGS_KEYS.providerToggles];
  const menuBarShowsHighestUsage = stored[LEGACY_SETTINGS_KEYS.menuBarShowsHighestUsage];
  const debugMenuEnabled = stored[LEGACY_SETTINGS_KEYS.debugMenuEnabled];
  const showAllTokenAccountsInMenu = stored[LEGACY_SETTINGS_KEYS.showAllTokenAccountsInMenu];

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

  if (typeof menuBarShowsBrandIconWithPercent === 'boolean') {
    updates.menuBarDisplayTextEnabled = menuBarShowsBrandIconWithPercent;
  }

  if (typeof menuBarDisplayModeRaw === 'string') {
    if (menuBarDisplayModeRaw === 'percent'
      || menuBarDisplayModeRaw === 'pace'
      || menuBarDisplayModeRaw === 'both') {
      updates.menuBarDisplayTextMode = menuBarDisplayModeRaw;
    }
  }

  if (typeof mergeIcons === 'boolean') {
    updates.displayMode = mergeIcons ? 'merged' : 'separate';
  }

  if (typeof switcherShowsIcons === 'boolean') {
    updates.switcherShowsIcons = switcherShowsIcons;
  }

  if (typeof showOptionalCreditsAndExtraUsage === 'boolean') {
    updates.showExtraUsage = showOptionalCreditsAndExtraUsage;
  }

  if (typeof debugMenuEnabled === 'boolean') {
    updates.debugMenuEnabled = debugMenuEnabled;
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

  if (typeof showAllTokenAccountsInMenu === 'boolean') {
    updates.showAllTokenAccountsInMenu = showAllTokenAccountsInMenu;
  }

  return {
    ...settings,
    ...updates,
  };
};

migrateLegacySettingsStorage();

interface SettingsStore extends AppSettings {
  hasHydrated: boolean;
  // Actions
  setRefreshInterval: (seconds: number) => void;
  toggleProvider: (id: ProviderId) => void;
  enableProvider: (id: ProviderId) => void;
  disableProvider: (id: ProviderId) => void;
  setProviderOrder: (order: ProviderId[]) => void;
  setDisplayMode: (mode: 'merged' | 'separate') => void;
  setMenuBarDisplayMode: (mode: MenuBarDisplayMode) => void;
  setMenuBarDisplayTextEnabled: (enabled: boolean) => void;
  setMenuBarDisplayTextMode: (mode: MenuBarDisplayTextMode) => void;
  setUsageBarDisplayMode: (mode: UsageBarDisplayMode) => void;
  setResetTimeDisplayMode: (mode: 'relative' | 'absolute') => void;
  setSwitcherShowsIcons: (enabled: boolean) => void;
  setShowAllTokenAccountsInMenu: (enabled: boolean) => void;
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
  setRedactPersonalInfo: (enabled: boolean) => void;
  setCookieSource: (providerId: ProviderId, source: CookieSource) => void;
  getCookieSource: (providerId: ProviderId) => CookieSource;
  resetToDefaults: () => void;
  setDebugMenuEnabled: (enabled: boolean) => void;
  setDebugFileLogging: (enabled: boolean) => void;
  setDebugKeepCliSessionsAlive: (enabled: boolean) => void;
  setDebugRandomBlink: (enabled: boolean) => void;
  setHidePersonalInfo: (enabled: boolean) => void;
  setDebugDisableKeychainAccess: (enabled: boolean) => void;
  setInstallOrigin: (origin: string | null) => void;
  setHasHydrated: (hydrated: boolean) => void;
  // Initialization
  initAutostart: () => Promise<void>;
  setCrashRecoveryAt: (timestamp: string) => void;
  syncProviderEnabled: (id: ProviderId, enabled: boolean) => Promise<void>;
}

export const useSettingsStore = create<SettingsStore>()(
  persist(
    (set, get) => ({
      ...DEFAULT_SETTINGS,
      hasHydrated: false,

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

      setMenuBarDisplayTextEnabled: (enabled) => set({ menuBarDisplayTextEnabled: enabled }),

      setMenuBarDisplayTextMode: (mode) => set({ menuBarDisplayTextMode: mode }),

      setUsageBarDisplayMode: (mode) => set({ usageBarDisplayMode: mode }),

      setResetTimeDisplayMode: (mode) => set({ resetTimeDisplayMode: mode }),

      setSwitcherShowsIcons: (enabled) => set({ switcherShowsIcons: enabled }),

      setShowAllTokenAccountsInMenu: (enabled) =>
        set({ showAllTokenAccountsInMenu: enabled }),

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

      setRedactPersonalInfo: (enabled) => set({ redactPersonalInfo: enabled }),

      setDebugMenuEnabled: (enabled) => set({ debugMenuEnabled: enabled }),

      setDebugFileLogging: (enabled) => set({ debugFileLogging: enabled }),

      setDebugKeepCliSessionsAlive: (enabled) =>
        set({ debugKeepCliSessionsAlive: enabled }),

      setDebugRandomBlink: (enabled) => set({ debugRandomBlink: enabled }),

      setHidePersonalInfo: (enabled) => set({ hidePersonalInfo: enabled }),

      setDebugDisableKeychainAccess: (enabled) =>
        set({ debugDisableKeychainAccess: enabled }),

      setInstallOrigin: (origin) => set({ installOrigin: origin ?? undefined }),

      setHasHydrated: (hydrated) => {
        console.log('[settingsStore] setHasHydrated called with:', hydrated);
        console.log('[settingsStore] Current hasHydrated before set:', get().hasHydrated);
        set({ hasHydrated: hydrated });
        console.log('[settingsStore] hasHydrated after set:', get().hasHydrated);
      },

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
      partialize: (state) => {
        // Exclude hasHydrated from persistence - it must start as false each session
        const { hasHydrated: _, ...rest } = state;
        return rest;
      },
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
      onRehydrateStorage: () => {
        console.log('[settingsStore] onRehydrateStorage: outer function called');
        return (state, error) => {
          console.log('[settingsStore] onRehydrateStorage: inner callback fired, state:', state?.enabledProviders, 'error:', error);
          if (error) {
            console.error('[settingsStore] Hydration error:', error);
          }
          try {
            if (typeof localStorage === 'undefined') {
              return;
            }
            const legacySettingsRaw = localStorage.getItem('settings-store');
            if (!legacySettingsRaw) {
              return;
            }
            const parsed = JSON.parse(legacySettingsRaw);
            if (!isRecord(parsed)) {
              return;
            }
            const legacySettings = parsed;
            if (legacySettings?.legacyDefaults || legacySettings?.legacyConfig) {
              return;
            }
            const merged = {
              ...legacySettings,
              legacyDefaults: legacySettings,
            };
            localStorage.setItem('settings-store', JSON.stringify(merged));
          } catch (migrationError) {
            console.warn('Failed to migrate legacy settings defaults', migrationError);
          }
        };
      },
    }
  )
);

// Use Zustand persist's onFinishHydration API to reliably set hasHydrated
// Also check if hydration already happened (synchronous localStorage read)
useSettingsStore.persist.onFinishHydration(() => {
  console.log('[settingsStore] onFinishHydration callback - setting hasHydrated to true');
  useSettingsStore.setState({ hasHydrated: true });
  // Ensure backend registry matches current settings on startup
  const enabledProviders = useSettingsStore.getState().enabledProviders;
  import('@tauri-apps/api/core')
    .then(({ invoke }) => invoke('set_enabled_providers', { providerIds: enabledProviders }))
    .catch((error) => {
      console.warn('[settingsStore] Failed to sync enabled providers on hydration', error);
    });
});

// Check if hydration already completed synchronously before onFinishHydration was registered
if (useSettingsStore.persist.hasHydrated()) {
  console.log('[settingsStore] Already hydrated on module load - setting hasHydrated to true');
  useSettingsStore.setState({ hasHydrated: true });
  // Also sync providers since we missed the onFinishHydration callback
  const enabledProviders = useSettingsStore.getState().enabledProviders;
  import('@tauri-apps/api/core')
    .then(({ invoke }) => invoke('set_enabled_providers', { providerIds: enabledProviders }))
    .catch((error) => {
      console.warn('[settingsStore] Failed to sync enabled providers on module load hydration', error);
    });
}
// Note: We don't unsubscribe because we only need this to fire once on app start

// Cross-window sync: emit settings-updated event when enabledProviders or providerOrder changes
// This is done via subscription so it works regardless of which action modified the state
let lastEnabledProviders: string | null = null;
let lastProviderOrder: string | null = null;

const emitSettingsUpdated = async (enabledProviders: ProviderId[], providerOrder: ProviderId[]) => {
  const { invoke } = await import('@tauri-apps/api/core');
  try {
    await invoke('broadcast_settings_updated', { enabledProviders, providerOrder });
  } catch (error) {
    console.warn('[settingsStore] Failed to broadcast settings update', error);
  }
};

useSettingsStore.subscribe((state, prevState) => {
  // Only emit after hydration is complete to avoid sending default values
  if (!state.hasHydrated) {
    return;
  }
  
  const currentEnabled = state.enabledProviders.join('|');
  const currentOrder = state.providerOrder.join('|');
  const prevEnabled = prevState.enabledProviders.join('|');
  const prevOrder = prevState.providerOrder.join('|');
  
  // Skip if nothing changed (also handles initial subscription call)
  if (currentEnabled === prevEnabled && currentOrder === prevOrder) {
    return;
  }
  
  if (lastEnabledProviders === null) {
    lastEnabledProviders = prevEnabled;
    lastProviderOrder = prevOrder;
  }

  // Check if values actually changed from our tracked state
  if (currentEnabled === lastEnabledProviders && currentOrder === lastProviderOrder) {
    return;
  }

  lastEnabledProviders = currentEnabled;
  lastProviderOrder = currentOrder;

  void emitSettingsUpdated(state.enabledProviders, state.providerOrder);
});

// Selectors
export const useEnabledProviderIds = () =>
  useSettingsStore((state) => state.enabledProviders);

export const useRefreshInterval = () =>
  useSettingsStore((state) => state.refreshIntervalSeconds);

export const useCookieSource = (providerId: ProviderId) =>
  useSettingsStore(
    (state) => state.cookieSources[providerId] ?? DEFAULT_COOKIE_SOURCE
  );
