const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const storagePath = path.join(root, 'src-tauri', 'src', 'storage', 'widget_snapshot.rs');
const storageModPath = path.join(root, 'src-tauri', 'src', 'storage', 'mod.rs');
const commandsPath = path.join(root, 'src-tauri', 'src', 'commands', 'mod.rs');
const tasksPath = path.join(root, 'tasks.yaml');

const storageFile = fs.readFileSync(storagePath, 'utf-8');
const storageModFile = fs.readFileSync(storageModPath, 'utf-8');
const commandsFile = fs.readFileSync(commandsPath, 'utf-8');
const tasksFile = fs.readFileSync(tasksPath, 'utf-8');

if (!storageFile.includes('WIDGET_SNAPSHOT_FILENAME')) {
  throw new Error('Widget snapshot filename missing.');
}

if (!storageFile.includes('write_widget_snapshot')) {
  throw new Error('Widget snapshot writer missing.');
}

if (!storageModFile.includes('widget_snapshot')) {
  throw new Error('Widget snapshot module not exported.');
}

if (!commandsFile.includes('write_widget_snapshot')) {
  throw new Error('Widget snapshot writer not wired into commands.');
}

if (!tasksFile.includes('widget snapshot')) {
  throw new Error('Tasks list missing widget snapshot entry.');
}

console.log('Widget snapshot pipeline checks passed.');
