#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(here, "..");
const repoRoot = resolve(pkgDir, "..", "..");
const srcDir = resolve(pkgDir, "src");
const distDir = resolve(pkgDir, "dist");
const nativeDir = resolve(pkgDir, "native");

const args = new Set(process.argv.slice(2));
const skipCargo = args.has("--skip-cargo");
const debugProfile = args.has("--debug");

function resolveCargoBin() {
  const exe = process.platform === "win32" ? "cargo.exe" : "cargo";
  if (process.env.CARGO && existsSync(process.env.CARGO)) {
    return process.env.CARGO;
  }
  const homeCargo = join(homedir(), ".cargo", "bin", exe);
  if (existsSync(homeCargo)) {
    return homeCargo;
  }
  for (const dir of (process.env.PATH || "").split(delimiter)) {
    if (!dir) continue;
    const candidate = join(dir, exe);
    try {
      if (existsSync(candidate)) return candidate;
    } catch {
      // ignore inaccessible PATH entries
    }
  }
  return exe;
}

function sharedLibName() {
  if (process.platform === "darwin") return "libpkl_napi.dylib";
  if (process.platform === "linux") return "libpkl_napi.so";
  if (process.platform === "win32") return "pkl_napi.dll";
  throw new Error(`Unsupported platform for @pkl-rs/napi build: ${process.platform}`);
}

function platformBinaryName() {
  return `pkl_napi.${process.platform}-${process.arch}.node`;
}

if (!skipCargo) {
  const cargoArgs = ["build", "-p", "pkl-napi", "--locked"];
  if (!debugProfile) {
    cargoArgs.push("--release");
  }
  execFileSync(resolveCargoBin(), cargoArgs, {
    cwd: repoRoot,
    stdio: "inherit",
  });
}

const targetBase = process.env.CARGO_TARGET_DIR
  ? resolve(repoRoot, process.env.CARGO_TARGET_DIR)
  : resolve(repoRoot, "target");
const profileDir = debugProfile ? "debug" : "release";
const builtLib = resolve(targetBase, profileDir, sharedLibName());

if (!existsSync(builtLib)) {
  throw new Error(`Built pkl-napi library not found at ${builtLib}`);
}

if (existsSync(nativeDir)) {
  rmSync(nativeDir, { recursive: true, force: true });
}
mkdirSync(nativeDir, { recursive: true });
const platformAddon = resolve(nativeDir, platformBinaryName());
copyFileSync(builtLib, platformAddon);

if (existsSync(distDir)) {
  rmSync(distDir, { recursive: true, force: true });
}
mkdirSync(distDir, { recursive: true });
copyFileSync(resolve(srcDir, "runtime.cjs"), resolve(distDir, "index.cjs"));
copyFileSync(resolve(srcDir, "index.mjs"), resolve(distDir, "index.mjs"));
copyFileSync(resolve(srcDir, "index.d.ts"), resolve(distDir, "index.d.ts"));

console.log(
  JSON.stringify({
    package: "@pkl-rs/napi",
    platform: process.platform,
    arch: process.arch,
    profile: profileDir,
    addon: platformAddon,
    dist: distDir,
  }),
);
