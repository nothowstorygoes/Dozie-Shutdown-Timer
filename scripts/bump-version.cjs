const fs = require('fs');
const ver = process.argv[2];

// tauri.conf.json
const conf = JSON.parse(fs.readFileSync('src-tauri/tauri.conf.json', 'utf8'));
conf.version = ver;
fs.writeFileSync('src-tauri/tauri.conf.json', JSON.stringify(conf, null, 2));

// Cargo.toml (regex sul campo version nel primo blocco [package])
let cargo = fs.readFileSync('src-tauri/Cargo.toml', 'utf8');
cargo = cargo.replace(/^version = ".*?"/m, `version = "${ver}"`);
fs.writeFileSync('src-tauri/Cargo.toml', cargo);

console.log('Bumped to', ver);