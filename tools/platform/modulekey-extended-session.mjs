// Authored Host inputs for the extended ModuleKey controls. Evaluated modules,
// source wrappers, URI semantics and caches remain owned by the shared engine.
import {completeDefaultModuleRequest, validateModuleRequest} from './module-policy-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

const equal = (a, b) => Array.isArray(a) && Array.isArray(b)
  && a.length === b.length && a.every((value, index) => value === b[index]);
const hex = bytes => bytes.map(byte => byte.toString(16).padStart(2, '0')).join('');
const stringUnits = text => Array.from({length: text.length}, (_, i) => text.charCodeAt(i));
const own = (object, key) => object != null && Object.hasOwn(object, key);
const dispositions = new Set(['language-error', 'unsupported', 'limit-exceeded', 'internal-failure']);
const classified = call => call?.status === 'success' ? call.disposition === null
  : call?.status === 'error' && dispositions.has(call.disposition);
const list = value => { if (!Array.isArray(value)) throw Error('missing configured Host table'); return value; };
function unique(entries, keyOf, validate) {
  const map = new Map();
  for (const entry of list(entries)) {
    if (!validate(entry)) throw Error('invalid configured Host entry');
    const key = keyOf(entry);
    if (map.has(key)) throw Error('duplicate configured Host entry');
    map.set(key, entry);
  }
  return map;
}
const tuple = (...parts) => JSON.stringify(parts);
const strings = (entry, fields) => entry && fields.every(name => typeof entry[name] === 'string');
function moduleTable(entries) {
  return unique(entries, entry => entry.requestedUri, entry => strings(entry,
    ['requestedUri', 'selectedUri', 'resolvedUri', 'sourceUri', 'sourceId', 'inferredName'])
    && ['host', 'synthetic'].includes(entry.keyKind)
    && ['true', 'false', 'dynamic'].includes(entry.cacheMode)
    && ['fixed', 'state-at-load', 'state-at-factory'].includes(entry.sourceMode)
    && typeof entry.fragmentPaths === 'boolean');
}
function rawMessage(entry) {
  const message = entry.message;
  if (message === null) {
    if (entry.messageNull !== true || !equal(entry.messageUnits, []) || entry.messageJavaUtf8Hex !== '')
      throw Error('invalid null Host cause');
  } else if (typeof message !== 'string' || entry.messageNull === true
    || !equal(stringUnits(message), entry.messageUnits)
    || hex(javaUtf8Bytes(entry.messageUnits)) !== entry.messageJavaUtf8Hex) {
    throw Error('Host cause bytes differ from configured text');
  }
  return message;
}

