const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const providersPath = path.join(root, 'src', 'lib', 'providers.ts');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const progressBarPath = path.join(root, 'src', 'components', 'ProgressBar.tsx');
const resetCountdownPath = path.join(root, 'src', 'components', 'ResetCountdown.tsx');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const providersFile = fs.readFileSync(providersPath, 'utf-8');
const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const progressBarFile = fs.readFileSync(progressBarPath, 'utf-8');
const resetCountdownFile = fs.readFileSync(resetCountdownPath, 'utf-8');

if (!typesFile.includes('ResetTimeDisplayMode')) {
  throw new Error('ResetTimeDisplayMode missing from types.');
}

if (!typesFile.includes('resetTimeDisplayMode')) {
  throw new Error('AppSettings missing resetTimeDisplayMode.');
}

if (!providersFile.includes('resetTimeDisplayMode')) {
  throw new Error('DEFAULT_SETTINGS missing resetTimeDisplayMode.');
}

if (!settingsPanelFile.includes('data-testid="reset-time-display-relative"')) {
  throw new Error('SettingsPanel missing reset time relative toggle.');
}

if (!settingsPanelFile.includes('data-testid="reset-time-display-absolute"')) {
  throw new Error('SettingsPanel missing reset time absolute toggle.');
}

if (!menuCardFile.includes('resetTimeDisplayMode')) {
  throw new Error('MenuCard missing reset time display wiring.');
}

if (!progressBarFile.includes('resetTimeDisplayMode')) {
  throw new Error('ProgressBar missing reset time display wiring.');
}

if (!resetCountdownFile.includes('Number.isNaN(resetDate.getTime())')) {
  throw new Error('ResetCountdown missing invalid reset date guard.');
}

console.log('Reset time display mode checks passed.');
