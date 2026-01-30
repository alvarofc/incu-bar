const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const appPath = path.join(root, 'src', 'App.tsx');

const appFile = fs.readFileSync(appPath, 'utf-8');

const requiredCleanup = [
  'void unlisten.then((fn) => fn()).catch(console.error);',
  'void unlistenRefresh.then((fn) => fn()).catch(console.error);',
  'void unlistenRefreshing.then((fn) => fn()).catch(console.error);',
];

for (const cleanup of requiredCleanup) {
  if (!appFile.includes(cleanup)) {
    throw new Error(`Missing unlisten cleanup with error handling: ${cleanup}`);
  }
}

console.log('Unlisten cleanup promise handling checks passed.');
