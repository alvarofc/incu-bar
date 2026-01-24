import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { sendNotification } from '@tauri-apps/plugin-notification';
import { PopupWindow } from './components/PopupWindow';
import { SettingsPanel } from './components/SettingsPanel';
import { useUsageStore } from './stores/usageStore';
import { useSettingsStore } from './stores/settingsStore';
import type { ProviderId, ProviderIncident, RefreshingEvent, UpdateChannel, UsageUpdateEvent } from './lib/types';
import type {
  CreditsNotificationState,
  RefreshFailureNotificationState,
  SessionNotificationState,
  StaleUsageNotificationState,
} from './lib/notifications';
import {
  evaluateCreditsNotifications,
  evaluateRefreshFailureNotifications,
  evaluateSessionNotifications,
  evaluateStaleUsageNotifications,
} from './lib/notifications';
import { PROVIDERS } from './lib/providers';
import { getStaleAfterMs, isTimestampStale } from './lib/staleness';
import { restoreSafeStateAfterCrash } from './lib/crashRecovery';
import './styles/globals.css';

type View = 'main' | 'settings';

interface AuthStatus {
  authenticated: boolean;
}

function App() {
  const [currentView, setCurrentView] = useState<View>('main');
  const setProviderUsage = useUsageStore((s) => s.setProviderUsage);
  const setProviderStatus = useUsageStore((s) => s.setProviderStatus);
  const initializeProviders = useUsageStore((s) => s.initializeProviders);
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const refreshIntervalSeconds = useSettingsStore((s) => s.refreshIntervalSeconds);
  const showNotifications = useSettingsStore((s) => s.showNotifications);
  const autoUpdateEnabled = useSettingsStore((s) => s.autoUpdateEnabled);
  const updateChannel = useSettingsStore((s) => s.updateChannel);
  const notifySessionUsage = useSettingsStore((s) => s.notifySessionUsage);
  const notifyCreditsLow = useSettingsStore((s) => s.notifyCreditsLow);
  const notifyRefreshFailure = useSettingsStore((s) => s.notifyRefreshFailure);
  const notifyStaleUsage = useSettingsStore((s) => s.notifyStaleUsage);
  const pollProviderStatus = useSettingsStore((s) => s.pollProviderStatus);
  const debugFileLogging = useSettingsStore((s) => s.debugFileLogging);
  const debugKeepCliSessionsAlive = useSettingsStore(
    (s) => s.debugKeepCliSessionsAlive
  );
  const debugRandomBlink = useSettingsStore((s) => s.debugRandomBlink);
  const redactPersonalInfo = useSettingsStore((s) => s.redactPersonalInfo);
  const initAutostart = useSettingsStore((s) => s.initAutostart);
  const setInstallOrigin = useSettingsStore((s) => s.setInstallOrigin);
  const initializedRef = useRef(false);
  const notificationStateRef = useRef(new Map<ProviderId, SessionNotificationState>());
  const creditsNotificationStateRef = useRef(new Map<ProviderId, CreditsNotificationState>());
  const refreshFailureNotificationRef = useRef(
    new Map<ProviderId, RefreshFailureNotificationState>()
  );
  const staleUsageNotificationRef = useRef(new Map<ProviderId, StaleUsageNotificationState>());
  const lastUpdateCheckChannelRef = useRef<UpdateChannel | null>(null);

  // Initialize enabled providers from settings (only once on mount)
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      restoreSafeStateAfterCrash();
      initializeProviders(enabledProviders);
      // Sync autostart status from system
      initAutostart();
      void invoke<string>('get_install_origin')
        .then((origin) => {
          setInstallOrigin(origin);
        })
        .catch((error) => {
          console.warn('Failed to load install origin', error);
          setInstallOrigin(null);
        });
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    let active = true;

    const autoEnableAuthenticatedProviders = async () => {
      try {
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        if (!active) return;
        const settingsStore = useSettingsStore.getState();
        const usageStore = useUsageStore.getState();

        Object.entries(status).forEach(([id, providerStatus]) => {
          if (providerStatus?.authenticated !== true) {
            return;
          }
          if (!(id in PROVIDERS)) {
            return;
          }
          const providerId = id as ProviderId;
          if (settingsStore.enabledProviders.includes(providerId)) {
            return;
          }
          settingsStore.enableProvider(providerId);
          void settingsStore.syncProviderEnabled(providerId, true);
          usageStore.setProviderEnabled(providerId, true);
        });
      } catch (error) {
        console.error('Failed to detect provider authentication:', error);
      }
    };

    autoEnableAuthenticatedProviders();

    return () => {
      active = false;
    };
  }, []);

  useEffect(() => {
    if (!autoUpdateEnabled) {
      lastUpdateCheckChannelRef.current = null;
      return;
    }

    if (lastUpdateCheckChannelRef.current === updateChannel) {
      return;
    }

    const checkForUpdates = async () => {
      lastUpdateCheckChannelRef.current = updateChannel;
      try {
        const update = await check({ headers: { 'X-Update-Channel': updateChannel } });
        if (!update) {
          return;
        }
        await update.downloadAndInstall();
        await relaunch();
      } catch (error) {
        console.warn('Auto-update check failed', error);
      }
    };

    void checkForUpdates();
  }, [autoUpdateEnabled, updateChannel]);

  useEffect(() => {
    void invoke('set_debug_file_logging', { enabled: debugFileLogging });
  }, [debugFileLogging]);

  useEffect(() => {
    void invoke('set_debug_keep_cli_sessions_alive', {
      enabled: debugKeepCliSessionsAlive,
    });
  }, [debugKeepCliSessionsAlive]);

  useEffect(() => {
    void invoke('set_debug_random_blink', { enabled: debugRandomBlink });
  }, [debugRandomBlink]);

  useEffect(() => {
    void invoke('set_redact_personal_info', { enabled: redactPersonalInfo });
  }, [redactPersonalInfo]);

  // Sync enabled providers when settings change
  useEffect(() => {
    if (initializedRef.current) {
      initializeProviders(enabledProviders);
    }
  }, [enabledProviders, initializeProviders]);

  // Listen for usage updates from Rust backend
  useEffect(() => {
    const unlisten = listen<UsageUpdateEvent>('usage-updated', (event) => {
      const { providerId, usage } = event.payload;
      setProviderUsage(providerId, usage);
      const metadata = PROVIDERS[providerId];
      evaluateSessionNotifications({
        providerId,
        providerName: metadata.name,
        sessionLabel: metadata.sessionLabel,
        usage,
        showNotifications: showNotifications && notifySessionUsage,
        stateMap: notificationStateRef.current,
        notify: (title, body) => void sendNotification({ title, body }),
      });
      evaluateCreditsNotifications({
        providerId,
        providerName: metadata.name,
        usage,
        showNotifications: showNotifications && notifyCreditsLow,
        stateMap: creditsNotificationStateRef.current,
        notify: (title, body) => void sendNotification({ title, body }),
      });
      evaluateRefreshFailureNotifications({
        providerId,
        providerName: metadata.name,
        error: usage.error,
        showNotifications: showNotifications && notifyRefreshFailure,
        stateMap: refreshFailureNotificationRef.current,
        notify: (title, body) => void sendNotification({ title, body }),
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setProviderUsage, showNotifications, notifySessionUsage, notifyCreditsLow, notifyRefreshFailure]);

  useEffect(() => {
    const unlistenRefresh = listen('refresh-requested', () => {
      useUsageStore.getState().refreshAllProviders();
    });

    return () => {
      unlistenRefresh.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlistenRefreshing = listen<RefreshingEvent>('refreshing-provider', (event) => {
      useUsageStore.getState().setProviderLoading(
        event.payload.providerId,
        event.payload.isRefreshing
      );
    });

    return () => {
      unlistenRefreshing.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlistenRefreshFailure = listen<UsageUpdateEvent>('refresh-failed', (event) => {
      const { providerId, usage } = event.payload;
      if (!usage?.error) return;
      const metadata = PROVIDERS[providerId];
      evaluateRefreshFailureNotifications({
        providerId,
        providerName: metadata.name,
        error: usage.error,
        showNotifications: showNotifications && notifyRefreshFailure,
        stateMap: refreshFailureNotificationRef.current,
        notify: (title, body) => void sendNotification({ title, body }),
      });
    });

    return () => {
      unlistenRefreshFailure.then((fn) => fn());
    };
  }, [showNotifications, notifyRefreshFailure]);

  useEffect(() => {
    if (refreshIntervalSeconds <= 0) return undefined;

    const intervalMs = refreshIntervalSeconds * 1000;
    const intervalId = window.setInterval(() => {
      const { providers } = useUsageStore.getState();
      Object.values(providers).forEach((provider) => {
        if (!provider.enabled || !provider.usage?.updatedAt) return;
        const metadata = PROVIDERS[provider.id];
        const staleAfterMs = getStaleAfterMs(refreshIntervalSeconds);
        evaluateStaleUsageNotifications({
          providerId: provider.id,
          providerName: metadata.name,
          updatedAt: provider.usage.updatedAt,
          showNotifications: showNotifications && notifyStaleUsage,
          staleAfterMs,
          stateMap: staleUsageNotificationRef.current,
          notify: (title, body) => void sendNotification({ title, body }),
        });
      });
    }, intervalMs);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [refreshIntervalSeconds, showNotifications, notifyStaleUsage]);


  useEffect(() => {
    let active = true;

    if (!pollProviderStatus) {
      (Object.keys(PROVIDERS) as ProviderId[]).forEach((providerId) => {
        setProviderStatus(providerId, null);
      });
      return () => {
        active = false;
      };
    }

    const pollStatus = async () => {
      try {
        const statuses = await invoke<Record<ProviderId, ProviderIncident | null>>(
          'poll_provider_statuses'
        );
        if (!active) return;
        const staleAfterMs = getStaleAfterMs(refreshIntervalSeconds);
        Object.entries(statuses).forEach(([providerId, status]) => {
          if (status?.updatedAt && isTimestampStale(status.updatedAt, staleAfterMs)) {
            return;
          }
          setProviderStatus(providerId as ProviderId, status);
        });
      } catch (e) {
        console.error('Failed to poll provider status:', e);
      }
    };

    pollStatus();
    if (refreshIntervalSeconds <= 0) {
      return () => {
        active = false;
      };
    }

    const interval = window.setInterval(
      pollStatus,
      refreshIntervalSeconds * 1000
    );

    return () => {
      active = false;
      window.clearInterval(interval);
    };
  }, [pollProviderStatus, refreshIntervalSeconds, setProviderStatus]);

  const handleOpenSettings = () => {
    setCurrentView('settings');
  };

  const handleBackToMain = () => {
    setCurrentView('main');
  };

  if (currentView === 'settings') {
    return <SettingsPanel onBack={handleBackToMain} />;
  }

  return <PopupWindow onOpenSettings={handleOpenSettings} />;
}

export default App;
