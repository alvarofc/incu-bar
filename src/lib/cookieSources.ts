import type { CookieSource } from './types';

export const COOKIE_SOURCES: CookieSource[] = [
  'chrome',
  'safari',
  'firefox',
  'arc',
  'edge',
  'brave',
  'opera',
];

export const COOKIE_SOURCE_LABELS: Record<CookieSource, string> = {
  chrome: 'Chrome',
  safari: 'Safari',
  firefox: 'Firefox',
  arc: 'Arc',
  edge: 'Edge',
  brave: 'Brave',
  opera: 'Opera',
};

export const DEFAULT_COOKIE_SOURCE: CookieSource = 'chrome';
