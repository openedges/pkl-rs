import test from 'node:test';
import assert from 'node:assert/strict';
import {completeDefaultModuleRequest} from './module-policy-session.mjs';
import {runBootstrapCase} from './bootstrap-session.mjs';
import {runModuleInfoCase} from './moduleinfo-session.mjs';
import {runSecurityDenialCase} from './security-denial-session.mjs';
import {runGuestCatchCase} from './guest-catch-session.mjs';

const edge = {kind: 'module-request', token: 'edge-token', module: {
  id: '18446744073709551615', kind: 'check-import', originalUri: 'pkl:test', importUri: 'pkl:test',
  importerOriginalUri: 'file:///main.pkl', importerResolvedUri: 'file:///resolved.pkl',
}};

test('default policy transport preserves identity and distinguishes unsupported from malformed results', () => {
  for (const result of [true, false]) {
    let calls = 0;
    const session = {completeDefaultModule(token, now) {
      calls++; assert.equal(token, edge.token); assert.equal(now, 0); return result;
    }};
    assert.equal(completeDefaultModuleRequest(session, edge), result);
    assert.equal(calls, 1);
  }
  for (const result of [undefined, null, 0, 'false']) {
    assert.throws(() => completeDefaultModuleRequest({completeDefaultModule: () => result}, edge),
      /invalid default module completion result/);
  }
  assert.throws(() => completeDefaultModuleRequest({completeDefaultModule() { throw Error('stale token'); }}, edge),
    /stale token/);
});

test('incomplete or contradictory request metadata never reaches a completion callback', () => {
  const invalid = ['originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'].map(name => {
    const request = {...edge.module}; delete request[name]; return request;
  });
  invalid.push({...edge.module, id: 1}, {...edge.module, kind: 'load-source'},
    {...edge.module, sourceUri: 'pkl:test'}, {...edge.module, injected: true});
  for (const module of invalid) {
    assert.throws(() => completeDefaultModuleRequest({completeDefaultModule() {
      assert.fail('invalid context must not consume a token');
    }}, {...edge, module}), /invalid default-policy module request context/);
  }
});

const fixture = {id: 'transport-only', repeat: 0, source: 'value = 7', moduleUri: 'file:///main.pkl',
  moduleName: 'main', expressions: ['value'], formats: ['absent'], causes: [], deniedUri: 'denied:/child',
  callbacks: [], comparisonScope: 'raw-calls', group: 'transport', mode: 'plain'};
function api(events, observed) {
  class Session {
    static forExpressions() { return new Session(); }
    startPath() { this.events = events.map(event => structuredClone(event)); }
    poll() { return this.events.shift() ?? {kind: 'value', value: Uint16Array.of(55)}; }
    completeDefaultModule(token) { observed.push(['default', token]); return true; }
    rejectModuleResolution(token, kind, cause) { observed.push(['deny', token, kind, cause]); }
    completeModuleResolution(...args) { observed.push(['resolve', ...args]); }
    completeModuleSource(...args) { observed.push(['source', ...args]); }
    completeReadText(token, value, now) { observed.push(['read-text', token, Array.from(value), now]); }
    respondConfigured(token, now) { observed.push(['read-default', token, now]); }
    collect() {} close() {} free() {} heapSlots() { return 0; }
  }
  return {SourceSession: Session};
}

test('existing bootstrap, ModuleInfo, security and catch consumers retain real new requests', () => {
  for (const run of [runBootstrapCase, runModuleInfoCase, runSecurityDenialCase, runGuestCatchCase]) {
    const observed = [], actual = run(api([edge], observed), fixture);
    const session = actual.sessions?.[0] ?? actual;
    assert.equal(session.acquisitionError, null);
    assert.equal(session.calls.length, 1);
    assert.deepEqual(session.moduleRequests[0].module, edge.module);
    assert.deepEqual(observed, [['default', 'edge-token']]);
    assert.equal(session.architecture.terminalSlots, 0);
    if (run === runGuestCatchCase) {
      assert.deepEqual(actual.events, ['outer-0 success']);
      assert.equal(actual.counts.resolve, 0);
    }
  }
});

test('bootstrap property replies use typed Resource completion for cache ownership', () => {
  const observed = [], actual = runBootstrapCase(api([
    {kind: 'read', token: 'property-token', uri: 'prop:pkl.outputFormat'},
  ], observed), {...fixture, formats: ['json', 'absent']});
  assert.equal(actual.resourceCompletion, 'typed-resource-text-and-configured-default');
  assert.deepEqual(actual.sessions.map(session => session.acquisitionError), [null, null]);
  assert.deepEqual(actual.sessions.map(session => session.reads), [1, 1]);
  assert.deepEqual(observed, [
    ['read-text', 'property-token', [106, 115, 111, 110], 0],
    ['read-default', 'property-token', 0],
  ]);
});

test('typed resolve denial remains explicit after default import admission', () => {
  const resolved = {...edge, token: 'resolve-token', module: {...edge.module, id: '1', kind: 'resolve',
    originalUri: fixture.deniedUri, importUri: fixture.deniedUri}};
  const observed = [], actual = runSecurityDenialCase(api([edge, resolved], observed),
    {...fixture, causes: [{errorClass: 'org.pkl.core.SecurityManagerException', uri: fixture.deniedUri,
      units: [100, 0xd800]}]});
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(observed, [['default', 'edge-token'],
    ['deny', 'resolve-token', 'security-manager-exception', 'd\ud800']]);
  assert.equal(actual.denials.length, 1);
  assert.equal(actual.moduleRequests.length, 2);
});

test('catch resolver/source callbacks retain their original projected events after admission', () => {
  const resolve = {...edge, token: 'resolve-token', module: {...edge.module, id: '1', kind: 'resolve',
    originalUri: 'catch:/child', importUri: 'catch:/child'}};
  const source = {kind: 'module-request', token: 'source-token', module: {id: '2', kind: 'load-source',
    originalUri: 'catch:/child', resolvedUri: 'catch:/child', sourceUri: 'catch:/child'}};
  const observed = [], actual = runGuestCatchCase(api([edge, resolve, source], observed), fixture);
  assert.equal(actual.acquisitionError, null);
  assert.equal(actual.counts.resolve, 1);
  assert.equal(actual.counts.source, 1);
  assert.equal(actual.moduleRequests.length, 3);
  assert.deepEqual(actual.events, ['resolve catch:/child attempt=1', 'source-start catch:/child attempt=1',
    'source-end catch:/child attempt=1', 'outer-0 success']);
  assert.deepEqual(observed.map(row => row[0]), ['default', 'resolve', 'source']);
});
