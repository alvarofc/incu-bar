const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const appPath = path.join(root, 'src', 'App.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');
const usageStorePath = path.join(root, 'src', 'stores', 'usageStore.ts');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');

const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const appFile = fs.readFileSync(appPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');
const usageStoreFile = fs.readFileSync(usageStorePath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');

const requiredMarkers = [
  { name: 'storeUsageHistory', sources: [settingsStoreFile, typesFile, providersFile, usageStoreFile] },
  { name: 'pollProviderStatus', sources: [settingsStoreFile, typesFile, providersFile, appFile] },
  { name: 'redactPersonalInfo', sources: [settingsStoreFile, typesFile, providersFile, appFile] },
  { name: 'set_redact_personal_info', sources: [appFile, commandsFile, libFile] },
  { name: 'setStoreUsageHistory', sources: [settingsStoreFile] },
  { name: 'setPollProviderStatus', sources: [settingsStoreFile] },
  { name: 'data-testid="privacy-preferences"', sources: [settingsPanelFile] },
  { name: 'clearUsageHistory', sources: [usageStoreFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Privacy settings marker missing: ${name}`);
  }
});

if (!settingsPanelFile.includes('Privacy')) {
  throw new Error('SettingsPanel missing privacy section heading.');
}

if (!settingsPanelFile.includes('Store usage history')) {
  throw new Error('SettingsPanel missing store usage history toggle.');
}

if (!settingsPanelFile.includes('Poll provider status')) {
  throw new Error('SettingsPanel missing poll provider status toggle.');
}

if (!settingsPanelFile.includes('Incubar keeps all usage data on-device')) {
  throw new Error('SettingsPanel missing privacy messaging.');
}

if (!settingsPanelFile.includes('Status polling checks provider health pages')) {
  throw new Error('SettingsPanel missing status polling messaging.');
}

if (!settingsPanelFile.includes('Redact Personal Info in Logs')) {
  throw new Error('SettingsPanel missing log redaction toggle.');
}

console.log('Privacy preferences checks passed.');
