import test from 'node:test';
import assert from 'node:assert/strict';
import {runModuleKeyCase, compareModuleKeyCase} from './modulekey-session.mjs';

const uri = 'pkl:base', owner = 'file:///main.pkl';
const modulePlan = {requestedUri: uri, selectedUri: uri, resolvedUri: 'memo:/shared', sourceUri: uri,
  sourceText: 'tag = 99', inferredName: 'base', fragmentPaths: false, cacheMode: 'dynamic'};
const profile = {id: 'fixture', customModules: [modulePlan], fileModules: []};
const expected = {status: 'success', stdoutUnits: [55], stderrUnits: [], stdoutHex: '37', stderrHex: ''};
const fixture = {id: 'fixture', cohort: 'spi132', repeat: 0, session: 0, initialPolicy: 'allow',
  source: 'value=7', uri: owner, moduleName: 'main', expressions: ['value', 'value'],
  operations: [{kind: 'evaluate', source: 'main.pkl', expression: 'value'}, {kind: 'cache', value: 'off'},
    {kind: 'evaluate', source: 'main.pkl', expression: 'value'}, {kind: 'gc'}], expected: [expected, expected]};
const keyFields = keyId => ({keyId, keyKind: 'host', keyHandle: `configured-key:${keyId}`});
function requests(keyId, target = uri) {
  const resolvedKeyId = `${keyId}0`, descriptor = keyFields(keyId);
  return [
    {kind: 'select-key', originalUri: target, keyId, selectionPhase: 'runtime-import'},
    {kind: 'query-cache-policy', originalUri: target, ...descriptor},
    {kind: 'resolve', originalUri: target, importUri: target, importerOriginalUri: owner,
      importerResolvedUri: owner, ...descriptor, resolvedKeyId},
    {kind: 'load-source', originalUri: target, resolvedUri: modulePlan.resolvedUri, sourceUri: target,
      ...descriptor, resolvedKeyId, resolvedKeyHandle: `configured-resolved:${resolvedKeyId}`},
    {kind: 'query-fragment-paths', originalUri: target, ...descriptor},
  ];
}
function setup(programs, options = {}) {
  const calls = [], payloads = [];
  class Session {
    constructor() { this.nextId = 0; this.nextCall = 0; }
    static forExpressions() { return new Session(); }
    static moduleEvaluator() { calls.push(['moduleEvaluator']); return new Session(); }
    startPath(index) { this.pending = [...programs[index]]; this.index = index; }
    evaluateModuleSource(...args) { calls.push(['evaluateModuleSource', ...args]); this.startPath(this.nextCall++); }
    poll() {
      const module = this.pending.shift();
      if (module) {
        const id = String(this.nextId++); this.token = `token-${id}`;
        return {kind: 'module-request', token: this.token, module: {id, ...module}};
      }
      const value = Uint16Array.of(55); payloads.push(value);
      return options.terminalError ? {kind: 'error', error: value} : {kind: 'value', value};
    }
    completeDefaultModule() { assert.fail('configured custom key must precede the default pkl factory'); }
    collect() {} heapSlots() { return 0; } free() {}
    close() { if (options.invalidatePayload) for (const view of payloads) view[0] = 0; }
  }
  for (const name of ['completeModuleKey', 'completeModuleCachePolicy', 'completeModuleFragmentPaths',
    'completeModuleResolvedKey', 'completeModuleSource', 'rejectModuleKey']) Session.prototype[name] = function (...args) {
    assert.equal(args[0], this.token);
    calls.push([name, ...args]); if (options.throwAt === name) throw Error('stale completion token');
  };
  return {api: {SourceSession: Session}, calls};
}

test('custom builtin-spelling keys keep separate factory snapshots and actual per-key source requests', () => {
  const {api, calls} = setup([requests('1'), requests('2')]);
  const actual = runModuleKeyCase(api, fixture, profile);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(calls.filter(row => row[0] === 'completeModuleCachePolicy'), [
    ['completeModuleCachePolicy', 'token-1', '1', true, 0],
    ['completeModuleCachePolicy', 'token-6', '2', false, 0],
  ]);
  assert.equal(calls.filter(row => row[0] === 'completeModuleSource').length, 2);
  assert.equal(actual.requests.length, 10);
  assert.deepEqual(actual.hostEvents.filter(row => row.kind === 'factory-key').map(row => row.cachedSnapshot), [true, false]);
  assert.equal(actual.hostEvents.at(-1).kind, 'explicit-collect');
  assert.equal(actual.architecture.releasedHostKeys, true);
  const comparison = compareModuleKeyCase(fixture, actual);
  assert.equal(comparison.rawApiPassed, true);
  assert.equal(comparison.fullLifecyclePassed, false);
});

