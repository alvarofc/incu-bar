import { create } from 'zustand';
import { invoke } from '@tauri-apps/api/core';
import { useShallow } from 'zustand/shallow';
import type {
  ProviderId,
  ProviderState,
  UsageSnapshot,
  ProviderIncident,
} from '../lib/types';
import { PROVIDERS, DEFAULT_ENABLED_PROVIDERS } from '../lib/providers';
import { useSettingsStore } from './settingsStore';

const MAX_HISTORY_POINTS = 30;
const USAGE_HISTORY_STORAGE_KEY = 'incubar-usage-history';

type UsageHistoryStorage = Partial<Record<ProviderId, ProviderState['usageHistory']>>;

const loadUsageHistoryStorage = (): UsageHistoryStorage => {
  if (typeof localStorage === 'undefined') {
    return {};
  }
  const stored = localStorage.getItem(USAGE_HISTORY_STORAGE_KEY);
  if (!stored) {
    return {};
  }
  try {
    const parsed = JSON.parse(stored) as UsageHistoryStorage;
    return parsed ?? {};
  } catch (error) {
    console.warn('Failed to parse usage history storage', error);
    return {};
  }
};

const saveUsageHistoryStorage = (next: UsageHistoryStorage) => {
  if (typeof localStorage === 'undefined') {
    return;
  }
  localStorage.setItem(USAGE_HISTORY_STORAGE_KEY, JSON.stringify(next));
};

const clearUsageHistoryStorage = () => {
  if (typeof localStorage === 'undefined') {
    return;
  }
  localStorage.removeItem(USAGE_HISTORY_STORAGE_KEY);
};

const shouldStoreUsageHistory = () => useSettingsStore.getState().storeUsageHistory;

const persistUsageHistory = (providers: Record<ProviderId, ProviderState>) => {
  if (!shouldStoreUsageHistory()) {
    clearUsageHistoryStorage();
    return;
  }
  const nextStorage = Object.fromEntries(
    (Object.keys(providers) as ProviderId[])
      .map((providerId) => [
        providerId,
        (providers[providerId].usageHistory ?? []).slice(-MAX_HISTORY_POINTS),
      ])
      .filter(([, points]) => points.length > 0)
  ) as UsageHistoryStorage;
  saveUsageHistoryStorage(nextStorage);
};

let storedUsageHistory = loadUsageHistoryStorage();
if (!shouldStoreUsageHistory()) {
  clearUsageHistoryStorage();
  storedUsageHistory = {};
}

interface UsageStore {
  // State
  providers: Record<ProviderId, ProviderState>;
  activeProvider: ProviderId;
  isRefreshing: boolean;
  lastGlobalRefresh: Date | null;

  // Actions
  setActiveProvider: (id: ProviderId) => void;
  setProviderUsage: (id: ProviderId, usage: UsageSnapshot) => void;
  setProviderStatus: (id: ProviderId, status: ProviderIncident | null) => void;
  setProviderLoading: (id: ProviderId, isLoading: boolean) => void;
  setProviderError: (id: ProviderId, error: string | undefined) => void;
  setProviderEnabled: (id: ProviderId, enabled: boolean) => void;
  refreshProvider: (id: ProviderId) => Promise<void>;
  refreshAllProviders: () => Promise<void>;
  initializeProviders: (enabledIds: ProviderId[]) => void;
  clearUsageHistory: () => void;
  syncUsageHistoryStorage: () => void;
  resetState: () => void;
}

// Initialize provider states
const createInitialProviderState = (id: ProviderId, enabled: boolean): ProviderState => ({
  id,
  name: PROVIDERS[id].name,
  enabled,
  isLoading: false,
  usageHistory: shouldStoreUsageHistory()
    ? (storedUsageHistory[id] ?? []).slice(-MAX_HISTORY_POINTS)
    : [],
});

const initialProviders: Record<ProviderId, ProviderState> = Object.fromEntries(
  (Object.keys(PROVIDERS) as ProviderId[]).map((id) => [
    id,
    createInitialProviderState(id, DEFAULT_ENABLED_PROVIDERS.includes(id)),
  ])
) as Record<ProviderId, ProviderState>;

const orderProviderStates = (
  providers: Record<ProviderId, ProviderState>,
  order: ProviderId[]
) => {
  const providerIds = Object.keys(providers) as ProviderId[];
  const normalizedOrder = order.filter((id) => providerIds.includes(id));
  const missingProviders = providerIds.filter((id) => !normalizedOrder.includes(id));
  return [...normalizedOrder, ...missingProviders].map((id) => providers[id]);
};

