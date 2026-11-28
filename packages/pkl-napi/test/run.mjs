#!/usr/bin/env node
import assert from "node:assert/strict";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync, readFileSync, existsSync } from "node:fs";
import http from "node:http";
import { createRequire } from "node:module";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

import * as esmPkg from "../dist/index.mjs";
import {
  PKL_VERSION,
  capabilities,
  PklError,
  ModuleSource,
  PklDuration,
  PklDataSize,
  PklPair,
  PklRegex,
  PklObject,
  PklClass,
  PklTypeAlias,
  PklIntSeq,
  PklFunction,
  PklReference,
  Project,
  createEvaluator,
  createEvaluatorSync,
  createRepl,
  analyzeImportGraph,
  format,
  decodePklBinary,
  monotonicClockMs,
} from "../dist/index.mjs";
import {
  roundtripValue,
  roundtripInteger,
  roundtripBytes,
  roundtripString,
  measureConversion,
} from "../dist/testing.mjs";

const require = createRequire(import.meta.url);
const cjsPkg = require("../dist/index.cjs");

const MIN_I64 = -(1n << 63n);
const MAX_I64 = (1n << 63n) - 1n;
const MAX_U64 = (1n << 64n) - 1n;

const results = [];
async function test(name, fn) {
  const start = performance.now();
  try {
    await fn();
    results.push({ name, status: "pass", ms: Number((performance.now() - start).toFixed(2)) });
  } catch (err) {
    results.push({ name, status: "fail", error: err.stack || String(err) });
    throw err;
  }
}

await test("dual ESM/CJS identity and capabilities", async () => {
  assert.equal(PKL_VERSION, "0.32.0");
  assert.equal(cjsPkg.PKL_VERSION, "0.32.0");
  assert.equal(esmPkg.PklError, cjsPkg.PklError);
  assert.equal(esmPkg.ModuleSource, cjsPkg.ModuleSource);
  assert.equal(esmPkg.PklObject, cjsPkg.PklObject);
  for (const internalName of [
    "roundtripValue",
    "roundtripInteger",
    "roundtripBytes",
    "roundtripString",
    "measureConversion",
  ]) {
    assert.equal(internalName in esmPkg, false, `${internalName} must not be on ESM top-level`);
    assert.equal(internalName in cjsPkg, false, `${internalName} must not be on CJS top-level`);
  }

  const caps = capabilities();
  assert.deepEqual(caps, {
    vfs: false,
    asyncModuleLoader: true,
    asyncResourceLoader: true,
    nativeFs: true,
    nativeHttp: true,
    externalReaders: true,
    modulePathJar: true,
    projectResolution: true,
    packageCache: true,
    formatter: true,
    repl: true,
    schema: true,
    analyzeImports: true,
    bigint64: true,
    uint8ArrayBytes: true,
    clock64Bit: true,
    platform: process.platform,
    runtime: "napi-node",
    arch: process.arch,
  });
});

await test("M1 exports preserved on built .node addon", async () => {
  const here = fileURLToPath(new URL(".", import.meta.url));
  const nativeDir = resolve(here, "..", "native");
  const candidates = [
    process.platform === "linux"
      ? resolve(nativeDir, `pkl_napi.linux-${process.arch}-musl.node`)
      : null,
    process.platform === "linux"
      ? resolve(nativeDir, `pkl_napi.linux-${process.arch}-gnu.node`)
      : null,
    resolve(nativeDir, `pkl_napi.${process.platform}-${process.arch}.node`),
  ].filter(Boolean);
  const addonPath = candidates.find((p) => existsSync(p));
  assert.ok(addonPath, `Built .node addon not found in ${candidates.join(", ")}`);
  const addon = require(addonPath);
  for (const sym of [
    "primitive",
    "evaluate",
    "evaluateLanguage",
    "evaluateModuleFrontend",
    "evaluateModuleOutput",
    "evaluateLanguageSequence",
    "roundtripString",
    "roundtripInteger",
    "roundtripBytes",
    "HostProbe",
    "SourceSession",
    "transformFrameForPolicy",
    "globTransportCodec",
  ]) {
    assert.ok(sym in addon, `missing preserved M1 export: ${sym}`);
  }
  assert.equal(addon.roundtripInteger(42n), 42n);
  assert.equal(addon.roundtripString("hi\uD800"), "hi\uD800");
});

