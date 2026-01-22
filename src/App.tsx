import { useEffect, useRef, useState } from 'react';
import { listen } from '@tauri-apps/api/event';
import { PopupWindow } from './components/PopupWindow';
import { SettingsPanel } from './components/SettingsPanel';
import { useUsageStore } from './stores/usageStore';
import { useSettingsStore } from './stores/settingsStore';
import type { UsageUpdateEvent } from './lib/types';
import './styles/globals.css';

type View = 'main' | 'settings';

function App() {
  const [currentView, setCurrentView] = useState<View>('main');
  const setProviderUsage = useUsageStore((s) => s.setProviderUsage);
  const initializeProviders = useUsageStore((s) => s.initializeProviders);
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const initializedRef = useRef(false);

  // Initialize enabled providers from settings (only once on mount)
  useEffect(() => {
    if (!initializedRef.current) {
      initializedRef.current = true;
      initializeProviders(enabledProviders);
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
      setProviderUsage(event.payload.providerId, event.payload.usage);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [setProviderUsage]);

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
