const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const popupPath = path.join(root, 'src', 'components', 'PopupWindow.tsx');
const trayPath = path.join(root, 'src-tauri', 'src', 'tray', 'mod.rs');

const popupFile = fs.readFileSync(popupPath, 'utf-8');
const trayFile = fs.readFileSync(trayPath, 'utf-8');

if (popupFile.includes("window.addEventListener('blur'")) {
  throw new Error('PopupWindow should not hide itself from a frontend blur listener.');
}

if (!trayFile.includes('WindowEvent::Focused(false)')) {
  throw new Error('Tray popup should still hide from the native window focus handler.');
}

console.log('Popup window focus handling checks passed.');
