import * as wasmBindings from './wasm/pkl_wasm.js';
import { PklError, toPklError } from './errors.mjs';

export const PKL_VERSION = '0.32.0';

const isNode =
  typeof process !== 'undefined' &&
  process !== null &&
  typeof process.versions === 'object' &&
  typeof process.versions.node === 'string';

let initialized = false;
/** @type {Promise<void> | null} */
let initPromise = null;
/** @type {WebAssembly.Memory | null} */
let wasmMemory = null;

/**
 * Returns the exact host capability profile for this Wasm runtime.
 */
export function capabilities() {
  return {
    vfs: true,
    asyncModuleLoader: true,
    asyncResourceLoader: true,
    nativeFs: false,
    nativeHttp: false,
    externalReaders: false,
    modulePathJar: false,
    projectResolution: false,
    packageCache: false,
    formatter: true,
    repl: true,
    schema: true,
    analyzeImports: true,
    bigint64: true,
    uint8ArrayBytes: true,
    clock64Bit: true,
    platform: isNode ? process.platform : 'browser',
    runtime: isNode ? 'wasm-node' : 'wasm-browser',
    arch: 'wasm32',
  };
}

/**
 * Initializes the bundled WebAssembly module. Idempotent and safe to call multiple times.
 * Automatically called by async APIs (`createEvaluator`, `createRepl`, `analyzeImportGraph`),
 * and automatically initialized synchronously in Node.js for sync APIs (`format`, `decodePklBinary`).
 *
 * @param {RequestInfo | URL | Response | BufferSource | WebAssembly.Module} [input]
 * @returns {Promise<void>}
 */
export async function init(input = undefined) {
  if (initialized) return;
  if (initPromise && input === undefined) {
    return initPromise;
  }
  const run = async () => {
    try {
      if (input !== undefined) {
        const exportsObj = await wasmBindings.default({ module_or_path: input });
        wasmMemory = exportsObj?.memory ?? null;
        initialized = true;
        return;
      }
      const wasmUrl = new URL('./wasm/pkl_wasm_bg.wasm', import.meta.url);
      if (isNode) {
        if (typeof process.getBuiltinModule === 'function') {
          const fs = process.getBuiltinModule('node:fs');
          const urlMod = process.getBuiltinModule('node:url');
          const bytes = fs.readFileSync(urlMod.fileURLToPath(wasmUrl));
          const exportsObj = wasmBindings.initSync({ module: bytes });
          wasmMemory = exportsObj?.memory ?? null;
          initialized = true;
          return;
        }
        const fsModuleName = 'node:fs/promises';
        const urlModuleName = 'node:url';
        const fs = await import(/* @vite-ignore */ fsModuleName);
        const urlMod = await import(/* @vite-ignore */ urlModuleName);
        const bytes = await fs.readFile(urlMod.fileURLToPath(wasmUrl));
        const exportsObj = await wasmBindings.default({ module_or_path: bytes });
        wasmMemory = exportsObj?.memory ?? null;
        initialized = true;
        return;
      }
      const exportsObj = await wasmBindings.default({ module_or_path: wasmUrl });
      wasmMemory = exportsObj?.memory ?? null;
      initialized = true;
    } catch (err) {
      initPromise = null;
      throw toPklError(err, 'configuration');
    }
  };
  initPromise = run();
  return initPromise;
}

/**
 * Ensures the Wasm module is ready for synchronous APIs (`format`, `decodePklBinary`, `roundtripValue`).
 * In Node.js, synchronously loads the bundled `.wasm` artifact if `init()` was not yet called.
 * In browsers, throws a `PklError` (`configuration`) instructing callers to `await init()` first.
 */
export function ensureInitSync() {
  if (initialized) return;
  if (isNode && typeof process.getBuiltinModule === 'function') {
    const fs = process.getBuiltinModule('node:fs');
    const urlMod = process.getBuiltinModule('node:url');
    const wasmUrl = new URL('./wasm/pkl_wasm_bg.wasm', import.meta.url);
    const bytes = fs.readFileSync(urlMod.fileURLToPath(wasmUrl));
    const exportsObj = wasmBindings.initSync({ module: bytes });
    wasmMemory = exportsObj?.memory ?? null;
    initialized = true;
    return;
  }
  throw new PklError(
    'Wasm module is not initialized. Call `await init()` before invoking synchronous APIs in a browser.',
    'configuration',
  );
}

export function getWasmBindings() {
  ensureInitSync();
  return wasmBindings;
}

export function getWasmMemory() {
  ensureInitSync();
  return wasmMemory;
}

export function isNodeRuntime() {
  return isNode;
}
