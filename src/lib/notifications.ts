import type { ProviderId, UsageSnapshot } from './types';

export const SESSION_QUOTA_THRESHOLDS = [80, 90];
const SESSION_RESET_DROP = 5;
export const CREDIT_REMAINING_THRESHOLDS = [20, 10];
const CREDIT_RESET_RISE = 5;

export type SessionNotificationState = {
  lastPercent: number;
  triggered: Set<number>;
  resetMarker?: string;
};

export type SessionNotificationInput = {
  providerId: ProviderId;
  providerName: string;
  sessionLabel: string;
  usage: UsageSnapshot;
  showNotifications: boolean;
  stateMap: Map<ProviderId, SessionNotificationState>;
  notify: (title: string, body: string) => void;
};

export type CreditsNotificationState = {
  lastPercent: number;
  triggered: Set<number>;
  lastTotal?: number;
};

export type CreditsNotificationInput = {
  providerId: ProviderId;
  providerName: string;
  usage: UsageSnapshot;
  showNotifications: boolean;
  stateMap: Map<ProviderId, CreditsNotificationState>;
  notify: (title: string, body: string) => void;
};

const clampPercent = (value: number) => Math.min(100, Math.max(0, value));

const shouldResetSession = (
  currentPercent: number,
  lastPercent: number,
  resetMarker: string | undefined,
  nextMarker: string | undefined
) => {
  if (resetMarker && nextMarker && resetMarker !== nextMarker) return true;
  return lastPercent - currentPercent >= SESSION_RESET_DROP;
};

const shouldResetCredits = (
  currentPercent: number,
  lastPercent: number,
  lastTotal: number | undefined,
  nextTotal: number
) => {
  if (Number.isFinite(lastTotal) && lastTotal !== nextTotal) return true;
  return currentPercent - lastPercent >= CREDIT_RESET_RISE;
};

export const evaluateSessionNotifications = ({
  providerId,
  providerName,
  sessionLabel,
  usage,
  showNotifications,
  stateMap,
  notify,
}: SessionNotificationInput) => {
  if (!showNotifications) return;
  const session = usage.primary;
  if (!session || !Number.isFinite(session.usedPercent)) return;

  const currentPercent = clampPercent(session.usedPercent);
  const nextMarker = session.resetsAt;
  const previous = stateMap.get(providerId) ?? {
    lastPercent: currentPercent,
    triggered: new Set<number>(),
    resetMarker: nextMarker,
  };

  if (shouldResetSession(currentPercent, previous.lastPercent, previous.resetMarker, nextMarker)) {
    previous.triggered.clear();
  }

  SESSION_QUOTA_THRESHOLDS.forEach((threshold) => {
    if (currentPercent >= threshold && previous.lastPercent < threshold && !previous.triggered.has(threshold)) {
      notify(
        `${providerName} ${sessionLabel} usage`,
        `Reached ${threshold}% of ${sessionLabel.toLowerCase()} quota.`
      );
      previous.triggered.add(threshold);
    }
  });

  previous.lastPercent = currentPercent;
  previous.resetMarker = nextMarker ?? previous.resetMarker;
  stateMap.set(providerId, previous);
};

export const evaluateCreditsNotifications = ({
  providerId,
  providerName,
  usage,
  showNotifications,
  stateMap,
  notify,
}: CreditsNotificationInput) => {
  if (!showNotifications) return;
  const credits = usage.credits;
  if (!credits || !Number.isFinite(credits.remaining)) return;
  const total = credits.total;
  if (typeof total !== 'number' || !Number.isFinite(total)) return;
  if (total <= 0) return;

  const currentPercent = clampPercent((credits.remaining / total) * 100);
  const previous = stateMap.get(providerId) ?? {
    lastPercent: currentPercent,
    triggered: new Set<number>(),
    lastTotal: total,
  };

  if (shouldResetCredits(currentPercent, previous.lastPercent, previous.lastTotal, total)) {
    previous.triggered.clear();
  }

  CREDIT_REMAINING_THRESHOLDS.forEach((threshold) => {
    if (currentPercent <= threshold && previous.lastPercent > threshold && !previous.triggered.has(threshold)) {
      notify(
        `${providerName} credits low`,
        `Remaining ${credits.unit} is below ${threshold}% (${Math.max(0, credits.remaining).toLocaleString()} left).`
      );
      previous.triggered.add(threshold);
    }
  });

  previous.lastPercent = currentPercent;
  previous.lastTotal = total;
  stateMap.set(providerId, previous);
};
