import { relaunch } from '@tauri-apps/plugin-process';
import { check } from '@tauri-apps/plugin-updater';
import type { UpdateChannel } from './types';

const UPDATE_LOCK_KEY = 'incubar-update-lock';
const UPDATE_LOCK_TTL_MS = 30 * 60 * 1000;
const UPDATE_LOCK_OWNER = `${Date.now()}-${Math.random().toString(36).slice(2)}`;

export type UpdateProgressState = 'checking' | 'installing';
export type UpdateResultStatus = 'busy' | 'upToDate' | 'installed' | 'error';

export interface RunAppUpdateResult {
  status: UpdateResultStatus;
  message: string;
}

interface RunAppUpdateOptions {
  channel: UpdateChannel;
  onProgress?: (state: UpdateProgressState, message: string) => void;
}

interface UpdateLockRecord {
  owner: string;
  expiresAt: number;
}

const parseUpdateLockRecord = (raw: string | null): UpdateLockRecord | null => {
  if (!raw) {
    return null;
  }

  try {
    const parsed = JSON.parse(raw) as Partial<UpdateLockRecord>;
    if (
      typeof parsed.owner !== 'string'
      || typeof parsed.expiresAt !== 'number'
      || !Number.isFinite(parsed.expiresAt)
    ) {
      return null;
    }
    return {
      owner: parsed.owner,
      expiresAt: parsed.expiresAt,
    };
  } catch {
    return null;
  }
};

const acquireUpdateLock = (): (() => void) | null => {
  if (typeof localStorage === 'undefined') {
    return () => undefined;
  }

  const now = Date.now();
  const existing = parseUpdateLockRecord(localStorage.getItem(UPDATE_LOCK_KEY));
  if (existing && existing.owner !== UPDATE_LOCK_OWNER && existing.expiresAt > now) {
    return null;
  }

  localStorage.setItem(
    UPDATE_LOCK_KEY,
    JSON.stringify({
      owner: UPDATE_LOCK_OWNER,
      expiresAt: now + UPDATE_LOCK_TTL_MS,
    } satisfies UpdateLockRecord)
  );

  return () => {
    const current = parseUpdateLockRecord(localStorage.getItem(UPDATE_LOCK_KEY));
    if (current?.owner === UPDATE_LOCK_OWNER) {
      localStorage.removeItem(UPDATE_LOCK_KEY);
    }
  };
};

export const getUpdaterRequestHeaders = (channel: UpdateChannel) => ({
  'X-Update-Channel': channel,
});

export const runAppUpdate = async ({
  channel,
  onProgress,
}: RunAppUpdateOptions): Promise<RunAppUpdateResult> => {
  const releaseLock = acquireUpdateLock();
  if (!releaseLock) {
    return {
      status: 'busy',
      message: 'Another update check is already in progress.',
    };
  }

  try {
    onProgress?.('checking', 'Checking for updates...');
    const update = await check({ headers: getUpdaterRequestHeaders(channel) });

    if (!update) {
      return {
        status: 'upToDate',
        message: 'No updates available.',
      };
    }

    onProgress?.('installing', 'Update found. Downloading and installing...');
    await update.downloadAndInstall();

    if (import.meta.env.DEV) {
      return {
        status: 'installed',
        message: 'Update installed successfully. Restart the dev server to apply changes.',
      };
    }

    try {
      await relaunch();
      return {
        status: 'installed',
        message: 'Update installed. Relaunching...',
      };
    } catch {
      return {
        status: 'installed',
        message: 'Update installed successfully. Please restart the application to complete the update.',
      };
    }
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    return {
      status: 'error',
      message: `Update failed: ${message}`,
    };
  } finally {
    releaseLock();
  }
};
