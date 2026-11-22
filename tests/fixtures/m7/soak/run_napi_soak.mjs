// SPDX-License-Identifier: Apache-2.0
// Node.js N-API (@pkl-rs/napi) long-running memory reclamation soak runner for Milestone M7.
// Exercises all 6 required scenarios and emits synchronized JSON checkpoint samples
// (process.memoryUsage().rss, heapUsed, external, arrayBuffers, open_fds) for tools/m7_memory_soak.py.
import {
  copyFileSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

function parseArgs(argv) {
  const cfg = {
    scenario: "",
    iterations: 600,
    warmup: 60,
    sampleInterval: 60,
    workDir: ".",
    pkgEntry: "packages/pkl-napi/dist/index.mjs",
    pkgPoolDir: null,
    extReaderPython: null,
    extReaderScript: null,
    syncStdin: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === "--scenario") cfg.scenario = argv[++i];
    else if (a === "--iterations") cfg.iterations = Number.parseInt(argv[++i], 10);
    else if (a === "--warmup") cfg.warmup = Number.parseInt(argv[++i], 10);
    else if (a === "--sample-interval") cfg.sampleInterval = Number.parseInt(argv[++i], 10);
    else if (a === "--work-dir") cfg.workDir = argv[++i];
    else if (a === "--pkg-entry") cfg.pkgEntry = argv[++i];
    else if (a === "--pkg-pool-dir") cfg.pkgPoolDir = argv[++i];
    else if (a === "--ext-reader-python") cfg.extReaderPython = argv[++i];
    else if (a === "--ext-reader-script") cfg.extReaderScript = argv[++i];
    else if (a === "--sync-stdin") cfg.syncStdin = true;
    else throw new Error(`unknown arg: ${a}`);
  }
  if (!cfg.scenario) throw new Error("--scenario is required");
  if (!Number.isInteger(cfg.iterations) || cfg.iterations <= 0) {
    throw new Error("--iterations must be > 0");
  }
  if (!Number.isInteger(cfg.sampleInterval) || cfg.sampleInterval <= 0) {
    cfg.sampleInterval = Math.max(1, Math.floor(cfg.iterations / 10));
  }
  return cfg;
}

function countOpenFds() {
  for (const dir of ["/dev/fd", "/proc/self/fd"]) {
    try {
      return Math.max(0, readdirSync(dir).length - 1);
    } catch {
      // try next
    }
  }
  return null;
}

const stdinLines = [];
const stdinWaiters = [];
let stdinStarted = false;
function initStdinReader() {
  if (stdinStarted) return;
  stdinStarted = true;
  let buf = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk) => {
    buf += chunk;
    let idx;
    while ((idx = buf.indexOf("\n")) >= 0) {
      const line = buf.slice(0, idx);
      buf = buf.slice(idx + 1);
      if (stdinWaiters.length > 0) {
        stdinWaiters.shift()(line);
      } else {
        stdinLines.push(line);
      }
    }
  });
  process.stdin.resume();
}

function waitStdinAck() {
  initStdinReader();
  if (stdinLines.length > 0) {
    return Promise.resolve(stdinLines.shift());
  }
  return new Promise((res) => stdinWaiters.push(res));
}

async function emitSample(phase, iter, startMs, syncStdin) {
  if (typeof globalThis.gc === "function") {
    globalThis.gc();
  }
  const mu = process.memoryUsage();
  const payload = {
    type: "sample",
    phase,
    iter,
    elapsed_ms: Date.now() - startMs,
    self_rss_bytes: mu.rss,
    heap_used_bytes: mu.heapUsed,
    heap_total_bytes: mu.heapTotal,
    external_bytes: mu.external,
    array_buffers_bytes: mu.arrayBuffers,
    open_fds: countOpenFds(),
    gc_exposed: typeof globalThis.gc === "function",
  };
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  if (syncStdin) {
    await waitStdinAck();
  }
}

function makePadding(seed, repeats) {
  let out = "";
  for (let k = 0; k < repeats; k += 1) {
    out += `seg_${seed}_${String(k).padStart(2, "0")}_abcdefghij`;
  }
  return out;
}

function baseOpts() {
  return {
    allowedModules: ["pkl:", "repl:", "file:", "modulepath:", "package:", "uncached:", "churn:"],
    allowedResources: ["prop:", "env:", "file:", "modulepath:", "package:", "extres:"],
    noCache: true,
    env: {},
    properties: {},
  };
}

