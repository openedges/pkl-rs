// Exact fixture-backed Host operations. URI search, caching and Pkl evaluation
// remain in the shared runtime. Provider tables are unordered input facts.
import {runResourceCase, compareResourceCase} from './resource-session.mjs';
import {completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest} from './module-policy-session.mjs';

const requestFields = {
  'check-import': ['kind', 'originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'],
  capabilities: ['kind', 'originalUri', 'importerOriginalUri', 'importerResolvedUri'],
  'has-element': ['kind', 'originalUri', 'importerOriginalUri', 'importerResolvedUri'],
  resolve: ['kind', 'originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'],
  'load-source': ['kind', 'originalUri', 'resolvedUri', 'sourceUri'],
};
const descriptorFields = new Set(['keyId', 'keyKind', 'keyHandle', 'resolvedKeyId', 'resolvedKeyHandle', 'selectionPhase', 'resolutionPhase']);
const allRequestFields = new Set(['id', ...Object.values(requestFields).flat(), ...descriptorFields]);
const same = (left, right) => Array.isArray(left) && Array.isArray(right)
  && left.length === right.length && left.every((value, index) => value === right[index]);
function strings(value, name) {
  if (!Array.isArray(value) || value.some(entry => typeof entry !== 'string'))
    throw Error(`invalid ${name}`);
  return value;
}
function requestKey(request, provider = false) {
  const fields = requestFields[request?.kind];
  if (!fields || fields.some(name => typeof request[name] !== 'string'))
    throw Error('incomplete module request identity');
  if (provider && !same(Object.keys(request).sort(), [...fields].sort()))
    throw Error('unexpected provider request fields');
  if (!provider && (Object.keys(request).some(name => !allRequestFields.has(name))
      || [...allRequestFields].some(name => !fields.includes(name) && name !== 'id' && !descriptorFields.has(name)
        && request[name] != null))) throw Error('unexpected module request fields');
  return JSON.stringify(fields.map(name => request[name]));
}
function failure(value) {
  if (value?.kind !== 'failure' || !['not-found', 'io', 'denied', 'unsupported',
    'security-manager-exception'].includes(value.errorKind)
    || !Object.hasOwn(value, 'message')
    || (typeof value.message !== 'string' && value.message !== null)
    || (['denied', 'unsupported'].includes(value.errorKind) && value.message === null))
    throw Error('invalid typed module failure');
}
function completion(kind, result) {
  if (result?.kind === 'failure') {
    failure(result);
    if (kind === 'check-import' && !['denied', 'unsupported', 'security-manager-exception'].includes(result.errorKind))
      throw Error('invalid import-check failure provenance');
    return;
  }
  if (kind === 'check-import' && result?.kind === 'import-checked') return;
  if (kind === 'capabilities' && result?.kind === 'capabilities'
      && ['local', 'hierarchical', 'fragmentPaths'].every(name => typeof result[name] === 'boolean')) return;
  if (kind === 'has-element' && result?.kind === 'has-element' && typeof result.exists === 'boolean') return;
  if (kind === 'resolve' && result?.kind === 'resolve'
      && ['resolvedUri', 'sourceUri'].every(name => typeof result[name] === 'string')) return;
  if (kind === 'load-source' && result?.kind === 'source'
      && ['text', 'inferredName'].every(name => typeof result[name] === 'string')) return;
  throw Error('module completion does not match request kind');
}
function readCompletion(result, counted) {
  if (!counted) {
    if (result?.kind !== 'security-manager-exception' || !Object.hasOwn(result, 'message')
        || (result.message !== null && typeof result.message !== 'string'))
      throw Error('uncounted read must be a typed security denial');
    return;
  }
  if (result?.kind !== 'resource' || typeof result.resourceUri !== 'string'
      || !Array.isArray(result.bytes)
      || result.bytes.some(value => !Number.isInteger(value) || value < 0 || value > 255))
    throw Error('invalid File Resource provider');
}
function tables(fixture) {
  if (!Array.isArray(fixture.moduleProviders) || !Array.isArray(fixture.readProviders))
    throw Error('missing module/read providers');
  const modules = new Map(), reads = new Map();
  for (const entry of fixture.moduleProviders) {
    const key = requestKey(entry.request, true);
    if (modules.has(key)) throw Error('duplicate module provider identity');
    completion(entry.request.kind, entry.result);
    strings(entry.observableEvents, 'module provider observable events');
    modules.set(key, entry);
  }
  for (const entry of fixture.readProviders) {
    if (typeof entry?.uri !== 'string' || typeof entry.countedReader !== 'boolean')
      throw Error('invalid read provider identity');
    if (reads.has(entry.uri)) throw Error('duplicate read provider identity');
    readCompletion(entry.result, entry.countedReader);
    strings(entry.observableEvents, 'read provider observable events');
    reads.set(entry.uri, entry);
  }
  for (const entry of fixture.readerPlan) {
    const provider = reads.get(entry.uri), value = entry.result;
    if (!provider?.countedReader || value?.kind !== 'resource'
        || value.resourceUri !== provider.result.resourceUri
        || !same(value.bytes, provider.result.bytes))
      throw Error('counted reader plan differs from File provider inputs');
  }
  // These are configured factory inputs, independent of source lookup or caches.
  const moduleUris = new Set(fixture.moduleProviders.filter(entry => entry.request.kind === 'resolve')
    .map(entry => entry.request.originalUri));
  return {modules, reads, moduleUris};
}
function complete(session, token, request, result) {
  if (result.kind === 'failure') {
    if (request.kind === 'check-import') session.rejectModuleImportCheck(token, result.errorKind, result.message, 0);
    else if (request.kind === 'capabilities' || request.kind === 'has-element')
      session.rejectModulePreparation(token, request.kind === 'has-element', result.errorKind, result.message, 0);
    else if (request.kind === 'resolve') session.rejectModuleResolution(token, result.errorKind, result.message, 0);
    else session.rejectModuleSource(token, result.errorKind, result.message, 0);
  } else if (result.kind === 'import-checked') session.completeModuleImportCheck(token, 0);
  else if (result.kind === 'capabilities')
    session.completeModuleCapabilities(token, result.local, result.hierarchical, result.fragmentPaths, 0);
  else if (result.kind === 'has-element') session.completeModuleHasElement(token, result.exists, 0);
  else if (result.kind === 'resolve')
    session.completeModuleResolution(token, request.originalUri, result.resolvedUri, result.sourceUri, 0);
  else session.completeModuleSource(token, result.text, result.inferredName, 0);
}