test('public entrypoint loads a URI ModuleSource without substituting supplied contents', () => {
  const plan = {...modulePlan, requestedUri: owner, selectedUri: owner, sourceUri: owner};
  const events = requests('1', owner);
  events[0].selectionPhase = 'primary';
  const {importUri, importerOriginalUri, importerResolvedUri, ...selected} = events[2];
  events[2] = {...selected, kind: 'resolve-key', resolutionPhase: 'primary'};
  const one = {...fixture, operations: fixture.operations.slice(0, 1), expressions: ['value'], expected: [expected]};
  const {api, calls} = setup([events]);
  const actual = runModuleKeyCase(api, one, {...profile, customModules: [], fileModules: [plan]}, 7,
    {entrypoint: 'public'});
  assert.equal(actual.acquisitionError, null);
  assert.equal(actual.profile, 'public');
  assert.deepEqual(calls.filter(row => row[0] === 'evaluateModuleSource'), [
    ['evaluateModuleSource', owner, null, 'value', 0],
  ]);
  assert.equal(actual.requests[2].module.resolutionPhase, 'primary');
  assert.equal(compareModuleKeyCase(one, actual).rawApiPassed, true);
  assert.equal(compareModuleKeyCase(one, actual).fullLifecyclePassed, false);
});

test('repeated transport IDs cannot disappear behind otherwise matching API values', () => {
  const first = requests('1'); first[1].id = '0';
  const {api, calls} = setup([first, []]);
  const actual = runModuleKeyCase(api, fixture, profile);
  assert.match(actual.acquisitionError.message, /request identity/);
  assert.equal(actual.requests.length, 2);
  assert.equal(calls.some(row => row[0] === 'completeModuleCachePolicy'), false);
});

test('resolved-key ownership cannot be substituted by matching URI strings', () => {
  for (const changed of [{resolvedKeyHandle: 'foreign'}, {resolvedKeyId: '11'}]) {
    const first = requests('1'); first[3] = {...first[3], ...changed};
    const {api, calls} = setup([first, []]);
    const actual = runModuleKeyCase(api, fixture, profile);
    assert.match(actual.acquisitionError.message, /resolved-key descriptor ownership mismatch/);
    assert.equal(calls.some(row => row[0] === 'completeModuleSource'), false);
    assert.equal(actual.architecture.terminalSlots, 0);
  }
});

test('selection does not resolve or load a source until those requests actually arrive', () => {
  const {api, calls} = setup([[requests('1')[0]], [requests('2')[0]]]);
  const actual = runModuleKeyCase(api, fixture, profile);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(calls.map(row => row[0]), ['completeModuleKey', 'completeModuleKey']);
});

test('inherited import URI resolution validates selected-key ownership before completing the request', () => {
  const resolve = {kind: 'resolve-import-uri', originalUri: uri, importUri: 'child.pkl', ...keyFields('1')};
  for (const changed of [null, {keyHandle: 'foreign'}, {keyKind: 'synthetic'},
    {originalUri: 'memo:/other'}, {keyHandle: undefined}]) {
    const {api, calls} = setup([[requests('1')[0], {...resolve, ...changed}], []]);
    api.SourceSession.prototype.completeDefaultModule = function (...args) {
      assert.equal(args[0], this.token); calls.push(['completeDefaultModule', ...args]); return true;
    };
    const actual = runModuleKeyCase(api, fixture, profile);
    if (changed === null) {
      assert.equal(actual.acquisitionError, null);
      assert.deepEqual(calls.filter(row => row[0] === 'completeDefaultModule'), [
        ['completeDefaultModule', 'token-1', 0],
      ]);
      assert.equal(compareModuleKeyCase(fixture, actual).rawApiPassed, true);
    } else {
      assert.match(actual.acquisitionError.message, /ownership mismatch|invalid default-policy/);
      assert.equal(calls.some(row => row[0] === 'completeDefaultModule'), false);
      assert.equal(actual.architecture.terminalSlots, 0);
      assert.equal(compareModuleKeyCase(fixture, actual).rawApiPassed, false);
    }
  }
});

