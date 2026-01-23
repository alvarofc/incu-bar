const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const appPath = path.join(root, 'src', 'App.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');
const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml');

const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const appFile = fs.readFileSync(appPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');
const cargoFile = fs.readFileSync(cargoPath, 'utf-8');

if (!settingsStoreFile.includes("invoke('set_autostart_enabled'")) {
  throw new Error('Settings store missing set_autostart_enabled invoke.');
}

if (!settingsStoreFile.includes("invoke<boolean>('get_autostart_enabled'")) {
  throw new Error('Settings store missing get_autostart_enabled invoke.');
}

if (!settingsPanelFile.includes('Launch at Login')) {
  throw new Error('SettingsPanel missing launch at login toggle label.');
}

if (!appFile.includes('initAutostart')) {
  throw new Error('App initialization missing autostart sync.');
}

if (!commandsFile.includes('get_autostart_enabled')) {
  throw new Error('Rust commands missing get_autostart_enabled handler.');
}

if (!commandsFile.includes('set_autostart_enabled')) {
  throw new Error('Rust commands missing set_autostart_enabled handler.');
}

if (!libFile.includes('tauri_plugin_autostart::init')) {
  throw new Error('Tauri autostart plugin not initialized in lib.rs.');
}

if (!cargoFile.includes('tauri-plugin-autostart')) {
  throw new Error('Cargo.toml missing tauri-plugin-autostart dependency.');
}

console.log('Launch at login checks passed.');
