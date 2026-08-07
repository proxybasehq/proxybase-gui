import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, "..");

// Accept "0.1.55", "v0.1.55", or the full release tag "proxybase-gui-v0.1.55"
// — a tag name must never end up inside Cargo.toml's version field (invalid
// semver breaks the whole workspace build).
const newVersion = process.argv[2]
  ?.replace(/^proxybase-gui-v/i, "")
  .replace(/^v/i, "");
if (!newVersion) {
  console.error("Usage: pnpm version-sync <version>");
  process.exit(1);
}

function syncFile(filePath, updater) {
  const absPath = path.resolve(root, filePath);
  if (!fs.existsSync(absPath)) {
    console.log(`  skip ${filePath} (not found)`);
    return;
  }
  const before = fs.readFileSync(absPath, "utf-8");
  const after = updater(before, newVersion);
  if (before !== after) {
    fs.writeFileSync(absPath, after);
    console.log(`  ✓ ${filePath}`);
  } else {
    console.log(`  - ${filePath} (already ${newVersion})`);
  }
}

console.log(`Synchronizing all files to version ${newVersion}...`);

// 1. package.json
syncFile("package.json", (content) => {
  const pkg = JSON.parse(content);
  pkg.version = newVersion;
  return JSON.stringify(pkg, null, 2) + "\n";
});

// 2. src-tauri/Cargo.toml
syncFile("src-tauri/Cargo.toml", (content) =>
  content.replace(/^version = "[^"]+"/m, `version = "${newVersion}"`)
);

// 3. src-tauri/tauri.conf.json
syncFile("src-tauri/tauri.conf.json", (content) => {
  const conf = JSON.parse(content);
  conf.version = newVersion;
  return JSON.stringify(conf, null, 2) + "\n";
});

// 4. Android build.gradle.kts
syncFile("src-tauri/gen/android/app/build.gradle.kts", (content) =>
  content.replace(/versionName\s*=\s*"[^"]+"/, `versionName = "${newVersion}"`)
);

console.log(`\nAll files synchronized to v${newVersion}.`);
