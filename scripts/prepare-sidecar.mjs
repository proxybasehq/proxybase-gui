#!/usr/bin/env node
/**
 * Build `proxybase-tunneld` and place it where Tauri's externalBin bundler
 * expects it:
 *
 *   src-tauri/binaries/proxybase-tunneld-<target-triple><.exe>
 *   src-tauri/binaries/proxybase-tunneld-universal-apple-darwin   (macOS)
 *
 * The GUI repository is checked out standalone in CI, so we must build the
 * crate directly via `--manifest-path` instead of assuming a parent Cargo
 * workspace exists. In the local monorepo, the same command still resolves
 * through the root workspace (the crate is a workspace member).
 *
 * Usage: node scripts/prepare-sidecar.mjs [debug|release]
 *
 * Tauri sets TAURI_ENV_TARGET_TRIPLE / TAURI_ENV_PLATFORM for build hooks.
 * Mobile builds have no bundled tunneld (Android uses VpnService + JNI), so
 * the script exits successfully there.
 */
import { execFileSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdirSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const projectDir = resolve(scriptDir, "..");
const tauriDir = join(projectDir, "src-tauri");
const manifestPath = join(projectDir, "crates", "proxybase-tunneld", "Cargo.toml");
const outputDir = join(tauriDir, "binaries");
// Share Tauri's Rust target dir so CI's rust-cache covers the daemon too.
const targetDir = join(tauriDir, "target");

const profile = process.argv[2] === "debug" ? "debug" : "release";
const isWindows = process.platform === "win32";
const exeSuffix = isWindows ? ".exe" : "";

const requestedTriple = (process.env.TAURI_ENV_TARGET_TRIPLE || "").trim();
const requestedPlatform = (process.env.TAURI_ENV_PLATFORM || "").trim().toLowerCase();
const requestedArch = (process.env.TAURI_ENV_ARCH || "").trim().toLowerCase();

function hostTriple() {
  const rustcVersion = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  const triple = rustcVersion.match(/^host:\s*(\S+)$/m)?.[1];
  if (!triple) throw new Error("could not determine the Rust host triple");
  return triple;
}

function isMacPlatform(triple) {
  return (
    triple.includes("apple-darwin") ||
    requestedPlatform.includes("darwin") ||
    requestedPlatform.includes("macos") ||
    (requestedPlatform === "" && process.platform === "darwin")
  );
}

function installedRustTargets() {
  try {
    return execFileSync("rustup", ["target", "list", "--installed"], {
      encoding: "utf8",
    })
      .split(/\s+/)
      .filter(Boolean);
  } catch {
    return [];
  }
}

function buildUniversalMac() {
  if (process.platform !== "darwin") {
    throw new Error("universal-apple-darwin sidecar can only be built on macOS");
  }
  const arm = buildTarget("aarch64-apple-darwin");
  const intel = buildTarget("x86_64-apple-darwin");
  // Tauri's universal bundler looks for a single fat sidecar named after
  // the universal target triple.
  const armDest = publish(arm, "aarch64-apple-darwin");
  const intelDest = publish(intel, "x86_64-apple-darwin");
  const universalDest = join(outputDir, "proxybase-tunneld-universal-apple-darwin");
  console.log(`[sidecar] lipo -create ${armDest} ${intelDest}`);
  execFileSync("lipo", ["-create", armDest, intelDest, "-output", universalDest], {
    stdio: "inherit",
  });
  chmodSync(universalDest, 0o755);
  console.log(`[sidecar] created ${universalDest}`);
}

function isMobileTarget(triple) {
  return (
    triple.includes("android") ||
    triple.includes("ios") ||
    requestedPlatform === "android" ||
    requestedPlatform === "ios"
  );
}

function buildTarget(triple) {
  const args = [
    "build",
    "--manifest-path",
    manifestPath,
    "--target",
    triple,
  ];
  if (profile === "release") args.push("--release");

  console.log(`[sidecar] cargo ${args.join(" ")}`);
  execFileSync("cargo", args, {
    cwd: projectDir,
    stdio: "inherit",
    env: { ...process.env, CARGO_TARGET_DIR: targetDir },
  });

  const artifact = join(
    targetDir,
    triple,
    profile,
    `proxybase-tunneld${exeSuffix}`
  );
  if (!existsSync(artifact)) {
    throw new Error(
      `sidecar binary not found at ${artifact}\n` +
        `Did cargo build --target ${triple} succeed and is that target installed?`
    );
  }
  return artifact;
}

function publish(artifact, outputTriple) {
  mkdirSync(outputDir, { recursive: true });
  const dest = join(outputDir, `proxybase-tunneld-${outputTriple}${exeSuffix}`);
  copyFileSync(artifact, dest);
  if (!isWindows) chmodSync(dest, 0o755);
  console.log(`[sidecar] ${artifact} -> ${dest}`);
  return dest;
}

function main() {
  if (isMobileTarget(requestedTriple)) {
    console.log(
      `[sidecar] skipping tunneld for mobile target ${requestedTriple || requestedPlatform} ` +
        "(Android/iOS use the in-process VpnService engine)"
    );
    return;
  }

  const universalMac =
    requestedTriple === "universal-apple-darwin" || requestedArch === "universal";

  if (universalMac) {
    buildUniversalMac();
    return;
  }

  const triple =
    requestedTriple && requestedTriple !== "universal-apple-darwin"
      ? requestedTriple
      : hostTriple();
  if (isMobileTarget(triple)) {
    console.log(`[sidecar] skipping tunneld for mobile target ${triple}`);
    return;
  }

  // Insurance for older Tauri CLI versions that do not populate
  // TAURI_ENV_TARGET_TRIPLE for a `--target universal-apple-darwin` build.
  // When a macOS release build has both Rust targets installed, also emit the
  // fat sidecar (the host binary is published either way).
  if (
    profile === "release" &&
    isMacPlatform(triple) &&
    !triple.includes("universal") &&
    installedRustTargets().includes("x86_64-apple-darwin") &&
    installedRustTargets().includes("aarch64-apple-darwin")
  ) {
    buildUniversalMac();
    return;
  }

  publish(buildTarget(triple), triple);
}

try {
  main();
} catch (error) {
  console.error(`[sidecar] ${error.message ?? error}`);
  process.exit(1);
}