test('unknown Host key identities cannot bypass registration through inherited defaults', () => {
  for (const kind of ['query-fragment-paths', 'resolve-import-uri']) {
    const event = {kind, originalUri: uri, ...keyFields('99'),
      ...(kind === 'resolve-import-uri' ? {importUri: 'child.pkl'} : {})};
    const {api, calls} = setup([[event], []]);
    api.SourceSession.prototype.completeDefaultModule = () => {
      assert.fail('unregistered Host key must be rejected before a default callback');
    };
    const actual = runModuleKeyCase(api, fixture, profile);
    assert.match(actual.acquisitionError.message, /unregistered Host ModuleKey/);
    assert.equal(calls.length, 0);
    assert.equal(actual.observations.filter(event => event.kind === 'module-request').length, 1);
    assert.equal(actual.observations.some(event => event.kind === 'host-call'), false);
  }
});

test('unknown profiles and mismatched operation sequences cannot become compatibility passes', () => {
  for (const change of [{cohort: 'unknown'}, {initialPolicy: 'unknown'}, {expressions: ['different']}]) {
    const {api} = setup([[], []]);
    const actual = runModuleKeyCase(api, {...fixture, ...change}, profile);
    assert.notEqual(actual.acquisitionError, null);
    assert.equal(compareModuleKeyCase(fixture, actual).rawApiPassed, false);
  }
});

test('configured factory failures run only on actual selection and precede custom key lookup', () => {
  const target = 'throwing:x', event = {...requests('1', target)[0], selectionPhase: 'clause-preparation'};
  const configured = {...profile,
    customModules: [{...modulePlan, requestedUri: target, selectedUri: target}],
    factoryFailures: [{scheme: 'throwing', errorKind: 'io', message: 'authored factory failure'}]};
  const {api, calls} = setup([[event], [{...event, keyId: '2'}]], {terminalError: true});
  const actual = runModuleKeyCase(api, {...fixture, cohort: 'factory80'}, configured);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(calls, [
    ['rejectModuleKey', 'token-0', false, 'io', 'authored factory failure', 0],
    ['rejectModuleKey', 'token-1', false, 'io', 'authored factory failure', 0],
  ]);
  assert.deepEqual(actual.calls.map(call => [call.status, call.stderrUnits]), [['error', [55]], ['error', [55]]]);
  assert.equal(actual.hostEvents.filter(event => event.kind === 'factory-failure').length, 2);
  assert.equal(actual.architecture.terminalSlots, 0);
  assert.equal(actual.architecture.releasedHostKeys, true);
});

test('factory scheme matching preserves case and delegates unknown schemes to the shared default', () => {
  const {api, calls} = setup([[requests('1', 'Throwing:x')[0]], [requests('2', 'noLoader:x')[0]]]);
  api.SourceSession.prototype.completeDefaultModule = function (...args) {
    assert.equal(args[0], this.token); calls.push(['completeDefaultModule', ...args]); return true;
  };
  const actual = runModuleKeyCase(api, {...fixture, cohort: 'factory80'}, {...profile,
    factoryFailures: [{scheme: 'throwing', errorKind: 'io', message: 'authored factory failure'}]});
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(calls.map(call => call[0]), ['completeDefaultModule', 'completeDefaultModule']);
});

test('factory rejection completion errors remain acquisition failures and release Host keys', () => {
  const {api, calls} = setup([[requests('1', 'throwing:x')[0]], []], {throwAt: 'rejectModuleKey'});
  const actual = runModuleKeyCase(api, {...fixture, cohort: 'factory80'}, {...profile,
    factoryFailures: [{scheme: 'throwing', errorKind: 'io', message: 'authored factory failure'}]});
  assert.match(actual.acquisitionError.message, /stale completion token/);
  assert.equal(calls.length, 1);
  assert.equal(actual.calls.length, 0);
  assert.equal(actual.architecture.terminalSlots, 0);
  assert.equal(actual.architecture.releasedHostKeys, true);
  assert.equal(compareModuleKeyCase(fixture, actual).rawApiPassed, false);
  assert.deepEqual(actual.observations.slice(-4).map(event => event.kind),
    ['host-throw', 'acquisition-error', 'close-call', 'close-return']);
  assert.equal(actual.observations.some(event => event.kind === 'api-result'), false);
});

