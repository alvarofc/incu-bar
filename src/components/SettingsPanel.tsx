import { useCallback, useEffect, useMemo, useState, type DragEvent } from 'react';
import { ArrowLeft, Check, RotateCcw, LogIn, Loader2, AlertCircle, ClipboardPaste, Cookie, Copy, ExternalLink, ChevronUp, ChevronDown, GripVertical } from 'lucide-react';
import type { MenuBarDisplayMode } from '../lib/types';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
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

interface SettingsPanelProps {
  onBack: () => void;
}

export function SettingsPanel({ onBack }: SettingsPanelProps) {
  const enabledProviders = useSettingsStore((s) => s.enabledProviders);
  const providerOrder = useSettingsStore((s) => s.providerOrder);
  const setProviderOrder = useSettingsStore((s) => s.setProviderOrder);
  const refreshIntervalSeconds = useSettingsStore((s) => s.refreshIntervalSeconds);
  const displayMode = useSettingsStore((s) => s.displayMode);
  const menuBarDisplayMode = useSettingsStore((s) => s.menuBarDisplayMode);
  const showCredits = useSettingsStore((s) => s.showCredits);
  const showCost = useSettingsStore((s) => s.showCost);
  const showNotifications = useSettingsStore((s) => s.showNotifications);
  const launchAtLogin = useSettingsStore((s) => s.launchAtLogin);

  const [authStatus, setAuthStatus] = useState<Record<string, AuthStatus>>({});
  const [loggingIn, setLoggingIn] = useState<string | null>(null);
  const [loginMessage, setLoginMessage] = useState<string | null>(null);
  const [cursorLoginOpen, setCursorLoginOpen] = useState(false);
  const [manualCookieInputs, setManualCookieInputs] = useState<Partial<Record<ProviderId, string>>>({});
  const [manualCookiePanels, setManualCookiePanels] = useState<Partial<Record<ProviderId, boolean>>>({});
  const cookieSources = useSettingsStore((s) => s.cookieSources);
  const [draggingProviderId, setDraggingProviderId] = useState<ProviderId | null>(null);
  const [dragOverProviderId, setDragOverProviderId] = useState<ProviderId | null>(null);

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

  const [selectedProviderId, setSelectedProviderId] = useState<ProviderId>(
    () => implementedProviders[0] ?? 'claude'
  );
  
  const [copilotDeviceCode, setCopilotDeviceCode] = useState<CopilotDeviceCode | null>(null);
  const [copilotCodeCopied, setCopilotCodeCopied] = useState(false);
  const [copilotPolling, setCopilotPolling] = useState(false);

  useEffect(() => {
    const checkAuth = async () => {
      try {
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
      } catch (e) {
        console.error('Failed to check auth status:', e);
      }
    };
    checkAuth();
  }, []);

  useEffect(() => {
    if (orderedProviderIds.join('|') !== providerOrder.join('|')) {
      setProviderOrder(orderedProviderIds);
    }
  }, [orderedProviderIds, providerOrder, setProviderOrder]);

  useEffect(() => {
    if (!implementedProviders.includes(selectedProviderId)) {
      setSelectedProviderId(implementedProviders[0] ?? 'claude');
    }
  }, [implementedProviders, selectedProviderId]);

  useEffect(() => {
    const unlistenLogin = listen('cursor-login-detected', async () => {
      setLoginMessage('Login detected! Extracting cookies…');
      try {
        const result = await invoke<LoginResult>('extract_cursor_cookies');
        if (result.success) {
          setLoginMessage(result.message);
          setCursorLoginOpen(false);
          setManualCookiePanels((state) => ({ ...state, cursor: false }));
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          setAuthStatus(status);
          useSettingsStore.getState().enableProvider('cursor');
          useUsageStore.getState().refreshProvider('cursor');
        }
      } catch (e) {
        setLoginMessage(`Cookie extraction error: ${e}`);
        setManualCookiePanels((state) => ({ ...state, cursor: true }));
      }
    });

    const unlistenCompleted = listen('login-completed', async (event: { payload: { providerId: ProviderId; success: boolean; message: string } }) => {
      const { providerId, success, message } = event.payload;
      if (success) {
        setLoginMessage(message);
        setCursorLoginOpen(false);
        setManualCookiePanels((state) => ({ ...state, [providerId]: false }));
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider(providerId as ProviderId);
        useUsageStore.getState().refreshProvider(providerId as ProviderId);
      }
    });

    return () => {
      unlistenLogin.then(fn => fn());
      unlistenCompleted.then(fn => fn());
    };
  }, []);

  const handleToggleProvider = useCallback((id: ProviderId) => {
    useSettingsStore.getState().toggleProvider(id);
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

  const handleSetShowNotifications = useCallback((show: boolean) => {
    useSettingsStore.getState().setShowNotifications(show);
  }, []);

  const handleSetDisplayMode = useCallback((mode: 'merged' | 'separate') => {
    useSettingsStore.getState().setDisplayMode(mode);
  }, []);

  const handleSetMenuBarDisplayMode = useCallback((mode: MenuBarDisplayMode) => {
    useSettingsStore.getState().setMenuBarDisplayMode(mode);
  }, []);

  const handleSetLaunchAtLogin = useCallback((launch: boolean) => {
    useSettingsStore.getState().setLaunchAtLogin(launch);
  }, []);

  const handleResetToDefaults = useCallback(() => {
    useSettingsStore.getState().resetToDefaults();
  }, []);

  const handleLogin = useCallback(async (providerId: ProviderId) => {
    setLoggingIn(providerId);
    setLoginMessage(null);
    
    try {
      if (providerId === 'cursor') {
        const cookieSource = useSettingsStore.getState().getCookieSource('cursor');
        setLoginMessage(`Importing cookies from ${COOKIE_SOURCE_LABELS[cookieSource]}…`);
        const importResult = await invoke<LoginResult>('import_cursor_browser_cookies_from_source', {
          source: { source: cookieSource },
        });

        if (importResult.success) {
          setLoginMessage(importResult.message);
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          setAuthStatus(status);
          useSettingsStore.getState().enableProvider('cursor');
          useUsageStore.getState().refreshProvider('cursor');
          setLoggingIn(null);
          return;
        }

        setLoginMessage('Could not import from browser. Opening login window…');
      }

      if (providerId === 'factory' || providerId === 'augment' || providerId === 'kimi' || providerId === 'minimax' || providerId === 'amp' || providerId === 'opencode') {
        const cookieSource = useSettingsStore.getState().getCookieSource(providerId);
        setLoginMessage(`Importing cookies from ${COOKIE_SOURCE_LABELS[cookieSource]}…`);
          const importCommand = providerId === 'factory'
            ? 'import_factory_browser_cookies_from_source'
            : providerId === 'augment'
              ? 'import_augment_browser_cookies_from_source'
              : providerId === 'minimax'
                ? 'import_minimax_browser_cookies_from_source'
                : providerId === 'amp'
                  ? 'import_amp_browser_cookies_from_source'
                  : providerId === 'opencode'
                    ? 'import_opencode_browser_cookies_from_source'
                    : 'import_kimi_browser_cookies_from_source';
        const importResult = await invoke<LoginResult>(importCommand, {
          source: { source: cookieSource },
        });

        if (importResult.success) {
          setLoginMessage(importResult.message);
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          setAuthStatus(status);
          useSettingsStore.getState().enableProvider(providerId);
          useUsageStore.getState().refreshProvider(providerId);
          setLoggingIn(null);
          return;
        }

        setLoginMessage('Could not import from browser. You can paste cookies manually below.');
        setManualCookiePanels((state) => ({ ...state, [providerId]: true }));
        setLoggingIn(null);
        return;
      }

      if (providerId === 'kiro') {
        setLoginMessage('Kiro uses the CLI. Run `kiro-cli login` in Terminal, then refresh.');
        setLoggingIn(null);
        return;
      }
      
      if (providerId === 'copilot') {
        setLoginMessage('Requesting device code from GitHub…');
        const deviceCode = await invoke<CopilotDeviceCode>('copilot_request_device_code');
        setCopilotDeviceCode(deviceCode);
        setCopilotCodeCopied(false);
        setLoginMessage(null);
        setLoggingIn(null);
        return;
      }
      
      const result = await invoke<LoginResult>('start_login', { providerId });

      if (result.success) {
        setLoginMessage(`${PROVIDERS[providerId].name} connected!`);
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider(providerId);
        useUsageStore.getState().refreshProvider(providerId);
      } else {
        setLoginMessage(result.message);
      }
    } catch (e) {
      setLoginMessage(`Login failed: ${e}`);
    } finally {
      setLoggingIn(null);
    }
  }, []);

  const handleSubmitCookies = useCallback(async (providerId: ProviderId) => {
    const cookieHeader = manualCookieInputs[providerId]?.trim() ?? '';
    if (!cookieHeader) {
      setLoginMessage('Please paste your cookies first');
      return;
    }

    setLoggingIn(providerId);
    try {
      const storeCommand = providerId === 'factory'
        ? 'store_factory_cookies'
        : providerId === 'augment'
          ? 'store_augment_cookies'
          : providerId === 'kimi'
            ? 'store_kimi_cookies'
            : providerId === 'minimax'
              ? 'store_minimax_cookies'
              : providerId === 'amp'
                ? 'store_amp_cookies'
                : providerId === 'opencode'
                  ? 'store_opencode_cookies'
                  : 'store_cursor_cookies';
      const result = await invoke<LoginResult>(storeCommand, { cookieHeader });
      if (result.success) {
        setLoginMessage(`${PROVIDERS[providerId].name} cookies saved!`);
        setManualCookieInputs((state) => ({ ...state, [providerId]: '' }));
        setManualCookiePanels((state) => ({ ...state, [providerId]: false }));
        if (providerId === 'cursor') {
          setCursorLoginOpen(false);
          await invoke('close_cursor_login');
        }
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider(providerId);
        useUsageStore.getState().refreshProvider(providerId);
      } else {
        setLoginMessage(result.message);
      }
    } catch (e) {
      setLoginMessage(`Failed to save cookies: ${e}`);
    } finally {
      setLoggingIn(null);
    }
  }, [manualCookieInputs]);

  const handleExtractCookies = useCallback(async () => {
    setLoggingIn('cursor');
    setLoginMessage('Extracting cookies…');

    try {
      const result = await invoke<LoginResult>('extract_cursor_cookies');
      if (result.success) {
        setLoginMessage(result.message);
        setCursorLoginOpen(false);
        setManualCookiePanels((state) => ({ ...state, cursor: false }));
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useUsageStore.getState().refreshProvider('cursor');
      } else {
        setLoginMessage(result.message);
        setManualCookiePanels((state) => ({ ...state, cursor: true }));
      }
    } catch (e) {
      setLoginMessage(`Failed to extract cookies: ${e}`);
      setManualCookiePanels((state) => ({ ...state, cursor: true }));
    } finally {
      setLoggingIn(null);
    }
  }, []);

  const handleImportBrowserCookies = useCallback(async (providerId: ProviderId) => {
    setLoggingIn(providerId);
    const cookieSource = useSettingsStore.getState().getCookieSource(providerId);
    setLoginMessage(`Importing cookies from ${COOKIE_SOURCE_LABELS[cookieSource]}…`);

    const importCommand = providerId === 'factory'
      ? 'import_factory_browser_cookies_from_source'
      : providerId === 'augment'
        ? 'import_augment_browser_cookies_from_source'
        : providerId === 'kimi'
          ? 'import_kimi_browser_cookies_from_source'
          : providerId === 'minimax'
            ? 'import_minimax_browser_cookies_from_source'
            : providerId === 'amp'
              ? 'import_amp_browser_cookies_from_source'
              : providerId === 'opencode'
                ? 'import_opencode_browser_cookies_from_source'
                : 'import_cursor_browser_cookies_from_source';

    try {
      const result = await invoke<LoginResult>(importCommand, {
        source: { source: cookieSource },
      });
      if (result.success) {
        setLoginMessage(result.message);
        setCursorLoginOpen(false);
        setManualCookiePanels((state) => ({ ...state, [providerId]: false }));
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider(providerId);
        useUsageStore.getState().refreshProvider(providerId);
      } else {
        setLoginMessage(result.message);
        setManualCookiePanels((state) => ({ ...state, [providerId]: true }));
      }
    } catch (e) {
      setLoginMessage(`Failed to import cookies: ${e}`);
      setManualCookiePanels((state) => ({ ...state, [providerId]: true }));
    } finally {
      setLoggingIn(null);
    }
  }, []);

  const handleCookieSourceChange = useCallback((providerId: ProviderId, source: CookieSource) => {
    useSettingsStore.getState().setCookieSource(providerId, source);
  }, []);

  const handleCopyCopilotCode = useCallback(async () => {
    if (!copilotDeviceCode) return;
    try {
      await navigator.clipboard.writeText(copilotDeviceCode.userCode);
      setCopilotCodeCopied(true);
      setTimeout(() => setCopilotCodeCopied(false), 2000);
    } catch (e) {
      console.error('Failed to copy:', e);
    }
  }, [copilotDeviceCode]);

  const handleOpenCopilotVerification = useCallback(() => {
    if (!copilotDeviceCode) return;
    window.open(copilotDeviceCode.verificationUri, '_blank');
  }, [copilotDeviceCode]);

  const handleCopilotContinue = useCallback(async () => {
    if (!copilotDeviceCode) return;
    
    setCopilotPolling(true);
    setLoginMessage('Waiting for authorization…');
    
    try {
      const result = await invoke<LoginResult>('copilot_poll_for_token', {
        deviceCode: copilotDeviceCode.deviceCode,
      });
      
      if (result.success) {
        setLoginMessage(result.message);
        setCopilotDeviceCode(null);
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider('copilot');
        useUsageStore.getState().refreshProvider('copilot');
      } else {
        setLoginMessage(result.message);
      }
    } catch (e) {
      setLoginMessage(`Copilot login failed: ${e}`);
    } finally {
      setCopilotPolling(false);
    }
  }, [copilotDeviceCode]);

  const handleCopilotCancel = useCallback(() => {
    setCopilotDeviceCode(null);
    setLoginMessage(null);
  }, []);

  const refreshIntervals = [
    { label: '1m', value: 60 },
    { label: '5m', value: 300 },
    { label: '15m', value: 900 },
    { label: '30m', value: 1800 },
  ];

  const selectedProvider = PROVIDERS[selectedProviderId];
  const selectedStatus = authStatus[selectedProviderId];
  const selectedIsAuthenticated = selectedStatus?.authenticated === true;
  const selectedIsEnabled = enabledProviders.includes(selectedProviderId);
  const selectedUsesCookies = selectedProvider.authMethod === 'cookies';
  const selectedCookieSource = selectedUsesCookies
    ? cookieSources[selectedProviderId] ?? useSettingsStore.getState().getCookieSource(selectedProviderId)
    : null;

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
    <div className="popup-container">
      {/* Header */}
      <header className="flex items-center gap-3 px-4 py-3 border-b border-[var(--border-subtle)]">
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
        {/* Status Message */}
        {loginMessage && (
          <div className="mx-4 mt-4 p-3 rounded-lg bg-[var(--bg-surface)] border border-[var(--border-subtle)]" role="status" aria-live="polite">
            <p className="text-[13px] text-[var(--text-secondary)]">{loginMessage}</p>
          </div>
        )}

        {/* Copilot Device Code Dialog */}
        {copilotDeviceCode && (
          <div className="mx-4 mt-4 p-4 rounded-lg bg-[var(--accent-success)]/5 border border-[var(--accent-success)]/20">
            <h3 className="text-[14px] font-semibold text-[var(--accent-success)] mb-2">
              GitHub Copilot
            </h3>
            <p className="text-[13px] text-[var(--text-tertiary)] mb-4">
              Enter this code on GitHub to authorize:
            </p>
            
            <div className="flex items-center gap-3 mb-4">
              <div className="flex-1 bg-[var(--bg-base)] rounded-lg p-4 text-center">
                <span className="text-2xl font-mono font-bold text-[var(--text-primary)] tracking-[0.2em]">
                  {copilotDeviceCode.userCode}
                </span>
              </div>
              <button
                onClick={handleCopyCopilotCode}
                className="btn btn-ghost focus-ring"
                aria-label="Copy code"
              >
                {copilotCodeCopied ? (
                  <Check className="w-4 h-4 text-[var(--accent-success)]" aria-hidden="true" />
                ) : (
                  <Copy className="w-4 h-4" aria-hidden="true" />
                )}
              </button>
            </div>
            
            <div className="flex gap-2">
              <button
                onClick={handleOpenCopilotVerification}
                className="btn btn-primary focus-ring flex-1"
              >
                <ExternalLink className="w-4 h-4" aria-hidden="true" />
                <span>Open GitHub</span>
              </button>
              <button
                onClick={handleCopilotContinue}
                disabled={copilotPolling}
                className="btn btn-ghost focus-ring flex-1"
              >
                {copilotPolling ? (
                  <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
                ) : (
                  <Check className="w-4 h-4" aria-hidden="true" />
                )}
                <span>{copilotPolling ? 'Waiting…' : "I've Authorized"}</span>
              </button>
            </div>
            
            <button
              onClick={handleCopilotCancel}
              className="w-full mt-3 btn btn-ghost focus-ring text-[13px]"
            >
              Cancel
            </button>
          </div>
        )}

        {/* Providers Section */}
        <section className="p-4">
          <h2 className="text-[11px] font-semibold text-[var(--text-quaternary)] uppercase tracking-wider mb-3">
            Providers
          </h2>
          <div className="space-y-1.5" data-testid="provider-settings-list">
            {implementedProviders.map((id) => {
              const provider = PROVIDERS[id];
              const status = authStatus[id];
              const isAuthenticated = status?.authenticated === true;
              const isSelected = selectedProviderId === id;
              const isEnabled = enabledProviders.includes(id);

              return (
                <div
                  key={id}
                  className={`w-full flex items-center justify-between px-3 py-2.5 rounded-lg border transition-colors ${
                    isSelected
                      ? 'bg-[var(--bg-subtle)] border-[var(--border-strong)]'
                      : 'bg-[var(--bg-surface)] border-[var(--border-subtle)] hover:bg-[var(--bg-overlay)]'
                  } ${dragOverProviderId === id ? 'ring-1 ring-[var(--accent-primary)]' : ''} ${draggingProviderId === id ? 'opacity-70' : ''}`}
                  onDragOver={(event) => handleDragOver(event, id)}
                  onDrop={(event) => handleDrop(event, id)}
                  onDragLeave={() => setDragOverProviderId(null)}
                  data-testid={`provider-order-item-${id}`}
                >
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
                  <button
                    type="button"
                    onClick={() => setSelectedProviderId(id)}
                    className="flex items-center gap-3 flex-1 min-w-0 text-left focus-ring"
                    aria-pressed={isSelected}
                  >
                    <ProviderIcon 
                      providerId={id} 
                      className={`w-5 h-5 flex-shrink-0 ${isAuthenticated ? 'opacity-100' : 'opacity-50'}`}
                      aria-hidden="true"
                    />
                    <div className="min-w-0">
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
                  </button>
                  <div className="flex items-center gap-1">
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
              );
          })}
          </div>

          {selectedProvider && (
            <div
              className="mt-4 p-4 rounded-lg bg-[var(--bg-surface)] border border-[var(--border-subtle)]"
              data-testid="provider-detail-pane"
            >
              <div className="flex items-start justify-between gap-3">
                <div className="flex items-start gap-3">
                  <ProviderIcon
                    providerId={selectedProviderId}
                    className={`w-6 h-6 ${selectedIsAuthenticated ? 'opacity-100' : 'opacity-60'}`}
                    aria-hidden="true"
                  />
                  <div>
                    <h3 className="text-[14px] font-semibold text-[var(--text-primary)]">
                      {selectedProvider.name} Settings
                    </h3>
                    <p className="text-[12px] text-[var(--text-tertiary)] mt-1">
                      {selectedIsAuthenticated
                        ? `Connected · ${getAuthMethodLabel(selectedStatus?.method ?? selectedProvider.authMethod)}`
                        : `Not connected · ${getAuthMethodLabel(selectedProvider.authMethod)}`}
                    </p>
                    {selectedStatus?.email && (
                      <p className="text-[12px] text-[var(--text-quaternary)] mt-1 truncate">
                        {selectedStatus.email}
                      </p>
                    )}
                  </div>
                </div>

                <button
                  onClick={() => handleToggleProvider(selectedProviderId)}
                  className="toggle focus-ring"
                  data-state={selectedIsEnabled ? 'checked' : 'unchecked'}
                  role="switch"
                  aria-checked={selectedIsEnabled}
                  aria-label={selectedIsEnabled ? `Hide ${selectedProvider.name}` : `Show ${selectedProvider.name}`}
                >
                  <span className="toggle-thumb" />
                </button>
              </div>

              {selectedStatus?.error && (
                <div
                  className="flex items-start gap-2.5 p-3 mt-3 rounded-lg bg-[var(--accent-warning)]/10 border border-[var(--accent-warning)]/20"
                  role="alert"
                >
                  <AlertCircle className="w-4 h-4 text-[var(--accent-warning)] flex-shrink-0 mt-0.5" aria-hidden="true" />
                  <p className="text-[12px] text-[var(--accent-warning)]/90">
                    {selectedStatus.error}
                  </p>
                </div>
              )}

              <div className="mt-4 space-y-3">
                <div className="flex flex-wrap items-center gap-2">
                  <button
                    onClick={() => handleLogin(selectedProviderId)}
                    disabled={loggingIn === selectedProviderId}
                    className="btn btn-primary focus-ring"
                  >
                    {loggingIn === selectedProviderId ? (
                      <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                    ) : (
                      <LogIn className="w-3.5 h-3.5" aria-hidden="true" />
                    )}
                    <span>{selectedIsAuthenticated ? 'Reconnect' : 'Connect'}</span>
                  </button>
                  <span className="text-[11px] text-[var(--text-quaternary)]">
                    Visible in popup tabs once connected
                  </span>
                </div>

                {selectedUsesCookies && selectedCookieSource && (
                  <div className="flex flex-wrap items-center gap-2">
                    <label
                      htmlFor={`cookie-source-${selectedProviderId}`}
                      className="text-[11px] text-[var(--text-quaternary)]"
                    >
                      Cookies
                    </label>
                    <select
                      id={`cookie-source-${selectedProviderId}`}
                      value={selectedCookieSource}
                      onChange={(event) => handleCookieSourceChange(selectedProviderId, event.target.value as CookieSource)}
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
                      onClick={() => setManualCookiePanels((state) => ({
                        ...state,
                        [selectedProviderId]: !state[selectedProviderId],
                      }))}
                      className="text-[11px] text-[var(--text-secondary)] hover:text-[var(--text-primary)] transition-colors"
                    >
                      {manualCookiePanels[selectedProviderId] ? 'Hide manual' : 'Paste cookies'}
                    </button>
                  </div>
                )}
              </div>

              {selectedUsesCookies && manualCookiePanels[selectedProviderId] && (
                <div className="mt-3 p-3 rounded-lg bg-[var(--accent-warning)]/5 border border-[var(--accent-warning)]/20">
                  <p className="text-[12px] text-[var(--text-tertiary)] mb-2">
                    Manual fallback: Copy Cookie header from DevTools Network tab
                  </p>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      value={manualCookieInputs[selectedProviderId] ?? ''}
                      onChange={(event) => setManualCookieInputs((state) => ({
                        ...state,
                        [selectedProviderId]: event.target.value,
                      }))}
                      placeholder="Paste Cookie header…"
                      aria-label={`Cookie header value for ${selectedProvider.name}`}
                      className="flex-1 px-3 py-2 text-[13px] bg-[var(--bg-base)] rounded-md border border-[var(--border-default)] text-[var(--text-primary)] placeholder:text-[var(--text-quaternary)] focus:outline-none focus:border-[var(--accent-primary)] transition-colors"
                      autoComplete="off"
                      spellCheck={false}
                    />
                    <button
                      onClick={() => handleSubmitCookies(selectedProviderId)}
                      disabled={loggingIn === selectedProviderId}
                      className="btn btn-primary focus-ring"
                      aria-label={`Submit cookies for ${selectedProvider.name}`}
                    >
                      {loggingIn === selectedProviderId ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                      ) : (
                        <ClipboardPaste className="w-3.5 h-3.5" aria-hidden="true" />
                      )}
                    </button>
                  </div>
                </div>
              )}

              {selectedProviderId === 'cursor' && cursorLoginOpen && (
                <div className="mt-3 p-3 rounded-lg bg-[var(--accent-primary)]/5 border border-[var(--accent-primary)]/20">
                  <p className="text-[12px] text-[var(--text-tertiary)] mb-3">
                    Login window open. Complete login in browser.
                  </p>
                  <div className="flex gap-2">
                    <button
                      onClick={() => handleImportBrowserCookies('cursor')}
                      disabled={loggingIn === 'cursor'}
                      className="btn btn-primary focus-ring flex-1 text-[12px]"
                    >
                      {loggingIn === 'cursor' ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                      ) : (
                        <Cookie className="w-3.5 h-3.5" aria-hidden="true" />
                      )}
                      <span>Import from Browser</span>
                    </button>
                    <button
                      onClick={handleExtractCookies}
                      disabled={loggingIn === 'cursor'}
                      className="btn btn-ghost focus-ring flex-1 text-[12px]"
                    >
                      <span>Extract from Window</span>
                    </button>
                  </div>
                </div>
              )}
            </div>
          )}
          
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

        <div className="divider mx-4" />

        {/* Refresh Interval */}
        <section className="p-4">
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

        <div className="divider mx-4" />

        {/* Display Options */}
        <section className="p-4">
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
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-2">
                Choose session usage, weekly usage, or weekly pace.
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
            </div>
            <div className="divider" />
            <div className="space-y-1">
              <ToggleOption label="Show Credits" enabled={showCredits} onChange={handleSetShowCredits} />
              <ToggleOption label="Show Cost" enabled={showCost} onChange={handleSetShowCost} />
              <ToggleOption label="Notifications" enabled={showNotifications} onChange={handleSetShowNotifications} />
              <ToggleOption label="Launch at Login" enabled={launchAtLogin} onChange={handleSetLaunchAtLogin} />
            </div>
          </div>
        </section>

        <div className="divider mx-4" />

        {/* Reset */}
        <section className="p-4">
          <button
            onClick={handleResetToDefaults}
            className="w-full btn btn-ghost focus-ring justify-center"
          >
            <RotateCcw className="w-3.5 h-3.5" aria-hidden="true" />
            <span>Reset to Defaults</span>
          </button>
        </section>

        {/* Version */}
        <footer className="px-4 pb-4 pt-2 text-center">
          <span className="text-[11px] text-[var(--text-quaternary)]">IncuBar v0.1.0</span>
        </footer>
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
