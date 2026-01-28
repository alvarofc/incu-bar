import { useCallback, useEffect, useMemo, useState, type DragEvent } from 'react';
import { ArrowLeft, Check, RotateCcw, LogIn, Loader2, AlertCircle, ClipboardPaste, Copy, ExternalLink, ChevronUp, ChevronDown, GripVertical, Download } from 'lucide-react';
import type {
  MenuBarDisplayMode,
  MenuBarDisplayTextMode,
  ResetTimeDisplayMode,
  UpdateChannel,
  UsageBarDisplayMode,
} from '../lib/types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { openUrl } from '@tauri-apps/plugin-opener';
import type { ProviderId, CookieSource } from '../lib/types';
import { PROVIDERS } from '../lib/providers';
import { COOKIE_SOURCES, COOKIE_SOURCE_LABELS } from '../lib/cookieSources';
import { useSettingsStore } from '../stores/settingsStore';
import { useUsageStore } from '../stores/usageStore';
import { ProviderIcon } from './ProviderIcons';

interface AuthStatus {
  authenticated: boolean;
  method?: string;
  email?: string;
  error?: string;
}

interface LoginResult {
  success: boolean;
  message: string;
  providerId: ProviderId;
}

interface CopilotDeviceCode {
  userCode: string;
  verificationUri: string;
  deviceCode: string;
  expiresIn: number;
  interval: number;
}

// Per-provider UI state for inline login flows
interface ProviderLoginState {
  isLoggingIn: boolean;
  message: string | null;
  isError: boolean;
  showCookieInput: boolean;
  deviceCode: CopilotDeviceCode | null;
  deviceCodeCopied: boolean;
  isPolling: boolean;
}

type SettingsTab = 'providers' | 'preferences' | 'updates' | 'debug' | 'advanced' | 'about';

interface SettingsPanelProps {
  onBack: () => void;
  showTabs?: boolean;
}

