const fs = require('node:fs');
const path = require('node:path');

const root = path.resolve(__dirname, '..', '..');
const versionArg = process.argv[2];

if (!versionArg) {
  console.error('Missing version. Usage: npm run release:stamp -- <version>');
  process.exit(1);
}

const version = versionArg.startsWith('v') ? versionArg.slice(1) : versionArg;

const packageJsonPath = path.join(root, 'package.json');
const tauriConfigPath = path.join(root, 'src-tauri', 'tauri.conf.json');
const cargoTomlPath = path.join(root, 'src-tauri', 'Cargo.toml');

const packageJson = JSON.parse(fs.readFileSync(packageJsonPath, 'utf-8'));
packageJson.version = version;
fs.writeFileSync(packageJsonPath, `${JSON.stringify(packageJson, null, 2)}\n`);

const tauriConfig = JSON.parse(fs.readFileSync(tauriConfigPath, 'utf-8'));
tauriConfig.version = version;
fs.writeFileSync(tauriConfigPath, `${JSON.stringify(tauriConfig, null, 2)}\n`);

const cargoLines = fs.readFileSync(cargoTomlPath, 'utf-8').split('\n');
let inPackage = false;
let cargoUpdated = false;

const updatedCargo = cargoLines.map((line) => {
  if (line.trim().startsWith('[')) {
    inPackage = line.trim() === '[package]';
  }
  if (inPackage && line.trim().startsWith('version = ')) {
    cargoUpdated = true;
    return `version = "${version}"`;
  }
  return line;
});

if (!cargoUpdated) {
  console.error('Could not update version in Cargo.toml.');
  process.exit(1);
}

fs.writeFileSync(cargoTomlPath, `${updatedCargo.join('\n')}\n`);

console.log(`Stamped version ${version} in package.json, tauri.conf.json, and Cargo.toml.`);