export const useUsageStore = create<UsageStore>((set, get) => ({
  providers: initialProviders,
  activeProvider: DEFAULT_ENABLED_PROVIDERS[0] || 'claude',
  isRefreshing: false,
  lastGlobalRefresh: null,

  setActiveProvider: (id) => set({ activeProvider: id }),

  setProviderUsage: (id, usage) =>
    set((state) => {
      const previous = state.providers[id];
      const history = previous.usageHistory ?? [];
      const storeUsageHistory = shouldStoreUsageHistory();
      if (!storeUsageHistory) {
        return {
          providers: {
            ...state.providers,
            [id]: {
              ...previous,
              usage,
              usageHistory: [] as ProviderState['usageHistory'],
              isLoading: false,
              lastError: usage.error,
            },
          },
        };
      }
      const nextPoint = {
        timestamp: usage.updatedAt,
        cost: usage.cost?.todayAmount ?? usage.cost?.monthAmount,
        credits: usage.credits?.remaining,
      };
      const lastPoint = history[history.length - 1];
      const shouldAppend = !lastPoint || lastPoint.timestamp !== nextPoint.timestamp;
      const nextHistory = shouldAppend
        ? [...history, nextPoint].slice(-MAX_HISTORY_POINTS)
        : history;

      const nextProviders = {
        providers: {
          ...state.providers,
          [id]: {
            ...previous,
            usage,
            usageHistory: nextHistory,
            isLoading: false,
            lastError: usage.error,
          },
        },
      };

      persistUsageHistory(nextProviders.providers);

      return nextProviders;
    }),

  setProviderStatus: (id, status) =>
    set((state) => ({
      providers: {
        ...state.providers,
        [id]: {
          ...state.providers[id],
          status: status ?? undefined,
        },
      },
    })),

  setProviderLoading: (id, isLoading) =>
    set((state) => ({
      providers: {
        ...state.providers,
        [id]: {
          ...state.providers[id],
          isLoading,
        },
      },
    })),

  setProviderError: (id, error) =>
    set((state) => ({
      providers: {
        ...state.providers,
        [id]: {
          ...state.providers[id],
          lastError: error,
          isLoading: false,
        },
      },
    })),

  setProviderEnabled: (id, enabled) =>
    set((state) => ({
      providers: {
        ...state.providers,
        [id]: {
          ...state.providers[id],
          enabled,
        },
      },
    })),

  refreshProvider: async (id) => {
    const { setProviderLoading, setProviderUsage, setProviderError } = get();
    console.log('[usageStore] refreshProvider - starting:', id);
    setProviderLoading(id, true);

    // Frontend timeout to ensure we don't hang indefinitely waiting for backend
    const REFRESH_TIMEOUT_MS = 30_000; // 30 seconds (backend has 10s status + 15s usage timeouts)
    
    const timeoutPromise = new Promise<never>((_, reject) => {
      setTimeout(() => reject(new Error('Refresh timed out')), REFRESH_TIMEOUT_MS);
    });

    try {
      const usage = await Promise.race([
        invoke<UsageSnapshot>('refresh_provider', { providerId: id }),
        timeoutPromise,
      ]);
      console.log('[usageStore] refreshProvider - success:', id, usage);
      setProviderUsage(id, usage);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      console.log('[usageStore] refreshProvider - error:', id, message);
      setProviderError(id, message);
    }
  },

  refreshAllProviders: async () => {
    const { providers, refreshProvider } = get();
    set({ isRefreshing: true });

    const enabledProviders = Object.values(providers).filter((p) => p.enabled);
    console.log('[usageStore] refreshAllProviders - enabled providers:', enabledProviders.map(p => p.id));
    
    const results = await Promise.allSettled(
      enabledProviders.map((p) => refreshProvider(p.id))
    );
    console.log('[usageStore] refreshAllProviders - completed, results:', results);

    set({ isRefreshing: false, lastGlobalRefresh: new Date() });
  },

  initializeProviders: (enabledIds) =>
    set((state) => {
      // Set activeProvider to first enabled provider if current one is not in the list
      const newActiveProvider = enabledIds.includes(state.activeProvider)
        ? state.activeProvider
        : enabledIds[0] || state.activeProvider;
      
      return {
        activeProvider: newActiveProvider,
        providers: Object.fromEntries(
          (Object.keys(state.providers) as ProviderId[]).map((id) => [
            id,
            {
              ...state.providers[id],
              enabled: enabledIds.includes(id),
            },
          ])
        ) as Record<ProviderId, ProviderState>,
      };
    }),

  clearUsageHistory: () =>
    set((state) => {
      const nextProviders = Object.fromEntries(
        (Object.keys(state.providers) as ProviderId[]).map((id) => [
          id,
          {
            ...state.providers[id],
            usageHistory: [] as ProviderState['usageHistory'],
          },
        ])
      ) as Record<ProviderId, ProviderState>;

      clearUsageHistoryStorage();

      return { providers: nextProviders };
    }),

  syncUsageHistoryStorage: () =>
    set((state) => {
      if (!shouldStoreUsageHistory()) {
        clearUsageHistoryStorage();
        return state;
      }
      persistUsageHistory(state.providers);
      return state;
    }),

  resetState: () => set({
    providers: initialProviders,
    activeProvider: DEFAULT_ENABLED_PROVIDERS[0] || 'claude',
    isRefreshing: false,
    lastGlobalRefresh: null,
  }),
}));

// Selectors
export const useActiveProvider = () => {
  const providerOrder = useSettingsStore((state) => state.providerOrder);
  return useUsageStore((state) => {
    const active = state.providers[state.activeProvider];
    // Only return if authenticated (has usage and no error)
    if (active?.usage && !active.lastError) {
      return active;
    }
    // Otherwise find first authenticated provider by preferred order
    const authenticated = orderProviderStates(state.providers, providerOrder).find(
      (p) => p.enabled && p.usage && !p.lastError
    );
    return authenticated || null;
  });
};

export const useEnabledProviders = () => {
  const providerOrder = useSettingsStore((state) => state.providerOrder);
  return useUsageStore(
    useShallow((state) =>
      orderProviderStates(state.providers, providerOrder).filter(
        (p) => p.enabled
      )
    )
  );
};

export const useAuthenticatedProviders = () => {
  const providerOrder = useSettingsStore((state) => state.providerOrder);
  return useUsageStore(
    useShallow((state) =>
      orderProviderStates(state.providers, providerOrder).filter(
        (p) => p.enabled && p.usage && !p.lastError
      )
    )
  );
};

export const useProviderById = (id: ProviderId) =>
  useUsageStore((state) => state.providers[id]);
