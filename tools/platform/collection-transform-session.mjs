// Shared browser/Node transport. Collection evaluation and caches stay in Rust.
import {completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest} from './module-policy-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

const dispositions = new Set(['language-error', 'unsupported', 'limit-exceeded', 'internal-failure']);
const scopes = new Set(['ordinary', 'intent-not-reached', 'public-api-boundary']);
const same = (left, right) => JSON.stringify(left) === JSON.stringify(right);
const textUnits = text => {
  if (typeof text !== 'string') throw Error('expected textual transport field');
  return Array.from({length: text.length}, (_, index) => text.charCodeAt(index));
};
const hex = units => javaUtf8Bytes(units).map(byte => byte.toString(16).padStart(2, '0')).join('');
const integer = value => Number.isSafeInteger(value) && value >= 0;
function vector(value) {
  if (!Array.isArray(value) || value.some(unit => !integer(unit) || unit > 65535))
    throw Error('invalid UTF16 vector');
  return value;
}
function encoded(value) {
  vector(value?.units);
  if (typeof value.javaUtf8Hex !== 'string' || hex(value.units) !== value.javaUtf8Hex)
    throw Error('inconsistent Java UTF16/UTF8 record');
  return value;
}

export function runCollectionTransformCase(api, fixture, budget = 1) {
  let session, reads = 0, logs = 0, polls = 0;
  const retained = [], attempts = new Map(), keys = new Map(), resolved = new Map();
  const result = {id: fixture?.id, calls: [], effects: [], observations: [], moduleRequests: [], resourcePolicyRequests: [],
    acquisitionError: null, cleanupErrors: [],
    architecture: {collections: 0, terminalSlots: null, ownedResultsStable: false, providerEntriesReleased: false},
    profile: 'public-module-uri/exact-virtual-source/shared-default-import-policy/ct-prefix-resource-policy/explicit-reader-rules'};
  const observe = (kind, details = {}) => result.observations.push({kind, ...details});
  function units(view) {
    if (!(view instanceof Uint16Array)) throw Error('missing lossless runtime payload');
    const snapshot = Array.from(view);
    retained.push({view, snapshot});
    return snapshot;
  }
  function recordLog(callIndex, log) {
    if (!log || !['trace', 'warn'].includes(log.level) || !Object.hasOwn(log, 'memberName')
        || !Array.isArray(log.sourceLines)) throw Error('invalid logger payload');
    const frame = {moduleUri: units(log.moduleUri),
      memberName: log.memberName === null ? null : units(log.memberName),
      sourceLines: log.sourceLines.map(units)};
    for (const field of ['startLine', 'startColumn', 'endLine', 'endColumn']) {
      if (!integer(log[field])) throw Error('invalid logger position');
      frame[field] = log[field];
    }
    result.effects.push({callIndex, kind: 'log', level: log.level, message: units(log.message), frame});
    logs++;
  }
  function module(event) {
    result.moduleRequests.push({token: event.token ?? null, request: structuredClone(event.module ?? null)});
    const request = validateModuleRequest(event);
    const configured = request.originalUri === fixture.uri;
    if (configured && request.kind === 'select-key') {
      if (keys.has(request.keyId)) throw Error('reused fixture key identity');
      completeFixtureModuleKey(session, event, new Set([fixture.uri]));
      keys.set(request.keyId, `fixture-key:${request.keyId}`);
      return;
    }
    if (configured && request.keyKind === 'host') {
      if (!keys.has(request.keyId) || keys.get(request.keyId) !== request.keyHandle)
        throw Error('fixture key does not belong to this provider');
      if (completeFixtureModuleKey(session, event, new Set([fixture.uri]))) return;
      if (request.kind === 'resolve' || request.kind === 'resolve-key') {
        if (resolved.has(request.resolvedKeyId)) throw Error('reused resolved key identity');
        const handle = `collection-resolved:${request.resolvedKeyId}`;
        session.completeModuleResolvedKey(event.token, request.keyId, request.resolvedKeyId,
          handle, fixture.uri, fixture.uri, fixture.uri, 0);
        resolved.set(request.resolvedKeyId, {key: request.keyId, handle});
        return;
      }
      if (request.kind === 'load-source') {
        const owner = resolved.get(request.resolvedKeyId);
        if (!owner || owner.key !== request.keyId || owner.handle !== request.resolvedKeyHandle
            || request.resolvedUri !== fixture.uri || request.sourceUri !== fixture.uri)
          throw Error('source request does not belong to this provider');
        session.completeModuleSource(event.token, fixture.source, fixture.moduleName, 0);
        return;
      }
      if (request.kind === 'capabilities') {
        session.completeModuleCapabilities(event.token, true, true, false, 0); return;
      }
    }
    if (!['check-import', 'select-key', 'query-cache-policy', 'query-fragment-paths',
      'resolve', 'resolve-key', 'resolve-import-uri'].includes(request.kind)
        || !completeDefaultModuleRequest(session, event))
      throw Error('unconfigured module operation');
  }
  try {
    if (!integer(budget) || budget === 0 || typeof fixture.source !== 'string'
        || typeof fixture.uri !== 'string' || typeof fixture.moduleName !== 'string'
        || !Array.isArray(fixture.operations) || !Array.isArray(fixture.readerRules))
      throw Error('invalid collection fixture inputs');
    const rules = new Map();
    for (const rule of fixture.readerRules) {
      if (typeof rule.uri !== 'string' || typeof rule.value !== 'string'
          || !['text', 'io-first', 'io'].includes(rule.kind) || rules.has(rule.uri))
        throw Error('invalid or duplicate reader rule');
      rules.set(rule.uri, rule);
    }
    session = api.SourceSession.moduleEvaluator('compact', true, false, false);
    observe('constructed');
    for (const operation of fixture.operations) {
      if (operation.kind === 'gc') {
        if (operation.value !== undefined && operation.value !== '') throw Error('GC operand must be empty');
        session.collect(); result.architecture.collections++; observe('explicit-collect'); continue;
      }
      if (operation.kind === 'close') {
        if (operation.value !== undefined && operation.value !== '') throw Error('close operand must be empty');
        observe('close-call'); session.close(); observe('close-return'); continue;
      }
      if (operation.kind !== 'evaluate' || typeof operation.value !== 'string')
        throw Error('invalid collection operation');
      const call = {index: result.calls.length, expression: operation.value,
        readsBefore: reads, logsBefore: logs, stdoutUnits: [], stderrUnits: [], disposition: null, phase: null};
      observe('evaluate-call', {index: call.index, expression: call.expression});
      let admitted = true;
      try { session.evaluateModuleSource(fixture.uri, null, operation.value, 0, 8000); }
      catch (error) {
        admitted = false;
        call.status = 'api-error'; call.stderrUnits = textUnits(error.message ?? String(error));
        call.apiErrorName = error.name ?? null;
      }
      if (admitted) {
        while (true) {
          if (++polls > 1_000_000) throw Error('collection poll bound exceeded');
          const event = session.poll(0, budget);
          session.collect(); result.architecture.collections++;
          if (event.kind === 'yield') continue;
          if (event.kind === 'module-request') { module(event); continue; }
          if (event.kind === 'read') {
            if (typeof event.uri !== 'string' || typeof event.token !== 'string')
              throw Error('unconfigured reader URI or token');
            // CollectionProbe uses Pattern.compile("ct:.*") with lookingAt(),
            // so matching is a case-sensitive prefix test even before a newline.
            // Security runs before reader lookup, attempts, counters and effects.
            const allowed = event.uri.startsWith('ct:');
            const message = allowed ? '' : `Refusing to read resource \`${event.uri}\` because it does not match any entry in the resource allowlist (\`--allowed-resources\`).`;
            result.resourcePolicyRequests.push({callIndex: call.index, uri: event.uri,
              allowed, message, token: event.token});
            if (!allowed) {
              session.rejectReadSecurityManager(event.token, message, 0);
              continue;
            }
            const rule = rules.get(event.uri);
            if (!rule) throw Error('unconfigured reader URI or token');
            const attempt = (attempts.get(event.uri) ?? 0) + 1;
            attempts.set(event.uri, attempt); reads++;
            let response;
            if (rule.kind === 'io' || (rule.kind === 'io-first' && attempt === 1)) {
              response = {kind: 'io', message: rule.value};
              result.effects.push({callIndex: call.index, kind: 'read', uri: event.uri, attempt, result: response});
              session.rejectReadIo(event.token, rule.value, 0);
            } else {
              response = {kind: 'text', units: textUnits(rule.kind === 'io-first' ? 'recovered' : rule.value)};
              result.effects.push({callIndex: call.index, kind: 'read', uri: event.uri, attempt, result: response});
              session.completeReadText(event.token, Uint16Array.from(response.units), 0);
            }
            continue;
          }
          if (event.kind === 'log') { recordLog(call.index, event.log); session.acknowledgeLog(event.token, undefined, 0); continue; }
          if (event.kind !== 'value' && event.kind !== 'error') throw Error('unexpected collection event');
          call.status = event.kind === 'value' ? 'success' : 'error';
          call[event.kind === 'value' ? 'stdoutUnits' : 'stderrUnits'] = units(event.kind === 'value' ? event.value : event.error);
          call.disposition = event.disposition ?? null; call.phase = event.phase ?? null;
          break;
        }
      }
      call.readsAfter = reads; call.logsAfter = logs;
      result.calls.push(call); observe('evaluate-return', {index: call.index, status: call.status});
      if (call.status === 'error' && !dispositions.has(call.disposition))
        throw Error('unknown runtime error classification');
      if (call.status === 'success' && call.disposition !== null)
        throw Error('successful runtime result has an error classification');
    }
  } catch (error) { result.acquisitionError = String(error.message ?? error); }
  finally {
    if (session) {
      try { observe('final-close-call'); session.close(); observe('final-close-return'); }
      catch (error) { result.cleanupErrors.push(String(error.message ?? error)); }
      try { session.collect(); result.architecture.collections++; result.architecture.terminalSlots = session.heapSlots(); }
      catch (error) { result.cleanupErrors.push(String(error.message ?? error)); }
      try { session.free?.(); }
      catch (error) { result.cleanupErrors.push(String(error.message ?? error)); }
      try { result.architecture.ownedResultsStable = retained.every(({view, snapshot}) => same(Array.from(view), snapshot)); }
      catch (error) { result.cleanupErrors.push(String(error.message ?? error)); }
    }
    keys.clear(); resolved.clear(); attempts.clear();
    result.architecture.providerEntriesReleased = true;
  }
  return result;
}

