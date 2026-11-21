# `@pkl-rs/wasm`

WebAssembly distribution of the shared Rust Apple Pkl `0.32.0` runtime and code formatter (`pkl-runtime` + `pkl-formatter`), targeting both browsers (ESM) and Node.js.

## Features

- Full Pkl `0.32.0` evaluation engine: all 23 `pkl:*` stdlib modules, all 542 builtins, and all 9 output renderers (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `jsonnet`, `textproto`, `pkl-binary`).
- Pure-Wasm `VirtualFileSystem` with lexical `rootDir` confinement, directory listing, globbing, and `file:///` URI mounting.
- Cooperative async Host suspension/resumption supporting custom async `moduleReaders`, `resourceReaders`, `httpFetch`, `packageTransport` (with strict SHA-256 verification and pure-Rust ZIP DEFLATE decompression), `securityManager`, `logger`, and `frameTransformer`.
- Stateful `createRepl()`, static `analyzeImportGraph()`, `evaluateSchema()`, `evaluateTest()`, `format()`, and lossless `decodePklBinary()`.
- Lossless JS/Wasm boundary: exact signed 64-bit `BigInt`, owned `Uint8Array` bytes, UTF-16 strings, 64-bit unsigned monotonic clocks & timeouts, and `AbortSignal` / `cancel()` / idempotent `close()` cleanup.
- Explicit capability reporting via `capabilities()` and structured `PklError` (`code: "capability-unavailable"`) whenever an unavailable native host capability is requested.

## Usage

```js
import {
  init,
  capabilities,
  createEvaluator,
  ModuleSource,
  VirtualFileSystem,
  format,
} from '@pkl-rs/wasm';

// Optional in Node.js (auto-initialized); call once in browsers before sync APIs.
await init();

const vfs = new VirtualFileSystem({
  '/app/config.pkl': 'name = "pkl-wasm"\nport = 8080\n',
}, { rootDir: '/app' });

const evaluator = await createEvaluator({ vfs, outputFormat: 'json' });
try {
  const json = await evaluator.evaluateOutputText(ModuleSource.path('/app/config.pkl'));
  console.log(json);
} finally {
  evaluator.close();
}

console.log(format('x=1+2\n'));
console.log(capabilities());
```

## Explicit Wasm Capability Boundaries

Wasm runs without direct OS filesystem, raw TCP/TLS sockets, external reader subprocesses, JAR/modulepath archives, `PklProject` disk resolution, or persistent package disk caches. Calling `capabilities()` reports these accurately (`nativeFs: false`, `nativeHttp: false`, `externalReaders: false`, `modulePathJar: false`, `projectResolution: false`, `packageCache: false`), and requesting an unprovided capability raises `PklError` with `code === "capability-unavailable"` and the missing `capability` field.