await test("core evaluation methods (async + sync)", async () => {
  const ev = await createEvaluator({
    properties: { envName: "prod" },
    env: { MY_SECRET: "token123" },
  });
  try {
    const src = ModuleSource.text(`
      name = "pkl-napi"
      propVal = read("prop:envName")
      envVal = read("env:MY_SECRET")
      count = 9223372036854775807
      minVal = -9223372036854775808
      negZero = -0.0
      nanVal = 0.0 / 0.0
      infVal = 1.0 / 0.0
      dur = 250.ms
      size = 64.kib
      pair = Pair("a", 7)
      rx = Regex("^[a-z]+$")
      raw = Bytes(1, 2, 255)
      items = List(1, 2, 3)
      uniq = Set("x", "y")
      dict = Map("k", 99)
      output {
        files {
          ["b.txt"] { text = "second" }
          ["a.txt"] { text = "first" }
        }
      }
    `);

    const text = await ev.evaluateOutputText(src);
    assert.ok(text.includes('name = "pkl-napi"'));
    assert.ok(text.includes('propVal = "prod"'));
    assert.ok(text.includes('envVal = "token123"'));
    assert.equal(ev.evaluateOutputTextSync(src), text);

    const bytes = await ev.evaluateOutputBytes(src);
    assert.ok(bytes instanceof Uint8Array);
    assert.equal(Buffer.from(bytes).toString("utf8"), text);

    const files = await ev.evaluateOutputFiles(src);
    assert.ok(files instanceof Map);
    assert.deepEqual(Array.from(files.keys()), ["b.txt", "a.txt"]);
    assert.equal(Buffer.from(files.get("b.txt")).toString("utf8"), "second");
    assert.equal(Buffer.from(files.get("a.txt")).toString("utf8"), "first");

    const val = await ev.evaluateOutputValue(src);
    assert.ok(val instanceof PklObject);
    assert.equal(val.properties.name, "pkl-napi");
    assert.equal(val.properties.count, MAX_I64);
    assert.equal(val.properties.minVal, MIN_I64);
    assert.ok(Object.is(val.properties.negZero, -0));
    assert.ok(Number.isNaN(val.properties.nanVal));
    assert.equal(val.properties.infVal, Infinity);
    assert.ok(val.properties.dur instanceof PklDuration);
    assert.equal(val.properties.dur.value, 250);
    assert.equal(val.properties.dur.unit, "ms");
    assert.ok(val.properties.size instanceof PklDataSize);
    assert.equal(val.properties.size.value, 64);
    assert.equal(val.properties.size.unit, "kib");
    assert.ok(val.properties.pair instanceof PklPair);
    assert.equal(val.properties.pair.first, "a");
    assert.equal(val.properties.pair.second, 7n);
    assert.ok(val.properties.rx instanceof PklRegex);
    assert.equal(val.properties.rx.pattern, "^[a-z]+$");
    assert.deepEqual(Array.from(val.properties.raw), [1, 2, 255]);
    assert.deepEqual(val.properties.items, [1n, 2n, 3n]);
    assert.ok(val.properties.uniq instanceof Set);
    assert.deepEqual(Array.from(val.properties.uniq), ["x", "y"]);
    assert.ok(val.properties.dict instanceof Map);
    assert.equal(val.properties.dict.get("k"), 99n);

    assert.equal(await ev.evaluateExpression(src, "name"), "pkl-napi");
    assert.equal(await ev.evaluateExpressionValue(src, "count"), MAX_I64);

    const pklBin = await ev.evaluateExpressionPklBinary(src, "pair");
    assert.ok(pklBin instanceof Uint8Array);
    const decodedPair = decodePklBinary(pklBin);
    assert.ok(decodedPair instanceof PklPair);
    assert.equal(decodedPair.first, "a");
    assert.equal(decodedPair.second, 7n);
  } finally {
    await ev.close();
  }
});

await test("default env uses process.env", async () => {
  process.env.PKL_NAPI_TEST_ENV_KEY = "hello-from-node-env";
  const ev = await createEvaluator();
  try {
    const out = await ev.evaluateExpression(
      ModuleSource.text('v = read("env:PKL_NAPI_TEST_ENV_KEY")'),
      "v",
    );
    assert.equal(out, "hello-from-node-env");
  } finally {
    delete process.env.PKL_NAPI_TEST_ENV_KEY;
    await ev.close();
  }
});

await test("evaluateSchema, evaluateTest, format, and ReplSession", async () => {
  const ev = await createEvaluator({ powerAssertions: true });
  try {
    const schemaSrc = ModuleSource.text(`
      /// Sample module doc
      module example.Config

      typealias Port = Int(isBetween(1, 65535))

      /// Server definition
      class Server {
        host: String
        port: Port = 8080
      }

      server: Server
    `);
    const schema = await ev.evaluateSchema(schemaSrc);
    assert.equal(schema.moduleName, "example.Config");
    assert.equal(schema.shortModuleName, "Config");
    assert.ok("Server" in schema.classes);
    assert.ok("Port" in schema.typeAliases);
    assert.equal(schema.classes.Server.properties.host.type.classInfo.simpleName, "String");

    const testSrc = ModuleSource.text(`
      amends "pkl:test"
      facts {
        ["math"] {
          1 + 1 == 2
          2 * 3 == 7
        }
      }
    `);
    const testRes = await ev.evaluateTest(testSrc, { reporter: "spec" });
    assert.equal(testRes.failed, true);
    assert.equal(testRes.totalTests, 1);
    assert.equal(testRes.totalFailures, 1);
    assert.equal(testRes.totalAsserts, 2);
    assert.equal(testRes.totalAssertsFailed, 1);
    assert.ok(testRes.rendered.includes("2 * 3 == 7"));
    assert.ok(testRes.rendered.includes("tests pass"));
    assert.ok(testRes.rendered.includes("asserts pass"));

    const junitRes = await ev.evaluateTest(testSrc, { reporter: "junit" });
    assert.ok(junitRes.rendered.includes("<testsuite"));
  } finally {
    await ev.close();
  }

  const formatted = format("a=1\nb =   2\n", { grammarVersion: 2 });
  assert.equal(formatted, "a = 1\nb = 2\n");
  assert.throws(
    () => format("a = ", { grammarVersion: 2 }),
    (err) => err instanceof PklError && err.kind === "evaluation",
  );
  assert.throws(
    () => format("a = 1", { grammarVersion: 99 }),
    (err) => err instanceof PklError && err.kind === "invalid-argument",
  );

  const repl = await createRepl();
  try {
    const r1 = await repl.eval("x = 21");
    assert.ok(Array.isArray(r1));
    const r2 = await repl.eval("x * 2");
    assert.equal(r2[0].kind, "evalSuccess");
    assert.equal(r2[0].result, "42");
    const comps = await repl.complete("x");
    assert.ok(Array.isArray(comps));
    assert.ok(comps.includes("isEven"));
    await repl.reset();
    const r3 = await repl.eval("x");
    assert.equal(r3[0].kind, "evalError");
  } finally {
    await repl.close();
  }
  assert.equal(repl.isClosed, true);
  await assert.rejects(
    () => repl.eval("1 + 1"),
    (err) => err instanceof PklError && err.kind === "closed",
  );
});

