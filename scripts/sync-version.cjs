#!/usr/bin/env node
/**
 * Sync version from package.json to Cargo.toml and tauri.conf.json.
 * Called by semantic-release during the prepare step.
 */
const fs = require('fs');
const path = require('path');

const root = path.resolve(__dirname, '..');

// Read version from package.json
const pkgPath = path.join(root, 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf8'));
const version = pkg.version;

if (!version) {
  console.error('No version found in package.json');
  process.exit(1);
}

console.log(`Syncing version ${version} to Cargo.toml and tauri.conf.json ...`);

// Update Cargo.toml
const cargoPath = path.join(root, 'src-tauri', 'Cargo.toml');
let cargoContent = fs.readFileSync(cargoPath, 'utf8');
cargoContent = cargoContent.replace(
  /^version = ".+"/m,
  `version = "${version}"`
);
fs.writeFileSync(cargoPath, cargoContent);
console.log(`  → src-tauri/Cargo.toml updated`);

// Update tauri.conf.json
const tauriConfPath = path.join(root, 'src-tauri', 'tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf8'));
tauriConf.version = version;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log(`  → src-tauri/tauri.conf.json updated`);

console.log('Version sync complete.');
