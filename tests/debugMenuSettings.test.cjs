const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
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
  { name: 'debugFileLogging', sources: [settingsStoreFile, appFile, commandsFile] },
  { name: 'debugKeepCliSessionsAlive', sources: [settingsStoreFile, appFile, commandsFile] },
  { name: 'debugRandomBlink', sources: [settingsStoreFile, appFile, commandsFile] },
  { name: 'data-testid="debug-settings"', sources: [settingsPanelFile] },
  { name: 'set_debug_file_logging', sources: [commandsFile, libFile, appFile] },
  { name: 'set_debug_keep_cli_sessions_alive', sources: [commandsFile, libFile, appFile] },
  { name: 'set_debug_random_blink', sources: [commandsFile, libFile, appFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Debug settings marker missing: ${name}`);
  }
});

if (!settingsPanelFile.includes('File Logging')) {
  throw new Error('SettingsPanel missing File Logging toggle.');
}

if (!settingsPanelFile.includes('Keep CLI Sessions Alive')) {
  throw new Error('SettingsPanel missing Keep CLI Sessions Alive toggle.');
}

if (!settingsPanelFile.includes('Random Blink')) {
  throw new Error('SettingsPanel missing Random Blink toggle.');
}

console.log('Debug settings checks passed.');
