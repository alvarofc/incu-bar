const path = require('node:path');
const fs = require('node:fs');

const root = path.resolve(__dirname, '..');
const usageStorePath = path.join(root, 'src', 'stores', 'usageStore.ts');
const usageStoreFile = fs.readFileSync(usageStorePath, 'utf-8');

const requiredSelectors = [
  'useActiveProvider',
  'useEnabledProviders',
  'useAuthenticatedProviders',
];

requiredSelectors.forEach((selector) => {
  const pattern = new RegExp(
    `export const ${selector} = \\(\\) => \\{[\\s\\S]*?const providerOrder = useSettingsStore\\(\\(state\\) => state\\.providerOrder\\)`
  );
  if (!pattern.test(usageStoreFile)) {
    throw new Error(`${selector} must subscribe to providerOrder via useSettingsStore`);
  }
});

if (usageStoreFile.includes('useSettingsStore.getState().providerOrder')) {
  throw new Error('Selectors should not read providerOrder via getState.');
}

console.log('providerOrder subscription checks passed.');
