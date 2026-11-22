import { PklError, toPklError } from './errors.mjs';
import { fileUriToPath, pathToFileUri } from './vfs.mjs';

const U64_MAX = 18446744073709551615n;
const MAX_TIMER_DELAY_MS = 2_147_483_647n;
const DEFAULT_POLL_BUDGET = 65536;
const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: true });

/**
 * Strictly validates a 64-bit unsigned millisecond clock or timeout value.
 * Accepts JS safe integers (`0..=2^53-1`) and `BigInt` (`0n..=2^64-1`).
 * @param {number | bigint | null | undefined} value
 * @param {string} [label]
 * @returns {bigint | null}
 */
export function normalizeTimeoutMs(value, label = 'timeoutMs') {
  if (value === undefined || value === null) return null;
  if (typeof value === 'number') {
    if (!Number.isFinite(value) || !Number.isInteger(value) || value < 0) {
      throw new PklError(`${label} must be a non-negative integer`, 'invalid-argument');
    }
    if (value > Number.MAX_SAFE_INTEGER) {
      throw new PklError(
        `${label} exceeds Number.MAX_SAFE_INTEGER; pass a BigInt for values above 2^53 - 1`,
        'invalid-argument',
      );
    }
    return BigInt(value);
  }
  if (typeof value === 'bigint') {
    if (value < 0n || value > U64_MAX) {
      throw new PklError(
        `${label} BigInt must be within 0n..=${U64_MAX}n`,
        'invalid-argument',
      );
    }
    return value;
  }
  throw new PklError(`${label} must be a number or BigInt`, 'invalid-argument');
}

/**
 * Returns a strictly monotonic 64-bit millisecond timestamp (`BigInt`).
 * @param {{ last: bigint }} state
 * @returns {bigint}
 */
export function monotonicNowMs(state) {
  const rawMs =
    typeof performance !== 'undefined' && typeof performance.now === 'function'
      ? Math.floor(performance.now())
      : Date.now();
  const current = BigInt(Math.max(0, rawMs));
  if (current < state.last) {
    return state.last;
  }
  state.last = current;
  return current;
}

/**
 * Converts a `Record<string, string>` or `Map<string, string>` into `[string, string][]`.
 * @param {Record<string, string> | Map<string, string> | undefined} input
 * @param {string} label
 * @returns {[string, string][]}
 */
export function normalizeStringPairs(input, label) {
  if (input === undefined || input === null) return [];
  if (input instanceof Map) {
    const out = [];
    for (const [k, v] of input.entries()) {
      if (typeof k !== 'string' || typeof v !== 'string') {
        throw new PklError(`${label} entries must have string keys and values`, 'configuration');
      }
      out.push([k, v]);
    }
    return out;
  }
  if (typeof input === 'object' && !Array.isArray(input)) {
    const out = [];
    for (const [k, v] of Object.entries(input)) {
      if (typeof v !== 'string') {
        throw new PklError(`${label}.${k} must be a string`, 'configuration');
      }
      out.push([k, v]);
    }
    return out;
  }
  throw new PklError(`${label} must be a Record<string, string> or Map<string, string>`, 'configuration');
}

/**
 * Resolves `color` option (`boolean | "never" | "auto" | "always"`) to a concrete boolean.
 * @param {boolean | "never" | "auto" | "always" | undefined} color
 * @returns {boolean}
 */
export function resolveColorOption(color) {
  if (color === undefined || color === false || color === 'never') return false;
  if (color === true || color === 'always') return true;
  if (color === 'auto') {
    return Boolean(
      typeof process !== 'undefined' &&
        process !== null &&
        process.stderr &&
        process.stderr.isTTY,
    );
  }
  throw new PklError(
    `Invalid color option \`${String(color)}\` (expected boolean, "never", "auto", or "always")`,
    'configuration',
  );
}

/**
 * Checks for Wasm-unavailable explicit options and returns a structured
 * `capability-unavailable` PklError (deferred to evaluation time so callers that construct
 * an evaluator before calling `evaluate*` always observe explicit capability rejection).
 * @param {any} options
 * @returns {PklError | null}
 */
