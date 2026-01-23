const fs = require('node:fs');
const path = require('node:path');

const trayPath = path.join(__dirname, 'src-tauri', 'src', 'tray', 'mod.rs');
const commandsPath = path.join(__dirname, 'src-tauri', 'src', 'commands', 'mod.rs');

const trayFile = fs.readFileSync(trayPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');

const requiredTrayMarkers = [
  'TrayStatus::Loading',
  'TrayStatus::Disabled',
  'blink_enabled',
  'draw_arc',
  'set_loading_state',
  'set_blinking_state',
];

requiredTrayMarkers.forEach((marker) => {
  if (!trayFile.includes(marker)) {
    throw new Error(`Tray icon state missing marker: ${marker}`);
  }
});

const requiredCommandMarkers = [
  'LoadingGuard',
  'set_provider_enabled',
  'set_blinking_state',
];

requiredCommandMarkers.forEach((marker) => {
  if (!commandsFile.includes(marker)) {
    throw new Error(`Commands missing tray animation marker: ${marker}`);
  }
});

console.log('Tray icon state checks passed.');
