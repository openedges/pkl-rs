// SPDX-License-Identifier: Apache-2.0
// Shared browser + Node case executor for Milestone M6 (@pkl-rs/wasm and @pkl-rs/napi).
import { serializeCanonicalValue, serializeCanonicalSchema } from "./canonical_value.mjs";

const encoder = new TextEncoder();

function toBytes(text) {
  return encoder.encode(text);
}

function bytesToBase64(u8) {
  if (typeof Buffer !== "undefined") {
    return Buffer.from(u8).toString("base64");
  }
  let bin = "";
  const chunk = 0x8000;
  for (let i = 0; i < u8.length; i += chunk) {
    bin += String.fromCharCode(...u8.subarray(i, i + chunk));
  }
  return btoa(bin);
}

function base64ToBytes(b64) {
  if (typeof Buffer !== "undefined") {
    return new Uint8Array(Buffer.from(b64, "base64"));
  }
  const bin = atob(b64);
  const out = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) out[i] = bin.charCodeAt(i);
  return out;
}

function nowNs() {
  if (typeof process !== "undefined" && process.hrtime?.bigint) {
    return process.hrtime.bigint();
  }
  return BigInt(Math.round(performance.now() * 1e6));
}

function formatPklErrorText(err) {
  const msg = err && typeof err.message === "string" ? err.message : String(err);
  return msg.endsWith("\n") ? msg : msg + "\n";
}

function buildMemReaders() {
  const MODULES = {
    "/a.pkl":
      'greeting = "hello from memmod"\nvalue = trace(42)\n@Deprecated { message = "use value" }\nfunction oldFn() = 7\n',
    "/dir/b.pkl": "nested = true\n",
  };
  const RESOURCES = {
    "/secret.txt": "top-secret-42",
    "/items/one.txt": "1",
    "/items/two.txt": "2",
  };
  const pathOf = (uri) => {
    const idx = uri.indexOf(":");
    return idx >= 0 ? uri.slice(idx + 1) : uri;
  };
  const moduleReader = {
    scheme: "memmod",
    hasHierarchicalUris: true,
    isGlobbable: true,
    isLocal: false,
    async read(uri) {
      await Promise.resolve();
      const p = pathOf(uri);
      if (!(p in MODULES)) throw new Error(`memmod not found: ${uri}`);
      return MODULES[p];
    },
    async hasElement(uri) {
      await Promise.resolve();
      const p = pathOf(uri);
      return p in MODULES || p === "/" || p === "/dir";
    },
    async listElements(uri) {
      await Promise.resolve();
      const p = pathOf(uri);
      if (p === "/" || p === "") {
        return [
          { name: "a.pkl", isDirectory: false },
          { name: "dir", isDirectory: true },
        ];
      }
      if (p === "/dir" || p === "/dir/") {
        return [{ name: "b.pkl", isDirectory: false }];
      }
      return [];
    },
  };
  const resourceReader = {
    scheme: "memres",
    hasHierarchicalUris: true,
    isGlobbable: true,
    async read(uri) {
      await Promise.resolve();
      const p = pathOf(uri);
      if (!(p in RESOURCES)) return null;
      return toBytes(RESOURCES[p]);
    },
    async hasElement(uri) {
      await Promise.resolve();
      const p = pathOf(uri);
      return p in RESOURCES || p === "/" || p === "/items";
    },
    async listElements(uri) {
      await Promise.resolve();
      const p = pathOf(uri);
      if (p === "/items" || p === "/items/") {
        return [
          { name: "one.txt", isDirectory: false },
          { name: "two.txt", isDirectory: false },
        ];
      }
      if (p === "/" || p === "") {
        return [
          { name: "items", isDirectory: true },
          { name: "secret.txt", isDirectory: false },
        ];
      }
      return [];
    },
  };
  return { moduleReader, resourceReader };
}

