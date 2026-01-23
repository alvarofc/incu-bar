import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
import { sendNotification } from '@tauri-apps/plugin-notification';
import { PopupWindow } from './components/PopupWindow';
import { SettingsPanel } from './components/SettingsPanel';
import { useUsageStore } from './stores/usageStore';
import { useSettingsStore } from './stores/settingsStore';
import type { ProviderId, ProviderIncident, RefreshingEvent, UsageUpdateEvent } from './lib/types';
import type { CreditsNotificationState, SessionNotificationState } from './lib/notifications';
import { evaluateCreditsNotifications, evaluateSessionNotifications } from './lib/notifications';
import { PROVIDERS } from './lib/providers';
import { restoreSafeStateAfterCrash } from './lib/crashRecovery';
import './styles/globals.css';

type View = 'main' | 'settings';

function App() {
  const [currentView, setCurrentView] = useState<View>('main');
  const setProviderUsage = useUsageStore((s) => s.setProviderUsage);
  const setProviderStatus = useUsageStore((s) => s.setProviderStatus);
  const initializeProviders = useUsageStore((s) => s.initializeProviders);
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const refreshIntervalSeconds = useSettingsStore((s) => s.refreshIntervalSeconds);
  const showNotifications = useSettingsStore((s) => s.showNotifications);
  const initAutostart = useSettingsStore((s) => s.initAutostart);
  const initializedRef = useRef(false);
  const notificationStateRef = useRef(new Map<ProviderId, SessionNotificationState>());
  const creditsNotificationStateRef = useRef(new Map<ProviderId, CreditsNotificationState>());

  // Initialize enabled providers from settings (only once on mount)
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      restoreSafeStateAfterCrash();
      initializeProviders(enabledProviders);
      // Sync autostart status from system
      initAutostart();
    }
  }, []); // eslint-disable-line react-hooks/exhaustive-deps

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
        showNotifications,
        stateMap: notificationStateRef.current,
        notify: (title, body) => void sendNotification({ title, body }),
      });
      evaluateCreditsNotifications({
        providerId,
        providerName: metadata.name,
        usage,
        showNotifications,
        stateMap: creditsNotificationStateRef.current,
        notify: (title, body) => void sendNotification({ title, body }),
      });
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setProviderUsage, showNotifications]);

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
    let active = true;

    const pollStatus = async () => {
      try {
        const statuses = await invoke<Record<ProviderId, ProviderIncident | null>>(
          'poll_provider_statuses'
        );
        if (!active) return;
        Object.entries(statuses).forEach(([providerId, status]) => {
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
  }, [refreshIntervalSeconds, setProviderStatus]);

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
