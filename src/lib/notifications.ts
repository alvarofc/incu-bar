import type { ProviderId, UsageSnapshot } from './types';

export const SESSION_QUOTA_THRESHOLDS = [80, 90];
const SESSION_RESET_DROP = 5;

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
