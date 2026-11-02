import test from 'node:test';
import assert from 'node:assert/strict';
import {completeDefaultModuleRequest, completeFixtureModuleKey, validateModuleRequest} from './module-policy-session.mjs';
import {runBootstrapCase} from './bootstrap-session.mjs';
import {runModuleInfoCase} from './moduleinfo-session.mjs';
import {runSecurityDenialCase} from './security-denial-session.mjs';
import {runGuestCatchCase} from './guest-catch-session.mjs';
import {validateGlobRequest, completeDefaultGlob, completeGlobBoolean} from './glob-transport.mjs';

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

test('explicit catch and security callbacks reject incomplete selected-key descriptors before completion', () => {
  for (const run of [runSecurityDenialCase, runGuestCatchCase]) {
    const uri = run === runSecurityDenialCase ? fixture.deniedUri : 'catch:/child';
    const resolve = {...edge, module: {...edge.module, kind: 'resolve', originalUri: uri, importUri: uri, keyId: '7'}};
    const observed = [], input = {...fixture, causes: [{errorClass: 'org.pkl.core.SecurityManagerException', uri, units: [100]}]};
    const result = run(api([resolve], observed), input);
    assert.match(result.acquisitionError.message, /module request context/);
    assert.deepEqual(observed, []);
    assert.equal(result.calls.length, 0);
    assert.equal(result.moduleRequests.length, 1);
  }
});

const keyId = '18446744073709551615';
const key = (kind, metadata = {}) => ({kind: 'module-request', token: 'key-token', module: {
  id: '9007199254740993', kind, originalUri: 'mem:/child', ...metadata,
}});
const selection = key('select-key', {keyId, selectionPhase: 'clause-preparation'});
const policy = key('query-cache-policy', {keyId, keyKind: 'host', keyHandle: `fixture-key:${keyId}`});
const resolution = {...edge, module: {...edge.module, kind: 'resolve', keyId, keyKind: 'host',
  keyHandle: 'opaque selected handle', resolvedKeyId: '9007199254740995'}};
const loaded = key('load-source', {resolvedUri: 'mem:/resolved', sourceUri: 'mem:/diagnostic',
  keyId, keyKind: 'host', keyHandle: 'opaque selected handle', resolvedKeyId: '9007199254740995',
  resolvedKeyHandle: 'opaque resolved handle'});

test('all seven request kinds preserve new identities and explicit legacy resolve/source compatibility', () => {
  const events = [edge, selection, policy, resolution, loaded,
    key('capabilities', {importerOriginalUri: 'mem:/original', importerResolvedUri: 'mem:/resolved'}),
    key('has-element', {importerOriginalUri: 'mem:/original', importerResolvedUri: 'mem:/resolved'}),
    {...edge, module: {...edge.module, kind: 'resolve'}},
    key('load-source', {resolvedUri: 'mem:/resolved', sourceUri: 'mem:/diagnostic'})];
  for (const event of events) {
    const before = structuredClone(event);
    assert.equal(validateModuleRequest(event), event.module);
    assert.deepEqual(event, before);
  }
  for (const event of [selection, policy, resolution]) {
    const seen = [];
    assert.equal(completeDefaultModuleRequest({completeDefaultModule(...args) {
      seen.push(args); return true;
    }}, event), true);
    assert.deepEqual(seen, [[event.token, 0]]);
  }
  assert.equal(validateModuleRequest({...loaded, module: {...loaded.module,
    keyKind: 'supplied-text'}}).keyKind, 'supplied-text');
  assert.equal(validateModuleRequest({...policy, module: {...policy.module,
    keyKind: 'synthetic'}}).keyKind, 'synthetic');
  const legacy = {...edge, module: {...edge.module, kind: 'resolve', keyId: null,
    keyKind: null, keyHandle: null, resolvedKeyId: null, resolvedKeyHandle: null, selectionPhase: null}};
  assert.equal(validateModuleRequest(legacy), legacy.module);
});

