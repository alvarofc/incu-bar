import type { UpdateChannel } from './types';

const PRERELEASE_KEYWORDS = ['beta', 'alpha', 'rc', 'pre', 'dev'];

export const UPDATE_CHANNEL_DESCRIPTIONS: Record<UpdateChannel, string> = {
  stable: 'Receive only stable, production-ready releases.',
  beta: 'Receive beta previews when a separate preview feed is published.',
};

export const UPDATE_CHANNEL_LIMITATION_NOTE =
  'Beta previews are not published to the current GitHub updater feed yet, so both channels currently use the same release feed.';

export const isPrereleaseVersion = (version: string): boolean => {
  const normalized = version.trim().toLowerCase();
  if (!normalized) {
    return false;
  }
  return PRERELEASE_KEYWORDS.some((keyword) => normalized.includes(keyword));
};

export const getDefaultUpdateChannelForVersion = (version: string): UpdateChannel =>
  isPrereleaseVersion(version) ? 'beta' : 'stable';