export function getDeferredWasmCapabilityError(options) {
  if (!options || typeof options !== 'object') return null;
  if (
    (options.externalModuleReaders && Object.keys(options.externalModuleReaders).length > 0) ||
    (options.externalResourceReaders && Object.keys(options.externalResourceReaders).length > 0)
  ) {
    return new PklError(
      'External reader subprocesses (`externalModuleReaders` / `externalResourceReaders`) are unavailable in Wasm.',
      'capability-unavailable',
      'externalReaders',
    );
  }
  if (Array.isArray(options.modulePaths) && options.modulePaths.length > 0) {
    return new PklError(
      '`modulePaths` (filesystem/JAR modulepath archives) are unavailable in Wasm.',
      'capability-unavailable',
      'modulePathJar',
    );
  }
  if (options.projectDir || options.project) {
    return new PklError(
      '`projectDir` / `project` resolution is unavailable in Wasm.',
      'capability-unavailable',
      'projectResolution',
    );
  }
  if (typeof options.cacheDir === 'string' && options.cacheDir.length > 0) {
    return new PklError(
      'Persistent disk `cacheDir` is unavailable in Wasm.',
      'capability-unavailable',
      'packageCache',
    );
  }
  return null;
}

/**
 * Builds the Rust-side `WasmHostConfig` object from user `EvaluatorOptions`.
 * @param {any} options
 */
export function buildRustHostConfig(options = {}) {
  const deferredCapabilityError = getDeferredWasmCapabilityError(options);
  const vfs = options.vfs ?? null;
  const effectiveRootDir =
    typeof options.rootDir === 'string' && options.rootDir.length > 0
      ? options.rootDir
      : (vfs?.rootDir ?? null);

  const moduleReaders = Array.isArray(options.moduleReaders) ? options.moduleReaders : [];
  const resourceReaders = Array.isArray(options.resourceReaders) ? options.resourceReaders : [];
  const securityManager = options.securityManager ?? null;
  const hasCustomSecurityCheckImport = Boolean(
    securityManager && typeof securityManager.checkImportModule === 'function',
  );
  const hasCustomSecurityCheckResolve = Boolean(
    securityManager &&
      (typeof securityManager.checkResolveModule === 'function' ||
        typeof securityManager.checkImportModule === 'function'),
  );

  return {
    deferredCapabilityError,
    rustConfig: {
      allowedModules: Array.isArray(options.allowedModules) ? [...options.allowedModules] : null,
      allowedResources: Array.isArray(options.allowedResources)
        ? [...options.allowedResources]
        : null,
      rootDir: effectiveRootDir,
      properties: normalizeStringPairs(options.properties, 'properties'),
      env: normalizeStringPairs(options.env, 'env'),
      outputFormat: typeof options.outputFormat === 'string' ? options.outputFormat : null,
      traceMode: options.traceMode ?? 'compact',
      powerAssertions: Boolean(options.powerAssertions),
      color: resolveColorOption(options.color),
      testMode: Boolean(options.testMode),
      hasVfs: Boolean(vfs),
      hasHttpFetch: typeof options.httpFetch === 'function',
      hasPackageTransport: typeof options.packageTransport === 'function',
      hasCustomLogger: Boolean(
        options.logger &&
          (typeof options.logger.trace === 'function' || typeof options.logger.warn === 'function'),
      ),
      hasCustomFrameTransformer: typeof options.frameTransformer === 'function',
      hasCustomSecurityCheckImport,
      hasCustomSecurityCheckRead: Boolean(
        securityManager && typeof securityManager.checkReadResource === 'function',
      ),
      hasCustomSecurityCheckResolve,
      platform: options.platform ?? null,
      moduleReaders: moduleReaders.map((r) => ({
        scheme: String(r.scheme),
        isLocal: Boolean(r.isLocal),
        isGlobbable: Boolean(r.isGlobbable),
        hasHierarchicalUris: Boolean(r.hasHierarchicalUris),
        isCached: r.isCached === undefined ? true : Boolean(r.isCached),
        hasFragmentPaths: Boolean(r.hasFragmentPaths),
      })),
      resourceReaders: resourceReaders.map((r) => ({
        scheme: String(r.scheme),
        isGlobbable: Boolean(r.isGlobbable),
        hasHierarchicalUris: Boolean(r.hasHierarchicalUris),
      })),
    },
    hostHooks: {
      vfs,
      httpFetch: typeof options.httpFetch === 'function' ? options.httpFetch : null,
      packageTransport:
        typeof options.packageTransport === 'function' ? options.packageTransport : null,
      logger: options.logger ?? null,
      frameTransformer:
        typeof options.frameTransformer === 'function' ? options.frameTransformer : null,
      securityManager,
      moduleReaders,
      resourceReaders,
      useColor: resolveColorOption(options.color),
      powerAssertions: Boolean(options.powerAssertions),
    },
  };
}

