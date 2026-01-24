const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const popupPath = path.join(root, 'src', 'components', 'PopupWindow.tsx');
const providerTabsPath = path.join(root, 'src', 'components', 'ProviderTabs.tsx');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');

const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const popupFile = fs.readFileSync(popupPath, 'utf-8');
const providerTabsFile = fs.readFileSync(providerTabsPath, 'utf-8');
const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');

const extractSection = (source, startMarker, endMarker) => {
  const startIndex = source.indexOf(startMarker);
  if (startIndex === -1) {
    throw new Error(`Missing section marker: ${startMarker}`);
  }
  const slice = source.slice(startIndex);
  const endIndex = slice.indexOf(endMarker);
  if (endIndex === -1) {
    throw new Error(`Missing section marker: ${endMarker}`);
  }
  return slice.slice(0, endIndex);
};

const providerIdSection = extractSection(typesFile, 'export type ProviderId', '// Rate window');
const providerIds = (providerIdSection.match(/'([^']+)'/g) ?? []).map((id) => id.replace(/'/g, ''));

const providersSection = extractSection(
  providersFile,
  'export const PROVIDERS',
  'export const DEFAULT_ENABLED_PROVIDERS'
);
const providerKeys = (providersSection.match(/^\s*([a-z0-9_]+):\s*{/gm) ?? []).map(
  (line) => line.trim().split(':')[0]
);

providerIds.forEach((id) => {
  if (!providerKeys.includes(id)) {
    throw new Error(`PROVIDERS missing provider metadata: ${id}`);
  }
});

providerKeys.forEach((id) => {
  if (!providerIds.includes(id)) {
    throw new Error(`ProviderId missing provider type: ${id}`);
  }
});

const orderMatch = providersFile.match(
  /export const DEFAULT_PROVIDER_ORDER:[^=]*=\s*\[([\s\S]*?)\];/m
);
if (!orderMatch) {
  throw new Error('DEFAULT_PROVIDER_ORDER missing.');
}
const orderIds = (orderMatch[1].match(/'([^']+)'/g) ?? []).map((id) => id.replace(/'/g, ''));

providerIds.forEach((id) => {
  if (!orderIds.includes(id)) {
    throw new Error(`DEFAULT_PROVIDER_ORDER missing provider: ${id}`);
  }
});

orderIds.forEach((id) => {
  if (!providerIds.includes(id)) {
    throw new Error(`DEFAULT_PROVIDER_ORDER includes unknown provider: ${id}`);
  }
});

const enabledMatch = providersFile.match(
  /export const DEFAULT_ENABLED_PROVIDERS:[^=]*=\s*\[([\s\S]*?)\];/m
);
if (!enabledMatch) {
  throw new Error('DEFAULT_ENABLED_PROVIDERS missing.');
}
const enabledIds = (enabledMatch[1].match(/'([^']+)'/g) ?? []).map((id) => id.replace(/'/g, ''));

enabledIds.forEach((id) => {
  if (!providerIds.includes(id)) {
    throw new Error(`DEFAULT_ENABLED_PROVIDERS includes unknown provider: ${id}`);
  }
});

const settingsMarkers = [
  'Show Credits',
  'Show Cost',
  'Show Extra Usage',
  'Notifications',
  'Launch at Login',
  'Update Channel',
];

settingsMarkers.forEach((marker) => {
  if (!settingsPanelFile.includes(marker)) {
    throw new Error(`SettingsPanel missing settings marker: ${marker}`);
  }
});

const popupMarkers = ['Loading…', 'Refresh', 'Settings', 'Welcome to IncuBar'];
popupMarkers.forEach((marker) => {
  if (!popupFile.includes(marker)) {
    throw new Error(`PopupWindow missing UI marker: ${marker}`);
  }
});

const tabMarkers = ['role="tablist"', 'role="tab"', 'aria-controls={`panel-'];
tabMarkers.forEach((marker) => {
  if (!providerTabsFile.includes(marker)) {
    throw new Error(`ProviderTabs missing UI marker: ${marker}`);
  }
});

const menuCardMarkers = [
  'data-testid="provider-freshness-line"',
  'data-testid="provider-status-line"',
  'data-testid="menu-bar-display-text"',
];
menuCardMarkers.forEach((marker) => {
  if (!menuCardFile.includes(marker)) {
    throw new Error(`MenuCard missing UI marker: ${marker}`);
  }
});

console.log('Providers, settings, and UI parity checks passed.');
