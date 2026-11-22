// SPDX-License-Identifier: Apache-2.0
// Milestone M7 benchmark driver for `@pkl-rs/wasm` running inside real headless Chromium
// via pinned Playwright 1.64.1 (`tools/platform/package.json`).
// Measures browser fetch+compile+init, warm persistent evaluations (verified 100% raw-byte
// against Java 0.32.0 BEFORE timing), and samples real Chromium process-tree RSS via `ps`.
import { createRequire } from "node:module";
import { createServer } from "node:http";
import { execFileSync } from "node:child_process";
import { readFile, stat, writeFile } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { createHash } from "node:crypto";

const [planPathArg, resultPathArg] = process.argv.slice(2);
if (!resultPathArg) {
  throw new Error("usage: bench_wasm_browser.mjs PLAN_JSON RESULT_JSON");
}

const plan = JSON.parse(await readFile(resolve(planPathArg), "utf8"));
const repoRoot = resolve(plan.repo_root);
const platformDir = resolve(repoRoot, "tools/platform");
const requireFromPlatform = createRequire(resolve(platformDir, "package.json"));

// Enforce exact pinned Playwright version matching tools/platform/package.json & package-lock.json
const pkgManifest = JSON.parse(await readFile(resolve(platformDir, "package.json"), "utf8"));
const pkgLock = JSON.parse(await readFile(resolve(platformDir, "package-lock.json"), "utf8"));
const pin = pkgManifest.devDependencies.playwright;
const playwrightVersion = requireFromPlatform("playwright/package.json").version;
const playwrightCoreVersion = requireFromPlatform("playwright-core/package.json").version;
if (
  !/^\d+\.\d+\.\d+$/.test(pin) ||
  playwrightVersion !== pin ||
  playwrightCoreVersion !== pin ||
  pkgLock.packages[""].devDependencies.playwright !== pin ||
  pkgLock.packages["node_modules/playwright"].version !== pin ||
  pkgLock.packages["node_modules/playwright-core"].version !== pin
) {
  throw new Error(`Playwright pin mismatch: expected ${pin}, got ${playwrightVersion}/${playwrightCoreVersion}`);
}

const playwrightEngines = requireFromPlatform("playwright");
const browserEngine = plan.browser_engine || "chromium";
if (!["chromium", "firefox", "webkit"].includes(browserEngine)) {
  throw new Error(`Unsupported browser_engine: ${browserEngine}`);
}
const browserType = playwrightEngines[browserEngine];
const launchArgs = Array.isArray(plan.launch_args) ? plan.launch_args.map(String) : [];
const pkgDir = resolve(plan.wasm_pkg_dir);
const wasmBinaryBytes = (await stat(resolve(pkgDir, "dist/wasm/pkl_wasm_bg.wasm"))).size;
const moduleSourceText = await readFile(resolve(plan.module_path), "utf8");
const expectedTextB64 = (await readFile(resolve(plan.expected_text_path))).toString("base64");
const expectedBinaryB64 = plan.expected_binary_path
  ? (await readFile(resolve(plan.expected_binary_path))).toString("base64")
  : null;

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json; charset=utf-8",
};

function classifyBrowserProcessRole(cmd, ppid, driverPid) {
  const lower = cmd.toLowerCase();
  if (
    lower.includes("--type=renderer") ||
    lower.includes("-contentproc") ||
    lower.includes("webcontent") ||
    lower.includes("webkitwebprocess")
  ) {
    return "renderer";
  }
  if (
    lower.includes("--type=gpu-process") ||
    lower.includes("gpuprocess") ||
    lower.includes("webkitgpuprocess") ||
    lower.includes("plugin-container")
  ) {
    return "gpu";
  }
  if (
    lower.includes("--type=utility") ||
    lower.includes("--type=zygote") ||
    lower.includes("webkitnetworkprocess") ||
    lower.includes("networking") ||
    lower.includes("crashpad")
  ) {
    return "utility_network";
  }
  if (ppid === driverPid || (!lower.includes("--type=") && !lower.includes("helper"))) {
    return "browser_main";
  }
  return "other";
}

