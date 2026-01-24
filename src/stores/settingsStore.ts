import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import type {
  ProviderId,
  AppSettings,
  CookieSource,
  MenuBarDisplayMode,
  UsageBarDisplayMode,
} from '../lib/types';
import { DEFAULT_COOKIE_SOURCE } from '../lib/cookieSources';
import { DEFAULT_SETTINGS } from '../lib/providers';

const SETTINGS_STORAGE_KEY = 'incubar-settings';
const LEGACY_SETTINGS_STORAGE_KEYS = ['codexbar-settings'];

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
