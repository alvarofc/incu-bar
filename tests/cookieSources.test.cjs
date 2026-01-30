const fs = require('node:fs');
const path = require('node:path');

const expectedSources = [
  'chrome',
  'safari',
  'firefox',
  'arc',
  'edge',
  'brave',
  'opera',
];

const root = path.resolve(__dirname, '..');
const cookieSourcesPath = path.join(root, 'src', 'lib', 'cookieSources.ts');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');

const cookieSourcesFile = fs.readFileSync(cookieSourcesPath, 'utf-8');
const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');

expectedSources.forEach((source) => {
  if (!cookieSourcesFile.includes(`'${source}'`)) {
    throw new Error(`cookieSources.ts missing source: ${source}`);
  }

  if (!cookieSourcesFile.includes(`${source}:`)) {
    throw new Error(`cookieSources.ts missing label for source: ${source}`);
  }
});

const requiredCommands = [
  'import_cursor_browser_cookies_from_source',
  'import_factory_browser_cookies_from_source',
  'import_augment_browser_cookies_from_source',
  'import_kimi_browser_cookies_from_source',
  'import_minimax_browser_cookies_from_source',
  'import_amp_browser_cookies_from_source',
  'import_opencode_browser_cookies_from_source',
  'store_amp_cookies',
  'store_opencode_cookies',
];

requiredCommands.forEach((command) => {
  if (!settingsPanelFile.includes(command)) {
    throw new Error(`SettingsPanel missing command: ${command}`);
  }
});

console.log('Cookie source selection checks passed.');