function sampleDescendantTreeRssBytes(parentPid) {
  const raw = execFileSync("/bin/ps", ["-ww", "-axo", "pid=,ppid=,rss=,args="], {
    encoding: "utf8",
    env: { ...process.env, COLUMNS: "4096" },
  });
  const children = new Map();
  const rssKib = new Map();
  const ppidOf = new Map();
  const cmdOf = new Map();
  for (const line of raw.split("\n")) {
    const trimmed = line.trim();
    if (!trimmed) continue;
    const m = /^(\d+)\s+(\d+)\s+(\d+)\s*(.*)$/.exec(trimmed);
    if (!m) continue;
    const pid = Number(m[1]);
    const ppid = Number(m[2]);
    const rss = Number(m[3]);
    const cmd = m[4] || "";
    if (!Number.isFinite(pid) || !Number.isFinite(ppid) || !Number.isFinite(rss)) continue;
    rssKib.set(pid, rss);
    ppidOf.set(pid, ppid);
    cmdOf.set(pid, cmd);
    if (!children.has(ppid)) children.set(ppid, []);
    children.get(ppid).push(pid);
  }
  const visited = [];
  const stack = [...(children.get(parentPid) || [])];
  const seen = new Set();
  while (stack.length > 0) {
    const pid = stack.pop();
    if (seen.has(pid)) continue;
    seen.add(pid);
    if (rssKib.has(pid)) visited.push(pid);
    for (const child of children.get(pid) || []) {
      stack.push(child);
    }
  }
  let totalKib = 0;
  const roleBytes = { browser_main: 0, renderer: 0, gpu: 0, utility_network: 0, other: 0 };
  const processes = [];
  for (const pid of visited) {
    const kib = rssKib.get(pid) || 0;
    const bytes = kib * 1024;
    const cmd = cmdOf.get(pid) || "";
    const role = classifyBrowserProcessRole(cmd, ppidOf.get(pid), parentPid);
    totalKib += kib;
    roleBytes[role] = (roleBytes[role] || 0) + bytes;
    processes.push({
      pid,
      ppid: ppidOf.get(pid),
      rss_bytes: bytes,
      role,
      cmd_prefix: cmd.slice(0, 140),
    });
  }
  processes.sort((a, b) => b.rss_bytes - a.rss_bytes);
  return {
    total_bytes: totalKib * 1024,
    process_count: visited.length,
    pids: visited,
    role_rss_bytes: roleBytes,
    processes,
  };
}

const ISOLATION_HEADERS = {
  "Cross-Origin-Opener-Policy": "same-origin",
  "Cross-Origin-Embedder-Policy": "require-corp",
  "Cross-Origin-Resource-Policy": "same-origin",
};

const browserErrors = [];
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://127.0.0.1");
    if (url.pathname === "/") {
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8", ...ISOLATION_HEADERS });
      response.end("<!doctype html><title>M7 Wasm Browser Benchmark</title>");
      return;
    }
    if (url.pathname.startsWith("/pkg/")) {
      const rel = decodeURIComponent(url.pathname.slice("/pkg/".length));
      const candidate = resolve(pkgDir, rel);
      if (candidate.startsWith(pkgDir + sep)) {
        const bytes = await readFile(candidate);
        const ct = MIME[extname(candidate)] || "application/octet-stream";
        response.writeHead(200, { "Content-Type": ct, ...ISOLATION_HEADERS });
        response.end(bytes);
        return;
      }
    }
    response.writeHead(404, ISOLATION_HEADERS);
    response.end();
  } catch {
    response.writeHead(404, ISOLATION_HEADERS);
    response.end();
  }
});

await new Promise((res) => server.listen(0, "127.0.0.1", res));
const uiPort = server.address().port;

