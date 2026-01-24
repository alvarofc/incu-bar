const fs = require('node:fs');
const path = require('node:path');

const root = __dirname;
const checklistPath = path.join(root, 'scripts', 'release', 'checklist.md');
const stampScriptPath = path.join(root, 'scripts', 'release', 'stamp-version.cjs');
const packageJsonPath = path.join(root, 'package.json');
const tauriConfigPath = path.join(root, 'src-tauri', 'tauri.conf.json');
const cargoTomlPath = path.join(root, 'src-tauri', 'Cargo.toml');

const requiredFiles = [
  checklistPath,
  stampScriptPath,
  packageJsonPath,
  tauriConfigPath,
  cargoTomlPath,
];

requiredFiles.forEach((filePath) => {
  if (!fs.existsSync(filePath)) {
    throw new Error(`Release checklist parity missing file: ${filePath}`);
  }
});

const checklist = fs.readFileSync(checklistPath, 'utf-8');
const stampScript = fs.readFileSync(stampScriptPath, 'utf-8');

if (!checklist.includes('release:stamp')) {
  throw new Error('Release checklist missing release:stamp usage.');
}

if (!checklist.includes('CHANGELOG.md')) {
  throw new Error('Release checklist missing CHANGELOG.md step.');
}

const requiredStampMarkers = [
  'package.json',
  'tauri.conf.json',
  'Cargo.toml',
  'Stamped version',
];

requiredStampMarkers.forEach((marker) => {
  if (!stampScript.includes(marker)) {
    throw new Error(`Release stamping script missing marker: ${marker}`);
  }
});

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf-8'));
const cargoToml = fs.readFileSync(cargoTomlPath, 'utf-8');

if (!packageJson.version || !tauriConfig.version) {
  throw new Error('Version fields missing in package.json or tauri.conf.json.');
}

if (!cargoToml.includes('version =')) {
  throw new Error('Cargo.toml missing version field.');
}

console.log('Release checklist parity checks passed.');
