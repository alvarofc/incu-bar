const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const progressBarPath = path.join(root, 'src', 'components', 'ProgressBar.tsx');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');

const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const progressBarFile = fs.readFileSync(progressBarPath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');

if (!typesFile.includes("UsageBarDisplayMode")) {
  throw new Error('UsageBarDisplayMode missing from types.');
}

if (!typesFile.includes("usageBarDisplayMode")) {
  throw new Error('AppSettings missing usageBarDisplayMode.');
}

if (!providersFile.includes("usageBarDisplayMode")) {
  throw new Error('DEFAULT_SETTINGS missing usageBarDisplayMode.');
}

if (!settingsPanelFile.includes('data-testid="usage-bar-display-remaining"')) {
  throw new Error('SettingsPanel missing usage bar remaining toggle.');
}

if (!settingsPanelFile.includes('data-testid="usage-bar-display-used"')) {
  throw new Error('SettingsPanel missing usage bar used toggle.');
}

if (!menuCardFile.includes('usageBarDisplayMode')) {
  throw new Error('MenuCard missing usage bar display mode wiring.');
}

if (!progressBarFile.includes("displayMode")) {
  throw new Error('ProgressBar missing displayMode handling.');
}

console.log('Usage bar display mode checks passed.');
