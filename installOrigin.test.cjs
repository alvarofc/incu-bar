const path = require('node:path');
const fs = require('node:fs');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');

const appFile = fs.readFileSync(appPath, 'utf-8');
const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');

const requiredMarkers = [
  { name: 'get_install_origin', sources: [appFile, commandsFile, libFile] },
  { name: 'installOrigin', sources: [settingsStoreFile, settingsPanelFile] },
  { name: 'Installed via', sources: [settingsPanelFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Install origin marker missing: ${name}`);
  }
});

console.log('Install origin checks passed.');
