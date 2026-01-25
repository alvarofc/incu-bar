const fs = require('node:fs');
const path = require('node:path');

const appPath = path.join(__dirname, 'src', 'App.tsx');
const appFile = fs.readFileSync(appPath, 'utf-8');

if (!appFile.includes('for (const [id, providerStatus] of Object.entries(status))')) {
  throw new Error('autoEnableAuthenticatedProviders loop not found');
}

if (!appFile.includes('if (!active)')) {
  throw new Error('Missing active guard inside autoEnableAuthenticatedProviders loop');
}

console.log('autoEnableAuthenticatedProviders guard test passed.');
