# `@pkl-rs/napi`

Node.js N-API native bindings for the pure-Rust **Apple Pkl `0.32.0`** engine (`pkl-runtime`, `pkl-host-native`, `pkl`, and `pkl-formatter`) with zero Java runtime dependency.

## Installation

```sh
npm install @pkl-rs/napi
```

### Requirements & Supported Platforms

- **Node.js**: `>= 18.0.0` (N-API v8)
- **Module formats**: Dual ESM (`import`) and CommonJS (`require`) with bundled TypeScript declarations (`dist/index.d.ts`).
- **Prebuilt native addons (7 targets)**:
  - macOS: `darwin-arm64`, `darwin-x64`
  - Linux (glibc `>= 2.28`): `linux-x64-gnu`, `linux-arm64-gnu`
  - Linux (musl / Alpine): `linux-x64-musl`, `linux-arm64-musl`
  - Windows: `win32-x64-msvc`

## Features

- Non-blocking `Promise` evaluation on libuv worker threads (`createEvaluator`, `Evaluator`) plus optional `*Sync` methods (`createEvaluatorSync`) for pure-native evaluations.
- Full native Node.js Host I/O: filesystem (`file:`), `env:`, `prop:`, `modulepath:` (directories + JAR/ZIP), HTTP/HTTPS (`rustls` + bundled `PklCARoots.pem`, custom CA certificates, proxy/`noProxy`, headers, rewrites), `PklProject` / `PklProject.deps.json` (`Project.load`), `package:` / `projectpackage:` + `package-2` disk cache, and external module/resource reader subprocesses (`0x26`–`0x32`).
- Custom sync/async JS extensions: `moduleReaders`, `resourceReaders`, `logger`, `frameTransformer`, and `securityManager`.
- Lossless value boundaries: exact signed 64-bit `BigInt`, owned `Uint8Array` copies, lossless UTF-16 strings, typed `PklDuration`, `PklDataSize`, `PklPair`, `PklRegex`, `PklObject`, `PklClass`, `PklTypeAlias`, `PklIntSeq`, `PklFunction`, `PklReference`, and 64-bit monotonic clocks/timeouts (`u64`).

## Quick Start

### ESM (`import`)

```js
import { createEvaluator, ModuleSource, format } from '@pkl-rs/napi';

const evaluator = await createEvaluator({ outputFormat: 'json' });
try {
  const json = await evaluator.evaluateOutputText(
    ModuleSource.text('name = "world"\ngreeting = "Hello, \\(name)!"'),
  );
  console.log(json);
} finally {
  await evaluator.close();
}

console.log(format('x=1+2\n'));
```

### CommonJS (`require`)

```js
const { createEvaluatorSync, ModuleSource } = require('@pkl-rs/napi');

const evaluator = createEvaluatorSync();
try {
  console.log(evaluator.evaluateExpressionSync(ModuleSource.text('x = 20 + 22'), 'x'));
} finally {
  evaluator.close();
}
```

## Compatibility & Versioning

Version `0.32.0` tracks the **Apple Pkl `0.32.0`** compatibility baseline (`PKL_VERSION === "0.32.0"`). See the [pkl-rs Versioning Guide](https://github.com/openedges/pkl-rs/blob/main/docs/versioning.md).

## License & Trademarks

Licensed under the [Apache License, Version 2.0](LICENSE) (Copyright 2025–2026 The pkl-rs Authors). Attribution and third-party notices are bundled in [NOTICE](NOTICE) and [THIRD-PARTY-NOTICES.md](THIRD-PARTY-NOTICES.md).

`@pkl-rs/napi` is part of the independent open-source `pkl-rs` project and is not affiliated with, sponsored by, or endorsed by Apple Inc. "Pkl" is referenced solely to describe language compatibility.
