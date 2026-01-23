import { useEffect, useRef, useCallback } from 'react';
import { Settings, RefreshCw, Plug } from 'lucide-react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { MenuCard } from './MenuCard';
import { ProviderTabs, ProviderSwitcherButtons } from './ProviderTabs';
import { useUsageStore, useActiveProvider, useEnabledProviders } from '../stores/usageStore';
import { useSettingsStore } from '../stores/settingsStore';

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

  // Check if we're still loading (first refresh in progress)
  const isInitialLoading = !lastGlobalRefresh && enabledProviders.some((p) => p.isLoading);

  const handleRefreshAll = useCallback(() => {
    useUsageStore.getState().refreshAllProviders();
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
