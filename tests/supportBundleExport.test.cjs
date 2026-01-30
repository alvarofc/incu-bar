const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const settingsPanelPath = path.join(root, 'src', 'components', 'SettingsPanel.tsx');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const libPath = path.join(root, 'src-tauri', 'src', 'lib.rs');

const settingsPanelFile = fs.readFileSync(settingsPanelPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const libFile = fs.readFileSync(libPath, 'utf-8');

const requiredMarkers = [
  { name: 'export_support_bundle', sources: [settingsPanelFile, commandsFile, libFile] },
  { name: 'Support Bundle', sources: [settingsPanelFile] },
  { name: 'Export Support Bundle', sources: [settingsPanelFile] },
];

requiredMarkers.forEach(({ name, sources }) => {
  if (!sources.some((source) => source.includes(name))) {
    throw new Error(`Support bundle parity marker missing: ${name}`);
  }
});

console.log('Support bundle export checks passed.');