function compareEffect(expected, actual) {
  if (!actual || expected.kind !== actual.kind || expected.callIndex !== actual.callIndex) return false;
  if (expected.kind === 'read') {
    if (expected.result.kind === 'text') vector(expected.result.units);
    else if (expected.result.kind !== 'io' || typeof expected.result.message !== 'string') throw Error('invalid expected reader result');
    return expected.uri === actual.uri && expected.attempt === actual.attempt && same(expected.result, actual.result);
  }
  if (expected.kind !== 'log') throw Error('invalid expected effect kind');
  const text = (record, value) => same(encoded(record).units, value) && record.javaUtf8Hex === hex(value);
  const frame = expected.frame, other = actual.frame;
  if (!other || typeof frame.memberNameNull !== 'boolean') return false;
  if (frame.memberNameNull && (!same(encoded(frame.memberName).units, []) || frame.memberName.javaUtf8Hex !== ''))
    throw Error('invalid null member payload');
  return expected.level === actual.level && text(expected.message, actual.message)
    && text(frame.moduleUri, other.moduleUri)
    && (frame.memberNameNull ? other.memberName === null : other.memberName !== null && text(frame.memberName, other.memberName))
    && ['startLine', 'startColumn', 'endLine', 'endColumn'].every(key => frame[key] === other[key])
    && frame.sourceLines.length === other.sourceLines.length
    && frame.sourceLines.every((line, index) => text(line, other.sourceLines[index]));
}

