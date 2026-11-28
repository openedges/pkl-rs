#!/usr/bin/env node
import { execFileSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(here, "..");
const repoRoot = resolve(pkgDir, "..", "..");
const srcDir = resolve(pkgDir, "src");
const distDir = resolve(pkgDir, "dist");
const nativeDir = resolve(pkgDir, "native");

const argv = process.argv.slice(2);
const args = new Set(argv);
const distOnly = args.has("--dist-only");
const skipCargo = distOnly || args.has("--skip-cargo");
const debugProfile = args.has("--debug");
const targetIdx = argv.indexOf("--target");
const rustTarget = targetIdx >= 0 ? argv[targetIdx + 1] : process.env.PKL_NAPI_RUST_TARGET || null;

function detectLinuxLibc() {
  if (rustTarget) {
    if (rustTarget.includes("musl")) return "musl";
    if (rustTarget.includes("gnu")) return "gnu";
  }
  if (process.platform !== "linux") return null;
  try {
    const report =
      process.report && typeof process.report.getReport === "function"
        ? process.report.getReport()
        : null;
    if (report && report.header && report.header.glibcVersionRuntime) return "gnu";
    const shared = report && Array.isArray(report.sharedObjects) ? report.sharedObjects : [];
    if (shared.some((s) => typeof s === "string" && (s.includes("musl") || s.includes("ld-musl")))) {
      return "musl";
    }
  } catch {
    // Fall through
  }
  if (existsSync("/lib/ld-musl-x86_64.so.1") || existsSync("/lib/ld-musl-aarch64.so.1")) {
    return "musl";
  }
  return "gnu";
}

function targetArchForName() {
  if (rustTarget) {
    if (rustTarget.startsWith("x86_64-")) return "x64";
    if (rustTarget.startsWith("aarch64-")) return "arm64";
  }
  return process.arch;
}

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

function targetPlatformForName() {
  if (rustTarget) {
    if (rustTarget.includes("-apple-darwin")) return "darwin";
    if (rustTarget.includes("-linux-")) return "linux";
    if (rustTarget.includes("-windows-")) return "win32";
  }
  return process.platform;
}

function sharedLibName() {
  const plat = targetPlatformForName();
  if (plat === "darwin") return "libpkl_napi.dylib";
  if (plat === "linux") return "libpkl_napi.so";
  if (plat === "win32") return "pkl_napi.dll";
  throw new Error(`Unsupported platform for @openedges/pkl-napi build: ${plat}`);
}

function platformBinaryName() {
  const arch = targetArchForName();
  const plat = targetPlatformForName();
  if (plat === "linux") {
    const libc = detectLinuxLibc() || "gnu";
    return `pkl_napi.linux-${arch}-${libc}.node`;
  }
  return `pkl_napi.${plat}-${arch}.node`;
}

if (!skipCargo) {
  const cargoArgs = ["build", "-p", "pkl-napi", "--locked"];
  if (rustTarget) {
    cargoArgs.push("--target", rustTarget);
  }
  if (!debugProfile) {
    cargoArgs.push("--release");
  }
  execFileSync(resolveCargoBin(), cargoArgs, {
    cwd: repoRoot,
    stdio: "inherit",
  });
}

let platformAddon = null;
if (!distOnly) {
  const targetBase = process.env.CARGO_TARGET_DIR
    ? resolve(repoRoot, process.env.CARGO_TARGET_DIR)
    : resolve(repoRoot, "target");
  const profileDir = debugProfile ? "debug" : "release";
  const builtLib = rustTarget
    ? resolve(targetBase, rustTarget, profileDir, sharedLibName())
    : resolve(targetBase, profileDir, sharedLibName());

  if (!existsSync(builtLib)) {
    throw new Error(`Built pkl-napi library not found at ${builtLib}`);
  }

  if (existsSync(nativeDir)) {
    rmSync(nativeDir, { recursive: true, force: true });
  }
  mkdirSync(nativeDir, { recursive: true });
  platformAddon = resolve(nativeDir, platformBinaryName());
  copyFileSync(builtLib, platformAddon);
}

const profileDir = debugProfile ? "debug" : "release";

if (existsSync(distDir)) {
  rmSync(distDir, { recursive: true, force: true });
}
mkdirSync(distDir, { recursive: true });
copyFileSync(resolve(srcDir, "runtime.cjs"), resolve(distDir, "index.cjs"));
copyFileSync(resolve(srcDir, "index.mjs"), resolve(distDir, "index.mjs"));
copyFileSync(resolve(srcDir, "testing.cjs"), resolve(distDir, "testing.cjs"));
copyFileSync(resolve(srcDir, "testing.mjs"), resolve(distDir, "testing.mjs"));

const indexDts = readFileSync(resolve(srcDir, "index.d.ts"), "utf8");
writeFileSync(resolve(distDir, "index.d.ts"), indexDts);
writeFileSync(resolve(distDir, "index.d.cts"), indexDts);
writeFileSync(
  resolve(distDir, "index.d.mts"),
  `${indexDts.trimEnd()}\n\ndeclare const _default: {\n` +
    `  PKL_VERSION: typeof PKL_VERSION;\n` +
    `  capabilities: typeof capabilities;\n` +
    `  PklError: typeof PklError;\n` +
    `  ModuleSource: typeof ModuleSource;\n` +
    `  PklDuration: typeof PklDuration;\n` +
    `  PklDataSize: typeof PklDataSize;\n` +
    `  PklPair: typeof PklPair;\n` +
    `  PklRegex: typeof PklRegex;\n` +
    `  PklObject: typeof PklObject;\n` +
    `  PklClass: typeof PklClass;\n` +
    `  PklTypeAlias: typeof PklTypeAlias;\n` +
    `  PklIntSeq: typeof PklIntSeq;\n` +
    `  PklFunction: typeof PklFunction;\n` +
    `  PklReference: typeof PklReference;\n` +
    `  Evaluator: typeof Evaluator;\n` +
    `  Project: typeof Project;\n` +
    `  ReplSession: typeof ReplSession;\n` +
    `  createEvaluator: typeof createEvaluator;\n` +
    `  createEvaluatorSync: typeof createEvaluatorSync;\n` +
    `  createRepl: typeof createRepl;\n` +
    `  analyzeImportGraph: typeof analyzeImportGraph;\n` +
    `  format: typeof format;\n` +
    `  decodePklBinary: typeof decodePklBinary;\n` +
    `  monotonicClockMs: typeof monotonicClockMs;\n` +
    `};\nexport default _default;\n`,
);

const testingDts = readFileSync(resolve(srcDir, "testing.d.ts"), "utf8");
writeFileSync(resolve(distDir, "testing.d.ts"), testingDts);
writeFileSync(
  resolve(distDir, "testing.d.cts"),
  testingDts.replace('from "./index"', 'from "./index.cjs"'),
);
writeFileSync(
  resolve(distDir, "testing.d.mts"),
  `${testingDts.replace('from "./index"', 'from "./index.mjs"').trimEnd()}\n\ndeclare const _testingDefault: {\n` +
    `  roundtripValue: typeof roundtripValue;\n` +
    `  roundtripInteger: typeof roundtripInteger;\n` +
    `  roundtripBytes: typeof roundtripBytes;\n` +
    `  roundtripString: typeof roundtripString;\n` +
    `  measureConversion: typeof measureConversion;\n` +
    `};\nexport default _testingDefault;\n`,
);

for (const legalFile of ["LICENSE", "NOTICE", "THIRD-PARTY-NOTICES.md"]) {
  copyFileSync(resolve(repoRoot, legalFile), resolve(pkgDir, legalFile));
}

console.log(
  JSON.stringify({
    package: "@openedges/pkl-napi",
    platform: process.platform,
    arch: process.arch,
    profile: profileDir,
    addon: platformAddon,
    dist: distDir,
  }),
);
