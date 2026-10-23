import test from 'node:test';
import assert from 'node:assert/strict';
import {runModuleKeyExtendedCase, compareModuleKeyExtendedCase} from './modulekey-extended-session.mjs';

// Transport/Host-input mocks only. No recorded oracle fixture tree is loaded.
const ID = 'unit/modulekey-extended', MAIN = 'file:///unit/main.pkl';
const REQUESTED = 'memo:/requested.pkl', SELECTED = 'memo:/selected.pkl', RESOLVED = 'memo:/resolved.pkl';
const units = text => Array.from({length: text.length}, (_, i) => text.charCodeAt(i));
function javaHex(value) {
  let text = '';
  for (let i = 0; i < value.length; i++) {
    const c = value[i], next = value[i + 1];
    if (c >= 0xd800 && c <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {text += String.fromCharCode(c, next); i++;}
    else text += c >= 0xd800 && c <= 0xdfff ? '?' : String.fromCharCode(c);
  }
  return Buffer.from(text).toString('hex');
}
const evaluate = (sourceProfileId = 'main.pkl', expression = 'value') => ({kind: 'evaluate', sourceProfileId, expression});
function expected(operation, change = {}) {
  return {status: 'success', statusRawHex: Buffer.from('success\n').toString('hex'),
    sourceProfileId: operation.sourceProfileId, scope: 'ordinary', errorClass: '', messageNull: false,
    stdoutUnits: [55], stdoutHex: '37', stderrUnits: [], stderrHex: '', ...change};
}
function fixture(operations = [evaluate()]) {
  const evaluations = operations.filter(o => o.kind === 'evaluate');
  return {id: ID, providerProfileId: ID, caseId: 'unit', oracleId: 'java01', cohort: 'java01', repeat: 0, session: 0,
    uri: MAIN, moduleName: 'main', source: 'value=7', operations,
    expressions: evaluations.map(o => o.expression), expected: evaluations.map(o => expected(o))};
}
function plan(change = {}) {
  return {requestedUri: REQUESTED, selectedUri: SELECTED, resolvedUri: RESOLVED, sourceUri: SELECTED,
    keyKind: 'host', sourceId: 'good', sourceMode: 'fixed', cacheMode: 'dynamic', fragmentPaths: true,
    inferredName: 'host-source-metadata-only', ...change};
}
function profile(change = {}) {
  return {id: ID, initial: {cached: true, customFactory: true, sourceState: 'good', policy: 'allow'},
    sources: {'main.pkl': {kind: 'text', text: 'value=7'}, good: {kind: 'text', text: 'tag="good"'},
      two: {kind: 'text', text: 'tag="two"'}, empty: {kind: 'text', text: ''}},
    sourceProfiles: {'main.pkl': {kind: 'uri', uri: MAIN, contents: null},
      supplied: {kind: 'contents', uri: REQUESTED, sourceId: 'good'},
      empty: {kind: 'contents', uri: REQUESTED, sourceId: 'empty'},
      repl: {kind: 'repl', uri: 'repl:text', sourceId: 'good'}},
    customModules: [plan()], fileModules: [], capabilities: [{selectedUri: SELECTED, local: true, hierarchical: true, fragmentPaths: true}],
    elements: [], elementDefaults: [], uriOverrides: [], policyRules: [], rawCauses: [], ...change};
}
const select = (id = '9007199254740993', uri = REQUESTED, selectionPhase = 'runtime-import') =>
  ({kind: 'select-key', originalUri: uri, keyId: id, selectionPhase});
const descriptor = (session, id) => {
  const key = session.keys.get(id); assert.ok(key, 'test requested an unselected custom descriptor');
  return {keyId: id, keyKind: key.kind, keyHandle: key.handle};
};
const getter = (kind, id = '9007199254740993') => session => ({kind, originalUri: session.keys.get(id).uri, ...descriptor(session, id)});
const resolveKey = (id = '9007199254740993', resolvedId = '18446744073709551614', phase = null) => session => ({
  kind: phase ? 'resolve-key' : 'resolve', ...descriptor(session, id), resolvedKeyId: resolvedId,
  originalUri: phase ? session.keys.get(id).uri : REQUESTED,
  ...(phase ? {resolutionPhase: phase} : {importUri: REQUESTED, importerOriginalUri: MAIN, importerResolvedUri: 'file:///unit/resolved-main.pkl'}),
});
const loadSource = (id = '9007199254740993', resolvedId = '18446744073709551614') => session => {
  const resolved = session.resolved.get(resolvedId); assert.ok(resolved, 'test needs a resolved key');
  return {kind: 'load-source', ...descriptor(session, id), resolvedKeyId: resolvedId, resolvedKeyHandle: resolved.handle,
    originalUri: resolved.original, resolvedUri: resolved.resolved, sourceUri: resolved.source};
};
const context = {importerOriginalUri: 'memo:/wrapper-module.pkl', importerResolvedUri: 'memo:/resolved-wrapper.pkl'};
const capabilities = (id = '9007199254740993') => session => ({kind: 'capabilities', originalUri: context.importerOriginalUri, ...context, ...descriptor(session, id)});
const hasElement = (candidate, id = '9007199254740993') => session => ({kind: 'has-element', originalUri: candidate, ...context, ...descriptor(session, id)});
const importUri = (authored = './child.pkl', id = '9007199254740993') => session => ({kind: 'resolve-import-uri', originalUri: session.keys.get(id).uri, importUri: authored, ...descriptor(session, id)});

function mock(programs, options = {}) {
  const calls = [], sessions = [], payloads = [];
  class Session {
    constructor() {this.nextId = 1; this.nextCall = 0; this.pending = null; this.keys = new Map(); this.resolved = new Map(); sessions.push(this);}
    static moduleEvaluator(...args) {calls.push(['moduleEvaluator', ...args]); return new Session();}
    evaluateModuleSource(...args) {
      assert.equal(this.pending, null, 'API operation replaced a pending request');
      calls.push(['evaluateModuleSource', ...args]); this.queue = [...(programs[this.nextCall] ?? [])]; this.call = this.nextCall++;
    }
    poll() {
      assert.ok(!this.pending, 'adapter polled without completing its pending request');
      const entry = this.queue.shift();
      if (entry) {
        const module = typeof entry === 'function' ? entry(this) : {...entry};
        const id = module.id ?? String(this.nextId++), token = module.testToken ?? `token-${id}`;
        delete module.testToken;
        this.pending = {token, module: {id, ...module}};
        return {kind: 'module-request', ...this.pending};
      }
      const data = options.errorCalls?.includes(this.call) ? Uint16Array.of(120) : Uint16Array.of(55);
      payloads.push(data);
      return options.errorCalls?.includes(this.call)
        ? {kind: 'error', error: data, disposition: Object.hasOwn(options, 'disposition') ? options.disposition : 'language-error', phase: 'evaluation', reason: null}
        : {kind: 'value', value: data};
    }
    consume(method, args, mutate = () => {}) {
      assert.ok(this.pending); assert.equal(args[0], this.pending.token);
      calls.push([method, ...args]);
      if (options.throwAt === method) throw Error('stale completion token');
      mutate(); this.pending = null;
    }
    completeModuleKey(...args) {this.consume('completeModuleKey', args, () => this.keys.set(args[1], {uri: args[2], kind: args[3], handle: args[4]}));}
    completeModuleResolvedKey(...args) {this.consume('completeModuleResolvedKey', args, () => this.resolved.set(args[2], {handle: args[3], original: args[4], resolved: args[5], source: args[6]}));}
    completeDefaultModule(...args) {
      assert.equal(args[0], this.pending?.token); calls.push(['completeDefaultModule', ...args]);
      const value = options.defaultModule ? options.defaultModule(this.pending.module, this) : false;
      if (value === true) this.pending = null;
      return value;
    }
    collect() {calls.push(['collect']);}
    close() {calls.push(['close']); if (options.throwOnClose) throw Error('close failed'); this.pending = null; this.closed = true;}
    heapSlots() {return this.closed ? 0 : 1;}
    free() {calls.push(['free']); if (options.invalidateAtFree) for (const p of payloads) p[0] = 0;}
  }
  // Do not narrow IDs, reinterpret source strings or manufacture engine results.
  for (const name of ['completeModuleCachePolicy', 'completeModuleFragmentPaths', 'completeModuleSource',
    'completeImportUri', 'completeModuleCapabilities', 'completeModuleHasElement', 'rejectModuleKey',
    'rejectModuleResolution', 'rejectModuleImportCheck', 'rejectModuleSource']) {
    Session.prototype[name] = function (...args) {this.consume(name, args);};
  }
  return {api: {SourceSession: Session}, calls, sessions};
}
const completed = (m, name) => m.calls.filter(row => row[0] === name);
const run = (m, f = fixture(), p = profile()) => runModuleKeyExtendedCase(m.api, f, p, 7);

test('public API uses operation source profiles and distinguishes absent contents from an empty string', () => {
  const f = fixture([evaluate(), evaluate('empty'), evaluate('repl')]), m = mock([[], [], []]);
  const actual = run(m, f);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'evaluateModuleSource'), [
    ['evaluateModuleSource', MAIN, null, 'value', 0], ['evaluateModuleSource', REQUESTED, '', 'value', 0],
    ['evaluateModuleSource', 'repl:text', 'tag="good"', 'value', 0],
  ]);
  assert.deepEqual(actual.calls.map(call => call.sourceProfileId), ['main.pkl', 'empty', 'repl']);
  assert.equal(compareModuleKeyExtendedCase(f, actual).fullLifecyclePassed, false);
});