function buildEvaluatorOptions(api, spec, ctx) {
  const opts = {};
  const c = spec.target_options || {};
  if (c.outputFormat !== undefined) opts.outputFormat = c.outputFormat;
  if (c.properties !== undefined) opts.properties = c.properties;
  if (c.env !== undefined) opts.env = c.env;
  if (c.traceMode !== undefined) opts.traceMode = c.traceMode;
  if (c.powerAssertions !== undefined) opts.powerAssertions = c.powerAssertions;
  if (c.color !== undefined) opts.color = c.color;
  if (c.timeoutMs !== undefined) opts.timeoutMs = c.timeoutMs;
  if (c.allowedModules !== undefined) opts.allowedModules = c.allowedModules;
  if (c.allowedResources !== undefined) opts.allowedResources = c.allowedResources;
  if (c.rootDir !== undefined) opts.rootDir = c.rootDir;
  if (c.workingDir !== undefined) opts.workingDir = c.workingDir;
  if (c.modulePaths !== undefined) opts.modulePaths = c.modulePaths;
  if (c.projectDir !== undefined) opts.projectDir = c.projectDir;
  if (c.cacheDir !== undefined) opts.cacheDir = c.cacheDir;
  if (c.noCache !== undefined) opts.noCache = c.noCache;
  if (c.http !== undefined) opts.http = c.http;
  if (c.externalModuleReaders !== undefined) opts.externalModuleReaders = c.externalModuleReaders;
  if (c.externalResourceReaders !== undefined) opts.externalResourceReaders = c.externalResourceReaders;

  if (c.vfsFiles && api.VirtualFileSystem) {
    const vfs = new api.VirtualFileSystem({}, c.vfsRootDir ? { rootDir: c.vfsRootDir } : undefined);
    for (const [vpath, content] of Object.entries(c.vfsFiles)) {
      vfs.write(vpath, content);
    }
    opts.vfs = vfs;
  }

  if (c.enableHttpFetch && ctx.httpFetch) {
    opts.httpFetch = ctx.httpFetch;
  }
  if (c.enablePackageTransport && ctx.packageTransport) {
    opts.packageTransport = ctx.packageTransport;
  }
  return opts;
}

function makeSource(api, spec) {
  const src = spec.source;
  if (!src) return null;
  if (src.kind === "text") {
    return src.uri ? api.ModuleSource.text(src.text, src.uri) : api.ModuleSource.text(src.text);
  }
  if (src.kind === "path") {
    return api.ModuleSource.path(src.path);
  }
  if (src.kind === "uri") {
    return api.ModuleSource.uri(src.uri);
  }
  if (src.kind === "modulePath") {
    return api.ModuleSource.modulePath(src.path);
  }
  throw new Error(`Unknown source kind: ${src.kind}`);
}

