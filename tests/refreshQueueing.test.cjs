const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const usageStorePath = path.join(root, 'src', 'stores', 'usageStore.ts');

const usageStoreFile = fs.readFileSync(usageStorePath, 'utf-8');

const requiredMarkers = [
  'let refreshAllInFlight: Promise<void> | null = null;',
  'let queuedRefreshAll = false;',
  'if (refreshAllInFlight) {',
  'queuedRefreshAll = true;',
  '} while (queuedRefreshAll);',
];

for (const marker of requiredMarkers) {
  if (!usageStoreFile.includes(marker)) {
    throw new Error(`Refresh queueing marker missing: ${marker}`);
  }
}

console.log('Refresh queueing checks passed.');
