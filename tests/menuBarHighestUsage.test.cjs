const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');

const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');

if (!typesFile.includes("'highest'")) {
  throw new Error('MenuBarDisplayMode missing highest mode.');
}

if (!settingsPanelFile.includes('data-testid="menu-bar-display-highest"')) {
  throw new Error('SettingsPanel missing highest usage display toggle.');
}

if (!menuCardFile.includes("menuBarDisplayMode === 'highest'")) {
  throw new Error('MenuCard missing highest usage selection logic.');
}

if (!menuCardFile.includes('current.window.usedPercent > highest.window.usedPercent')) {
  throw new Error('MenuCard missing highest usage aggregation comparison.');
}

console.log('Highest usage display mode checks passed.');
