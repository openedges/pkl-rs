// Actual public module evaluator callbacks. Java authority and compatibility
// comparison are external; this transport records owned bytes and API outcomes.
import { completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest } from './module-policy-session.mjs';
import { javaUtf8Bytes } from './resource-session.mjs';

const units = value => Uint16Array.from({ length: value.length }, (_, index) => value.charCodeAt(index));
const errorRecord = error => ({ type: typeof error, name: error?.name ?? null,
  message: String(error?.message ?? error), stack: error?.stack ?? null });
const fields = ['startLine', 'startColumn', 'endLine', 'endColumn'];
const callbackOrigin = value => {
  if (value == null) return '';
  if (!['log-transformer', 'log-delegate', 'diagnostic-transformer', 'diagnostic-renderer'].includes(value))
    throw Error('invalid callback origin metadata');
  return value;
};
export function validateFrame(frame) {
  if (!frame || !(frame.moduleUri instanceof Uint16Array) || !Object.hasOwn(frame, 'memberName')
      || (frame.memberName !== null && !(frame.memberName instanceof Uint16Array))
      || !Array.isArray(frame.sourceLines) || frame.sourceLines.some(line => !(line instanceof Uint16Array))
      || fields.some(key => !Number.isInteger(frame[key]) || frame[key] < 0 || frame[key] > 0xffffffff))
    throw Error('invalid owned UTF16 frame');
  return frame;
}
function replacementFrame(record) {
  const owned = value => {
    if (!Array.isArray(value) || value.some(unit => !Number.isInteger(unit) || unit < 0 || unit > 0xffff))
      throw Error('invalid replacement UTF16 units');
    return Uint16Array.from(value);
  };
  if (!record || !Array.isArray(record.sourceLines)) throw Error('invalid replacement frame');
  return validateFrame({ ...record, moduleUri: owned(record.moduleUri),
    memberName: record.memberName === null ? null : owned(record.memberName),
    sourceLines: record.sourceLines.map(owned) });
}

