const fs = require('node:fs');
const path = require('node:path');

/**
 * Regression test for PR #15: Copilot poll_for_token timeout protection
 * 
 * This test ensures the 10-minute timeout in poll_for_token cannot be
 * accidentally removed, preventing infinite loops if GitHub's OAuth server
 * never responds with success/failure.
 * 
 * See: https://github.com/alvarofc/incu-bar/pull/15#discussion_r2751750385
 */

const root = path.resolve(__dirname, '..');
const copilotProviderPath = path.join(root, 'src-tauri', 'src', 'providers', 'copilot.rs');

if (!fs.existsSync(copilotProviderPath)) {
  throw new Error(`Copilot provider file not found at: ${copilotProviderPath}`);
}

const copilotSource = fs.readFileSync(copilotProviderPath, 'utf-8');

// Check for the 10-minute (600 second) timeout constant
if (!copilotSource.includes('from_secs(600)')) {
  throw new Error(
    'Copilot provider missing 10-minute timeout constant. ' +
    'Expected to find "from_secs(600)" in poll_for_token function.'
  );
}

// Check for the timeout error message
const expectedErrorMsg = 'Authorization timed out after 10 minutes. Please try again.';
if (!copilotSource.includes(expectedErrorMsg)) {
  throw new Error(
    `Copilot provider missing timeout error message. ` +
    `Expected to find: "${expectedErrorMsg}"`
  );
}

// Check for the timeout check logic
if (!copilotSource.includes('start_time.elapsed()') || 
    !copilotSource.includes('max_duration')) {
  throw new Error(
    'Copilot provider missing timeout check logic. ' +
    'Expected to find timeout enforcement code using start_time.elapsed() and max_duration.'
  );
}

// Check for the poll_for_token function documentation mentioning timeout
const pollForTokenMatch = copilotSource.match(/\/\/\/[^\n]*poll_for_token[^{]*{/s);
if (!pollForTokenMatch) {
  throw new Error('Could not find poll_for_token function documentation');
}

const docComment = pollForTokenMatch[0];
if (!docComment.toLowerCase().includes('timeout') && 
    !docComment.toLowerCase().includes('10 minute')) {
  console.warn(
    'Warning: poll_for_token documentation does not mention timeout behavior'
  );
}

console.log('✓ Copilot 10-minute timeout protection is present');
console.log('  - Timeout constant: 600 seconds');
console.log('  - Timeout error message: verified');
console.log('  - Timeout check logic: verified');
