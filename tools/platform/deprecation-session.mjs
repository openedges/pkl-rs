// The same public module evaluator transport runs in N-API and Chromium Wasm.
// No warning policy or Java exception classification is implemented here.
import { completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest } from './module-policy-session.mjs';
import { javaUtf8Bytes } from './resource-session.mjs';

const units = value => Uint16Array.from({ length: value.length }, (_, index) => value.charCodeAt(index));
const errorRecord = error => ({ type: typeof error, name: error?.name ?? null,
  message: String(error?.message ?? error), stack: error?.stack ?? null });

export function runDeprecationCase(api, fixture, budget = 1) {
  const result = { id: fixture.id, payloads: {}, protocol: [], trace: [], acquisitionError: null,
    architecture: { polls: 0, collections: 0, terminalSlots: null, terminalDepth: null,
      sessionCreated: false, retainedPayloadsStable: true } };
  let session, logs = 0, warnings = 0, depth = 0, callbackUsed = false, closed = false;
  const retained = [];
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
      observe('api-return', { method, value: value ?? null });
      return value;
    } catch (error) {
      observe('api-throw', { method, error: errorRecord(error) });
      throw error;
    }
  };
  function apiFailure(prefix, error) {
    const actual = errorRecord(error);
    string(prefix + '.stdout', new Uint16Array());
    string(prefix + '.stderr', units(actual.message));
    text(prefix + '.status.bin', 'api-exception\n');
    observe('call-api-error', { prefix, error: actual });
    result.protocol.push(`call-exit=${prefix} status=api-exception`);
  }
  function capture(expression, prefix, callbackToken = null) {
    result.protocol.push('call-enter=' + prefix);
    try {
      if (callbackToken != null && !closed)
        invoke('reenterModuleSource', [callbackToken, fixture.uri, fixture.source, expression, 0, null]);
      else invoke('evaluateModuleSource', [fixture.uri, fixture.source, expression, 0, null]);
    } catch (error) { apiFailure(prefix, error); return; }
    for (;;) {
      if (++result.architecture.polls > 2_000_000) throw Error('session polling bound exceeded');
      let event;
      try { event = session.poll(0, budget); }
      catch (error) { apiFailure(prefix, error); return; }
      session.collect(); result.architecture.collections++;
      if (event.kind === 'yield') continue;
      if (event.kind === 'module-request') {
        observe('module-request', { event: structuredClone(event) });
        const request = validateModuleRequest(event);
        const moduleHost = Object.fromEntries(['completeModuleKey', 'completeModuleCachePolicy', 'completeModuleFragmentPaths']
          .map(method => [method, (...args) => invoke(method, args)]));
        // Supplied text wraps the selected primary key with a distinct key ID.
        // Its fragment policy belongs to the shared implementation, even when
        // its original URI is also in the Host's exact primary-source registry.
        if ((request.kind === 'select-key' || request.keyKind === 'host')
            && completeFixtureModuleKey(moduleHost, event, new Set([fixture.uri]))) continue;
        if (!completeDefaultModuleRequest({ completeDefaultModule: (...args) => invoke('completeDefaultModule', args) }, event))
          throw Error('unconfigured module request');
        continue;
      }
      if (event.kind === 'log') { logger(event); continue; }
      if (!['value', 'error'].includes(event.kind)) throw Error('unexpected event: ' + event.kind);
      const success = event.kind === 'value';
      const status = success ? 'success' : event.disposition === 'language-error' ? 'error' : 'candidate-error';
      string(prefix + '.stdout', success ? event.value : new Uint16Array());
      string(prefix + '.stderr', success ? new Uint16Array() : event.error);
      text(prefix + '.status.bin', status + '\n');
      for (const name of ['disposition', 'phase', 'reason']) text(`${prefix}.${name}.bin`, event[name] ?? '');
      observe('terminal', { prefix, event: structuredClone(event) });
      result.protocol.push(`call-exit=${prefix} status=${status}`);
      return;
    }
  }
  function logger(event) {
    const log = event.log, index = logs++, warning = warnings;
    if (!log || !['warn', 'trace'].includes(log.level) || typeof event.token !== 'string'
        || !Object.hasOwn(log, 'memberName') || (log.memberName !== null && !(log.memberName instanceof Uint16Array)))
      throw Error('invalid logger payload');
    if (log.level === 'warn') warnings++;
    const prefix = 'log-' + index;
    result.protocol.push(`log-enter=${index} level=${log.level} depth=${depth}`);
    observe('logger', { token: event.token, log: structuredClone(log) });
    text(prefix + '.level.bin', log.level);
    string(prefix + '.message', log.message);
    text(prefix + '.message.length.bin', String(log.message.length));
    string(prefix + '.uri', log.moduleUri);
    string(prefix + '.member', log.memberName ?? new Uint16Array());
    text(prefix + '.member.null.bin', String(log.memberName === null));
    text(prefix + '.position.bin', [log.startLine, log.startColumn, log.endLine, log.endColumn].join(':'));
    text(prefix + '.source-count.bin', String(log.sourceLines.length));
    log.sourceLines.forEach((line, index) => string(prefix + '.source-' + index, line));
    depth++;
    let failed = false;
    try {
      if (log.level === 'warn') {
        if (depth === 1 && !callbackUsed && fixture.callback && warning >= fixture.callback_after) {
          callbackUsed = true;
          capture(fixture.callback, 'log-callback-' + index, event.token);
        }
        failed = fixture.action === 'throw' || (fixture.action === 'throw-once' && warning === 0);
        if (fixture.action.startsWith('close')) { invoke('closeFromCallback', [event.token]); closed = true; }
        if (fixture.action === 'close-reenter') capture('stable', 'post-close-callback-' + index);
      }
    } finally { depth--; }
    // Callback close rejects new calls but permits issued continuations.
    invoke('acknowledgeLog', [event.token, failed ? units('stage54-logger-failure') : undefined, 0]);
    if (!failed) result.protocol.push('log-exit=' + index);
  }
  try {
    if (typeof fixture.id !== 'string' || typeof fixture.source !== 'string' || typeof fixture.uri !== 'string'
        || !['capture', 'noop', 'throw', 'throw-once', 'close', 'close-reenter'].includes(fixture.action)
        || typeof fixture.callback !== 'string' || !Number.isSafeInteger(fixture.callback_after) || fixture.callback_after < 0
        || !Array.isArray(fixture.expressions) || !fixture.expressions.length
        || fixture.expressions.some(value => typeof value !== 'string' || !value || /[\r\n]/.test(value))
        || !Number.isSafeInteger(budget) || budget < 1) throw Error('invalid deprecation session profile');
    observe('constructor', { method: 'moduleEvaluator', args: ['compact', fixture.action !== 'noop', false, false] });
    session = api.SourceSession.moduleEvaluator('compact', fixture.action !== 'noop', false, false);
    result.architecture.sessionCreated = true;
    fixture.expressions.forEach((expression, index) => capture(expression, 'outer-' + index));
    invoke('close', []);
    result.protocol.push(`closed logs=${logs} warnings=${warnings}`);
  } catch (error) { result.acquisitionError = errorRecord(error); }
  finally {
    if (session) {
      try {
        session.close(); session.collect(); result.architecture.collections++;
        result.architecture.terminalSlots = session.heapSlots();
        result.architecture.terminalDepth = session.contextDepth();
        session.free?.();
      } catch (error) { result.acquisitionError = { ...result.acquisitionError, cleanup: errorRecord(error) }; }
    }
    result.architecture.retainedPayloadsStable = retained.every(({ view, copy }) =>
      view.length === copy.length && copy.every((unit, index) => unit === view[index]));
    result.protocolBytes = Array.from(new TextEncoder().encode(result.protocol.join('\n') + '\n'));
  }
  return result;
}
