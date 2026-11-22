// SPDX-License-Identifier: Apache-2.0
// Milestone M7 benchmark driver for `@pkl-rs/napi` (Node.js native addon).
// Verifies 100% raw-byte output parity against pinned Java 0.32.0 BEFORE timing,
// measures init, first-request, warmup convergence, measurement windows,
// JS<->Rust BigInt/Uint8Array conversion, and Node process memoryUsage().
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [planPathArg, resultPathArg] = process.argv.slice(2);
if (!resultPathArg) {
  throw new Error("usage: bench_napi.mjs PLAN_JSON RESULT_JSON");
}

const plan = JSON.parse(await readFile(resolve(planPathArg), "utf8"));
const pkgEntry = resolve(plan.napi_pkg_dir, "dist/index.mjs");
const modulePath = resolve(plan.module_path);
const expectedText = await readFile(resolve(plan.expected_text_path));
const expectedBinary = plan.expected_binary_path
  ? await readFile(resolve(plan.expected_binary_path))
  : null;
const opMode = plan.op_mode; // "eval-expression" | "typed-conversion"
const expr = plan.expr || "run(0)";
const sampling = plan.sampling;

const encoder = new TextEncoder();

function bytesEqual(a, b) {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) {
    if (a[i] !== b[i]) return false;
  }
  return true;
}

function stable(values, windows, tolerance) {
  if (values.length < windows) return false;
  const recent = values.slice(values.length - windows).sort((a, b) => a - b);
  const median = (recent[windows / 2 - 1] + recent[windows / 2]) / 2;
  return median > 0 && (recent[windows - 1] - recent[0]) / median <= tolerance;
}

function verifyTypedValue(val) {
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
}

const initStart = process.hrtime.bigint();
const api = await import(pathToFileURL(pkgEntry).href);
const ev = await api.createEvaluator({ noCache: true, color: false, env: {}, properties: {} });
const initNs = Number(process.hrtime.bigint() - initStart);

const source = api.ModuleSource.path(modulePath);

try {
  // Preflight 100% raw-byte verification against Java BEFORE timing!
  const preText = await ev.evaluateOutputText(source);
  const preTextBytes = encoder.encode(preText);
  if (!bytesEqual(preTextBytes, expectedText)) {
    throw new Error(
      `napi preflight output.text raw-byte mismatch (${preTextBytes.length}B vs ${expectedText.length}B)`
    );
  }
  if (expectedBinary) {
    const preBin = await ev.evaluateExpressionPklBinary(source, "module");
    if (!bytesEqual(preBin, expectedBinary)) {
      throw new Error(
        `napi preflight pkl-binary raw-byte mismatch (${preBin.length}B vs ${expectedBinary.length}B)`
      );
    }
  }

  async function invokeOnce() {
    if (opMode === "eval-expression") {
      return await ev.evaluateExpression(source, expr);
    }
    return await ev.evaluateExpressionValue(source, "module");
  }

  function checkOnce(out) {
    if (opMode === "eval-expression") {
      const b = encoder.encode(out);
      if (!bytesEqual(b, expectedText)) {
        throw new Error("napi timed eval-expression output bytes differ from Java preflight");
      }
    } else {
      verifyTypedValue(out);
    }
  }

  const firstStart = process.hrtime.bigint();
  const firstOut = await invokeOnce();
  const firstNs = Number(process.hrtime.bigint() - firstStart);
  checkOnce(firstOut);

  const sampleNs = BigInt(Math.round(sampling.sample_seconds * 1e9));
  const minWarmupNs = BigInt(Math.round(sampling.minimum_warmup_seconds * 1e9));
  const maxWarmupNs = BigInt(Math.round(sampling.maximum_warmup_seconds * 1e9));

  async function runWindow(phase, index) {
    let elapsedNs = 0n;
    let iterations = 0;
    while (elapsedNs < sampleNs) {
      const t0 = process.hrtime.bigint();
      const out = await invokeOnce();
      elapsedNs += process.hrtime.bigint() - t0;
      checkOnce(out);
      iterations += 1;
    }
    const elapsed = Number(elapsedNs);
    const nsPerOp = elapsed / iterations;
    return { phase, index, elapsed_ns: elapsed, iterations, ns_per_op: nsPerOp };
  }

  const warmupRows = [];
  const warmupValues = [];
  const warmupStart = process.hrtime.bigint();
  let warmupElapsed = 0n;
  let converged = false;
  do {
    const row = await runWindow("warmup", warmupRows.length);
    warmupRows.push(row);
    warmupValues.push(row.ns_per_op);
    warmupElapsed = process.hrtime.bigint() - warmupStart;
    converged =
      warmupElapsed >= minWarmupNs &&
      stable(warmupValues, sampling.convergence_windows, sampling.maximum_relative_spread);
  } while (!converged && warmupElapsed < maxWarmupNs);

  const measurementRows = [];
  for (let i = 0; i < sampling.samples; i++) {
    measurementRows.push(await runWindow("measurement", i));
  }

  const mem = process.memoryUsage();
  const report = {
    schema_version: 1,
    runtime: "napi-node",
    op_mode: opMode,
    node: process.version,
    platform: process.platform,
    arch: process.arch,
    preflight_raw_byte_verified: true,
    initialize_ns: initNs,
    first_request_ns: firstNs,
    warmup: {
      elapsed_ns: Number(warmupElapsed),
      converged,
      windows: warmupRows.length,
      rows: warmupRows,
    },
    measurements: measurementRows,
    node_memory_usage: {
      rss: mem.rss,
      heapTotal: mem.heapTotal,
      heapUsed: mem.heapUsed,
      external: mem.external,
      arrayBuffers: mem.arrayBuffers,
    },
  };
  await writeFile(resolve(resultPathArg), JSON.stringify(report, null, 2) + "\n", "utf8");
} finally {
  await ev.close();
}
