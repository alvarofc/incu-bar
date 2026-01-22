import { formatDistanceToNow, format, differenceInHours } from 'date-fns';

interface ResetCountdownProps {
  resetsAt?: string;        // ISO date string
  description?: string;     // Pre-formatted description from backend
  className?: string;
}

export function ResetCountdown({
  resetsAt,
  description,
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
    const now = new Date();
    const hoursUntilReset = differenceInHours(resetDate, now);

    // Show relative time for near future, absolute for far future
    const formattedTime = hoursUntilReset < 24
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
