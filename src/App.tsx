import { useCallback, useEffect, useMemo, useRef } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { check } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { sendNotification } from '@tauri-apps/plugin-notification';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { PopupWindow } from './components/PopupWindow';
import { SettingsPanel } from './components/SettingsPanel';
import { useUsageStore } from './stores/usageStore';
import { useSettingsStore } from './stores/settingsStore';
import type { ProviderId, ProviderIncident, RefreshingEvent, UpdateChannel, UsageUpdateEvent } from './lib/types';
import { parseUsageUpdateEvent } from './lib/eventValidation';
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

interface AuthStatus {
  authenticated: boolean;
}

function App() {
  const isSettingsWindow = useMemo(
    () => new URLSearchParams(window.location.search).get('view') === 'settings',
    []
  );
  const setProviderUsage = useUsageStore((s) => s.setProviderUsage);
  const setProviderStatus = useUsageStore((s) => s.setProviderStatus);
  const initializeProviders = useUsageStore((s) => s.initializeProviders);
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const hasHydrated = useSettingsStore((s) => s.hasHydrated);
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
  const employerReportingEnabled = useSettingsStore((s) => s.employerReportingEnabled);
  const employerReportingGatewayUrl = useSettingsStore((s) => s.employerReportingGatewayUrl);
  const employerReportingEmployeeId = useSettingsStore((s) => s.employerReportingEmployeeId);
  const employerReportingEmployeeEmail = useSettingsStore((s) => s.employerReportingEmployeeEmail);
  const employerReportingIncludeEmployeeEmail = useSettingsStore(
    (s) => s.employerReportingIncludeEmployeeEmail
  );
  const initAutostart = useSettingsStore((s) => s.initAutostart);
  const setInstallOrigin = useSettingsStore((s) => s.setInstallOrigin);
  const initializedRef = useRef(false);
  const enabledProvidersRef = useRef<ProviderId[]>([]);
  const notificationStateRef = useRef(new Map<ProviderId, SessionNotificationState>());
  const creditsNotificationStateRef = useRef(new Map<ProviderId, CreditsNotificationState>());
  const refreshFailureNotificationRef = useRef(
    new Map<ProviderId, RefreshFailureNotificationState>()
  );
  const staleUsageNotificationRef = useRef(new Map<ProviderId, StaleUsageNotificationState>());
  const lastUpdateCheckChannelRef = useRef<UpdateChannel | null>(null);
  const employerOpenReportSentRef = useRef(false);

  // Initialize enabled providers from settings (only once after hydration)
  useEffect(() => {
    // Wait for settings to hydrate from localStorage before initializing
    if (!hasHydrated) {
      return;
    }
    if (!initializedRef.current) {
      initializedRef.current = true;
      restoreSafeStateAfterCrash();
      initializeProviders(enabledProviders);
      enabledProvidersRef.current = enabledProviders;
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
  }, [hasHydrated, enabledProviders, initializeProviders, initAutostart, setInstallOrigin]);

  useEffect(() => {
    let active = true;

    const autoEnableAuthenticatedProviders = async () => {
      try {
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        if (!active) return;
        const settingsStore = useSettingsStore.getState();
        const usageStore = useUsageStore.getState();

        for (const [id, providerStatus] of Object.entries(status)) {
          if (!active) {
            return;
          }
          if (providerStatus?.authenticated !== true) {
            continue;
          }
          if (!(id in PROVIDERS)) {
            continue;
          }
          const providerId = id as ProviderId;
          if (settingsStore.enabledProviders.includes(providerId)) {
            continue;
          }
          settingsStore.enableProvider(providerId);
          void settingsStore.syncProviderEnabled(providerId, true);
          usageStore.setProviderEnabled(providerId, true);
        }
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
    if (isSettingsWindow) {
      return;
    }

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
        // relaunch() fails in dev mode (no binary exists), skip it during development
        if (!import.meta.env.DEV) {
          await relaunch();
        }
      } catch (error) {
        console.warn('Auto-update check failed', error);
      }
    };

    void checkForUpdates();
  }, [autoUpdateEnabled, isSettingsWindow, updateChannel]);

  useEffect(() => {
    invoke('set_debug_file_logging', { enabled: debugFileLogging }).catch(console.error);
  }, [debugFileLogging]);

  useEffect(() => {
    invoke('set_debug_keep_cli_sessions_alive', {
      enabled: debugKeepCliSessionsAlive,
    }).catch(console.error);
  }, [debugKeepCliSessionsAlive]);

  useEffect(() => {
    invoke('set_debug_random_blink', { enabled: debugRandomBlink }).catch(console.error);
  }, [debugRandomBlink]);

  useEffect(() => {
    invoke('set_redact_personal_info', { enabled: redactPersonalInfo }).catch(console.error);
  }, [redactPersonalInfo]);

  useEffect(() => {
    if (!hasHydrated) {
      return;
    }

    const payload = {
      enabled: employerReportingEnabled,
      gatewayUrl: employerReportingGatewayUrl,
      employeeId: employerReportingEmployeeId,
      employeeEmail:
        employerReportingEmployeeEmail && employerReportingEmployeeEmail.trim().length > 0
          ? employerReportingEmployeeEmail.trim()
          : null,
      includeEmployeeEmail: employerReportingIncludeEmployeeEmail,
    };

    invoke('set_employer_reporting_config', { config: payload }).catch((error) => {
      console.error('Failed to sync employer reporting config:', error);
    });
  }, [
    hasHydrated,
    employerReportingEnabled,
    employerReportingGatewayUrl,
    employerReportingEmployeeId,
    employerReportingEmployeeEmail,
    employerReportingIncludeEmployeeEmail,
  ]);

  useEffect(() => {
    if (!employerReportingEnabled) {
      employerOpenReportSentRef.current = false;
    }
  }, [employerReportingEnabled]);

  useEffect(() => {
    if (!hasHydrated || isSettingsWindow || !employerReportingEnabled) {
      return;
    }

    if (employerOpenReportSentRef.current) {
      return;
    }
    employerOpenReportSentRef.current = true;

    const timeoutId = window.setTimeout(() => {
      invoke('send_employer_usage_report', {
        reason: 'open',
        force: false,
      }).catch((error) => {
        console.error('Failed to send open employer report:', error);
      });
    }, 12_000);

    return () => {
      window.clearTimeout(timeoutId);
    };
  }, [hasHydrated, isSettingsWindow, employerReportingEnabled]);

  useEffect(() => {
    if (!hasHydrated || isSettingsWindow || !employerReportingEnabled) {
      return undefined;
    }

    const intervalId = window.setInterval(() => {
      invoke('send_employer_usage_report', {
        reason: 'daily',
        force: false,
      }).catch((error) => {
        console.error('Failed to send daily employer report:', error);
      });
    }, 24 * 60 * 60 * 1000);

    return () => {
      window.clearInterval(intervalId);
    };
  }, [hasHydrated, isSettingsWindow, employerReportingEnabled]);

  // Sync enabled providers when settings change (only after hydration)
  useEffect(() => {
    if (initializedRef.current && hasHydrated) {
      initializeProviders(enabledProviders);
      enabledProvidersRef.current = enabledProviders;
    }
  }, [hasHydrated, enabledProviders, initializeProviders]);

  useEffect(() => {
    if (isSettingsWindow) {
      return undefined;
    }
    const syncFromSettings = (payload?: { enabledProviders?: ProviderId[]; providerOrder?: ProviderId[] }) => {
      if (payload?.enabledProviders || payload?.providerOrder) {
        useSettingsStore.setState({
          enabledProviders: payload?.enabledProviders ?? useSettingsStore.getState().enabledProviders,
          providerOrder: payload?.providerOrder ?? useSettingsStore.getState().providerOrder,
        });
      }

      const nextEnabled = payload?.enabledProviders ?? useSettingsStore.getState().enabledProviders;
      const prevEnabled = enabledProvidersRef.current;
      
      // Find newly enabled providers (were not in previous list but are in new list)
      const newlyEnabled = nextEnabled.filter((id) => !prevEnabled.includes(id));

      if (nextEnabled.join('|') === prevEnabled.join('|')) {
        return;
      }

      enabledProvidersRef.current = nextEnabled;
      useUsageStore.getState().initializeProviders(nextEnabled);

      const usageState = useUsageStore.getState();
      if (!nextEnabled.includes(usageState.activeProvider)) {
        useUsageStore.getState().setActiveProvider(nextEnabled[0] ?? 'claude');
      }
      
      // Refresh newly enabled providers to fetch their usage data
      for (const providerId of newlyEnabled) {
        useUsageStore.getState().refreshProvider(providerId);
      }
    };

    const unlistenSettings = listen<{ enabledProviders?: ProviderId[]; providerOrder?: ProviderId[] }>(
      'settings-updated',
      (event) => {
        syncFromSettings(event.payload);
      }
    );

    return () => {
      void unlistenSettings.then((fn) => fn()).catch(console.error);
    };
  }, [isSettingsWindow]);

  // Listen for usage updates from Rust backend
  useEffect(() => {
    if (isSettingsWindow) {
      return undefined;
    }

    const unlisten = listen<UsageUpdateEvent>('usage-updated', (event) => {
      const parsedUsageUpdate = parseUsageUpdateEvent(event.payload);
      if (!parsedUsageUpdate) {
        return;
      }
      const { providerId, usage } = parsedUsageUpdate;
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
      void unlisten.then((fn) => fn()).catch(console.error);
    };
  }, [
    isSettingsWindow,
    notifyCreditsLow,
    notifyRefreshFailure,
    notifySessionUsage,
    setProviderUsage,
    showNotifications,
  ]);

  useEffect(() => {
    if (isSettingsWindow) {
      return undefined;
    }

    const unlistenRefresh = listen('refresh-requested', () => {
      useUsageStore.getState().refreshAllProviders();
    });

    return () => {
      void unlistenRefresh.then((fn) => fn()).catch(console.error);
    };
  }, [isSettingsWindow]);

  useEffect(() => {
    const unlistenRefreshing = listen<RefreshingEvent>('refreshing-provider', (event) => {
      useUsageStore.getState().setProviderLoading(
        event.payload.providerId,
        event.payload.isRefreshing
      );
    });

    return () => {
      void unlistenRefreshing.then((fn) => fn()).catch(console.error);
    };
  }, []);

  useEffect(() => {
    if (isSettingsWindow) {
      return undefined;
    }

    const unlistenRefreshFailure = listen<UsageUpdateEvent>('refresh-failed', (event) => {
      const parsedUsageUpdate = parseUsageUpdateEvent(event.payload);
      if (!parsedUsageUpdate) return;
      const { providerId, usage } = parsedUsageUpdate;
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
      void unlistenRefreshFailure.then((fn) => fn()).catch(console.error);
    };
  }, [isSettingsWindow, showNotifications, notifyRefreshFailure]);

  useEffect(() => {
    if (isSettingsWindow || refreshIntervalSeconds <= 0) return undefined;

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
  }, [isSettingsWindow, refreshIntervalSeconds, showNotifications, notifyStaleUsage]);


  useEffect(() => {
    let active = true;

    if (isSettingsWindow) {
      return () => {
        active = false;
      };
    }

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
  }, [isSettingsWindow, pollProviderStatus, refreshIntervalSeconds, setProviderStatus]);

  const handleOpenSettings = useCallback(async () => {
    try {
      await invoke('open_settings_window');
      if (!isSettingsWindow) {
        const win = getCurrentWindow();
        await win.hide();
      }
    } catch (error) {
      console.error('Failed to open settings window', error);
    }
  }, [isSettingsWindow]);

  if (isSettingsWindow) {
    return <SettingsPanel showTabs />;
  }

  return <PopupWindow onOpenSettings={handleOpenSettings} />;
}

export default App;
