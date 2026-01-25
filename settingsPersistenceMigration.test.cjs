const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');

const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');

const requiredMarkers = [
  'SETTINGS_STORAGE_KEY',
  'LEGACY_SETTINGS_STORAGE_KEYS',
  'migrateLegacySettingsStorage',
  'codexbar-settings',
];

requiredMarkers.forEach((marker) => {
  if (!settingsStoreFile.includes(marker)) {
    throw new Error(`Settings migration marker missing: ${marker}`);
  }
});

if (!settingsStoreFile.includes('localStorage.setItem(SETTINGS_STORAGE_KEY')) {
  throw new Error('Settings migration missing legacy-to-IncuBar copy.');
}

if (!settingsStoreFile.includes('localStorage.removeItem(legacyKey)')) {
  throw new Error('Settings migration missing legacy cleanup.');
}

if (!settingsStoreFile.includes('isRecord(parsed)')) {
  throw new Error('Settings migration missing legacy settings validation.');
}

console.log('Settings persistence migration checks passed.');
