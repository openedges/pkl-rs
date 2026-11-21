import { PklError, toPklError } from './errors.mjs';
import {
  buildRustHostConfig,
  driveWasmSession,
  monotonicNowMs,
  normalizeTimeoutMs,
} from './host.mjs';
import {
  PKL_VERSION,
  capabilities,
  ensureInitSync,
  getWasmBindings,
  init,
} from './runtime.mjs';
import {
  PklClass,
  PklDataSize,
  PklDuration,
  PklFunction,
  PklIntSeq,
  PklObject,
  PklPair,
  PklReference,
  PklRegex,
  PklTypeAlias,
  dehydrateValue,
  hydrateSchema,
  hydrateValue,
} from './values.mjs';
import { VirtualFileSystem, fileUriToPath, pathToFileUri } from './vfs.mjs';

export {
  PKL_VERSION,
  PklClass,
  PklDataSize,
  PklDuration,
  PklError,
  PklFunction,
  PklIntSeq,
  PklObject,
  PklPair,
  PklReference,
  PklRegex,
  PklTypeAlias,
  VirtualFileSystem,
  capabilities,
  init,
};

/**
 * Module input constructors matching Apple Pkl `ModuleSource`.
 */
export const ModuleSource = Object.freeze({
  /**
   * Inline Pkl source text (defaults to `uri = "repl:text"`).
   * @param {string} text
   * @param {string} [uri]
   * @returns {{ readonly uri: string, readonly contents: string }}
   */
  text(text, uri = 'repl:text') {
    if (typeof text !== 'string') {
      throw new PklError('ModuleSource.text expects a string', 'invalid-argument');
    }
    if (typeof uri !== 'string') {
      throw new PklError('ModuleSource.text uri must be a string', 'invalid-argument');
    }
    return Object.freeze({ uri, contents: text });
  },

  /**
   * Module backed by an arbitrary URI (`file:///...`, `https://...`, `package://...`, custom scheme).
   * @param {string} uri
   * @returns {{ readonly uri: string }}
   */
  uri(uri) {
    if (typeof uri !== 'string' || uri.length === 0) {
      throw new PklError('ModuleSource.uri expects a non-empty URI string', 'invalid-argument');
    }
    return Object.freeze({ uri });
  },

  /**
   * Module backed by a VFS/POSIX path converted to a `file://` URI.
   * @param {string} path
   * @returns {{ readonly uri: string }}
   */
  path(path) {
    return Object.freeze({ uri: pathToFileUri(path) });
  },

  /**
   * Module backed by a `modulepath:` URI.
   * @param {string} path
   * @returns {{ readonly uri: string }}
   */
  modulePath(path) {
    if (typeof path !== 'string' || path.length === 0) {
      throw new PklError('ModuleSource.modulePath expects a non-empty path', 'invalid-argument');
    }
    const uri = path.startsWith('modulepath:')
      ? path
      : path.startsWith('/')
        ? `modulepath:${path}`
        : `modulepath:/${path}`;
    return Object.freeze({ uri });
  },
});

function normalizeModuleSource(source) {
  if (!source || typeof source !== 'object' || typeof source.uri !== 'string') {
    throw new PklError(
      'Expected a ModuleSource created via ModuleSource.text/uri/path/modulePath',
      'invalid-argument',
    );
  }
  return {
    uri: source.uri,
    contents: source.contents === undefined ? null : String(source.contents),
  };
}

/**
 * Formats Pkl source text with `pkl-formatter` (grammar version 1 or 2, default 2).
 * Auto-initializes synchronously in Node.js; call `await init()` first in browsers.
 *
 * @param {string} source
 * @param {{ grammarVersion?: 1 | 2 }} [options]
 * @returns {string}
 */
export function format(source, options = undefined) {
  const wasm = getWasmBindings();
  const gv = options?.grammarVersion ?? 2;
  try {
    return wasm.wasmFormat(source, gv);
  } catch (err) {
    throw toPklError(err);
  }
}

/**
 * Decodes raw `pkl-binary` MessagePack bytes into lossless JS/TS Pkl values.
 * Auto-initializes synchronously in Node.js; call `await init()` first in browsers.
 *
 * @param {Uint8Array} bytes
 * @param {{ strict?: boolean }} [options]
 * @returns {any}
 */
export function decodePklBinary(bytes, options = undefined) {
  const wasm = getWasmBindings();
  if (!(bytes instanceof Uint8Array)) {
    throw new PklError('decodePklBinary expects a Uint8Array', 'invalid-argument');
  }
  try {
    const raw = wasm.wasmDecodePklBinary(bytes, Boolean(options?.strict));
    return hydrateValue(raw);
  } catch (err) {
    throw toPklError(err, 'decode');
  }
}