test('execution never reads Java expectations to decide Host replies', () => {
  const f = fixture(); Object.defineProperty(f, 'expected', {get() {throw Error('oracle expectations read during execution');}});
  const m = mock([[]]), actual = run(m, f);
  assert.equal(actual.acquisitionError, null); assert.equal(actual.calls.length, 1);
});

test('phase-specific URIs preserve selected, import-original, resolved and source identities', () => {
  const m = mock([[select(), getter('query-cache-policy'), resolveKey(), loadSource()]]), actual = run(m);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'completeModuleResolvedKey')[0].slice(5), [SELECTED, RESOLVED, SELECTED, 0]);
  assert.deepEqual(completed(m, 'completeModuleSource')[0].slice(2), ['tag="good"', 'host-source-metadata-only', 0]);
  assert.equal(actual.requests.length, 4);
  assert.equal(completed(m, 'completeModuleCachePolicy')[0][2], '9007199254740993');
});

test('cache and source-at-factory snapshots survive state changes and disabling custom selection', () => {
  const f = fixture([evaluate(), {kind: 'cache', value: 'off'}, {kind: 'source', value: 'two'},
    {kind: 'factory', value: 'builtin'}, evaluate()]);
  const p = profile({customModules: [plan({keyKind: 'synthetic', sourceMode: 'state-at-factory'})]});
  const m = mock([[select()], [getter('query-cache-policy'), resolveKey(), loadSource()]]), actual = run(m, f, p);
  assert.equal(actual.acquisitionError, null);
  assert.equal(completed(m, 'completeModuleKey')[0][4], 'synthetic');
  assert.equal(completed(m, 'completeModuleCachePolicy')[0][3], true);
  assert.equal(completed(m, 'completeModuleSource')[0][2], 'tag="good"');
});

