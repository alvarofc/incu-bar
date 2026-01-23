const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const notificationsPath = path.join(root, 'src', 'lib', 'notifications.ts');

const appFile = fs.readFileSync(appPath, 'utf-8');
const notificationsFile = fs.readFileSync(notificationsPath, 'utf-8');

if (!notificationsFile.includes('SESSION_QUOTA_THRESHOLDS')) {
  throw new Error('Session quota thresholds are missing.');
}

if (!notificationsFile.includes('evaluateSessionNotifications')) {
  throw new Error('Session notification evaluator missing.');
}

if (!appFile.includes('evaluateSessionNotifications')) {
  throw new Error('App missing session notification evaluation.');
}

if (!appFile.includes('sendNotification')) {
  throw new Error('App missing notification dispatch wiring.');
}

console.log('Session quota notification checks passed.');
