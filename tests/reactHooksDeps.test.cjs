const path = require('node:path');
const fs = require('node:fs');

const root = path.resolve(__dirname, '..');
const appPath = path.join(root, 'src', 'App.tsx');

const appFile = fs.readFileSync(appPath, 'utf-8');

if (appFile.includes('eslint-disable-line react-hooks/exhaustive-deps')) {
  throw new Error('Unexpected exhaustive-deps suppression in App.tsx');
}

const initEffectPattern = /useEffect\(\(\) => \{[\s\S]*?initializeProviders\(enabledProviders\);[\s\S]*?\}, \[(.*?)\]\);/;
const match = appFile.match(initEffectPattern);

if (!match) {
  throw new Error('Initialization effect not found in App.tsx');
}

const deps = match[1];
const requiredDeps = [
  'enabledProviders',
  'initializeProviders',
  'initAutostart',
  'setInstallOrigin',
];

requiredDeps.forEach((dep) => {
  if (!deps.includes(dep)) {
    throw new Error(`Missing ${dep} in initialization effect dependencies`);
  }
});

console.log('React hooks dependency checks passed.');
