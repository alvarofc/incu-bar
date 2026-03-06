const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const appUpdaterPath = path.join(root, 'src', 'lib', 'appUpdater.ts');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');
const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const appUpdaterFile = fs.readFileSync(appUpdaterPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');
const cargoFile = fs.readFileSync(cargoPath, 'utf-8');

// Manual update button checks
if (!settingsPanelFile.includes('data-testid="manual-update-button"')) {
  throw new Error('SettingsPanel missing manual update button testid.');
}

// Check for the button label logic that includes "Check for updates"
if (!settingsPanelFile.includes("? 'Check for updates'") && 
    !settingsPanelFile.includes("'Check for updates'")) {
  throw new Error('SettingsPanel missing "Check for updates" button text.');
}

// Update status handling checks
if (!settingsPanelFile.includes('data-testid="update-status-message"')) {
  throw new Error('SettingsPanel missing update status message testid.');
}

if (!settingsPanelFile.includes("updateStatus === 'checking'") || 
    !settingsPanelFile.includes("updateStatus === 'installing'")) {
  throw new Error('SettingsPanel missing update status handling logic.');
}

if (!settingsPanelFile.includes('Checking...') || !settingsPanelFile.includes('Updating...')) {
  throw new Error('SettingsPanel missing update button state labels.');
}

// Shared updater integration checks
if (!settingsPanelFile.includes("import { runAppUpdate } from '../lib/appUpdater'")) {
  throw new Error('SettingsPanel missing shared app updater import.');
}

if (!settingsPanelFile.includes('await runAppUpdate(')) {
  throw new Error('SettingsPanel missing shared updater invocation.');
}

if (!appUpdaterFile.includes("import { check } from '@tauri-apps/plugin-updater'")) {
  throw new Error('Shared updater missing updater check import.');
}

if (!appUpdaterFile.includes("import { relaunch } from '@tauri-apps/plugin-process'")) {
  throw new Error('Shared updater missing process relaunch import.');
}

if (!appUpdaterFile.includes('await check(')) {
  throw new Error('Shared updater missing updater check invocation.');
}

if (!appUpdaterFile.includes('await update.downloadAndInstall()')) {
  throw new Error('Shared updater missing update download and install logic.');
}

if (!appUpdaterFile.includes('await relaunch()')) {
  throw new Error('Shared updater missing relaunch invocation.');
}

// Updater plugin backend checks
if (!libFile.includes('tauri_plugin_updater::Builder::new().build()')) {
  throw new Error('Tauri updater plugin not initialized in lib.rs.');
}

if (!cargoFile.includes('tauri-plugin-updater')) {
  throw new Error('Cargo.toml missing tauri-plugin-updater dependency.');
}

// Close button checks
if (!settingsPanelFile.includes('data-testid="settings-close-button"')) {
  throw new Error('SettingsPanel missing close button testid.');
}

// Check for close button with testid context to ensure it's the correct button
const closeButtonPattern = /data-testid="settings-close-button"[\s\S]{0,200}Close[\s\S]{0,100}<\/button>/;
if (!closeButtonPattern.test(settingsPanelFile)) {
  throw new Error('SettingsPanel missing "Close" button text near settings-close-button testid.');
}

if (!settingsPanelFile.includes("import { getCurrentWindow } from '@tauri-apps/api/window'")) {
  throw new Error('SettingsPanel missing getCurrentWindow import.');
}

if (!settingsPanelFile.includes('await window.close()')) {
  throw new Error('SettingsPanel missing window close invocation.');
}

console.log('Manual update and close controls checks passed.');