test('state-at-load reads the current immutable source entry, not the factory-time state', () => {
  const f = fixture([evaluate(), {kind: 'source', value: 'two'}, evaluate()]);
  const m = mock([[select()], [resolveKey(), loadSource()]]);
  const actual = run(m, f, profile({customModules: [plan({sourceMode: 'state-at-load', sourceId: '$state'})]}));
  assert.equal(actual.acquisitionError, null); assert.equal(completed(m, 'completeModuleSource')[0][2], 'tag="two"');
});

test('an unused initial source state need not exist before the first source operation', () => {
  const f = fixture([{kind: 'source', value: 'one'}, evaluate()]);
  const p = profile({sources: {one: {kind: 'text', text: 'one'}},
    customModules: [plan({keyKind: 'synthetic', sourceMode: 'state-at-factory', sourceId: '$state'})]});
  const m = mock([[select(), resolveKey(), loadSource()]]), actual = run(m, f, p);
  assert.equal(actual.acquisitionError, null); assert.equal(completed(m, 'completeModuleSource')[0][2], 'one');
});

test('factory builtin selection delegates new keys while existing owned keys remain usable', () => {
  const f = fixture([evaluate(), {kind: 'factory', value: 'builtin'}, evaluate()]);
  const m = mock([[select()], [getter('query-cache-policy'), select('22', 'pkl:json')]], {defaultModule: () => true});
  const actual = run(m, f);
  assert.equal(actual.acquisitionError, null); assert.equal(completed(m, 'completeModuleKey').length, 1);
  assert.equal(completed(m, 'completeModuleCachePolicy')[0][3], true); assert.equal(completed(m, 'completeDefaultModule').length, 1);
});

