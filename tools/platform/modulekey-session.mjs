// Bounded SPI adapter with separate supplied-primary and public profiles. The engine owns URI/site caches and Pkl
// evaluation. These tables hold configured Host keys, not evaluated modules.
import {completeDefaultModuleRequest, validateModuleRequest} from './module-policy-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

const equal = (a, b) => Array.isArray(a) && Array.isArray(b)
  && a.length === b.length && a.every((value, index) => value === b[index]);
const hex = bytes => bytes.map(byte => byte.toString(16).padStart(2, '0')).join('');
function table(entries) {
  if (!Array.isArray(entries)) throw Error('missing configured ModuleKey table');
  const result = new Map();
  for (const entry of entries) {
    if (['requestedUri', 'selectedUri', 'resolvedUri', 'sourceUri', 'sourceText', 'inferredName']
      .some(name => typeof entry?.[name] !== 'string') || typeof entry.fragmentPaths !== 'boolean')
      throw Error('incomplete configured ModuleKey');
    if (result.has(entry.requestedUri)) throw Error('duplicate configured ModuleKey URI');
    if (entry.cacheMode != null && !['true', 'false', 'dynamic'].includes(entry.cacheMode))
      throw Error('unknown factory cache snapshot profile');
    if (entry.cacheMode == null && typeof entry.isCached !== 'boolean')
      throw Error('missing factory cache snapshot profile');
    result.set(entry.requestedUri, entry);
  }
  return result;
}

