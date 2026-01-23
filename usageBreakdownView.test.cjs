const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');
const usageStorePath = path.join(root, 'src', 'stores', 'usageStore.ts');
const typesPath = path.join(root, 'src', 'lib', 'types.ts');

const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');
const usageStoreFile = fs.readFileSync(usageStorePath, 'utf-8');
const typesFile = fs.readFileSync(typesPath, 'utf-8');

if (!typesFile.includes('UsageHistoryPoint')) {
  throw new Error('UsageHistoryPoint missing from types.');
}

if (!typesFile.includes('usageHistory')) {
  throw new Error('ProviderState missing usageHistory field.');
}

if (!usageStoreFile.includes('usageHistory')) {
  throw new Error('Usage store missing usageHistory updates.');
}

if (!menuCardFile.includes('Usage Breakdown')) {
  throw new Error('MenuCard missing usage breakdown section.');
}

if (!menuCardFile.includes('Cost History')) {
  throw new Error('MenuCard missing cost history section.');
}

if (!menuCardFile.includes('Credits History')) {
  throw new Error('MenuCard missing credits history section.');
}

if (!menuCardFile.includes('On pace')) {
  throw new Error('MenuCard missing pace detail text.');
}

console.log('Usage breakdown view checks passed.');