test('capabilities and elements use registered selected URI while module and candidate URIs differ', () => {
  const yes = 'memo:/candidate/yes.pkl', no = 'memo:/candidate/no.pkl';
  const p = profile({elements: [{selectedUri: SELECTED, candidateUri: yes, exists: true}],
    elementDefaults: [{selectedUri: SELECTED, exists: false}]});
  const m = mock([[select(), capabilities(), hasElement(yes), hasElement(no)]]), actual = run(m, fixture(), p);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'completeModuleCapabilities')[0].slice(2), [true, true, true, 0]);
  assert.deepEqual(completed(m, 'completeModuleHasElement').map(c => c.slice(2)), [[true, 0], [false, 0]]);
});

test('missing explicit element/default cannot be silently completed as false', () => {
  const m = mock([[select(), hasElement('memo:/undeclared.pkl')]], {defaultModule: () => assert.fail('owned membership must not fall through')});
  const actual = run(m);
  assert.notEqual(actual.acquisitionError, null); assert.equal(completed(m, 'completeModuleHasElement').length, 0);
  assert.equal(actual.requests.length, 2); assert.equal(actual.architecture.terminalSlots, 0);
});

test('exact declaration-key override runs only for the registered key and authored operand', () => {
  const p = profile({uriOverrides: [{selectedUri: SELECTED, authoredUri: './child.pkl', absoluteUri: 'memo:/override.pkl'}]});
  const m = mock([[select(), importUri(), importUri('./different.pkl')]], {defaultModule: () => true});
  const actual = run(m, fixture(), p);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'completeImportUri')[0].slice(2), ['9007199254740993', 'memo:/override.pkl', 0]);
  assert.equal(completed(m, 'completeDefaultModule').length, 1);
});

function policyProfile(message = 'raw denied', messageNull = false) {
  const messageUnits = messageNull ? [] : units(message);
  return profile({policyRules: [{mode: 'deny-repl', kind: 'module', match: {prefix: 'repl:'}},
    {mode: 'deny-supplied', kind: 'module', match: {equals: SELECTED}}],
    rawCauses: [{mode: 'deny-repl', kind: 'module', context: 'repl:text', policyCallKind: 'checkResolveModule',
      contextSource: 'actual selected/policy URI', message: messageNull ? null : message, messageNull,
      messageUnits, messageJavaUtf8Hex: javaHex(messageUnits)},
    {mode: 'deny-supplied', kind: 'module', context: SELECTED, policyCallKind: 'checkResolveModule',
      contextSource: 'actual selected/policy URI', message: messageNull ? null : message, messageNull,
      messageUnits, messageJavaUtf8Hex: javaHex(messageUnits)}]});
}
test('deny modes do not turn allowed-module patterns into import-edge trust checks', () => {
  const edge = {kind: 'check-import', originalUri: SELECTED, importUri: SELECTED,
    importerOriginalUri: MAIN, importerResolvedUri: 'file:///unit/resolved.pkl'};
  const f = fixture([{kind: 'policy', value: 'deny-supplied'}, evaluate()]);
  const m = mock([[edge, select(), resolveKey()]], {defaultModule: () => true});
  const actual = run(m, f, policyProfile());
  assert.equal(actual.acquisitionError, null); assert.equal(completed(m, 'rejectModuleImportCheck').length, 0);
  assert.equal(completed(m, 'completeDefaultModule').length, 1);
  assert.deepEqual(completed(m, 'rejectModuleResolution')[0].slice(2), ['security-manager-exception', 'raw denied', 0]);
});

test('internal query Synthetic resolution applies deny-repl rather than bypassing policy', () => {
  const request = {kind: 'resolve-key', originalUri: 'repl:text', keyId: '88', keyKind: 'synthetic', keyHandle: '',
    resolvedKeyId: '89', resolutionPhase: 'query'};
  const m = mock([[request]], {defaultModule: () => assert.fail('denied internal query must not complete default')});
  const actual = run(m, fixture([{kind: 'policy', value: 'deny-repl'}, evaluate()]), policyProfile());
  assert.equal(actual.acquisitionError, null); assert.equal(completed(m, 'rejectModuleResolution').length, 1);
});

