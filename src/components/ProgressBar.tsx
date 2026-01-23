import { useMemo } from 'react';

interface ProgressBarProps {
  percent: number;        // 0-100, represents USED percentage
  label?: string;
  resetDescription?: string;
  showPercentage?: boolean;
  displayMode?: 'remaining' | 'used';
  size?: 'sm' | 'md';
  className?: string;
}

export function ProgressBar({
  percent,
  label,
  resetDescription,
  showPercentage = true,
  displayMode = 'remaining',
  size = 'md',
  className = '',
}: ProgressBarProps) {
  // Clamp percent between 0 and 100
  const clampedPercent = Math.min(100, Math.max(0, percent));
  const remainingPercent = 100 - clampedPercent;
  const isRemaining = displayMode === 'remaining';
  const trackPercent = isRemaining ? remainingPercent : clampedPercent;

  // Determine color based on remaining capacity
  const colorClass = useMemo(() => {
    if (isRemaining) {
      if (remainingPercent > 50) return 'progress-fill-success';
      if (remainingPercent > 20) return 'progress-fill-warning';
      return 'progress-fill-danger';
    }
    if (clampedPercent > 70) return 'progress-fill-danger';
    if (clampedPercent > 40) return 'progress-fill-warning';
    return 'progress-fill-success';
  }, [clampedPercent, isRemaining, remainingPercent]);

  const heightClass = size === 'sm' ? 'h-1' : 'h-1';

  return (
    <div className={`w-full ${className}`} role="progressbar" aria-valuenow={trackPercent} aria-valuemin={0} aria-valuemax={100}>
      {(label || showPercentage) && (
        <div className="flex items-baseline justify-between mb-2">
          {label && (
            <span className="text-[13px] font-medium text-[var(--text-secondary)]">
              {label}
            </span>
          )}
          <div className="flex items-baseline gap-1.5">
            {showPercentage && (
              <span className="text-[13px] font-semibold text-[var(--text-primary)] tabular-nums">
                {Math.round(trackPercent)}%
              </span>
            )}
            {showPercentage && (
              <span className="text-[11px] text-[var(--text-quaternary)]">
                {isRemaining ? 'remaining' : 'used'}
              </span>
            )}
          </div>
        </div>
      )}
      
      <div className={`progress-track ${heightClass}`}>
        <div
          className={`progress-fill ${colorClass}`}
          style={{ width: `${trackPercent}%` }}
        />
      </div>

      {resetDescription && (
        <div className="mt-1.5">
          <span className="text-[11px] text-[var(--text-quaternary)]">
            {resetDescription}
          </span>
        </div>
      )}
    </div>
  );
}