await test("filesystem, rootDir security, Project.load, and analyzeImportGraph", async () => {
  const dir = mkdtempSync(join(tmpdir(), "pkl-napi-test-"));
  try {
    const libPath = join(dir, "lib.pkl");
    const mainPath = join(dir, "main.pkl");
    const outsideDir = mkdtempSync(join(tmpdir(), "pkl-napi-out-"));
    const outsideFile = join(outsideDir, "secret.txt");
    writeFileSync(outsideFile, "top-secret");
    writeFileSync(libPath, 'greeting = "hello from file"');
    writeFileSync(mainPath, 'import "lib.pkl"\nimport "pkl:math"\nmsg = lib.greeting\nmaxVal = math.max(1, 2)\n');

    const projFile = join(dir, "PklProject");
    writeFileSync(
      projFile,
      `amends "pkl:Project"
package {
  name = "demo"
  baseUri = "package://example.com/demo"
  version = "1.0.0"
  packageZipUrl = "https://example.com/demo@1.0.0.zip"
}
evaluatorSettings {
  externalProperties {
    ["fromProject"] = "yes"
  }
}
`,
    );

    const project = await Project.load(dir);
    assert.equal(project.package.name, "demo");
    assert.equal(project.package.version, "1.0.0");
    assert.equal(project.evaluatorSettings.externalProperties.fromProject, "yes");
    const projectSync = Project.loadSync(projFile);
    assert.equal(projectSync.package.name, "demo");

    const ev = await createEvaluator({ project, rootDir: dir });
    try {
      const msg = await ev.evaluateExpression(ModuleSource.path(mainPath), "msg");
      assert.equal(msg, "hello from file");
      const fromProj = await ev.evaluateExpression(
        ModuleSource.text('v = read("prop:fromProject")'),
        "v",
      );
      assert.equal(fromProj, "yes");

      const outsideUri = ModuleSource.path(outsideFile).uri;
      await assert.rejects(
        () => ev.evaluateOutputText(ModuleSource.text(`s = read(${JSON.stringify(outsideUri)})`)),
        (err) => err instanceof PklError && err.kind === "security",
      );
    } finally {
      await ev.close();
    }

    const graph = await analyzeImportGraph([ModuleSource.path(mainPath).uri]);
    const mainUri = ModuleSource.path(mainPath).uri;
    const libUri = ModuleSource.path(libPath).uri;
    assert.ok(mainUri in graph.imports);
    assert.ok(graph.imports[mainUri].includes(libUri));
    assert.ok(graph.imports[mainUri].includes("pkl:math"));
    assert.deepEqual(graph.imports["pkl:math"], []);
    assert.equal(graph.resolvedImports["pkl:math"], "pkl:math");
    rmSync(outsideDir, { recursive: true, force: true });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

await test("custom async JS moduleReaders, resourceReaders, logger, frameTransformer, securityManager", async () => {
  const logs = [];
  const ev = await createEvaluator({
    logger: {
      trace(msg, frame) {
        logs.push({ level: "trace", msg, uri: frame?.moduleUri });
      },
      warn(msg, frame) {
        logs.push({ level: "warn", msg, uri: frame?.moduleUri });
      },
    },
    frameTransformer: async (frame) => ({
      ...frame,
      moduleUri: `transformed:${frame.moduleUri}`,
    }),
    moduleReaders: [
      {
        scheme: "mem",
        isLocal: true,
        isGlobbable: true,
        hasHierarchicalUris: true,
        async read(uri) {
          await new Promise((r) => setTimeout(r, 5));
          if (uri === "mem:/a.pkl") return 'val = "from-async-mem-a"';
          if (uri === "mem:/b.pkl") return 'val = "from-async-mem-b"';
          throw new Error(`not found: ${uri}`);
        },
        async hasElement(uri) {
          return uri === "mem:/a.pkl" || uri === "mem:/b.pkl" || uri === "mem:/";
        },
        async listElements(uri) {
          if (uri === "mem:/") {
            return [
              { name: "a.pkl", isDirectory: false },
              { name: "b.pkl", isDirectory: false },
            ];
          }
          return [];
        },
      },
    ],
    resourceReaders: [
      {
        scheme: "kv",
        hasHierarchicalUris: false,
        async read(uri) {
          await new Promise((r) => setTimeout(r, 5));
          if (uri === "kv:bytes") return new Uint8Array([10, 20, 30]);
          if (uri === "kv:text") return "hello-kv";
          return null;
        },
      },
    ],
    securityManager: {
      async checkReadResource(uri) {
        if (uri === "kv:forbidden") {
          throw new PklError("Custom security denied kv:forbidden", "security");
        }
      },
    },
  });

  try {
    const res = await ev.evaluateOutputValue(
      ModuleSource.text(`
        import "mem:/a.pkl"
        import* "mem:/*.pkl" as allMem
        t = trace("hello-log")
        txt = read("kv:text")
        bin = read("kv:bytes").bytes
        missing = read?("kv:missing")
        aVal = a.val
        globKeys = allMem.keys.toList()
      `),
    );
    assert.equal(res.properties.aVal, "from-async-mem-a");
    assert.equal(res.properties.txt, "hello-kv");
    assert.deepEqual(Array.from(res.properties.bin), [10, 20, 30]);
    assert.equal(res.properties.missing, null);
    assert.deepEqual(res.properties.globKeys, ["mem:/a.pkl", "mem:/b.pkl"]);
    assert.ok(
      logs.some((l) => l.level === "trace" && l.msg.includes("hello-log") && l.uri === "transformed:repl:text"),
      `Expected transformed frame URI on trace log, got: ${JSON.stringify(logs)}`,
    );

    await assert.rejects(
      () => ev.evaluateOutputText(ModuleSource.text('x = read("kv:forbidden")')),
      (err) => err instanceof PklError && err.message.includes("Custom security denied kv:forbidden"),
    );

    await assert.rejects(
      () => ev.evaluateOutputText(ModuleSource.text('x = throw("boom")')),
      (err) => err instanceof PklError && err.message.includes("transformed:repl:text"),
    );
  } finally {
    await ev.close();
  }
});

await test("non-blocking event loop, 64-bit timeouts, AbortSignal, cancel(), and idempotent close()", async () => {
  // 64-bit monotonic clock
  const t0 = monotonicClockMs();
  assert.equal(typeof t0, "bigint");
  assert.ok(t0 >= 0n);

  // 64-bit timeout validation (> 2^32 - 1 accepted; invalid rejected)
  for (const bad of [-1, 1.5, NaN, Infinity, -1n, MAX_U64 + 1n]) {
    await assert.rejects(
      () => createEvaluator({ timeoutMs: bad }),
      (err) => err instanceof PklError && err.kind === "invalid-argument",
    );
  }

  const bigTimeoutEv = await createEvaluator({ timeoutMs: 5_000_000_000n });
  try {
    assert.equal(
      await bigTimeoutEv.evaluateExpression(ModuleSource.text("x = 1 + 2"), "x"),
      "3",
    );
  } finally {
    await bigTimeoutEv.close();
  }

  const maxU64Ev = await createEvaluator({ timeoutMs: MAX_U64 });
  try {
    assert.equal(await maxU64Ev.evaluateExpression(ModuleSource.text("x = 7"), "x"), "7");
  } finally {
    await maxU64Ev.close();
  }

  // Non-blocking event loop proof: setInterval MUST keep ticking while Rust runs CPU work
  const cpuEv = await createEvaluator();
  try {
    let ticks = 0;
    const timer = setInterval(() => {
      ticks += 1;
    }, 5);
    try {
      const fib = await cpuEv.evaluateExpression(
        ModuleSource.text(`
          function fib(n: Int): Int = if (n <= 1) n else fib(n - 1) + fib(n - 2)
          ans = fib(29)
        `),
        "ans",
      );
      assert.equal(fib, "514229");
    } finally {
      clearInterval(timer);
    }
    assert.ok(ticks >= 2, `Expected Node event loop to tick during evaluation, got ticks=${ticks}`);
  } finally {
    await cpuEv.close();
  }

  // Real timeout aborting a hanging async custom reader Promise
  const timeoutEv = await createEvaluator({
    timeoutMs: 60,
    moduleReaders: [
      {
        scheme: "hang",
        read() {
          return new Promise(() => {});
        },
      },
    ],
  });
  try {
    await assert.rejects(
      () => timeoutEv.evaluateOutputText(ModuleSource.text('import "hang:/forever" as h\nx = h.a')),
      (err) => err instanceof PklError && err.kind === "timeout",
    );
  } finally {
    await timeoutEv.close();
  }

  // Pre-aborted AbortSignal rejects immediately on evaluate* with kind: "cancelled"
  const preAborted = new AbortController();
  preAborted.abort();
  const preEv = await createEvaluator({ signal: preAborted.signal });
  try {
    await assert.rejects(
      () => preEv.evaluateOutputText(ModuleSource.text("a = 1")),
      (err) => err instanceof PklError && err.kind === "cancelled" && err.code === "cancelled",
    );
  } finally {
    await preEv.close();
  }

  // Mid-flight AbortSignal aborting a hanging async reader
  const midAc = new AbortController();
  const midEv = await createEvaluator({
    signal: midAc.signal,
    moduleReaders: [
      {
        scheme: "hang2",
        read() {
          return new Promise(() => {});
        },
      },
    ],
  });
  try {
    setTimeout(() => midAc.abort(), 30);
    await assert.rejects(
      () => midEv.evaluateOutputText(ModuleSource.text('import "hang2:/wait" as h\nx = h.a')),
      (err) => err instanceof PklError && err.kind === "cancelled",
    );
  } finally {
    await midEv.close();
  }

  // Synchronous race-free cancel() right after starting evaluation
  const cancelEv = await createEvaluator();
  try {
    const pending = cancelEv.evaluateExpression(
      ModuleSource.text(`
        function fib(n: Int): Int = if (n <= 1) n else fib(n - 1) + fib(n - 2)
        ans = fib(32)
      `),
      "ans",
    );
    cancelEv.cancel();
    await assert.rejects(
      () => pending,
      (err) => err instanceof PklError && err.kind === "cancelled",
    );
  } finally {
    await cancelEv.close();
  }

  // Idempotent close() + closed rejection
  assert.equal(cancelEv.isClosed, true);
  await cancelEv.close();
  await assert.rejects(
    () => cancelEv.evaluateOutputText(ModuleSource.text("x = 1")),
    (err) => err instanceof PklError && err.kind === "closed",
  );
});

await test("lossless BigInt, Uint8Array, UTF-16 surrogates, roundtripValue, and measureConversion", async () => {
  assert.equal(roundtripInteger(MIN_I64), MIN_I64);
  assert.equal(roundtripInteger(MAX_I64), MAX_I64);
  assert.equal(roundtripInteger(0n), 0n);
  assert.throws(
    () => roundtripInteger(MAX_I64 + 1n),
    (err) => err instanceof PklError && err.kind === "invalid-argument",
  );
  assert.throws(
    () => roundtripInteger(MIN_I64 - 1n),
    (err) => err instanceof PklError && err.kind === "invalid-argument",
  );

  // Owned Uint8Array copy (no buffer aliasing)
  const inputBytes = new Uint8Array([1, 2, 3, 4]);
  const outBytes = roundtripBytes(inputBytes);
  inputBytes[0] = 99;
  assert.equal(outBytes[0], 1);
  outBytes[1] = 88;
  assert.equal(inputBytes[1], 2);

  // Lossless UTF-16 including unpaired surrogates
  const tricky = "a\uD800b\uDC00c🚀";
  assert.equal(roundtripString(tricky), tricky);

  // Full typed value roundtrip
  const complex = new PklObject(
    "my.Mod#Item",
    "repl:test",
    {
      big: MIN_I64,
      neg0: -0,
      nan: NaN,
      str: tricky,
      bytes: new Uint8Array([7, 8, 9]),
      dur: new PklDuration(12.5, "s"),
      size: new PklDataSize(4, "gib"),
      pair: new PklPair(1n, "two"),
      rx: new PklRegex("foo.*"),
      cls: new PklClass("pkl.base#String", "pkl:base"),
      alias: new PklTypeAlias("my.Mod#Port", "repl:test"),
      seq: new PklIntSeq(1n, 10n, 2n),
      fn: new PklFunction(),
      ref: new PklReference("dom", 42n, ["a", 0n]),
      set: new Set([1n, "x"]),
      map: new Map([
        ["k", 123n],
        [2n, "v"],
      ]),
    },
    new Map([["entryKey", "entryVal"]]),
    [10n, 20n],
  );

  const rt = roundtripValue(complex);
  assert.ok(rt instanceof PklObject);
  assert.equal(rt.className, "my.Mod#Item");
  assert.equal(rt.properties.big, MIN_I64);
  assert.ok(Object.is(rt.properties.neg0, -0));
  assert.ok(Number.isNaN(rt.properties.nan));
  assert.equal(rt.properties.str, tricky);
  assert.deepEqual(Array.from(rt.properties.bytes), [7, 8, 9]);
  assert.ok(rt.properties.dur instanceof PklDuration);
  assert.ok(rt.properties.size instanceof PklDataSize);
  assert.ok(rt.properties.pair instanceof PklPair);
  assert.ok(rt.properties.rx instanceof PklRegex);
  assert.ok(rt.properties.cls instanceof PklClass);
  assert.ok(rt.properties.alias instanceof PklTypeAlias);
  assert.ok(rt.properties.seq instanceof PklIntSeq);
  assert.equal(rt.properties.seq.step, 2n);
  assert.ok(rt.properties.fn instanceof PklFunction);
  assert.ok(rt.properties.ref instanceof PklReference);
  assert.equal(rt.entries.get("entryKey"), "entryVal");
  assert.deepEqual(rt.elements, [10n, 20n]);

  // Lazy PklObject empty entries/elements remain isolated mutable containers per instance
  const lazyA = new PklObject("Lazy#A", "repl:a", { x: 1n });
  const lazyB = new PklObject("Lazy#A", "repl:a", { x: 1n });
  assert.deepStrictEqual(lazyA, lazyB);
  assert.ok(lazyA.entries instanceof Map);
  assert.ok(Array.isArray(lazyA.elements));
  lazyA.entries.set("k", 99n);
  lazyA.elements.push("item");
  assert.equal(lazyB.entries.size, 0);
  assert.equal(lazyB.elements.length, 0);
  const lazyRt = roundtripValue(lazyA);
  assert.equal(lazyRt.entries.get("k"), 99n);
  assert.deepEqual(lazyRt.elements, ["item"]);

  const bench = measureConversion(200);
  assert.equal(bench.iterations, 200);
  assert.ok(bench.elapsedMs > 0);
  assert.ok(bench.perIterationUs > 0);
  assert.ok(bench.lastValue instanceof PklObject);
});

await test("external reader subprocess (0x26-0x32) with zero orphan PIDs on close", async () => {
  const dir = mkdtempSync(join(tmpdir(), "pkl-napi-extreader-"));
  const pidFile = join(dir, "reader.pid");
  const scriptPath = join(dir, "reader.mjs");
  // Minimal MessagePack external resource reader supporting 0x26..0x32 (initialize + read + close).
  writeFileSync(
    scriptPath,
    `import { writeFileSync } from "node:fs";
writeFileSync(${JSON.stringify(pidFile)}, String(process.pid));

function encodeStr(s) {
  const b = Buffer.from(s, "utf8");
  if (b.length < 32) return Buffer.concat([Buffer.from([0xa0 | b.length]), b]);
  return Buffer.concat([Buffer.from([0xd9, b.length]), b]);
}
function encodeBin(b) {
  return Buffer.concat([Buffer.from([0xc4, b.length]), b]);
}
function encodeUint(n) {
  if (n < 128) return Buffer.from([n]);
  if (n < 256) return Buffer.from([0xcc, n]);
  const buf = Buffer.alloc(5);
  buf[0] = 0xce;
  buf.writeUInt32BE(n, 1);
  return buf;
}
function encodeMessage(code, body) {
  const entries = Object.entries(body);
  const parts = [Buffer.from([0x92, code, 0x80 | entries.length])];
  for (const [k, v] of entries) {
    parts.push(encodeStr(k));
    if (typeof v === "boolean") parts.push(Buffer.from([v ? 0xc3 : 0xc2]));
    else if (typeof v === "number") parts.push(encodeUint(v));
    else if (typeof v === "string") parts.push(encodeStr(v));
    else if (Buffer.isBuffer(v)) parts.push(encodeBin(v));
  }
  return Buffer.concat(parts);
}

let acc = Buffer.alloc(0);
function decodeOne(buf, off) {
  if (off >= buf.length) return null;
  const b = buf[off];
  if (b <= 0x7f) return [b, off + 1];
  if (b >= 0x90 && b <= 0x9f) {
    const n = b & 0x0f;
    let p = off + 1;
    const arr = [];
    for (let i = 0; i < n; i++) {
      const r = decodeOne(buf, p);
      if (!r) return null;
      arr.push(r[0]);
      p = r[1];
    }
    return [arr, p];
  }
  if (b >= 0x80 && b <= 0x8f) {
    const n = b & 0x0f;
    let p = off + 1;
    const obj = {};
    for (let i = 0; i < n; i++) {
      const kr = decodeOne(buf, p);
      if (!kr) return null;
      const vr = decodeOne(buf, kr[1]);
      if (!vr) return null;
      obj[kr[0]] = vr[0];
      p = vr[1];
    }
    return [obj, p];
  }
  if (b >= 0xa0 && b <= 0xbf) {
    const len = b & 0x1f;
    if (off + 1 + len > buf.length) return null;
    return [buf.subarray(off + 1, off + 1 + len).toString("utf8"), off + 1 + len];
  }
  if (b === 0xd9) {
    if (off + 2 > buf.length) return null;
    const len = buf[off + 1];
    if (off + 2 + len > buf.length) return null;
    return [buf.subarray(off + 2, off + 2 + len).toString("utf8"), off + 2 + len];
  }
  if (b === 0xcc) {
    if (off + 2 > buf.length) return null;
    return [buf[off + 1], off + 2];
  }
  if (b === 0xcd) {
    if (off + 3 > buf.length) return null;
    return [buf.readUInt16BE(off + 1), off + 3];
  }
  if (b === 0xce) {
    if (off + 5 > buf.length) return null;
    return [buf.readUInt32BE(off + 1), off + 5];
  }
  if (b === 0xcf) {
    if (off + 9 > buf.length) return null;
    return [Number(buf.readBigUInt64BE(off + 1)), off + 9];
  }
  if (b === 0xc2) return [false, off + 1];
  if (b === 0xc3) return [true, off + 1];
  throw new Error("Unexpected msgpack tag: 0x" + b.toString(16));
}

process.stdin.on("data", (chunk) => {
  acc = Buffer.concat([acc, chunk]);
  while (acc.length > 0) {
    const decoded = decodeOne(acc, 0);
    if (!decoded) break;
    const [[code, body], nextOff] = decoded;
    acc = acc.subarray(nextOff);
    if (code === 0x30) {
      // InitializeResourceReaderRequest (0x30) -> InitializeResourceReaderResponse (0x31)
      const resp = Buffer.concat([
        Buffer.from([0x92, 0x31, 0x82]),
        encodeStr("requestId"), encodeUint(body.requestId),
        encodeStr("spec"), Buffer.from([0x83]),
        encodeStr("scheme"), encodeStr(body.scheme),
        encodeStr("hasHierarchicalUris"), Buffer.from([0xc2]),
        encodeStr("isGlobbable"), Buffer.from([0xc2]),
      ]);
      process.stdout.write(resp);
    } else if (code === 0x26) {
      // ReadResourceRequest (0x26) -> ReadResourceResponse (0x27)
      process.stdout.write(encodeMessage(0x27, {
        requestId: body.requestId,
        evaluatorId: body.evaluatorId,
        contents: Buffer.from("ext-payload:" + body.uri, "utf8"),
      }));
    } else if (code === 0x32) {
      process.exit(0);
    }
  }
});
`,
  );

  try {
    const ev = await createEvaluator({
      externalResourceReaders: {
        exttest: {
          executable: process.execPath,
          arguments: [scriptPath],
        },
      },
    });
    try {
      const out = await ev.evaluateExpression(
        ModuleSource.text('v = read("exttest:hello").text'),
        "v",
      );
      assert.equal(out, "ext-payload:exttest:hello");
    } finally {
      await ev.close();
    }

    assert.ok(existsSync(pidFile), "external reader PID file should exist");
    const childPid = Number(readFileSync(pidFile, "utf8").trim());
    assert.ok(Number.isInteger(childPid) && childPid > 0);
    // Verify subprocess exited cleanly (no orphan PID)
    let alive = true;
    try {
      process.kill(childPid, 0);
    } catch (err) {
      if (err.code === "ESRCH") alive = false;
    }
    assert.equal(alive, false, `External reader PID ${childPid} leaked after close()`);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

// Optional loopback HTTP check (passes whenever localhost bind is permitted)
await test("native HTTP in same Node process (non-blocking loopback)", async () => {
  const server = http.createServer((req, res) => {
    if (req.url === "/mod.pkl") {
      const hdr = req.headers["x-pkl-token"] || "missing";
      res.writeHead(200, { "content-type": "text/plain" });
      res.end(`fromHttp = "ok:${hdr}"\n`);
      return;
    }
    res.writeHead(404);
    res.end("not found");
  });

  let bound = false;
  try {
    await new Promise((resolveListen, rejectListen) => {
      server.once("error", rejectListen);
      server.listen(0, "127.0.0.1", () => {
        bound = true;
        resolveListen();
      });
    });
  } catch (err) {
    if (err && (err.code === "EPERM" || err.code === "EACCES")) {
      console.log("  [note] loopback HTTP listen blocked by sandbox; will verify unsandboxed");
      return;
    }
    throw err;
  }

  try {
    const { port } = server.address();
    const baseUrl = `http://127.0.0.1:${port}`;
    const ev = await createEvaluator({
      allowedModules: ["pkl:", "repl:", "http:"],
      http: {
        headers: {
          [`${baseUrl}/*`]: { "X-Pkl-Token": "abc999" },
        },
        rewrites: {
          "http://virtual.local/": `${baseUrl}/`,
        },
      },
    });
    try {
      const out = await ev.evaluateExpression(
        ModuleSource.uri("http://virtual.local/mod.pkl"),
        "fromHttp",
      );
      assert.equal(out, "ok:abc999");
    } finally {
      await ev.close();
    }
  } finally {
    if (bound) {
      await new Promise((r) => server.close(r));
    }
  }
});

await test("Wave 2 security/stability regressions: decoder DoS, cyclic values, __proto__ schema", async () => {
  // 1. P1-16: Truncated 4B-element List header rejects cleanly without OOM preallocation.
  const hugeList = new Uint8Array([0x92, 0x04, 0xdd, 0x7f, 0xff, 0xff, 0xff]);
  assert.throws(
    () => decodePklBinary(hugeList),
    (e) => e instanceof PklError && e.kind === "decode",
  );

  // 2. P1-18: Cyclic JS objects reject cleanly instead of overflowing the stack.
  const cyclic = [];
  cyclic.push(cyclic);
  assert.throws(
    () => roundtripValue(cyclic),
    (e) => e instanceof PklError && e.kind === "invalid-argument",
  );

  // 3. P1-19: `__proto__` property in schema is defined as an own property without prototype mutation.
  const ev = await createEvaluator();
  try {
    const schema = await ev.evaluateSchema(ModuleSource.text("`__proto__`: Int = 42\n"));
    const props = schema.moduleClass.properties;
    assert.equal(Object.getPrototypeOf(props), Object.prototype);
    assert.ok(Object.prototype.hasOwnProperty.call(props, "__proto__"));
    assert.equal(props["__proto__"].simpleName, "__proto__");
  } finally {
    await ev.close();
  }
});

await test("Round 2 regressions: PklObject reassignment, single-call cancel() health, invalid options", async () => {
  // 1. PklObject entries/elements setters can be reassigned repeatedly and survive roundtripValue.
  const obj = new PklObject("Mod#Obj", "repl:r2", { a: 1n });
  obj.entries = new Map([["k1", 10n]]);
  obj.entries = new Map([["k2", 20n]]);
  obj.elements = [1n, 2n];
  obj.elements = [3n];
  assert.equal(obj.entries.get("k2"), 20n);
  assert.equal(obj.entries.has("k1"), false);
  assert.deepEqual(obj.elements, [3n]);
  const rt = roundtripValue(obj);
  assert.equal(rt.entries.get("k2"), 20n);
  assert.deepEqual(rt.elements, [3n]);

  // 2. Single-call cancel() rejects only the cancelled evaluation without poisoning subsequent calls.
  const ev = await createEvaluator();
  try {
    const pending = ev.evaluateExpression(
      ModuleSource.text(`
        function fib(n: Int): Int = if (n <= 1) n else fib(n - 1) + fib(n - 2)
        ans = fib(32)
      `),
      "ans",
    );
    ev.cancel();
    await assert.rejects(
      () => pending,
      (err) => err instanceof PklError && err.kind === "cancelled",
    );
    const after = await ev.evaluateExpression(ModuleSource.text("x = 20 + 22"), "x");
    assert.equal(after, "42");
  } finally {
    await ev.close();
  }

  // 3. Invalid http.testPort and externalResourceReaders reject cleanly with PklError.
  await assert.rejects(
    () => createEvaluator({ http: { testPort: -1 } }),
    (err) => err instanceof PklError && err.kind === "invalid-argument",
  );
  await assert.rejects(
    () => createEvaluator({ externalResourceReaders: { bad: {} } }),
    (err) => err instanceof PklError && err.kind === "invalid-argument",
  );
});

await test("Round 3 regressions: API unification, non-closing timeout/cancel, BOM, Uint8Array reader, strict decode", async () => {
  // 1. ModuleSource.modulePath normalizes bare `modulepath:` prefix & empty cacheDir rejects.
  assert.equal(ModuleSource.modulePath("modulepath:foo/bar.pkl").uri, "modulepath:/foo/bar.pkl");
  assert.equal(ModuleSource.modulePath("modulepath:/foo/bar.pkl").uri, "modulepath:/foo/bar.pkl");
  await assert.rejects(
    () => createEvaluator({ cacheDir: "" }),
    (err) => err instanceof PklError && err.kind === "configuration",
  );

  // 2. Hardened PklObject: empty collection identity, frozen getters, setter validation, toJSON, undefined rejection.
  const emptyMap = new Map();
  const emptyArr = [];
  const obj = new PklObject("Mod#Obj", "repl:r3", { a: 1n }, emptyMap, emptyArr);
  assert.equal(obj.entries, emptyMap);
  assert.equal(obj.elements, emptyArr);
  assert.deepEqual(obj.toJSON(), {
    className: "Mod#Obj",
    moduleUri: "repl:r3",
    properties: { a: 1n },
    entries: [],
    elements: [],
  });
  const frozen = Object.freeze(new PklObject("Mod#Frozen", "repl:r3", {}));
  assert.ok(frozen.entries instanceof Map);
  assert.ok(Array.isArray(frozen.elements));
  assert.throws(
    () => {
      obj.entries = "bad";
    },
    (e) => e instanceof PklError && e.kind === "invalid-argument",
  );
  assert.throws(
    () => {
      obj.elements = {};
    },
    (e) => e instanceof PklError && e.kind === "invalid-argument",
  );
  assert.throws(
    () => roundtripValue(undefined),
    (e) => e instanceof PklError && e.kind === "invalid-argument",
  );

  // 3. Unified measureConversion overloads & superset fields.
  const m1 = measureConversion(5);
  assert.equal(m1.iterations, 5);
  assert.equal(typeof m1.totalMs, "number");
  assert.equal(typeof m1.elapsedMs, "number");
  assert.equal(typeof m1.avgUs, "number");
  assert.equal(typeof m1.perIterationUs, "number");
  assert.equal(typeof m1.opsPerSec, "number");
  assert.ok(m1.sample !== undefined && m1.lastValue !== undefined);
  const m2 = measureConversion(42n, 5);
  assert.equal(m2.iterations, 5);
  assert.equal(m2.sample, 42n);
  assert.equal(m2.lastValue, 42n);

  // 4. Unified schema shape + BOM preservation + strict decode + owned binary fast-path + typed security error.
  const ev = await createEvaluator();
  try {
    const schema = await ev.evaluateSchema(
      ModuleSource.text(`
        module example.Unified
        typealias Port = Int(isBetween(1, 65535))
        class Server { host: String }
      `),
    );
    assert.equal(schema.classes.Server.info.simpleName, "Server");
    assert.equal(schema.classes.Server.moduleUri, schema.classes.Server.info.moduleUri);
    assert.equal(schema.typeAliases.Port.info.simpleName, "Port");

    // Leading UTF-8 BOM U+FEFF must be preserved in long (>64B) strings.
    const longBom = await ev.evaluateExpressionValue(
      ModuleSource.text(`s = "\\u{feff}" + "a".repeat(80)\n`),
      "s",
    );
    assert.equal(longBom.charCodeAt(0), 0xfeff);
    assert.equal(longBom.length, 81);

    // decodePklBinary({ strict: true }) rejects Function while default lossless decodes PklFunction.
    const fnBytes = await ev.evaluateExpressionPklBinary(ModuleSource.text("f = (x) -> x + 1\n"), "f");
    assert.ok(decodePklBinary(fnBytes) instanceof PklFunction);
    assert.throws(
      () => decodePklBinary(fnBytes, { strict: true }),
      (e) => e instanceof PklError && e.kind === "decode",
    );

    // Fast-path evaluateExpressionPklBinary returns owned Uint8Array (not a Node Buffer slab view).
    const warmSrc = ModuleSource.text("x = 42\n", "file:///owned_bin_check.pkl");
    await ev.evaluateExpressionPklBinary(warmSrc, "x");
    const warmBytes = await ev.evaluateExpressionPklBinary(warmSrc, "x");
    assert.equal(warmBytes.byteOffset, 0);
    assert.equal(warmBytes.buffer.byteLength, warmBytes.byteLength);

    // Substring "Refusing to " in user throw() must stay kind === 'evaluation', not 'security'.
    await assert.rejects(
      () => ev.evaluateOutputText(ModuleSource.text('x = throw("Refusing to eat broccoli")\n')),
      (e) => e instanceof PklError && e.kind === "evaluation",
    );
  } finally {
    await ev.close();
  }

  // 5. ModuleReader returning Uint8Array (with BOM) and logger error propagation.
  const enc = new TextEncoder();
  const readerEv = await createEvaluator({
    allowedModules: ["pkl:", "repl:", "mem:"],
    moduleReaders: [
      {
        scheme: "mem",
        isLocal: true,
        hasHierarchicalUris: true,
        hasFragmentPaths: false,
        read() {
          return enc.encode('msg = "from-bytes"\n');
        },
      },
    ],
  });
  try {
    const v = await readerEv.evaluateExpression(
      ModuleSource.text('import "mem:/a.pkl" as m\nv = m.msg\n'),
      "v",
    );
    assert.equal(v, "from-bytes");
  } finally {
    await readerEv.close();
  }

  const throwingLogEv = await createEvaluator({
    logger: {
      trace() {
        throw new Error("boom-from-logger");
      },
    },
  });
  try {
    await assert.rejects(
      () => throwingLogEv.evaluateExpression(ModuleSource.text("x = trace(42)\n"), "x"),
      (e) => e instanceof PklError && e.message.includes("boom-from-logger"),
    );
  } finally {
    await throwingLogEv.close();
  }

  // 6. Non-closing timeout reuse on Evaluator and ReplSession + ReplSession.cancel/evalDefinitions.
  const timeoutEv = await createEvaluator({ timeoutMs: 25 });
  try {
    await assert.rejects(
      () =>
        timeoutEv.evaluateExpression(
          ModuleSource.text("function fib(n: Int): Int = if (n <= 1) n else fib(n - 1) + fib(n - 2)\nx = fib(32)\n"),
          "x",
        ),
      (e) => e instanceof PklError && e.kind === "timeout",
    );
    assert.equal(timeoutEv.isClosed, false);
    assert.equal(
      await timeoutEv.evaluateExpression(ModuleSource.text("x = 20 + 22\n"), "x"),
      "42",
    );
  } finally {
    await timeoutEv.close();
  }

  const repl = await createRepl({ timeoutMs: 25 });
  try {
    await assert.rejects(
      () =>
        repl.eval("function fib(n: Int): Int = if (n <= 1) n else fib(n - 1) + fib(n - 2)\nfib(32)"),
      (e) => e instanceof PklError && e.kind === "timeout",
    );
    assert.equal(repl.isClosed, false);
    const okResps = await repl.eval("a = 10 + 5", { evalDefinitions: true });
    assert.equal(okResps[0].kind, "evalSuccess");
    assert.equal(okResps[0].result, "15");
    assert.equal(typeof repl.cancel, "function");

    // Round 4: N-API ReplSession.load(ModuleSource.text(...)) stages inline contents.
    const loadResps = await repl.load(ModuleSource.text("loadedVal = 99\n", "file:///ephemeral.pkl"));
    assert.ok(loadResps.every((r) => r.kind !== "evalError"));
    const readLoaded = await repl.eval("loadedVal");
    assert.equal(readLoaded[0].kind, "evalSuccess");
    assert.equal(readLoaded[0].result, "99");

    // Round 4: format(null) & invalid PklIntSeq step throw PklError.
    assert.throws(() => format(null), (e) => e instanceof PklError);
    assert.throws(() => new PklIntSeq(1, 5, 0), (e) => e instanceof PklError && e.kind === "invalid-argument");

    // Round 5 (P1-19): ReplSession.load(ModuleSource.text(...)) reloads fresh source without stale cache.
    const reload1 = await repl.load(ModuleSource.text("reloaded = 11\n", "file:///r5_reload.pkl"));
    assert.ok(reload1.every((r) => r.kind !== "evalError"));
    assert.equal((await repl.eval("reloaded"))[0].result, "11");
    const reload2 = await repl.load(ModuleSource.text("reloaded = 22\n", "file:///r5_reload.pkl"));
    assert.ok(reload2.every((r) => r.kind !== "evalError"));
    assert.equal((await repl.eval("reloaded"))[0].result, "22");
    await assert.rejects(
      () => repl.load(""),
      (e) => e instanceof PklError && e.kind === "invalid-argument",
    );
  } finally {
    await repl.close();
  }

  // Round 5 (P1-5): projectDir + custom securityManager fails closed with configuration error (no deadlock/bypass).
  await assert.rejects(
    () =>
      createEvaluator({
        projectDir: tmpdir(),
        securityManager: { checkResolveModule() {} },
      }),
    (e) => e instanceof PklError && e.kind === "configuration",
  );

  // Round 5 (P1-22 & P2-9): warm expression fast path preserves unpaired UTF-16 surrogates,
  // analyzeImportGraph AbortSignal reports cancelled, and PklReference / empty source.uri validate cleanly.
  const r5Ev = await createEvaluator();
  try {
    const surrSrc = ModuleSource.text('s = "ok"\n', "file:///r5_surrogate_warm.pkl");
    assert.equal(await r5Ev.evaluateExpression(surrSrc, "s"), "ok");
    const loneHigh = String.fromCharCode(0xd800);
    const warmSurr = await r5Ev.evaluateExpression(surrSrc, `"${loneHigh}"`);
    assert.equal(warmSurr.length, 1);
    assert.equal(warmSurr.charCodeAt(0), 0xd800);
    await assert.rejects(
      () => r5Ev.evaluateOutputText({ uri: "" }),
      (e) => e instanceof PklError && e.kind === "invalid-argument",
    );
  } finally {
    await r5Ev.close();
  }

  const ac = new AbortController();
  await assert.rejects(
    () =>
      analyzeImportGraph(["file:///r5_abort_graph.pkl"], {
        signal: ac.signal,
        securityManager: {
          async checkResolveModule() {
            ac.abort();
            throw new PklError("aborted during callback", "security");
          },
        },
      }),
    (e) => e instanceof PklError && e.kind === "cancelled",
  );

  assert.throws(
    () => new PklReference("d", 1n, null),
    (e) => e instanceof PklError && e.kind === "invalid-argument",
  );
});

console.log(JSON.stringify({ passed: results.length, results }, null, 2));