/**
 * Formats a millisecond timeout matching Pkl 0.32.0's `format_timeout_message`.
 * @param {bigint} timeoutMs
 * @returns {string}
 */
function formatTimeoutMessage(timeoutMs) {
  const quotient = timeoutMs / 10n;
  const remainder = timeoutMs % 10n;
  const hundredths =
    remainder > 5n || (remainder === 5n && quotient % 2n === 1n) ? quotient + 1n : quotient;
  const whole = hundredths / 100n;
  const frac = hundredths % 100n;
  let secs;
  if (frac === 0n) {
    secs = whole.toString();
  } else if (frac % 10n === 0n) {
    secs = `${whole}.${(frac / 10n).toString()}`;
  } else {
    secs = `${whole}.${frac.toString().padStart(2, '0')}`;
  }
  return `Evaluation timed out after ${secs} second(s).`;
}

/**
 * Cooperative macrotask yield so timers, `AbortSignal`, and browser painting stay responsive.
 */
function yieldEventLoop() {
  return new Promise((resolve) => {
    setTimeout(resolve, 0);
  });
}

/**
 * Drives any cooperative Wasm session (`WasmEvaluator`, `WasmReplSession`, or `WasmImportAnalyzer`)
 * through `poll()` and async Host effect dispatch with full 64-bit timeout, `AbortSignal`,
 * `cancel()`, `close()`, and zero-leak cleanup.
 *
 * @param {{
 *   wasmObj: any,
 *   hostHooks: any,
 *   clockState: { last: bigint },
 *   timeoutMs: bigint | null,
 *   signals: AbortSignal[],
 *   onRegisterCancel: (fn: (reasonKind: "cancelled" | "closed") => void) => () => void,
 *   isAnalyzer?: boolean,
 *   onCapturedLog?: (message: string) => void,
 * }} params
 * @returns {Promise<any>}
 */
