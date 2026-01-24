const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const popupPath = path.join(root, 'src', 'components', 'PopupWindow.tsx');

const popupFile = fs.readFileSync(popupPath, 'utf-8');

const requiredMarkers = [
  'data-testid="provider-enable-empty-state"',
  'No providers enabled',
  'Enable providers in Settings',
  'Enable Providers',
];

requiredMarkers.forEach((marker) => {
  if (!popupFile.includes(marker)) {
    throw new Error(`PopupWindow missing provider enable empty state marker: ${marker}`);
  }
});

console.log('Provider enable empty state checks passed.');