export function runModuleKeyCase(api, fixture, profile, budget = 7, {entrypoint = 'provided-primary'} = {}) {
  let session, polls = 0, cached = true, callIndex = 0;
  const keys = new Map(), resolvedKeys = new Map(), retained = [];
  const requestIds = new Set(), tokens = new Set();
  let lastRequestId = null;
  const result = {id: fixture.id, repeat: fixture.repeat, session: fixture.session,
    profile: entrypoint, calls: [], requests: [], hostEvents: [], observations: [],
    observationScope: 'Actual API operations, delivered module requests, completion calls and returns, explicit fixture GC/cache operations, and close. Automatic per-poll collections are counted separately; this is not the Java event/counter contract.',
    constructorError: null, acquisitionError: null,
    architecture: {collections: 0, terminalSlots: null, ownedResultsStable: true, releasedHostKeys: false}};
  const observe = (kind, fields = {}) => result.observations.push({
    sequence: result.observations.length, callIndex, kind, ...fields,
  });
  const invoke = (method, args) => {
    observe('host-call', {method, args});
    try {
      const returned = session[method](...args);
      observe('host-return', {method, returnedUndefined: returned === undefined,
        value: returned === undefined ? null : returned});
      return returned;
    } catch (error) {
      observe('host-throw', {method, message: String(error.message ?? error)});
      throw error;
    }
  };
  const defaultRequest = event => completeDefaultModuleRequest({
    completeDefaultModule: (...args) => invoke('completeDefaultModule', args),
  }, event);
  const units = view => {
    if (!(view instanceof Uint16Array)) throw Error('expected owned UTF16 payload');
    const copy = Array.from(view); retained.push({view, copy}); return copy;
  };
  try {
    if (!['provided-primary', 'public'].includes(entrypoint) || !['spi132', 'factory80'].includes(fixture.cohort)
      || profile?.id !== fixture.id || fixture.initialPolicy !== 'allow'
      || !Array.isArray(fixture.operations) || !Number.isSafeInteger(budget) || budget <= 0)
      throw Error('unsupported ModuleKey acquisition profile');
    const custom = table(profile.customModules), files = table(profile.fileModules);
    if (profile.noLoaderUris != null && (!Array.isArray(profile.noLoaderUris)
      || profile.noLoaderUris.some(uri => typeof uri !== 'string' || !/^[A-Za-z][A-Za-z0-9+.-]*:/.test(uri))
      || new Set(profile.noLoaderUris).size !== profile.noLoaderUris.length))
      throw Error('invalid configured no-loader URI registry');
    const noLoaderUris = new Set(profile.noLoaderUris ?? []);
    const factoryFailures = new Map();
    if (profile.factoryFailures != null && !Array.isArray(profile.factoryFailures))
      throw Error('invalid configured factory failures');
    for (const rule of profile.factoryFailures ?? []) {
      if (!rule || typeof rule.scheme !== 'string' || !/^[A-Za-z][A-Za-z0-9+.-]*$/.test(rule.scheme)
        || rule.errorKind !== 'io' || typeof rule.message !== 'string' || factoryFailures.has(rule.scheme))
        throw Error('invalid configured factory failure rule');
      factoryFailures.set(rule.scheme, rule);
    }
    const expressions = fixture.operations.filter(operation => operation.kind === 'evaluate')
      .map(operation => operation.expression);
    if (!equal(expressions, fixture.expressions)) throw Error('operation/expression order differs');
    try {
      observe('constructor-call', {method: entrypoint === 'public' ? 'moduleEvaluator' : 'forExpressions'});
      session = entrypoint === 'public'
        ? api.SourceSession.moduleEvaluator('compact', false, false, true)
        : api.SourceSession.forExpressions(fixture.source, fixture.uri, fixture.moduleName,
          expressions, 'compact', false, false, true);
      observe('constructor-return');
    } catch (error) {
      observe('constructor-throw', {message: String(error.message ?? error)});
      result.constructorError = {message: String(error.message ?? error)}; return result;
    }
    const owned = request => {
      const key = keys.get(request.keyId);
      if (!key || request.keyKind !== 'host' || request.keyHandle !== key.handle
        || request.originalUri !== key.plan.selectedUri) throw Error('ModuleKey descriptor ownership mismatch');
      return key;
    };
    function module(event) {
      result.requests.push({callIndex, token: event.token, module: {...event.module}});
      observe('module-request', {token: event.token, module: {...event.module}});
      const request = validateModuleRequest(event);
      if (requestIds.has(request.id) || tokens.has(event.token)
        || (lastRequestId != null && (request.id.length < lastRequestId.length
          || (request.id.length === lastRequestId.length && request.id <= lastRequestId))))
        throw Error('reused or out-of-order module request identity');
      requestIds.add(request.id); tokens.add(event.token); lastRequestId = request.id;
      if (request.keyKind === 'host' && !keys.has(request.keyId))
        throw Error('unregistered Host ModuleKey descriptor');
      if (request.kind === 'select-key') {
        // Match the configured factory's case-sensitive URI scheme predicate.
        // A URL parser would lowercase schemes and change the authored Host rule.
        const colon = request.originalUri.indexOf(':');
        const failure = colon < 0 ? null : factoryFailures.get(request.originalUri.slice(0, colon));
        if (failure) {
          invoke('rejectModuleKey', [event.token, false, failure.errorKind, failure.message, 0]);
          result.hostEvents.push({kind: 'factory-failure', keyId: request.keyId,
            requestedUri: request.originalUri, errorKind: failure.errorKind, message: failure.message});
          return;
        }
        const plan = custom.get(request.originalUri) ?? files.get(request.originalUri);
        if (!plan) {
          if (!defaultRequest(event)) {
            if (!noLoaderUris.has(request.originalUri)) throw Error('unconfigured factory URI');
            invoke('rejectModuleKey', [event.token, false, 'no-loader', null, 0]);
            result.hostEvents.push({kind: 'factory-miss', keyId: request.keyId, requestedUri: request.originalUri});
          }
          return;
        }
        if (keys.has(request.keyId)) throw Error('reused ModuleKey identity');
        const snapshot = plan.cacheMode === 'dynamic' ? cached
          : plan.cacheMode != null ? plan.cacheMode === 'true' : plan.isCached;
        const handle = `configured-key:${request.keyId}`;
        invoke('completeModuleKey', [event.token, request.keyId, plan.selectedUri, 'host', handle, 0]);
        keys.set(request.keyId, {plan, handle, cached: snapshot});
        result.hostEvents.push({kind: 'factory-key', keyId: request.keyId,
          requestedUri: request.originalUri, selectedUri: plan.selectedUri, cachedSnapshot: snapshot});
      } else if (request.kind === 'query-cache-policy' || request.kind === 'query-fragment-paths') {
        if (!keys.has(request.keyId)) {
          if (!defaultRequest(event)) throw Error('unconfigured selected key getter');
          return;
        }
        const key = owned(request);
        result.hostEvents.push({kind: request.kind, keyId: request.keyId});
        if (request.kind === 'query-cache-policy') invoke('completeModuleCachePolicy', [event.token, request.keyId, key.cached, 0]);
        else invoke('completeModuleFragmentPaths', [event.token, request.keyId, key.plan.fragmentPaths, 0]);
      } else if (request.kind === 'resolve' || request.kind === 'resolve-key') {
        if (!keys.has(request.keyId)) {
          if (!defaultRequest(event)) throw Error('unconfigured key resolution');
          return;
        }
        const key = owned(request), plan = key.plan;
        if (resolvedKeys.has(request.resolvedKeyId)) throw Error('reused resolved-key identity');
        const handle = `configured-resolved:${request.resolvedKeyId}`;
        invoke('completeModuleResolvedKey', [event.token, request.keyId, request.resolvedKeyId, handle,
          plan.selectedUri, plan.resolvedUri, plan.sourceUri, 0]);
        resolvedKeys.set(request.resolvedKeyId, {keyId: request.keyId, key, handle});
        result.hostEvents.push({kind: 'key-resolve', keyId: request.keyId, resolvedKeyId: request.resolvedKeyId});
      } else if (request.kind === 'load-source') {
        const resolved = resolvedKeys.get(request.resolvedKeyId), key = owned(request);
        if (!resolved || resolved.keyId !== request.keyId || resolved.key !== key
          || resolved.handle !== request.resolvedKeyHandle || request.resolvedUri !== key.plan.resolvedUri
          || request.sourceUri !== key.plan.sourceUri) throw Error('resolved-key descriptor ownership mismatch');
        result.hostEvents.push({kind: 'key-source', keyId: request.keyId, resolvedKeyId: request.resolvedKeyId});
        invoke('completeModuleSource', [event.token, key.plan.sourceText, key.plan.inferredName, 0]);
      } else if (request.kind === 'resolve-import-uri') {
        // This fixture's keys inherit ModuleKey.resolveUri. Custom overrides
        // require separate configured inputs; URI semantics stay in Rust.
        if (keys.has(request.keyId)) owned(request);
        if (!defaultRequest(event)) throw Error('unsupported inherited import URI preparation');
      } else if (request.kind === 'check-import') {
        if (!defaultRequest(event)) throw Error('unsupported shared import policy');
      } else throw Error('unsupported ModuleKey Host phase');
    }
    for (const operation of fixture.operations) {
      if (operation.kind === 'gc') {
        observe('explicit-gc-call');
        session.collect(); result.architecture.collections++;
        observe('explicit-gc-return');
        result.hostEvents.push({kind: 'explicit-collect'}); continue;
      }
      if (operation.kind === 'cache') {
        if (!['on', 'off'].includes(operation.value)) throw Error('invalid cache configuration operation');
        cached = operation.value === 'on'; result.hostEvents.push({kind: 'cache-config', cached});
        observe('cache-config', {cached}); continue;
      }
      if (operation.kind !== 'evaluate' || operation.source !== 'main.pkl')
        throw Error('unsupported ModuleSource operation in supplied-primary profile');
      const method = entrypoint === 'public' ? 'evaluateModuleSource' : 'startPath';
      const args = entrypoint === 'public' ? [fixture.uri, null, operation.expression, 0] : [callIndex, 0];
      observe('api-call', {method, args});
      session[method](...args);
      observe('api-started', {method});
      while (true) {
        if (++polls > 1_000_000) throw Error('ModuleKey poll bound exceeded');
        const event = session.poll(0, budget); session.collect(); result.architecture.collections++;
        if (event.kind === 'yield') continue;
        if (event.kind === 'module-request') { module(event); continue; }
        if (!['value', 'error'].includes(event.kind)) throw Error('unexpected ModuleKey session event: ' + event.kind);
        const value = units(event.kind === 'value' ? event.value : event.error);
        result.calls.push({index: callIndex, status: event.kind === 'value' ? 'success' : 'error',
          stdoutUnits: event.kind === 'value' ? value : [], stderrUnits: event.kind === 'error' ? value : [],
          phase: event.phase ?? null, disposition: event.disposition ?? null, reason: event.reason ?? null});
        const call = result.calls.at(-1);
        observe('api-result', {call: {...call, stdoutUnits: [...call.stdoutUnits], stderrUnits: [...call.stderrUnits]}});
        callIndex++; break;
      }
    }
  } catch (error) {
    result.acquisitionError = {message: String(error.message ?? error), stack: error.stack ?? null};
    observe('acquisition-error', {message: result.acquisitionError.message});
  }
  finally {
    if (session) try {
      observe('close-call');
      session.close(); session.collect(); result.architecture.terminalSlots = session.heapSlots();
      if (typeof session.free === 'function') session.free();
      result.architecture.ownedResultsStable = retained.every(({view, copy}) => equal(Array.from(view), copy));
      observe('close-return', {terminalSlots: result.architecture.terminalSlots});
    } catch (error) {
      result.acquisitionError = {message: String(error.message ?? error), stack: error.stack ?? null};
      observe('close-throw', {message: result.acquisitionError.message});
    }
    keys.clear(); resolvedKeys.clear(); result.architecture.releasedHostKeys = true;
  }
  return result;
}