export async function driveWasmSession({
  wasmObj,
  hostHooks,
  clockState,
  timeoutMs,
  signals,
  onRegisterCancel,
  isAnalyzer = false,
  onCapturedLog = undefined,
}) {
  /** @type {Map<string, AbortController>} */
  const inFlight = new Map();
  let abortedKind = /** @type {"cancelled" | "closed" | "timeout" | null} */ (null);
  let timerId = /** @type {any} */ (null);
  const startNow = monotonicNowMs(clockState);
  const deadline = timeoutMs !== null ? startNow + timeoutMs : null;

  const abortAllInFlight = () => {
    if (typeof wasmObj.drainCancelledOperations === 'function') {
      const drained = wasmObj.drainCancelledOperations();
      for (const tok of drained) {
        const ctrl = inFlight.get(tok);
        if (ctrl) {
          ctrl.abort();
          inFlight.delete(tok);
        }
      }
    }
    for (const ctrl of inFlight.values()) {
      ctrl.abort();
    }
    inFlight.clear();
  };

  const triggerAbort = (kind) => {
    if (abortedKind !== null) return;
    abortedKind = kind;
    if (timerId !== null) {
      clearTimeout(timerId);
      timerId = null;
    }
    if (kind === 'closed') {
      if (typeof wasmObj.close === 'function') wasmObj.close();
    } else if (typeof wasmObj.cancel === 'function') {
      wasmObj.cancel();
    }
    abortAllInFlight();
  };

  const armTimer = () => {
    if (deadline === null || abortedKind !== null) return;
    const now = monotonicNowMs(clockState);
    if (now >= deadline) {
      triggerAbort('timeout');
      return;
    }
    const remaining = deadline - now;
    const delay = remaining > MAX_TIMER_DELAY_MS ? Number(MAX_TIMER_DELAY_MS) : Number(remaining);
    timerId = setTimeout(() => {
      timerId = null;
      armTimer();
    }, delay);
  };

  /** @type {Array<{ signal: AbortSignal, handler: () => void }>} */
  const signalListeners = [];
  for (const sig of signals) {
    if (!sig) continue;
    if (sig.aborted) {
      triggerAbort('cancelled');
      break;
    }
    const handler = () => triggerAbort('cancelled');
    sig.addEventListener('abort', handler, { once: true });
    signalListeners.push({ signal: sig, handler });
  }

  const unregisterCancel = onRegisterCancel((kind) => triggerAbort(kind));

  const throwIfAborted = () => {
    if (abortedKind === 'timeout') {
      throw new PklError(formatTimeoutMessage(timeoutMs ?? 0n), 'timeout');
    }
    if (abortedKind === 'closed') {
      throw new PklError('Evaluator has been closed.', 'closed');
    }
    if (abortedKind === 'cancelled') {
      throw new PklError('Evaluation was cancelled.', 'cancelled');
    }
  };

  try {
    throwIfAborted();
    armTimer();

    let yieldCounter = 0;
    while (true) {
      throwIfAborted();
      const now = monotonicNowMs(clockState);
      if (deadline !== null && now >= deadline) {
        triggerAbort('timeout');
        throwIfAborted();
      }

      const event = isAnalyzer ? wasmObj.poll() : wasmObj.poll(now, DEFAULT_POLL_BUDGET);

      if (event.kind === 'yielded') {
        yieldCounter += 1;
        if (yieldCounter % 4 === 0) {
          await yieldEventLoop();
        } else {
          await Promise.resolve();
        }
        continue;
      }

      if (event.kind === 'ready') {
        return event;
      }

      if (event.kind === 'error') {
        throw new PklError(
          event.message,
          event.errorKind || 'evaluation',
          event.capability,
        );
      }

      await dispatchSuspendEvent({
        event,
        wasmObj,
        hostHooks,
        clockState,
        inFlight,
        isAnalyzer,
        onCapturedLog,
        isAborted: () => abortedKind !== null,
      });
    }
  } catch (err) {
    if (abortedKind !== null) {
      throwIfAborted();
    }
    throw toPklError(err);
  } finally {
    if (timerId !== null) {
      clearTimeout(timerId);
      timerId = null;
    }
    unregisterCancel();
    for (const { signal, handler } of signalListeners) {
      signal.removeEventListener('abort', handler);
    }
    abortAllInFlight();
  }
}

/**
 * Dispatches one typed `WasmSuspendRequest` event to VFS / `httpFetch` / `packageTransport` /
 * custom readers / `securityManager` / `logger` / `frameTransformer` and resumes Wasm.
 */