test('supplied wrapper bypasses delegate override and resolve policy while delegating capabilities by key identity', () => {
  const wrapper = {keyId: '77', keyKind: 'supplied-text', keyHandle: '', originalUri: SELECTED};
  const p = policyProfile(); p.uriOverrides = [{selectedUri: SELECTED, authoredUri: './child.pkl', absoluteUri: 'memo:/wrong-delegate-override'}];
  const m = mock([[select('9007199254740993', REQUESTED, 'primary'),
    {kind: 'query-fragment-paths', ...wrapper},
    {kind: 'resolve-import-uri', ...wrapper, importUri: './child.pkl'},
    {kind: 'resolve-key', ...wrapper, resolvedKeyId: '78', resolutionPhase: 'primary'}, capabilities()]], {defaultModule: () => true});
  const actual = run(m, fixture([{kind: 'policy', value: 'deny-supplied'}, evaluate('supplied')]), p);
  assert.equal(actual.acquisitionError, null); assert.equal(completed(m, 'rejectModuleResolution').length, 0);
  assert.equal(completed(m, 'completeImportUri').length, 0); assert.equal(completed(m, 'completeDefaultModule').length, 3);
  assert.deepEqual(completed(m, 'completeModuleCapabilities')[0].slice(2), [true, true, true, 0]);
});

test('raw policy cause preserves null and unmatched UTF16 instead of a formatted diagnostic', () => {
  for (const [message, isNull] of [['', true], [String.fromCharCode(0xd800, 0, 120), false]]) {
    const m = mock([[{kind: 'resolve-key', originalUri: 'repl:text', keyId: '88', keyKind: 'synthetic', keyHandle: '',
      resolvedKeyId: '89', resolutionPhase: 'query'}]]);
    const actual = run(m, fixture([{kind: 'policy', value: 'deny-repl'}, evaluate()]), policyProfile(message, isNull));
    assert.equal(actual.acquisitionError, null);
    assert.equal(completed(m, 'rejectModuleResolution')[0][3], isNull ? null : message);
  }
});

test('source IO uses its own source phase and raw configured message', () => {
  const p = profile({sources: {good: {kind: 'io', message: 'stage49-source-io', messageUnits: units('stage49-source-io'),
    messageJavaUtf8Hex: javaHex(units('stage49-source-io')), encodingProvenance: 'ascii-input-derived-not-independent-utf16'}}});
  const m = mock([[select(), resolveKey(), loadSource()]]), actual = run(m, fixture(), p);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'rejectModuleSource')[0].slice(2), ['io', 'stage49-source-io', 0]);
  assert.equal(completed(m, 'rejectModuleResolution').length, 0); assert.equal(completed(m, 'completeModuleSource').length, 0);
});

test('source IO preserves null and unmatched UTF16 through the source callback', () => {
  for (const message of [null, String.fromCharCode(0xd800, 0, 120)]) {
    const messageUnits = message === null ? [] : units(message);
    const p = profile({sources: {good: {kind: 'io', message, messageNull: message === null,
      messageUnits, messageJavaUtf8Hex: javaHex(messageUnits), encodingProvenance: 'mock-transport-only'}}});
    const m = mock([[select(), resolveKey(), loadSource()]]), actual = run(m, fixture(), p);
    assert.equal(actual.acquisitionError, null);
    assert.deepEqual(completed(m, 'rejectModuleSource')[0].slice(2), ['io', message, 0]);
    assert.equal(completed(m, 'rejectModuleResolution').length, 0);
  }
});

function ordinaryErrorFixture() {
  const f = fixture();
  f.expected[0] = expected(f.operations[0], {status: 'error', statusRawHex: Buffer.from('error\n').toString('hex'),
    stdoutUnits: [], stdoutHex: '', stderrUnits: [120], stderrHex: '78', errorClass: 'org.pkl.core.PklException'});
  return f;
}

