// Public SourceProbe transport shared by N-API and Chromium Wasm. Class semantics,
// URI resolution and evaluation caches stay in Rust; Java comparison is external.
import { completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest } from './module-policy-session.mjs';
import { javaUtf8Bytes } from './resource-session.mjs';

const units = value => Uint16Array.from({ length: value.length }, (_, index) => value.charCodeAt(index));
const errorRecord = error => ({ type: typeof error, name: error?.name ?? null,
  message: String(error?.message ?? error), stack: error?.stack ?? null });
const positions = ['startLine', 'startColumn', 'endLine', 'endColumn'];
const defaultKinds = new Set(['check-import', 'select-key', 'query-cache-policy', 'query-fragment-paths',
  'resolve', 'resolve-key', 'resolve-import-uri']);
const callbackOrigin = value => {
  if (value == null) return '';
  if (!['log-transformer', 'log-delegate', 'diagnostic-transformer', 'diagnostic-renderer'].includes(value))
    throw Error('invalid callback origin metadata');
  return value;
};
const validUri = value => typeof value === 'string' && /^[a-z][a-z0-9+.-]*:/.test(value) && !/[\r\n\t]/.test(value);

export function runClassValuesCase(api, fixture, budget = 1) {
  const result = { id: fixture?.id, payloads: {}, protocol: [], trace: [], acquisitionError: null,
    providerBoundary: 'Actual candidate SourceProbe Host requests/completions are trace metadata only. Java ModuleKeyFactory/ModuleKey provider API parity is not executed or claimed.',
    architecture: { polls: 0, collections: 0, terminalSlots: null, terminalDepth: null,
      sessionCreated: false, retainedPayloadsStable: true } };
  let session, logs = 0, warnings = 0, depth = 0;
  let callbackUsed = false, throwUsed = false, closeUsed = false;
  const retained = [], sources = new Map(), keys = new Map(), resolved = new Map();
  const requestIds = new Set(), tokens = new Set();
  const observe = (kind, fields = {}) => result.trace.push({ kind, ...fields });
  const write = (name, bytes) => {
    if (Object.hasOwn(result.payloads, name)) throw Error('duplicate observation: ' + name);
    result.payloads[name] = Array.from(bytes);
  };
  const text = (name, value) => write(name, javaUtf8Bytes(Array.from(units(value))));
  const string = (name, value) => {
    if (!(value instanceof Uint16Array)) throw Error('expected owned UTF16 payload: ' + name);
    retained.push({ view: value, copy: Array.from(value) });
    write(name + '.code-units.bin', Array.from(value).flatMap(unit => [unit >> 8, unit & 255]));
    write(name + '.bin', javaUtf8Bytes(Array.from(value)));
  };
  const invoke = (method, args) => {
    observe('api-call', { method, args: structuredClone(args) });
    try {
      const value = session[method](...args);
      observe('api-return', { method, value: value ?? null }); return value;
    } catch (error) { observe('api-throw', { method, error: errorRecord(error) }); throw error; }
  };
  function defaultRequest(event) {
    if (!defaultKinds.has(event.module.kind)
        || !completeDefaultModuleRequest({ completeDefaultModule: (...args) => invoke('completeDefaultModule', args) }, event))
      throw Error('unconfigured module request');
  }
  function module(event) {
    observe('module-request', { event: structuredClone(event) });
    const request = validateModuleRequest(event);
    if (requestIds.has(request.id) || tokens.has(event.token)) throw Error('reused module request identity');
    requestIds.add(request.id); tokens.add(event.token);
    const host = Object.fromEntries(['completeModuleKey', 'completeModuleCachePolicy', 'completeModuleFragmentPaths']
      .map(method => [method, (...args) => invoke(method, args)]));
    const primary = request.originalUri === fixture.uri && fixture.uri.startsWith('file:')
      && request.kind === 'select-key' && request.selectionPhase === 'primary';
    if (request.kind === 'select-key' && (sources.has(request.originalUri) || primary)) {
      if (keys.has(request.keyId)) throw Error('reused Host ModuleKey identity');
      completeFixtureModuleKey(host, event, new Set([request.originalUri]));
      keys.set(request.keyId, { uri: request.originalUri, handle: `fixture-key:${request.keyId}`, primary }); return;
    }
    if (request.keyKind !== 'host') { defaultRequest(event); return; }
    const key = keys.get(request.keyId);
    if (!key || key.uri !== request.originalUri || key.handle !== request.keyHandle)
      throw Error('Host ModuleKey descriptor ownership mismatch');
    if (completeFixtureModuleKey(host, event, new Set([key.uri]))) return;
    if (request.kind === 'resolve' || request.kind === 'resolve-key') {
      if (key.primary) { defaultRequest(event); return; }
      if (resolved.has(request.resolvedKeyId)) throw Error('reused resolved-key identity');
      const handle = `class-resolved:${request.resolvedKeyId}`;
      invoke('completeModuleResolvedKey', [event.token, request.keyId, request.resolvedKeyId,
        handle, key.uri, key.uri, key.uri, 0]);
      resolved.set(request.resolvedKeyId, { keyId: request.keyId, handle }); return;
    }
    if (request.kind === 'load-source') {
      const owner = resolved.get(request.resolvedKeyId), source = sources.get(key.uri);
      if (!source || key.primary || !owner || owner.keyId !== request.keyId || owner.handle !== request.resolvedKeyHandle
          || request.resolvedUri !== key.uri || request.sourceUri !== key.uri)
        throw Error('resolved-key descriptor ownership mismatch');
      invoke('completeModuleSource', [event.token, source.source, source.moduleName, 0]); return;
    }
    if (request.kind === 'capabilities') {
      invoke('completeModuleCapabilities', [event.token, key.primary, true, false, 0]); return;
    }
    defaultRequest(event);
  }
  function apiFailure(prefix, error) {
    const actual = errorRecord(error);
    string(prefix + '.stdout', new Uint16Array()); string(prefix + '.stderr', units(actual.message));
    text(prefix + '.status.bin', 'api-exception\n');
    text(prefix + '.callback-origin.bin', callbackOrigin(error?.callbackOrigin));
    observe('call-api-error', { prefix, error: actual });
    result.protocol.push(`call-exit=${prefix} status=api-exception`);
  }
  function capture(expression, prefix, token = null) {
    result.protocol.push('call-enter=' + prefix);
    try {
      if (token !== null) invoke('reenterModuleSource', [token, fixture.uri, fixture.source, expression, 0, null]);
      else invoke('evaluateModuleSource', [fixture.uri, fixture.source, expression, 0, null]);
    } catch (error) { apiFailure(prefix, error); return; }
    for (;;) {
      if (++result.architecture.polls > 2_000_000) throw Error('session polling bound exceeded');
      let event;
      try { event = session.poll(0, budget); } catch (error) { apiFailure(prefix, error); return; }
      session.collect(); result.architecture.collections++;
      if (event.kind === 'yield') continue;
      if (event.kind === 'module-request') { module(event); continue; }
      if (event.kind === 'log') { logger(event); continue; }
      if (!['value', 'error'].includes(event.kind)) throw Error('unexpected event: ' + event.kind);
      const success = event.kind === 'value';
      const status = success ? 'success' : event.disposition === 'language-error' ? 'error' : 'candidate-error';
      string(prefix + '.stdout', success ? event.value : new Uint16Array());
      string(prefix + '.stderr', success ? new Uint16Array() : event.error);
      text(prefix + '.status.bin', status + '\n');
      for (const name of ['disposition', 'phase', 'reason']) text(`${prefix}.${name}.bin`, event[name] ?? '');
      text(prefix + '.callback-origin.bin', callbackOrigin(event.callbackOrigin));
      observe('terminal', { prefix, event: structuredClone(event) });
      result.protocol.push(`call-exit=${prefix} status=${status}`); return;
    }
  }
  function logger(event) {
    const log = event.log, index = logs++, profile = fixture.logger;
    if (typeof event.token !== 'string' || !event.token || !log || !['trace', 'warn'].includes(log.level)
        || !(log.moduleUri instanceof Uint16Array) || !Object.hasOwn(log, 'memberName')
        || (log.memberName !== null && !(log.memberName instanceof Uint16Array))
        || !Array.isArray(log.sourceLines) || log.sourceLines.some(line => !(line instanceof Uint16Array))
        || positions.some(name => !Number.isInteger(log[name]) || log[name] < 0 || log[name] > 0xffffffff))
      throw Error('invalid owned UTF16 logger frame');
    if (log.level === 'warn') warnings++;
    const prefix = 'log-' + index;
    result.protocol.push(`log-enter=${index} level=${log.level} depth=${depth}`);
    observe('logger', { token: event.token, log: structuredClone(log) });
    text(prefix + '.level.bin', log.level); string(prefix + '.message', log.message);
    text(prefix + '.message.length.bin', String(log.message.length));
    string(prefix + '.uri', log.moduleUri); string(prefix + '.member', log.memberName ?? new Uint16Array());
    text(prefix + '.member.null.bin', String(log.memberName === null));
    text(prefix + '.position.bin', positions.map(name => log[name]).join(':'));
    text(prefix + '.source-count.bin', String(log.sourceLines.length));
    log.sourceLines.forEach((line, lineIndex) => string(prefix + '.source-' + lineIndex, line));
    depth++;
    try {
      if (!callbackUsed && profile.callback && index >= profile.callback_after) {
        callbackUsed = true; capture(profile.callback, 'log-callback-' + index, event.token);
      }
      // Actions run after reentry; a nested logger may consume the once flag.
      if (!throwUsed && profile.action === 'throw-once' && index >= profile.callback_after) {
        throwUsed = true; result.protocol.push('log-throw=' + index);
        invoke('acknowledgeLog', [event.token, units('stage56-logger-failure'), 0]); return;
      }
      if (!closeUsed && profile.action === 'close' && index >= profile.callback_after) {
        closeUsed = true; invoke('closeFromCallback', [event.token]); result.protocol.push('logger-close=' + index);
      }
    } finally { depth--; }
    invoke('acknowledgeLog', [event.token, undefined, 0]); result.protocol.push('log-exit=' + index);
  }
  try {
    const profile = fixture?.logger;
    if (typeof fixture?.id !== 'string' || !validUri(fixture.uri) || typeof fixture.source !== 'string'
        || !profile || !['capture', 'noop'].includes(profile.mode) || !['none', 'throw-once', 'close'].includes(profile.action)
        || typeof profile.callback !== 'string' || /[\r\n]/.test(profile.callback)
        || !Number.isSafeInteger(profile.callback_after) || profile.callback_after < 0
        || (profile.mode === 'noop' && (profile.action !== 'none' || profile.callback !== ''))
        || !Array.isArray(fixture.expressions) || !fixture.expressions.length
        || fixture.expressions.some(value => typeof value !== 'string' || !value || /[\r\n]/.test(value))
        || !Array.isArray(fixture.support_modules) || !Number.isSafeInteger(budget) || budget < 1)
      throw Error('invalid Class session profile');
    for (const source of [fixture, ...fixture.support_modules]) {
      if (!validUri(source.uri) || typeof source.source !== 'string' || typeof source.moduleName !== 'string'
          || sources.has(source.uri) || (source !== fixture && source.uri === fixture.uri)) throw Error('invalid or duplicate authored module source');
      if (source !== fixture) sources.set(source.uri, source);
    }
    const args = ['compact', profile.mode !== 'noop', false, false];
    observe('constructor', { method: 'moduleEvaluator', args });
    session = api.SourceSession.moduleEvaluator(...args); result.architecture.sessionCreated = true;
    fixture.expressions.forEach((expression, index) => capture(expression, 'outer-' + index));
    invoke('close', []); result.protocol.push(`closed logs=${logs} warnings=${warnings}`);
  } catch (error) { result.acquisitionError = errorRecord(error); }
  finally {
    if (session) try {
      session.close(); session.collect(); result.architecture.collections++;
      result.architecture.terminalSlots = session.heapSlots(); result.architecture.terminalDepth = session.contextDepth(); session.free?.();
    } catch (error) { result.acquisitionError = { ...result.acquisitionError, cleanup: errorRecord(error) }; }
    result.architecture.retainedPayloadsStable = retained.every(({ view, copy }) =>
      view.length === copy.length && copy.every((unit, index) => unit === view[index]));
    result.protocolBytes = Array.from(new TextEncoder().encode(result.protocol.join('\n') + '\n'));
  }
  return result;
}