test('partial descriptors, contradictory phases and malformed IDs never consume a default token', () => {
  const invalid = [];
  for (const event of [selection, policy, resolution, loaded]) {
    const required = event === selection ? ['keyId', 'selectionPhase']
      : ['keyId', 'keyKind', 'keyHandle', ...(event === policy ? [] : ['resolvedKeyId']),
        ...(event === loaded ? ['resolvedKeyHandle'] : [])];
    for (const name of required) {
      const module = {...event.module}; delete module[name]; invalid.push({...event, module});
    }
  }
  for (const bad of [1, 9007199254740992, '', '01', '-1', '1.0', '18446744073709551616']) {
    invalid.push({...selection, module: {...selection.module, keyId: bad}},
      {...selection, module: {...selection.module, id: bad}},
      {...resolution, module: {...resolution.module, resolvedKeyId: bad}});
  }
  invalid.push({...selection, module: {...selection.module, selectionPhase: 'query'}},
    {...selection, module: {...selection.module, keyHandle: 'too early'}},
    {...policy, module: {...policy.module, selectionPhase: 'primary'}},
    {...resolution, module: {...resolution.module, resolvedKeyHandle: 'too early'}},
    {...loaded, module: {...loaded.module, importUri: 'unexpected'}},
    {...policy, module: {...policy.module, keyKind: 'bundled'}},
    {...policy, module: {...policy.module, keyHandle: 7}},
    {...loaded, module: {...loaded.module, resolvedKeyHandle: 7}},
    {...edge, module: {...edge.module, keyId: '1'}},
    {...edge, module: {...edge.module, originalUri: 'contradictory:/edge'}},
    {...policy, module: {...policy.module, cached: true}});
  for (const event of invalid) {
    assert.throws(() => completeDefaultModuleRequest({completeDefaultModule() {
      assert.fail('malformed context must not consume a token');
    }}, event), /invalid default-policy module request context/);
  }
});

test('empty opaque handles survive transport while fixture ownership still requires its exact handle', () => {
  for (const event of [policy, resolution, loaded]) {
    const module = {...event.module, keyKind: 'standard-library', keyHandle: ''};
    if (event === loaded) module.resolvedKeyHandle = '';
    const actual = {...event, module};
    assert.equal(validateModuleRequest(actual), module);
    if (event !== loaded) assert.equal(completeDefaultModuleRequest({completeDefaultModule: () => true}, actual), true);
  }
  const emptyFixture = {...policy, module: {...policy.module, keyHandle: ''}};
  assert.throws(() => completeFixtureModuleKey({completeModuleCachePolicy() {
    assert.fail('invalid owner must not consume the token');
  }}, emptyFixture, new Set(['mem:/child'])), /descriptor does not match its owner/);
});

test('fragment-path getter is a distinct deferred request and uses the selected-key identity', () => {
  const event = {...policy, module: {...policy.module, kind: 'query-fragment-paths'}};
  assert.equal(validateModuleRequest(event), event.module);
  const seen = [], session = {
    completeModuleKey(...args) { seen.push(['key', ...args]); },
    completeModuleFragmentPaths(...args) { seen.push(['fragment', ...args]); },
    get completeModuleCachePolicy() { assert.fail('fragment getter must not query the cache policy'); },
  };
  const registry = new Set(['mem:/child']);
  assert.equal(completeFixtureModuleKey(session, selection, registry), true);
  assert.deepEqual(seen.map(row => row[0]), ['key']);
  assert.equal(completeFixtureModuleKey(session, event, registry), true);
  assert.deepEqual(seen.at(-1), ['fragment', event.token, event.module.keyId, false, 0]);
  assert.equal(completeDefaultModuleRequest({completeDefaultModule: () => true}, event), true);
  assert.throws(() => validateModuleRequest({...event, module: {...event.module, selectionPhase: 'primary'}}),
    /module request context/);
});

test('primary and query resolution carry selected keys without fabricated importing modules', () => {
  for (const resolutionPhase of ['primary', 'query']) {
    const event = key('resolve-key', {keyId, keyKind: 'synthetic', keyHandle: '',
      resolvedKeyId: '18446744073709551615', resolutionPhase});
    assert.equal(validateModuleRequest(event), event.module);
    assert.equal(completeDefaultModuleRequest({completeDefaultModule: () => true}, event), true);
    for (const fields of [{importerOriginalUri: 'file:///invented'}, {importUri: 'pkl:base'},
      {selectionPhase: 'primary'}, {resolutionPhase: 'runtime-import'}, {resolvedKeyId: null}])
      assert.throws(() => validateModuleRequest({...event, module: {...event.module, ...fields}}), /module request context/);
  }
});

test('declaration URI overrides and delegated capability getters preserve their distinct contexts', () => {
  const descriptor = {keyId, keyKind: 'host', keyHandle: 'declaration'};
  const uri = key('resolve-import-uri', {...descriptor, importUri: '../child.pkl'});
  const before = structuredClone(uri);
  assert.equal(validateModuleRequest(uri), uri.module);
  assert.equal(completeDefaultModuleRequest({completeDefaultModule: () => true}, uri), true);
  assert.deepEqual(uri, before);
  for (const kind of ['capabilities', 'has-element']) {
    const event = key(kind, {...descriptor, importerOriginalUri: 'mem:/original', importerResolvedUri: 'mem:/resolved'});
    assert.equal(validateModuleRequest(event), event.module);
    assert.throws(() => validateModuleRequest({...event, module: {...event.module, keyHandle: null}}), /module request context/);
  }
  assert.throws(() => validateModuleRequest({...uri, module: {...uri.module, importerOriginalUri: 'mem:/invented'}}),
    /module request context/);
});

