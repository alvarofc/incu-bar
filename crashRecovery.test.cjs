const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const mainPath = path.join(root, 'src', 'main.tsx');
const crashRecoveryPath = path.join(root, 'src', 'lib', 'crashRecovery.ts');
const settingsStorePath = path.join(root, 'src', 'stores', 'settingsStore.ts');
const usageStorePath = path.join(root, 'src', 'stores', 'usageStore.ts');

const appFile = fs.readFileSync(appPath, 'utf-8');
const mainFile = fs.readFileSync(mainPath, 'utf-8');
const crashRecoveryFile = fs.readFileSync(crashRecoveryPath, 'utf-8');
const settingsStoreFile = fs.readFileSync(settingsStorePath, 'utf-8');
const usageStoreFile = fs.readFileSync(usageStorePath, 'utf-8');

if (!mainFile.includes('CRASH_RECOVERY_KEY')) {
  throw new Error('Error boundary missing crash recovery flag storage.');
}

if (!appFile.includes('restoreSafeStateAfterCrash')) {
  throw new Error('App initialization missing crash recovery restore.');
}

if (!crashRecoveryFile.includes('restoreSafeStateAfterCrash')) {
  throw new Error('Crash recovery module missing restore helper.');
}

if (!crashRecoveryFile.includes('safeResetUsageStore')) {
  throw new Error('Crash recovery missing usage reset.');
}

if (!crashRecoveryFile.includes('safeResetSettingsStore')) {
  throw new Error('Crash recovery missing settings reset.');
}

if (!settingsStoreFile.includes('setCrashRecoveryAt')) {
  throw new Error('Settings store missing crash recovery timestamp setter.');
}

if (!usageStoreFile.includes('resetState')) {
  throw new Error('Usage store missing resetState action.');
}

console.log('Crash recovery checks passed.');
