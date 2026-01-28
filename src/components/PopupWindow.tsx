import { useEffect, useRef, useCallback, useMemo, useState } from 'react';
import { Settings, RefreshCw, Plug, AlertCircle } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MenuCard } from './MenuCard';
import { ProviderTabs, ProviderSwitcherButtons } from './ProviderTabs';
import { useUsageStore, useActiveProvider, useEnabledProviders } from '../stores/usageStore';
import { useSettingsStore } from '../stores/settingsStore';
import { ProviderIcon } from './ProviderIcons';
import { PROVIDERS } from '../lib/providers';
import type { ProviderId } from '../lib/types';

interface PopupWindowProps {
  onOpenSettings?: () => void;
}

export function PopupWindow({ onOpenSettings }: PopupWindowProps) {
  const activeProvider = useActiveProvider();
  const enabledProviders = useEnabledProviders();
  const settingsEnabledProviders = useSettingsStore((s) => s.enabledProviders);
  const isRefreshing = useUsageStore((s) => s.isRefreshing);
  const lastGlobalRefresh = useUsageStore((s) => s.lastGlobalRefresh);
  const hasHydrated = useSettingsStore((s) => s.hasHydrated);
  const displayMode = useSettingsStore((s) => s.displayMode);
  const selectedProviderId = useUsageStore((s) => s.activeProvider);
  const lastRefreshKeyRef = useRef<string | null>(null);
  const hasEnabledProvidersInSettings = settingsEnabledProviders.length > 0;
  
  // Get the currently selected provider (might be an error provider)
  const selectedProvider = enabledProviders.find((p) => p.id === selectedProviderId);
  const selectedHasError = selectedProvider?.lastError && !selectedProvider?.usage;
  const selectedIsLoading = selectedProvider?.isLoading;
  const selectedHasNoData = selectedProvider && !selectedProvider.usage && !selectedProvider.lastError && !selectedProvider.isLoading;
  
  // Check if usage store is synced with settings store
  const usageEnabledIds = enabledProviders.map((p) => p.id).sort().join('|');
  const settingsEnabledIds = [...settingsEnabledProviders].sort().join('|');
  const isProvidersSynced = usageEnabledIds === settingsEnabledIds;

  // Check if any enabled provider already has results (usage or error)
  const hasAnyResult = enabledProviders.some((p) => p.usage || p.lastError);

  // Timeout for initial loading - don't show spinner forever
  const [loadingTimedOut, setLoadingTimedOut] = useState(false);
  useEffect(() => {
    // Reset timeout when we get any result or global refresh completes
    if (hasAnyResult || lastGlobalRefresh) {
      setLoadingTimedOut(false);
      return;
    }
    const timer = setTimeout(() => {
      console.log('[PopupWindow] Initial loading timed out after 5s');
      setLoadingTimedOut(true);
    }, 5000);
    return () => clearTimeout(timer);
  }, [hasAnyResult, lastGlobalRefresh]);

  // Check if we're still loading (first refresh in progress)
  // Show loading only if we have no results yet from any provider
  // But cap it at 5 seconds to avoid infinite loading
  const isInitialLoading = hasHydrated && !hasAnyResult && hasEnabledProvidersInSettings && !loadingTimedOut;

  // DEBUG: Log state for troubleshooting
  console.log('[PopupWindow] State:', {
    hasHydrated,
    settingsEnabledProviders,
    hasEnabledProvidersInSettings,
    activeProvider: activeProvider?.id ?? null,
    enabledProvidersCount: enabledProviders.length,
    lastGlobalRefresh,
    isInitialLoading,
    isRefreshing,
    isProvidersSynced,
    loadingTimedOut,
    hasAnyResult,
  });

  const handleRefreshAll = useCallback(() => {
    useUsageStore.getState().refreshAllProviders();
  }, []);

  const handleRefreshProvider = useCallback((providerId: ProviderId) => {
    void useUsageStore.getState().refreshProvider(providerId);
  }, []);

  const enabledProviderIdsKey = useMemo(
    () => settingsEnabledProviders.join('|'),
    [settingsEnabledProviders]
  );

  // Refresh providers when enabled set changes (including initial hydration)
  // Wait for usage store to be synced with settings before triggering refresh
  useEffect(() => {
    console.log('[PopupWindow] Refresh effect check:', { hasHydrated, enabledProviderIdsKey, isRefreshing, isProvidersSynced, lastRefreshKey: lastRefreshKeyRef.current });
    if (!hasHydrated || !enabledProviderIdsKey || isRefreshing || !isProvidersSynced) {
      return;
    }
    if (lastRefreshKeyRef.current === enabledProviderIdsKey) {
      return;
    }
    console.log('[PopupWindow] Triggering refreshAllProviders');
    lastRefreshKeyRef.current = enabledProviderIdsKey;
    handleRefreshAll();
  }, [hasHydrated, enabledProviderIdsKey, handleRefreshAll, isRefreshing, isProvidersSynced]);

  // Handle click outside to close the popup
  useEffect(() => {
    const handleBlur = async () => {
      const win = getCurrentWindow();
      await win.hide();
    };

    window.addEventListener('blur', handleBlur);
    return () => window.removeEventListener('blur', handleBlur);
  }, []);

  // Handle escape key to close
  useEffect(() => {
    const handleKeyDown = async (e: KeyboardEvent) => {
      if (e.key === 'Escape') {
        const win = getCurrentWindow();
        await win.hide();
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, []);

  return (
    <div className="popup-container">
      {/* Provider tabs */}
      {displayMode === 'merged' ? <ProviderSwitcherButtons /> : <ProviderTabs />}

      {/* Main content */}
      <div className="flex-1 overflow-y-auto">
        {isInitialLoading ? (
          <div className="flex flex-col items-center justify-center py-12 animate-fade-in">
            <RefreshCw 
              className="w-5 h-5 text-[var(--text-quaternary)] animate-spin" 
              aria-hidden="true"
            />
            <p className="mt-3 text-sm text-[var(--text-tertiary)]">
              Loading…
            </p>
          </div>
        ) : selectedIsLoading && selectedProvider ? (
          // Show loading for selected provider
          <div className="flex flex-col items-center justify-center py-12 animate-fade-in">
            <RefreshCw 
              className="w-5 h-5 text-[var(--text-quaternary)] animate-spin" 
              aria-hidden="true"
            />
            <p className="mt-3 text-sm text-[var(--text-tertiary)]">
              Loading {PROVIDERS[selectedProvider.id].name}…
            </p>
          </div>
        ) : selectedHasError && selectedProvider ? (
          // Show error details for the selected error provider
          <div
            className="flex flex-col items-center justify-center py-10 px-6 text-center animate-slide-up"
            data-testid="provider-error-detail"
            role="alert"
          >
            <div className="w-11 h-11 rounded-full bg-[var(--accent-warning)]/10 flex items-center justify-center mb-4">
              <AlertCircle className="w-5 h-5 text-[var(--accent-warning)]" aria-hidden="true" />
            </div>
            <div className="flex items-center gap-2 mb-2">
              <ProviderIcon providerId={selectedProvider.id} className="w-4 h-4" aria-hidden="true" />
              <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
                {PROVIDERS[selectedProvider.id].name} needs attention
              </h2>
            </div>
            <p className="text-sm text-[var(--text-tertiary)] mb-3 max-w-[230px]">
              We could not refresh usage. Reconnect in Settings or try again.
            </p>
            <div className="w-full max-w-[240px] rounded-lg border border-[var(--accent-warning)]/20 bg-[var(--accent-warning)]/5 px-3 py-2 text-[12px] text-[var(--accent-warning)]/90 mb-4">
              {selectedProvider.lastError}
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => handleRefreshProvider(selectedProvider.id)}
                disabled={selectedProvider.isLoading}
                className="btn btn-primary focus-ring"
              >
                {selectedProvider.isLoading ? 'Refreshing…' : 'Try Again'}
              </button>
              <button
                onClick={onOpenSettings}
                className="btn btn-ghost focus-ring"
              >
                Open Settings
              </button>
            </div>
          </div>
        ) : selectedHasNoData && selectedProvider ? (
          // Selected provider has no data yet - needs setup or app not running
          <div
            className="flex flex-col items-center justify-center py-10 px-6 text-center animate-slide-up"
            data-testid="provider-no-data"
          >
            <div className="w-11 h-11 rounded-full bg-[var(--bg-subtle)] flex items-center justify-center mb-4">
              <ProviderIcon providerId={selectedProvider.id} className="w-5 h-5 text-[var(--text-quaternary)]" aria-hidden="true" />
            </div>
            <h2 className="text-[15px] font-semibold text-[var(--text-primary)] mb-1.5 text-balance">
              {PROVIDERS[selectedProvider.id].name}
            </h2>
            <p className="text-sm text-[var(--text-tertiary)] mb-5 max-w-[220px]">
              {PROVIDERS[selectedProvider.id].authMethod === 'local_config'
                ? `Launch ${PROVIDERS[selectedProvider.id].name} to see usage data`
                : 'No usage data available. Try refreshing or reconnect in Settings.'}
            </p>
            <div className="flex gap-2">
              <button
                onClick={() => handleRefreshProvider(selectedProvider.id)}
                disabled={selectedProvider.isLoading}
                className="btn btn-primary focus-ring"
              >
                Refresh
              </button>
              <button
                onClick={onOpenSettings}
                className="btn btn-ghost focus-ring"
              >
                Settings
              </button>
            </div>
          </div>
        ) : activeProvider ? (
          <MenuCard provider={activeProvider} />
        ) : !hasEnabledProvidersInSettings ? (
          <div
            className="flex flex-col items-center justify-center py-10 px-6 text-center animate-slide-up"
            data-testid="provider-enable-empty-state"
          >
            <div className="w-11 h-11 rounded-full bg-[var(--bg-subtle)] flex items-center justify-center mb-4">
              <Plug className="w-5 h-5 text-[var(--text-quaternary)]" aria-hidden="true" />
            </div>
            <h2 className="text-[15px] font-semibold text-[var(--text-primary)] mb-1.5 text-balance">
              No providers enabled
            </h2>
            <p className="text-sm text-[var(--text-tertiary)] mb-5 max-w-[220px]">
              Enable providers in Settings to start tracking usage
            </p>
            <button
              onClick={onOpenSettings}
              className="btn btn-primary focus-ring"
            >
              Enable Providers
            </button>
          </div>
        ) : (
          <div className="flex flex-col items-center justify-center py-10 px-6 text-center animate-slide-up">
            <div className="w-11 h-11 rounded-full bg-[var(--bg-subtle)] flex items-center justify-center mb-4">
              <Plug className="w-5 h-5 text-[var(--text-quaternary)]" aria-hidden="true" />
            </div>
            <h2 className="text-[15px] font-semibold text-[var(--text-primary)] mb-1.5 text-balance">
              Welcome to IncuBar
            </h2>
            <p className="text-sm text-[var(--text-tertiary)] mb-5 max-w-[200px]">
              Connect your AI coding assistants to track usage
            </p>
            <button
              onClick={onOpenSettings}
              className="btn btn-primary focus-ring"
            >
              Set Up Providers
            </button>
          </div>
        )}
      </div>

      {/* Footer */}
      <div className="flex items-center justify-between px-3 py-2.5 border-t border-[var(--border-subtle)]">
        <button
          onClick={handleRefreshAll}
          disabled={isRefreshing}
          className="btn btn-ghost focus-ring"
          aria-label="Refresh all providers"
        >
          <RefreshCw 
            className={`w-3.5 h-3.5 ${isRefreshing ? 'animate-spin' : ''}`} 
            aria-hidden="true"
          />
          <span className="text-[13px]">Refresh</span>
        </button>

        <button
          onClick={onOpenSettings}
          className="btn btn-ghost focus-ring"
          aria-label="Open settings"
        >
          <Settings className="w-3.5 h-3.5" aria-hidden="true" />
          <span className="text-[13px]">Settings</span>
        </button>
      </div>
    </div>
  );
}