export function SettingsPanel({ onBack, showTabs = true }: SettingsPanelProps) {
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const providerOrder = useSettingsStore((s) => s.providerOrder);
  const setProviderOrder = useSettingsStore((s) => s.setProviderOrder);
  const refreshIntervalSeconds = useSettingsStore((s) => s.refreshIntervalSeconds);
  const displayMode = useSettingsStore((s) => s.displayMode);
  const menuBarDisplayMode = useSettingsStore((s) => s.menuBarDisplayMode);
  const menuBarDisplayTextEnabled = useSettingsStore((s) => s.menuBarDisplayTextEnabled);
  const menuBarDisplayTextMode = useSettingsStore((s) => s.menuBarDisplayTextMode);
  const usageBarDisplayMode = useSettingsStore((s) => s.usageBarDisplayMode);
  const resetTimeDisplayMode = useSettingsStore((s) => s.resetTimeDisplayMode);
  const switcherShowsIcons = useSettingsStore((s) => s.switcherShowsIcons);
  const showAllTokenAccountsInMenu = useSettingsStore((s) => s.showAllTokenAccountsInMenu);
  const showCredits = useSettingsStore((s) => s.showCredits);
  const showCost = useSettingsStore((s) => s.showCost);
  const showExtraUsage = useSettingsStore((s) => s.showExtraUsage);
  const storeUsageHistory = useSettingsStore((s) => s.storeUsageHistory);
  const pollProviderStatus = useSettingsStore((s) => s.pollProviderStatus);
  const redactPersonalInfo = useSettingsStore((s) => s.redactPersonalInfo);
  const autoUpdateEnabled = useSettingsStore((s) => s.autoUpdateEnabled);
  const updateChannel = useSettingsStore((s) => s.updateChannel);
  const showNotifications = useSettingsStore((s) => s.showNotifications);
  const notifySessionUsage = useSettingsStore((s) => s.notifySessionUsage);
  const notifyCreditsLow = useSettingsStore((s) => s.notifyCreditsLow);
  const notifyRefreshFailure = useSettingsStore((s) => s.notifyRefreshFailure);
  const notifyStaleUsage = useSettingsStore((s) => s.notifyStaleUsage);
  const launchAtLogin = useSettingsStore((s) => s.launchAtLogin);
  const debugMenuEnabled = useSettingsStore((s) => s.debugMenuEnabled);
  const debugFileLogging = useSettingsStore((s) => s.debugFileLogging);
  const debugKeepCliSessionsAlive = useSettingsStore(
    (s) => s.debugKeepCliSessionsAlive
  );
  const debugRandomBlink = useSettingsStore((s) => s.debugRandomBlink);
  const hidePersonalInfo = useSettingsStore((s) => s.hidePersonalInfo);
  const setHidePersonalInfo = useSettingsStore((s) => s.setHidePersonalInfo);
  const setRedactPersonalInfo = useSettingsStore((s) => s.setRedactPersonalInfo);
  const debugDisableKeychainAccess = useSettingsStore((s) => s.debugDisableKeychainAccess);
  const installOrigin = useSettingsStore((s) => s.installOrigin);
  const setMenuBarDisplayTextEnabled = useSettingsStore((s) => s.setMenuBarDisplayTextEnabled);
  const setMenuBarDisplayTextMode = useSettingsStore((s) => s.setMenuBarDisplayTextMode);

  const [authStatus, setAuthStatus] = useState<Record<string, AuthStatus>>({});
  // Per-provider login state for inline flows
  const [providerLoginStates, setProviderLoginStates] = useState<Partial<Record<ProviderId, ProviderLoginState>>>({});
  const [manualCookieInputs, setManualCookieInputs] = useState<Partial<Record<ProviderId, string>>>({});
  const [expandedProvider, setExpandedProvider] = useState<ProviderId | null>(null);
  const [supportExportPath, setSupportExportPath] = useState<string | null>(null);
  const [supportExporting, setSupportExporting] = useState(false);
  const [supportMessage, setSupportMessage] = useState<string | null>(null);
  const cookieSources = useSettingsStore((s) => s.cookieSources);
  const [draggingProviderId, setDraggingProviderId] = useState<ProviderId | null>(null);
  const [dragOverProviderId, setDragOverProviderId] = useState<ProviderId | null>(null);
  const [activeTab, setActiveTab] = useState<SettingsTab>('providers');
  const headerPaddingClass = showTabs ? 'px-6 py-4' : 'px-4 py-3';
  const sectionPaddingClass = showTabs ? 'p-6' : 'p-4';
  const dividerClass = showTabs ? 'divider mx-6' : 'divider mx-4';
  const messageMarginClass = showTabs ? 'mx-6' : 'mx-4';
  const footerPaddingClass = showTabs ? 'px-6 pb-6 pt-3' : 'px-4 pb-4 pt-2';

  const settingsTabs: Array<{ id: SettingsTab; label: string }> = [
    { id: 'providers', label: 'Providers' },
    { id: 'preferences', label: 'Preferences' },
    { id: 'updates', label: 'Updates' },
    { id: 'debug', label: 'Debug' },
    { id: 'advanced', label: 'Advanced' },
    { id: 'about', label: 'About' },
  ];

  // Helper to update per-provider login state
  const updateProviderLoginState = useCallback((providerId: ProviderId, updates: Partial<ProviderLoginState>) => {
    setProviderLoginStates((prev) => ({
      ...prev,
      [providerId]: {
        isLoggingIn: false,
        message: null,
        isError: false,
        showCookieInput: false,
        deviceCode: null,
        deviceCodeCopied: false,
        isPolling: false,
        ...prev[providerId],
        ...updates,
      },
    }));
  }, []);

  const clearProviderLoginState = useCallback((providerId: ProviderId) => {
    setProviderLoginStates((prev) => {
      const next = { ...prev };
      delete next[providerId];
      return next;
    });
    setExpandedProvider(null);
  }, []);

  const syncAuthStatus = useCallback((status: Record<string, AuthStatus>) => {
    setAuthStatus(status);
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
  }, []);

  const orderedProviderIds = useMemo(() => {
    const allProviders = Object.keys(PROVIDERS) as ProviderId[];
    const uniqueOrder = providerOrder.filter((id) => allProviders.includes(id));
    const missingProviders = allProviders.filter((id) => !uniqueOrder.includes(id));
    return [...uniqueOrder, ...missingProviders];
  }, [providerOrder]);

  const implementedProviders = useMemo(
    () => orderedProviderIds.filter((id) => PROVIDERS[id].implemented),
    [orderedProviderIds]
  );

  const upcomingProviders = useMemo(
    () => orderedProviderIds.filter((id) => !PROVIDERS[id].implemented),
    [orderedProviderIds]
  );

  useEffect(() => {
    const checkAuth = async () => {
      try {
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        syncAuthStatus(status);
      } catch (e) {
        console.error('Failed to check auth status:', e);
      }
    };
    checkAuth();
  }, [syncAuthStatus]);

  useEffect(() => {
    if (orderedProviderIds.join('|') !== providerOrder.join('|')) {
      setProviderOrder(orderedProviderIds);
    }
  }, [orderedProviderIds, providerOrder, setProviderOrder]);

  useEffect(() => {
    const unlistenLogin = listen('cursor-login-detected', async () => {
      updateProviderLoginState('cursor', { message: 'Login detected! Extracting cookies…' });
      try {
        const result = await invoke<LoginResult>('extract_cursor_cookies');
        if (result.success) {
          updateProviderLoginState('cursor', { 
            message: result.message, 
            isError: false,
            showCookieInput: false,
            isLoggingIn: false,
          });
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          syncAuthStatus(status);
          const settingsStore = useSettingsStore.getState();
          settingsStore.enableProvider('cursor');
          void settingsStore.syncProviderEnabled('cursor', true);
          useUsageStore.getState().setProviderEnabled('cursor', true);
          useUsageStore.getState().refreshProvider('cursor');
          // Auto-collapse after success
          setTimeout(() => clearProviderLoginState('cursor'), 2000);
        }
      } catch (e) {
        updateProviderLoginState('cursor', { 
          message: `Cookie extraction error: ${e}`,
          isError: true,
          showCookieInput: true,
          isLoggingIn: false,
        });
      }
    });

    const unlistenCompleted = listen('login-completed', async (event: { payload: { providerId: ProviderId; success: boolean; message: string } }) => {
      const { providerId, success, message } = event.payload;
      if (success) {
        updateProviderLoginState(providerId, { 
          message, 
          isError: false,
          showCookieInput: false,
          isLoggingIn: false,
        });
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        syncAuthStatus(status);
        const settingsStore = useSettingsStore.getState();
        settingsStore.enableProvider(providerId);
        void settingsStore.syncProviderEnabled(providerId, true);
        useUsageStore.getState().setProviderEnabled(providerId, true);
        useUsageStore.getState().refreshProvider(providerId);
        // Auto-collapse after success
        setTimeout(() => clearProviderLoginState(providerId), 2000);
      }
    });

    return () => {
      unlistenLogin.then(fn => fn());
      unlistenCompleted.then(fn => fn());
    };
  }, [updateProviderLoginState, clearProviderLoginState, syncAuthStatus]);

  const handleToggleProvider = useCallback((id: ProviderId) => {
    const store = useSettingsStore.getState();
    const currentlyEnabled = store.enabledProviders.includes(id);
    store.toggleProvider(id);
    void store.syncProviderEnabled(id, !currentlyEnabled);
    useUsageStore.getState().setProviderEnabled(id, !currentlyEnabled);
  }, []);

  const handleMoveProvider = useCallback((id: ProviderId, direction: 'up' | 'down') => {
    const currentIndex = implementedProviders.indexOf(id);
    if (currentIndex === -1) return;
    const nextIndex = direction === 'up' ? currentIndex - 1 : currentIndex + 1;
    if (nextIndex < 0 || nextIndex >= implementedProviders.length) return;

    const reorderedProviders = [...implementedProviders];
    [reorderedProviders[currentIndex], reorderedProviders[nextIndex]] = [
      reorderedProviders[nextIndex],
      reorderedProviders[currentIndex],
    ];

    setProviderOrder([...reorderedProviders, ...upcomingProviders]);
  }, [implementedProviders, upcomingProviders, setProviderOrder]);

  const handleDragStart = useCallback((event: DragEvent, id: ProviderId) => {
    event.dataTransfer.effectAllowed = 'move';
    event.dataTransfer.setData('text/plain', id);
    setDraggingProviderId(id);
  }, []);

  const handleDragOver = useCallback((event: DragEvent, id: ProviderId) => {
    event.preventDefault();
    setDragOverProviderId(id);
  }, []);

  const handleDrop = useCallback((event: DragEvent, targetId: ProviderId) => {
    event.preventDefault();
    const sourceId = draggingProviderId ?? (event.dataTransfer.getData('text/plain') as ProviderId | undefined);
    if (!sourceId || sourceId === targetId) {
      setDragOverProviderId(null);
      setDraggingProviderId(null);
      return;
    }

    const reorderedProviders = [...implementedProviders];
    const sourceIndex = reorderedProviders.indexOf(sourceId);
    const targetIndex = reorderedProviders.indexOf(targetId);
    if (sourceIndex === -1 || targetIndex === -1) {
      setDragOverProviderId(null);
      setDraggingProviderId(null);
      return;
    }

    reorderedProviders.splice(sourceIndex, 1);
    reorderedProviders.splice(targetIndex, 0, sourceId);
    setProviderOrder([...reorderedProviders, ...upcomingProviders]);
    setDragOverProviderId(null);
    setDraggingProviderId(null);
  }, [draggingProviderId, implementedProviders, upcomingProviders, setProviderOrder]);

  const handleDragEnd = useCallback(() => {
    setDraggingProviderId(null);
    setDragOverProviderId(null);
  }, []);

  const handleSetRefreshInterval = useCallback((seconds: number) => {
    useSettingsStore.getState().setRefreshInterval(seconds);
  }, []);

  const handleSetShowCredits = useCallback((show: boolean) => {
    useSettingsStore.getState().setShowCredits(show);
  }, []);

  const handleSetShowCost = useCallback((show: boolean) => {
    useSettingsStore.getState().setShowCost(show);
  }, []);

  const handleSetShowExtraUsage = useCallback((show: boolean) => {
    useSettingsStore.getState().setShowExtraUsage(show);
  }, []);

  const handleSetStoreUsageHistory = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setStoreUsageHistory(enabled);
    if (!enabled) {
      useUsageStore.getState().clearUsageHistory();
    }
    useUsageStore.getState().syncUsageHistoryStorage();
  }, []);

  const handleSetPollProviderStatus = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setPollProviderStatus(enabled);
  }, []);

  const handleSetShowNotifications = useCallback((show: boolean) => {
    useSettingsStore.getState().setShowNotifications(show);
  }, []);

  const handleSetNotifySessionUsage = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setNotifySessionUsage(enabled);
  }, []);

  const handleSetNotifyCreditsLow = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setNotifyCreditsLow(enabled);
  }, []);

  const handleSetNotifyRefreshFailure = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setNotifyRefreshFailure(enabled);
  }, []);

  const handleSetNotifyStaleUsage = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setNotifyStaleUsage(enabled);
  }, []);

  const handleSetDisplayMode = useCallback((mode: 'merged' | 'separate') => {
    useSettingsStore.getState().setDisplayMode(mode);
  }, []);

  const handleSetMenuBarDisplayMode = useCallback((mode: MenuBarDisplayMode) => {
    useSettingsStore.getState().setMenuBarDisplayMode(mode);
    void invoke('save_menu_bar_display_settings', {
      menuBarDisplayMode: mode,
      menuBarDisplayTextEnabled,
      menuBarDisplayTextMode,
      usageBarDisplayMode,
    });
  }, [
    menuBarDisplayTextEnabled,
    menuBarDisplayTextMode,
    usageBarDisplayMode,
  ]);

  const handleSetMenuBarDisplayTextEnabled = useCallback((enabled: boolean) => {
    setMenuBarDisplayTextEnabled(enabled);
    void invoke('save_menu_bar_display_settings', {
      menuBarDisplayMode,
      menuBarDisplayTextEnabled: enabled,
      menuBarDisplayTextMode,
      usageBarDisplayMode,
    });
  }, [
    setMenuBarDisplayTextEnabled,
    menuBarDisplayMode,
    menuBarDisplayTextMode,
    usageBarDisplayMode,
  ]);

  const handleSetMenuBarDisplayTextMode = useCallback((mode: MenuBarDisplayTextMode) => {
    setMenuBarDisplayTextMode(mode);
    void invoke('save_menu_bar_display_settings', {
      menuBarDisplayMode,
      menuBarDisplayTextEnabled,
      menuBarDisplayTextMode: mode,
      usageBarDisplayMode,
    });
  }, [
    setMenuBarDisplayTextMode,
    menuBarDisplayMode,
    menuBarDisplayTextEnabled,
    usageBarDisplayMode,
  ]);

  const handleSetUsageBarDisplayMode = useCallback((mode: UsageBarDisplayMode) => {
    useSettingsStore.getState().setUsageBarDisplayMode(mode);
    void invoke('save_menu_bar_display_settings', {
      menuBarDisplayMode,
      menuBarDisplayTextEnabled,
      menuBarDisplayTextMode,
      usageBarDisplayMode: mode,
    });
  }, [
    menuBarDisplayMode,
    menuBarDisplayTextEnabled,
    menuBarDisplayTextMode,
  ]);

  const handleSetResetTimeDisplayMode = useCallback((mode: ResetTimeDisplayMode) => {
    useSettingsStore.getState().setResetTimeDisplayMode(mode);
  }, []);

  const handleSetSwitcherShowsIcons = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setSwitcherShowsIcons(enabled);
  }, []);

  const handleSetShowAllTokenAccountsInMenu = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setShowAllTokenAccountsInMenu(enabled);
  }, []);

  const handleSetAutoUpdateEnabled = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setAutoUpdateEnabled(enabled);
  }, []);

  const handleSetUpdateChannel = useCallback((channel: UpdateChannel) => {
    useSettingsStore.getState().setUpdateChannel(channel);
  }, []);

  const handleSetLaunchAtLogin = useCallback((launch: boolean) => {
    useSettingsStore.getState().setLaunchAtLogin(launch);
  }, []);

  const handleSetDebugMenuEnabled = useCallback((enabled: boolean) => {
    const store = useSettingsStore.getState();
    store.setDebugMenuEnabled(enabled);
    if (!enabled) {
      store.setDebugFileLogging(false);
      store.setDebugKeepCliSessionsAlive(false);
      store.setDebugRandomBlink(false);
    }
  }, []);

  const handleSetDebugFileLogging = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setDebugFileLogging(enabled);
  }, []);

  const handleSetDebugKeepCliSessionsAlive = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setDebugKeepCliSessionsAlive(enabled);
  }, []);

  const handleSetDebugRandomBlink = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setDebugRandomBlink(enabled);
  }, []);

  const handleSetDebugDisableKeychainAccess = useCallback((enabled: boolean) => {
    useSettingsStore.getState().setDebugDisableKeychainAccess(enabled);
  }, []);

  const handleResetToDefaults = useCallback(() => {
    useSettingsStore.getState().resetToDefaults();
  }, []);

  const handleLogin = useCallback(async (providerId: ProviderId) => {
    setExpandedProvider(providerId);
    updateProviderLoginState(providerId, { isLoggingIn: true, message: null, isError: false });
    
    try {
      // Cookie-based providers
      const cookieProviders: ProviderId[] = ['cursor', 'factory', 'augment', 'kimi', 'minimax', 'amp', 'opencode'];
      if (cookieProviders.includes(providerId)) {
        const cookieSource = useSettingsStore.getState().getCookieSource(providerId);
        if (cookieSource === 'manual') {
          updateProviderLoginState(providerId, { 
            message: 'Paste your Cookie header below to continue.',
            showCookieInput: true,
            isLoggingIn: false,
          });
          return;
        }

        updateProviderLoginState(providerId, { message: `Importing cookies from ${COOKIE_SOURCE_LABELS[cookieSource]}…` });
        
        const importCommands: Record<string, string> = {
          cursor: 'import_cursor_browser_cookies_from_source',
          factory: 'import_factory_browser_cookies_from_source',
          augment: 'import_augment_browser_cookies_from_source',
          kimi: 'import_kimi_browser_cookies_from_source',
          minimax: 'import_minimax_browser_cookies_from_source',
          amp: 'import_amp_browser_cookies_from_source',
          opencode: 'import_opencode_browser_cookies_from_source',
        };
        
        const importResult = await invoke<LoginResult>(importCommands[providerId], {
          source: { source: cookieSource },
        });

        if (importResult.success) {
          updateProviderLoginState(providerId, { 
            message: importResult.message,
            isError: false,
            isLoggingIn: false,
          });
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          syncAuthStatus(status);
          const settingsStore = useSettingsStore.getState();
          settingsStore.enableProvider(providerId);
          void settingsStore.syncProviderEnabled(providerId, true);
          useUsageStore.getState().setProviderEnabled(providerId, true);
          useUsageStore.getState().refreshProvider(providerId);
          setTimeout(() => clearProviderLoginState(providerId), 2000);
          return;
        }

        updateProviderLoginState(providerId, { 
          message: 'Could not import from browser. You can paste cookies manually below.',
          isError: true,
          showCookieInput: true,
          isLoggingIn: false,
        });
        return;
      }

      // CLI-based providers
      if (providerId === 'kiro') {
        updateProviderLoginState(providerId, { 
          message: 'Kiro uses the CLI. Run `kiro-cli login` in Terminal, then refresh.',
          isLoggingIn: false,
        });
        return;
      }

      // Auto-detect providers
      if (providerId === 'antigravity') {
        updateProviderLoginState(providerId, { 
          message: 'Launch Antigravity to connect automatically. Usage is detected when the app is running.',
          isLoggingIn: false,
        });
        return;
      }

      if (providerId === 'jetbrains') {
        updateProviderLoginState(providerId, { 
          message: 'Open a JetBrains IDE with AI Assistant enabled to connect automatically.',
          isLoggingIn: false,
        });
        return;
      }
      
      // Copilot device flow
      if (providerId === 'copilot') {
        updateProviderLoginState(providerId, { message: 'Requesting device code from GitHub…' });
        const deviceCode = await invoke<CopilotDeviceCode>('copilot_request_device_code');
        updateProviderLoginState(providerId, { 
          deviceCode,
          deviceCodeCopied: false,
          message: null,
          isLoggingIn: false,
        });
        return;
      }
      
      // Generic OAuth/login flow
      const result = await invoke<LoginResult>('start_login', { providerId });

      if (result.success) {
        updateProviderLoginState(providerId, { 
          message: `${PROVIDERS[providerId].name} connected!`,
          isError: false,
          isLoggingIn: false,
        });
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        syncAuthStatus(status);
        const settingsStore = useSettingsStore.getState();
        settingsStore.enableProvider(providerId);
        void settingsStore.syncProviderEnabled(providerId, true);
        useUsageStore.getState().setProviderEnabled(providerId, true);
        useUsageStore.getState().refreshProvider(providerId);
        setTimeout(() => clearProviderLoginState(providerId), 2000);
      } else {
        updateProviderLoginState(providerId, { 
          message: result.message,
          isError: true,
          isLoggingIn: false,
        });
      }
    } catch (e) {
      updateProviderLoginState(providerId, { 
        message: `Login failed: ${e}`,
        isError: true,
        isLoggingIn: false,
      });
    }
  }, [updateProviderLoginState, clearProviderLoginState, syncAuthStatus]);

  const handleSubmitCookies = useCallback(async (providerId: ProviderId) => {
    const cookieHeader = manualCookieInputs[providerId]?.trim() ?? '';
    if (!cookieHeader) {
      updateProviderLoginState(providerId, { message: 'Please paste your cookies first', isError: true });
      return;
    }

    updateProviderLoginState(providerId, { isLoggingIn: true });
    try {
      const storeCommands: Record<string, string> = {
        factory: 'store_factory_cookies',
        augment: 'store_augment_cookies',
        kimi: 'store_kimi_cookies',
        minimax: 'store_minimax_cookies',
        amp: 'store_amp_cookies',
        opencode: 'store_opencode_cookies',
        codex: 'store_codex_cookies',
        cursor: 'store_cursor_cookies',
      };
      const storeCommand = storeCommands[providerId] || 'store_cursor_cookies';
      const result = await invoke<LoginResult>(storeCommand, { cookieHeader });
      
      if (result.success) {
        updateProviderLoginState(providerId, { 
          message: `${PROVIDERS[providerId].name} cookies saved!`,
          isError: false,
          showCookieInput: false,
          isLoggingIn: false,
        });
        setManualCookieInputs((state) => ({ ...state, [providerId]: '' }));
        if (providerId === 'cursor') {
          await invoke('close_cursor_login');
        }
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        syncAuthStatus(status);
        const settingsStore = useSettingsStore.getState();
        settingsStore.enableProvider(providerId);
        void settingsStore.syncProviderEnabled(providerId, true);
        useUsageStore.getState().setProviderEnabled(providerId, true);
        useUsageStore.getState().refreshProvider(providerId);
        setTimeout(() => clearProviderLoginState(providerId), 2000);
      } else {
        updateProviderLoginState(providerId, { 
          message: result.message,
          isError: true,
          isLoggingIn: false,
        });
      }
    } catch (e) {
      updateProviderLoginState(providerId, { 
        message: `Failed to save cookies: ${e}`,
        isError: true,
        isLoggingIn: false,
      });
    }
  }, [manualCookieInputs, updateProviderLoginState, clearProviderLoginState, syncAuthStatus]);

  const handleImportBrowserCookies = useCallback(async (providerId: ProviderId) => {
    if (debugDisableKeychainAccess) {
      updateProviderLoginState(providerId, { 
        message: 'Keychain access is disabled. Paste cookies manually to continue.',
        showCookieInput: true,
      });
      setExpandedProvider(providerId);
      return;
    }
    
    updateProviderLoginState(providerId, { isLoggingIn: true });
    setExpandedProvider(providerId);
    
    const cookieSource = useSettingsStore.getState().getCookieSource(providerId);
    if (cookieSource === 'manual') {
      updateProviderLoginState(providerId, { 
        message: 'Paste your Cookie header below to continue.',
        showCookieInput: true,
        isLoggingIn: false,
      });
      return;
    }
    
    updateProviderLoginState(providerId, { message: `Importing cookies from ${COOKIE_SOURCE_LABELS[cookieSource]}…` });

    const importCommands: Record<string, string> = {
      factory: 'import_factory_browser_cookies_from_source',
      augment: 'import_augment_browser_cookies_from_source',
      kimi: 'import_kimi_browser_cookies_from_source',
      minimax: 'import_minimax_browser_cookies_from_source',
      amp: 'import_amp_browser_cookies_from_source',
      opencode: 'import_opencode_browser_cookies_from_source',
      codex: 'import_codex_browser_cookies_from_source',
      cursor: 'import_cursor_browser_cookies_from_source',
    };
    const importCommand = importCommands[providerId] || 'import_cursor_browser_cookies_from_source';

    try {
      const result = await invoke<LoginResult>(importCommand, {
        source: { source: cookieSource },
      });
      if (result.success) {
        updateProviderLoginState(providerId, { 
          message: result.message,
          isError: false,
          showCookieInput: false,
          isLoggingIn: false,
        });
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        syncAuthStatus(status);
        const settingsStore = useSettingsStore.getState();
        settingsStore.enableProvider(providerId);
        void settingsStore.syncProviderEnabled(providerId, true);
        useUsageStore.getState().setProviderEnabled(providerId, true);
        useUsageStore.getState().refreshProvider(providerId);
        setTimeout(() => clearProviderLoginState(providerId), 2000);
      } else {
        updateProviderLoginState(providerId, { 
          message: result.message,
          isError: true,
          showCookieInput: true,
          isLoggingIn: false,
        });
      }
    } catch (e) {
      updateProviderLoginState(providerId, { 
        message: `Failed to import cookies: ${e}`,
        isError: true,
        showCookieInput: true,
        isLoggingIn: false,
      });
    }
  }, [debugDisableKeychainAccess, updateProviderLoginState, clearProviderLoginState, syncAuthStatus]);

  const handleCookieSourceChange = useCallback((providerId: ProviderId, source: CookieSource) => {
    useSettingsStore.getState().setCookieSource(providerId, source);
  }, []);

  const handleCopyCopilotCode = useCallback(async (providerId: ProviderId) => {
    const state = providerLoginStates[providerId];
    if (!state?.deviceCode) return;
    try {
      await navigator.clipboard.writeText(state.deviceCode.userCode);
      updateProviderLoginState(providerId, { deviceCodeCopied: true });
      setTimeout(() => updateProviderLoginState(providerId, { deviceCodeCopied: false }), 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }, [providerLoginStates, updateProviderLoginState]);

  const handleOpenCopilotVerification = useCallback((providerId: ProviderId) => {
    const state = providerLoginStates[providerId];
    if (!state?.deviceCode) return;
    openUrl(state.deviceCode.verificationUri);
  }, [providerLoginStates]);

  const handleCopilotContinue = useCallback(async (providerId: ProviderId) => {
    const state = providerLoginStates[providerId];
    if (!state?.deviceCode) return;
    
    updateProviderLoginState(providerId, { isPolling: true, message: 'Waiting for authorization…' });
    
    try {
      const result = await invoke<LoginResult>('copilot_poll_for_token', {
        deviceCode: state.deviceCode.deviceCode,
      });
      
      if (result.success) {
        updateProviderLoginState(providerId, { 
          message: result.message,
          isError: false,
          deviceCode: null,
          isPolling: false,
        });
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        syncAuthStatus(status);
        const settingsStore = useSettingsStore.getState();
        settingsStore.enableProvider('copilot');
        void settingsStore.syncProviderEnabled('copilot', true);
        useUsageStore.getState().setProviderEnabled('copilot', true);
        useUsageStore.getState().refreshProvider('copilot');
        setTimeout(() => clearProviderLoginState(providerId), 2000);
      } else {
        updateProviderLoginState(providerId, { 
          message: result.message,
          isError: true,
          isPolling: false,
        });
      }
    } catch (e) {
      updateProviderLoginState(providerId, { 
        message: `Copilot login failed: ${e}`,
        isError: true,
        isPolling: false,
      });
    }
  }, [providerLoginStates, updateProviderLoginState, clearProviderLoginState, syncAuthStatus]);

  const handleCancelLogin = useCallback((providerId: ProviderId) => {
    clearProviderLoginState(providerId);
  }, [clearProviderLoginState]);

  const handleExportSupportBundle = useCallback(async () => {
    if (supportExporting) return;
    setSupportExporting(true);
    setSupportExportPath(null);

    try {
      const settings = useSettingsStore.getState();
      const usage = useUsageStore.getState();
      const providerSnapshots = Object.fromEntries(
        (Object.keys(usage.providers) as ProviderId[]).map((providerId) => {
          const provider = usage.providers[providerId];
          const usageSnapshot = provider.usage ? {
            ...provider.usage,
            identity: provider.usage.identity
              ? { ...provider.usage.identity, email: undefined, name: undefined }
              : undefined,
          } : undefined;

          return [
            providerId,
            {
              enabled: provider.enabled,
              isLoading: provider.isLoading,
              lastError: provider.lastError,
              status: provider.status,
              usage: usageSnapshot,
              usageHistoryCount: provider.usageHistory?.length ?? 0,
              lastUsageHistoryPoint: provider.usageHistory?.slice(-1)[0] ?? null,
            },
          ];
        })
      );

      const payload = {
        appVersion: import.meta.env.PACKAGE_VERSION,
        platform: navigator.platform,
        locale: navigator.language,
        settings: {
          refreshIntervalSeconds: settings.refreshIntervalSeconds,
          enabledProviders: settings.enabledProviders,
          providerOrder: settings.providerOrder,
          displayMode: settings.displayMode,
          menuBarDisplayMode: settings.menuBarDisplayMode,
          menuBarDisplayTextEnabled: settings.menuBarDisplayTextEnabled,
          menuBarDisplayTextMode: settings.menuBarDisplayTextMode,
          usageBarDisplayMode: settings.usageBarDisplayMode,
          resetTimeDisplayMode: settings.resetTimeDisplayMode,
          switcherShowsIcons: settings.switcherShowsIcons,
          showAllTokenAccountsInMenu: settings.showAllTokenAccountsInMenu,
          autoUpdateEnabled: settings.autoUpdateEnabled,
          updateChannel: settings.updateChannel,
          showNotifications: settings.showNotifications,
          notifySessionUsage: settings.notifySessionUsage,
          notifyCreditsLow: settings.notifyCreditsLow,
          notifyRefreshFailure: settings.notifyRefreshFailure,
          notifyStaleUsage: settings.notifyStaleUsage,
          launchAtLogin: settings.launchAtLogin,
          showCredits: settings.showCredits,
          showCost: settings.showCost,
          showExtraUsage: settings.showExtraUsage,
          storeUsageHistory: settings.storeUsageHistory,
          pollProviderStatus: settings.pollProviderStatus,
          redactPersonalInfo: settings.redactPersonalInfo,
          debugMenuEnabled: settings.debugMenuEnabled,
          debugFileLogging: settings.debugFileLogging,
          debugKeepCliSessionsAlive: settings.debugKeepCliSessionsAlive,
          debugRandomBlink: settings.debugRandomBlink,
          hidePersonalInfo: settings.hidePersonalInfo,
          debugDisableKeychainAccess: settings.debugDisableKeychainAccess,
        },
        providers: providerSnapshots,
      };

      const path = await invoke<string>('export_support_bundle', { payload });
      setSupportExportPath(path);
      setSupportMessage('Support bundle exported. Share it with support.');
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setSupportMessage(`Support bundle export failed: ${message}`);
    } finally {
      setSupportExporting(false);
    }
  }, [supportExporting]);

  const refreshIntervals = [
    { label: 'Manual', value: 0 },
    { label: '1m', value: 60 },
    { label: '2m', value: 120 },
    { label: '5m', value: 300 },
    { label: '15m', value: 900 },
  ];

  const keychainPromptSources: CookieSource[] = ['chrome', 'arc', 'edge', 'brave', 'opera'];

  const getAuthMethodLabel = (method: string) => {
    switch (method) {
      case 'oauth': return 'OAuth';
      case 'cookies': return 'Browser';
      case 'cli': return 'CLI';
      case 'api_key': return 'API Key';
      case 'local_config': return 'Local';
      default: return method;
    }
  };

  return (
    <div className="popup-container settings-window">
      {/* Header */}
      <header className={`flex items-center gap-3 border-b border-[var(--border-subtle)] ${headerPaddingClass}`}>
        <button
          onClick={onBack}
          className="btn btn-icon focus-ring"
          aria-label="Go back"
        >
          <ArrowLeft className="w-4 h-4" aria-hidden="true" />
        </button>
        <h1 className="text-[15px] font-semibold text-[var(--text-primary)]">Settings</h1>
      </header>

      {/* Content */}
      <div className="flex-1 overflow-y-auto">
        {showTabs && (
          <nav className="sticky top-0 z-10 bg-[var(--bg-base)]/95 backdrop-blur border-b border-[var(--border-subtle)]">
            <div className="flex gap-2 px-4 py-2 overflow-x-auto">
              {settingsTabs.map((tab) => (
                <button
                  key={tab.id}
                  type="button"
                  onClick={() => setActiveTab(tab.id)}
                  className={`px-3 py-1.5 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    activeTab === tab.id
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                >
                  {tab.label}
                </button>
              ))}
            </div>
          </nav>
        )}
        {/* Support Export Message */}
        {supportMessage && (
          <div className={`${messageMarginClass} mt-4 p-3 rounded-lg bg-[var(--bg-surface)] border border-[var(--border-subtle)]`} role="status" aria-live="polite">
            <p className="text-[13px] text-[var(--text-secondary)]">{supportMessage}</p>
          </div>
        )}

        {/* Providers Section */}
        {(activeTab === 'providers' || !showTabs) && (
        <section className={sectionPaddingClass}>
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Providers
          </h2>
          <div className="space-y-1.5" data-testid="provider-settings-list">
            {implementedProviders.map((id) => {
              const provider = PROVIDERS[id];
              const status = authStatus[id];
              const isAuthenticated = status?.authenticated === true;
              const isEnabled = enabledProviders.includes(id);
              const loginState = providerLoginStates[id];
              const isExpanded = expandedProvider === id || loginState?.isLoggingIn || loginState?.message || loginState?.deviceCode;
              const usesCookies = provider.authMethod === 'cookies' || id === 'codex';
              const cookieSource = usesCookies
                ? cookieSources[id] ?? useSettingsStore.getState().getCookieSource(id)
                : null;
              const showKeychainGuidance = cookieSource && keychainPromptSources.includes(cookieSource);

              return (
                <div key={id} className="space-y-0" data-testid="provider-detail-pane">
                  {/* Compact Provider Row */}
                  <div
                    className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg border transition-colors ${
                      isExpanded
                        ? 'bg-[var(--bg-subtle)] border-[var(--border-strong)] rounded-b-none'
                        : 'bg-[var(--bg-surface)] border-[var(--border-subtle)] hover:bg-[var(--bg-overlay)]'
                    } ${dragOverProviderId === id ? 'ring-1 ring-[var(--accent-primary)]' : ''} ${draggingProviderId === id ? 'opacity-70' : ''}`}
                    onDragOver={(event) => handleDragOver(event, id)}
                    onDrop={(event) => handleDrop(event, id)}
                    onDragLeave={() => setDragOverProviderId(null)}
                    data-testid={`provider-order-item-${id}`}
                  >
                    {/* Drag handle */}
                    <button
                      type="button"
                      onDragStart={(event) => handleDragStart(event, id)}
                      onDragEnd={handleDragEnd}
                      draggable
                      className="btn btn-icon focus-ring"
                      aria-label={`Reorder ${provider.name}`}
                      data-testid={`provider-order-handle-${id}`}
                    >
                      <GripVertical className="w-3.5 h-3.5" aria-hidden="true" />
                    </button>

                    {/* Provider info */}
                    <div className="flex items-center gap-3 flex-1 min-w-0">
                      <ProviderIcon 
                        providerId={id} 
                        className={`w-5 h-5 flex-shrink-0 ${isAuthenticated ? 'opacity-100' : 'opacity-50'}`}
                        aria-hidden="true"
                      />
                      <div className="min-w-0 flex-1">
                        <span className="text-[13px] font-medium text-[var(--text-primary)]">
                          {provider.name}
                        </span>
                        {isAuthenticated ? (
                          <div className="flex items-center gap-1.5 mt-0.5">
                            <span className="w-1.5 h-1.5 rounded-full bg-[var(--accent-success)]" />
                            <span className="text-[11px] text-[var(--text-tertiary)]">
                              Connected{status.method ? ` · ${status.method}` : ''}
                            </span>
                          </div>
                        ) : status?.error ? (
                          <div className="flex items-center gap-1.5 mt-0.5">
                            <AlertCircle className="w-3 h-3 text-[var(--accent-warning)]" aria-hidden="true" />
                            <span className="text-[11px] text-[var(--accent-warning)] truncate">
                              {status.error}
                            </span>
                          </div>
                        ) : (
                          <span className="text-[11px] text-[var(--text-quaternary)] block mt-0.5">
                            Not connected
                          </span>
                        )}
                      </div>
                    </div>

                    {/* Actions: Connect button, toggle, reorder */}
                    <div className="flex items-center gap-1">
                      <button
                        type="button"
                        onClick={() => {
                          if (isExpanded && !loginState?.isLoggingIn) {
                            setExpandedProvider(null);
                          } else {
                            handleLogin(id);
                          }
                        }}
                        disabled={loginState?.isLoggingIn}
                        className="btn btn-sm btn-ghost focus-ring text-[11px]"
                        data-testid={`provider-connect-${id}`}
                      >
                        {loginState?.isLoggingIn ? (
                          <Loader2 className="w-3 h-3 animate-spin" aria-hidden="true" />
                        ) : (
                          <LogIn className="w-3 h-3" aria-hidden="true" />
                        )}
                        <span>{isAuthenticated ? 'Reconnect' : 'Connect'}</span>
                      </button>
                      <button
                        type="button"
                        onClick={() => handleToggleProvider(id)}
                        className="toggle focus-ring"
                        data-state={isEnabled ? 'checked' : 'unchecked'}
                        role="switch"
                        aria-checked={isEnabled}
                        aria-label={isEnabled ? `Hide ${provider.name}` : `Show ${provider.name}`}
                        data-testid={`provider-enable-toggle-${id}`}
                      >
                        <span className="toggle-thumb" />
                      </button>
                      <button
                        type="button"
                        onClick={() => handleMoveProvider(id, 'up')}
                        className="btn btn-icon focus-ring"
                        aria-label={`Move ${provider.name} up`}
                        disabled={implementedProviders[0] === id}
                        data-testid={`provider-order-up-${id}`}
                      >
                        <ChevronUp className="w-3.5 h-3.5" aria-hidden="true" />
                      </button>
                      <button
                        type="button"
                        onClick={() => handleMoveProvider(id, 'down')}
                        className="btn btn-icon focus-ring"
                        aria-label={`Move ${provider.name} down`}
                        disabled={implementedProviders[implementedProviders.length - 1] === id}
                        data-testid={`provider-order-down-${id}`}
                      >
                        <ChevronDown className="w-3.5 h-3.5" aria-hidden="true" />
                      </button>
                    </div>
                  </div>

                  {/* Inline Expander - shows below the row */}
                  {isExpanded && (
                    <div 
                      className="px-3 py-3 bg-[var(--bg-subtle)] border border-t-0 border-[var(--border-strong)] rounded-b-lg space-y-3"
                      data-testid={`provider-expander-${id}`}
                    >
                      {/* Status message */}
                      {loginState?.message && (
                        <div 
                          className={`flex items-start gap-2 p-2 rounded text-[12px] ${
                            loginState.isError 
                              ? 'bg-[var(--accent-warning)]/10 text-[var(--accent-warning)]' 
                              : 'bg-[var(--bg-surface)] text-[var(--text-secondary)]'
                          }`}
                          role={loginState.isError ? 'alert' : 'status'}
                        >
                          {loginState.isError && <AlertCircle className="w-3.5 h-3.5 flex-shrink-0 mt-0.5" aria-hidden="true" />}
                          <span>{loginState.message}</span>
                        </div>
                      )}

                      {/* Copilot device code flow */}
                      {id === 'copilot' && loginState?.deviceCode && (
                        <div className="p-3 rounded-lg bg-[var(--accent-success)]/5 border border-[var(--accent-success)]/20">
                          <p className="text-[12px] text-[var(--text-tertiary)] mb-3">
                            Enter this code on GitHub to authorize:
                          </p>
                          <div className="flex items-center gap-3 mb-3">
                            <div className="flex-1 bg-[var(--bg-base)] rounded-lg p-3 text-center">
                              <span className="text-xl font-mono font-bold text-[var(--text-primary)] tracking-[0.2em]">
                                {loginState.deviceCode.userCode}
                              </span>
                            </div>
                            <button
                              onClick={() => handleCopyCopilotCode(id)}
                              className="btn btn-ghost focus-ring"
                              aria-label="Copy code"
                            >
                              {loginState.deviceCodeCopied ? (
                                <Check className="w-4 h-4 text-[var(--accent-success)]" aria-hidden="true" />
                              ) : (
                                <Copy className="w-4 h-4" aria-hidden="true" />
                              )}
                            </button>
                          </div>
                          <div className="flex gap-2">
                            <button
                              onClick={() => handleOpenCopilotVerification(id)}
                              className="btn btn-primary focus-ring flex-1 text-[12px]"
                            >
                              <ExternalLink className="w-3.5 h-3.5" aria-hidden="true" />
                              <span>Open GitHub</span>
                            </button>
                            <button
                              onClick={() => handleCopilotContinue(id)}
                              disabled={loginState.isPolling}
                              className="btn btn-ghost focus-ring flex-1 text-[12px]"
                            >
                              {loginState.isPolling ? (
                                <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                              ) : (
                                <Check className="w-3.5 h-3.5" aria-hidden="true" />
                              )}
                              <span>{loginState.isPolling ? 'Waiting…' : "I've Authorized"}</span>
                            </button>
                          </div>
                          <button
                            onClick={() => handleCancelLogin(id)}
                            className="w-full mt-2 btn btn-ghost focus-ring text-[11px]"
                          >
                            Cancel
                          </button>
                        </div>
                      )}

                      {/* Cookie-based providers: source selector and manual input */}
                      {usesCookies && cookieSource && !loginState?.deviceCode && (
                        <div className="space-y-2">
                          <div className="flex flex-wrap items-center gap-2">
                            <label
                              htmlFor={`cookie-source-${id}`}
                              className="text-[11px] text-[var(--text-quaternary)]"
                            >
                              Cookies
                            </label>
                            <select
                              id={`cookie-source-${id}`}
                              value={cookieSource}
                              onChange={(event) => handleCookieSourceChange(id, event.target.value as CookieSource)}
                              disabled={debugDisableKeychainAccess}
                              className="bg-[var(--bg-base)] text-[11px] text-[var(--text-secondary)] border border-[var(--border-default)] rounded-md px-2 py-1 focus:outline-none focus:border-[var(--accent-primary)]"
                            >
                              {COOKIE_SOURCES.map((source) => (
                                <option key={source} value={source}>
                                  {COOKIE_SOURCE_LABELS[source]}
                                </option>
                              ))}
                            </select>
                            <button
                              type="button"
                              onClick={() => handleImportBrowserCookies(id)}
                              disabled={debugDisableKeychainAccess || cookieSource === 'manual' || loginState?.isLoggingIn}
                              className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors disabled:opacity-50"
                            >
                              Import now
                            </button>
                            <button
                              type="button"
                              onClick={() => updateProviderLoginState(id, { showCookieInput: !loginState?.showCookieInput })}
                              className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
                            >
                              {loginState?.showCookieInput ? 'Hide manual' : 'Paste cookies'}
                            </button>
                          </div>
                          {(debugDisableKeychainAccess || showKeychainGuidance) && (
                            <div className="space-y-1">
                              {debugDisableKeychainAccess && (
                                <p className="text-[11px] text-[var(--text-quaternary)]">
                                  Keychain access is disabled. Paste cookies manually.
                                </p>
                              )}
                              {showKeychainGuidance && (
                                <p className="text-[11px] text-[var(--text-quaternary)]">
                                  On macOS, Chromium browsers prompt for keychain access. Choose "Always Allow" to add Incubar to the allow-list.
                                </p>
                              )}
                            </div>
                          )}

                          {/* Manual cookie input */}
                          {loginState?.showCookieInput && (
                            <div className="p-2 rounded bg-[var(--accent-warning)]/5 border border-[var(--accent-warning)]/20">
                              <p className="text-[11px] text-[var(--text-tertiary)] mb-2">
                                Manual fallback: Copy Cookie header from DevTools Network tab
                              </p>
                              <div className="flex gap-2">
                                <input
                                  type="text"
                                  value={manualCookieInputs[id] ?? ''}
                                  onChange={(event) => setManualCookieInputs((state) => ({
                                    ...state,
                                    [id]: event.target.value,
                                  }))}
                                  placeholder="Paste Cookie header…"
                                  aria-label={`Cookie header value for ${provider.name}`}
                                  className="flex-1 px-2 py-1.5 text-[12px] bg-[var(--bg-base)] rounded border border-[var(--border-default)] text-[var(--text-primary)] placeholder:text-[var(--text-quaternary)] focus:outline-none focus:border-[var(--accent-primary)]"
                                  autoComplete="off"
                                  spellCheck={false}
                                />
                                <button
                                  onClick={() => handleSubmitCookies(id)}
                                  disabled={loginState?.isLoggingIn}
                                  className="btn btn-sm btn-primary focus-ring"
                                  aria-label={`Submit cookies for ${provider.name}`}
                                >
                                  {loginState?.isLoggingIn ? (
                                    <Loader2 className="w-3 h-3 animate-spin" aria-hidden="true" />
                                  ) : (
                                    <ClipboardPaste className="w-3 h-3" aria-hidden="true" />
                                  )}
                                </button>
                              </div>
                            </div>
                          )}
                        </div>
                      )}

                      {/* Cancel/Close button for non-device-code flows */}
                      {!loginState?.deviceCode && (
                        <button
                          onClick={() => {
                            if (loginState?.isLoggingIn) {
                              handleCancelLogin(id);
                            } else {
                              clearProviderLoginState(id);
                            }
                          }}
                          className="text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors"
                        >
                          {loginState?.isLoggingIn ? 'Cancel' : 'Close'}
                        </button>
                      )}
                    </div>
                  )}
                </div>
              );
            })}
          </div>
          
          {/* Upcoming Providers */}
          {upcomingProviders.length > 0 && (
            <div className="mt-5">
              <h3 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
                Coming Soon
              </h3>
              <div className="space-y-1">
                {upcomingProviders.map((id) => {
                  const provider = PROVIDERS[id];
                  return (
                    <div
                      key={id}
                      className="flex items-center gap-3 px-3 py-2 rounded-lg opacity-40"
                    >
                      <ProviderIcon providerId={id} className="w-4 h-4" aria-hidden="true" />
                      <div className="flex-1">
                        <span className="text-[13px] text-[var(--text-tertiary)]">{provider.name}</span>
                      </div>
                      <span className="text-[11px] text-[var(--text-quaternary)]">
                        {getAuthMethodLabel(provider.authMethod)}
                      </span>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
          
          </section>
        )}

        {(activeTab === 'providers' || !showTabs) && <div className={dividerClass} />}

        {/* Refresh Interval */}
        {(activeTab === 'preferences' || !showTabs) && (
        <section className={sectionPaddingClass}>
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Refresh Interval
          </h2>
          <div className="flex gap-1.5">
            {refreshIntervals.map(({ label, value }) => (
              <button
                key={value}
                onClick={() => handleSetRefreshInterval(value)}
                className={`flex-1 px-3 py-2 text-[13px] font-medium rounded-md transition-colors focus-ring ${
                  refreshIntervalSeconds === value
                    ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                    : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                }`}
              >
                {label}
              </button>
            ))}
          </div>
        </section>
        )}

        {(activeTab === 'preferences' || !showTabs) && <div className={dividerClass} />}

        {/* Display Options */}
        {(activeTab === 'preferences' || !showTabs) && (
        <section className={sectionPaddingClass}>
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Display
          </h2>
          <div className="space-y-3">
            <div>
              <span className="text-[11px] text-[var(--text-quaternary)] uppercase tracking-wider">
                Menu Bar Display
              </span>
              <div className="flex gap-1.5 mt-2">
                <button
                  type="button"
                  onClick={() => handleSetMenuBarDisplayMode('session')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    menuBarDisplayMode === 'session'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="menu-bar-display-session"
                >
                  Session
                </button>
                <button
                  type="button"
                  onClick={() => handleSetMenuBarDisplayMode('weekly')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    menuBarDisplayMode === 'weekly'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="menu-bar-display-weekly"
                >
                  Weekly
                </button>
                <button
                  type="button"
                  onClick={() => handleSetMenuBarDisplayMode('pace')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    menuBarDisplayMode === 'pace'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="menu-bar-display-pace"
                >
                  Pace
                </button>
                <button
                  type="button"
                  onClick={() => handleSetMenuBarDisplayMode('highest')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    menuBarDisplayMode === 'highest'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="menu-bar-display-highest"
                >
                  Highest
                </button>
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Choose session usage, weekly usage, weekly pace, or highest usage.
              </p>
              <div className="mt-3" data-testid="menu-bar-text-toggle">
                <ToggleOption
                  label="Menu bar shows text"
                  enabled={menuBarDisplayTextEnabled}
                  onChange={handleSetMenuBarDisplayTextEnabled}
                />
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Show percent, pace, or both next to the menu bar icon.
              </p>
              {menuBarDisplayTextEnabled && (
                <div className="mt-3">
                  <span className="text-[11px] text-[var(--text-quaternary)] uppercase tracking-wider">
                    Menu Bar Text
                  </span>
                  <div className="flex gap-1.5 mt-2">
                    <button
                      type="button"
                      onClick={() => handleSetMenuBarDisplayTextMode('percent')}
                      className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                        menuBarDisplayTextMode === 'percent'
                          ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                      }`}
                      data-testid="menu-bar-text-percent"
                    >
                      Percent
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSetMenuBarDisplayTextMode('pace')}
                      className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                        menuBarDisplayTextMode === 'pace'
                          ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                      }`}
                      data-testid="menu-bar-text-pace"
                    >
                      Pace
                    </button>
                    <button
                      type="button"
                      onClick={() => handleSetMenuBarDisplayTextMode('both')}
                      className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                        menuBarDisplayTextMode === 'both'
                          ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                          : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                      }`}
                      data-testid="menu-bar-text-both"
                    >
                      Both
                    </button>
                  </div>
                  <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                    Percent uses the selected display window. Pace uses weekly usage for Codex or Claude.
                  </p>
                </div>
              )}
            </div>
            <div className="divider" />
            <div>
              <span className="text-[11px] text-[var(--text-quaternary)] uppercase tracking-wider">
                Usage Bar Display
              </span>
              <div className="flex gap-1.5 mt-2">
                <button
                  type="button"
                  onClick={() => handleSetUsageBarDisplayMode('remaining')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    usageBarDisplayMode === 'remaining'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="usage-bar-display-remaining"
                >
                  Remaining
                </button>
                <button
                  type="button"
                  onClick={() => handleSetUsageBarDisplayMode('used')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    usageBarDisplayMode === 'used'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="usage-bar-display-used"
                >
                  Used
                </button>
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Show usage bars as remaining capacity or used capacity.
              </p>
            </div>
            <div className="divider" />
            <div>
              <span className="text-[11px] text-[var(--text-quaternary)] uppercase tracking-wider">
                Reset Time Display
              </span>
              <div className="flex gap-1.5 mt-2">
                <button
                  type="button"
                  onClick={() => handleSetResetTimeDisplayMode('relative')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    resetTimeDisplayMode === 'relative'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="reset-time-display-relative"
                >
                  Relative
                </button>
                <button
                  type="button"
                  onClick={() => handleSetResetTimeDisplayMode('absolute')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    resetTimeDisplayMode === 'absolute'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="reset-time-display-absolute"
                >
                  Absolute
                </button>
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Show reset times as relative text or full date/time.
              </p>
            </div>
            <div className="divider" />
            <div>
              <span className="text-[11px] text-[var(--text-quaternary)] uppercase tracking-wider">
                Provider Switcher
              </span>
              <div className="flex gap-1.5 mt-2">
                <button
                  type="button"
                  onClick={() => handleSetDisplayMode('separate')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    displayMode === 'separate'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="display-mode-separate"
                >
                  Tabs
                </button>
                <button
                  type="button"
                  onClick={() => handleSetDisplayMode('merged')}
                  className={`flex-1 px-3 py-2 text-[12px] font-medium rounded-md transition-colors focus-ring ${
                    displayMode === 'merged'
                      ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                      : 'text-[var(--text-tertiary)] hover:bg-[var(--bg-surface)] hover:text-[var(--text-secondary)]'
                  }`}
                  data-testid="display-mode-merged"
                >
                  Icons
                </button>
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Choose tab labels or icon-only switcher.
              </p>
              {displayMode === 'merged' && (
                <div className="mt-2">
                  <ToggleOption
                    label="Switcher shows icons"
                    enabled={switcherShowsIcons}
                    onChange={handleSetSwitcherShowsIcons}
                  />
                </div>
              )}
            </div>
            <div className="divider" />
            <div>
              <span className="text-[11px] text-[var(--text-quaternary)] uppercase tracking-wider">
                Token Accounts
              </span>
              <div className="mt-2">
                <ToggleOption
                  label="Show all token accounts"
                  enabled={showAllTokenAccountsInMenu}
                  onChange={handleSetShowAllTokenAccountsInMenu}
                />
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Stack token accounts in the menu instead of showing a switcher bar.
              </p>
            </div>
            <div className="divider" />
            <div className="space-y-1">
              <ToggleOption label="Show Credits" enabled={showCredits} onChange={handleSetShowCredits} />
              <ToggleOption label="Show Cost" enabled={showCost} onChange={handleSetShowCost} />
              <ToggleOption label="Show Extra Usage" enabled={showExtraUsage} onChange={handleSetShowExtraUsage} />
              <div className="mt-3">
                <span className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider">
                  Privacy
                </span>
                <p className="mt-2 text-[11px] text-[var(--text-quaternary)]">
                  Incubar keeps all usage data on-device. Disable history to avoid storing usage snapshots.
                </p>
                <ToggleOption
                  label="Hide Personal Info"
                  enabled={hidePersonalInfo}
                  onChange={setHidePersonalInfo}
                />
                <ToggleOption
                  label="Redact Personal Info in Logs"
                  enabled={redactPersonalInfo}
                  onChange={setRedactPersonalInfo}
                />
                <p className="text-[11px] text-[var(--text-quaternary)]">
                  Redacts emails, names, and raw usage responses from debug logs.
                </p>
                <div className="mt-2 space-y-1" data-testid="privacy-preferences">
                  <ToggleOption
                    label="Store usage history"
                    enabled={storeUsageHistory}
                    onChange={handleSetStoreUsageHistory}
                  />
                  <ToggleOption
                    label="Poll provider status"
                    enabled={pollProviderStatus}
                    onChange={handleSetPollProviderStatus}
                  />
                </div>
                <p className="mt-2 text-[11px] text-[var(--text-quaternary)]">
                  Status polling checks provider health pages and never sends your usage data.
                </p>
              </div>
              <ToggleOption label="Notifications" enabled={showNotifications} onChange={handleSetShowNotifications} />
              {showNotifications && (
                <div className="space-y-1 pl-2" data-testid="notification-preferences">
                  <ToggleOption
                    label="Session usage alerts"
                    enabled={notifySessionUsage}
                    onChange={handleSetNotifySessionUsage}
                  />
                  <ToggleOption
                    label="Credits low alerts"
                    enabled={notifyCreditsLow}
                    onChange={handleSetNotifyCreditsLow}
                  />
                  <ToggleOption
                    label="Refresh failure alerts"
                    enabled={notifyRefreshFailure}
                    onChange={handleSetNotifyRefreshFailure}
                  />
                  <ToggleOption
                    label="Stale usage alerts"
                    enabled={notifyStaleUsage}
                    onChange={handleSetNotifyStaleUsage}
                  />
                  <button
                    type="button"
                    onClick={() => void invoke('send_test_notification')}
                    className="mt-1 w-full text-left text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] transition-colors"
                    data-testid="notification-test-button"
                  >
                    Send test notification
                  </button>
                </div>
              )}
              <ToggleOption label="Launch at Login" enabled={launchAtLogin} onChange={handleSetLaunchAtLogin} />
            </div>
          </div>
        </section>
        )}

        {(activeTab === 'updates' || !showTabs) && <div className={dividerClass} />}

        {/* Updates */}
        {(activeTab === 'updates' || !showTabs) && (
        <section className={sectionPaddingClass}>
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Updates
          </h2>
          <div className="space-y-2" data-testid="update-settings">
            <ToggleOption
              label="Check for updates automatically"
              enabled={autoUpdateEnabled}
              onChange={handleSetAutoUpdateEnabled}
            />
            <div className="flex items-center justify-between px-3 py-2.5 rounded-md bg-[var(--bg-surface)] border border-[var(--border-subtle)]">
              <div>
                <div className="text-[13px] text-[var(--text-secondary)]">Update Channel</div>
                <div className="text-[11px] text-[var(--text-quaternary)]">
                  {updateChannel === 'beta'
                    ? 'Receive stable releases plus beta previews.'
                    : 'Receive only stable, production-ready releases.'}
                </div>
              </div>
              <select
                value={updateChannel}
                onChange={(event) => handleSetUpdateChannel(event.target.value as UpdateChannel)}
                className="bg-[var(--bg-base)] text-[11px] text-[var(--text-secondary)] border border-[var(--border-default)] rounded-md px-2 py-1 focus:outline-none focus:border-[var(--accent-primary)]"
              >
                <option value="stable">Stable</option>
                <option value="beta">Beta</option>
              </select>
            </div>
          </div>
        </section>
        )}

        {(activeTab === 'debug' || !showTabs) && <div className={dividerClass} />}

        {/* Debug */}
        {(activeTab === 'debug' || !showTabs) && (
        <section className={sectionPaddingClass} data-testid="debug-settings">
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Debug
          </h2>
          <div className="space-y-1">
            <ToggleOption
              label="Show Debug Settings"
              enabled={debugMenuEnabled}
              onChange={handleSetDebugMenuEnabled}
            />
            {debugMenuEnabled && (
              <div className="space-y-1">
                <ToggleOption
                  label="File Logging"
                  enabled={debugFileLogging}
                  onChange={handleSetDebugFileLogging}
                />
                <ToggleOption
                  label="Keep CLI Sessions Alive"
                  enabled={debugKeepCliSessionsAlive}
                  onChange={handleSetDebugKeepCliSessionsAlive}
                />
                <ToggleOption
                  label="Random Blink"
                  enabled={debugRandomBlink}
                  onChange={handleSetDebugRandomBlink}
                />
                <div className="mt-2 space-y-2 rounded-md bg-[var(--bg-surface)] border border-[var(--border-subtle)] px-3 py-2">
                  <div className="text-[13px] text-[var(--text-secondary)]">Support Bundle</div>
                  <p className="text-[11px] text-[var(--text-quaternary)]">
                    Export usage snapshots, settings, and status info for support.
                  </p>
                  <button
                    type="button"
                    onClick={handleExportSupportBundle}
                    disabled={supportExporting}
                    className="btn btn-ghost focus-ring w-full justify-center text-[12px]"
                  >
                    {supportExporting ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <Download className="w-3.5 h-3.5" aria-hidden="true" />
                    )}
                    <span>{supportExporting ? 'Exporting…' : 'Export Support Bundle'}</span>
                  </button>
                  {supportExportPath && (
                    <p className="text-[11px] text-[var(--text-quaternary)] break-all">
                      Saved to {supportExportPath}
                    </p>
                  )}
                </div>
              </div>
            )}
          </div>
        </section>
        )}

        {(activeTab === 'advanced' || !showTabs) && <div className={dividerClass} />}

        {(activeTab === 'advanced' || !showTabs) && (
        <section className={sectionPaddingClass}>
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Advanced
          </h2>
          <div className="space-y-2">
            <div className="flex items-center justify-between px-3 py-2.5 rounded-md bg-[var(--bg-surface)] border border-[var(--border-subtle)]">
              <div>
                <div className="text-[13px] text-[var(--text-secondary)]">Global shortcut</div>
                <div className="text-[11px] text-[var(--text-quaternary)]">
                  Trigger the menu bar popup from anywhere.
                </div>
              </div>
              <span className="text-[11px] font-semibold text-[var(--text-primary)]">Cmd/Ctrl+R</span>
            </div>
            <div className="flex items-center justify-between px-3 py-2.5 rounded-md bg-[var(--bg-surface)] border border-[var(--border-subtle)]">
              <div>
                <div className="text-[13px] text-[var(--text-secondary)]">CLI status</div>
                <div className="text-[11px] text-[var(--text-quaternary)]">
                  Bundled CLI supports status + cost commands.
                </div>
              </div>
              <span className="text-[11px] text-[var(--text-quaternary)]">codexbar</span>
            </div>
            <ToggleOption
              label="Disable Keychain Access"
              enabled={debugDisableKeychainAccess}
              onChange={handleSetDebugDisableKeychainAccess}
            />
            <p className="text-[11px] text-[var(--text-quaternary)]">
              Disable Keychain access to require manual cookie paste for browser providers.
            </p>
          </div>
        </section>
        )}

        {(activeTab === 'preferences' || !showTabs) && <div className={dividerClass} />}

        {/* Reset */}
        {(activeTab === 'preferences' || !showTabs) && (
        <section className={sectionPaddingClass}>
          <button
            onClick={handleResetToDefaults}
            className="w-full btn btn-ghost focus-ring justify-center"
          >
            <RotateCcw className="w-3.5 h-3.5" aria-hidden="true" />
            <span>Reset to Defaults</span>
          </button>
        </section>
        )}

        {/* About */}
        {(activeTab === 'about' || !showTabs) && (
        <footer className={`${footerPaddingClass} text-center`}>
          <div className="text-[11px] text-[var(--text-quaternary)]">
            IncuBar v{import.meta.env.PACKAGE_VERSION}
          </div>
          {installOrigin && (
            <div className="mt-1 text-[10px] text-[var(--text-quaternary)]">
              Installed via {installOrigin}
            </div>
          )}
        </footer>
        )}
      </div>
    </div>
  );
}

interface ToggleOptionProps {
  label: string;
  enabled: boolean;
  onChange: (enabled: boolean) => void;
}

function ToggleOption({ label, enabled, onChange }: ToggleOptionProps) {
  return (
    <button
      onClick={() => onChange(!enabled)}
      className="w-full flex items-center justify-between px-3 py-2.5 rounded-md hover:bg-[var(--bg-surface)] transition-colors focus-ring"
      role="switch"
      aria-checked={enabled}
    >
      <span className="text-[13px] text-[var(--text-secondary)]">{label}</span>
      <div 
        className="toggle" 
        data-state={enabled ? 'checked' : 'unchecked'}
        aria-hidden="true"
      >
        <span className="toggle-thumb" />
      </div>
    </button>
  );
}
