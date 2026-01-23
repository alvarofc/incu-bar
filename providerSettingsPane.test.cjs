const fs = require('node:fs');
const path = require('node:path');

const settingsPanelPath = path.join(__dirname, 'src', 'components', 'SettingsPanel.tsx');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');

const requiredMarkers = [
  'data-testid="provider-settings-list"',
  'data-testid="provider-detail-pane"',
  'Settings',
  'Not connected',
];

requiredMarkers.forEach((marker) => {
  if (!settingsPanelFile.includes(marker)) {
    throw new Error(`SettingsPanel missing provider settings marker: ${marker}`);
  }
});

console.log('Provider settings pane checks passed.');