test('explicit no-loader inputs reject the same token only after the shared default declines', () => {
  const {api, calls} = setup([[requests('1', 'noLoader:x')[0]], [requests('2', 'noLoader:x')[0]]],
    {terminalError: true});
  api.SourceSession.prototype.completeDefaultModule = function (...args) {
    assert.equal(args[0], this.token); calls.push(['completeDefaultModule', ...args]); return false;
  };
  const actual = runModuleKeyCase(api, {...fixture, cohort: 'factory80'}, {...profile, noLoaderUris: ['noLoader:x']});
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(calls, [
    ['completeDefaultModule', 'token-0', 0], ['rejectModuleKey', 'token-0', false, 'no-loader', null, 0],
    ['completeDefaultModule', 'token-1', 0], ['rejectModuleKey', 'token-1', false, 'no-loader', null, 0],
  ]);
  assert.equal(actual.hostEvents.filter(event => event.kind === 'factory-miss').length, 2);
  assert.equal(actual.architecture.terminalSlots, 0);
  assert.deepEqual(actual.observations.map(event => event.sequence), actual.observations.map((_, i) => i));
  const firstCall = actual.observations.filter(event => event.callIndex === 0).slice(2);
  assert.deepEqual(firstCall.map(event => event.kind), ['api-call', 'api-started', 'module-request',
    'host-call', 'host-return', 'host-call', 'host-return', 'api-result']);
  assert.deepEqual(firstCall[3].args, ['token-0', 0]);
  assert.deepEqual([firstCall[4].returnedUndefined, firstCall[4].value], [false, false]);
  assert.deepEqual(firstCall[5].args, ['token-0', false, 'no-loader', null, 0]);
  assert.deepEqual([firstCall[6].returnedUndefined, firstCall[6].value], [true, null]);
  assert.deepEqual(actual.observations.filter(event => event.kind.startsWith('explicit-gc')).map(event => event.kind),
    ['explicit-gc-call', 'explicit-gc-return']);
  assert.deepEqual(actual.observations.find(event => event.kind === 'cache-config').cached, false);
  actual.calls[0].stderrUnits.push(10);
  assert.deepEqual(firstCall.at(-1).call.stderrUnits, [55]);
});

test('unconfigured URIs do not become fabricated no-loader diagnostics', () => {
  const {api, calls} = setup([[requests('1', 'file:///unconfigured.pkl')[0]], []]);
  api.SourceSession.prototype.completeDefaultModule = () => false;
  const actual = runModuleKeyCase(api, {...fixture, cohort: 'factory80'}, {...profile, noLoaderUris: ['noLoader:x']});
  assert.match(actual.acquisitionError.message, /unconfigured factory URI/);
  assert.equal(calls.length, 0);
  assert.equal(actual.calls.length, 0);
  assert.equal(compareModuleKeyCase(fixture, actual).rawApiPassed, false);
});

test('binding exceptions and invalidated result ownership remain failed acquisitions', () => {
  const stale = setup([requests('1'), []], {throwAt: 'completeModuleResolvedKey'});
  const actual = runModuleKeyCase(stale.api, fixture, profile);
  assert.match(actual.acquisitionError.message, /stale completion token/);
  assert.equal(stale.calls.some(row => row[0] === 'completeModuleSource'), false);
  const invalid = setup([[], []], {invalidatePayload: true});
  const changed = runModuleKeyCase(invalid.api, fixture, profile);
  assert.equal(changed.architecture.ownedResultsStable, false);
  assert.equal(compareModuleKeyCase(fixture, changed).rawApiPassed, false);
});

test('raw channel and missing-call differences survive comparison without normalization', () => {
  const {api} = setup([[], []]);
  const actual = runModuleKeyCase(api, fixture, profile);
  actual.calls[0].stdoutUnits.push(10);
  actual.calls.pop();
  assert.deepEqual(compareModuleKeyCase(fixture, actual).failures, ['call-count', 'raw-api-0', 'raw-api-1']);
});