async function dispatchSuspendEvent({
  event,
  wasmObj,
  hostHooks,
  clockState,
  inFlight,
  isAnalyzer,
  onCapturedLog,
  isAborted,
}) {
  const token = event.token;
  const controller = new AbortController();
  inFlight.set(token, controller);

  const now = () => monotonicNowMs(clockState);
  const passNow = (fn, ...args) => (isAnalyzer ? fn(...args) : fn(...args, now()));

  try {
    switch (event.kind) {
      case 'readModule': {
        await handleReadModule({ event, wasmObj, hostHooks, controller, isAborted, passNow });
        return;
      }
      case 'moduleHasElement': {
        await handleModuleHasElement({ event, wasmObj, hostHooks, controller, isAborted, passNow });
        return;
      }
      case 'readResource': {
        await handleReadResource({ event, wasmObj, hostHooks, controller, isAborted, passNow });
        return;
      }
      case 'resourceHasElement': {
        await handleResourceHasElement({ event, wasmObj, hostHooks, controller, isAborted, passNow });
        return;
      }
      case 'listModules':
      case 'listResources': {
        await handleListElements({ event, wasmObj, hostHooks, controller, isAborted, passNow });
        return;
      }
      case 'packageMetadata': {
        await handlePackageFetch({
          event,
          url: event.requestUrl,
          completeFn: wasmObj.completePackageMetadata.bind(wasmObj),
          rejectFn: wasmObj.rejectPackageMetadata.bind(wasmObj),
          hostHooks,
          controller,
          isAborted,
          passNow,
        });
        return;
      }
      case 'packageArchive': {
        await handlePackageFetch({
          event,
          url: event.zipUrl,
          completeFn: wasmObj.completePackageArchive.bind(wasmObj),
          rejectFn: wasmObj.rejectPackageArchive.bind(wasmObj),
          hostHooks,
          controller,
          isAborted,
          passNow,
        });
        return;
      }
      case 'checkImportModule': {
        let errorMsg = null;
        try {
          await hostHooks.securityManager.checkImportModule(
            event.importingUri,
            event.importedUri,
          );
        } catch (err) {
          errorMsg = err instanceof Error ? err.message : String(err);
        }
        if (!isAborted()) {
          passNow(wasmObj.completeSecurityCheck.bind(wasmObj), token, errorMsg);
        }
        return;
      }
      case 'checkResolveModule': {
        let errorMsg = null;
        try {
          if (typeof hostHooks.securityManager.checkResolveModule === 'function') {
            await hostHooks.securityManager.checkResolveModule(event.uri);
          } else if (typeof hostHooks.securityManager.checkImportModule === 'function') {
            await hostHooks.securityManager.checkImportModule('', event.uri);
          }
        } catch (err) {
          errorMsg = err instanceof Error ? err.message : String(err);
        }
        if (!isAborted()) {
          passNow(wasmObj.completeSecurityCheck.bind(wasmObj), token, errorMsg);
        }
        return;
      }
      case 'checkReadResource': {
        let errorMsg = null;
        try {
          await hostHooks.securityManager.checkReadResource(event.uri);
        } catch (err) {
          errorMsg = err instanceof Error ? err.message : String(err);
        }
        if (!isAborted()) {
          passNow(wasmObj.completeSecurityCheck.bind(wasmObj), token, errorMsg);
        }
        return;
      }
      case 'log': {
        if (onCapturedLog) {
          onCapturedLog(event.message);
        }
        let errorMsg = null;
        try {
          if (hostHooks.logger) {
            if (event.level === 'trace' && typeof hostHooks.logger.trace === 'function') {
              await hostHooks.logger.trace(event.message, event.frame);
            } else if (event.level === 'warn' && typeof hostHooks.logger.warn === 'function') {
              await hostHooks.logger.warn(event.message, event.frame);
            }
          }
        } catch (err) {
          errorMsg = err instanceof Error ? err.message : String(err);
        }
        if (!isAborted()) {
          wasmObj.acknowledgeLog(token, errorMsg, now());
        }
        return;
      }
      case 'transformFrame': {
        let transformed = null;
        let errorMsg = null;
        try {
          transformed = await hostHooks.frameTransformer(event.frame);
        } catch (err) {
          errorMsg = err instanceof Error ? err.message : String(err);
        }
        if (!isAborted()) {
          wasmObj.completeFrameTransform(token, transformed, errorMsg, now());
        }
        return;
      }
      case 'resolveAnalysisGlob': {
        try {
          const matched = await resolveAnalysisGlobPattern(event.patternUri, hostHooks);
          if (!isAborted()) {
            wasmObj.completeAnalysisGlob(matched);
          }
        } catch (err) {
          if (!isAborted()) {
            const msg = err instanceof Error ? err.message : String(err);
            wasmObj.rejectAnalysisGlob('io', msg);
          }
        }
        return;
      }
      default:
        throw new PklError(`Unhandled Wasm host event kind: ${event.kind}`, 'evaluation');
    }
  } finally {
    inFlight.delete(token);
  }
}

