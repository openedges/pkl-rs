// One public SourceProbe for an entire ordered canonical DataSize string session.
// Java typed export and exception-class metadata remain separate authority.
import { completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest } from './module-policy-session.mjs';
import { javaUtf8Bytes } from './resource-session.mjs';

const units = text => Uint16Array.from({ length: text.length }, (_, index) => text.charCodeAt(index));
const errorRecord = error => ({ name: error?.name ?? null, message: String(error?.message ?? error),
  stack: error?.stack ?? null, callbackOrigin: error?.callbackOrigin ?? null });
const callbackOrigin = value => {
  if (value == null) return '';
  if (!['log-transformer', 'log-delegate', 'diagnostic-transformer', 'diagnostic-renderer'].includes(value))
    throw Error('invalid callback origin metadata');
  return value;
};
const defaultKinds = new Set(['check-import', 'select-key', 'query-cache-policy',
  'query-fragment-paths', 'resolve', 'resolve-key', 'resolve-import-uri']);

export function runDataSizeSession(api, fixture, budget = 1) {
  const result = { schema: 1, rows: [], protocol: [], trace: [], acquisitionError: null,
    boundary: 'String query status and complete UTF16/Java UTF8 output only; Java error class and typed DataSize export are not synthesized.',
    architecture: { sessionCreated: false, polls: 0, collections: 0, terminalSlots: null,
      terminalDepth: null, retainedPayloadsStable: null, closed: false } };
  let session;
  const retained = [], keys = new Map(), requestIds = new Set(), tokens = new Set();
  const observe = (kind, fields = {}) => result.trace.push({ kind, ...fields });
  const call = (method, args) => {
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
  const write = (payloads, name, bytes) => {
    if (Object.hasOwn(payloads, name)) throw Error('duplicate observation: ' + name);
    payloads[name] = Array.from(bytes);
  };
  const text = (payloads, name, value) => {
    if (typeof value !== 'string') throw Error('invalid candidate text metadata: ' + name);
    write(payloads, name, javaUtf8Bytes(Array.from(units(value))));
  };
  const string = (payloads, name, value, owned = false) => {
    if (!(value instanceof Uint16Array)) throw Error('expected UTF16 payload: ' + name);
    if (owned) retained.push({ view: value, copy: Array.from(value) });
    write(payloads, name + '.code-units.bin', Array.from(value).flatMap(unit => [unit >> 8, unit & 255]));
    write(payloads, name + '.bin', javaUtf8Bytes(Array.from(value)));
  };
  const defaultRequest = event => {
    if (!defaultKinds.has(event.module.kind)
        || !completeDefaultModuleRequest({ completeDefaultModule: (...args) => call('completeDefaultModule', args) }, event))
      throw Error('unconfigured module request');
  };
  const moduleRequest = event => {
    observe('module-request', { event: structuredClone(event) });
    const request = validateModuleRequest(event);
    if (requestIds.has(request.id) || tokens.has(event.token)) throw Error('reused module request identity');
    requestIds.add(request.id); tokens.add(event.token);
    const host = Object.fromEntries(['completeModuleKey', 'completeModuleCachePolicy', 'completeModuleFragmentPaths']
      .map(method => [method, (...args) => call(method, args)]));
    const primary = request.kind === 'select-key' && request.selectionPhase === 'primary'
      && request.originalUri === fixture.uri && fixture.uri.startsWith('file:');
    if (primary) {
      if (keys.has(request.keyId)) throw Error('reused Host ModuleKey identity');
      completeFixtureModuleKey(host, event, new Set([fixture.uri]));
      keys.set(request.keyId, { uri: fixture.uri, handle: `fixture-key:${request.keyId}` });
      return;
    }
    if (request.keyKind !== 'host') { defaultRequest(event); return; }
    const key = keys.get(request.keyId);
    if (!key || key.uri !== request.originalUri || key.handle !== request.keyHandle)
      throw Error('Host ModuleKey descriptor ownership mismatch');
    if (completeFixtureModuleKey(host, event, new Set([key.uri]))) return;
    if (request.kind === 'capabilities') {
      call('completeModuleCapabilities', [event.token, true, true, false, 0]); return;
    }
    // The canonical module has no support sources. The shared default handles
    // import security, standard-library keys and URI policy where configured.
    defaultRequest(event);
  };
  const finishRow = (row, status, value, metadata) => {
    const success = status === 'success';
    text(row.payloads, 'result.status.bin', status + '\n');
    string(row.payloads, 'result.stdout', success ? value : new Uint16Array(), success);
    string(row.payloads, 'result.stderr', success ? new Uint16Array() : value, !success);
    for (const [name, content] of Object.entries(metadata)) text(row.payloads, `candidate.${name}.bin`, content);
    row.status = status;
    result.protocol.push(`call-exit=string-${row.index}-${row.id} status=${status}`);
  };
  const apiFailure = (row, error) => {
    const actual = errorRecord(error);
    observe('api-error', { index: row.index, id: row.id, error: actual });
    finishRow(row, 'api-exception', units(actual.message), {
      'api-error-name': actual.name ?? '', 'callback-origin': callbackOrigin(actual.callbackOrigin),
    });
  };
  try {
    if (!api?.SourceSession || typeof api.SourceSession.moduleEvaluator !== 'function'
        || !fixture || typeof fixture.uri !== 'string' || !/^file:\/\//.test(fixture.uri)
        || /[\r\n\t]/.test(fixture.uri) || typeof fixture.source !== 'string'
        || !Array.isArray(fixture.rows) || fixture.rows.length === 0
        || !Number.isSafeInteger(budget) || budget < 1) throw Error('invalid DataSize session profile');
    const ids = new Set();
    for (const row of fixture.rows) {
      if (typeof row?.id !== 'string' || !/^[a-z0-9-]+$/.test(row.id) || ids.has(row.id)
          || typeof row.expression !== 'string' || !row.expression || /[\r\n\t]/.test(row.expression))
        throw Error('invalid or duplicate DataSize query');
      ids.add(row.id);
    }
    result.protocol.push(`session-open mode=string rows=${fixture.rows.length}`);
    observe('constructor', { method: 'moduleEvaluator', args: ['compact', false, false, false] });
    session = api.SourceSession.moduleEvaluator('compact', false, false, false);
    result.architecture.sessionCreated = true;
    for (const [index, query] of fixture.rows.entries()) {
      const row = { index, id: query.id, status: null, payloads: {} };
      result.rows.push(row);
      string(row.payloads, 'expression', units(query.expression));
      result.protocol.push(`call-enter=string-${index}-${query.id}`);
      observe('evaluate-module-source', { uri: fixture.uri, expression: query.expression, index, id: query.id });
      try { call('evaluateModuleSource', [fixture.uri, fixture.source, query.expression, 0, null]); }
      catch (error) { apiFailure(row, error); continue; }
      for (;;) {
        if (++result.architecture.polls > 2_000_000) throw Error('session polling bound exceeded');
        let event;
        try { event = session.poll(0, budget); }
        catch (error) { apiFailure(row, error); break; }
        session.collect(); result.architecture.collections++;
        if (event.kind === 'yield') continue;
        if (event.kind === 'module-request') { moduleRequest(event); continue; }
        if (!['value', 'error'].includes(event.kind)) throw Error('unexpected event in nonlogging session: ' + event.kind);
        const success = event.kind === 'value';
        const status = success ? 'success' : event.disposition === 'language-error' ? 'error' : 'candidate-error';
        const value = success ? event.value : event.error;
        if (!(value instanceof Uint16Array)) throw Error('missing owned terminal UTF16 payload');
        observe('terminal', { index, id: query.id, event: structuredClone(event) });
        finishRow(row, status, value, {
          disposition: event.disposition ?? '', phase: event.phase ?? '', reason: event.reason ?? '',
          'callback-origin': callbackOrigin(event.callbackOrigin),
        });
        break;
      }
    }
    call('close', []); result.architecture.closed = true;
    session.collect(); result.architecture.collections++;
    result.architecture.terminalSlots = session.heapSlots();
    result.architecture.terminalDepth = session.contextDepth();
    if (result.architecture.terminalSlots !== 0 || result.architecture.terminalDepth !== 0)
      throw Error('nonzero state after terminal close');
    const success = result.rows.filter(row => row.status === 'success').length;
    result.protocol.push(`session-closed mode=string rows=${result.rows.length} public-calls=${result.rows.length}`
      + ` success=${success} errors=${result.rows.length - success} datasize-rows=0 repeated-export-calls=0`);
  } catch (error) { result.acquisitionError = errorRecord(error); }
  finally {
    if (session) try {
      if (!result.architecture.closed) { session.close(); result.architecture.closed = true; }
      session.collect(); result.architecture.collections++;
      result.architecture.terminalSlots = session.heapSlots();
      result.architecture.terminalDepth = session.contextDepth();
      session.free?.();
    } catch (error) { result.acquisitionError = { ...result.acquisitionError, cleanup: errorRecord(error) }; }
    try { result.architecture.retainedPayloadsStable = retained.every(({ view, copy }) =>
      view.length === copy.length && copy.every((unit, index) => unit === view[index])); }
    catch { result.architecture.retainedPayloadsStable = false; }
    if (result.architecture.retainedPayloadsStable === false && result.acquisitionError === null)
      result.acquisitionError = { name: 'OwnedPayloadError', message: 'owned UTF16 result changed after close/free' };
    result.protocolBytes = Array.from(new TextEncoder().encode(result.protocol.join('\n') + '\n'));
  }
  return result;
}