test('missing or unknown error classification retains raw results but fails acquisition', () => {
  for (const disposition of [null, undefined, 'unknown-classification']) {
    const f = ordinaryErrorFixture(), m = mock([[]], {errorCalls: [0], disposition}), actual = run(m, f);
    assert.notEqual(actual.acquisitionError, null);
    assert.equal(actual.calls.length, 1); assert.deepEqual(actual.calls[0].stderrUnits, [120]);
    assert.equal(actual.observations.filter(o => o.kind === 'api-result').length, 1);
    const comparison = compareModuleKeyExtendedCase(f, actual);
    assert.equal(comparison.rawApiPassed, true); assert.equal(comparison.acquisitionIntegrityPassed, false);
    assert.equal(comparison.ordinaryCompatibilityPassed, false); assert.equal(comparison.intendedOrdinaryMatched, 0);
    assert.deepEqual(comparison.classificationFailures, ['classification-0']);
    assert.equal(actual.architecture.terminalSlots, 0); assert.equal(completed(m, 'free').length, 1);
  }
});

test('classified execution limit preserves raw equality and acquisition without ordinary compatibility', () => {
  const f = ordinaryErrorFixture(), m = mock([[]], {errorCalls: [0], disposition: 'limit-exceeded'}), actual = run(m, f);
  assert.equal(actual.acquisitionError, null);
  const comparison = compareModuleKeyExtendedCase(f, actual);
  assert.equal(comparison.rawApiPassed, true); assert.equal(comparison.acquisitionIntegrityPassed, true);
  assert.equal(comparison.ordinaryCompatibilityPassed, false); assert.equal(comparison.intendedOrdinaryMatched, 0);
  assert.deepEqual(comparison.classificationFailures, []);
});

test('standalone comparison rejects forged missing classification even when reported acquisition was clean', () => {
  const f = ordinaryErrorFixture(), actual = run(mock([[]], {errorCalls: [0]}), f);
  assert.equal(actual.acquisitionError, null); delete actual.calls[0].disposition;
  const comparison = compareModuleKeyExtendedCase(f, actual);
  assert.equal(comparison.rawApiPassed, true); assert.equal(comparison.acquisitionIntegrityPassed, false);
  assert.equal(comparison.ordinaryCompatibilityPassed, false); assert.deepEqual(comparison.classificationFailures, ['classification-0']);
});

test('configured element membership does not acquire a resolve-policy check', () => {
  const p = policyProfile(); p.elements = [{selectedUri: SELECTED, candidateUri: 'memo:/present.pkl', exists: true}];
  const m = mock([[select(), hasElement('memo:/present.pkl')]], {defaultModule: () => assert.fail('membership is configured')});
  const actual = run(m, fixture([{kind: 'policy', value: 'deny-supplied'}, evaluate()]), p);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'completeModuleHasElement')[0].slice(2), [true, 0]);
  assert.equal(completed(m, 'rejectModuleResolution').length, 0);
});

test('internal REPL primary resolution is admitted only with its actual source profile', () => {
  const request = {kind: 'resolve-key', originalUri: 'repl:text', keyId: '88', keyKind: 'synthetic', keyHandle: '',
    resolvedKeyId: '89', resolutionPhase: 'primary'};
  const m = mock([[request]], {defaultModule: () => true}), actual = run(m, fixture([evaluate('repl')]));
  assert.equal(actual.acquisitionError, null);
  assert.equal(completed(m, 'completeDefaultModule').length, 1);
  assert.equal(completed(m, 'completeModuleKey').length, 0);
});

test('factory IOException precedes disabled custom Plan lookup', () => {
  const p = profile({initial: {cached: true, customFactory: false, sourceState: 'good', policy: 'allow'},
    factoryFailures: [{scheme: 'throwing', errorKind: 'io', message: 'stage48-factory-create'}]});
  const m = mock([[select('9', 'throwing:x', 'clause-preparation')]], {defaultModule: () => assert.fail('factory IOException precedes default')});
  const actual = run(m, fixture(), p);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'rejectModuleKey')[0].slice(2), [false, 'io', 'stage48-factory-create', 0]);
});

test('IDs are not narrowed and resolved IDs cannot substitute another key handle', () => {
  const id = '9007199254740993';
  const m = mock([[select(id), resolveKey(id, id), loadSource(id, id)]]), actual = run(m);
  assert.equal(actual.acquisitionError, null);
  assert.equal(completed(m, 'completeModuleResolvedKey')[0][2], id);
  assert.equal(completed(m, 'completeModuleResolvedKey')[0][3], id);
});

