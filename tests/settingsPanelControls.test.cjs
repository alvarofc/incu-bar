const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');

const requiredMarkers = [
  { name: 'data-testid="settings-close-button"', sources: [settingsPanelFile] },
  { name: 'data-testid="manual-update-button"', sources: [settingsPanelFile] },
  { name: 'handleCloseSettings', sources: [settingsPanelFile] },
  { name: 'handleCheckForUpdates', sources: [settingsPanelFile] },
  { name: 'getCurrentWindow', sources: [settingsPanelFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Settings panel control marker missing: ${name}`);
  }
});

if (!settingsPanelFile.includes('data-testid="settings-close-button"') || 
    !settingsPanelFile.match(/>\s*Close\s*<\/button>/)) {
  throw new Error('SettingsPanel missing Close button with expected text.');
}

if (!settingsPanelFile.includes('onClick={handleCloseSettings}')) {
  throw new Error('SettingsPanel missing close button click handler.');
}

if (!settingsPanelFile.includes('onClick={handleCheckForUpdates}')) {
  throw new Error('SettingsPanel missing manual update button click handler.');
}

if (!settingsPanelFile.includes('getCurrentWindow()') || 
    !settingsPanelFile.includes('await window.close()')) {
  throw new Error('SettingsPanel missing window close implementation in close handler.');
}

if (!settingsPanelFile.includes('check(') ||
    !settingsPanelFile.includes('headers') ||
    !settingsPanelFile.includes('X-Update-Channel') ||
    !settingsPanelFile.includes('updateChannel')) {
  throw new Error('SettingsPanel missing update check with channel header.');
}

console.log('Settings panel controls checks passed.');
