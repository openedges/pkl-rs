import { PklError, toPklError } from './errors.mjs';
import {
  buildRustHostConfig,
  driveWasmSession,
  formatTimeoutMessage,
  monotonicNowMs,
  normalizeTimeoutMs,
} from './host.mjs';
import {
  PKL_VERSION,
  capabilities,
  getWasmBindings,
  getWasmMemory,
  init,
} from './runtime.mjs';
import {
  DECODE_FALLBACK,
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
  hydrateSchema,
  hydrateValue,
  tryDecodePklBinaryFast,
  unpackColumnarSlice,
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
export class ModuleSource {
  /**
   * @param {string} uri
   * @param {string} [contents]
   */
  constructor(uri, contents = undefined) {
    if (typeof uri !== 'string' || uri.length === 0) {
      throw new PklError('ModuleSource uri must be a non-empty string.', 'invalid-argument');
    }
    if (contents !== undefined && typeof contents !== 'string') {
      throw new PklError('ModuleSource contents must be a string or undefined.', 'invalid-argument');
    }
    this.uri = uri;
    if (contents !== undefined) {
      this.contents = contents;
    }
    Object.freeze(this);
  }

  /**
   * Inline Pkl source text (defaults to `uri = "repl:text"`).
   * @param {string} text
   * @param {string} [uri]
   * @returns {ModuleSource}
   */
  static text(text, uri = 'repl:text') {
    if (typeof text !== 'string') {
      throw new PklError('ModuleSource.text expects a string', 'invalid-argument');
    }
    if (typeof uri !== 'string' || uri.length === 0) {
      throw new PklError('ModuleSource.text uri must be a non-empty string', 'invalid-argument');
    }
    return new ModuleSource(uri, text);
  }

  /**
   * Module backed by an arbitrary URI (`file:///...`, `https://...`, `package://...`, custom scheme).
   * @param {string} uri
   * @returns {ModuleSource}
   */
  static uri(uri) {
    if (typeof uri !== 'string' || uri.length === 0) {
      throw new PklError('ModuleSource.uri expects a non-empty URI string', 'invalid-argument');
    }
    return new ModuleSource(uri, undefined);
  }

  /**
   * Module backed by a VFS/POSIX path converted to a `file://` URI.
   * @param {string} path
   * @returns {ModuleSource}
   */
  static path(path) {
    return new ModuleSource(pathToFileUri(path), undefined);
  }

  /**
   * Module backed by a `modulepath:` URI.
   * @param {string} path
   * @returns {ModuleSource}
   */
  static modulePath(path) {
    if (typeof path !== 'string' || path.length === 0) {
      throw new PklError('ModuleSource.modulePath expects a non-empty path', 'invalid-argument');
    }
    const rest = path.startsWith('modulepath:') ? path.slice('modulepath:'.length) : path;
    const uri = rest.startsWith('/') ? `modulepath:${rest}` : `modulepath:/${rest}`;
    return new ModuleSource(uri, undefined);
  }
}

function normalizeModuleSource(source) {
  if (!source || typeof source !== 'object' || typeof source.uri !== 'string' || source.uri.length === 0) {
    throw new PklError(
      'Expected a ModuleSource created via ModuleSource.text/uri/path/modulePath',
      'invalid-argument',
    );
  }
  if (source.contents !== undefined && typeof source.contents !== 'string') {
    throw new PklError('ModuleSource.contents must be a string.', 'invalid-argument');
  }
  return {
    uri: source.uri,
    contents: source.contents === undefined ? null : source.contents,
  };
}

/**
 * Formats Pkl source text with `pkl-formatter` (grammar version 1 or 2, default 2).
 * Auto-initializes synchronously in Node.js; call `await init()` first in browsers.
 *
 * @param {string} source
 * @param {{ grammarVersion?: 1 | 2 | null }} [options]
 * @returns {string}
 */
export function format(source, options = undefined) {
  if (typeof source !== 'string') {
    throw new PklError('format source must be a string.', 'invalid-argument');
  }
  const grammarVersion = options?.grammarVersion;
  if (
    grammarVersion !== undefined &&
    grammarVersion !== null &&
    grammarVersion !== 1 &&
    grammarVersion !== 2
  ) {
    throw new PklError(
      `Invalid grammarVersion "${grammarVersion}". Expected 1 or 2.`,
      'invalid-argument',
    );
  }
  const wasm = getWasmBindings();
  const gv = grammarVersion ?? 2;
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
  if (!(bytes instanceof Uint8Array)) {
    throw new PklError('decodePklBinary expects a Uint8Array', 'invalid-argument');
  }
  const strict = Boolean(options?.strict);
  if (!strict) {
    const fast = tryDecodePklBinaryFast(bytes, false);
    if (fast !== DECODE_FALLBACK) {
      return fast;
    }
  }
  const wasm = getWasmBindings();
  try {
    const raw = wasm.wasmDecodePklBinary(bytes, strict);
    return hydrateValue(raw);
  } catch (err) {
    throw toPklError(err, 'decode');
  }
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

const WASM_FINALIZER =
  typeof FinalizationRegistry === 'function'
    ? new FinalizationRegistry((box) => {
        const wasm = box.wasm;
        box.wasm = null;
        if (!wasm) return;
        try {
          if (typeof wasm.close === 'function') wasm.close();
        } catch {
          // Best-effort GC cleanup
        }
        try {
          if (typeof wasm.free === 'function') wasm.free();
        } catch {
          // Best-effort GC cleanup
        }
      })
    : null;

function isControlFlowPklError(err) {
  return (
    err instanceof PklError &&
    (err.kind === 'cancelled' || err.kind === 'timeout' || err.kind === 'closed')
  );
}

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
    this._wasmBox = { wasm: wasmEvaluator };
    if (WASM_FINALIZER && wasmEvaluator) {
      WASM_FINALIZER.register(this, this._wasmBox, this);
    }
    this._bindings = wasmBindings;
    this._hostHooks = hostHooks;
    this._defaultTimeoutMs = defaultTimeoutMs;
    this._defaultSignal = defaultSignal;
    this._deferredCapabilityError = deferredCapabilityError;
    this._clockState = { last: 0n };
    this._closed = false;
    this._closePromise = null;
    this._resolveClose = null;
    this._cancelled = false;
    this._cancelGen = 0;
    this._inFlightCount = 0;
    this._pendingQueueCount = 0;
    this._colCache = { ab: null, base: -1, len: -1, gen: 0, hdr: null };
    this._fastUriIds = new Map();
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

  _disposeWasmIfClosed() {
    if (this._closed && this._inFlightCount === 0) {
      if (this._wasm) {
        const wasm = this._wasm;
        this._wasm = null;
        this._wasmBox.wasm = null;
        this._colCache = null;
        this._fastUriIds = null;
        if (WASM_FINALIZER) {
          WASM_FINALIZER.unregister(this);
        }
        if (typeof wasm.free === 'function') {
          wasm.free();
        }
      }
      if (this._resolveClose) {
        const resolve = this._resolveClose;
        this._resolveClose = null;
        resolve();
      }
    }
  }

  get isClosed() {
    return this._closed || !this._wasm || this._wasm.isClosed();
  }

  cancel() {
    if (this._closed || !this._wasm) return;
    this._cancelGen += 1;
    this._wasm.cancel();
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('cancelled');
    }
  }

  close() {
    if (this._closed) return this._closePromise || Promise.resolve();
    this._closed = true;
    this._cancelGen += 1;
    if (this._defaultSignal) {
      this._defaultSignal.removeEventListener('abort', this._onDefaultAbort);
    }
    if (this._wasm) {
      this._wasm.close();
    }
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('closed');
    }
    if (this._inFlightCount > 0) {
      this._closePromise = new Promise((resolve) => {
        this._resolveClose = resolve;
      });
      this._disposeWasmIfClosed();
      return this._closePromise;
    }
    this._disposeWasmIfClosed();
    this._closePromise = Promise.resolve();
    return this._closePromise;
  }

  async _executeOp(callOptions, startFn) {
    if (this._closed || !this._wasm || this._wasm.isClosed()) {
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
    this._inFlightCount += 1;
    try {
      try {
        startFn(now, timeoutMs);
      } catch (err) {
        throw toPklError(err);
      }
      const signals = [];
      if (this._defaultSignal) signals.push(this._defaultSignal);
      if (callOptions?.signal) signals.push(callOptions.signal);
      return await driveWasmSession({
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
    } finally {
      this._inFlightCount -= 1;
      this._disposeWasmIfClosed();
    }
  }

  _enqueueTask(task) {
    const enqueueGen = this._cancelGen;
    const guarded = () => {
      if (this._closed || !this._wasm || this._wasm.isClosed()) {
        throw new PklError('Evaluator has been closed.', 'closed');
      }
      if (this._cancelGen !== enqueueGen) {
        throw new PklError('Evaluation was cancelled.', 'cancelled');
      }
      return task();
    };
    this._pendingQueueCount += 1;
    const next = this._queue.then(guarded, guarded).finally(() => {
      this._pendingQueueCount -= 1;
    });
    this._queue = next.then(
      () => undefined,
      () => undefined,
    );
    return next;
  }

  /**
   * Serializes operations on one `WasmEvaluator` instance while respecting `close()`,
   * pre-aborted `signal`, `cancel()`, and per-call `timeoutMs` / `signal`.
   */
  _runSerialized(callOptions, startFn) {
    return this._enqueueTask(() => this._executeOp(callOptions, startFn));
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
    return this.evaluateExpressionValue(source, 'output.value', callOptions);
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
    const spec = normalizeModuleSource(source);
    if (typeof expr !== 'string') {
      throw new PklError('expression must be a string', 'invalid-argument');
    }
    if (
      !this._closed &&
      this._wasm &&
      this._inFlightCount === 0 &&
      this._pendingQueueCount === 0 &&
      !this._deferredCapabilityError &&
      !this._cancelled &&
      !this._defaultSignal?.aborted &&
      !callOptions?.signal &&
      callOptions?.timeoutMs === undefined &&
      this._defaultTimeoutMs === null &&
      (expr === 'module' || expr === 'output.value')
    ) {
      const hasContents = spec.contents !== null;
      const exprTag = expr === 'module' ? 0 : 1;
      let status = 0;
      if (this._fastUriIds !== null) {
        let uriId = this._fastUriIds.get(spec.uri);
        if (uriId === undefined && this._fastUriIds.size < 64) {
          const registered = this._wasm.registerFastUri(spec.uri);
          if (registered !== 0xffffffff) {
            uriId = registered;
            this._fastUriIds.set(spec.uri, uriId);
          }
        }
        if (uriId !== undefined) {
          status = this._wasm.tryEvaluateExpressionColumnarFastById(uriId, hasContents, exprTag);
        } else {
          status = this._wasm.tryEvaluateExpressionColumnarFast(spec.uri, hasContents, expr);
        }
      } else {
        status = this._wasm.tryEvaluateExpressionColumnarFast(spec.uri, hasContents, expr);
      }
      if (status === 1) {
        const mem = getWasmMemory();
        if (mem) {
          const ptr = this._wasm.fastColumnarPtr();
          const len = this._wasm.fastColumnarLen();
          const unpacked = unpackColumnarSlice(mem.buffer, ptr, len, this._colCache);
          if (len > 262144) {
            this._colCache = { ab: null, base: -1, len: -1, gen: 0, hdr: null };
            this._wasm.releaseOversizedColumnar();
          }
          if (unpacked !== DECODE_FALLBACK) {
            return unpacked;
          }
        }
      }
    }
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

  async _evalExpressionDirect(source, expr, callOptions) {
    const spec = normalizeModuleSource(source);
    const res = await this._executeOp(callOptions, (now, timeoutMs) => {
      this._wasm.startExpression(spec.uri, spec.contents, expr, now, timeoutMs);
    });
    return res.value;
  }

  async _evalExpressionValueDirect(source, expr, callOptions) {
    const spec = normalizeModuleSource(source);
    const res = await this._executeOp(callOptions, (now, timeoutMs) => {
      this._wasm.startExpressionPklBinary(spec.uri, spec.contents, expr, now, timeoutMs);
    });
    return decodePklBinary(res.value);
  }

  /**
   * Evaluates a `pkl:test` module (`facts` and `examples`), returning structured `TestResults`
   * plus Pkl 0.32.0 `rendered` output (`spec` | `minimal` | `junit`).
   *
   * @param {any} source
   * @param {{ overwrite?: boolean, reporter?: "spec" | "minimal" | "junit", timeoutMs?: number | bigint, signal?: AbortSignal }} [options]
   * @returns {Promise<any>}
   */
  evaluateTest(source, options = undefined) {
    const spec = normalizeModuleSource(source);
    const reporter = options?.reporter ?? 'spec';
    const overwrite = Boolean(options?.overwrite);
    const requestedTimeoutMs =
      options?.timeoutMs !== undefined
        ? normalizeTimeoutMs(options.timeoutMs)
        : this._defaultTimeoutMs;
    const callSignal = options?.signal;
    const useColor = this._hostHooks.useColor;

    return this._enqueueTask(async () => {
      if (this.isClosed) {
        throw new PklError('Evaluator has been closed.', 'closed');
      }
      if (this._deferredCapabilityError) {
        throw this._deferredCapabilityError;
      }
      if (
        this._cancelled ||
        this._defaultSignal?.aborted ||
        callSignal?.aborted
      ) {
        throw new PklError('Evaluation was cancelled.', 'cancelled');
      }

      const startCancelGen = this._cancelGen;
      const startNow = monotonicNowMs(this._clockState);
      const sharedDeadline =
        requestedTimeoutMs !== null ? startNow + requestedTimeoutMs : null;
      const budget = {
        startCancelGen,
        requestedTimeoutMs,
        sharedDeadline,
        signal: callSignal,
        nextCallOptions: () => {
          if (this.isClosed) {
            throw new PklError('Evaluator has been closed.', 'closed');
          }
          if (
            this._cancelled ||
            this._cancelGen !== startCancelGen ||
            this._defaultSignal?.aborted ||
            callSignal?.aborted
          ) {
            throw new PklError('Evaluation was cancelled.', 'cancelled');
          }
          if (sharedDeadline === null) {
            return { timeoutMs: undefined, signal: callSignal };
          }
          const now = monotonicNowMs(this._clockState);
          if (now >= sharedDeadline) {
            throw new PklError(formatTimeoutMessage(requestedTimeoutMs ?? 0n), 'timeout');
          }
          return { timeoutMs: sharedDeadline - now, signal: callSignal };
        },
      };

      this._testLogBuffer = [];
      this._inFlightCount += 1;
      this._wasm.setCaptureLogs(true);
      try {
        const checkVal = await this._evalExpressionValueDirect(
          source,
          TEST_CHECK_EXPR,
          budget.nextCallOptions(),
        );
        const moduleName = (checkVal instanceof Map && checkVal.get('moduleName')) || 'test';
        const moduleUri = (checkVal instanceof Map && checkVal.get('moduleUri')) || spec.uri;
        const isTest = Boolean(checkVal instanceof Map && checkVal.get('isTest'));
        const displayUri = moduleUri;

        if (!isTest) {
          let renderedErr;
          try {
            await this._evalExpressionDirect(
              source,
              'module as import("pkl:test")',
              budget.nextCallOptions(),
            );
            renderedErr = `–– Pkl Error ––\nExpected value of type \`pkl.test\`, but got type \`Class\`.\nValue: ${moduleName}\n`;
          } catch (err) {
            if (isControlFlowPklError(err)) throw err;
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
          const rendered = (() => {
            try {
              return this._bindings.wasmRenderTestReport(rawResults, reporter, useColor);
            } catch (err) {
              throw toPklError(err);
            }
          })();
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
          budget,
        );

        // 2. Evaluate `examples`
        const examplesSection = await this._evaluateTestExamples(
          source,
          moduleUri,
          moduleSourceText,
          overwrite,
          useColor,
          budget,
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
        const rendered = (() => {
          try {
            return this._bindings.wasmRenderTestReport(rawResults, reporter, useColor);
          } catch (err) {
            throw toPklError(err);
          }
        })();
        return finalizeTestResults(rawResults, rendered);
      } catch (err) {
        if (err instanceof PklError && err.kind === 'timeout' && requestedTimeoutMs !== null) {
          throw new PklError(formatTimeoutMessage(requestedTimeoutMs), 'timeout');
        }
        throw err;
      } finally {
        if (this._wasm) {
          this._wasm.setCaptureLogs(false);
        }
        this._testLogBuffer = null;
        this._inFlightCount -= 1;
        this._disposeWasmIfClosed();
      }
    });
  }

  async _awaitWithBudget(promise, budget) {
    let timerId = null;
    let settled = false;
    const listeners = [];
    let unregisterCancel = () => {};
    const abortPromise = new Promise((_, reject) => {
      const fail = (kind) => {
        if (settled) return;
        settled = true;
        if (kind === 'timeout') {
          reject(new PklError(formatTimeoutMessage(budget.requestedTimeoutMs ?? 0n), 'timeout'));
        } else if (kind === 'closed') {
          reject(new PklError('Evaluator has been closed.', 'closed'));
        } else {
          reject(new PklError('Evaluation was cancelled.', 'cancelled'));
        }
      };
      if (this.isClosed) {
        fail('closed');
        return;
      }
      if (
        this._cancelled ||
        (budget.startCancelGen !== undefined && this._cancelGen !== budget.startCancelGen) ||
        this._defaultSignal?.aborted ||
        budget.signal?.aborted
      ) {
        fail('cancelled');
        return;
      }
      const hook = (kind) => fail(kind);
      this._activeCancelHooks.add(hook);
      unregisterCancel = () => this._activeCancelHooks.delete(hook);
      for (const sig of [this._defaultSignal, budget.signal]) {
        if (!sig) continue;
        const handler = () => fail('cancelled');
        sig.addEventListener('abort', handler, { once: true });
        listeners.push({ sig, handler });
      }
      const armTimer = () => {
        if (settled || budget.sharedDeadline === null) return;
        const now = monotonicNowMs(this._clockState);
        if (now >= budget.sharedDeadline) {
          fail('timeout');
          return;
        }
        const rem = budget.sharedDeadline - now;
        const delayMs = rem > 2_147_483_647n ? 2_147_483_647 : Number(rem);
        timerId = setTimeout(() => {
          timerId = null;
          armTimer();
        }, delayMs);
      };
      armTimer();
    });
    abortPromise.catch(() => {});
    try {
      return await Promise.race([promise, abortPromise]);
    } finally {
      settled = true;
      if (timerId !== null) clearTimeout(timerId);
      unregisterCancel();
      for (const { sig, handler } of listeners) {
        sig.removeEventListener('abort', handler);
      }
    }
  }

  async _evaluateTestFacts(source, spec, moduleUri, moduleSourceText, useColor, budget) {
    const shape = await this._evalExpressionValueDirect(
      source,
      'if (facts == null) null else facts.toMap().mapValues((_, g) -> g.length)',
      budget.nextCallOptions(),
    );
    if (!(shape instanceof Map)) {
      return { name: 'facts', results: [] };
    }
    const factSpans =
      typeof moduleSourceText === 'string'
        ? (() => {
            try {
              return this._bindings.wasmExtractFactSpans(moduleSourceText);
            } catch (err) {
              throw toPklError(err);
            }
          })()
        : {};
    const results = [];

    for (const [groupKey, lenVal] of shape.entries()) {
      const groupName = String(groupKey);
      const count = typeof lenVal === 'bigint' ? Number(lenVal) : Number(lenVal || 0);
      const groupLocs = Object.prototype.hasOwnProperty.call(factSpans, groupName)
        ? factSpans[groupName]
        : [];
      let totalAsserts = 0;
      const failures = [];
      const errors = [];

      for (let idx = 0; idx < count; idx += 1) {
        totalAsserts += 1;
        const expr = `facts[${pklEscapeStr(groupName)}][${idx}]`;
        try {
          const val = await this._evalExpressionValueDirect(
            source,
            expr,
            budget.nextCallOptions(),
          );
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
                  const probeRes = await this._executeOp(
                    budget.nextCallOptions(),
                    (now, timeoutMs) => {
                      this._wasm.startPowerAssertionProbe(
                        spec.uri,
                        spec.contents,
                        probeExpr,
                        location,
                        useColor,
                        now,
                        timeoutMs,
                      );
                    },
                  );
                  paFailure = probeRes.value;
                } catch (probeErr) {
                  if (isControlFlowPklError(probeErr)) throw probeErr;
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
          if (isControlFlowPklError(err)) throw err;
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
    budget,
  ) {
    const isNull = await this._evalExpressionValueDirect(
      source,
      'examples == null',
      budget.nextCallOptions(),
    );
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

    try {
      if (typeof this._hostHooks.securityManager?.checkResolveModule === 'function') {
        await this._awaitWithBudget(
          Promise.resolve().then(() =>
            this._hostHooks.securityManager.checkResolveModule(moduleUri),
          ),
          budget,
        );
        await this._awaitWithBudget(
          Promise.resolve().then(() =>
            this._hostHooks.securityManager.checkResolveModule(expectedUri),
          ),
          budget,
        );
        await this._awaitWithBudget(
          Promise.resolve().then(() =>
            this._hostHooks.securityManager.checkResolveModule(actualUri),
          ),
          budget,
        );
      } else {
        this._wasm.checkResolveModule(moduleUri);
        this._wasm.checkResolveModule(expectedUri);
        this._wasm.checkResolveModule(actualUri);
      }
    } catch (err) {
      if (isControlFlowPklError(err)) throw err;
      throw toPklError(err, 'security');
    }

    budget.nextCallOptions();
    if (vfs.isFile(actualPath)) vfs.delete(actualPath);
    if (overwrite && vfs.isFile(expectedPath)) vfs.delete(expectedPath);

    const actualMap = await this._evalExpressionValueDirect(
      source,
      RENDER_ACTUAL_EXAMPLES_EXPR,
      budget.nextCallOptions(),
    );
    const actualGroups = mapToRenderedGroups(actualMap);
    const fullDoc = await this._evalExpressionDirect(
      source,
      RENDER_EXAMPLES_DOC_EXPR,
      budget.nextCallOptions(),
    );

    let expectedGroups = null;
    let expectedSource = null;
    if (vfs.isFile(expectedPath)) {
      expectedSource = vfs.readText(expectedPath);
      const expMap = await this._evalExpressionValueDirect(
        ModuleSource.text(expectedSource, expectedUri),
        RENDER_EXPECTED_EXAMPLES_EXPR,
        budget.nextCallOptions(),
      );
      expectedGroups = mapToRenderedGroups(expMap);
    }

    let evalOut;
    try {
      evalOut = this._bindings.wasmEvaluateExamples(
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
    } catch (err) {
      throw toPklError(err);
    }

    budget.nextCallOptions();
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
  const enrichSection = (sec) => {
    const rawResults = sec.results || [];
    let totalTests = rawResults.length;
    let totalFailures = 0;
    let totalAsserts = 0;
    let totalAssertsFailed = 0;
    const results = rawResults.map((r) => {
      const failures = r.failures || [];
      const errors = r.errors || [];
      const f = failures.length + errors.length;
      const asserts = r.totalAsserts || 0;
      totalAsserts += asserts;
      totalAssertsFailed += f;
      if (f > 0) totalFailures += 1;
      return {
        name: r.name,
        totalAsserts: asserts,
        failed: f > 0,
        isExampleWritten: Boolean(r.isExampleWritten),
        failures,
        errors,
      };
    });
    return {
      section: {
        name: sec.name,
        failed: totalFailures > 0,
        totalTests,
        totalFailures,
        totalAsserts,
        results,
      },
      totalTests,
      totalFailures,
      totalAsserts,
      totalAssertsFailed,
    };
  };
  const fStats = enrichSection(raw.facts);
  const eStats = enrichSection(raw.examples);
  const failed = Boolean(raw.error) || fStats.totalFailures > 0 || eStats.totalFailures > 0;
  return {
    moduleName: raw.moduleName,
    displayUri: raw.displayUri,
    failed,
    totalTests: fStats.totalTests + eStats.totalTests,
    totalFailures: fStats.totalFailures + eStats.totalFailures,
    totalAsserts: fStats.totalAsserts + eStats.totalAsserts,
    totalAssertsFailed: fStats.totalAssertsFailed + eStats.totalAssertsFailed,
    facts: fStats.section,
    examples: eStats.section,
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
    this._wasmBox = { wasm: wasmRepl };
    if (WASM_FINALIZER && wasmRepl) {
      WASM_FINALIZER.register(this, this._wasmBox, this);
    }
    this._hostHooks = hostHooks;
    this._defaultTimeoutMs = defaultTimeoutMs;
    this._defaultSignal = defaultSignal;
    this._deferredCapabilityError = deferredCapabilityError;
    this._clockState = { last: 0n };
    this._nextId = 1;
    this._closed = false;
    this._closePromise = null;
    this._resolveClose = null;
    this._cancelled = false;
    this._cancelGen = 0;
    this._inFlightCount = 0;
    this._pendingQueueCount = 0;
    this._activeCancelHooks = new Set();
    this._queue = Promise.resolve();

    this._onDefaultAbort = () => {
      if (this._closed || !this._wasm) return;
      this._cancelled = true;
      this._cancelGen += 1;
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

  _disposeWasmIfClosed() {
    if (this._closed && this._inFlightCount === 0) {
      if (this._wasm) {
        const wasm = this._wasm;
        this._wasm = null;
        this._wasmBox.wasm = null;
        if (WASM_FINALIZER) {
          WASM_FINALIZER.unregister(this);
        }
        if (typeof wasm.free === 'function') {
          wasm.free();
        }
      }
      if (this._resolveClose) {
        const resolve = this._resolveClose;
        this._resolveClose = null;
        resolve();
      }
    }
  }

  get isClosed() {
    return this._closed || !this._wasm || this._wasm.isClosed();
  }

  cancel() {
    if (this._closed || !this._wasm) return;
    this._cancelGen += 1;
    this._wasm.cancel();
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('cancelled');
    }
  }

  close() {
    if (this._closed) return this._closePromise || Promise.resolve();
    this._closed = true;
    this._cancelGen += 1;
    if (this._defaultSignal) {
      this._defaultSignal.removeEventListener('abort', this._onDefaultAbort);
    }
    if (this._wasm) {
      this._wasm.close();
    }
    for (const hook of Array.from(this._activeCancelHooks)) {
      hook('closed');
    }
    if (this._inFlightCount > 0) {
      this._closePromise = new Promise((resolve) => {
        this._resolveClose = resolve;
      });
      this._disposeWasmIfClosed();
      return this._closePromise;
    }
    this._disposeWasmIfClosed();
    this._closePromise = Promise.resolve();
    return this._closePromise;
  }

  _runRequest(explicitId, startFn) {
    const enqueueGen = this._cancelGen;
    const task = async () => {
      if (this.isClosed) {
        throw new PklError('REPL session has been closed.', 'closed');
      }
      if (this._deferredCapabilityError) {
        throw this._deferredCapabilityError;
      }
      if (this._cancelled || this._defaultSignal?.aborted || this._cancelGen !== enqueueGen) {
        throw new PklError('REPL evaluation was cancelled.', 'cancelled');
      }
      const seq = this._nextId++;
      const id =
        typeof explicitId === 'string' && explicitId.length > 0 ? explicitId : `req-${seq}`;
      const now = monotonicNowMs(this._clockState);
      this._inFlightCount += 1;
      try {
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
      } finally {
        this._inFlightCount -= 1;
        this._disposeWasmIfClosed();
      }
    };
    this._pendingQueueCount += 1;
    const next = this._queue.then(task, task).finally(() => {
      this._pendingQueueCount -= 1;
    });
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
    let inlineContents = null;
    if (typeof uriOrSource === 'string') {
      if (uriOrSource.length === 0) {
        throw new PklError(
          'ReplSession.load expects a non-empty URI string or ModuleSource',
          'invalid-argument',
        );
      }
      uri = uriOrSource;
    } else if (
      uriOrSource &&
      typeof uriOrSource === 'object' &&
      typeof uriOrSource.uri === 'string' &&
      uriOrSource.uri.length > 0
    ) {
      uri = uriOrSource.uri;
      if (uriOrSource.contents !== undefined && uriOrSource.contents !== null) {
        if (typeof uriOrSource.contents !== 'string') {
          throw new PklError('ModuleSource.contents must be a string.', 'invalid-argument');
        }
        inlineContents = uriOrSource.contents;
      }
    } else {
      throw new PklError(
        'ReplSession.load expects a non-empty URI string or ModuleSource',
        'invalid-argument',
      );
    }
    return this._runRequest(options?.id, (id, now, timeoutMs) => {
      this._wasm.startLoad(id, uri, inlineContents, now, timeoutMs);
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
    throw new PklError(
      'analyzeImportGraph expects an array of URI strings or ModuleSource instances',
      'invalid-argument',
    );
  }
  const normalizedUris = uris.map((item) => {
    if (typeof item === 'string') return item;
    if (item && typeof item === 'object' && typeof item.uri === 'string') return item.uri;
    throw new PklError(
      'analyzeImportGraph entries must be URI strings or ModuleSource instances',
      'invalid-argument',
    );
  });
  const wasm = getWasmBindings();
  const timeoutMs = normalizeTimeoutMs(options?.timeoutMs);
  const { rustConfig, hostHooks, deferredCapabilityError } = buildRustHostConfig(options);
  if (deferredCapabilityError) {
    throw deferredCapabilityError;
  }
  let analyzer;
  try {
    analyzer = new wasm.WasmImportAnalyzer(normalizedUris, rustConfig);
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
    if (typeof analyzer.free === 'function') {
      analyzer.free();
    }
  }
}
