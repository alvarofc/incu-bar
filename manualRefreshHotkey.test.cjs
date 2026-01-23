const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const trayPath = path.join(root, 'src-tauri', 'src', 'tray', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const appPath = path.join(root, 'src', 'App.tsx');

const trayFile = fs.readFileSync(trayPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const appFile = fs.readFileSync(appPath, 'utf-8');

if (!trayFile.includes('TRAY_REFRESH_MENU_ID')) {
  throw new Error('Tray refresh menu id missing.');
}

if (!trayFile.includes('MenuItemBuilder') || !trayFile.includes('refresh-requested')) {
  throw new Error('Tray refresh menu emit missing.');
}

if (!libFile.includes('global_shortcut') || !libFile.includes('CmdOrCtrl+R')) {
  throw new Error('Global shortcut registration missing.');
}

if (!commandsFile.includes('refreshing-provider')) {
  throw new Error('Refresh loading event emission missing.');
}

if (!appFile.includes('refresh-requested')) {
  throw new Error('App refresh listener missing.');
}

console.log('Manual refresh menu + hotkey checks passed.');
