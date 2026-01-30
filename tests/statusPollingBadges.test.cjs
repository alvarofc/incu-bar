const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const appPath = path.join(root, 'src', 'App.tsx');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const providerTabsPath = path.join(root, 'src', 'components', 'ProviderTabs.tsx');
const providerIconsPath = path.join(root, 'src', 'components', 'ProviderIcons.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');

const appFile = fs.readFileSync(appPath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');
const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const providerTabsFile = fs.readFileSync(providerTabsPath, 'utf-8');
const providerIconsFile = fs.readFileSync(providerIconsPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');

const requiredMarkers = ['poll_provider_statuses', 'status-updated', 'StatusIndicator'];

requiredMarkers.forEach((marker) => {
  if (!commandsFile.includes(marker) && !appFile.includes(marker) && !typesFile.includes(marker)) {
    throw new Error(`Status polling missing marker: ${marker}`);
  }
});

if (!appFile.includes('poll_provider_statuses')) {
  throw new Error('App missing status polling invoke.');
}

if (!providerIconsFile.includes('ProviderIconWithOverlay')) {
  throw new Error('Provider icon overlay helper missing.');
}

if (!menuCardFile.includes('ProviderIconWithOverlay')) {
  throw new Error('MenuCard missing incident badge overlay.');
}

if (!menuCardFile.includes('provider-status-section')) {
  throw new Error('MenuCard missing provider status section test id.');
}

if (!menuCardFile.includes('provider-status-link')) {
  throw new Error('MenuCard missing provider status link test id.');
}

if (!providerTabsFile.includes('ProviderIconWithOverlay')) {
  throw new Error('ProviderTabs missing incident badge overlay.');
}

if (!typesFile.includes('StatusIndicator')) {
  throw new Error('Types missing status indicator definitions.');
}

console.log('Status polling and badge checks passed.');
