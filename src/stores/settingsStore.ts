import { create } from 'zustand';
import { persist, createJSONStorage } from 'zustand/middleware';
import { invoke } from '@tauri-apps/api/core';
import type { ProviderId, AppSettings, CookieSource, MenuBarDisplayMode } from '../lib/types';
import { DEFAULT_COOKIE_SOURCE } from '../lib/cookieSources';
import { DEFAULT_SETTINGS } from '../lib/providers';

interface SettingsStore extends AppSettings {
  // Actions
  setRefreshInterval: (seconds: number) => void;
  toggleProvider: (id: ProviderId) => void;
  enableProvider: (id: ProviderId) => void;
  disableProvider: (id: ProviderId) => void;
  setProviderOrder: (order: ProviderId[]) => void;
  setDisplayMode: (mode: 'merged' | 'separate') => void;
  setMenuBarDisplayMode: (mode: MenuBarDisplayMode) => void;
  setShowNotifications: (show: boolean) => void;
  setLaunchAtLogin: (launch: boolean) => void;
  setShowCredits: (show: boolean) => void;
  setShowCost: (show: boolean) => void;
  setCookieSource: (providerId: ProviderId, source: CookieSource) => void;
  getCookieSource: (providerId: ProviderId) => CookieSource;
  resetToDefaults: () => void;
  // Initialization
  initAutostart: () => Promise<void>;
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

      setShowNotifications: (show) => set({ showNotifications: show }),

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
      name: 'incubar-settings',
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