test('fixture selection precedes defaults and never reads or completes cache policy eagerly', () => {
  const observed = [], registry = new Set(['mem:/child']);
  const session = {
    completeModuleKey(...args) { observed.push(['select', ...args]); },
    completeModuleCachePolicy(...args) { observed.push(['cache-policy', ...args]); },
    completeDefaultModule(...args) { observed.push(['default', ...args]); return true; },
  };
  for (const event of [selection, policy])
    assert.equal(completeFixtureModuleKey(session, event, registry)
      || completeDefaultModuleRequest(session, event), true);
  assert.deepEqual(observed, [
    ['select', 'key-token', keyId, 'mem:/child', 'host', `fixture-key:${keyId}`, 0],
    ['cache-policy', 'key-token', keyId, true, 0],
  ]);
  const unconfigured = {...selection, module: {...selection.module, originalUri: 'mem:/CHILD'}};
  assert.equal(completeFixtureModuleKey(session, unconfigured, registry), false);
  assert.equal(completeDefaultModuleRequest(session, unconfigured), true);
  assert.deepEqual(observed.at(-1), ['default', 'key-token', 0]);
  const builtinSpelling = {...selection, module: {...selection.module, originalUri: 'pkl:base'}};
  assert.equal(completeFixtureModuleKey(session, builtinSpelling, new Set(['pkl:base']))
    || completeDefaultModuleRequest(session, builtinSpelling), true);
  assert.deepEqual(observed.at(-1), ['select', 'key-token', keyId, 'pkl:base', 'host',
    `fixture-key:${keyId}`, 0]);
  const noCacheGetter = {completeModuleKey() {}, get completeModuleCachePolicy() {
    assert.fail('selection must not inspect a later getter');
  }};
  assert.equal(completeFixtureModuleKey(noCacheGetter, selection, registry), true);
});

test('fixture factory returns false without consuming other phases and rejects forged descriptor ownership', () => {
  const unreachable = new Proxy({}, {get() { assert.fail('unhandled request consumed token'); }});
  for (const event of [edge, resolution, loaded, {kind: 'read'}])
    assert.equal(completeFixtureModuleKey(unreachable, event, new Set(['mem:/child'])), false);
  assert.equal(completeFixtureModuleKey(unreachable, policy, new Set()), false);
  for (const module of [{...policy.module, keyKind: 'standard-library'},
    {...policy.module, keyHandle: 'someone-else'}, {...policy.module, keyId: '1'}])
    assert.throws(() => completeFixtureModuleKey(unreachable, {...policy, module},
      new Set(['mem:/child'])), /descriptor does not match its owner/);
  assert.throws(() => completeFixtureModuleKey(unreachable, selection, []), /URI registry/);
});

test('fixture completions preserve stale binding exceptions and each explicit selection call', () => {
  for (const event of [selection, policy]) assert.throws(() => completeFixtureModuleKey({
    completeModuleKey() { throw Error('stale key token'); },
    completeModuleCachePolicy() { throw Error('stale key token'); },
  }, event, new Set(['mem:/child'])), /stale key token/);
  let calls = 0;
  const session = {completeModuleKey() { calls++; }};
  completeFixtureModuleKey(session, selection, new Set(['mem:/child']));
  completeFixtureModuleKey(session, selection, new Set(['mem:/child']));
  assert.equal(calls, 2); // No JS success/site/URI cache is introduced.
});

test('bootstrap and ModuleInfo retain new select/cache/resolve requests without inventing reader events', () => {
  for (const run of [runBootstrapCase, runModuleInfoCase]) {
    const events = [selection, policy, resolution], observed = [];
    const actual = run(api(events, observed), fixture);
    const session = actual.sessions?.[0] ?? actual;
    assert.equal(session.acquisitionError, null);
    assert.deepEqual(session.moduleRequests.map(row => row.module), events.map(event => event.module));
    assert.deepEqual(observed, events.map(event => ['default', event.token]));
    assert.equal(session.calls.length, 1);
    if (run === runBootstrapCase) {
      assert.equal(session.calls[0].readsBefore, 0);
      assert.equal(session.calls[0].readsAfter, 0);
    } else {
      assert.equal(Object.hasOwn(session.calls[0], 'readsBefore'), false);
      assert.equal(Object.hasOwn(session.calls[0], 'readsAfter'), false);
    }
    assert.equal(session.architecture.terminalSlots, 0);
  }
});