async function handleReadModule({ event, wasmObj, hostHooks, controller, isAborted, passNow }) {
  const token = event.token;
  try {
    if (event.backend === 'vfs') {
      const vfs = hostHooks.vfs;
      const vfsPath = fileUriToPath(event.uri);
      if (vfs.isDirectory(vfsPath)) {
        if (!isAborted()) {
          passNow(wasmObj.rejectReadModule.bind(wasmObj), token, 'io', 'Is a directory');
        }
        return;
      }
      if (!vfs.isFile(vfsPath)) {
        if (!isAborted()) {
          passNow(wasmObj.rejectReadModule.bind(wasmObj), token, 'not-found', null);
        }
        return;
      }
      const bytes = vfs.read(vfsPath);
      const text = textDecoder.decode(bytes);
      const resolvedUri = pathToFileUri(vfsPath);
      if (!isAborted()) {
        passNow(wasmObj.completeReadModule.bind(wasmObj), token, resolvedUri, text);
      }
      return;
    }

    if (event.backend === 'http') {
      const resp = await hostHooks.httpFetch(event.uri, {
        method: 'GET',
        headers: {},
        signal: controller.signal,
      });
      if (isAborted()) return;
      if (resp.status === 404) {
        passNow(wasmObj.rejectReadModule.bind(wasmObj), token, 'not-found', null);
        return;
      }
      if (resp.status < 200 || resp.status >= 300) {
        passNow(
          wasmObj.rejectReadModule.bind(wasmObj),
          token,
          'http',
          `Received HTTP status code ${resp.status} when requesting \`${event.uri}\`.`,
        );
        return;
      }
      const bytes = resp.bytes instanceof Uint8Array ? resp.bytes : new Uint8Array(resp.bytes);
      const text = textDecoder.decode(bytes);
      passNow(wasmObj.completeReadModule.bind(wasmObj), token, event.uri, text);
      return;
    }

    if (event.backend === 'custom') {
      const reader = hostHooks.moduleReaders[event.readerIndex];
      const result = await reader.read(event.uri);
      if (isAborted()) return;
      if (result === null || result === undefined) {
        passNow(wasmObj.rejectReadModule.bind(wasmObj), token, 'not-found', null);
        return;
      }
      const text =
        typeof result === 'string'
          ? result
          : result instanceof Uint8Array
            ? textDecoder.decode(result)
            : String(result);
      passNow(wasmObj.completeReadModule.bind(wasmObj), token, event.uri, text);
      return;
    }

    passNow(wasmObj.rejectReadModule.bind(wasmObj), token, 'no-loader', null);
  } catch (err) {
    if (isAborted()) return;
    if (err instanceof PklError && err.kind === 'security') {
      passNow(wasmObj.rejectReadModule.bind(wasmObj), token, 'security', err.message);
      return;
    }
    const kind = event.backend === 'http' ? 'http' : 'io';
    const msg = err instanceof Error ? err.message : String(err);
    passNow(wasmObj.rejectReadModule.bind(wasmObj), token, kind, msg);
  }
}

async function handleModuleHasElement({ event, wasmObj, hostHooks, isAborted, passNow }) {
  const token = event.token;
  try {
    if (event.backend === 'vfs') {
      const vfsPath = fileUriToPath(event.uri);
      const exists = hostHooks.vfs.has(vfsPath);
      if (!isAborted()) {
        passNow(wasmObj.completeModuleHasElement.bind(wasmObj), token, exists);
      }
      return;
    }
    if (event.backend === 'custom') {
      const reader = hostHooks.moduleReaders[event.readerIndex];
      let exists = false;
      if (typeof reader.hasElement === 'function') {
        exists = Boolean(await reader.hasElement(event.uri));
      } else {
        try {
          const val = await reader.read(event.uri);
          exists = val !== null && val !== undefined;
        } catch {
          exists = false;
        }
      }
      if (!isAborted()) {
        passNow(wasmObj.completeModuleHasElement.bind(wasmObj), token, exists);
      }
      return;
    }
    if (!isAborted()) {
      passNow(wasmObj.completeModuleHasElement.bind(wasmObj), token, false);
    }
  } catch (err) {
    if (isAborted()) return;
    const msg = err instanceof Error ? err.message : String(err);
    passNow(wasmObj.rejectModuleHasElement.bind(wasmObj), token, 'io', msg);
  }
}