let browser;
try {
  const browserExecutable = browserType.executablePath();
  const browserExecutableSha256 = createHash("sha256").update(await readFile(browserExecutable)).digest("hex");
  const launchOpts = { headless: true, executablePath: browserExecutable };
  if (launchArgs.length > 0 && browserEngine !== "webkit") {
    launchOpts.args = launchArgs;
  }
  browser = await browserType.launch(launchOpts);
  const driverPid = process.pid;
  const rssSamples = [];
  const takeRss = (label) => {
    const snap = sampleDescendantTreeRssBytes(driverPid);
    rssSamples.push({ label, ...snap });
    return snap;
  };
  takeRss("after-launch");

  const page = await browser.newPage();
  page.on("pageerror", (err) => browserErrors.push(err.stack || String(err)));
  await page.goto(`http://127.0.0.1:${uiPort}/`);
  takeRss("after-goto");

  // Sample Chromium process-tree RSS periodically while page.evaluate runs
  const samplerTimer = setInterval(() => {
    try {
      takeRss("during-eval");
    } catch {
      // ignore transient ps error
    }
  }, 500);

  let pageResult;
  try {
    pageResult = await page.evaluate(
      async ({
        moduleSourceText: srcText,
        moduleUri,
        expectedTextB64: expTextB64,
        expectedBinaryB64: expBinB64,
        opMode,
        expr,
        sampling,
      }) => {
        const encoder = new TextEncoder();
        const decodeB64 = (b64) => {
          const bin = atob(b64);
          const out = new Uint8Array(bin.length);
          for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
          return out;
        };
        const bytesEqual = (a, b) => {
          if (a.length !== b.length) return false;
          for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
          return true;
        };
        const stable = (values, windows, tolerance) => {
          if (values.length < windows) return false;
          const recent = values.slice(values.length - windows).sort((a, b) => a - b);
          const median = (recent[windows / 2 - 1] + recent[windows / 2]) / 2;
          return median > 0 && (recent[windows - 1] - recent[0]) / median <= tolerance;
        };
        const verifyTypedValue = (val) => {
          const props = val?.properties;
          if (!props) throw new Error("typed value missing properties");
          if (props.count !== 200n) throw new Error(`unexpected count: ${String(props.count)}`);
          if (props.maxBig !== 9007199254741193n) throw new Error(`unexpected maxBig: ${String(props.maxBig)}`);
          const payload = props.payload;
          if (!(payload instanceof Uint8Array) || payload.length !== 7 || payload[0] !== 0 || payload[6] !== 255) {
            throw new Error("unexpected Uint8Array payload");
          }
          const items = props.items;
          if (!Array.isArray(items) || items.length !== 200) {
            throw new Error(`unexpected items length: ${items?.length}`);
          }
          const first = items[0]?.properties;
          if (!first || first.id !== 9007199254740994n || first.negId !== -9007199254740994n) {
            throw new Error("BigInt precision check failed");
          }
          if (first.label !== "row-1-한글-😀") {
            throw new Error(`UTF-16 string check failed: ${first.label}`);
          }
          if (!(first.raw instanceof Uint8Array) || first.raw.length !== 4 || first.raw[3] !== 255) {
            throw new Error("nested Uint8Array check failed");
          }
        };

        const expectedText = decodeB64(expTextB64);
        const expectedBinary = expBinB64 ? decodeB64(expBinB64) : null;

        const initStart = performance.now();
        const api = await import("/pkg/dist/index.mjs");
        const runtimeApi = await import("/pkg/dist/runtime.mjs");
        await api.init();
        const wasmInitOnlyNs = Math.round((performance.now() - initStart) * 1e6);
        const readWasmMem = () =>
          runtimeApi.getWasmMemory ? runtimeApi.getWasmMemory().buffer.byteLength : null;
        const wasmMemAfterInit = readWasmMem();
        let wasmMemPeak = wasmMemAfterInit || 0;
        const updateWasmMemPeak = () => {
          const cur = readWasmMem();
          if (typeof cur === "number" && cur > wasmMemPeak) wasmMemPeak = cur;
        };
        const ev = await api.createEvaluator({ color: false, env: {}, properties: {} });
        const initNs = Math.round((performance.now() - initStart) * 1e6);
        updateWasmMemPeak();

        const source = api.ModuleSource.text(srcText, moduleUri);
        try {
          const preText = await ev.evaluateOutputText(source);
          const preTextBytes = encoder.encode(preText);
          if (!bytesEqual(preTextBytes, expectedText)) {
            throw new Error(
              `wasm-browser preflight output.text mismatch (${preTextBytes.length}B vs ${expectedText.length}B)`
            );
          }
          if (expectedBinary) {
            const preBin = await ev.evaluateExpressionPklBinary(source, "module");
            if (!bytesEqual(preBin, expectedBinary)) {
              throw new Error(
                `wasm-browser preflight pkl-binary mismatch (${preBin.length}B vs ${expectedBinary.length}B)`
              );
            }
          }

          const invokeOnce = async () =>
            opMode === "eval-expression"
              ? await ev.evaluateExpression(source, expr)
              : await ev.evaluateExpressionValue(source, "module");

          const checkOnce = (out) => {
            if (opMode === "eval-expression") {
              if (!bytesEqual(encoder.encode(out), expectedText)) {
                throw new Error("wasm-browser timed eval-expression mismatch");
              }
            } else {
              verifyTypedValue(out);
            }
          };

          const firstStart = performance.now();
          const firstOut = await invokeOnce();
          const firstNs = Math.max(1, Math.round((performance.now() - firstStart) * 1e6));
          checkOnce(firstOut);

          const sampleMs = sampling.sample_seconds * 1000;
          const minWarmupMs = sampling.minimum_warmup_seconds * 1000;
          const maxWarmupMs = sampling.maximum_warmup_seconds * 1000;

          // Adaptive inner batching prevents performance.now() timer quantization if ops become < 100 us.
          let batchSize = 1;
          const runWindow = async (phase, index) => {
            let elapsedMs = 0;
            let iterations = 0;
            while (elapsedMs < sampleMs) {
              const outputs = new Array(batchSize);
              const t0 = performance.now();
              for (let b = 0; b < batchSize; b++) {
                outputs[b] = await invokeOnce();
              }
              const dt = performance.now() - t0;
              elapsedMs += dt;
              for (let b = 0; b < batchSize; b++) {
                checkOnce(outputs[b]);
              }
              iterations += batchSize;
              if (dt < 1.0 && batchSize < 64) {
                batchSize *= 2;
              }
            }
            const elapsedNs = Math.max(1, Math.round(elapsedMs * 1e6));
            return { phase, index, elapsed_ns: elapsedNs, iterations, ns_per_op: elapsedNs / iterations };
          };

          const warmupRows = [];
          const warmupValues = [];
          const warmupStart = performance.now();
          let warmupElapsedMs = 0;
          let converged = false;
          do {
            const row = await runWindow("warmup", warmupRows.length);
            warmupRows.push(row);
            warmupValues.push(row.ns_per_op);
            warmupElapsedMs = performance.now() - warmupStart;
            converged =
              warmupElapsedMs >= minWarmupMs &&
              stable(warmupValues, sampling.convergence_windows, sampling.maximum_relative_spread);
          } while (!converged && warmupElapsedMs < maxWarmupMs);

          const measurementRows = [];
          for (let i = 0; i < sampling.samples; i++) {
            measurementRows.push(await runWindow("measurement", i));
            updateWasmMemPeak();
          }

          const pm = performance.memory;
          const jsHeapAfterEval = pm
            ? {
                jsHeapSizeLimit: pm.jsHeapSizeLimit,
                totalJSHeapSize: pm.totalJSHeapSize,
                usedJSHeapSize: pm.usedJSHeapSize,
              }
            : null;

          return {
            preflight_raw_byte_verified: true,
            cross_origin_isolated: Boolean(globalThis.crossOriginIsolated),
            final_batch_size: batchSize,
            wasm_linear_memory_bytes_after_init: wasmMemAfterInit,
            wasm_linear_memory_bytes_peak: wasmMemPeak > 0 ? wasmMemPeak : null,
            wasm_linear_memory_pages_peak: wasmMemPeak > 0 ? Math.round(wasmMemPeak / 65536) : null,
            js_heap_after_eval: jsHeapAfterEval,
            wasm_init_only_ns: wasmInitOnlyNs,
            initialize_ns: initNs,
            first_request_ns: firstNs,
            warmup: {
              elapsed_ns: Math.max(1, Math.round(warmupElapsedMs * 1e6)),
              converged,
              windows: warmupRows.length,
              rows: warmupRows,
            },
            measurements: measurementRows,
          };
        } finally {
          ev.close();
        }
      },
      {
        moduleSourceText,
        moduleUri: plan.module_uri || "repl:bench.pkl",
        expectedTextB64,
        expectedBinaryB64,
        opMode: plan.op_mode,
        expr: plan.expr || "run(0)",
        sampling: plan.sampling,
      }
    );
  } finally {
    clearInterval(samplerTimer);
  }
  takeRss("after-eval");

  let peakSample = rssSamples[0];
  for (const s of rssSamples) {
    if (s.total_bytes > peakSample.total_bytes) peakSample = s;
  }
  const peakChromiumTreeRss = peakSample.total_bytes;
  const peakRendererRss = Math.max(...rssSamples.map((s) => (s.role_rss_bytes?.renderer || 0)));
  const output = {
    schema_version: 1,
    runtime: "wasm-browser",
    browser_engine: browserEngine,
    launch_args: launchArgs,
    op_mode: plan.op_mode,
    node: process.version,
    browser: browser.version(),
    browserExecutable,
    browserExecutableSha256,
    playwright: playwrightVersion,
    playwrightCore: playwrightCoreVersion,
    playwrightPin: pin,
    browserErrors,
    driver_pid: driverPid,
    wasm_binary_bytes: wasmBinaryBytes,
    chromium_peak_tree_rss_bytes: peakChromiumTreeRss,
    peak_tree_rss_bytes: peakChromiumTreeRss,
    peak_renderer_rss_bytes: peakRendererRss,
    peak_role_rss_bytes: peakSample.role_rss_bytes || {},
    peak_process_breakdown: peakSample.processes || [],
    chromium_rss_samples: rssSamples.map(({ processes: _p, ...rest }) => rest),
    memory_boundary:
      "Sum of RSS across browser + renderer + GPU/utility child process tree sampled via /bin/ps (with per-role breakdown and Wasm linear memory separately recorded), plus separate Node driver wait4 peak RSS",
    ...pageResult,
  };
  await writeFile(resolve(resultPathArg), JSON.stringify(output, null, 2) + "\n", "utf8");
  if (browserErrors.length > 0) {
    throw new Error(`${browserEngine} reported uncaught page errors: ${browserErrors.join("; ")}`);
  }
} finally {
  if (browser) await browser.close();
  await new Promise((res) => server.close(res));
}
