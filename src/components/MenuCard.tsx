import { useCallback, useMemo } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { RefreshCw, AlertCircle, Loader2 } from 'lucide-react';
import { ProgressBar } from './ProgressBar';
import { ProviderIcon } from './ProviderIcons';
import type { ProviderState } from '../lib/types';
import { PROVIDERS } from '../lib/providers';
import { useUsageStore } from '../stores/usageStore';
import { useSettingsStore } from '../stores/settingsStore';

interface MenuCardProps {
  provider: ProviderState;
}

export function MenuCard({ provider }: MenuCardProps) {
  const showCredits = useSettingsStore((s) => s.showCredits);
  const showCost = useSettingsStore((s) => s.showCost);
  const menuBarDisplayMode = useSettingsStore((s) => s.menuBarDisplayMode);

  const metadata = PROVIDERS[provider.id];
  const { usage, isLoading, lastError } = provider;

  const handleRefresh = useCallback(() => {
    useUsageStore.getState().refreshProvider(provider.id);
  }, [provider.id]);

  const lastUpdatedText = usage?.updatedAt
    ? formatDistanceToNow(new Date(usage.updatedAt), { addSuffix: true })
    : 'Never';

  const paceDelta = useMemo(() => {
    if (!usage?.secondary || !usage.secondary.windowMinutes || !usage.secondary.resetsAt) return null;
    const resetsAt = new Date(usage.secondary.resetsAt).getTime();
    if (Number.isNaN(resetsAt)) return null;
    const totalMinutes = usage.secondary.windowMinutes;
    if (totalMinutes <= 0) return null;
    const minutesRemaining = Math.max(0, (resetsAt - Date.now()) / 60000);
    const elapsedMinutes = Math.max(0, totalMinutes - minutesRemaining);
    const expectedUsedPercent = (elapsedMinutes / totalMinutes) * 100;
    if (expectedUsedPercent < 3) return null;
    const deltaPercent = usage.secondary.usedPercent - expectedUsedPercent;
    return {
      deltaPercent,
      expectedUsedPercent,
    };
  }, [usage]);

  const paceText = useMemo(() => {
    if (!paceDelta) return null;
    const deltaValue = Math.round(Math.abs(paceDelta.deltaPercent));
    const sign = paceDelta.deltaPercent >= 0 ? '+' : '-';
    return `${sign}${deltaValue}%`;
  }, [paceDelta]);

  const weeklyLabel = metadata.weeklyLabel || 'Weekly';
  const sessionLabel = metadata.sessionLabel || 'Session';

  const primaryWindow = useMemo(() => {
    if (!usage) return null;
    if (menuBarDisplayMode === 'weekly') return usage.secondary ?? null;
    return usage.primary ?? null;
  }, [menuBarDisplayMode, usage]);

  const primaryLabel = useMemo(() => {
    if (menuBarDisplayMode === 'weekly') return weeklyLabel;
    return sessionLabel;
  }, [menuBarDisplayMode, sessionLabel, weeklyLabel]);

  const showSecondary = menuBarDisplayMode === 'session' && !!usage?.secondary;

  return (
    <div 
      className="p-4 animate-fade-in"
      role="tabpanel"
      id={`panel-${provider.id}`}
      aria-label={`${metadata.name} usage`}
    >
      {/* Header */}
      <div className="flex items-start justify-between mb-4">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2.5">
            <ProviderIcon 
              providerId={provider.id} 
              className="w-5 h-5 text-[var(--text-secondary)]" 
              aria-hidden="true"
            />
            <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
              {metadata.name}
            </h2>
            {usage?.identity?.plan && (
              <span className="badge">
                {usage.identity.plan}
              </span>
            )}
          </div>
          {usage?.identity?.email && (
            <p className="text-[12px] text-[var(--text-tertiary)] mt-1 truncate">
              {usage.identity.email}
            </p>
          )}
        </div>

        <button
          onClick={handleRefresh}
          disabled={isLoading}
          className="btn btn-icon focus-ring"
          aria-label={`Refresh ${metadata.name}`}
        >
          {isLoading ? (
            <Loader2 className="w-4 h-4 animate-spin" aria-hidden="true" />
          ) : (
            <RefreshCw className="w-4 h-4" aria-hidden="true" />
          )}
        </button>
      </div>

      {/* Error State */}
      {lastError && !usage && (
        <div 
          className="flex items-start gap-2.5 p-3 mb-4 rounded-lg bg-[var(--accent-danger)]/10 border border-[var(--accent-danger)]/20"
          role="alert"
        >
          <AlertCircle className="w-4 h-4 text-[var(--accent-danger)] flex-shrink-0 mt-0.5" aria-hidden="true" />
          <p className="text-[13px] text-[var(--accent-danger)]/90">{lastError}</p>
        </div>
      )}

      {/* Usage Bars */}
      {usage && (
        <div className="space-y-4">
          {primaryWindow && (
            <ProgressBar
              percent={primaryWindow.usedPercent}
              label={primaryWindow.label || primaryLabel}
              resetDescription={primaryWindow.resetDescription}
              size="md"
            />
          )}

          {showSecondary && usage.secondary && (
            <ProgressBar
              percent={usage.secondary.usedPercent}
              label={usage.secondary.label || weeklyLabel}
              resetDescription={usage.secondary.resetDescription}
              size="md"
            />
          )}

          {usage.tertiary && (
            <ProgressBar
              percent={usage.tertiary.usedPercent}
              label={usage.tertiary.label || 'Extra'}
              resetDescription={usage.tertiary.resetDescription}
              size="sm"
            />
          )}

          {menuBarDisplayMode === 'pace' && usage.secondary && (
            <div className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-2.5">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-medium text-[var(--text-secondary)]">Pace</span>
                <span className="text-[13px] font-semibold text-[var(--text-primary)] tabular-nums">
                  {paceText ?? '—'}
                </span>
              </div>
              <p className="text-[11px] text-[var(--text-quaternary)] mt-1">
                {paceText ? `Expected usage ${Math.round(paceDelta?.expectedUsedPercent ?? 0)}% by now.` : 'Need a weekly window to estimate pace.'}
              </p>
            </div>
          )}
        </div>
      )}

      {/* Credits */}
      {showCredits && usage?.credits && (
        <div className="mt-4 pt-4 border-t border-[var(--border-subtle)]">
          <div className="flex items-baseline justify-between">
            <span className="text-[13px] text-[var(--text-tertiary)]">Credits</span>
            <div className="flex items-baseline gap-1">
              <span className="text-[14px] font-semibold text-[var(--text-primary)] tabular-nums">
                {usage.credits.remaining.toLocaleString()}
              </span>
              <span className="text-[11px] text-[var(--text-quaternary)]">
                {usage.credits.unit}
              </span>
            </div>
          </div>
        </div>
      )}

      {/* Cost */}
      {showCost && usage?.cost && (
        <div className="mt-4 pt-4 border-t border-[var(--border-subtle)] space-y-2">
          <div className="flex items-baseline justify-between">
            <span className="text-[13px] text-[var(--text-tertiary)]">Today</span>
            <div className="flex items-baseline gap-1.5">
              <span className="text-[14px] font-semibold text-[var(--text-primary)] tabular-nums">
                {usage.cost.currency}{usage.cost.todayAmount.toFixed(2)}
              </span>
              <span className="text-[11px] text-[var(--text-quaternary)] tabular-nums">
                {(usage.cost.todayTokens / 1000).toFixed(1)}K&nbsp;tokens
              </span>
            </div>
          </div>
          <div className="flex items-baseline justify-between">
            <span className="text-[13px] text-[var(--text-tertiary)]">This Month</span>
            <span className="text-[13px] text-[var(--text-secondary)] tabular-nums">
              {usage.cost.currency}{usage.cost.monthAmount.toFixed(2)}
            </span>
          </div>
        </div>
      )}

      {/* Footer */}
      <div className="mt-4 pt-3 border-t border-[var(--border-subtle)]">
        <span className="text-[11px] text-[var(--text-quaternary)]">
          Updated {lastUpdatedText}
        </span>
      </div>
    </div>
  );
}