export function compareModuleKeyCase(fixture, actual) {
  const failures = [];
  if (actual.constructorError || actual.acquisitionError) failures.push('acquisition');
  if (actual.calls.length !== fixture.expected.length) failures.push('call-count');
  for (const [index, expected] of fixture.expected.entries()) {
    const call = actual.calls[index];
    if (!call || call.status !== expected.status || !equal(call.stdoutUnits, expected.stdoutUnits)
      || !equal(call.stderrUnits, expected.stderrUnits)
      || hex(javaUtf8Bytes(call.stdoutUnits)) !== expected.stdoutHex
      || hex(javaUtf8Bytes(call.stderrUnits)) !== expected.stderrHex) failures.push(`raw-api-${index}`);
    if (call && ['unsupported', 'internal-failure'].includes(call.disposition)) failures.push(`disposition-${index}`);
    if (expected.errorClass === 'org.pkl.core.PklBugException') failures.push(`oracle-error-${index}`);
  }
  if (actual.architecture.terminalSlots !== 0 || !actual.architecture.ownedResultsStable
    || !actual.architecture.releasedHostKeys) failures.push('lifecycle-cleanup');
  return {rawApiPassed: failures.length === 0, failures, fullLifecyclePassed: false,
    fullLifecycleReason: actual.profile === 'public'
      ? 'Full ordered Java event/counter equality has not been compared; actual public-entrypoint requests are retained.'
      : 'Actual primary/query lifecycle and full Java event/counter comparison are not executed by this supplied-primary profile.'};
}