export function runModuleKeyExtendedCase(api, fixture, profile, budget = 7) {
  let session, callIndex = 0, polls = 0, currentSource = null;
  let cached, customFactory, sourceState, policy, lastRequestId = null;
  const keys = new Map(), internalKeys = new Map(), resolvedKeys = new Map();
  const selectedIds = new Set(), resolvedIds = new Set(), wrapperUris = new Set();
  const requestIds = new Set(), tokens = new Set(), retained = [];
  const result = {id: fixture.id, repeat: fixture.repeat, session: fixture.session, profile: 'public',
    calls: [], requests: [], hostEvents: [], observations: [], constructorError: null, acquisitionError: null,
    observationScope: 'Actual public API operations, delivered module requests, Host completions, explicit configuration/GC operations and close. Automatic per-poll GC is counted separately. These observations are not fabricated Java callbacks or counters.',
    architecture: {collections: 0, terminalSlots: null, ownedResultsStable: true, releasedHostKeys: false}};
  const observe = (kind, fields = {}) => result.observations.push({
    sequence: result.observations.length, callIndex, kind, ...fields,
  });
  const invoke = (method, args) => {
    observe('host-call', {method, args});
    try {
      const value = session[method](...args);
      observe('host-return', {method, returnedUndefined: value === undefined, value: value === undefined ? null : value});
      return value;
    } catch (error) {
      observe('host-throw', {method, message: String(error.message ?? error)}); throw error;
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
    if (profile?.id !== fixture.providerProfileId || !Array.isArray(fixture.operations)
      || !Number.isSafeInteger(budget) || budget <= 0
      || typeof profile.initial?.cached !== 'boolean' || typeof profile.initial.customFactory !== 'boolean'
      || typeof profile.initial.sourceState !== 'string' || typeof profile.initial.policy !== 'string'
      || !profile.sources || typeof profile.sources !== 'object' || Array.isArray(profile.sources)
      || !profile.sourceProfiles || typeof profile.sourceProfiles !== 'object' || Array.isArray(profile.sourceProfiles))
      throw Error('unsupported extended ModuleKey acquisition profile');
    ({cached, customFactory, sourceState, policy} = profile.initial);
    const custom = moduleTable(profile.customModules), files = moduleTable(profile.fileModules);
    const overrides = unique(profile.uriOverrides, e => tuple(e.selectedUri, e.authoredUri),
      e => strings(e, ['selectedUri', 'authoredUri', 'absoluteUri']));
    const capabilities = unique(profile.capabilities, e => e.selectedUri, e => strings(e, ['selectedUri'])
      && ['local', 'hierarchical', 'fragmentPaths'].every(k => typeof e[k] === 'boolean'));
    const elements = unique(profile.elements, e => tuple(e.selectedUri, e.candidateUri),
      e => strings(e, ['selectedUri', 'candidateUri']) && typeof e.exists === 'boolean');
    const elementDefaults = unique(profile.elementDefaults, e => e.selectedUri,
      e => strings(e, ['selectedUri']) && typeof e.exists === 'boolean');
    const policyRules = list(profile.policyRules);
    for (const rule of policyRules) {
      if (!strings(rule, ['mode']) || rule.kind !== 'module' || !rule.match
        || Object.keys(rule.match).length !== 1
        || !['prefix', 'equals'].some(k => typeof rule.match[k] === 'string'))
        throw Error('invalid configured resolve policy');
    }
    const validPolicy = value => value === 'allow' || policyRules.some(rule => rule.mode === value);
    if (!validPolicy(policy)) throw Error('unknown initial policy');
    const causes = unique(profile.rawCauses, e => tuple(e.mode, e.kind, e.context),
      e => strings(e, ['mode', 'kind', 'context', 'policyCallKind', 'contextSource']));
    for (const cause of causes.values()) rawMessage(cause);
    const factoryFailures = unique(profile.factoryFailures ?? [], e => e.scheme,
      e => strings(e, ['scheme', 'message']) && /^[A-Za-z][A-Za-z0-9+.-]*$/.test(e.scheme) && e.errorKind === 'io');
    const noLoaderUris = new Set(list(profile.noLoaderUris ?? []));
    if ([...noLoaderUris].some(uri => typeof uri !== 'string')) throw Error('invalid configured no-loader URI');
    const source = id => {
      if (!own(profile.sources, id)) throw Error('unconfigured source state: ' + id);
      const item = profile.sources[id];
      if (item?.kind === 'text' && typeof item.text === 'string') return item;
      if (item?.kind === 'io') { rawMessage(item); return item; }
      throw Error('invalid configured source');
    };
    const sourceProfile = id => {
      if (!own(profile.sourceProfiles, id)) throw Error('unconfigured ModuleSource profile');
      const item = profile.sourceProfiles[id];
      if (typeof item?.uri !== 'string' || !['uri', 'contents', 'repl'].includes(item.kind)
        || (item.kind === 'uri' && item.contents !== null)
        || (item.kind === 'repl' && item.uri !== 'repl:text')) throw Error('invalid ModuleSource profile');
      if (item.kind === 'uri') return {profile: item, contents: null};
      const body = source(item.sourceId);
      if (body.kind !== 'text') throw Error('ModuleSource contents must be authored text');
      return {profile: item, contents: body.text};
    };
    const expressions = fixture.operations.filter(op => op.kind === 'evaluate').map(op => op.expression);
    if (!equal(expressions, fixture.expressions) || expressions.some(value => typeof value !== 'string'))
      throw Error('operation/expression order differs');
    // Initial state need not name a source until used: Java06 first sets one.
    try {
      observe('constructor-call', {method: 'moduleEvaluator'});
      session = api.SourceSession.moduleEvaluator('compact', false, false, true);
      observe('constructor-return');
    } catch (error) {
      result.constructorError = {message: String(error.message ?? error)};
      observe('constructor-throw', result.constructorError); return result;
    }
    function owned(request) {
      const key = keys.get(request.keyId);
      if (key) {
        if (request.keyKind !== key.plan.keyKind || request.keyHandle !== key.handle)
          throw Error('ModuleKey descriptor ownership mismatch');
        if (['query-cache-policy', 'query-fragment-paths', 'resolve-import-uri', 'resolve-key', 'load-source']
          .includes(request.kind) && request.originalUri !== key.plan.selectedUri)
          throw Error('ModuleKey selected URI ownership mismatch');
        return key;
      }
      let internal = internalKeys.get(request.keyId);
      if (!internal && request.keyHandle === '' && !selectedIds.has(request.keyId)) {
        const synthetic = request.keyKind === 'synthetic' && request.originalUri === 'repl:text'
          && request.kind === 'resolve-key'
          && (request.resolutionPhase === 'query' || currentSource?.profile.kind === 'repl');
        const wrapper = request.keyKind === 'supplied-text' && wrapperUris.has(request.originalUri)
          && ['query-fragment-paths', 'resolve-import-uri'].includes(request.kind);
        if (synthetic || wrapper) {
          internal = {kind: request.keyKind, uri: request.originalUri, handle: ''};
          internalKeys.set(request.keyId, internal); selectedIds.add(request.keyId);
        }
      }
      if (!internal || internal.kind !== request.keyKind || internal.handle !== request.keyHandle
        || (['query-cache-policy', 'query-fragment-paths', 'resolve-import-uri', 'resolve-key', 'load-source']
          .includes(request.kind) && internal.uri !== request.originalUri))
        throw Error('unregistered or foreign internal ModuleKey descriptor');
      return null;
    }
    function denyResolve(event, uri) {
      const denied = policyRules.some(rule => rule.mode === policy
        && (own(rule.match, 'equals') ? uri === rule.match.equals : uri.startsWith(rule.match.prefix)));
      if (!denied) return false;
      const cause = causes.get(tuple(policy, 'module', uri));
      if (!cause || cause.policyCallKind !== 'checkResolveModule')
        throw Error('missing raw resolve policy cause for actual context');
      invoke('rejectModuleResolution', [event.token, 'security-manager-exception', rawMessage(cause), 0]);
      result.hostEvents.push({kind: 'policy-denial', mode: policy, context: uri, keyId: event.module.keyId});
      return true;
    }
    function module(event) {
      result.requests.push({callIndex, token: event.token, module: {...event.module}});
      observe('module-request', {token: event.token, module: {...event.module}});
      const request = validateModuleRequest(event);
      if (requestIds.has(request.id) || tokens.has(event.token)
        || (lastRequestId != null && (request.id.length < lastRequestId.length
          || (request.id.length === lastRequestId.length && request.id <= lastRequestId))))
        throw Error('reused or out-of-order module request identity');
      requestIds.add(request.id); tokens.add(event.token); lastRequestId = request.id;
      if (request.kind === 'select-key') {
        if (selectedIds.has(request.keyId)) throw Error('reused ModuleKey identity');
        selectedIds.add(request.keyId);
        // The authored throwing hook precedes the custom factory toggle.
        const colon = request.originalUri.indexOf(':');
        const failure = colon < 0 ? null : factoryFailures.get(request.originalUri.slice(0, colon));
        if (failure) {
          invoke('rejectModuleKey', [event.token, false, 'io', failure.message, 0]);
          result.hostEvents.push({kind: 'factory-failure', keyId: request.keyId, requestedUri: request.originalUri}); return;
        }
        const plan = (customFactory ? custom.get(request.originalUri) : null) ?? files.get(request.originalUri);
        if (!plan) {
          if (defaultRequest(event)) {
            if (!request.originalUri.startsWith('pkl:')) throw Error('unconfigured successful default factory');
            internalKeys.set(request.keyId, {kind: 'standard-library', uri: request.originalUri, handle: ''});
            if (request.selectionPhase === 'primary' && currentSource?.contents != null) wrapperUris.add(request.originalUri);
          } else {
            if (!noLoaderUris.has(request.originalUri)) throw Error('unconfigured factory URI');
            invoke('rejectModuleKey', [event.token, false, 'no-loader', null, 0]);
          }
          return;
        }
        const snapshot = plan.cacheMode === 'dynamic' ? cached : plan.cacheMode === 'true';
        const sourceId = plan.sourceMode === 'state-at-factory' ? sourceState : plan.sourceId;
        if (plan.sourceMode === 'state-at-factory') source(sourceId);
        const handle = `configured-key:${request.keyId}`;
        invoke('completeModuleKey', [event.token, request.keyId, plan.selectedUri, plan.keyKind, handle, 0]);
        keys.set(request.keyId, {plan, handle, cached: snapshot, sourceId});
        if (request.selectionPhase === 'primary' && currentSource?.contents != null) wrapperUris.add(plan.selectedUri);
        result.hostEvents.push({kind: 'factory-key', keyId: request.keyId, keyKind: plan.keyKind,
          requestedUri: request.originalUri, selectedUri: plan.selectedUri, cachedSnapshot: snapshot,
          sourceSnapshot: plan.sourceMode === 'state-at-factory' ? sourceId : null}); return;
      }
      if (request.kind === 'check-import') {
        // These allowlist modes change checkResolveModule, not checkImportModule.
        if (!defaultRequest(event)) throw Error('unsupported shared import policy'); return;
      }
      const key = owned(request);
      if (request.kind === 'query-cache-policy' || request.kind === 'query-fragment-paths') {
        if (!key) {
          if (!defaultRequest(event)) throw Error('unsupported internal key getter'); return;
        }
        const cache = request.kind === 'query-cache-policy';
        invoke(cache ? 'completeModuleCachePolicy' : 'completeModuleFragmentPaths',
          [event.token, request.keyId, cache ? key.cached : key.plan.fragmentPaths, 0]); return;
      }
      if (request.kind === 'resolve' || request.kind === 'resolve-key') {
        if (typeof request.resolvedKeyId !== 'string' || resolvedIds.has(request.resolvedKeyId))
          throw Error('missing or reused resolved-key identity');
        resolvedIds.add(request.resolvedKeyId);
        const selectedUri = key ? key.plan.selectedUri : internalKeys.get(request.keyId).uri;
        if (request.keyKind !== 'supplied-text' && denyResolve(event, selectedUri)) return;
        if (!key) {
          if (!defaultRequest(event)) throw Error('unsupported internal key resolution'); return;
        }
        const handle = `configured-resolved:${request.resolvedKeyId}`;
        invoke('completeModuleResolvedKey', [event.token, request.keyId, request.resolvedKeyId, handle,
          key.plan.selectedUri, key.plan.resolvedUri, key.plan.sourceUri, 0]);
        resolvedKeys.set(request.resolvedKeyId, {keyId: request.keyId, key, handle});
        result.hostEvents.push({kind: 'key-resolve', keyId: request.keyId, resolvedKeyId: request.resolvedKeyId}); return;
      }
      if (request.kind === 'load-source') {
        const resolved = resolvedKeys.get(request.resolvedKeyId);
        if (!key || !resolved || resolved.keyId !== request.keyId || resolved.key !== key
          || resolved.handle !== request.resolvedKeyHandle || key.plan.resolvedUri !== request.resolvedUri
          || key.plan.sourceUri !== request.sourceUri) throw Error('resolved-key descriptor ownership mismatch');
        const sourceId = key.plan.sourceMode === 'state-at-load' ? sourceState : key.sourceId;
        const body = source(sourceId);
        result.hostEvents.push({kind: 'key-source', keyId: request.keyId, resolvedKeyId: request.resolvedKeyId, sourceId});
        if (body.kind === 'io') invoke('rejectModuleSource', [event.token, 'io', rawMessage(body), 0]);
        else invoke('completeModuleSource', [event.token, body.text, key.plan.inferredName, 0]); return;
      }
      if (request.kind === 'resolve-import-uri') {
        // SuppliedText inherits resolveUri; it never invokes the delegate override.
        const override = key && overrides.get(tuple(key.plan.selectedUri, request.importUri));
        if (override) invoke('completeImportUri', [event.token, request.keyId, override.absoluteUri, 0]);
        else if (!defaultRequest(event)) throw Error('unsupported inherited import URI preparation'); return;
      }
      if (request.kind === 'capabilities') {
        const entry = key && capabilities.get(key.plan.selectedUri);
        if (!entry) throw Error('unconfigured selected-key capabilities');
        invoke('completeModuleCapabilities', [event.token, entry.local, entry.hierarchical, entry.fragmentPaths, 0]); return;
      }
      if (request.kind === 'has-element') {
        const entry = key && (elements.get(tuple(key.plan.selectedUri, request.originalUri))
          ?? elementDefaults.get(key.plan.selectedUri));
        if (!entry) throw Error('unconfigured selected-key element membership');
        invoke('completeModuleHasElement', [event.token, entry.exists, 0]); return;
      }
      throw Error('unsupported ModuleKey Host phase');
    }
    for (const operation of fixture.operations) {
      if (operation.kind === 'gc') {
        observe('explicit-gc-call'); session.collect(); result.architecture.collections++;
        observe('explicit-gc-return'); result.hostEvents.push({kind: 'explicit-collect'}); continue;
      }
      if (operation.kind === 'cache') {
        if (!['on', 'off'].includes(operation.value)) throw Error('invalid cache configuration');
        cached = operation.value === 'on'; observe('cache-config', {cached});
        result.hostEvents.push({kind: 'cache-config', cached}); continue;
      }
      if (operation.kind === 'factory') {
        if (!['custom', 'builtin'].includes(operation.value)) throw Error('invalid factory configuration');
        customFactory = operation.value === 'custom'; observe('factory-config', {customFactory});
        result.hostEvents.push({kind: 'factory-config', customFactory}); continue;
      }
      if (operation.kind === 'source') {
        source(operation.value); sourceState = operation.value;
        observe('source-config', {state: sourceState}); result.hostEvents.push({kind: 'source-config', state: sourceState}); continue;
      }
      if (operation.kind === 'policy') {
        if (!validPolicy(operation.value)) throw Error('unconfigured policy mode');
        policy = operation.value; observe('policy-config', {mode: policy});
        result.hostEvents.push({kind: 'policy-config', mode: policy}); continue;
      }
      if (operation.kind !== 'evaluate') throw Error('unknown ModuleKey operation');
      currentSource = sourceProfile(operation.sourceProfileId);
      const args = [currentSource.profile.uri, currentSource.contents, operation.expression, 0];
      observe('api-call', {method: 'evaluateModuleSource', args});
      session.evaluateModuleSource(...args); observe('api-started', {method: 'evaluateModuleSource'});
      while (true) {
        if (++polls > 1_000_000) throw Error('ModuleKey poll bound exceeded');
        const event = session.poll(0, budget); session.collect(); result.architecture.collections++;
        if (event.kind === 'yield') continue;
        if (event.kind === 'module-request') { module(event); continue; }
        if (!['value', 'error'].includes(event.kind)) throw Error('unexpected ModuleKey session event: ' + event.kind);
        const value = units(event.kind === 'value' ? event.value : event.error);
        const status = event.kind === 'value' ? 'success' : 'error';
        const call = {index: callIndex, sourceProfileId: operation.sourceProfileId, status,
          statusRawHex: hex(javaUtf8Bytes(stringUnits(status + '\n'))),
          stdoutUnits: event.kind === 'value' ? value : [], stderrUnits: event.kind === 'error' ? value : [],
          phase: event.phase ?? null, disposition: event.disposition ?? null, reason: event.reason ?? null};
        result.calls.push(call);
        observe('api-result', {call: {...call, stdoutUnits: [...call.stdoutUnits], stderrUnits: [...call.stderrUnits]}});
        callIndex++;
        // Preserve the delivered raw result before rejecting malformed metadata.
        if (!classified(call)) throw Error('invalid result failure disposition');
        break;
      }
    }
  } catch (error) {
    result.acquisitionError = {message: String(error.message ?? error), stack: error.stack ?? null};
    observe('acquisition-error', {message: result.acquisitionError.message});
  } finally {
    if (session) {
      try {
        observe('close-call'); session.close(); session.collect(); result.architecture.terminalSlots = session.heapSlots();
        observe('close-return', {terminalSlots: result.architecture.terminalSlots});
      } catch (error) {
        result.acquisitionError ??= {message: String(error.message ?? error), stack: error.stack ?? null};
        observe('close-throw', {message: String(error.message ?? error)});
      } finally {
        try { if (typeof session.free === 'function') session.free(); }
        catch (error) {
          result.acquisitionError ??= {message: String(error.message ?? error), stack: error.stack ?? null};
          observe('free-throw', {message: String(error.message ?? error)});
        }
        try { result.architecture.ownedResultsStable = retained.every(({view, copy}) => equal(Array.from(view), copy)); }
        catch { result.architecture.ownedResultsStable = false; }
      }
    }
    keys.clear(); internalKeys.clear(); resolvedKeys.clear(); selectedIds.clear(); resolvedIds.clear(); wrapperUris.clear();
    result.architecture.releasedHostKeys = true;
  }
  return result;
}

export function compareModuleKeyExtendedCase(fixture, actual) {
  const rawFailures = [], ordinaryFailures = [], classificationFailures = [], cases = [];
  if (actual.calls.length !== fixture.expected.length) rawFailures.push('call-count');
  let intendedOrdinaryMatched = 0, diagnosticOnlyMatched = 0, internalBoundaryRawMatched = 0;
  for (const [index, expected] of fixture.expected.entries()) {
    const call = actual.calls[index];
    const matched = !!call && call.index === index && call.sourceProfileId === expected.sourceProfileId
      && call.status === expected.status && call.statusRawHex === expected.statusRawHex
      && equal(call.stdoutUnits, expected.stdoutUnits) && equal(call.stderrUnits, expected.stderrUnits)
      && hex(javaUtf8Bytes(call.stdoutUnits)) === expected.stdoutHex
      && hex(javaUtf8Bytes(call.stderrUnits)) === expected.stderrHex;
    if (!matched) rawFailures.push(`raw-api-${index}`);
    const ordinary = expected.scope === 'ordinary' && expected.errorClass !== 'org.pkl.core.PklBugException';
    if (call && !classified(call)) classificationFailures.push(`classification-${index}`);
    const supported = classified(call) && (call.status === 'success' || call.disposition === 'language-error');
    if (ordinary) {
      if (matched && supported) intendedOrdinaryMatched++; else ordinaryFailures.push(`ordinary-api-${index}`);
    } else if (expected.scope === 'intent-not-reached-diagnostic' && matched) diagnosticOnlyMatched++;
    else if (expected.scope === 'internal-boundary' && matched) internalBoundaryRawMatched++;
    cases.push({index, scope: expected.scope, rawMatched: matched, ordinaryCompatible: ordinary && matched && !!supported,
      oracleErrorClass: expected.errorClass, oracleMessageNull: expected.messageNull,
      disposition: call?.disposition ?? null});
  }
  const cleanupPassed = actual.architecture.terminalSlots === 0 && actual.architecture.ownedResultsStable
    && actual.architecture.releasedHostKeys;
  const acquisitionIntegrityPassed = !actual.constructorError && !actual.acquisitionError
    && actual.calls.length === fixture.expected.length && cleanupPassed && classificationFailures.length === 0;
  return {rawApiPassed: rawFailures.length === 0, rawFailures, ordinaryFailures, classificationFailures, cases,
    intendedOrdinaryMatched, diagnosticOnlyMatched, internalBoundaryRawMatched,
    ordinaryCompatibilityPassed: acquisitionIntegrityPassed && ordinaryFailures.length === 0,
    cleanupPassed, acquisitionIntegrityPassed, fullLifecyclePassed: false,
    fullLifecycleReason: 'Full ordered Java event/counter equality is not established by these public API results. Actual Host observations and original Java events/counters are retained separately.'};
}