function cyclicProgram(iterId) {
  const pad = makePadding(iterId, 12);
  return `class Node {
  id: Int
  payload: String
  next: Node
}
local a {
  tag = "a-${iterId}"
  payload = "${pad}"
  peer = b
}
local b {
  tag = "b-${iterId}"
  payload = "${pad}"
  peer = a
}
local n0: Node = new Node { id = ${iterId}; payload = "${pad}"; next = n1 }
local n1: Node = new Node { id = ${iterId + 1}; payload = "${pad}"; next = n2 }
local n2: Node = new Node { id = ${iterId + 2}; payload = "${pad}"; next = n3 }
local n3: Node = new Node { id = ${iterId + 3}; payload = "${pad}"; next = n4 }
local n4: Node = new Node { id = ${iterId + 4}; payload = "${pad}"; next = n5 }
local n5: Node = new Node { id = ${iterId + 5}; payload = "${pad}"; next = n6 }
local n6: Node = new Node { id = ${iterId + 6}; payload = "${pad}"; next = n7 }
local n7: Node = new Node { id = ${iterId + 7}; payload = "${pad}"; next = n0 }
local f = (k) -> if (k <= 0) n0.id else g.apply(k - 1) + 1
local g = (k) -> if (k <= 0) n1.id else f.apply(k - 1) + 2
local bag = new Mapping {
  ["a"] = a
  ["b"] = b
  ["ring"] = new Listing { n0; n1; n2; n3 }
}
summary = "\\(a.peer.peer.tag)|\\(b.peer.tag)|\\(n0.next.next.next.next.next.next.next.next.id)|\\(f.apply(6))|\\(bag["a"].tag)"
`;
}

function loadPkgPool(poolDir) {
  if (!poolDir) return [];
  try {
    const raw = readFileSync(join(poolDir, "pool.tsv"), "utf8");
    const rows = [];
    for (const line of raw.split("\n")) {
      if (!line.trim()) continue;
      const parts = line.split("\t");
      if (parts.length === 5) {
        rows.push({
          gen: Number.parseInt(parts[0], 10),
          metaSha: parts[1],
          metaJsonPath: join(poolDir, parts[2]),
          zipPath: join(poolDir, parts[3]),
          mpJarPath: join(poolDir, parts[4]),
        });
      }
    }
    return rows;
  } catch {
    return [];
  }
}

