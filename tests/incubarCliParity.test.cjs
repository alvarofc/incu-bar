const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const cliPath = path.join(root, 'src-tauri', 'src', 'bin', 'incubar.rs');

if (!fs.existsSync(cliPath)) {
  throw new Error('incubar CLI binary missing.');
}

const cliFile = fs.readFileSync(cliPath, 'utf-8');
const requiredMarkers = [
  'run_status',
  'run_cost',
  'ProviderRegistry::new',
  'load_cost_snapshot',
  'status',
  'cost',
  'usage',
  'status_indicator_string',
  'StatusPayload',
  'CostPayload',
  'status_page_url',
];

requiredMarkers.forEach((marker) => {
  if (!cliFile.includes(marker)) {
    throw new Error(`incubar CLI missing marker: ${marker}`);
  }
});

console.log('incubar CLI parity checks passed.');
