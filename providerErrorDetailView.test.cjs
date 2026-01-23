const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const popupPath = path.join(root, 'src', 'components', 'PopupWindow.tsx');

const popupFile = fs.readFileSync(popupPath, 'utf-8');

const requiredMarkers = [
  'data-testid="provider-error-detail"',
  'needs attention',
  'We could not refresh usage.',
  'Try Again',
  'Open Settings',
];

requiredMarkers.forEach((marker) => {
  if (!popupFile.includes(marker)) {
    throw new Error(`PopupWindow missing provider error detail marker: ${marker}`);
  }
});

console.log('Provider error detail view checks passed.');