/**
 * Lossless JS <-> Rust value round-trip helper across the Wasm boundary.
 * @param {any} value
 * @returns {any}
 */
export function roundtripValue(value) {
  const wasm = getWasmBindings();
  const dehydrated = dehydrateValue(value);
  try {
    const raw = wasm.wasmRoundtripValue(dehydrated);
    return hydrateValue(raw);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Lossless signed 64-bit BigInt round-trip helper (`i64::MIN..=i64::MAX`).
 * @param {bigint} value
 * @returns {bigint}
 */
export function roundtripInteger(value) {
  const wasm = getWasmBindings();
  try {
    return wasm.wasmValidateInt64(value);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Lossless owned `Uint8Array` round-trip helper.
 * @param {Uint8Array} value
 * @returns {Uint8Array}
 */
export function roundtripBytes(value) {
  const wasm = getWasmBindings();
  if (!(value instanceof Uint8Array)) {
    throw new PklError('expected Uint8Array', 'invalid-argument');
  }
  try {
    return wasm.roundtripBytes(value);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Lossless UTF-16 string round-trip helper (preserves unpaired surrogates and astral scalars).
 * @param {string} value
 * @returns {string}
 */
export function roundtripString(value) {
  const wasm = getWasmBindings();
  if (typeof value !== 'string') {
    throw new PklError('expected string', 'invalid-argument');
  }
  try {
    return wasm.roundtripString(value);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Measures JS <-> Rust Wasm value conversion round-trip overhead for M6 telemetry.
 * @param {number} [iterations]
 * @param {any} [sampleValue]
 */
export function measureConversion(iterations = 1000, sampleValue = undefined) {
  ensureInitSync();
  if (!Number.isInteger(iterations) || iterations <= 0) {
    throw new PklError('iterations must be a positive integer', 'invalid-argument');
  }
  const payload =
    sampleValue !== undefined
      ? sampleValue
      : new PklObject(
          'Benchmark#Payload',
          'file:///bench.pkl',
          {
            id: 9223372036854775807n,
            ratio: 3.141592653589793,
            label: 'hello-🚀-pkl',
            active: true,
            raw: new Uint8Array([0, 1, 127, 128, 255]),
            dur: new PklDuration(250, 'ms'),
            size: new PklDataSize(64, 'mib'),
            tags: ['a', 'b', 'c'],
          },
          new Map([['k', 42n]]),
          ['elem0'],
        );

  const now = () =>
    typeof performance !== 'undefined' && typeof performance.now === 'function'
      ? performance.now()
      : Date.now();
  const t0 = now();
  let last;
  for (let i = 0; i < iterations; i += 1) {
    last = roundtripValue(payload);
  }
  const totalMs = Math.max(0, now() - t0);
  return {
    iterations,
    totalMs,
    avgUs: (totalMs * 1000) / iterations,
    opsPerSec: totalMs > 0 ? (iterations * 1000) / totalMs : Infinity,
    sample: last,
  };
}

function pklEscapeStr(s) {
  let out = '"';
  for (const ch of s) {
    if (ch === '\\') out += '\\\\';
    else if (ch === '"') out += '\\"';
    else if (ch === '\n') out += '\\n';
    else if (ch === '\r') out += '\\r';
    else if (ch === '\t') out += '\\t';
    else out += ch;
  }
  out += '"';
  return out;
}

function extractShortErrorMessage(rendered) {
  // Strip ANSI escape codes if colored
  // eslint-disable-next-line no-control-regex
  const stripped = rendered.replace(/\x1b\[[0-9;]*m/g, '');
  const body = stripped.startsWith('–– Pkl Error ––\n')
    ? stripped.slice('–– Pkl Error ––\n'.length)
    : stripped;
  const firstPara = body.split('\n\n')[0] ?? body;
  return firstPara.replace(/\n+$/, '');
}

const OUTPUT_FILES_EXPR =
  'let (f = output.files) if (f == null) Map() else f.toMap().mapValues((_, v) -> v.bytes)';

const TEST_CHECK_EXPR = `
let (reflect = import("pkl:reflect"))
let (testMod = import("pkl:test"))
let (m = reflect.Module(module))
new Mapping {
  ["moduleName"] = m.name
  ["moduleUri"] = m.uri
  ["isTest"] = reflect.Class(module.getClass()).isSubclassOf(reflect.Class(testMod.getClass()))
}
`;

const RENDER_ACTUAL_EXAMPLES_EXPR = `
let (r = new PcfRenderer { omitNullProperties = false })
examples.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
`;

const RENDER_EXPECTED_EXAMPLES_EXPR = `
let (r = new PcfRenderer { omitNullProperties = false })
let (ex = examples)
if (ex == null) throw("Invalid output file structure")
else ex.toMap().mapValues((_, g) ->
  g.toList().map((v) ->
    let (s = r.renderValue(v))
    if (v is Typed || v is Dynamic || v is Listing || v is Mapping) "new " + s else s
  )
)
`;

const RENDER_EXAMPLES_DOC_EXPR = `
let (ex = examples)
if (ex == null) ""
else new PcfRenderer { omitNullProperties = false; useCustomStringDelimiters = true }.renderDocument(new Dynamic { examples = ex })
`;

/**
 * Product Pkl 0.32.0 WebAssembly evaluator with module caching across calls,
 * cooperative async Host suspension, 64-bit timeouts, and explicit lifecycle cleanup.
 */
export class Evaluator {
  /**
   * @param {any} wasmEvaluator
   * @param {any} wasmBindings
   * @param {any} hostHooks
   * @param {bigint | null} defaultTimeoutMs
   * @param {AbortSignal | null} defaultSignal
   */
  constructor(
    wasmEvaluator,
    wasmBindings,
    hostHooks,
    defaultTimeoutMs,
    defaultSignal,
    deferredCapabilityError = null,
  ) {
    this._wasm = wasmEvaluator;
    this._bindings = wasmBindings;
    this._hostHooks = hostHooks;
    this._defaultTimeoutMs = defaultTimeoutMs;
    this._defaultSignal = defaultSignal;
    this._deferredCapabilityError = deferredCapabilityError;
    this._clockState = { last: 0n };
    this._closed = false;
    this._cancelled = false;
    /** @type {Set<(kind: "cancelled" | "closed") => void>} */
    this._activeCancelHooks = new Set();
    /** @type {Promise<any>} */
    this._queue = Promise.resolve();
    this._testLogBuffer = /** @type {string[] | null} */ (null);

    this._onDefaultAbort = () => {
      this.cancel();
    };
    if (this._defaultSignal) {
      if (this._defaultSignal.aborted) {
        this._cancelled = true;
      } else {
        this._defaultSignal.addEventListener('abort', this._onDefaultAbort, { once: true });
      }
    }
  }

  get isClosed() {
    return this._closed || this._wasm.isClosed();
  }

  cancel() {
    if (this._cancelled) return;
    this._cancelled = true;
    this._wasm.cancel();
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('cancelled');
    }
  }

  close() {
    if (this._closed) return;
    this._closed = true;
    if (this._defaultSignal) {
      this._defaultSignal.removeEventListener('abort', this._onDefaultAbort);
    }
    this._wasm.close();
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('closed');
    }
  }

  /**
   * Serializes operations on one `WasmEvaluator` instance while respecting `close()`,
   * pre-aborted `signal`, `cancel()`, and per-call `timeoutMs` / `signal`.
   */
  _runSerialized(callOptions, startFn) {
    const task = async () => {
      if (this._closed || this._wasm.isClosed()) {
        throw new PklError('Evaluator has been closed.', 'closed');
      }
      if (this._deferredCapabilityError) {
        throw this._deferredCapabilityError;
      }
      if (
        this._cancelled ||
        this._defaultSignal?.aborted ||
        callOptions?.signal?.aborted
      ) {
        throw new PklError('Evaluation was cancelled.', 'cancelled');
      }
      const timeoutMs =
        callOptions?.timeoutMs !== undefined
          ? normalizeTimeoutMs(callOptions.timeoutMs)
          : this._defaultTimeoutMs;
      const now = monotonicNowMs(this._clockState);
      try {
        startFn(now, timeoutMs);
      } catch (err) {
        throw toPklError(err);
      }
      const signals = [];
      if (this._defaultSignal) signals.push(this._defaultSignal);
      if (callOptions?.signal) signals.push(callOptions.signal);
      return driveWasmSession({
        wasmObj: this._wasm,
        hostHooks: this._hostHooks,
        clockState: this._clockState,
        timeoutMs,
        signals,
        onRegisterCancel: (fn) => {
          this._activeCancelHooks.add(fn);
          return () => this._activeCancelHooks.delete(fn);
        },
        onCapturedLog: this._testLogBuffer
          ? (msg) => {
              if (this._testLogBuffer) this._testLogBuffer.push(msg);
            }
          : undefined,
      });
    };

    const next = this._queue.then(task, task);
    this._queue = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  /**
   * Evaluates a module's `output.text`.
   * @param {any} source
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<string>}
   */
  async evaluateOutputText(source, callOptions = undefined) {
    const spec = normalizeModuleSource(source);
    const res = await this._runSerialized(callOptions, (now, timeoutMs) => {
      this._wasm.startOutputText(spec.uri, spec.contents, now, timeoutMs);
    });
    return res.value;
  }

  /**
   * Evaluates a module's `output.bytes` as an owned `Uint8Array`.
   * @param {any} source
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<Uint8Array>}
   */
  async evaluateOutputBytes(source, callOptions = undefined) {
    const spec = normalizeModuleSource(source);
    const res = await this._runSerialized(callOptions, (now, timeoutMs) => {
      this._wasm.startOutputBytes(spec.uri, spec.contents, now, timeoutMs);
    });
    return res.value;
  }

  /**
   * Evaluates a module's `output.files` preserving Pkl `Mapping` order (`Map<string, Uint8Array>`).
   * @param {any} source
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<Map<string, Uint8Array>>}
   */
  async evaluateOutputFiles(source, callOptions = undefined) {
    const bytes = await this.evaluateExpressionPklBinary(source, OUTPUT_FILES_EXPR, callOptions);
    const decoded = decodePklBinary(bytes);
    const files = new Map();
    if (decoded instanceof Map) {
      const enc = new TextEncoder();
      for (const [k, v] of decoded.entries()) {
        const key = String(k);
        if (v instanceof Uint8Array) {
          files.set(key, v);
        } else if (typeof v === 'string') {
          files.set(key, enc.encode(v));
        } else {
          throw new PklError(`Unexpected FileOutput.bytes value for \`${key}\``, 'decode');
        }
      }
    }
    return files;
  }

  /**
   * Evaluates a module's `output.value` decoded into a lossless `PklValue`.
   * @param {any} source
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<any>}
   */
  async evaluateOutputValue(source, callOptions = undefined) {
    const bytes = await this.evaluateExpressionPklBinary(source, 'output.value', callOptions);
    return decodePklBinary(bytes);
  }

  /**
   * Evaluates a Pkl expression against `source` and returns its string representation.
   * @param {any} source
   * @param {string} expr
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<string>}
   */
  async evaluateExpression(source, expr, callOptions = undefined) {
    const spec = normalizeModuleSource(source);
    if (typeof expr !== 'string') {
      throw new PklError('expression must be a string', 'invalid-argument');
    }
    const res = await this._runSerialized(callOptions, (now, timeoutMs) => {
      this._wasm.startExpression(spec.uri, spec.contents, expr, now, timeoutMs);
    });
    return res.value;
  }

  /**
   * Evaluates a Pkl expression against `source` and returns raw `pkl-binary` `Uint8Array` bytes.
   * @param {any} source
   * @param {string} expr
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<Uint8Array>}
   */
  async evaluateExpressionPklBinary(source, expr, callOptions = undefined) {
    const spec = normalizeModuleSource(source);
    if (typeof expr !== 'string') {
      throw new PklError('expression must be a string', 'invalid-argument');
    }
    const res = await this._runSerialized(callOptions, (now, timeoutMs) => {
      this._wasm.startExpressionPklBinary(spec.uri, spec.contents, expr, now, timeoutMs);
    });
    return res.value;
  }

  /**
   * Evaluates a Pkl expression against `source` and decodes it into a lossless `PklValue`.
   * @param {any} source
   * @param {string} expr
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<any>}
   */
  async evaluateExpressionValue(source, expr, callOptions = undefined) {
    const bytes = await this.evaluateExpressionPklBinary(source, expr, callOptions);
    return decodePklBinary(bytes);
  }

  /**
   * Extracts the `ModuleSchema` of `source`.
   * @param {any} source
   * @param {{ timeoutMs?: number | bigint, signal?: AbortSignal }} [callOptions]
   * @returns {Promise<any>}
   */
  async evaluateSchema(source, callOptions = undefined) {
    const spec = normalizeModuleSource(source);
    const res = await this._runSerialized(callOptions, (now, timeoutMs) => {
      this._wasm.startSchema(spec.uri, spec.contents, now, timeoutMs);
    });
    return hydrateSchema(res.value);
  }

  /**
   * Evaluates a `pkl:test` module (`facts` and `examples`), returning structured `TestResults`
   * plus Pkl 0.32.0 `rendered` output (`spec` | `minimal` | `junit`).
   *
   * @param {any} source
   * @param {{ overwrite?: boolean, reporter?: "spec" | "minimal" | "junit", timeoutMs?: number | bigint, signal?: AbortSignal }} [options]
   * @returns {Promise<any>}
   */
  async evaluateTest(source, options = undefined) {
    if (this.isClosed) {
      throw new PklError('Evaluator has been closed.', 'closed');
    }
    const spec = normalizeModuleSource(source);
    const reporter = options?.reporter ?? 'spec';
    const overwrite = Boolean(options?.overwrite);
    const callOptions = { timeoutMs: options?.timeoutMs, signal: options?.signal };
    const useColor = this._hostHooks.useColor;

    this._testLogBuffer = [];
    this._wasm.setCaptureLogs(true);
    try {
      const checkVal = await this.evaluateExpressionValue(source, TEST_CHECK_EXPR, callOptions);
      const moduleName = (checkVal instanceof Map && checkVal.get('moduleName')) || 'test';
      const moduleUri = (checkVal instanceof Map && checkVal.get('moduleUri')) || spec.uri;
      const isTest = Boolean(checkVal instanceof Map && checkVal.get('isTest'));
      const displayUri = moduleUri;

      if (!isTest) {
        let renderedErr;
        try {
          await this.evaluateExpression(source, 'module as import("pkl:test")', callOptions);
          renderedErr = `–– Pkl Error ––\nExpected value of type \`pkl.test\`, but got type \`Class\`.\nValue: ${moduleName}\n`;
        } catch (err) {
          renderedErr = err instanceof Error ? err.message : String(err);
        }
        const logs = this._testLogBuffer.map((l) => `${l}\n`).join('');
        const rawResults = {
          moduleName,
          displayUri,
          facts: { name: 'facts', results: [] },
          examples: { name: 'examples', results: [] },
          logs,
          error: {
            message: extractShortErrorMessage(renderedErr),
            rendered: renderedErr,
          },
        };
        const rendered = this._bindings.wasmRenderTestReport(rawResults, reporter, useColor);
        return finalizeTestResults(rawResults, rendered);
      }

      // Resolve source text for fact/example spans (inline or VFS)
      let moduleSourceText = spec.contents;
      if (moduleSourceText === null && moduleUri.startsWith('file:') && this._hostHooks.vfs) {
        try {
          const p = fileUriToPath(moduleUri);
          if (this._hostHooks.vfs.isFile(p)) {
            moduleSourceText = this._hostHooks.vfs.readText(p);
          }
        } catch {
          moduleSourceText = null;
        }
      }

      // 1. Evaluate `facts`
      const factsSection = await this._evaluateTestFacts(
        source,
        spec,
        moduleUri,
        moduleSourceText,
        useColor,
        callOptions,
      );

      // 2. Evaluate `examples`
      const examplesSection = await this._evaluateTestExamples(
        source,
        moduleUri,
        moduleSourceText,
        overwrite,
        useColor,
        callOptions,
      );

      const logs = this._testLogBuffer.map((l) => `${l}\n`).join('');
      const rawResults = {
        moduleName,
        displayUri,
        facts: factsSection,
        examples: examplesSection,
        logs,
        error: null,
      };
      const rendered = this._bindings.wasmRenderTestReport(rawResults, reporter, useColor);
      return finalizeTestResults(rawResults, rendered);
    } finally {
      this._wasm.setCaptureLogs(false);
      this._testLogBuffer = null;
    }
  }

  async _evaluateTestFacts(source, spec, moduleUri, moduleSourceText, useColor, callOptions) {
    const shape = await this.evaluateExpressionValue(
      source,
      'if (facts == null) null else facts.toMap().mapValues((_, g) -> g.length)',
      callOptions,
    );
    if (!(shape instanceof Map)) {
      return { name: 'facts', results: [] };
    }
    const factSpans =
      typeof moduleSourceText === 'string'
        ? this._bindings.wasmExtractFactSpans(moduleSourceText)
        : {};
    const results = [];

    for (const [groupKey, lenVal] of shape.entries()) {
      const groupName = String(groupKey);
      const count = typeof lenVal === 'bigint' ? Number(lenVal) : Number(lenVal || 0);
      const groupLocs = factSpans[groupName] || [];
      let totalAsserts = 0;
      const failures = [];
      const errors = [];

      for (let idx = 0; idx < count; idx += 1) {
        totalAsserts += 1;
        const expr = `facts[${pklEscapeStr(groupName)}][${idx}]`;
        try {
          const val = await this.evaluateExpressionValue(source, expr, callOptions);
          if (val === false) {
            const locInfo = groupLocs[idx];
            if (!locInfo) {
              failures.push(
                this._bindings.wasmFormatFactFailure(`fact #${idx} == false`, moduleUri, useColor),
              );
            } else {
              const location = moduleUri.startsWith('file:')
                ? `${moduleUri}, line ${locInfo.line}`
                : moduleUri;
              let paFailure = null;
              if (this._hostHooks.powerAssertions) {
                const probeExpr = `(null as Any(${locInfo.expr}))`;
                try {
                  const probeRes = await this._runSerialized(callOptions, (now, timeoutMs) => {
                    this._wasm.startPowerAssertionProbe(
                      spec.uri,
                      spec.contents,
                      probeExpr,
                      location,
                      useColor,
                      now,
                      timeoutMs,
                    );
                  });
                  paFailure = probeRes.value;
                } catch {
                  paFailure = null;
                }
              }
              failures.push(
                paFailure ||
                  this._bindings.wasmFormatFactFailure(locInfo.expr, location, useColor),
              );
            }
          }
        } catch (err) {
          const rendered = err instanceof Error ? err.message : String(err);
          errors.push({
            message: extractShortErrorMessage(rendered),
            rendered,
          });
        }
      }

      results.push({
        name: groupName,
        totalAsserts,
        failures,
        errors,
        isExampleWritten: false,
      });
    }

    return { name: 'facts', results };
  }

  async _evaluateTestExamples(
    source,
    moduleUri,
    moduleSourceText,
    overwrite,
    useColor,
    callOptions,
  ) {
    const isNull = await this.evaluateExpressionValue(source, 'examples == null', callOptions);
    if (isNull === true) {
      return { name: 'examples', results: [] };
    }
    if (!moduleUri.startsWith('file:')) {
      throw new PklError(
        `–– Pkl Error ––\nCannot evaluate examples for non-file-based test module \`${moduleUri}\`.\n`,
        'evaluation',
      );
    }
    const vfs = this._hostHooks.vfs;
    if (!vfs) {
      throw new PklError(
        `Cannot evaluate examples for \`${moduleUri}\`: native filesystem is unavailable in Wasm (attach a VirtualFileSystem via options.vfs).`,
        'capability-unavailable',
        'nativeFs',
      );
    }

    const modulePath = fileUriToPath(moduleUri);
    const expectedPath = `${modulePath}-expected.pcf`;
    const actualPath = `${modulePath}-actual.pcf`;
    const expectedUri = pathToFileUri(expectedPath);
    const actualUri = pathToFileUri(actualPath);

    if (vfs.isFile(actualPath)) vfs.delete(actualPath);
    if (overwrite && vfs.isFile(expectedPath)) vfs.delete(expectedPath);

    const actualMap = await this.evaluateExpressionValue(
      source,
      RENDER_ACTUAL_EXAMPLES_EXPR,
      callOptions,
    );
    const actualGroups = mapToRenderedGroups(actualMap);
    const fullDoc = await this.evaluateExpression(source, RENDER_EXAMPLES_DOC_EXPR, callOptions);

    let expectedGroups = null;
    let expectedSource = null;
    if (vfs.isFile(expectedPath)) {
      expectedSource = vfs.readText(expectedPath);
      const expMap = await this.evaluateExpressionValue(
        ModuleSource.text(expectedSource, expectedUri),
        RENDER_EXPECTED_EXAMPLES_EXPR,
        callOptions,
      );
      expectedGroups = mapToRenderedGroups(expMap);
    }

    const evalOut = this._bindings.wasmEvaluateExamples(
      actualGroups,
      expectedGroups,
      moduleUri,
      expectedUri,
      actualUri,
      fullDoc,
      moduleSourceText,
      expectedSource,
      useColor,
    );

    if (typeof evalOut.generatedExpectedPcf === 'string') {
      vfs.write(expectedPath, evalOut.generatedExpectedPcf);
    }
    if (typeof evalOut.generatedActualPcf === 'string') {
      vfs.write(actualPath, evalOut.generatedActualPcf);
    }
    return evalOut.section;
  }
}

function mapToRenderedGroups(val) {
  if (!(val instanceof Map)) return [];
  const groups = [];
  for (const [k, v] of val.entries()) {
    const items = Array.isArray(v) ? v.map((x) => String(x)) : [];
    groups.push([String(k), items]);
  }
  return groups;
}

function finalizeTestResults(raw, rendered) {
  const sumSection = (sec) => {
    const results = sec.results || [];
    let totalTests = results.length;
    let totalFailures = 0;
    let totalAsserts = 0;
    let totalAssertsFailed = 0;
    for (const r of results) {
      const f = (r.failures?.length || 0) + (r.errors?.length || 0);
      totalAsserts += r.totalAsserts || 0;
      totalAssertsFailed += f;
      if (f > 0) totalFailures += 1;
    }
    return { totalTests, totalFailures, totalAsserts, totalAssertsFailed };
  };
  const fStats = sumSection(raw.facts);
  const eStats = sumSection(raw.examples);
  const failed = Boolean(raw.error) || fStats.totalFailures > 0 || eStats.totalFailures > 0;
  return {
    moduleName: raw.moduleName,
    displayUri: raw.displayUri,
    failed,
    totalTests: fStats.totalTests + eStats.totalTests,
    totalFailures: fStats.totalFailures + eStats.totalFailures,
    totalAsserts: fStats.totalAsserts + eStats.totalAsserts,
    totalAssertsFailed: fStats.totalAssertsFailed + eStats.totalAssertsFailed,
    facts: raw.facts,
    examples: raw.examples,
    logs: raw.logs,
    error: raw.error,
    rendered,
  };
}

/**
 * Creates an async Pkl 0.32.0 WebAssembly `Evaluator`.
 * @param {any} [options]
 * @returns {Promise<Evaluator>}
 */
export async function createEvaluator(options = {}) {
  await init();
  const wasm = getWasmBindings();
  const timeoutMs = normalizeTimeoutMs(options?.timeoutMs);
  const { rustConfig, hostHooks, deferredCapabilityError } = buildRustHostConfig(options);
  let wasmEval;
  try {
    wasmEval = new wasm.WasmEvaluator(rustConfig);
  } catch (err) {
    throw toPklError(err, 'configuration');
  }
  return new Evaluator(
    wasmEval,
    wasm,
    hostHooks,
    timeoutMs,
    options?.signal ?? null,
    deferredCapabilityError,
  );
}

/**
 * Stateful interactive Pkl 0.32.0 WebAssembly REPL session.
 */
export class ReplSession {
  /**
   * @param {any} wasmRepl
   * @param {any} hostHooks
   * @param {bigint | null} defaultTimeoutMs
   * @param {AbortSignal | null} defaultSignal
   */
  constructor(
    wasmRepl,
    hostHooks,
    defaultTimeoutMs,
    defaultSignal,
    deferredCapabilityError = null,
  ) {
    this._wasm = wasmRepl;
    this._hostHooks = hostHooks;
    this._defaultTimeoutMs = defaultTimeoutMs;
    this._defaultSignal = defaultSignal;
    this._deferredCapabilityError = deferredCapabilityError;
    this._clockState = { last: 0n };
    this._nextId = 1;
    this._closed = false;
    this._cancelled = false;
    this._activeCancelHooks = new Set();
    this._queue = Promise.resolve();

    this._onDefaultAbort = () => {
      this._cancelled = true;
      this._wasm.cancel();
      for (const hook of Array.from(this._activeCancelHooks)) {
        hook('cancelled');
      }
    };
    if (this._defaultSignal) {
      if (this._defaultSignal.aborted) {
        this._cancelled = true;
      } else {
        this._defaultSignal.addEventListener('abort', this._onDefaultAbort, { once: true });
      }
    }
  }

  get isClosed() {
    return this._closed || this._wasm.isClosed();
  }

  close() {
    if (this._closed) return;
    this._closed = true;
    if (this._defaultSignal) {
      this._defaultSignal.removeEventListener('abort', this._onDefaultAbort);
    }
    this._wasm.close();
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('closed');
    }
  }

  _runRequest(explicitId, startFn) {
    const task = async () => {
      if (this.isClosed) {
        throw new PklError('REPL session has been closed.', 'closed');
      }
      if (this._deferredCapabilityError) {
        throw this._deferredCapabilityError;
      }
      if (this._cancelled || this._defaultSignal?.aborted) {
        throw new PklError('REPL evaluation was cancelled.', 'cancelled');
      }
      const seq = this._nextId++;
      const id =
        typeof explicitId === 'string' && explicitId.length > 0 ? explicitId : `req-${seq}`;
      const now = monotonicNowMs(this._clockState);
      try {
        startFn(id, now, this._defaultTimeoutMs);
      } catch (err) {
        throw toPklError(err);
      }
      const res = await driveWasmSession({
        wasmObj: this._wasm,
        hostHooks: this._hostHooks,
        clockState: this._clockState,
        timeoutMs: this._defaultTimeoutMs,
        signals: this._defaultSignal ? [this._defaultSignal] : [],
        onRegisterCancel: (fn) => {
          this._activeCancelHooks.add(fn);
          return () => this._activeCancelHooks.delete(fn);
        },
      });
      return res.responses || [];
    };
    const next = this._queue.then(task, task);
    this._queue = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  /**
   * Evaluates one or more Pkl REPL statements/expressions.
   * @param {string} input
   * @param {{ force?: boolean, evalDefinitions?: boolean, id?: string }} [options]
   * @returns {Promise<Array<{ kind: string, result?: string, message?: string }>>}
   */
  async eval(input, options = undefined) {
    if (typeof input !== 'string') {
      throw new PklError('ReplSession.eval expects a string input', 'invalid-argument');
    }
    const evalDefinitions = Boolean(options?.evalDefinitions);
    const forceResults = Boolean(options?.force);
    return this._runRequest(options?.id, (id, now, timeoutMs) => {
      this._wasm.startEval(id, input, evalDefinitions, forceResults, now, timeoutMs);
    });
  }

  /**
   * Loads a module by URI string or `ModuleSource` into the REPL environment.
   * @param {string | { uri: string, contents?: string }} uriOrSource
   * @param {{ id?: string }} [options]
   * @returns {Promise<Array<{ kind: string, result?: string, message?: string }>>}
   */
  async load(uriOrSource, options = undefined) {
    let uri;
    if (typeof uriOrSource === 'string') {
      uri = uriOrSource;
    } else if (uriOrSource && typeof uriOrSource === 'object' && typeof uriOrSource.uri === 'string') {
      uri = uriOrSource.uri;
      if (typeof uriOrSource.contents === 'string' && uri.startsWith('file:') && this._hostHooks.vfs) {
        this._hostHooks.vfs.write(fileUriToPath(uri), uriOrSource.contents);
      }
    } else {
      throw new PklError('ReplSession.load expects a URI string or ModuleSource', 'invalid-argument');
    }
    return this._runRequest(options?.id, (id, now, timeoutMs) => {
      this._wasm.startLoad(id, uri, now, timeoutMs);
    });
  }

  /**
   * Resets the REPL environment to a fresh empty module.
   * @param {{ id?: string }} [options]
   * @returns {Promise<void>}
   */
  async reset(options = undefined) {
    await this._runRequest(options?.id, (id, now, timeoutMs) => {
      this._wasm.startReset(id, now, timeoutMs);
    });
  }

  /**
   * Returns member/method completion candidates for `prefix`.
   * @param {string} prefix
   * @param {{ id?: string }} [options]
   * @returns {Promise<string[]>}
   */
  async complete(prefix, options = undefined) {
    if (typeof prefix !== 'string') {
      throw new PklError('ReplSession.complete expects a string prefix', 'invalid-argument');
    }
    const responses = await this._runRequest(options?.id, (id, now, timeoutMs) => {
      this._wasm.startComplete(id, prefix, now, timeoutMs);
    });
    const first = responses.find((r) => r.kind === 'completion');
    return first && Array.isArray(first.members) ? first.members : [];
  }
}

/**
 * Creates a stateful Pkl 0.32.0 WebAssembly `ReplSession`.
 * @param {any} [options]
 * @returns {Promise<ReplSession>}
 */
export async function createRepl(options = {}) {
  await init();
  const wasm = getWasmBindings();
  const timeoutMs = normalizeTimeoutMs(options?.timeoutMs);
  const { rustConfig, hostHooks, deferredCapabilityError } = buildRustHostConfig(options);
  let workingDirUri = 'file:///';
  if (typeof options?.workingDir === 'string' && options.workingDir.length > 0) {
    workingDirUri = options.workingDir.includes('://') || options.workingDir.startsWith('file:')
      ? options.workingDir
      : pathToFileUri(options.workingDir);
  } else if (hostHooks.vfs?.rootDir) {
    workingDirUri = pathToFileUri(hostHooks.vfs.rootDir);
  }
  if (!workingDirUri.endsWith('/')) workingDirUri += '/';

  let wasmRepl;
  try {
    wasmRepl = new wasm.WasmReplSession(rustConfig, workingDirUri);
  } catch (err) {
    throw toPklError(err, 'configuration');
  }
  return new ReplSession(
    wasmRepl,
    hostHooks,
    timeoutMs,
    options?.signal ?? null,
    deferredCapabilityError,
  );
}

/**
 * Analyzes the static import graph reachable from `uris`.
 * @param {string[]} uris
 * @param {any} [options]
 * @returns {Promise<{ imports: Record<string, string[]>, resolvedImports: Record<string, string> }>}
 */
export async function analyzeImportGraph(uris, options = {}) {
  await init();
  if (!Array.isArray(uris)) {
    throw new PklError('analyzeImportGraph expects an array of URI strings', 'invalid-argument');
  }
  const wasm = getWasmBindings();
  const timeoutMs = normalizeTimeoutMs(options?.timeoutMs);
  const { rustConfig, hostHooks, deferredCapabilityError } = buildRustHostConfig(options);
  if (deferredCapabilityError) {
    throw deferredCapabilityError;
  }
  let analyzer;
  try {
    analyzer = new wasm.WasmImportAnalyzer(uris, rustConfig);
  } catch (err) {
    throw toPklError(err, 'configuration');
  }
  try {
    const res = await driveWasmSession({
      wasmObj: analyzer,
      hostHooks,
      clockState: { last: 0n },
      timeoutMs,
      signals: options?.signal ? [options.signal] : [],
      onRegisterCancel: () => () => {},
      isAnalyzer: true,
    });
    return {
      imports: res.imports,
      resolvedImports: res.resolvedImports,
    };
  } finally {
    analyzer.close();
  }
}
