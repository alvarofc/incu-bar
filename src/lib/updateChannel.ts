import type { UpdateChannel } from './types';

const PRERELEASE_KEYWORDS = ['beta', 'alpha', 'rc', 'pre', 'dev'];

export const isPrereleaseVersion = (version: string): boolean => {
  const normalized = version.trim().toLowerCase();
  if (!normalized) {
    return false;
  }
  return PRERELEASE_KEYWORDS.some((keyword) => normalized.includes(keyword));
};

export const getDefaultUpdateChannelForVersion = (version: string): UpdateChannel =>
  isPrereleaseVersion(version) ? 'beta' : 'stable';