test('malformed IDs, partial descriptors and phase contexts are retained without completion', () => {
  const cases = [select('18446744073709551616'), {...select(), selectionPhase: 'query'},
    {kind: 'query-cache-policy', originalUri: SELECTED, keyId: '7'},
    {kind: 'capabilities', originalUri: MAIN, keyId: '7', keyKind: 'host', keyHandle: 'foreign'},
    {kind: 'resolve-key', originalUri: 'repl:text', keyId: '8', keyKind: 'synthetic', keyHandle: 'foreign', resolvedKeyId: '9', resolutionPhase: 'query'},
    {kind: 'resolve-key', originalUri: 'repl:text', keyId: '8', keyKind: 'synthetic', keyHandle: '', resolvedKeyId: '9', resolutionPhase: 'primary'}];
  for (const request of cases) {
    const m = mock([[request]]), actual = run(m);
    assert.notEqual(actual.acquisitionError, null); assert.equal(actual.requests.length, 1);
    assert.equal(m.calls.filter(c => /^(complete|reject)/.test(c[0])).length, 0);
    assert.equal(actual.architecture.terminalSlots, 0);
  }
});

test('source descriptor tampering cannot be hidden by equal URI strings or bytes', () => {
  for (const change of [{keyHandle: 'foreign'}, {resolvedKeyHandle: 'foreign'}, {sourceUri: RESOLVED}, {resolvedUri: SELECTED}]) {
    const request = session => ({...loadSource()(session), ...change});
    const m = mock([[select(), resolveKey(), request]]), actual = run(m);
    assert.notEqual(actual.acquisitionError, null); assert.equal(completed(m, 'completeModuleSource').length, 0);
  }
});

test('a failed later acquisition preserves earlier raw calls and still closes/releases results', () => {
  const m = mock([[], [{kind: 'query-cache-policy', originalUri: SELECTED, keyId: '66', keyKind: 'host', keyHandle: 'foreign'}]]);
  const f = fixture([evaluate(), evaluate()]), actual = run(m, f);
  assert.notEqual(actual.acquisitionError, null); assert.equal(actual.calls.length, 1);
  assert.deepEqual(actual.calls[0].stdoutUnits, [55]); assert.equal(actual.architecture.terminalSlots, 0);
  assert.equal(completed(m, 'close').length, 1); assert.equal(completed(m, 'free').length, 1);
  const c = compareModuleKeyExtendedCase(f, actual); assert.equal(c.acquisitionIntegrityPassed, false);
});

test('callback exceptions are visible acquisitions and free-time result invalidation fails cleanup', () => {
  const stale = mock([[select()]], {throwAt: 'completeModuleKey'}), failed = run(stale);
  assert.notEqual(failed.acquisitionError, null); assert.match(failed.acquisitionError.message, /stale completion token/);
  assert.equal(failed.architecture.terminalSlots, 0);
  const f = fixture(), m = mock([[]], {invalidateAtFree: true}), actual = run(m, f);
  assert.equal(actual.architecture.ownedResultsStable, false);
  assert.equal(compareModuleKeyExtendedCase(f, actual).cleanupPassed, false);
});

test('comparison separates ordinary, diagnostic intent and internal raw matches without normalizing output', () => {
  const f = fixture([evaluate(), evaluate(), evaluate()]);
  for (const index of [1, 2]) f.expected[index] = {...f.expected[index], status: 'error', statusRawHex: '6572726f720a',
    stdoutUnits: [], stdoutHex: '', stderrUnits: [120], stderrHex: '78',
    scope: index === 1 ? 'intent-not-reached-diagnostic' : 'internal-boundary',
    errorClass: index === 1 ? 'org.pkl.core.PklException' : 'org.pkl.core.PklBugException'};
  const m = mock([[], [], []], {errorCalls: [1, 2]}), actual = run(m, f), c = compareModuleKeyExtendedCase(f, actual);
  assert.equal(c.intendedOrdinaryMatched, 1); assert.equal(c.diagnosticOnlyMatched, 1); assert.equal(c.internalBoundaryRawMatched, 1);
  assert.equal(c.ordinaryCompatibilityPassed, true); assert.equal(c.fullLifecyclePassed, false);
  assert.equal(c.cases[1].ordinaryCompatible, false); assert.equal(c.cases[2].ordinaryCompatible, false);
  actual.calls[0].stdoutUnits.push(10);
  assert.equal(compareModuleKeyExtendedCase(f, actual).rawApiPassed, false);
});

