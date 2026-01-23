const fs = require('node:fs');
const path = require('node:path');

const settingsPanelPath = path.join(__dirname, 'src', 'components', 'SettingsPanel.tsx');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');

const requiredMarkers = [
  'data-testid="provider-settings-list"',
  'data-testid="provider-detail-pane"',
  'provider-order-item-',
  'provider-order-handle-',
  'provider-enable-toggle-',
  'provider-order-up-',
  'provider-order-down-',
  'data-testid="display-mode-merged"',
  'data-testid="display-mode-separate"',
  'data-testid="menu-bar-display-session"',
  'data-testid="menu-bar-display-weekly"',
  'data-testid="menu-bar-display-pace"',
  'data-testid="menu-bar-display-highest"',
  'data-testid="usage-bar-display-remaining"',
  'data-testid="usage-bar-display-used"',
  'Manual',
  '1m',
  '2m',
  '5m',
  '15m',
  'Settings',
  'Not connected',
  'On macOS, Chromium browsers prompt for keychain access. Choose "Always Allow" to add Incubar to the allow-list.',
];

requiredMarkers.forEach((marker) => {
  if (!settingsPanelFile.includes(marker)) {
    throw new Error(`SettingsPanel missing provider settings marker: ${marker}`);
  }
});

console.log('Provider settings pane checks passed.');
