#!/usr/bin/env node
/**
 * Builds the self-contained `@pkl-rs/wasm` distribution into `packages/pkl-wasm/dist/`
 * using the pinned `.cache/m1-tools/bin/wasm-bindgen` (0.2.108) binary.
 */
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import {
  accessSync,
  constants,
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
} from 'node:fs';
import { delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const IS_WIN = process.platform === 'win32';
const SCRIPT_DIR = dirname(fileURLToPath(import.meta.url));
const PKG_DIR = resolve(SCRIPT_DIR, '..');
const REPO_ROOT = resolve(PKG_DIR, '..', '..');
const SRC_DIR = join(PKG_DIR, 'src');
const DIST_DIR = join(PKG_DIR, 'dist');
const WASM_OUT_DIR = join(DIST_DIR, 'wasm');
const BINDGEN = resolve(
  process.env.PKL_WASM_BINDGEN ||
    join(REPO_ROOT, '.cache/m1-tools/bin', IS_WIN ? 'wasm-bindgen.exe' : 'wasm-bindgen'),
);
const TARGET_DIR = resolve(REPO_ROOT, process.env.CARGO_TARGET_DIR || 'target');
const WASM_ARTIFACT = join(TARGET_DIR, 'wasm32-unknown-unknown/release/pkl_wasm.wasm');
const SKIP_CARGO = process.argv.includes('--skip-cargo');

function resolveExecutable(name, fallbacks = []) {
  if (name.includes('/') || name.includes('\\')) return name;
  const binName = IS_WIN && !name.endsWith('.exe') ? `${name}.exe` : name;
  const dirs = (process.env.PATH || '').split(delimiter).filter(Boolean);
  for (const dir of [...dirs, ...fallbacks]) {
    const candidate = join(dir, binName);
    try {
      if (IS_WIN) {
        if (existsSync(candidate)) return candidate;
      } else {
        accessSync(candidate, constants.X_OK);
        return candidate;
      }
    } catch {
      // Skip inaccessible or missing directories safely.
    }
  }
  return binName;
}

const CARGO =
  process.env.CARGO ||
  resolveExecutable('cargo', [
    process.env.HOME ? join(process.env.HOME, '.cargo/bin') : '',
    process.env.USERPROFILE ? join(process.env.USERPROFILE, '.cargo/bin') : '',
    '/opt/homebrew/bin',
    '/usr/local/bin',
  ].filter(Boolean));

function run(cmd, args, cwd = REPO_ROOT) {
  execFileSync(cmd, args, { cwd, stdio: 'inherit' });
}

if (!existsSync(BINDGEN)) {
  throw new Error(`Pinned wasm-bindgen binary not found at ${BINDGEN}`);
}
const versionOut = execFileSync(BINDGEN, ['--version'], { encoding: 'utf8' }).trim();
if (versionOut !== 'wasm-bindgen 0.2.108') {
  throw new Error(`Expected wasm-bindgen 0.2.108, got "${versionOut}"`);
}

// 1. Build release wasm32-unknown-unknown binary
if (!SKIP_CARGO) {
  run(CARGO, [
    'build',
    '-p',
    'pkl-wasm',
    '--target',
    'wasm32-unknown-unknown',
    '--release',
    '--locked',
  ]);
}

if (!existsSync(WASM_ARTIFACT)) {
  throw new Error(`Expected wasm artifact at ${WASM_ARTIFACT}`);
}

// 2. Clean and populate dist/
rmSync(DIST_DIR, { recursive: true, force: true });
mkdirSync(WASM_OUT_DIR, { recursive: true });

// 3. Run pinned wasm-bindgen targeting web ESM (works in both Chromium browser and Node)
run(BINDGEN, ['--target', 'web', '--out-dir', WASM_OUT_DIR, WASM_ARTIFACT]);

// 4. Copy JS wrapper sources and TypeScript declarations into dist/
for (const entry of readdirSync(SRC_DIR)) {
  if (entry.endsWith('.mjs') || entry.endsWith('.d.ts')) {
    copyFileSync(join(SRC_DIR, entry), join(DIST_DIR, entry));
  }
}

// 5. Summarize built distribution artifacts
const summary = {};
for (const rel of [
  'index.mjs',
  'index.d.ts',
  'wasm/pkl_wasm.js',
  'wasm/pkl_wasm_bg.wasm',
]) {
  const abs = join(DIST_DIR, rel);
  const bytes = readFileSync(abs);
  summary[rel] = {
    bytes: bytes.length,
    sha256: createHash('sha256').update(bytes).digest('hex'),
  };
}
console.log(JSON.stringify({ package: '@pkl-rs/wasm', dist: DIST_DIR, artifacts: summary }, null, 2));
