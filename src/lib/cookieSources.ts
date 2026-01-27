import type { CookieSource } from './types';

export const COOKIE_SOURCES: CookieSource[] = [
  'chrome',
  'safari',
  'firefox',
  'arc',
  'edge',
  'brave',
  'opera',
  'manual',
];

export const COOKIE_SOURCE_LABELS: Record<CookieSource, string> = {
  chrome: 'Chrome',
  safari: 'Safari',
  firefox: 'Firefox',
  arc: 'Arc',
  edge: 'Edge',
  brave: 'Brave',
  opera: 'Opera',
  manual: 'Manual',
};

export const DEFAULT_COOKIE_SOURCE: CookieSource = 'chrome';