export function runModulePreparationCase(api, fixture, budget = 7) {
  const {modules, reads, moduleUris} = tables(fixture), events = [], requests = [];
  let callIndex = -1, pendingCollections = 0;
  const wrappedApi = {SourceSession: {forExpressions(...arguments_) {
    const session = api.SourceSession.forExpressions(...arguments_);
    return new Proxy(session, {get(target, name) {
      if (name === 'startPath') return (index, ...rest) => {
        callIndex = index; events[index] = [];
        return target.startPath(index, ...rest);
      };
      if (name === 'poll') return (...args) => {
        const event = target.poll(...args);
        if (event.kind !== 'module-request' && event.kind !== 'read') return event;
        if (typeof event.token !== 'string' || !event.token.length || callIndex < 0)
          throw Error('invalid suspended Host request token');
        if (event.kind === 'module-request') {
          const request = event.module;
          requests.push({callIndex, token: event.token, kind: event.kind, module: {...request}});
          if (typeof request?.id !== 'string' || !/^[0-9]+$/.test(request.id))
            throw Error('invalid decimal module request ID');
          validateModuleRequest(event);
          if (['select-key', 'query-cache-policy', 'query-fragment-paths', 'resolve-import-uri'].includes(request.kind)) {
            target.collect(); pendingCollections++;
            if (completeFixtureModuleKey(target, event, moduleUris))
              requests[requests.length - 1].provider = 'configured-fixture-key';
            else if (completeDefaultModuleRequest(target, event))
              requests[requests.length - 1].provider = 'pkl-0.32.0-default';
            else throw Error('unconfigured exact module key request: ' + JSON.stringify(request));
            return {kind: 'yield'};
          }
          const provider = modules.get(requestKey(request));
          if (!provider) {
            if (request.kind !== 'check-import' && request.kind !== 'resolve')
              throw Error('unconfigured exact module request: ' + JSON.stringify(request));
            target.collect(); pendingCollections++;
            if (!completeDefaultModuleRequest(target, event))
              throw Error('unconfigured exact module request: ' + JSON.stringify(request));
            // These are actual new Host requests, not synthesized Java events.
            requests[requests.length - 1].provider = 'pkl-0.32.0-default';
            return {kind: 'yield'};
          }
          target.collect(); pendingCollections++;
          events[callIndex].push(...provider.observableEvents);
          complete(target, event.token, request, provider.result);
          return {kind: 'yield'};
        }
        const uri = target.resolvedReadUri(event.token), provider = reads.get(uri);
        requests.push({callIndex, token: event.token, kind: event.kind, uri,
          requestedUri: event.uri ?? null, countedReader: provider?.countedReader ?? null});
        if (!provider) throw Error('unconfigured exact File read: ' + uri);
        events[callIndex].push(...provider.observableEvents);
        if (provider.countedReader) return event;
        target.collect(); pendingCollections++;
        target.rejectReadSecurityManager(event.token, provider.result.message, 0);
        return {kind: 'yield'};
      };
      const value = Reflect.get(target, name, target);
      return typeof value === 'function' ? value.bind(target) : value;
    }});
  }}};
  const actual = runResourceCase(wrappedApi, fixture, budget);
  actual.moduleRequests = requests;
  actual.modulePolicy = 'exact-fixture-providers-then-pkl-0.32.0-default';
  actual.observableEventsByCall = events;
  actual.architecture.pendingModuleOrDeniedReadCollections = pendingCollections;
  for (const call of actual.calls) call.observableEvents = events[call.index];
  return actual;
}

export function compareModulePreparationCase(fixture, actual) {
  const comparison = compareResourceCase(fixture, actual);
  comparison.checks.observableCallCoverage = actual.observableEventsByCall.length === fixture.expected.length;
  for (let index = 0; index < fixture.expected.length; index++) {
    const expected = strings(fixture.expected[index].observableEvents, 'expected observable events');
    const row = comparison.comparisons[index];
    row.checks.observableEvents = same(actual.calls[index]?.observableEvents, expected);
    if (row.status === 'pass' && !row.checks.observableEvents) row.status = 'mismatch';
  }
  comparison.passed = Object.values(comparison.checks).every(Boolean)
    && comparison.comparisons.every(row => row.status === 'pass');
  return comparison;
}
