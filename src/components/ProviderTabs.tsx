import type { ProviderId } from '../lib/types';
import { PROVIDERS } from '../lib/providers';
import { useUsageStore, useEnabledProviders } from '../stores/usageStore';
import { ProviderIcon as BrandIcon } from './ProviderIcons';

export function ProviderTabs() {
  const activeProvider = useUsageStore((s) => s.activeProvider);
  const setActiveProvider = useUsageStore((s) => s.setActiveProvider);
  const enabledProviders = useEnabledProviders();

  // Only show providers that have valid usage data (authenticated)
  const authenticatedProviders = enabledProviders.filter(
    (p) => p.usage && !p.lastError
  );

  if (authenticatedProviders.length <= 1) {
    return null;
  }

  return (
    <nav 
      className="flex items-center gap-0.5 px-2 py-2 border-b border-[var(--border-subtle)]"
      role="tablist"
      aria-label="Provider tabs"
    >
      {authenticatedProviders.map((provider) => {
        const metadata = PROVIDERS[provider.id];
        const isActive = activeProvider === provider.id;

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
            <BrandIcon 
              providerId={provider.id} 
              className={`w-4 h-4 ${isActive ? 'opacity-100' : 'opacity-70'}`}
              aria-hidden="true"
            />
            <span>{metadata.name}</span>
          </button>
        );
      })}
    </nav>
  );
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
