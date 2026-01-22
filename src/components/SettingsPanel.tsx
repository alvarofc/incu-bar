import { useCallback, useEffect, useState } from 'react';
import { ArrowLeft, Check, RotateCcw, LogIn, Loader2, AlertCircle, ClipboardPaste, Cookie, Copy, ExternalLink } from 'lucide-react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import type { ProviderId } from '../lib/types';
import { PROVIDERS } from '../lib/providers';
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
  const refreshIntervalSeconds = useSettingsStore((s) => s.refreshIntervalSeconds);
  const showCredits = useSettingsStore((s) => s.showCredits);
  const showCost = useSettingsStore((s) => s.showCost);
  const showNotifications = useSettingsStore((s) => s.showNotifications);
  const launchAtLogin = useSettingsStore((s) => s.launchAtLogin);

  const [authStatus, setAuthStatus] = useState<Record<string, AuthStatus>>({});
  const [loggingIn, setLoggingIn] = useState<string | null>(null);
  const [loginMessage, setLoginMessage] = useState<string | null>(null);
  const [showCookieInput, setShowCookieInput] = useState(false);
  const [cookieInput, setCookieInput] = useState('');
  const [cursorLoginOpen, setCursorLoginOpen] = useState(false);
  
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
    const unlistenLogin = listen('cursor-login-detected', async () => {
      setLoginMessage('Login detected! Extracting cookies…');
      try {
        const result = await invoke<LoginResult>('extract_cursor_cookies');
        if (result.success) {
          setLoginMessage(result.message);
          setCursorLoginOpen(false);
          setShowCookieInput(false);
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          setAuthStatus(status);
          useSettingsStore.getState().enableProvider('cursor');
          useUsageStore.getState().refreshProvider('cursor');
        }
      } catch (e) {
        setLoginMessage(`Cookie extraction error: ${e}`);
        setShowCookieInput(true);
      }
    });

    const unlistenCompleted = listen('login-completed', async (event: { payload: { providerId: ProviderId; success: boolean; message: string } }) => {
      const { providerId, success, message } = event.payload;
      if (success) {
        setLoginMessage(message);
        setCursorLoginOpen(false);
        setShowCookieInput(false);
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
        setLoginMessage('Importing cookies from browser…');
        const importResult = await invoke<LoginResult>('import_cursor_browser_cookies');
        
        if (importResult.success) {
          setLoginMessage(importResult.message);
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          setAuthStatus(status);
          useSettingsStore.getState().enableProvider('cursor');
          useUsageStore.getState().refreshProvider('cursor');
          setLoggingIn(null);
          return;
        } else {
          setLoginMessage('Could not import from browser. Opening login window…');
        }
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
        if (providerId === 'cursor') {
          setLoginMessage('Login window opened. Login will be detected automatically.');
          setCursorLoginOpen(true);
        } else {
          setLoginMessage(`${PROVIDERS[providerId].name} connected!`);
          const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
          setAuthStatus(status);
          useSettingsStore.getState().enableProvider(providerId);
          useUsageStore.getState().refreshProvider(providerId);
        }
      } else {
        setLoginMessage(result.message);
      }
    } catch (e) {
      setLoginMessage(`Login failed: ${e}`);
    } finally {
      setLoggingIn(null);
    }
  }, []);

  const handleSubmitCookies = useCallback(async () => {
    if (!cookieInput.trim()) {
      setLoginMessage('Please paste your cookies first');
      return;
    }
    
    setLoggingIn('cursor');
    try {
      const result = await invoke<LoginResult>('store_cursor_cookies', { cookieHeader: cookieInput });
      if (result.success) {
        setLoginMessage('Cursor cookies saved!');
        setCookieInput('');
        setShowCookieInput(false);
        setCursorLoginOpen(false);
        await invoke('close_cursor_login');
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider('cursor');
        useUsageStore.getState().refreshProvider('cursor');
      } else {
        setLoginMessage(result.message);
      }
    } catch (e) {
      setLoginMessage(`Failed to save cookies: ${e}`);
    } finally {
      setLoggingIn(null);
    }
  }, [cookieInput]);

  const handleExtractCookies = useCallback(async () => {
    setLoggingIn('cursor');
    setLoginMessage('Extracting cookies…');
    
    try {
      const result = await invoke<LoginResult>('extract_cursor_cookies');
      if (result.success) {
        setLoginMessage(result.message);
        setCursorLoginOpen(false);
        setShowCookieInput(false);
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useUsageStore.getState().refreshProvider('cursor');
      } else {
        setLoginMessage(result.message);
        setShowCookieInput(true);
      }
    } catch (e) {
      setLoginMessage(`Failed to extract cookies: ${e}`);
      setShowCookieInput(true);
    } finally {
      setLoggingIn(null);
    }
  }, []);

  const handleImportBrowserCookies = useCallback(async () => {
    setLoggingIn('cursor');
    setLoginMessage('Importing cookies from browser…');
    
    try {
      const result = await invoke<LoginResult>('import_cursor_browser_cookies');
      if (result.success) {
        setLoginMessage(result.message);
        setCursorLoginOpen(false);
        setShowCookieInput(false);
        const status = await invoke<Record<string, AuthStatus>>('check_all_auth');
        setAuthStatus(status);
        useSettingsStore.getState().enableProvider('cursor');
        useUsageStore.getState().refreshProvider('cursor');
      } else {
        setLoginMessage(result.message);
        setShowCookieInput(true);
      }
    } catch (e) {
      setLoginMessage(`Failed to import cookies: ${e}`);
      setShowCookieInput(true);
    } finally {
      setLoggingIn(null);
    }
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

  const implementedProviders = (Object.keys(PROVIDERS) as ProviderId[]).filter(
    (id) => PROVIDERS[id].implemented
  );
  const upcomingProviders = (Object.keys(PROVIDERS) as ProviderId[]).filter(
    (id) => !PROVIDERS[id].implemented
  );

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
          <div className="space-y-1.5">
            {implementedProviders.map((id) => {
              const provider = PROVIDERS[id];
              const status = authStatus[id];
              const isLoggingIn = loggingIn === id;
              const isAuthenticated = status?.authenticated === true;
              const isEnabled = enabledProviders.includes(id);

              return (
                <div
                  key={id}
                  className="flex items-center justify-between px-3 py-2.5 rounded-lg bg-[var(--bg-surface)] border border-[var(--border-subtle)]"
                >
                  <div className="flex items-center gap-3 flex-1 min-w-0">
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
                  </div>
                  
                  <div className="flex items-center gap-2 flex-shrink-0">
                    {isAuthenticated && (
                      <button
                        onClick={() => handleToggleProvider(id)}
                        className="toggle focus-ring"
                        data-state={isEnabled ? 'checked' : 'unchecked'}
                        role="switch"
                        aria-checked={isEnabled}
                        aria-label={isEnabled ? `Hide ${provider.name}` : `Show ${provider.name}`}
                      >
                        <span className="toggle-thumb" />
                      </button>
                    )}
                    
                    <button
                      onClick={() => handleLogin(id)}
                      disabled={isLoggingIn}
                      className="btn btn-ghost focus-ring text-[13px]"
                    >
                      {isLoggingIn ? (
                        <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                      ) : (
                        <LogIn className="w-3.5 h-3.5" aria-hidden="true" />
                      )}
                      <span>{isAuthenticated ? 'Reconnect' : 'Connect'}</span>
                    </button>
                  </div>
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
          
          {/* Cursor Login Helper */}
          {cursorLoginOpen && (
            <div className="mt-3 p-3 rounded-lg bg-[var(--accent-primary)]/5 border border-[var(--accent-primary)]/20">
              <p className="text-[12px] text-[var(--text-tertiary)] mb-3">
                Login window open. Complete login in browser.
              </p>
              <div className="flex gap-2">
                <button
                  onClick={handleImportBrowserCookies}
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
          
          {/* Manual Cookie Input */}
          {showCookieInput && (
            <div className="mt-3 p-3 rounded-lg bg-[var(--accent-warning)]/5 border border-[var(--accent-warning)]/20">
              <p className="text-[12px] text-[var(--text-tertiary)] mb-2">
                Manual fallback: Copy Cookie header from DevTools Network tab
              </p>
              <div className="flex gap-2">
                <input
                  type="text"
                  value={cookieInput}
                  onChange={(e) => setCookieInput(e.target.value)}
                  placeholder="Paste Cookie header…"
                  aria-label="Cookie header value"
                  className="flex-1 px-3 py-2 text-[13px] bg-[var(--bg-base)] rounded-md border border-[var(--border-default)] text-[var(--text-primary)] placeholder:text-[var(--text-quaternary)] focus:outline-none focus:border-[var(--accent-primary)] transition-colors"
                  autoComplete="off"
                  spellCheck={false}
                />
                <button
                  onClick={handleSubmitCookies}
                  disabled={loggingIn === 'cursor'}
                  className="btn btn-primary focus-ring"
                  aria-label="Submit cookies"
                >
                  {loggingIn === 'cursor' ? (
                    <Loader2 className="w-3.5 h-3.5 animate-spin" aria-hidden="true" />
                  ) : (
                    <ClipboardPaste className="w-3.5 h-3.5" aria-hidden="true" />
                  )}
                </button>
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
          <div className="space-y-1">
            <ToggleOption label="Show Credits" enabled={showCredits} onChange={handleSetShowCredits} />
            <ToggleOption label="Show Cost" enabled={showCost} onChange={handleSetShowCost} />
            <ToggleOption label="Notifications" enabled={showNotifications} onChange={handleSetShowNotifications} />
            <ToggleOption label="Launch at Login" enabled={launchAtLogin} onChange={handleSetLaunchAtLogin} />
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