test('equal raw ordinary errors cannot turn explicit Unsupported into ordinary compatibility', () => {
  const f = fixture(); f.expected[0] = {...f.expected[0], status: 'error', statusRawHex: '6572726f720a',
    errorClass: 'org.pkl.core.PklException', stdoutUnits: [], stdoutHex: '', stderrUnits: [120], stderrHex: '78'};
  const m = mock([[]], {errorCalls: [0], disposition: 'unsupported'}), actual = run(m, f);
  const c = compareModuleKeyExtendedCase(f, actual);
  assert.equal(c.rawApiPassed, true); assert.equal(c.ordinaryCompatibilityPassed, false);
  assert.equal(c.intendedOrdinaryMatched, 0); assert.equal(c.cases[0].disposition, 'unsupported');
});

test('a required denial without an exact cause fails acquisition rather than default-allowing or inventing a message', () => {
  const p = policyProfile(); p.rawCauses = [];
  const m = mock([[{kind: 'resolve-key', originalUri: 'repl:text', keyId: '88', keyKind: 'synthetic', keyHandle: '',
    resolvedKeyId: '89', resolutionPhase: 'query'}]], {defaultModule: () => assert.fail('missing cause must not allow')});
  const actual = run(m, fixture([{kind: 'policy', value: 'deny-repl'}, evaluate()]), p);
  assert.notEqual(actual.acquisitionError, null); assert.equal(completed(m, 'rejectModuleResolution').length, 0);
});

test('raw cause bytes and explicit null flags are authenticated before any completion', () => {
  for (const mutate of [p => {p.rawCauses[0].messageUnits = [0xd800];},
    p => {p.rawCauses[0].messageJavaUtf8Hex = '00';},
    p => {p.rawCauses[0] = {...p.rawCauses[0], message: null, messageNull: false, messageUnits: [], messageJavaUtf8Hex: ''};}]) {
    const p = policyProfile(); mutate(p);
    const m = mock([[]]), actual = run(m, fixture(), p);
    assert.notEqual(actual.acquisitionError, null); assert.equal(m.calls.filter(c => /^(complete|reject)/.test(c[0])).length, 0);
  }
});

test('wrapper spelling without an observed supplied primary selection cannot create ownership', () => {
  const forged = {kind: 'query-fragment-paths', originalUri: SELECTED, keyId: '77', keyKind: 'supplied-text', keyHandle: ''};
  const m = mock([[forged]], {defaultModule: () => assert.fail('unowned wrapper must not default')});
  const actual = run(m, fixture([evaluate('supplied')]));
  assert.notEqual(actual.acquisitionError, null); assert.equal(actual.requests.length, 1);
});

test('replayed request IDs or tokens remain visible and cannot consume a later getter', () => {
  for (const mutation of [{id: '1'}, {testToken: 'token-1'}]) {
    const request = session => ({...getter('query-cache-policy')(session), ...mutation});
    const m = mock([[select(), request]]), actual = run(m);
    assert.notEqual(actual.acquisitionError, null); assert.equal(actual.requests.length, 2);
    assert.equal(completed(m, 'completeModuleCachePolicy').length, 0);
  }
});

test('default false preserves the same request for an explicitly configured no-loader rejection', () => {
  const p = profile({customModules: [], noLoaderUris: ['noLoader:x']});
  const m = mock([[select('99', 'noLoader:x')]]), actual = run(m, fixture(), p);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(completed(m, 'completeDefaultModule')[0].slice(1), ['token-1', 0]);
  assert.deepEqual(completed(m, 'rejectModuleKey')[0].slice(1), ['token-1', false, 'no-loader', null, 0]);
});

test('a nonboolean default result cannot masquerade as an unconsumed request', () => {
  for (const value of [undefined, null, 0, 'false']) {
    const m = mock([[select('99', 'pkl:json')]], {defaultModule: () => value}), actual = run(m);
    assert.notEqual(actual.acquisitionError, null); assert.equal(completed(m, 'rejectModuleKey').length, 0);
    assert.equal(actual.architecture.terminalSlots, 0);
  }
});

test('close failure remains visible while the owned session wrapper is still freed', () => {
  const m = mock([[]], {throwOnClose: true}), actual = run(m);
  assert.notEqual(actual.acquisitionError, null); assert.match(actual.acquisitionError.message, /close failed/);
  assert.equal(completed(m, 'free').length, 1);
  assert.equal(compareModuleKeyExtendedCase(fixture(), actual).cleanupPassed, false);
});
