const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const appPath = path.join(root, 'src', 'App.tsx');

const appFile = fs.readFileSync(appPath, 'utf-8');

const requiredMarkers = [
  "invoke<Partial<Record<ProviderId, UsageSnapshot>>>('get_all_usage')",
  "console.error('Failed to sync cached provider usage:'",
  'window.addEventListener(\'focus\', handleFocus)',
  "document.addEventListener('visibilitychange', handleVisibilityChange)",
  'window.setInterval(() => {',
];

for (const marker of requiredMarkers) {
  if (!appFile.includes(marker)) {
    throw new Error(`Cached usage sync marker missing: ${marker}`);
  }
}

console.log('Cached usage sync checks passed.');
