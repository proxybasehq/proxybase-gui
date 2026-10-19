#!/usr/bin/env node
/**
 * Build proxybase-tunneld and place it where Tauri's externalBin bundler
 * expects it: src-tauri/binaries/proxybase-tunneld-<target-triple><.exe>
 *
 * Usage: node scripts/prepare-sidecar.mjs [debug|release]
 */
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, chmodSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(scriptDir, "..");
const workspaceRoot = resolve(projectDir, "..");
const profile = process.argv[2] === "debug" ? "debug" : "release";
const isWindows = process.platform === "win32";
const exeSuffix = isWindows ? ".exe" : "";

const cargoArgs = ["build", "-p", "proxybase-tunneld"];
if (profile === "release") cargoArgs.push("--release");

console.log(`[sidecar] cargo ${cargoArgs.join(" ")}`);
execFileSync("cargo", cargoArgs, {
  cwd: workspaceRoot,
  stdio: "inherit",
});

const rustcVersion = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
const triple = rustcVersion.match(/^host:\s*(\S+)$/m)?.[1];
if (!triple) throw new Error("could not determine the Rust host triple");

const candidates = [
  join(workspaceRoot, "target", profile, `proxybase-tunneld${exeSuffix}`),
  join(projectDir, "src-tauri", "target", profile, `proxybase-tunneld${exeSuffix}`),
];
const built = candidates.find((path) => existsSync(path));
if (!built) {
  throw new Error(`sidecar binary not found; looked in:\n${candidates.join("\n")}`);
}

const outDir = join(projectDir, "src-tauri", "binaries");
mkdirSync(outDir, { recursive: true });
const dest = join(outDir, `proxybase-tunneld-${triple}${exeSuffix}`);
copyFileSync(built, dest);
if (!isWindows) chmodSync(dest, 0o755);
console.log(`[sidecar] ${built} -> ${dest}`);
