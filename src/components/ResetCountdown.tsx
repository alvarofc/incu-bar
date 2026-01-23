import { formatDistanceToNow, format } from 'date-fns';

interface ResetCountdownProps {
  resetsAt?: string;        // ISO date string
  description?: string;     // Pre-formatted description from backend
  displayMode?: 'relative' | 'absolute';
  className?: string;
}

export function ResetCountdown({
  resetsAt,
  description,
  displayMode = 'relative',
  className = '',
}: ResetCountdownProps) {
  if (!resetsAt && !description) return null;

  // If we have a pre-formatted description, use it
  if (description) {
    return (
      <span className={`text-[10px] text-white/40 ${className}`}>
        {description}
      </span>
    );
  }

  // Otherwise, format the reset time
  if (resetsAt) {
    const resetDate = new Date(resetsAt);
    const formattedTime = displayMode === 'relative'
      ? `Resets ${formatDistanceToNow(resetDate, { addSuffix: true })}`
      : `Resets ${format(resetDate, 'MMM d, h:mm a')}`;

    return (
      <span className={`text-[10px] text-white/40 ${className}`}>
        {formattedTime}
      </span>
    );
  }

  return null;
}
