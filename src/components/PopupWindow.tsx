import { useEffect, useRef, useCallback } from 'react';
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
  const isRefreshing = useUsageStore((s) => s.isRefreshing);
  const lastGlobalRefresh = useUsageStore((s) => s.lastGlobalRefresh);
  const displayMode = useSettingsStore((s) => s.displayMode);
  const hasRefreshedRef = useRef(false);
  const errorProviders = enabledProviders.filter(
    (provider) => provider.lastError && !provider.usage
  );
  const primaryErrorProvider = errorProviders[0];

  // Check if we're still loading (first refresh in progress)
  const isInitialLoading = !lastGlobalRefresh && enabledProviders.some((p) => p.isLoading);

  const handleRefreshAll = useCallback(() => {
    useUsageStore.getState().refreshAllProviders();
  }, []);

  const handleRefreshProvider = useCallback((providerId: ProviderId) => {
    void useUsageStore.getState().refreshProvider(providerId);
  }, []);

  // Refresh all providers on mount (only once)
  useEffect(() => {
    if (!hasRefreshedRef.current) {
      hasRefreshedRef.current = true;
      handleRefreshAll();
    }
  }, [handleRefreshAll]);

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
        ) : activeProvider ? (
          <MenuCard provider={activeProvider} />
        ) : primaryErrorProvider ? (
          <div
            className="flex flex-col items-center justify-center py-10 px-6 text-center animate-slide-up"
            data-testid="provider-error-detail"
            role="alert"
          >
            <div className="w-11 h-11 rounded-full bg-[var(--accent-danger)]/10 flex items-center justify-center mb-4">
              <AlertCircle className="w-5 h-5 text-[var(--accent-danger)]" aria-hidden="true" />
            </div>
            <div className="flex items-center gap-2 mb-2">
              <ProviderIcon providerId={primaryErrorProvider.id} className="w-4 h-4" aria-hidden="true" />
              <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
                {PROVIDERS[primaryErrorProvider.id].name} needs attention
              </h2>
            </div>
            <p className="text-sm text-[var(--text-tertiary)] mb-3 max-w-[230px]">
              We could not refresh usage. Reconnect in Settings or try again after updating your login.
            </p>
            <div className="w-full max-w-[240px] rounded-lg border border-[var(--accent-danger)]/20 bg-[var(--accent-danger)]/5 px-3 py-2 text-[12px] text-[var(--accent-danger)]/90 mb-4">
              {primaryErrorProvider.lastError}
            </div>
            <div className="flex gap-2">
              <button
                onClick={() => handleRefreshProvider(primaryErrorProvider.id)}
                disabled={primaryErrorProvider.isLoading}
                className="btn btn-primary focus-ring"
              >
                {primaryErrorProvider.isLoading ? 'Refreshing…' : 'Try Again'}
              </button>
              <button
                onClick={onOpenSettings}
                className="btn btn-ghost focus-ring"
              >
                Open Settings
              </button>
            </div>
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
