const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const notificationsPath = path.join(root, 'src', 'lib', 'notifications.ts');

const appFile = fs.readFileSync(appPath, 'utf-8');
const notificationsFile = fs.readFileSync(notificationsPath, 'utf-8');

if (!notificationsFile.includes('evaluateRefreshFailureNotifications')) {
  throw new Error('Refresh failure notification evaluator missing.');
}

if (!appFile.includes('refresh-failed')) {
  throw new Error('App missing refresh failure event listener.');
}

if (!appFile.includes('evaluateRefreshFailureNotifications')) {
  throw new Error('App missing refresh failure notification evaluation.');
}

if (!appFile.includes('refreshIntervalSeconds')) {
  throw new Error('App missing refresh interval wiring for failure resets.');
}

if (!appFile.includes('sendNotification')) {
  throw new Error('App missing notification dispatch wiring.');
}

console.log('Refresh failure notification checks passed.');
