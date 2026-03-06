const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const providersPath = path.join(root, 'src-tauri', 'src', 'providers', 'mod.rs');

const providersFile = fs.readFileSync(providersPath, 'utf-8');

const requiredMarkers = [
  'widget_snapshot::write_widget_snapshot(provider_id, &usage)',
  'tray::handle_usage_update(&app, provider_id, usage.clone())',
  'Failed to update tray icon',
];

for (const marker of requiredMarkers) {
  if (!providersFile.includes(marker)) {
    throw new Error(`Background refresh parity marker missing: ${marker}`);
  }
}

console.log('Background refresh parity checks passed.');
