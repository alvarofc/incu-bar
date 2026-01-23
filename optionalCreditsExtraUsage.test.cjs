const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');

if (!typesFile.includes('showExtraUsage')) {
  throw new Error('AppSettings missing showExtraUsage.');
}

if (!providersFile.includes('showExtraUsage')) {
  throw new Error('DEFAULT_SETTINGS missing showExtraUsage.');
}

if (!settingsPanelFile.includes('Show Extra Usage')) {
  throw new Error('SettingsPanel missing Show Extra Usage toggle.');
}

if (!menuCardFile.includes('showExtraUsage')) {
  throw new Error('MenuCard missing showExtraUsage toggle wiring.');
}

console.log('Optional credits and extra usage checks passed.');
