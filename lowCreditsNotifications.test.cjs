const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const notificationsPath = path.join(root, 'src', 'lib', 'notifications.ts');

const appFile = fs.readFileSync(appPath, 'utf-8');
const notificationsFile = fs.readFileSync(notificationsPath, 'utf-8');

if (!notificationsFile.includes('CREDIT_REMAINING_THRESHOLDS')) {
  throw new Error('Credit remaining thresholds are missing.');
}

if (!notificationsFile.includes('evaluateCreditsNotifications')) {
  throw new Error('Credits notification evaluator missing.');
}

if (!appFile.includes('evaluateCreditsNotifications')) {
  throw new Error('App missing credits notification evaluation.');
}

if (!appFile.includes('sendNotification')) {
  throw new Error('App missing notification dispatch wiring.');
}

console.log('Low credits notification checks passed.');
