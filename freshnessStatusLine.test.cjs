const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const menuCardPath = path.join(root, 'src', 'components', 'MenuCard.tsx');

const menuCardFile = fs.readFileSync(menuCardPath, 'utf-8');

if (!menuCardFile.includes('provider-freshness-line')) {
  throw new Error('MenuCard missing freshness line test id.');
}

if (!menuCardFile.includes('provider-status-line')) {
  throw new Error('MenuCard missing status line test id.');
}

if (!menuCardFile.includes('getStaleAfterMs') || !menuCardFile.includes('isTimestampStale')) {
  throw new Error('MenuCard missing staleness helpers for freshness timing.');
}

if (!menuCardFile.includes('Updated {lastUpdatedText}')) {
  throw new Error('MenuCard missing updated timestamp label.');
}

if (!menuCardFile.includes('Status {statusLine}')) {
  throw new Error('MenuCard missing status line output.');
}

console.log('Freshness timing and status line checks passed.');
