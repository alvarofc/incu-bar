const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const trayPath = path.join(root, 'src-tauri', 'src', 'tray', 'mod.rs');

const trayFile = fs.readFileSync(trayPath, 'utf-8');
const popupSectionStart = trayFile.indexOf('pub fn create_popup_window');
const settingsSectionStart = trayFile.indexOf('pub fn create_settings_window');

if (popupSectionStart === -1 || settingsSectionStart === -1) {
  throw new Error('Unable to isolate popup window section in tray module.');
}

const popupSection = trayFile.slice(popupSectionStart, settingsSectionStart);

const requiredMarkers = [
  'POPUP_FOCUS_GRACE_MS',
  'begin_popup_focus_guard()',
  'popup_focus_guard_active()',
  'Ignoring popup blur during focus grace window',
  '.focused(false);',
  'centering popup instead',
  'window.center()',
];

for (const marker of requiredMarkers) {
  if (!trayFile.includes(marker)) {
    throw new Error(`Tray popup focus guard marker missing: ${marker}`);
  }
}

if (popupSection.includes('window.open_devtools();')) {
  throw new Error('Tray popup should not auto-open devtools.');
}

console.log('Tray popup visibility guard checks passed.');
