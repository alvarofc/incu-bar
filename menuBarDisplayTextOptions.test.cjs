const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');
const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');

if (!typesFile.includes('MenuBarDisplayTextMode')) {
  throw new Error('MenuBarDisplayTextMode missing from types.');
}

if (!typesFile.includes('menuBarDisplayTextEnabled')) {
  throw new Error('AppSettings missing menuBarDisplayTextEnabled.');
}

if (!typesFile.includes('menuBarDisplayTextMode')) {
  throw new Error('AppSettings missing menuBarDisplayTextMode.');
}

if (!providersFile.includes('menuBarDisplayTextEnabled')) {
  throw new Error('DEFAULT_SETTINGS missing menuBarDisplayTextEnabled.');
}

if (!providersFile.includes('menuBarDisplayTextMode')) {
  throw new Error('DEFAULT_SETTINGS missing menuBarDisplayTextMode.');
}

if (!settingsStoreFile.includes('menuBarShowsBrandIconWithPercent')) {
  throw new Error('Settings migration missing menu bar text legacy key.');
}

if (!settingsPanelFile.includes('Menu bar shows text')) {
  throw new Error('SettingsPanel missing menu bar text toggle label.');
}

if (!settingsPanelFile.includes('data-testid="menu-bar-text-toggle"')) {
  throw new Error('SettingsPanel missing menu bar text toggle test id.');
}

if (!settingsPanelFile.includes('data-testid="menu-bar-text-percent"')) {
  throw new Error('SettingsPanel missing menu bar text percent toggle.');
}

if (!settingsPanelFile.includes('data-testid="menu-bar-text-pace"')) {
  throw new Error('SettingsPanel missing menu bar text pace toggle.');
}

if (!settingsPanelFile.includes('data-testid="menu-bar-text-both"')) {
  throw new Error('SettingsPanel missing menu bar text both toggle.');
}

if (!menuCardFile.includes('menuBarDisplayTextMode')) {
  throw new Error('MenuCard missing menu bar text mode wiring.');
}

if (!menuCardFile.includes('menuBarDisplayTextEnabled')) {
  throw new Error('MenuCard missing menu bar text enabled wiring.');
}

if (!menuCardFile.includes('menu-bar-display-text')) {
  throw new Error('MenuCard missing menu bar display text label.');
}

console.log('Menu bar display text options checks passed.');