export function compareCollectionTransformCase(fixture, actual) {
  const operations = fixture.operations.filter(operation => operation.kind === 'evaluate');
  if (!Array.isArray(fixture.expected) || fixture.expected.length !== operations.length
      || !Array.isArray(fixture.expectedEffects)) throw Error('missing Java collection observations');
  const comparisons = fixture.expected.map((expected, index) => {
    if (expected.index !== index || expected.expression !== operations[index].value || !scopes.has(expected.scope)
        || !['success', 'error', 'oracle-error', 'api-error'].includes(expected.status)
        || hex(textUnits(expected.status + '\n')) !== expected.statusRawHex)
      throw Error('invalid expected API identity/status/scope');
    for (const channel of ['stdout', 'stderr'])
      encoded({units: expected[channel + 'Units'], javaUtf8Hex: expected[channel + 'Hex']});
    for (const key of ['readsBefore', 'readsAfter', 'logsBefore', 'logsAfter'])
      if (!integer(expected[key])) throw Error('invalid expected reader/logger count');
    const call = actual.calls[index];
    if (!call) return {index, scope: expected.scope, rawMatch: false, compatible: false, missing: true};
    const rawMatch = call.index === index && call.expression === expected.expression && call.status === expected.status
      && ['stdout', 'stderr'].every(channel => same(call[channel + 'Units'], expected[channel + 'Units'])
        && hex(call[channel + 'Units']) === expected[channel + 'Hex']);
    const countsMatch = ['readsBefore', 'readsAfter', 'logsBefore', 'logsAfter'].every(key => call[key] === expected[key]);
    const effects = fixture.expectedEffects.filter(effect => effect.callIndex === index);
    const observed = actual.effects.filter(effect => effect.callIndex === index);
    const effectsMatch = effects.length === observed.length && effects.every((effect, position) => compareEffect(effect, observed[position]));
    const ordinaryDisposition = call.status === 'success' && call.disposition === null
      || call.status === 'error' && call.disposition === 'language-error';
    return {index, scope: expected.scope, rawMatch, countsMatch, effectsMatch,
      compatible: expected.scope === 'ordinary' && ordinaryDisposition && rawMatch && countsMatch && effectsMatch};
  });
  const acquisitionIntegrityPassed = actual.id === fixture.id && actual.acquisitionError === null
    && actual.cleanupErrors.length === 0 && actual.calls.length === fixture.expected.length
    && actual.calls.every(call => call.status === 'success' ? call.disposition === null
      : call.status === 'error' ? dispositions.has(call.disposition)
      : call.status === 'api-error' && call.disposition === null)
    && actual.architecture.terminalSlots === 0 && actual.architecture.ownedResultsStable === true
    && actual.architecture.providerEntriesReleased === true;
  const orderedEffectsPassed = fixture.expectedEffects.length === actual.effects.length
    && fixture.expectedEffects.every((effect, index) => compareEffect(effect, actual.effects[index]));
  return {id: fixture.id, comparisons, acquisitionIntegrityPassed, orderedEffectsPassed,
    rawByteMatches: comparisons.filter(row => row.rawMatch).length,
    ordinaryCompatible: comparisons.filter(row => row.compatible).length,
    ordinaryCount: comparisons.filter(row => row.scope === 'ordinary').length,
    diagnosticOnlyRawMatches: comparisons.filter(row => row.scope === 'intent-not-reached' && row.rawMatch).length,
    publicApiBoundaryRawMatches: comparisons.filter(row => row.scope === 'public-api-boundary' && row.rawMatch).length,
    fullJavaEventsCountersPassed: false,
    fullCompatibilityPassed: false};
}
