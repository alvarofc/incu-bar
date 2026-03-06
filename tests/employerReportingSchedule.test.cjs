const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const appPath = path.join(root, 'src', 'App.tsx');

const appFile = fs.readFileSync(appPath, 'utf-8');

if (!appFile.includes("reason: 'daily'")) {
  throw new Error('Daily employer reporting invoke is missing.');
}

if (!appFile.includes('24 * 60 * 60 * 1000')) {
  throw new Error('Daily employer reporting interval should run every 24 hours.');
}

if (!appFile.includes("if (isSettingsWindow) {\n      return undefined;\n    }\n\n    const unlisten = listen<UsageUpdateEvent>('usage-updated'")) {
  throw new Error('usage-updated listener should be skipped for the settings window.');
}

console.log('Employer reporting schedule and usage listener checks passed.');
