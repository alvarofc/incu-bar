const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const appPath = path.join(root, 'src', 'App.tsx');
const validationPath = path.join(root, 'src', 'lib', 'eventValidation.ts');

const appFile = fs.readFileSync(appPath, 'utf-8');
const validationFile = fs.readFileSync(validationPath, 'utf-8');

if (!validationFile.includes('usageUpdateEventSchema')) {
  throw new Error('Missing usage update schema definition in event validation module.');
}

const requiredUsageUpdate = ['Invalid usage update payload received'];

for (const snippet of requiredUsageUpdate) {
  if (!validationFile.includes(snippet)) {
    throw new Error(`Missing required usage update validation snippet: ${snippet}`);
  }
}

const requiredAppUsageUpdate = [
  "listen<UsageUpdateEvent>('usage-updated'",
  'parseUsageUpdateEvent(event.payload)',
  "listen<UsageUpdateEvent>('refresh-failed'",
];

for (const snippet of requiredAppUsageUpdate) {
  if (!appFile.includes(snippet)) {
    throw new Error(`App.tsx missing usage update validation hook: ${snippet}`);
  }
}

console.log('Usage update event validation checks passed.');
