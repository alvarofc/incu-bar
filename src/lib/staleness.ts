// Keep in sync with `src-tauri/src/tray/mod.rs` STALE_THRESHOLD_SECS (10 minutes).
export const DEFAULT_STALE_AFTER_MS = 600000;
export const STALE_USAGE_MULTIPLIER = 2;

export const getStaleAfterMs = (refreshIntervalSeconds: number) => {
  if (refreshIntervalSeconds <= 0) return DEFAULT_STALE_AFTER_MS;
  return refreshIntervalSeconds * 1000 * STALE_USAGE_MULTIPLIER;
};

export const isTimestampStale = (timestamp: string | undefined, staleAfterMs: number) => {
  if (!timestamp || staleAfterMs <= 0) return false;
  const parsed = Date.parse(timestamp);
  if (!Number.isFinite(parsed)) return false;
  return Date.now() - parsed > staleAfterMs;
};