async function handleReadResource({ event, wasmObj, hostHooks, controller, isAborted, passNow }) {
  const token = event.token;
  try {
    if (event.backend === 'vfs') {
      const vfs = hostHooks.vfs;
      const vfsPath = fileUriToPath(event.uri);
      if (vfs.isDirectory(vfsPath)) {
        const bytes = vfs.readDirectoryResourceBytes(vfsPath);
        if (!isAborted()) {
          passNow(wasmObj.completeReadResource.bind(wasmObj), token, bytes);
        }
        return;
      }
      if (!vfs.isFile(vfsPath)) {
        if (!isAborted()) {
          passNow(wasmObj.completeReadResource.bind(wasmObj), token, null);
        }
        return;
      }
      const bytes = vfs.read(vfsPath);
      if (!isAborted()) {
        passNow(wasmObj.completeReadResource.bind(wasmObj), token, bytes);
      }
      return;
    }

    if (event.backend === 'http') {
      const resp = await hostHooks.httpFetch(event.uri, {
        method: 'GET',
        headers: {},
        signal: controller.signal,
      });
      if (isAborted()) return;
      if (resp.status === 404) {
        passNow(wasmObj.completeReadResource.bind(wasmObj), token, null);
        return;
      }
      if (resp.status < 200 || resp.status >= 300) {
        passNow(
          wasmObj.rejectReadResource.bind(wasmObj),
          token,
          'http',
          `Received HTTP status code ${resp.status} when requesting \`${event.uri}\`.`,
        );
        return;
      }
      const bytes = resp.bytes instanceof Uint8Array ? resp.bytes : new Uint8Array(resp.bytes);
      passNow(wasmObj.completeReadResource.bind(wasmObj), token, bytes);
      return;
    }

    if (event.backend === 'custom') {
      const reader = hostHooks.resourceReaders[event.readerIndex];
      const result = await reader.read(event.uri);
      if (isAborted()) return;
      if (result === null || result === undefined) {
        passNow(wasmObj.completeReadResource.bind(wasmObj), token, null);
        return;
      }
      const bytes =
        result instanceof Uint8Array
          ? new Uint8Array(result)
          : typeof result === 'string'
            ? textEncoder.encode(result)
            : new Uint8Array(result);
      passNow(wasmObj.completeReadResource.bind(wasmObj), token, bytes);
      return;
    }

    passNow(wasmObj.completeReadResource.bind(wasmObj), token, null);
  } catch (err) {
    if (isAborted()) return;
    if (err instanceof PklError && err.kind === 'security') {
      passNow(wasmObj.rejectReadResource.bind(wasmObj), token, 'security', err.message);
      return;
    }
    const kind = event.backend === 'http' ? 'http' : 'io';
    const msg = err instanceof Error ? err.message : String(err);
    passNow(wasmObj.rejectReadResource.bind(wasmObj), token, kind, msg);
  }
}

async function handleResourceHasElement({ event, wasmObj, hostHooks, isAborted, passNow }) {
  const token = event.token;
  try {
    if (event.backend === 'vfs') {
      const vfsPath = fileUriToPath(event.uri);
      const exists = hostHooks.vfs.has(vfsPath);
      if (!isAborted()) {
        passNow(wasmObj.completeResourceHasElement.bind(wasmObj), token, exists);
      }
      return;
    }
    if (event.backend === 'custom') {
      const reader = hostHooks.resourceReaders[event.readerIndex];
      let exists = false;
      if (typeof reader.hasElement === 'function') {
        exists = Boolean(await reader.hasElement(event.uri));
      } else {
        const val = await reader.read(event.uri);
        exists = val !== null && val !== undefined;
      }
      if (!isAborted()) {
        passNow(wasmObj.completeResourceHasElement.bind(wasmObj), token, exists);
      }
      return;
    }
    if (!isAborted()) {
      passNow(wasmObj.completeResourceHasElement.bind(wasmObj), token, false);
    }
  } catch (err) {
    if (isAborted()) return;
    const msg = err instanceof Error ? err.message : String(err);
    passNow(wasmObj.rejectResourceHasElement.bind(wasmObj), token, 'io', msg);
  }
}