async function runScenario(api, cfg, startMs) {
  const { createEvaluator, ModuleSource, PklError } = api;
  const failures = [];
  let assertionsPassed = 0;
  let completed = 0;
  const totalSteps = cfg.warmup + cfg.iterations;

  const maybeSample = async (step) => {
    if (step + 1 === cfg.warmup) {
      await emitSample("warmup-end", 0, startMs, cfg.syncStdin);
    } else if (step >= cfg.warmup) {
      completed += 1;
      if (completed === cfg.iterations) {
        await emitSample("final", completed, startMs, cfg.syncStdin);
      } else if (completed % cfg.sampleInterval === 0) {
        await emitSample("sample", completed, startMs, cfg.syncStdin);
      }
    }
  };

  if (cfg.scenario === "repeated-success") {
    const baseFile = join(cfg.workDir, "napi_repeated_base.pkl");
    writeFileSync(
      baseFile,
      `class Entry { idx: Int; label: String; score: Int }
items = List(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
entries: Listing<Entry> = new { for (n in items) { new Entry { idx = n; label = "item-\\(n)"; score = n * n + 3 } } }
total = entries.toList().map((e) -> e.score).fold(0, (a, b) -> a + b)
`,
      "utf8",
    );
    const fileSrc = ModuleSource.path(baseFile);
    const ev = await createEvaluator(baseOpts());
    try {
      for (let step = 0; step < totalSteps; step += 1) {
        const mod = step % 100;
        if (step % 3 === 0) {
          const out = await ev.evaluateExpression(fileSrc, `total + ${mod}`);
          if (out === String(1544 + mod)) assertionsPassed += 1;
          else if (failures.length < 16) failures.push(`step ${step}: file expr mismatch ${out}`);
        } else {
          const pad = makePadding(step, 24);
          const src = ModuleSource.text(`// padding: ${pad}
local base = ${mod}
local nums = List(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
local f = (x) -> x + base
mapped = nums.map(f)
sum = mapped.fold(0, (a, b) -> a + b)
meta { tag = "iter-${step}"; payload = "${pad.slice(0, 256)}" }
`);
          const expectedNeedle = `sum = ${mod * 16 + 136}\n`;
          if (step % 3 === 1) {
            const out = await ev.evaluateOutputText(src);
            if (out.includes(expectedNeedle)) assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step}: missing ${expectedNeedle}`);
          } else {
            const bytes = await ev.evaluateOutputBytes(src);
            const text = Buffer.from(bytes).toString("utf8");
            if (text.includes(expectedNeedle)) assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step}: bytes missing ${expectedNeedle}`);
          }
        }
        await maybeSample(step);
      }
    } finally {
      await ev.close();
    }
  } else if (cfg.scenario === "repeated-failure") {
    const ev = await createEvaluator(baseOpts());
    try {
      for (let step = 0; step < totalSteps; step += 1) {
        const pad = makePadding(step, 32);
        const mode = step % 7;
        let srcText;
        let expectErr = true;
        if (mode === 0) srcText = `// ${pad}\nx: Int = "not-an-int-${step}"\n`;
        else if (mode === 1) srcText = `// ${pad}\nx: Int(this > 1000 && this < 2000) = ${step % 500}\n`;
        else if (mode === 2) srcText = `// ${pad}\nfunction dive(n) = if (n <= 0) throw("boom-${step}") else dive(n - 1)\nx = dive(6)\n`;
        else if (mode === 3) srcText = `// ${pad}\nxs = List(1, 2, 3)\nx = xs[${10 + (step % 5)}]\n`;
        else if (mode === 4) srcText = `// ${pad}\nx = (1 + 2 + \n`;
        else if (mode === 5) srcText = `// ${pad}\nlocal lazyBad = throw("lazy-${step}")\nsafe = 1 + ${step}\nboom = lazyBad\n`;
        else {
          srcText = `// ${pad}\nlocal missing = read?("prop:soakMissing_${step}") ?? "${step}"\nlocal nonMatch = "bad" is Int(this > 0)\nok = if (!nonMatch && missing == "${step}") ${step + 1} else -1\n`;
          expectErr = false;
        }
        if (expectErr) {
          try {
            await ev.evaluateOutputText(ModuleSource.text(srcText));
            if (failures.length < 16) failures.push(`step ${step} mode ${mode}: expected error`);
          } catch (err) {
            if (err instanceof PklError && err.kind === "evaluation") assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step} mode ${mode}: bad err ${err}`);
          }
        } else {
          const out = await ev.evaluateOutputText(ModuleSource.text(srcText));
          if (out === `ok = ${step + 1}\n`) assertionsPassed += 1;
          else if (failures.length < 16) failures.push(`step ${step}: caught mismatch ${out}`);
        }
        await maybeSample(step);
      }
    } finally {
      await ev.close();
    }
  } else if (cfg.scenario === "unique-sources-churn") {
    const uniqueDir = join(cfg.workDir, "napi_unique_sources");
    mkdirSync(uniqueDir, { recursive: true });
    const persistentEv = await createEvaluator({
      ...baseOpts(),
      moduleReaders: [
        {
          scheme: "uncached",
          hasHierarchicalUris: true,
          isCached: false,
          async read(uri) {
            const m = /^uncached:\/\/\/mod-(\d+)\.pkl$/.exec(uri);
            if (!m) throw new Error(`missing ${uri}`);
            const id = Number.parseInt(m[1], 10);
            const pad = makePadding(id, 80);
            return `// unique uncached ${id}: ${pad}
class Box_${id} { id: Int = ${id}; tag: String = "u-${id}"; vals: List<Int> = List(${id}, ${id + 1}, ${id + 2}) }
inst = new Box_${id} {}
checksum = inst.id * 2 + inst.vals.length
`;
          },
        },
      ],
    });
    try {
      for (let step = 0; step < totalSteps; step += 1) {
        if (step % 2 === 0) {
          const out = await persistentEv.evaluateExpression(
            ModuleSource.uri(`uncached:///mod-${step}.pkl`),
            "checksum",
          );
          if (out === String(step * 2 + 3)) assertionsPassed += 1;
          else if (failures.length < 16) failures.push(`step ${step} uncached mismatch ${out}`);
        } else {
          const pA = join(uniqueDir, `u_${step}_a.pkl`);
          const pB = join(uniqueDir, `u_${step}_b.pkl`);
          const pC = join(uniqueDir, `u_${step}_c.pkl`);
          const pad = makePadding(step, 64);
          writeFileSync(pA, `// ${pad}\nval = ${step}\n`, "utf8");
          writeFileSync(pB, `// ${pad}\nimport "u_${step}_a.pkl" as a\ndoubled = a.val * 2\n`, "utf8");
          writeFileSync(pC, `// ${pad}\nimport "u_${step}_b.pkl" as b\nresult = b.doubled + 7\n`, "utf8");
          const ev = await createEvaluator(baseOpts());
          try {
            const out = await ev.evaluateOutputText(ModuleSource.path(pC));
            if (out === `result = ${step * 2 + 7}\n`) assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step} fresh mismatch ${out}`);
          } finally {
            await ev.close();
            rmSync(pA, { force: true });
            rmSync(pB, { force: true });
            rmSync(pC, { force: true });
          }
        }
        await maybeSample(step);
      }
    } finally {
      await persistentEv.close();
    }
  } else if (cfg.scenario === "cyclic-graphs") {
    const ev = await createEvaluator(baseOpts());
    try {
      for (let step = 0; step < totalSteps; step += 1) {
        const src = cyclicProgram(step);
        const expected = `a-${step}|a-${step}|${step}|${step + 9}|a-${step}`;
        const out = await ev.evaluateExpression(ModuleSource.text(src), "summary");
        if (out === expected) assertionsPassed += 1;
        else if (failures.length < 16) failures.push(`step ${step} cyclic mismatch ${out}`);
        await maybeSample(step);
      }
    } finally {
      await ev.close();
    }
  } else if (cfg.scenario === "lifecycle-cancel-timeout-close") {
    const slowSrc = ModuleSource.text(
      `function fib(n) = if (n <= 1) n else fib(n - 1) + fib(n - 2)\nresult = fib(40)\n`,
    );
    for (let step = 0; step < totalSteps; step += 1) {
      const mode = step % 3;
      const useExt = step % 9 === 1 && cfg.extReaderPython && cfg.extReaderScript;
      const opts = baseOpts();
      if (useExt) {
        opts.externalResourceReaders = {
          extres: { executable: cfg.extReaderPython, arguments: [cfg.extReaderScript] },
        };
      }
      const ev = await createEvaluator(opts);
      try {
        const quickOut = await ev.evaluateOutputText(ModuleSource.text(`v = ${step} + 1\n`));
        if (quickOut === `v = ${step + 1}\n`) assertionsPassed += 1;
        else if (failures.length < 16) failures.push(`step ${step} quick mismatch ${quickOut}`);

        if (useExt) {
          const r = await ev.evaluateExpression(
            ModuleSource.text(`r = read("extres:/hello.txt").text`),
            "r",
          );
          if (r.length > 0) assertionsPassed += 1;
          else if (failures.length < 16) failures.push(`step ${step} extres empty`);
        }

        if (mode === 0) {
          try {
            await ev.evaluateOutputText(slowSrc, { timeoutMs: 25 });
            if (failures.length < 16) failures.push(`step ${step} expected timeout`);
          } catch (err) {
            if (err instanceof PklError && err.kind === "timeout") assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step} timeout bad err ${err}`);
          }
        } else if (mode === 1) {
          const timer = setTimeout(() => ev.cancel(), 8);
          try {
            await ev.evaluateOutputText(slowSrc);
            if (failures.length < 16) failures.push(`step ${step} expected cancel`);
          } catch (err) {
            if (err instanceof PklError && err.kind === "cancelled") assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step} cancel bad err ${err}`);
          } finally {
            clearTimeout(timer);
          }
        } else {
          const ctrl = new AbortController();
          const timer = setTimeout(() => ctrl.abort(), 8);
          try {
            await ev.evaluateOutputText(slowSrc, { signal: ctrl.signal });
            if (failures.length < 16) failures.push(`step ${step} expected abort cancel`);
          } catch (err) {
            if (err instanceof PklError && err.kind === "cancelled") assertionsPassed += 1;
            else if (failures.length < 16) failures.push(`step ${step} abort bad err ${err}`);
          } finally {
            clearTimeout(timer);
          }
        }
      } finally {
        await ev.close();
        await ev.close();
      }
      if (!ev.isClosed) {
        if (failures.length < 16) failures.push(`step ${step} not closed`);
      } else {
        try {
          await ev.evaluateOutputText(ModuleSource.text("x = 1\n"));
          if (failures.length < 16) failures.push(`step ${step} post-close unexpectedly succeeded`);
        } catch (err) {
          if (err instanceof PklError && err.kind === "closed") assertionsPassed += 1;
          else if (failures.length < 16) failures.push(`step ${step} post-close bad err ${err}`);
        }
      }
      await maybeSample(step);
    }
  } else if (cfg.scenario === "cache-churn") {
    const pool = loadPkgPool(cfg.pkgPoolDir);
    const churnDir = join(cfg.workDir, "napi_cache_churn");
    const activeCacheDir = join(churnDir, "active_cache");
    const activeModDir = join(churnDir, "mods");
    mkdirSync(activeModDir, { recursive: true });

    let liveVer = 0;
    const uncachedEv = await createEvaluator({
      ...baseOpts(),
      moduleReaders: [
        {
          scheme: "churn",
          hasHierarchicalUris: true,
          isCached: false,
          async read() {
            const pad = makePadding(liveVer, 30);
            return `// churn ${liveVer}: ${pad}\ngen = ${liveVer}\n`;
          },
        },
      ],
    });
    try {
      for (let step = 0; step < totalSteps; step += 1) {
        liveVer = step;
        const reloadOut = await uncachedEv.evaluateExpression(
          ModuleSource.uri("churn:///live.pkl"),
          "gen",
        );
        if (reloadOut === String(step)) assertionsPassed += 1;
        else if (failures.length < 16) failures.push(`step ${step} uncached reload ${reloadOut}`);

        rmSync(activeCacheDir, { recursive: true, force: true });
        const depPath = join(activeModDir, "dep.pkl");
        const mainPath = join(activeModDir, "main.pkl");
        writeFileSync(depPath, `// ${makePadding(step, 24)}\nlocalGen = ${step}\n`, "utf8");

        const opts = baseOpts();
        let expected;
        if (pool.length > 0) {
          const entry = pool[step % pool.length];
          const pkgDir = join(activeCacheDir, "package-2", "soak.local", "soakpkg@1.0.0");
          mkdirSync(pkgDir, { recursive: true });
          copyFileSync(entry.metaJsonPath, join(pkgDir, "soakpkg@1.0.0.json"));
          copyFileSync(entry.zipPath, join(pkgDir, "soakpkg@1.0.0.zip"));
          const activeJar = join(churnDir, "active_mp.jar");
          copyFileSync(entry.mpJarPath, activeJar);
          const pkgUri = `package://soak.local/soakpkg@1.0.0::sha256:${entry.metaSha}#/lib/info.pkl`;
          writeFileSync(
            mainPath,
            `import "dep.pkl"\nimport "modulepath:/mp.pkl"\nimport "${pkgUri}" as pkg\ncombined = "\\(dep.localGen):\\(mp.mpGen):\\(pkg.pkgGen)"\n`,
            "utf8",
          );
          opts.noCache = false;
          opts.cacheDir = activeCacheDir;
          opts.modulePaths = [activeJar];
          expected = `${step}:${entry.gen}:${entry.gen}`;
        } else {
          writeFileSync(mainPath, `import "dep.pkl"\ncombined = "\\(dep.localGen)"\n`, "utf8");
          expected = String(step);
        }

        const ev = await createEvaluator(opts);
        try {
          const mainSrc = ModuleSource.path(mainPath);
          for (let pass = 0; pass < 2; pass += 1) {
            const out = await ev.evaluateExpression(mainSrc, "combined");
            if (out === expected) assertionsPassed += 1;
            else if (failures.length < 16) {
              failures.push(`step ${step} pass ${pass} cache mismatch ${out} vs ${expected}`);
            }
          }
        } finally {
          await ev.close();
        }
        await maybeSample(step);
      }
    } finally {
      await uncachedEv.close();
      rmSync(activeCacheDir, { recursive: true, force: true });
    }
  } else {
    throw new Error(`unknown scenario: ${cfg.scenario}`);
  }

  return { completed, assertionsPassed, failures };
}

async function main() {
  const cfg = parseArgs(process.argv.slice(2));
  mkdirSync(cfg.workDir, { recursive: true });
  const api = await import(pathToFileURL(resolve(cfg.pkgEntry)).href);
  const startMs = Date.now();
  const { completed, assertionsPassed, failures } = await runScenario(api, cfg, startMs);
  process.stdout.write(
    `${JSON.stringify({
      type: "summary",
      scenario: cfg.scenario,
      runtime: "napi-node",
      node_version: process.version,
      gc_exposed: typeof globalThis.gc === "function",
      warmup_iterations: cfg.warmup,
      completed_iterations: completed,
      assertions_passed: assertionsPassed,
      elapsed_ms: Date.now() - startMs,
      assertion_failures: failures,
    })}\n`,
  );
  process.exit(failures.length === 0 && completed === cfg.iterations ? 0 : 1);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
