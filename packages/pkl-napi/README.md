# `@pkl-rs/napi`

Node.js N-API bindings for the shared pure-Rust Apple Pkl 0.32.0 engine (`pkl-runtime`, `pkl-host-native`, `pkl`, and `pkl-formatter`) with zero Java fallback.

## Features

- Non-blocking `Promise` evaluation on libuv worker threads (`createEvaluator`, `Evaluator`) plus optional `*Sync` methods for pure-native evaluations.
- Full Node.js native host I/O: filesystem (`file:`), `env:`, `prop:`, `modulepath:` (directories + JAR/ZIP), HTTP/HTTPS (rustls + bundled `PklCARoots.pem`, custom CA certificates, proxy/`noProxy`, headers, rewrites), `PklProject` / `PklProject.deps.json` (`Project.load`), `package:` / `projectpackage:` + `package-2` disk cache, and external module/resource reader subprocesses (`0x26`–`0x32`).
- Custom sync/async JS extensions: `moduleReaders`, `resourceReaders`, `logger`, `frameTransformer`, and `securityManager`.
- Lossless value boundaries: exact signed 64-bit `BigInt`, owned `Uint8Array` copies, lossless UTF-16 strings, typed `PklDuration`, `PklDataSize`, `PklPair`, `PklRegex`, `PklObject`, `PklClass`, `PklTypeAlias`, `PklIntSeq`, `PklFunction`, `PklReference`, and 64-bit monotonic clocks/timeouts (`u64`).
- Dual ESM (`import`) and CommonJS (`require`) distribution with TypeScript declarations.

## Quick Example

```js
import { createEvaluator, ModuleSource, format } from "@pkl-rs/napi";

const evaluator = await createEvaluator();
try {
  const text = await evaluator.evaluateOutputText(
    ModuleSource.text('name = "world"\ngreeting = "Hello, \\(name)!"'),
  );
  console.log(text);
} finally {
  await evaluator.close();
}
```
