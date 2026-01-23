const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const notificationsPath = path.join(root, 'src', 'lib', 'notifications.ts');

const appFile = fs.readFileSync(appPath, 'utf-8');
const notificationsFile = fs.readFileSync(notificationsPath, 'utf-8');

if (!notificationsFile.includes('evaluateStaleUsageNotifications')) {
  throw new Error('Stale usage notification evaluator missing.');
}

if (!notificationsFile.includes('formatDistanceToNow')) {
  throw new Error('Stale usage notifications need relative time formatting.');
}

if (!appFile.includes('evaluateStaleUsageNotifications')) {
  throw new Error('App missing stale usage notification evaluation.');
}

if (!appFile.includes('sendNotification')) {
  throw new Error('App missing notification dispatch wiring.');
}

console.log('Stale usage notification checks passed.');
