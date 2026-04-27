import fs from 'fs';
import { execSync } from 'child_process';
import path from 'path';
import { fileURLToPath } from 'url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(__dirname, '..');

const newVersion = process.argv[2];
if (!newVersion) {
    console.error('❌ Please provide a new version number (e.g. npm run release 4.1.32)');
    process.exit(1);
}

// validate format X.Y.Z
if (!/^\d+\.\d+\.\d+$/.test(newVersion)) {
    console.error('❌ Invalid version format. Use X.Y.Z (e.g. 4.1.32)');
    process.exit(1);
}

console.log(`🚀 Bumping version to ${newVersion}...`);

// 1. package.json
const pkgPath = path.join(rootDir, 'package.json');
const pkg = JSON.parse(fs.readFileSync(pkgPath, 'utf-8'));
pkg.version = newVersion;
fs.writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');
console.log('✅ package.json updated');

// 2. tauri.conf.json
const tauriConfPath = path.join(rootDir, 'src-tauri', 'tauri.conf.json');
const tauriConf = JSON.parse(fs.readFileSync(tauriConfPath, 'utf-8'));
tauriConf.version = newVersion;
fs.writeFileSync(tauriConfPath, JSON.stringify(tauriConf, null, 2) + '\n');
console.log('✅ tauri.conf.json updated');

// 3. Cargo.toml
const cargoTomlPath = path.join(rootDir, 'src-tauri', 'Cargo.toml');
let cargoToml = fs.readFileSync(cargoTomlPath, 'utf-8');
cargoToml = cargoToml.replace(/^version = ".*"/m, `version = "${newVersion}"`);
fs.writeFileSync(cargoTomlPath, cargoToml);
console.log('✅ Cargo.toml updated');

// 4. Git Commit & Push
console.log('\n📦 Committing and pushing to GitHub to trigger Actions...');
try {
    execSync('git add package.json src-tauri/tauri.conf.json src-tauri/Cargo.toml', { stdio: 'inherit', cwd: rootDir });
    execSync(`git commit -m "chore: release v${newVersion}"`, { stdio: 'inherit', cwd: rootDir });
    execSync(`git tag v${newVersion}`, { stdio: 'inherit', cwd: rootDir });
    
    // push branches and tags
    execSync('git push origin main', { stdio: 'inherit', cwd: rootDir });
    execSync(`git push origin v${newVersion}`, { stdio: 'inherit', cwd: rootDir });
    
    console.log(`\n🎉 Successfully released v${newVersion}!`);
    console.log(`➡️ GitHub Actions is now building the Docker image for tag v${newVersion}.`);
} catch (err) {
    console.error('❌ Failed to commit and push.', err.message);
}