async function handleListElements({ event, wasmObj, hostHooks, isAborted, passNow }) {
  const token = event.token;
  try {
    if (event.backend === 'vfs') {
      const vfsPath = fileUriToPath(event.uri);
      const elements = hostHooks.vfs.list(vfsPath);
      if (!isAborted()) {
        passNow(wasmObj.completeListElements.bind(wasmObj), token, elements);
      }
      return;
    }
    if (event.backend === 'custom') {
      const reader =
        event.kind === 'listResources'
          ? hostHooks.resourceReaders[event.readerIndex]
          : hostHooks.moduleReaders[event.readerIndex];
      if (typeof reader.listElements !== 'function') {
        if (!isAborted()) {
          passNow(
            wasmObj.rejectListElements.bind(wasmObj),
            token,
            'unsupported',
            'Custom reader does not implement listElements.',
          );
        }
        return;
      }
      const elements = await reader.listElements(event.uri);
      if (!isAborted()) {
        passNow(wasmObj.completeListElements.bind(wasmObj), token, elements || []);
      }
      return;
    }
    if (!isAborted()) {
      passNow(wasmObj.completeListElements.bind(wasmObj), token, []);
    }
  } catch (err) {
    if (isAborted()) return;
    const msg = err instanceof Error ? err.message : String(err);
    passNow(wasmObj.rejectListElements.bind(wasmObj), token, 'io', msg);
  }
}

async function handlePackageFetch({
  event,
  url,
  completeFn,
  rejectFn,
  hostHooks,
  controller,
  isAborted,
  passNow,
}) {
  const token = event.token;
  try {
    let bytes;
    if (hostHooks.packageTransport) {
      const raw = await hostHooks.packageTransport(url, { signal: controller.signal });
      if (isAborted()) return;
      bytes = raw instanceof Uint8Array ? raw : new Uint8Array(raw);
    } else if (hostHooks.httpFetch) {
      const resp = await hostHooks.httpFetch(url, {
        method: 'GET',
        headers: {},
        signal: controller.signal,
      });
      if (isAborted()) return;
      if (resp.status < 200 || resp.status >= 300) {
        passNow(
          rejectFn,
          token,
          'security',
          `Received HTTP status code ${resp.status} when requesting \`${url}\`.`,
        );
        return;
      }
      bytes = resp.bytes instanceof Uint8Array ? resp.bytes : new Uint8Array(resp.bytes);
    } else {
      passNow(
        rejectFn,
        token,
        'unsupported',
        `No package transport configured to fetch \`${url}\`.`,
      );
      return;
    }
    passNow(completeFn, token, bytes);
  } catch (err) {
    if (isAborted()) return;
    const msg = err instanceof Error ? err.message : String(err);
    passNow(rejectFn, token, 'http', msg);
  }
}

async function resolveAnalysisGlobPattern(patternUri, hostHooks) {
  if (patternUri.toLowerCase().startsWith('file:')) {
    if (!hostHooks.vfs) {
      throw new PklError(
        `Cannot glob \`${patternUri}\`: native filesystem is unavailable in Wasm.`,
        'capability-unavailable',
        'nativeFs',
      );
    }
    const vfsPattern = fileUriToPath(patternUri);
    return hostHooks.vfs.glob(vfsPattern).map((p) => pathToFileUri(p));
  }
  throw new PklError(
    `Unsupported glob pattern in import analyzer: \`${patternUri}\``,
    'evaluation',
  );
}
