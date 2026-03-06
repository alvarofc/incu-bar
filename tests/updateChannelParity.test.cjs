const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const updateChannelPath = path.join(root, 'src', 'lib', 'updateChannel.ts');
const appUpdaterPath = path.join(root, 'src', 'lib', 'appUpdater.ts');
const appPath = path.join(root, 'src', 'App.tsx');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');

const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const updateChannelFile = fs.readFileSync(updateChannelPath, 'utf-8');
const appUpdaterFile = fs.readFileSync(appUpdaterPath, 'utf-8');
const appFile = fs.readFileSync(appPath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');

const requiredMarkers = [
  'getDefaultUpdateChannelForVersion',
  'PRERELEASE_KEYWORDS',
  'autoUpdateEnabled',
  'updateChannel',
  'Update Channel',
  'UPDATE_CHANNEL_LIMITATION_NOTE',
];

requiredMarkers.forEach((marker) => {
  if (
    ![
      settingsStoreFile,
      updateChannelFile,
      appFile,
      settingsPanelFile,
    ].some((source) => source.includes(marker))
  ) {
    throw new Error(`Update channel marker missing: ${marker}`);
  }
});

if (!settingsStoreFile.includes('import.meta.env.PACKAGE_VERSION')) {
  throw new Error('Settings store missing package version lookup for update channel defaults.');
}

if (!appUpdaterFile.includes("'X-Update-Channel': channel")) {
  throw new Error('Shared updater missing updater channel header configuration.');
}

if (!appFile.includes('if (!hasHydrated || isSettingsWindow)')) {
  throw new Error('App missing hydration gate for auto updates.');
}

if (!settingsStoreFile.includes('autoUpdateEnabled') || !settingsStoreFile.includes('updateChannel')) {
  throw new Error('Settings store missing updater preference sync markers.');
}

if (!commandsFile.includes('auto_update_enabled') || !commandsFile.includes('update_channel')) {
  throw new Error('Tauri settings-updated payload missing updater preference fields.');
}

if (appFile.includes('target = `darwin-${updateChannel}`')) {
  throw new Error('Updater still overrides the Tauri target with the update channel.');
}

console.log('Update channel parity checks passed.');
