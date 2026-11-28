# `@openedges/pkl-wasm`

WebAssembly distribution of the pure-Rust **Apple Pkl `0.32.0`** evaluation engine and code formatter (`pkl-runtime` + `pkl-formatter`), targeting modern browsers (ESM) and Node.js with zero Java runtime dependency.

## Installation

```sh
npm install @openedges/pkl-wasm
```

- **Node.js**: `>= 20.16.0` (ES Modules)
- **Browsers**: Evergreen browsers supporting WebAssembly and ES Modules (Chromium, Firefox, WebKit/Safari)
- **Bundle size**: `pkl_wasm_bg.wasm` is kept within the `<= 2.50 MiB` (`2,621,440 B`) gzip-9 budget.

## Features

- Full Pkl `0.32.0` evaluation engine: all 23 `pkl:*` stdlib modules, all 542 builtins, and all 9 output renderers (`pcf`, `json`, `yaml`, `xml`, `properties`, `plist`, `jsonnet`, `textproto`, `pkl-binary`).
- Pure-Wasm `VirtualFileSystem` with lexical `rootDir` confinement, directory listing, globbing, and `file:///` URI mounting.
- Cooperative async Host suspension/resumption supporting custom async `moduleReaders`, `resourceReaders`, `httpFetch`, `packageTransport` (with strict SHA-256 verification and pure-Rust ZIP DEFLATE decompression), `securityManager`, `logger`, and `frameTransformer`.
- Stateful `createRepl()`, static `analyzeImportGraph()`, `evaluateSchema()`, `evaluateTest()`, `format()`, and lossless `decodePklBinary()`.
- Lossless JS/Wasm boundary: exact signed 64-bit `BigInt`, owned `Uint8Array` bytes, UTF-16 strings, 64-bit unsigned monotonic clocks & timeouts, and `AbortSignal` / `cancel()` / idempotent `close()` cleanup.
- Explicit capability reporting via `capabilities()` and structured `PklError` (`code: "capability-unavailable"`) whenever an unavailable native host capability is requested.

## Quick Start

```js
import {
  init,
  capabilities,
  createEvaluator,
  ModuleSource,
  VirtualFileSystem,
  format,
} from '@openedges/pkl-wasm';

// Required once in browsers before calling sync APIs; optional in Node.js (auto-initialized).
await init();

const vfs = new VirtualFileSystem(
  {
    '/app/config.pkl': 'name = "pkl-wasm"\nport = 8080\n',
  },
  { rootDir: '/app' },
);

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

WebAssembly runs in a sandbox without direct OS filesystem access, raw TCP/TLS sockets, external reader subprocesses, JAR/modulepath archives, `PklProject` disk resolution, or persistent package disk caches.

- Calling `capabilities()` reports these boundaries accurately (`nativeFs: false`, `nativeHttp: false`, `externalReaders: false`, `modulePathJar: false`, `projectResolution: false`, `packageCache: false`).
- Requesting an unprovided capability raises `PklError` with `code === "capability-unavailable"` and the missing `capability` field.
- For full native Node.js Host I/O (OS filesystem, `rustls` HTTPS, `PklProject`, `package:` disk cache, external readers), use [`@openedges/pkl-napi`](https://github.com/openedges/pkl-rs/tree/main/packages/pkl-napi).

## Compatibility & Versioning

Version `0.32.0` tracks the **Apple Pkl `0.32.0`** compatibility baseline (`PKL_VERSION === "0.32.0"`). See the [pkl-rs Versioning Guide](https://github.com/openedges/pkl-rs/blob/main/docs/versioning.md).

## License & Trademarks

Licensed under the [Apache License, Version 2.0](https://github.com/openedges/pkl-rs/blob/main/LICENSE) (Copyright 2025–2026 The pkl-rs Authors). Attribution and third-party notices are bundled in [NOTICE](https://github.com/openedges/pkl-rs/blob/main/NOTICE) and [THIRD-PARTY-NOTICES.md](https://github.com/openedges/pkl-rs/blob/main/THIRD-PARTY-NOTICES.md).

`@openedges/pkl-wasm` is part of the independent open-source `pkl-rs` project and is not affiliated with, sponsored by, or endorsed by Apple Inc. "Pkl" is referenced solely to describe language compatibility.
