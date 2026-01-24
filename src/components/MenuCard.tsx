import { useCallback, useMemo } from 'react';
import { formatDistanceToNow } from 'date-fns';
import { RefreshCw, AlertCircle, Loader2, ExternalLink } from 'lucide-react';
import { ProgressBar } from './ProgressBar';
import { ProviderIcon, ProviderIconWithOverlay } from './ProviderIcons';
import type { ProviderState } from '../lib/types';
import { PROVIDERS } from '../lib/providers';
import { useUsageStore } from '../stores/usageStore';
import { useSettingsStore } from '../stores/settingsStore';
import { getStaleAfterMs, isTimestampStale } from '../lib/staleness';

interface MenuCardProps {
  provider: ProviderState;
}

export function MenuCard({ provider }: MenuCardProps) {
  const showCredits = useSettingsStore((s) => s.showCredits);
  const showCost = useSettingsStore((s) => s.showCost);
  const showExtraUsage = useSettingsStore((s) => s.showExtraUsage);
  const menuBarDisplayMode = useSettingsStore((s) => s.menuBarDisplayMode);
  const usageBarDisplayMode = useSettingsStore((s) => s.usageBarDisplayMode);
  const resetTimeDisplayMode = useSettingsStore((s) => s.resetTimeDisplayMode);
  const refreshIntervalSeconds = useSettingsStore((s) => s.refreshIntervalSeconds);
  const hidePersonalInfo = useSettingsStore((s) => s.hidePersonalInfo);

  const metadata = PROVIDERS[provider.id];
  const { usage, isLoading, lastError, status } = provider;
  const usageHistory = provider.usageHistory ?? [];
  const statusPageUrl = metadata.statusPageUrl;

  const handleRefresh = useCallback(() => {
    useUsageStore.getState().refreshProvider(provider.id);
  }, [provider.id]);

  const lastUpdatedText = usage?.updatedAt
    ? formatDistanceToNow(new Date(usage.updatedAt), { addSuffix: true })
    : 'Never';
  const staleAfterMs = getStaleAfterMs(refreshIntervalSeconds);
  const isStale = isTimestampStale(usage?.updatedAt, staleAfterMs);
  const freshnessLabel = usage?.updatedAt ? (isStale ? 'Stale' : 'Fresh') : 'No data';

  const statusLabel = (() => {
    if (!status?.indicator || status.indicator === 'none') return 'Operational';
    return status.indicator.charAt(0).toUpperCase() + status.indicator.slice(1);
  })();
  const statusUpdatedText = status?.updatedAt
    ? formatDistanceToNow(new Date(status.updatedAt), { addSuffix: true })
    : null;
  const statusLine = `${statusLabel}${status?.description ? ` · ${status.description}` : ''}${
    statusUpdatedText ? ` · ${statusUpdatedText}` : ''
  }`;

  const paceDetail = useMemo(() => {
    if (!usage?.secondary || !usage.secondary.resetsAt) return null;
    if (!['codex', 'claude'].includes(provider.id)) return null;
    const resetsAt = new Date(usage.secondary.resetsAt).getTime();
    if (Number.isNaN(resetsAt)) return null;
    const totalMinutes = usage.secondary.windowMinutes ?? 10080;
    if (totalMinutes <= 0) return null;
    const minutesRemaining = Math.max(0, (resetsAt - Date.now()) / 60000);
    if (minutesRemaining <= 0 || minutesRemaining > totalMinutes) return null;
    const elapsedMinutes = Math.max(0, totalMinutes - minutesRemaining);
    const expectedUsedPercent = (elapsedMinutes / totalMinutes) * 100;
    if (expectedUsedPercent < 3) return null;
    const actualUsedPercent = Math.min(100, Math.max(0, usage.secondary.usedPercent));
    if (elapsedMinutes === 0 && actualUsedPercent > 0) return null;
    const deltaPercent = actualUsedPercent - expectedUsedPercent;
    const onTrack = Math.abs(deltaPercent) <= 2;

    let etaSeconds: number | null = null;
    let willLastToReset = false;
    if (elapsedMinutes > 0) {
      if (actualUsedPercent > 0) {
        const rate = actualUsedPercent / (elapsedMinutes * 60);
        if (rate > 0) {
          const remainingPercent = Math.max(0, 100 - actualUsedPercent);
          const candidate = remainingPercent / rate;
          if (candidate >= minutesRemaining * 60) {
            willLastToReset = true;
          } else {
            etaSeconds = candidate;
          }
        }
      } else {
        willLastToReset = true;
      }
    }

    const deltaValue = Math.round(Math.abs(deltaPercent));
    const leftLabel = onTrack
      ? 'On pace'
      : deltaPercent >= 0
        ? `${deltaValue}% in deficit`
        : `${deltaValue}% in reserve`;

    const rightLabel = (() => {
      if (willLastToReset) return 'Lasts until reset';
      if (etaSeconds === null) return null;
      if (etaSeconds <= 0) return 'Runs out now';
      const etaDate = new Date(Date.now() + etaSeconds * 1000);
      const durationText = formatDistanceToNow(etaDate, { addSuffix: false });
      return durationText === 'less than a minute' ? 'Runs out now' : `Runs out in ${durationText}`;
    })();

    return {
      leftLabel,
      rightLabel,
      expectedUsedPercent,
      deltaPercent,
      onTrack,
    };
  }, [provider.id, usage]);

  const paceText = useMemo(() => {
    if (!paceDetail) return null;
    if (paceDetail.onTrack) return 'On pace';
    const deltaValue = Math.round(Math.abs(paceDetail.deltaPercent));
    const sign = paceDetail.deltaPercent >= 0 ? '+' : '-';
    return `${sign}${deltaValue}%`;
  }, [paceDetail]);

  const weeklyLabel = metadata.weeklyLabel || 'Weekly';
  const sessionLabel = metadata.sessionLabel || 'Session';
  const extraLabel = usage?.tertiary?.label || 'Extra';

  const highestWindow = useMemo(() => {
    if (!usage) return null;
    const candidates = [
      usage.primary ? { window: usage.primary, label: usage.primary.label || sessionLabel } : null,
      usage.secondary ? { window: usage.secondary, label: usage.secondary.label || weeklyLabel } : null,
      usage.tertiary ? { window: usage.tertiary, label: usage.tertiary.label || extraLabel } : null,
    ].filter((candidate): candidate is { window: NonNullable<typeof usage.primary>; label: string } => !!candidate);

    if (!candidates.length) return null;
    return candidates.reduce((highest, current) =>
      current.window.usedPercent > highest.window.usedPercent ? current : highest
    );
  }, [extraLabel, sessionLabel, usage, weeklyLabel]);

  const primaryWindow = useMemo(() => {
    if (!usage) return null;
    if (menuBarDisplayMode === 'highest') return highestWindow?.window ?? null;
    if (menuBarDisplayMode === 'weekly') return usage.secondary ?? null;
    return usage.primary ?? null;
  }, [highestWindow, menuBarDisplayMode, usage]);

  const primaryLabel = useMemo(() => {
    if (menuBarDisplayMode === 'highest') return highestWindow?.label || sessionLabel;
    if (menuBarDisplayMode === 'weekly') return weeklyLabel;
    return sessionLabel;
  }, [highestWindow, menuBarDisplayMode, sessionLabel, weeklyLabel]);

  const showSecondary = menuBarDisplayMode === 'session' && !!usage?.secondary;
  const usageBreakdown = useMemo(() => {
    if (!usage) return [];
    return [
      usage.primary
        ? {
            label: usage.primary.label || sessionLabel,
            percent: usage.primary.usedPercent,
            color: 'var(--accent-primary)',
          }
        : null,
      usage.secondary
        ? {
            label: usage.secondary.label || weeklyLabel,
            percent: usage.secondary.usedPercent,
            color: 'var(--accent-warning)',
          }
        : null,
      usage.tertiary
        ? {
            label: usage.tertiary.label || extraLabel,
            percent: usage.tertiary.usedPercent,
            color: 'var(--accent-danger)',
          }
        : null,
    ].filter((item): item is { label: string; percent: number; color: string } => !!item);
  }, [extraLabel, sessionLabel, usage, weeklyLabel]);

  const costHistory = useMemo(
    () => usageHistory.map((point) => point.cost).filter((value): value is number => value !== undefined),
    [usageHistory]
  );

  const creditsHistory = useMemo(
    () => usageHistory.map((point) => point.credits).filter((value): value is number => value !== undefined),
    [usageHistory]
  );

  const latestCost = costHistory[costHistory.length - 1];
  const latestCredits = creditsHistory[creditsHistory.length - 1];

  const sparklinePoints = (values: number[], width = 120, height = 32) => {
    if (values.length < 2) return null;
    const min = Math.min(...values);
    const max = Math.max(...values);
    const range = Math.max(1e-6, max - min);
    return values
      .map((value, index) => {
        const x = (index / (values.length - 1)) * width;
        const y = height - ((value - min) / range) * height;
        return `${x.toFixed(2)},${y.toFixed(2)}`;
      })
      .join(' ');
  };

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
            <span className="relative">
              <ProviderIcon 
                providerId={provider.id} 
                className="w-5 h-5 text-[var(--text-secondary)]" 
                aria-hidden="true"
              />
              <ProviderIconWithOverlay indicator={status?.indicator} />
            </span>
            <h2 className="text-[15px] font-semibold text-[var(--text-primary)]">
              {metadata.name}
            </h2>
            {usage?.identity?.plan && !hidePersonalInfo && (
              <span className="badge">
                {usage.identity.plan}
              </span>
            )}
          </div>
          {usage?.identity?.email && !hidePersonalInfo && (
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
              resetsAt={primaryWindow.resetsAt}
              resetTimeDisplayMode={resetTimeDisplayMode}
              size="md"
              displayMode={usageBarDisplayMode}
            />
          )}

          {showSecondary && usage.secondary && (
            <div className="space-y-1">
              <ProgressBar
                percent={usage.secondary.usedPercent}
                label={usage.secondary.label || weeklyLabel}
                resetDescription={usage.secondary.resetDescription}
                resetsAt={usage.secondary.resetsAt}
                resetTimeDisplayMode={resetTimeDisplayMode}
                size="md"
                displayMode={usageBarDisplayMode}
              />
              {paceDetail && (
                <div className="flex items-center justify-between text-[11px]">
                  <span className="text-[var(--text-secondary)]">{paceDetail.leftLabel}</span>
                  {paceDetail.rightLabel && (
                    <span className="text-[var(--text-quaternary)]">{paceDetail.rightLabel}</span>
                  )}
                </div>
              )}
            </div>
          )}

          {showExtraUsage && usage.tertiary && (
            <ProgressBar
              percent={usage.tertiary.usedPercent}
              label={usage.tertiary.label || 'Extra'}
              resetDescription={usage.tertiary.resetDescription}
              resetsAt={usage.tertiary.resetsAt}
              resetTimeDisplayMode={resetTimeDisplayMode}
              size="sm"
              displayMode={usageBarDisplayMode}
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
                {paceDetail
                  ? paceDetail.rightLabel ?? `Expected usage ${Math.round(paceDetail.expectedUsedPercent)}% by now.`
                  : 'Need a weekly window to estimate pace.'}
              </p>
              {paceDetail && paceDetail.leftLabel !== paceText && (
                <p className="text-[11px] text-[var(--text-secondary)] mt-1">
                  {paceDetail.leftLabel}
                </p>
              )}
            </div>
          )}

          {status && status.indicator !== 'none' && statusPageUrl && (
            <div
              className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-2.5"
              data-testid="provider-status-section"
            >
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-medium text-[var(--text-secondary)]">Status</span>
                <span className="text-[12px] font-semibold uppercase text-[var(--text-primary)]">
                  {status.indicator}
                </span>
              </div>
              {status.description && (
                <p className="text-[11px] text-[var(--text-quaternary)] mt-1">
                  {status.description}
                </p>
              )}
              <a
                href={statusPageUrl}
                target="_blank"
                rel="noreferrer"
                className="mt-2 inline-flex items-center gap-1 text-[11px] text-[var(--text-tertiary)] hover:text-[var(--text-primary)] transition-colors"
                data-testid="provider-status-link"
              >
                <ExternalLink className="w-3 h-3" aria-hidden="true" />
                <span>View status page</span>
              </a>
            </div>
          )}
        </div>
      )}

      {usage && usageBreakdown.length > 0 && (
        <div className="mt-4 rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-3">
          <div className="flex items-center justify-between">
            <span className="text-[13px] font-medium text-[var(--text-secondary)]">Usage Breakdown</span>
            <span className="text-[11px] text-[var(--text-quaternary)]">Current windows</span>
          </div>
          <div className="mt-3 flex items-end gap-2 h-16">
            {usageBreakdown.map((item) => (
              <div key={item.label} className="flex-1 flex flex-col items-center gap-1">
                <div className="flex-1 w-full flex items-end">
                  <div
                    className="w-full rounded-sm"
                    style={{
                      height: `${Math.max(6, Math.min(item.percent, 100))}%`,
                      background: item.color,
                    }}
                  />
                </div>
                <span className="text-[10px] text-[var(--text-quaternary)] truncate w-full text-center">
                  {item.label}
                </span>
                <span className="text-[11px] font-medium text-[var(--text-secondary)] tabular-nums">
                  {Math.round(item.percent)}%
                </span>
              </div>
            ))}
          </div>
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

      {(showCost || showCredits) && (costHistory.length > 0 || creditsHistory.length > 0) && (
        <div className="mt-4 pt-4 border-t border-[var(--border-subtle)] space-y-3">
          {showCost && usage?.cost && costHistory.length > 0 && (
            <div className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-3">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-medium text-[var(--text-secondary)]">Cost History</span>
                <span className="text-[12px] text-[var(--text-primary)] tabular-nums">
                  {latestCost !== undefined ? `${usage.cost.currency}${latestCost.toFixed(2)}` : '—'}
                </span>
              </div>
              <div className="mt-2 h-8">
                {sparklinePoints(costHistory) ? (
                  <svg viewBox="0 0 120 32" className="w-full h-8" aria-hidden="true">
                    <polyline
                      points={sparklinePoints(costHistory) ?? ''}
                      fill="none"
                      stroke="var(--accent-primary)"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                ) : (
                  <div className="h-8 flex items-center text-[11px] text-[var(--text-quaternary)]">
                    Not enough data yet
                  </div>
                )}
              </div>
            </div>
          )}
          {showCredits && usage?.credits && creditsHistory.length > 0 && (
            <div className="rounded-lg border border-[var(--border-subtle)] bg-[var(--bg-surface)] px-3 py-3">
              <div className="flex items-center justify-between">
                <span className="text-[13px] font-medium text-[var(--text-secondary)]">Credits History</span>
                <span className="text-[12px] text-[var(--text-primary)] tabular-nums">
                  {latestCredits !== undefined ? latestCredits.toLocaleString() : '—'}
                </span>
              </div>
              <div className="mt-2 h-8">
                {sparklinePoints(creditsHistory) ? (
                  <svg viewBox="0 0 120 32" className="w-full h-8" aria-hidden="true">
                    <polyline
                      points={sparklinePoints(creditsHistory) ?? ''}
                      fill="none"
                      stroke="var(--accent-success)"
                      strokeWidth="2"
                      strokeLinecap="round"
                      strokeLinejoin="round"
                    />
                  </svg>
                ) : (
                  <div className="h-8 flex items-center text-[11px] text-[var(--text-quaternary)]">
                    Not enough data yet
                  </div>
                )}
              </div>
              <p className="mt-1 text-[10px] text-[var(--text-quaternary)]">
                Remaining {usage.credits.unit}
              </p>
            </div>
          )}
        </div>
      )}

      {/* Footer */}
      <div className="mt-4 pt-3 border-t border-[var(--border-subtle)] space-y-1">
        <div
          className="text-[11px] text-[var(--text-quaternary)]"
          data-testid="provider-freshness-line"
        >
          Updated {lastUpdatedText} · {freshnessLabel}
        </div>
        <div
          className="text-[11px] text-[var(--text-quaternary)]"
          data-testid="provider-status-line"
        >
          Status {statusLine}
        </div>
      </div>
    </div>
  );
}