const standardKeyKinds = ['standard-file', 'standard-http', 'standard-generic-url'];
const opaqueHandles = ['', 'arbitrary:\ud800x\udfff'];

test('standard keys retain phase descriptors and opaque handles through module dispatch', () => {
  for (const keyKind of standardKeyKinds) for (const keyHandle of opaqueHandles) {
    const descriptor = {keyId, keyKind, keyHandle};
    const events = [
      key('query-cache-policy', descriptor), key('query-fragment-paths', descriptor),
      key('resolve-import-uri', {...descriptor, importUri: '../child.pkl'}),
      key('resolve-key', {...descriptor, resolvedKeyId: '9007199254740995', resolutionPhase: 'query'}),
      {...resolution, module: {...resolution.module, ...descriptor}},
      {...loaded, module: {...loaded.module, ...descriptor, resolvedKeyHandle: keyHandle}},
      ...['capabilities', 'has-element'].map(kind => key(kind, {...descriptor,
        importerOriginalUri: 'mem:/original', importerResolvedUri: 'mem:/resolved'})),
    ];
    for (const event of events) {
      const before = structuredClone(event);
      assert.equal(validateModuleRequest(event), event.module);
      assert.deepEqual(event, before);
      if (['query-cache-policy', 'query-fragment-paths', 'resolve-import-uri', 'resolve-key', 'resolve'].includes(event.module.kind)) {
        for (const result of [false, true]) {
          const calls = [];
          assert.equal(completeDefaultModuleRequest({completeDefaultModule(...args) {
            calls.push(args); return result;
          }}, event), result);
          assert.deepEqual(calls, [[event.token, 0]]);
        }
      }
    }
    const event = events[0];
    assert.throws(() => completeFixtureModuleKey({}, event, new Set([event.module.originalUri])),
      /descriptor does not match its owner/);
    for (const override of [{keyKind: `${keyKind}-unknown`}, {keyHandle: null}, {keyHandle: 7}, {keyId: '18446744073709551616'}]) {
      assert.throws(() => completeDefaultModuleRequest({completeDefaultModule() {
        assert.fail('invalid descriptor must not consume the token');
      }}, {...event, module: {...event.module, ...override}}), /module request context/);
    }
    assert.throws(() => completeDefaultModuleRequest({completeDefaultModule() {
      throw Error('stale standard key token');
    }}, event), /stale standard key token/);
  }
});

test('standard module glob readers retain lossless descriptors and dispatch typed booleans', () => {
  for (const keyKind of standardKeyKinds) for (const readerHandle of opaqueHandles) {
    const event = {kind: 'glob-request', token: 'glob-key-token', glob: {
      id: '9007199254740993', kind: 'is-globbable', readerKind: 'module', readerId: keyId,
      readerUri: 'mem:/child', readerHandle, keyKind,
    }};
    const before = structuredClone(event);
    assert.equal(validateGlobRequest(event), event.glob);
    const calls = [];
    for (const result of [false, true]) {
      assert.equal(completeDefaultGlob({completeDefaultGlob(...args) {
        calls.push(['default', ...args]); return result;
      }}, event, 11), result);
      completeGlobBoolean({completeGlobBoolean(...args) { calls.push(['boolean', ...args]); }}, event, result, 12);
    }
    assert.deepEqual(calls, [
      ['default', event.token, 11], ['boolean', event.token, 'is-globbable', false, 12],
      ['default', event.token, 11], ['boolean', event.token, 'is-globbable', true, 12],
    ]);
    assert.deepEqual(event, before);
    for (const override of [{keyKind: `${keyKind}-unknown`}, {readerHandle: null},
      {readerHandle: 7}, {readerScheme: 'file'}, {readerId: '18446744073709551616'}]) {
      assert.throws(() => completeDefaultGlob({completeDefaultGlob() {
        assert.fail('invalid glob descriptor must not consume the token');
      }}, {...event, glob: {...event.glob, ...override}}), /glob|reader/);
    }
    assert.throws(() => completeDefaultGlob({completeDefaultGlob: () => 'false'}, event), /completion result/);
    assert.throws(() => completeGlobBoolean({completeGlobBoolean() {
      assert.fail('invalid boolean must not consume the token');
    }}, event, 'false'), /boolean completion/);
    assert.throws(() => completeDefaultGlob({completeDefaultGlob() {
      throw Error('stale glob token');
    }}, event), /stale glob token/);
  }
});
