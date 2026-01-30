const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..');
const workflowPath = path.join(root, '.github', 'workflows', 'release-cli.yml');
const tasksPath = path.join(root, 'tasks.yaml');

if (!fs.existsSync(workflowPath)) {
  throw new Error('Linux CLI release workflow missing.');
}

const workflowFile = fs.readFileSync(workflowPath, 'utf-8');
const tasksFile = fs.readFileSync(tasksPath, 'utf-8');

if (!workflowFile.includes('Release Linux CLI')) {
  throw new Error('Linux CLI workflow name missing.');
}

if (!workflowFile.includes('cargo build --release --bin codexbar')) {
  throw new Error('Linux CLI workflow missing codexbar build step.');
}

if (!workflowFile.includes('CodexBarCLI-${TAG}-linux-${ARCH}.tar.gz')) {
  throw new Error('Linux CLI workflow missing asset name.');
}

if (!tasksFile.includes('Linux CLI build pipeline')) {
  throw new Error('Tasks list missing Linux CLI pipeline entry.');
}

console.log('Linux CLI pipeline checks passed.');
