const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const providerTabsPath = path.join(root, 'src', 'components', 'ProviderTabs.tsx');
const appPath = path.join(root, 'src', 'App.tsx');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');
const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const providerTabsFile = fs.readFileSync(providerTabsPath, 'utf-8');
const appFile = fs.readFileSync(appPath, 'utf-8');

const requiredMarkers = [
  { name: 'switcherShowsIcons', sources: [typesFile, providersFile, settingsStoreFile, providerTabsFile] },
  { name: 'showAllTokenAccountsInMenu', sources: [typesFile, providersFile, settingsStoreFile] },
  { name: 'debugMenuEnabled', sources: [typesFile, providersFile, settingsStoreFile, settingsPanelFile] },
  { name: 'hidePersonalInfo', sources: [typesFile, providersFile, settingsStoreFile, menuCardFile, settingsPanelFile] },
  { name: 'redactPersonalInfo', sources: [typesFile, providersFile, settingsStoreFile, settingsPanelFile, appFile] },
  { name: 'debugDisableKeychainAccess', sources: [typesFile, providersFile, settingsStoreFile, settingsPanelFile] },
  { name: 'Show Debug Settings', sources: [settingsPanelFile] },
  { name: 'Switcher shows icons', sources: [settingsPanelFile] },
  { name: 'Show all token accounts', sources: [settingsPanelFile] },
  { name: 'Hide Personal Info', sources: [settingsPanelFile] },
  { name: 'Redact Personal Info in Logs', sources: [settingsPanelFile] },
  { name: 'Disable Keychain Access', sources: [settingsPanelFile] },
  { name: 'Advanced', sources: [settingsPanelFile] },
  { name: 'Debug', sources: [settingsPanelFile] },
  { name: 'IncuBar v', sources: [settingsPanelFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Settings parity marker missing: ${name}`);
  }
});

console.log('About/Advanced/Debug/Display settings parity checks passed.');
