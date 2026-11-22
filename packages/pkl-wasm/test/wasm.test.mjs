import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { dirname, join, resolve } from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

import {
  PKL_VERSION,
  PklClass,
  PklDataSize,
  PklDuration,
  PklError,
  PklFunction,
  PklIntSeq,
  PklObject,
  PklPair,
  PklRegex,
  PklTypeAlias,
  ModuleSource,
  VirtualFileSystem,
  analyzeImportGraph,
  capabilities,
  createEvaluator,
  createRepl,
  decodePklBinary,
  format,
  init,
  measureConversion,
  roundtripBytes,
  roundtripInteger,
  roundtripString,
  roundtripValue,
} from '../dist/index.mjs';

const PKG_DIR = resolve(dirname(fileURLToPath(import.meta.url)), '..');

function crc32(bytes) {
  let crc = 0xffffffff;
  for (let i = 0; i < bytes.length; i += 1) {
    crc ^= bytes[i];
    for (let j = 0; j < 8; j += 1) {
      crc = (crc & 1) !== 0 ? (crc >>> 1) ^ 0xedb88320 : crc >>> 1;
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function buildStoredZip(files) {
  const enc = new TextEncoder();
  const chunks = [];
  const central = [];
  let offset = 0;

  const u16 = (v) => new Uint8Array([v & 0xff, (v >>> 8) & 0xff]);
  const u32 = (v) =>
    new Uint8Array([v & 0xff, (v >>> 8) & 0xff, (v >>> 16) & 0xff, (v >>> 24) & 0xff]);

  const entries = Object.entries(files);
  for (const [name, content] of entries) {
    const nameBytes = enc.encode(name);
    const data = content instanceof Uint8Array ? content : enc.encode(content);
    const c = crc32(data);
    const localHeader = Buffer.concat([
      u32(0x04034b50),
      u16(20),
      u16(0),
      u16(0),
      u16(0),
      u16(0),
      u32(c),
      u32(data.length),
      u32(data.length),
      u16(nameBytes.length),
      u16(0),
      nameBytes,
      data,
    ]);
    chunks.push(localHeader);
    central.push(
      Buffer.concat([
        u32(0x02014b50),
        u16(20),
        u16(20),
        u16(0),
        u16(0),
        u16(0),
        u16(0),
        u32(c),
        u32(data.length),
        u32(data.length),
        u16(nameBytes.length),
        u16(0),
        u16(0),
        u16(0),
        u16(0),
        u32(0),
        u32(offset),
        nameBytes,
      ]),
    );
    offset += localHeader.length;
  }
  const cdBuf = Buffer.concat(central);
  const eocd = Buffer.concat([
    u32(0x06054b50),
    u16(0),
    u16(0),
    u16(entries.length),
    u16(entries.length),
    u32(cdBuf.length),
    u32(offset),
    u16(0),
  ]);
  return new Uint8Array(Buffer.concat([...chunks, cdBuf, eocd]));
}

test('metadata, capabilities, and idempotent init', async () => {
  assert.equal(PKL_VERSION, '0.32.0');
  const caps = capabilities();
  assert.equal(caps.vfs, true);
  assert.equal(caps.asyncModuleLoader, true);
  assert.equal(caps.asyncResourceLoader, true);
  assert.equal(caps.nativeFs, false);
  assert.equal(caps.nativeHttp, false);
  assert.equal(caps.externalReaders, false);
  assert.equal(caps.modulePathJar, false);
  assert.equal(caps.projectResolution, false);
  assert.equal(caps.packageCache, false);
  assert.equal(caps.formatter, true);
  assert.equal(caps.repl, true);
  assert.equal(caps.schema, true);
  assert.equal(caps.analyzeImports, true);
  assert.equal(caps.bigint64, true);
  assert.equal(caps.uint8ArrayBytes, true);
  assert.equal(caps.clock64Bit, true);
  assert.equal(caps.runtime, 'wasm-node');
  assert.equal(caps.arch, 'wasm32');
  assert.equal(caps.platform, process.platform);

  // Sync auto-init in Node works before explicit await init()
  assert.equal(format('x=1+2\n'), 'x = 1 + 2\n');
  assert.equal(format('x=1+2\n', { grammarVersion: 1 }), 'x = 1 + 2\n');

  // Idempotent init
  await init();
  await init();
});

test('all 9 output renderers and stdlib modules', async () => {
  const formats = [
    'pcf',
    'json',
    'yaml',
    'xml',
    'properties',
    'plist',
    'jsonnet',
    'textproto',
    'pkl-binary',
  ];
  const src = ModuleSource.text('a = 1\nb = "hello"\n');
  for (const fmt of formats) {
    const ev = await createEvaluator({ outputFormat: fmt });
    try {
      if (fmt === 'pkl-binary') {
        const bytes = await ev.evaluateOutputBytes(src);
        assert.ok(bytes instanceof Uint8Array);
        assert.ok(bytes.length > 0);
        const decoded = decodePklBinary(bytes);
        assert.ok(decoded instanceof PklObject);
        assert.equal(decoded.properties.a, 1n);
        assert.equal(decoded.properties.b, 'hello');
      } else {
        const text = await ev.evaluateOutputText(src);
        assert.equal(typeof text, 'string');
        assert.ok(text.length > 0);
      }
    } finally {
      ev.close();
    }
  }

  // Stdlib modules check
  const ev = await createEvaluator({ properties: { greeting: 'hi' }, env: { STAGE: 'prod' } });
  try {
    const expr = await ev.evaluateExpression(
      ModuleSource.text(`
import "pkl:math"
import "pkl:semver"
import "pkl:json"
v = semver.Version("1.2.3")
p = math.max(10, 42)
propVal = read("prop:greeting")
envVal = read("env:STAGE")
`),
      '"\\(v.major).\\(v.minor)-\\(p)-\\(propVal)-\\(envVal)"',
    );
    assert.equal(expr, '1.2-42-hi-prod');
  } finally {
    ev.close();
  }
});

test('output.files, output.value, expression values, and lossless boundary', async () => {
  const ev = await createEvaluator();
  try {
    const src = ModuleSource.text(`
output {
  files {
    ["b.txt"] { text = "second" }
    ["a.txt"] { text = "first" }
  }
}
bigMin = -9223372036854775808
bigMax = 9223372036854775807
dur = 5.s
sz = 4.mib
pair = Pair("k", 7)
re = Regex("a+b")
raw = Bytes(0, 127, 255)
seq = IntSeq(1, 5).step(2)
`);
    const files = await ev.evaluateOutputFiles(src);
    assert.ok(files instanceof Map);
    assert.deepEqual(Array.from(files.keys()), ['b.txt', 'a.txt']);
    assert.equal(new TextDecoder().decode(files.get('b.txt')), 'second');
    assert.equal(new TextDecoder().decode(files.get('a.txt')), 'first');

    assert.equal(await ev.evaluateExpressionValue(src, 'bigMin'), -9223372036854775808n);
    assert.equal(await ev.evaluateExpressionValue(src, 'bigMax'), 9223372036854775807n);
    const dur = await ev.evaluateExpressionValue(src, 'dur');
    assert.ok(dur instanceof PklDuration);
    assert.equal(dur.value, 5);
    assert.equal(dur.unit, 's');
    const sz = await ev.evaluateExpressionValue(src, 'sz');
    assert.ok(sz instanceof PklDataSize);
    assert.equal(sz.value, 4);
    assert.equal(sz.unit, 'mib');
    const pair = await ev.evaluateExpressionValue(src, 'pair');
    assert.ok(pair instanceof PklPair);
    assert.equal(pair.first, 'k');
    assert.equal(pair.second, 7n);
    const re = await ev.evaluateExpressionValue(src, 're');
    assert.ok(re instanceof PklRegex);
    assert.equal(re.pattern, 'a+b');
    const raw = await ev.evaluateExpressionValue(src, 'raw');
    assert.ok(raw instanceof Uint8Array);
    assert.deepEqual(Array.from(raw), [0, 127, 255]);
    const seq = await ev.evaluateExpressionValue(src, 'seq');
    assert.ok(seq instanceof PklIntSeq);
    assert.equal(seq.start, 1n);
    assert.equal(seq.end, 5n);
    assert.equal(seq.step, 2n);

    // PklClass, PklTypeAlias, PklFunction via decode_lossless
    const cls = await ev.evaluateExpressionValue(src, 'String');
    assert.ok(cls instanceof PklClass);
    const fn = await ev.evaluateExpressionValue(src, '(x) -> x + 1');
    assert.ok(fn instanceof PklFunction);

    // PklObject entries is a Map per contract #66
    const modVal = await ev.evaluateOutputValue(
      ModuleSource.text('output {\n  value = new Dynamic {\n    a = 1\n    ["k"] = "v"\n    "elem0"\n  }\n}\n'),
    );
    assert.ok(modVal instanceof PklObject);
    assert.equal(modVal.properties.a, 1n);
    assert.ok(modVal.entries instanceof Map);
    assert.equal(modVal.entries.get('k'), 'v');
    assert.deepEqual(modVal.elements, ['elem0']);
  } finally {
    ev.close();
  }

  // Lossless boundary round-trips & rejections
  assert.equal(roundtripInteger(-9223372036854775808n), -9223372036854775808n);
  assert.equal(roundtripInteger(9223372036854775807n), 9223372036854775807n);
  assert.throws(() => roundtripInteger(9223372036854775808n), (err) => {
    assert.ok(err instanceof PklError);
    assert.equal(err.kind, 'invalid-argument');
    return true;
  });
  assert.throws(() => roundtripInteger(-9223372036854775809n), (err) => {
    assert.ok(err instanceof PklError);
    assert.equal(err.kind, 'invalid-argument');
    return true;
  });

  // Unpaired UTF-16 surrogate & astral code points preserved losslessly
  const weird = 'a\uD800b🚀c\uDC00d';
  assert.equal(roundtripString(weird), weird);

  // Owned Uint8Array copy
  const orig = new Uint8Array([1, 2, 3]);
  const copy = roundtripBytes(orig);
  orig[0] = 99;
  assert.deepEqual(Array.from(copy), [1, 2, 3]);

  // Full value roundtrip + conversion benchmark
  const complex = new PklObject(
    'App#Config',
    'file:///app.pkl',
    { n: -0, inf: Infinity, nan: NaN, big: 9007199254740993n, ta: new PklTypeAlias('A', 'file:///a.pkl') },
    new Map([[1n, 'one']]),
    [new Set([1n, 2n])],
  );
  const rt = roundtripValue(complex);
  assert.ok(rt instanceof PklObject);
  assert.ok(Object.is(rt.properties.n, -0));
  assert.equal(rt.properties.inf, Infinity);
  assert.ok(Number.isNaN(rt.properties.nan));
  assert.equal(rt.properties.big, 9007199254740993n);
  assert.ok(rt.entries instanceof Map);
  assert.equal(rt.entries.get(1n), 'one');

  const bench = measureConversion(50);
  assert.equal(bench.iterations, 50);
  assert.ok(bench.totalMs >= 0);
});

test('VirtualFileSystem, imports, globbing, and rootDir security confinement', async () => {
  const vfs = new VirtualFileSystem(
    {
      '/sandbox/main.pkl': `
import "pkl:math"
import "helper.pkl"
import* "items/*.pkl" as items
data = read("data.txt").text
sum = math.max(0, helper.base) + items.toMap().values.map((m) -> m.v).fold(0, (a, b) -> a + b)
`,
      '/sandbox/helper.pkl': 'base = 10\n',
      '/sandbox/items/a.pkl': 'v = 2\n',
      '/sandbox/items/b.pkl': 'v = 3\n',
      '/sandbox/data.txt': 'hello-vfs',
    },
    { rootDir: '/sandbox' },
  );

  assert.equal(vfs.rootDir, '/sandbox');
  assert.equal(vfs.has('/sandbox/main.pkl'), true);
  assert.equal(vfs.isDirectory('/sandbox/items'), true);
  assert.deepEqual(vfs.glob('/sandbox/items/*.pkl'), ['/sandbox/items/a.pkl', '/sandbox/items/b.pkl']);
  // Host can stage files outside rootDir while evaluator SecurityPolicy confines access to rootDir
  vfs.write('/outside.pkl', 'secret = "no"\n');

  const ev = await createEvaluator({ vfs });
  try {
    assert.equal(await ev.evaluateExpression(ModuleSource.path('/sandbox/main.pkl'), 'sum'), '15');
    assert.equal(
      await ev.evaluateExpression(ModuleSource.path('/sandbox/main.pkl'), 'data'),
      'hello-vfs',
    );
    // Attempting to escape rootDir via forced relative import fails with security PklError
    await assert.rejects(
      ev.evaluateOutputText(
        ModuleSource.text('import "../outside.pkl"\nx = outside.secret\n', 'file:///sandbox/escape.pkl'),
      ),
      (err) => err instanceof PklError && err.kind === 'security',
    );
  } finally {
    ev.close();
  }

  // Static import graph analysis over VFS + builtin `pkl:*` stdlib modules
  const graph = await analyzeImportGraph(['file:///sandbox/main.pkl'], { vfs });
  assert.ok(graph.imports['file:///sandbox/main.pkl'].includes('pkl:math'));
  assert.ok(graph.imports['file:///sandbox/main.pkl'].includes('file:///sandbox/helper.pkl'));
  assert.ok(graph.imports['file:///sandbox/main.pkl'].includes('file:///sandbox/items/a.pkl'));
  assert.ok(graph.imports['file:///sandbox/main.pkl'].includes('file:///sandbox/items/b.pkl'));
  assert.deepEqual(graph.imports['pkl:math'], []);
  assert.equal(graph.resolvedImports['pkl:math'], 'pkl:math');
});

test('async moduleReaders, resourceReaders, httpFetch, packageTransport, logger, frameTransformer, securityManager', async () => {
  // Build deterministic ZIP package with SHA-256 verification
  const zipBytes = buildStoredZip({
    'PklProject': 'amends "pkl:Project"\n',
    'lib/greet.pkl': 'message = "from-package"\n',
  });
  const zipSha = createHash('sha256').update(zipBytes).digest('hex');
  const metadataBytes = new TextEncoder().encode(
    JSON.stringify({
      name: 'demo',
      packageUri: 'package://example.com/demo@1.0.0',
      version: '1.0.0',
      packageZipUrl: 'https://example.com/demo@1.0.0.zip',
      packageZipChecksums: { sha256: zipSha },
      dependencies: {},
    }),
  );
  const metaSha = createHash('sha256').update(metadataBytes).digest('hex');
  const pkgImport = `package://example.com/demo@1.0.0::sha256:${metaSha}#/lib/greet.pkl`;

  const logs = [];
  const ev = await createEvaluator({
    allowedModules: ['pkl:', 'repl:', 'mem:', 'https:', 'package:'],
    allowedResources: ['prop:', 'memres:', 'https:', 'package:'],
    moduleReaders: [
      {
        scheme: 'mem',
        hasHierarchicalUris: true,
        isGlobbable: true,
        async read(uri) {
          await Promise.resolve();
          if (uri === 'mem:///mod.pkl') return 'val = 77\n';
          return null;
        },
        async listElements() {
          return [{ name: 'mod.pkl', isDirectory: false }];
        },
      },
    ],
    resourceReaders: [
      {
        scheme: 'memres',
        async read(uri) {
          await Promise.resolve();
          if (uri === 'memres:secret') return new TextEncoder().encode('s3cr3t');
          return null;
        },
      },
    ],
    async httpFetch(url) {
      await Promise.resolve();
      if (url === 'https://mock.test/remote.pkl') {
        return { status: 200, bytes: new TextEncoder().encode('remoteVal = 99\n') };
      }
      if (url === 'https://mock.test/data.txt') {
        return { status: 200, bytes: new TextEncoder().encode('remote-text') };
      }
      return { status: 404, bytes: new Uint8Array() };
    },
    async packageTransport(url) {
      await Promise.resolve();
      if (url === 'https://example.com/demo@1.0.0') return metadataBytes;
      if (url === 'https://example.com/demo@1.0.0.zip') return zipBytes;
      throw new Error(`unexpected package URL: ${url}`);
    },
    logger: {
      trace(msg, frame) {
        logs.push({ level: 'trace', msg, uri: frame?.moduleUri });
      },
    },
    frameTransformer(frame) {
      return { ...frame, moduleUri: `transformed:${frame.moduleUri}` };
    },
  });

  try {
    const out = await ev.evaluateExpression(
      ModuleSource.text(`
import "mem:///mod.pkl"
import "https://mock.test/remote.pkl"
import "${pkgImport}" as pkg
res = read("memres:secret").text
httpRes = read("https://mock.test/data.txt").text
traced = trace(mod.val + remote.remoteVal)
result = "\\(traced)|\\(res)|\\(httpRes)|\\(pkg.message)"
`),
      'result',
    );
    assert.equal(out, '176|s3cr3t|remote-text|from-package');
    assert.equal(logs.length, 1);
    assert.equal(logs[0].msg, 'mod.val + remote.remoteVal = 176');
    assert.ok(logs[0].uri.startsWith('transformed:'));
  } finally {
    ev.close();
  }
});

test('schema, pkl:test (facts + examples + reporters), and stateful REPL', async () => {
  const vfs = new VirtualFileSystem({
    '/tests/math_test.pkl': `amends "pkl:test"

facts {
  ["addition"] {
    1 + 1 == 2
  }
}

examples {
  ["greeting"] {
    "hello"
  }
}
`,
    '/tests/math_test.pkl-expected.pcf': `examples {
  ["greeting"] {
    "hello"
  }
}
`,
    '/tests/lib.pkl': 'x = 10\ny = x * 2\n',
  });

  const ev = await createEvaluator({ vfs, powerAssertions: true });
  try {
    // Schema
    const schema = await ev.evaluateSchema(
      ModuleSource.text('class User { name: String\nage: Int }\nu: User = new { name = "Ada"; age = 36 }\n'),
    );
    assert.ok(schema.classes.User);
    assert.ok(schema.classes.User.properties.name);
    assert.ok(schema.moduleClass.properties.u);

    // Test evaluation (passing facts + matching expected.pcf examples)
    const testRes = await ev.evaluateTest(ModuleSource.path('/tests/math_test.pkl'), {
      reporter: 'spec',
    });
    assert.equal(testRes.failed, false);
    assert.equal(testRes.totalTests, 2);
    assert.ok(testRes.rendered.includes('✔ addition'));
    assert.ok(testRes.rendered.includes('✔ greeting'));
    assert.ok(testRes.rendered.includes('100.0% tests pass [2 passed], 100.0% asserts pass [2 passed]'));

    const junitRes = await ev.evaluateTest(ModuleSource.path('/tests/math_test.pkl'), {
      reporter: 'junit',
    });
    assert.ok(junitRes.rendered.includes('<testsuite'));

    // Failing fact triggers power assertion diagram
    vfs.write(
      '/tests/fail_test.pkl',
      `amends "pkl:test"

local x = 1 + 2

facts {
  ["broken"] {
    x == 5
  }
}
`,
    );
    const failRes = await ev.evaluateTest(ModuleSource.path('/tests/fail_test.pkl'), {
      reporter: 'spec',
    });
    assert.equal(failRes.failed, true);
    assert.equal(failRes.facts.results[0].failures.length, 1);
    assert.ok(failRes.rendered.includes('x == 5'));

    // Non-test module produces structured error result
    const nonTestRes = await ev.evaluateTest(ModuleSource.path('/tests/lib.pkl'), {
      reporter: 'minimal',
    });
    assert.equal(nonTestRes.failed, true);
    assert.ok(nonTestRes.error && nonTestRes.error.rendered.includes('Pkl Error'));
  } finally {
    ev.close();
  }

  // Stateful REPL (matches Pkl 0.32.0 ReplServer: definitions are silent unless evalDefinitions: true)
  const repl = await createRepl({ vfs, workingDir: '/tests' });
  try {
    const r1 = await repl.eval('a = 20');
    assert.deepEqual(r1, []);
    const r1Def = await repl.eval('b = 1', { evalDefinitions: true });
    assert.equal(r1Def[0].kind, 'evalSuccess');
    assert.equal(r1Def[0].result, '1');
    const r2 = await repl.eval('a + 22');
    assert.equal(r2[0].result, '42');
    const completions = await repl.complete('');
    assert.ok(completions.includes('a'));
    await repl.load('lib.pkl');
    const r3 = await repl.eval('y');
    assert.equal(r3[0].result, '20');
    await repl.reset();
    const r4 = await repl.eval('y');
    assert.equal(r4[0].kind, 'evalError');
  } finally {
    repl.close();
    assert.equal(repl.isClosed, true);
  }
});

test('64-bit timeout, AbortSignal pre-aborted + mid-flight abort, cancel, idempotent close', async () => {
  // 1. 64-bit timeout > u32::MAX (5 billion ms) must NOT overflow setTimeout or reject
  const ev64 = await createEvaluator({ timeoutMs: 5_000_000_000 });
  try {
    assert.equal(
      await ev64.evaluateExpression(ModuleSource.text('x = 123\n'), 'x'),
      '123',
    );
  } finally {
    ev64.close();
  }

  // BigInt timeout > 2^53 - 1
  const evBig = await createEvaluator({ timeoutMs: 9_007_199_254_740_999n });
  try {
    assert.equal(
      await evBig.evaluateExpression(ModuleSource.text('x = 456\n'), 'x'),
      '456',
    );
  } finally {
    evBig.close();
  }

  // Invalid timeoutMs rejected
  await assert.rejects(
    createEvaluator({ timeoutMs: -1 }),
    (e) => e instanceof PklError && e.kind === 'invalid-argument',
  );
  await assert.rejects(
    createEvaluator({ timeoutMs: 1.5 }),
    (e) => e instanceof PklError && e.kind === 'invalid-argument',
  );

  // 2. Pre-aborted signal: createEvaluator succeeds, evaluate* immediately rejects with cancelled
  const preCtrl = new AbortController();
  preCtrl.abort();
  const evPre = await createEvaluator({ signal: preCtrl.signal });
  try {
    await assert.rejects(
      evPre.evaluateOutputText(ModuleSource.text('x = 1\n')),
      (e) => e instanceof PklError && e.kind === 'cancelled' && e.code === 'cancelled',
    );
  } finally {
    evPre.close();
  }

  // 3. Mid-flight AbortSignal aborts in-flight httpFetch signal cleanly
  const midCtrl = new AbortController();
  let fetchAborted = false;
  const evMid = await createEvaluator({
    signal: midCtrl.signal,
    httpFetch(_url, initOpt) {
      return new Promise((_resolve, reject) => {
        initOpt.signal.addEventListener('abort', () => {
          fetchAborted = true;
          reject(new Error('aborted'));
        });
        setTimeout(() => midCtrl.abort(), 15);
      });
    },
  });
  try {
    await assert.rejects(
      evMid.evaluateOutputText(ModuleSource.uri('https://slow.test/mod.pkl')),
      (e) => e instanceof PklError && e.kind === 'cancelled',
    );
    assert.equal(fetchAborted, true);
  } finally {
    evMid.close();
    evMid.close(); // idempotent
    assert.equal(evMid.isClosed, true);
    await assert.rejects(
      evMid.evaluateOutputText(ModuleSource.text('x = 1\n')),
      (e) => e instanceof PklError && e.kind === 'closed',
    );
  }

  // 4. Short timeout fires cleanly during slow async fetch
  let timeoutFetchAborted = false;
  const evTimeout = await createEvaluator({
    timeoutMs: 30,
    httpFetch(_url, initOpt) {
      return new Promise((_resolve, reject) => {
        initOpt.signal.addEventListener('abort', () => {
          timeoutFetchAborted = true;
          reject(new Error('timed out'));
        });
      });
    },
  });
  try {
    await assert.rejects(
      evTimeout.evaluateOutputText(ModuleSource.uri('https://slow.test/timeout.pkl')),
      (e) => e instanceof PklError && e.kind === 'timeout',
    );
    assert.equal(timeoutFetchAborted, true);
  } finally {
    evTimeout.close();
  }
});

test('explicit capability-unavailable reporting (never silent skips)', async () => {
  for (const [opts, expectedCap] of [
    [{ externalModuleReaders: { ext: { executable: '/bin/false' } } }, 'externalReaders'],
    [{ modulePaths: ['/some/dir'] }, 'modulePathJar'],
    [{ projectDir: '/some/project' }, 'projectResolution'],
    [{ cacheDir: '/tmp/cache' }, 'packageCache'],
  ]) {
    const ev = await createEvaluator(opts);
    try {
      await assert.rejects(
        ev.evaluateOutputText(ModuleSource.text('x = 1\n')),
        (e) =>
          e instanceof PklError &&
          e.code === 'capability-unavailable' &&
          e.capability === expectedCap,
      );
    } finally {
      ev.close();
    }
  }

  // Runtime capability-unavailable without VFS / httpFetch
  const bare = await createEvaluator();
  try {
    await assert.rejects(
      bare.evaluateOutputText(ModuleSource.uri('file:///missing.pkl')),
      (e) =>
        e instanceof PklError &&
        e.code === 'capability-unavailable' &&
        e.capability === 'nativeFs',
    );
    // Even read?() must NOT silently return null when nativeFs capability is unavailable!
    await assert.rejects(
      bare.evaluateExpression(ModuleSource.text('x = read?("file:///missing.txt")\n'), 'x'),
      (e) =>
        e instanceof PklError &&
        e.code === 'capability-unavailable' &&
        e.capability === 'nativeFs',
    );
    await assert.rejects(
      bare.evaluateOutputText(ModuleSource.uri('https://example.invalid/a.pkl')),
      (e) =>
        e instanceof PklError &&
        e.code === 'capability-unavailable' &&
        e.capability === 'nativeHttp',
    );
    await assert.rejects(
      bare.evaluateOutputText(ModuleSource.modulePath('/a.pkl')),
      (e) =>
        e instanceof PklError &&
        e.code === 'capability-unavailable' &&
        e.capability === 'modulePathJar',
    );
    // Evaluator remains healthy after capability-unavailable rejections
    assert.equal(
      await bare.evaluateExpression(ModuleSource.text('ok = 1 + 2\n'), 'ok'),
      '3',
    );
  } finally {
    bare.close();
  }
});

test('npm pack tarball installs cleanly in isolated temp directory and executes in Node', () => {
  const workDir = mkdtempSync(join(tmpdir(), 'pkl-wasm-pack-test-'));
  try {
    const packDir = join(workDir, 'pack');
    const installDir = join(workDir, 'install');
    mkdirSync(packDir);
    mkdirSync(installDir);

    const isWin = process.platform === 'win32';
    const npmBin = isWin ? join(dirname(process.execPath), 'npm.cmd') : join(dirname(process.execPath), 'npm');
    const tarBin = isWin ? 'tar.exe' : '/usr/bin/tar';
    const npmCache = join(workDir, 'npm-cache');
    mkdirSync(npmCache);

    const tgzName = execFileSync(
      npmBin,
      ['pack', '--pack-destination', packDir, '--silent', '--cache', npmCache],
      {
        cwd: PKG_DIR,
        encoding: 'utf8',
        shell: isWin,
      },
    )
      .trim()
      .split('\n')
      .pop();
    assert.ok(tgzName && tgzName.endsWith('.tgz'), `expected .tgz, got ${tgzName}`);
    const tgzPath = join(packDir, tgzName);

    // Extract tarball directly into node_modules/@pkl-rs/wasm (100% offline clean install)
    // Use relative POSIX paths with cwd=workDir so GNU tar.exe on Windows does not treat `C:\...` as a remote host.
    const modDir = join(installDir, 'node_modules', '@pkl-rs', 'wasm');
    mkdirSync(modDir, { recursive: true });
    execFileSync(
      tarBin,
      ['-xzf', `pack/${tgzName}`, '--strip-components=1', '-C', 'install/node_modules/@pkl-rs/wasm'],
      { cwd: workDir },
    );

    writeFileSync(
      join(installDir, 'smoke.mjs'),
      `
import assert from 'node:assert/strict';
import { PKL_VERSION, capabilities, createEvaluator, format, ModuleSource, VirtualFileSystem } from '@pkl-rs/wasm';
assert.equal(PKL_VERSION, '0.32.0');
assert.equal(capabilities().runtime, 'wasm-node');
assert.equal(format('a=1\\n'), 'a = 1\\n');
const vfs = new VirtualFileSystem({ '/x.pkl': 'v = 21 * 2\\n' });
const ev = await createEvaluator({ vfs, outputFormat: 'json' });
const json = await ev.evaluateOutputText(ModuleSource.path('/x.pkl'));
ev.close();
assert.equal(json, '{\\n  "v": 42\\n}\\n');
console.log('CLEAN_INSTALL_OK');
`,
    );
    const out = execFileSync(process.execPath, ['smoke.mjs'], {
      cwd: installDir,
      encoding: 'utf8',
    });
    assert.ok(out.includes('CLEAN_INSTALL_OK'));
  } finally {
    rmSync(workDir, { recursive: true, force: true });
  }
});
