const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert');

const root = path.resolve(__dirname, '..');
const rustPath = path.join(root, 'src-tauri', 'src', 'tray', 'mod.rs');
const tsPath = path.join(root, 'src', 'lib', 'staleness.ts');

const rustFile = fs.readFileSync(rustPath, 'utf-8');
const tsFile = fs.readFileSync(tsPath, 'utf-8');

const rustMatch = rustFile.match(/STALE_THRESHOLD_SECS\s*:\s*i64\s*=\s*(\d+)/);
const tsMatch = tsFile.match(/DEFAULT_STALE_AFTER_MS\s*=\s*(\d+)/);

assert.ok(rustMatch, 'STALE_THRESHOLD_SECS not found in Rust tray module.');
assert.ok(tsMatch, 'DEFAULT_STALE_AFTER_MS not found in staleness.ts.');

const staleThresholdSecs = Number(rustMatch[1]);
const defaultStaleAfterMs = Number(tsMatch[1]);

assert.ok(Number.isFinite(staleThresholdSecs), 'STALE_THRESHOLD_SECS must be a number.');
assert.ok(Number.isFinite(defaultStaleAfterMs), 'DEFAULT_STALE_AFTER_MS must be a number.');
assert.ok(staleThresholdSecs > 0, 'STALE_THRESHOLD_SECS must be positive.');
assert.strictEqual(
  defaultStaleAfterMs,
  staleThresholdSecs * 1000,
  'DEFAULT_STALE_AFTER_MS must equal STALE_THRESHOLD_SECS * 1000.'
);

console.log('Staleness threshold sync checks passed.');