export function runFrameTransformerCase(api, fixture, budget = 1) {
  const result = { id: fixture.id, payloads: {}, protocol: [], trace: [], acquisitionError: null,
    architecture: { polls: 0, collections: 0, terminalSlots: null, terminalDepth: null,
      sessionCreated: false, retainedPayloadsStable: true } };
  let session, transforms = 0, logs = 0, warnings = 0, depth = 0;
  const used = { transform: false, log: false }, thrown = { transform: false, log: false }, retained = [];
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
  const frame = (prefix, value) => {
    validateFrame(value);
    string(prefix + '.uri', value.moduleUri);
    string(prefix + '.member', value.memberName ?? new Uint16Array());
    text(prefix + '.member.null.bin', String(value.memberName === null));
    text(prefix + '.position.bin', fields.map(key => value[key]).join(':'));
    text(prefix + '.source-count.bin', String(value.sourceLines.length));
    value.sourceLines.forEach((line, index) => string(prefix + '.source-' + index, line));
  };
  const invoke = (method, args) => {
    observe('api-call', { method, args: structuredClone(args) });
    try {
      const value = session[method](...args);
      observe('api-return', { method, value: value ?? null }); return value;
    } catch (error) { observe('api-throw', { method, error: errorRecord(error) }); throw error; }
  };
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
      if (event.kind === 'module-request') {
        observe('module-request', { event: structuredClone(event) });
        const request = validateModuleRequest(event);
        const host = Object.fromEntries(['completeModuleKey', 'completeModuleCachePolicy', 'completeModuleFragmentPaths']
          .map(method => [method, (...args) => invoke(method, args)]));
        if ((request.kind === 'select-key' || request.keyKind === 'host')
            && completeFixtureModuleKey(host, event, new Set([fixture.uri]))) continue;
        if (!completeDefaultModuleRequest({ completeDefaultModule: (...args) => invoke('completeDefaultModule', args) }, event))
          throw Error('unconfigured module request');
        continue;
      }
      if (event.kind === 'transform-frame') { transform(event); continue; }
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
  function callback(kind, profile, index, token) {
    if (!used[kind] && profile.callback && index >= profile.callback_after) {
      used[kind] = true;
      capture(profile.callback, `${kind}-callback-${index}`, token);
    }
  }
  function transform(event) {
    if (typeof event.token !== 'string') throw Error('invalid transformer token');
    const index = transforms++, input = validateFrame(event.frame), profile = fixture.transformer;
    result.protocol.push(`transform-enter=${index} depth=${depth}`);
    observe('transformer', { token: event.token, frame: structuredClone(input) });
    frame(`transform-${index}.input`, input);
    depth++;
    try { callback('transform', profile, index, event.token); } finally { depth--; }
    const failed = profile.action === 'throw-once' && !thrown.transform && index >= profile.callback_after;
    if (failed) {
      thrown.transform = true;
      result.protocol.push('transform-throw=' + index);
      invoke('completeFrameTransform', [event.token, null, units('stage55-transformer-failure'), 0]); return;
    }
    let output = input;
    if (profile.mode === 'full' || profile.mode === 'numbered') {
      output = replacementFrame(fixture.replacement);
      if (profile.mode === 'numbered') {
        const suffix = units('#' + index), uri = new Uint16Array(output.moduleUri.length + suffix.length);
        uri.set(output.moduleUri); uri.set(suffix, output.moduleUri.length); output.moduleUri = uri;
      }
    }
    frame(`transform-${index}.output`, output);
    invoke('completeFrameTransform', [event.token, output, undefined, 0]);
    result.protocol.push('transform-exit=' + index);
  }
  function logger(event) {
    const index = logs++, log = event.log, profile = fixture.logger;
    if (typeof event.token !== 'string' || !log || !['trace', 'warn'].includes(log.level)) throw Error('invalid logger payload');
    if (log.level === 'warn') warnings++;
    result.protocol.push(`log-enter=${index} level=${log.level} depth=${depth}`);
    observe('logger', { token: event.token, log: structuredClone(log) });
    text(`log-${index}.level.bin`, log.level); string(`log-${index}.message`, log.message);
    text(`log-${index}.message.length.bin`, String(log.message.length)); frame(`log-${index}`, log);
    depth++;
    try { callback('log', profile, index, event.token); } finally { depth--; }
    const failed = profile.action === 'throw-once' && !thrown.log && index >= profile.callback_after;
    if (failed) thrown.log = true;
    if (failed) result.protocol.push('log-throw=' + index);
    invoke('acknowledgeLog', [event.token, failed ? units('stage55-logger-failure') : undefined, 0]);
    if (!failed) result.protocol.push('log-exit=' + index);
  }
  try {
    const profile = (value, modes) => value && modes.includes(value.mode)
      && ['none', 'throw-once'].includes(value.action) && typeof value.callback === 'string'
      && Number.isSafeInteger(value.callback_after) && value.callback_after >= 0;
    if (typeof fixture.id !== 'string' || typeof fixture.uri !== 'string' || typeof fixture.source !== 'string'
        || (fixture.operation ?? 'expression') !== 'expression'
        || !profile(fixture.transformer, ['default', 'builtin-identity', 'identity', 'full', 'numbered'])
        || !profile(fixture.logger, ['capture', 'noop'])
        || !Array.isArray(fixture.expressions) || !fixture.expressions.length
        || fixture.expressions.some(value => typeof value !== 'string' || !value || /[\r\n]/.test(value))
        || !Number.isSafeInteger(budget) || budget < 1) throw Error('invalid frame transformer session profile');
    if (['full', 'numbered'].includes(fixture.transformer.mode)) replacementFrame(fixture.replacement);
    const hostTransform = ['identity', 'full', 'numbered'].includes(fixture.transformer.mode);
    const args = ['compact', fixture.logger.mode !== 'noop', hostTransform, false];
    observe('constructor', { method: 'moduleEvaluator', args });
    session = api.SourceSession.moduleEvaluator(...args); result.architecture.sessionCreated = true;
    if (fixture.transformer.mode === 'builtin-identity') invoke('setFrameTransformer', ['identity']);
    fixture.expressions.forEach((expression, index) => capture(expression, 'outer-' + index));
    invoke('close', []); result.protocol.push(`closed transforms=${transforms} logs=${logs} warnings=${warnings}`);
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
