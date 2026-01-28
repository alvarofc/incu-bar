import { AlertCircle } from 'lucide-react';
import type { ProviderId } from '../lib/types';
import { PROVIDERS } from '../lib/providers';
import { useUsageStore, useEnabledProviders } from '../stores/usageStore';
import { useSettingsStore } from '../stores/settingsStore';
import { ProviderIcon as BrandIcon, ProviderIconWithOverlay } from './ProviderIcons';

export function ProviderTabs() {
  const activeProvider = useUsageStore((s) => s.activeProvider);
  const setActiveProvider = useUsageStore((s) => s.setActiveProvider);
  const enabledProviders = useEnabledProviders();

  // Show providers that have usage data, have an error, or are the currently selected provider
  const visibleProviders = enabledProviders.filter(
    (p) => p.usage || p.lastError || p.id === activeProvider
  );

  if (visibleProviders.length <= 1) {
    return null;
  }

  return (
    <nav 
      className="flex items-center gap-0.5 px-2 py-2 border-b border-[var(--border-subtle)]"
      role="tablist"
      aria-label="Provider tabs"
    >
      {visibleProviders.map((provider) => {
        const metadata = PROVIDERS[provider.id];
        const isActive = activeProvider === provider.id;
        const hasError = provider.lastError && !provider.usage;
        const hasNoData = !provider.usage && !provider.lastError;

        return (
          <button
            key={provider.id}
            onClick={() => setActiveProvider(provider.id)}
            role="tab"
            aria-selected={isActive}
            aria-controls={`panel-${provider.id}`}
            className={`
              flex items-center gap-1.5 px-2.5 py-1.5 rounded-md text-[13px] font-medium
              transition-colors duration-[var(--transition-fast)]
              focus-ring
              ${isActive
                ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                : 'text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-surface)]'
              }
            `}
          >
            <span className="relative">
              <BrandIcon 
                providerId={provider.id} 
                className={`w-4 h-4 ${isActive ? 'opacity-100' : 'opacity-70'} ${hasNoData ? 'opacity-50' : ''}`}
                aria-hidden="true"
              />
              {hasError ? (
                <span className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-[var(--accent-warning)] rounded-full flex items-center justify-center">
                  <AlertCircle className="w-2 h-2 text-white" aria-hidden="true" />
                </span>
              ) : (
                <ProviderIconWithOverlay indicator={provider.status?.indicator} />
              )}
            </span>
            <span>{metadata.name}</span>
          </button>
        );
      })}
    </nav>
  );
}

export function ProviderSwitcherButtons() {
  const activeProvider = useUsageStore((s) => s.activeProvider);
  const setActiveProvider = useUsageStore((s) => s.setActiveProvider);
  const enabledProviders = useEnabledProviders();
  const switcherShowsIcons = useSettingsStore((s) => s.switcherShowsIcons);

  // Show providers that have usage data, have an error, or are the currently selected provider
  const visibleProviders = enabledProviders.filter(
    (provider) => provider.usage || provider.lastError || provider.id === activeProvider
  );

  if (visibleProviders.length <= 1) {
    return null;
  }

  if (switcherShowsIcons) {
    return (
      <div
        className="flex items-center gap-2 px-3 py-2 border-b border-[var(--border-subtle)]"
        role="toolbar"
        aria-label="Provider switcher"
        data-testid="provider-switcher"
      >
        {visibleProviders.map((provider) => {
          const isActive = activeProvider === provider.id;
          const hasError = provider.lastError && !provider.usage;
          const hasNoData = !provider.usage && !provider.lastError;

          return (
            <button
              key={provider.id}
              type="button"
              onClick={() => setActiveProvider(provider.id)}
              className={`flex items-center justify-center w-8 h-8 rounded-md transition-colors focus-ring ${
                isActive
                  ? 'bg-[var(--bg-subtle)] text-[var(--text-primary)]'
                  : 'text-[var(--text-tertiary)] hover:text-[var(--text-secondary)] hover:bg-[var(--bg-surface)]'
              }`}
              aria-pressed={isActive}
              aria-label={`Switch to ${PROVIDERS[provider.id].name}${hasError ? ' (needs attention)' : hasNoData ? ' (no data)' : ''}`}
              data-testid={`provider-switcher-button-${provider.id}`}
            >
              <span className="relative">
                <BrandIcon
                  providerId={provider.id}
                  className={`w-4 h-4 ${isActive ? 'opacity-100' : 'opacity-70'} ${hasNoData ? 'opacity-50' : ''}`}
                  aria-hidden="true"
                />
                {hasError ? (
                  <span className="absolute -top-1 -right-1 w-2.5 h-2.5 bg-[var(--accent-warning)] rounded-full flex items-center justify-center">
                    <AlertCircle className="w-2 h-2 text-white" aria-hidden="true" />
                  </span>
                ) : (
                  <ProviderIconWithOverlay indicator={provider.status?.indicator} />
                )}
              </span>
            </button>
          );
        })}
      </div>
    );
  }

  return null;
}

interface ProviderIconProps {
  providerId: ProviderId;
  size?: 'sm' | 'md' | 'lg';
  className?: string;
}

// Re-export the brand icon with size variants for backwards compatibility
export function ProviderIcon({ providerId, size = 'md', className = '' }: ProviderIconProps) {
  const sizeClass = {
    sm: 'w-3 h-3',
    md: 'w-4 h-4',
    lg: 'w-5 h-5',
  }[size];

  return <BrandIcon providerId={providerId} className={`${sizeClass} ${className}`} />;
}
