const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const appPath = path.join(root, 'src', 'App.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');

const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const appFile = fs.readFileSync(appPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');

const requiredMarkers = [
  { name: 'notifySessionUsage', sources: [settingsStoreFile, appFile] },
  { name: 'notifyCreditsLow', sources: [settingsStoreFile, appFile] },
  { name: 'notifyRefreshFailure', sources: [settingsStoreFile, appFile] },
  { name: 'notifyStaleUsage', sources: [settingsStoreFile, appFile] },
  { name: 'data-testid="notification-preferences"', sources: [settingsPanelFile] },
  { name: 'data-testid="notification-test-button"', sources: [settingsPanelFile] },
  { name: 'send_test_notification', sources: [commandsFile, libFile, settingsPanelFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Notification preference marker missing: ${name}`);
  }
});

if (!settingsPanelFile.includes('Session usage alerts')) {
  throw new Error('SettingsPanel missing session usage alert toggle.');
}

if (!settingsPanelFile.includes('Credits low alerts')) {
  throw new Error('SettingsPanel missing credits low alert toggle.');
}

if (!settingsPanelFile.includes('Refresh failure alerts')) {
  throw new Error('SettingsPanel missing refresh failure alert toggle.');
}

if (!settingsPanelFile.includes('Stale usage alerts')) {
  throw new Error('SettingsPanel missing stale usage alert toggle.');
}

if (!settingsPanelFile.includes('Send test notification')) {
  throw new Error('SettingsPanel missing test notification action.');
}

if (!commandsFile.includes('send_test_notification')) {
  throw new Error('Rust commands missing test notification handler.');
}

if (!libFile.includes('send_test_notification')) {
  throw new Error('Tauri invoke handler missing send_test_notification.');
}

if (!appFile.includes('notifySessionUsage')) {
  throw new Error('App missing session notification preference wiring.');
}

if (!appFile.includes('notifyCreditsLow')) {
  throw new Error('App missing credits notification preference wiring.');
}

if (!appFile.includes('notifyRefreshFailure')) {
  throw new Error('App missing refresh failure notification preference wiring.');
}

if (!appFile.includes('notifyStaleUsage')) {
  throw new Error('App missing stale usage notification preference wiring.');
}

console.log('Notification preference checks passed.');