export async function executeTargetCase(api, spec, ctx) {
  const start = nowNs();
  let stdout = new Uint8Array(0);
  let stderr = new Uint8Array(0);
  let exitCode = 0;
  const files = {};
  const assertions = [];
  let capabilityGap = null;
  let telemetry = null;

  try {
    const action = spec.action;

    if (action === "capabilities") {
      if (api.PKL_VERSION !== "0.32.0") {
        assertions.push(`PKL_VERSION=${api.PKL_VERSION}`);
      }
      const caps = api.capabilities();
      const expected = spec.expected_capabilities || {};
      const requiredKeys = [
        "vfs", "asyncModuleLoader", "asyncResourceLoader", "nativeFs", "nativeHttp",
        "externalReaders", "modulePathJar", "projectResolution", "packageCache",
        "formatter", "repl", "schema", "analyzeImports", "bigint64",
        "uint8ArrayBytes", "clock64Bit", "platform", "runtime", "arch",
      ];
      for (const k of requiredKeys) {
        if (!(k in caps)) assertions.push(`missing_capability_key:${k}`);
      }
      for (const [k, v] of Object.entries(expected)) {
        if (caps[k] !== v) assertions.push(`capability_mismatch:${k}=${JSON.stringify(caps[k])}!=${JSON.stringify(v)}`);
      }
      stdout = toBytes("0.32.0\n");
    } else if (action === "capability_gap") {
      const capName = spec.expected_capability;
      const caps = api.capabilities();
      const capReportedFalse = caps[capName] === false;
      const opts = buildEvaluatorOptions(api, spec, ctx);
      let ev = null;
      let caught = null;
      try {
        ev = await api.createEvaluator(opts);
        await ev.evaluateOutputText(makeSource(api, spec));
      } catch (err) {
        caught = err;
      } finally {
        if (ev) await ev.close();
      }
      const verified =
        capReportedFalse &&
        caught != null &&
        caught.code === "capability-unavailable" &&
        caught.capability === capName &&
        typeof caught.message === "string" &&
        caught.message.length > 0;
      capabilityGap = {
        expected_capability: capName,
        capability_reported_false: capReportedFalse,
        error_code: caught?.code ?? null,
        error_kind: caught?.kind ?? null,
        error_capability: caught?.capability ?? null,
        error_message: caught?.message ?? null,
        capability_contract_verified: verified,
      };
      if (caught) {
        stderr = toBytes(formatPklErrorText(caught));
        exitCode = 1;
      }
    } else if (action === "eval_text") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const out = await ev.evaluateOutputText(makeSource(api, spec));
        stdout = toBytes(out);
      } finally {
        await ev.close();
      }
    } else if (action === "eval_output_bytes") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const bytes = await ev.evaluateOutputBytes(makeSource(api, spec));
        if (!(bytes instanceof Uint8Array)) assertions.push("output_bytes_not_uint8array");
        stdout = new Uint8Array(bytes);
      } finally {
        await ev.close();
      }
    } else if (action === "eval_output_files") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const map = await ev.evaluateOutputFiles(makeSource(api, spec));
        if (!(map instanceof Map)) assertions.push("output_files_not_map");
        const orderLines = [];
        for (const [rel, u8] of map.entries()) {
          if (!(u8 instanceof Uint8Array)) assertions.push(`file_not_uint8array:${rel}`);
          orderLines.push(rel);
          files[`generated/${rel}`] = bytesToBase64(u8);
        }
        stdout = toBytes(orderLines.join("\n") + (orderLines.length ? "\n" : ""));
      } finally {
        await ev.close();
      }
    } else if (action === "eval_expr_and_binary") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const src = makeSource(api, spec);
        const expr = spec.expr;
        if (spec.binary) {
          const bin = await ev.evaluateExpressionPklBinary(src, expr);
          if (!(bin instanceof Uint8Array)) assertions.push("expr_binary_not_uint8array");
          stdout = new Uint8Array(bin);
        } else {
          const txt = await ev.evaluateExpression(src, expr);
          stdout = toBytes(txt + "\n");
        }
      } finally {
        await ev.close();
      }
    } else if (action === "eval_decode_values") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const src = makeSource(api, spec);
        const expr = spec.expr || "output.value";
        const rawBin = await ev.evaluateExpressionPklBinary(src, expr);
        const decoded1 = api.decodePklBinary(rawBin);
        const decoded2 = await ev.evaluateOutputValue(src);
        const c1 = serializeCanonicalValue(decoded1, api);
        const c2 = serializeCanonicalValue(decoded2, api);
        if (c1 !== c2) assertions.push("decodePklBinary_differs_from_evaluateOutputValue");
        // Verify owned Uint8Array copy isolation
        const props = decoded1?.properties || {};
        if (props.rawBytes instanceof Uint8Array && props.rawBytes.length > 0) {
          const orig0 = props.rawBytes[0];
          props.rawBytes[0] = (orig0 ^ 0xff) & 0xff;
          const decoded3 = api.decodePklBinary(rawBin);
          if (decoded3.properties.rawBytes[0] !== orig0) {
            assertions.push("uint8array_mutated_internal_buffer");
          }
        }
        // Verify out-of-range BigInt rejection if roundtrip helper exists
        if (typeof api.roundtripInteger === "function") {
          for (const bad of [9223372036854775808n, -9223372036854775809n]) {
            let rejected = false;
            try {
              api.roundtripInteger(bad);
            } catch {
              rejected = true;
            }
            if (!rejected) assertions.push(`out_of_range_bigint_accepted:${bad}`);
          }
        }
        stdout = toBytes(c1 + "\n");
      } finally {
        await ev.close();
      }
    } else if (action === "eval_schema") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const schema = await ev.evaluateSchema(makeSource(api, spec));
        stdout = toBytes(serializeCanonicalSchema(schema));
      } finally {
        await ev.close();
      }
    } else if (action === "eval_format") {
      const gv = spec.grammarVersion ?? 2;
      const pass1 = api.format(spec.formatInput, { grammarVersion: gv });
      const pass2 = api.format(pass1, { grammarVersion: gv });
      if (pass1 !== pass2) assertions.push("format_not_idempotent");
      stdout = toBytes(pass1);
    } else if (action === "eval_repl") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const repl = await api.createRepl(opts);
      const outLines = [];
      let reqIdx = 0;
      try {
        for (const raw of spec.replCommands) {
          const line = raw.trim();
          if (!line || line.startsWith("#")) continue;
          const id = `req-${++reqIdx}`;
          if (line.startsWith("EVAL ")) {
            const resps = await repl.eval(line.slice(5), { id });
            for (const r of resps) {
              if (r.kind === "evalSuccess") {
                outLines.push(`OK|${String(r.result ?? "").replace(/\n/g, "\\n")}`);
              } else if (r.kind === "evalError") {
                outLines.push(`ERR|${String(r.message ?? "").replace(/\n/g, "\\n")}`);
              } else {
                outLines.push(`OTHER|${r.kind}`);
              }
            }
          } else if (line === "RESET") {
            await repl.reset();
          } else if (line.startsWith("COMPLETE ")) {
            const items = (await repl.complete(line.slice(9))).slice().sort();
            outLines.push(`COMP|${items.join(",")}`);
          }
        }
      } finally {
        await repl.close();
        if (!repl.isClosed) assertions.push("repl_not_closed");
      }
      stdout = toBytes(outLines.join("\n") + "\n");
    } else if (action === "eval_analyze") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const graph = await api.analyzeImportGraph(spec.analyzeUris, opts);
      const basePrefix = spec.analyzeBaseUriPrefix || "";
      const norm = (u) => (basePrefix && u.startsWith(basePrefix) ? "base:/" + u.slice(basePrefix.length) : u);
      const rows = new Map();
      for (const [k, targets] of Object.entries(graph.imports || {})) {
        const nk = norm(k);
        const nt = [...new Set(targets.map(norm))].sort();
        rows.set(nk, nt);
      }
      const lines = [...rows.keys()].sort().map((k) => `${k} -> ${rows.get(k).join(",")}`);
      stdout = toBytes(lines.join("\n") + "\n");
    } else if (action === "eval_test") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      try {
        const res = await ev.evaluateTest(makeSource(api, spec), { reporter: spec.reporter || "spec" });
        stdout = toBytes(res.rendered || "");
        exitCode = res.failed ? 10 : 0;
      } finally {
        await ev.close();
      }
    } else if (action === "eval_custom_spi") {
      const { moduleReader, resourceReader } = buildMemReaders();
      const logs = [];
      const baseOpts = buildEvaluatorOptions(api, spec, ctx);
      const opts = {
        ...baseOpts,
        allowedModules: ["pkl:", "repl:", "file:", "^memmod:"],
        allowedResources: ["env:", "prop:", "^memres:"],
        moduleReaders: [moduleReader],
        resourceReaders: [resourceReader],
        logger: {
          trace(message, frame) {
            logs.push(`TRACE|${frame.moduleUri}|${frame.startLine}|${message}\n`);
          },
          warn(message, frame) {
            logs.push(`WARN|${frame.moduleUri}|${frame.startLine}|${message}\n`);
          },
        },
        frameTransformer(frame) {
          if (frame.moduleUri && frame.moduleUri.startsWith("memmod:")) {
            return { ...frame, moduleUri: `${frame.moduleUri}?transformed=1` };
          }
          return frame;
        },
      };
      const ev = await api.createEvaluator(opts);
      try {
        const out = await ev.evaluateOutputText(makeSource(api, spec));
        const combined = spec.withLogs ? `---OUTPUT---\n${out}---LOGS---\n${logs.join("")}` : out;
        stdout = toBytes(combined);
      } finally {
        await ev.close();
      }
    } else if (action === "eval_custom_security") {
      const baseOpts = buildEvaluatorOptions(api, spec, ctx);
      const opts = {
        ...baseOpts,
        securityManager: {
          checkImportModule(_importing, imported) {
            if (!imported.startsWith("pkl:") && !imported.startsWith("file:") && !imported.startsWith("repl:")) {
              throw new Error(`Custom security denied module: ${imported}`);
            }
          },
          checkReadResource(resource) {
            if (resource === "prop:pkl.outputFormat") return;
            throw new Error(`Custom security denied resource: ${resource}`);
          },
        },
      };
      const ev = await api.createEvaluator(opts);
      try {
        const out = await ev.evaluateOutputText(makeSource(api, spec));
        stdout = toBytes(out);
      } finally {
        await ev.close();
      }
    } else if (action === "eval_lifecycle") {
      const fastSrc = makeSource(api, { source: spec.fastSource });
      const slowSrc = makeSource(api, { source: spec.slowSource });
      const baseOpts = buildEvaluatorOptions(api, spec, ctx);

      // 1. Reuse + idempotent close + post-close rejection
      const ev1 = await api.createEvaluator(baseOpts);
      const first = await ev1.evaluateOutputText(fastSrc);
      const second = await ev1.evaluateOutputText(fastSrc);
      await ev1.close();
      await ev1.close();
      if (!ev1.isClosed) assertions.push("ev1_not_marked_closed");
      let closedRejected = false;
      try {
        await ev1.evaluateOutputText(fastSrc);
      } catch (e) {
        closedRejected = e?.kind === "closed" || e?.code === "closed";
      }

      // 2. Pre-aborted AbortSignal rejection
      const preCtrl = new AbortController();
      preCtrl.abort();
      const evAbort = await api.createEvaluator({ ...baseOpts, signal: preCtrl.signal });
      let preAbortRejected = false;
      try {
        await evAbort.evaluateOutputText(fastSrc);
      } catch (e) {
        preAbortRejected = e?.kind === "cancelled" || e?.code === "cancelled";
      } finally {
        await evAbort.close();
      }
      if (!preAbortRejected) assertions.push("pre_aborted_signal_not_rejected");

      // 3. Timeout enforcement
      const ev2 = await api.createEvaluator({ ...baseOpts, timeoutMs: 300 });
      let timeoutTriggered = false;
      try {
        await ev2.evaluateOutputText(slowSrc);
      } catch (e) {
        timeoutTriggered = e?.kind === "timeout" || e?.code === "timeout" || String(e?.message || "").includes("timed out");
      } finally {
        await ev2.close();
      }

      const lines = [
        `reuse_equal=${first === second}`,
        `reuse_output=${first.replace(/\n/g, "\\n")}`,
        `closed_rejected=${closedRejected}`,
        `timeout_triggered=${timeoutTriggered}`,
      ];
      stdout = toBytes(lines.join("\n") + "\n");
    } else if (action === "eval_clock64") {
      // Must accept > 2^32 - 1 ms (5_000_000_000 ms > 49.7 days) and reject negative / NaN / Infinity
      const ev = await api.createEvaluator({ timeoutMs: 5_000_000_000 });
      try {
        const out = await ev.evaluateOutputText(makeSource(api, spec));
        stdout = toBytes(out);
      } finally {
        await ev.close();
      }
      for (const bad of [-1, Number.NaN, Number.POSITIVE_INFINITY, 1.5]) {
        let rejected = false;
        try {
          const badEv = await api.createEvaluator({ timeoutMs: bad });
          await badEv.close();
        } catch (e) {
          rejected = e?.kind === "invalid-argument" || e?.code === "invalid-argument" || e instanceof Error;
        }
        if (!rejected) assertions.push(`invalid_timeout_accepted:${bad}`);
      }
    } else if (action === "eval_nonblocking") {
      // Verify Node event loop keeps ticking while async eval runs off-thread
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      let ticks = 0;
      const timer = setInterval(() => {
        ticks += 1;
      }, 5);
      try {
        const out = await ev.evaluateOutputText(makeSource(api, spec));
        stdout = toBytes(out);
      } finally {
        clearInterval(timer);
        await ev.close();
      }
      if (ticks < 2) assertions.push(`event_loop_blocked:ticks=${ticks}`);
    } else if (action === "eval_conversion_telemetry") {
      const opts = buildEvaluatorOptions(api, spec, ctx);
      const ev = await api.createEvaluator(opts);
      const iterations = spec.iterations || 20;
      try {
        const src = makeSource(api, spec);
        const rawBin = await ev.evaluateExpressionPklBinary(src, "output.value");
        const firstVal = await ev.evaluateOutputValue(src);
        const canonical = serializeCanonicalValue(firstVal, api);
        const samplesNs = [];
        for (let i = 0; i < iterations; i++) {
          const t0 = nowNs();
          const val = api.decodePklBinary(rawBin);
          const t1 = nowNs();
          if (!val || !val.properties) assertions.push("telemetry_decode_invalid");
          samplesNs.push(Number(t1 - t0));
        }
        samplesNs.sort((a, b) => a - b);
        const totalNs = samplesNs.reduce((a, b) => a + b, 0);
        telemetry = {
          iterations,
          payload_bytes: rawBin.length,
          min_ns: samplesNs[0],
          median_ns: samplesNs[Math.floor(samplesNs.length / 2)],
          p95_ns: samplesNs[Math.min(samplesNs.length - 1, Math.floor(samplesNs.length * 0.95))],
          mean_ns: Math.round(totalNs / iterations),
          max_ns: samplesNs[samplesNs.length - 1],
        };
        stdout = toBytes(canonical + "\n");
      } finally {
        await ev.close();
      }
    } else if (action === "eval_project_load") {
      const proj = await api.Project.load(spec.projectDir);
      const ev = await api.createEvaluator({
        project: proj,
        cacheDir: spec.cacheDir ?? null,
        http: spec.http,
      });
      try {
        const out = await ev.evaluateOutputText(makeSource(api, spec));
        stdout = toBytes(out);
      } finally {
        await ev.close();
      }
    } else {
      throw new Error(`Unsupported M6 action: ${action}`);
    }
  } catch (err) {
    stderr = toBytes(formatPklErrorText(err));
    exitCode = 1;
  }

  return {
    id: spec.id,
    stdout_b64: bytesToBase64(stdout),
    stderr_b64: bytesToBase64(stderr),
    exit_code: exitCode,
    files,
    assertions,
    capability_gap: capabilityGap,
    telemetry,
    elapsed_ns: Number(nowNs() - start),
  };
}
