import test from 'node:test';
import assert from 'node:assert/strict';
import { runClassValuesCase } from './class-values-session.mjs';

const u = value => Uint16Array.from({ length: value.length }, (_, index) => value.charCodeAt(index));
const value = text => ({ kind: 'value', value: u(text) });
const failure = (text, fields = {}) => ({ kind: 'error', error: u(text), disposition: 'language-error', phase: 'evaluation', ...fields });
const log = (token, level = 'trace', fields = {}) => ({ kind: 'log', token, log: {
  level, message: u('message'), moduleUri: u('file:///classes/main.pkl'), memberName: null,
  sourceLines: [u('trace(C)')], startLine: 1, startColumn: 1, endLine: 1, endColumn: 8, ...fields } });
const fixture = extra => ({ id: 'transport', source: 'exact source\r\n', moduleName: 'main', uri: 'file:///classes/main.pkl',
  expressions: ['C.simpleName', 'C == C'], support_modules: [],
  logger: { mode: 'capture', action: 'none', callback: '', callback_after: 0 }, ...extra });
const decoded = (row, name) => Buffer.from(row.payloads[name]).toString('utf8');
function mock(programs, { corruptOnFree = false, defaultResult = true, pollError = null } = {}) {
  const calls = [], frames = [], retained = [];
  let next = 0, closing = false;
  function remember(object) {
    if (object instanceof Uint16Array) retained.push(object);
    else if (object && typeof object === 'object') Object.values(object).forEach(remember);
  }
  const session = {
    evaluateModuleSource(...args) {
      calls.push(['evaluateModuleSource', ...args]);
      if (closing) throw Error('actual closed evaluator');
      frames.push([...programs[next++]]);
    },
    reenterModuleSource(...args) {
      calls.push(['reenterModuleSource', ...args]);
      if (closing) throw Error('actual closed evaluator');
      frames.push([...programs[next++]]);
    },
    poll(...args) {
      calls.push(['poll', ...args]);
      if (pollError) throw pollError;
      const event = frames.at(-1)?.shift();
      if (!event) throw Error('mock call did not settle');
      remember(event);
      if (['value', 'error'].includes(event.kind)) frames.pop();
      return event;
    },
    completeDefaultModule(...args) { calls.push(['completeDefaultModule', ...args]); return defaultResult; },
    closeFromCallback(...args) { calls.push(['closeFromCallback', ...args]); closing = true; },
    close() { calls.push(['close']); closing = true; frames.length = 0; },
    collect() {}, heapSlots() { return 0; }, contextDepth() { return frames.length; },
    free() { if (corruptOnFree) retained.forEach(view => view.fill(0)); },
  };
  for (const method of ['acknowledgeLog', 'completeModuleKey', 'completeModuleCachePolicy', 'completeModuleFragmentPaths',
    'completeModuleResolvedKey', 'completeModuleSource', 'completeModuleCapabilities'])
    session[method] = (...args) => calls.push([method, ...args]);
  return { calls, api: { SourceSession: { moduleEvaluator(...args) { calls.push(['moduleEvaluator', ...args]); return session; } } } };
}
let request = 0;
const moduleEvent = (kind, fields) => ({ kind: 'module-request', token: 'request-' + (++request),
  module: { id: String(request), kind, ...fields } });
const key = (id = '12', uri = 'file:///classes/child.pkl') => ({ originalUri: uri, keyId: id, keyKind: 'host', keyHandle: 'fixture-key:' + id });

test('exact expressions use public module evaluation, owned results and zero terminal state', () => {
  const m = mock([[value('first\r\n')], [value('second')]]), row = runClassValuesCase(m.api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', true, false, false]);
  assert.deepEqual(m.calls.filter(call => call[0] === 'evaluateModuleSource'),
    ['C.simpleName', 'C == C'].map(expression => ['evaluateModuleSource', 'file:///classes/main.pkl', 'exact source\r\n', expression, 0, null]));
  assert.deepEqual(row.payloads['outer-0.stdout.bin'], [...Buffer.from('first\r\n')]);
  assert.equal(row.architecture.terminalSlots, 0); assert.equal(row.architecture.terminalDepth, 0);
  assert.equal(row.architecture.retainedPayloadsStable, true);
  assert.equal(Object.keys(row.payloads).some(name => name.endsWith('error-class.bin')), false);
});

test('authored support sources are supplied only through owned Host keys and resolved keys', () => {
  const id = '18446744073709551614', resolvedId = '18446744073709551615', uri = 'file:///classes/child.pkl';
  const descriptor = key(id, uri);
  const m = mock([[
    moduleEvent('select-key', { originalUri: uri, keyId: id, selectionPhase: 'runtime-import' }),
    moduleEvent('query-cache-policy', descriptor), moduleEvent('query-fragment-paths', descriptor),
    moduleEvent('capabilities', { ...descriptor, importerOriginalUri: 'file:///classes/main.pkl', importerResolvedUri: 'file:///classes/main.pkl' }),
    moduleEvent('resolve-key', { ...descriptor, resolvedKeyId: resolvedId, resolutionPhase: 'query' }),
    moduleEvent('load-source', { ...descriptor, resolvedKeyId: resolvedId, resolvedKeyHandle: 'class-resolved:' + resolvedId,
      resolvedUri: uri, sourceUri: uri }), value('child.C'),
  ]]);
  const row = runClassValuesCase(m.api, fixture({ expressions: ['child.C.toString()'],
    support_modules: [{ uri, moduleName: 'child', source: 'module child\r\nclass C {}\r\n' }] }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleKey').slice(2), [id, uri, 'host', 'fixture-key:' + id, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleCachePolicy').slice(2), [id, true, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleFragmentPaths').slice(2), [id, false, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleCapabilities').slice(2), [false, true, false, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleResolvedKey').slice(2),
    [id, resolvedId, 'class-resolved:' + resolvedId, uri, uri, uri, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleSource').slice(2), ['module child\r\nclass C {}\r\n', 'child', 0]);
  assert.match(row.providerBoundary, /not executed or claimed/);
  assert.equal(Object.keys(row.payloads).some(name => name.startsWith('provider')), false);
});

test('supplied-text wrapper and import admission retain shared default policy', () => {
  const uri = 'file:///classes/main.pkl';
  const events = [moduleEvent('select-key', { originalUri: uri, keyId: '1', selectionPhase: 'primary' }),
    moduleEvent('query-fragment-paths', { originalUri: uri, keyId: '2', keyKind: 'supplied-text', keyHandle: '' }),
    moduleEvent('check-import', { originalUri: 'pkl:base', importUri: 'pkl:base', importerOriginalUri: uri, importerResolvedUri: uri }), value('ok')];
  const m = mock([events]), row = runClassValuesCase(m.api, fixture({ expressions: ['C.simpleName'] }));
  assert.equal(row.acquisitionError, null);
  assert.equal(m.calls.filter(call => call[0] === 'completeDefaultModule').length, 2);
  assert.equal(m.calls.some(call => call[0] === 'completeModuleFragmentPaths'), false);
});

test('unowned keys and mismatched resolved handles fail acquisition without a source completion', () => {
  const uri = 'file:///classes/child.pkl';
  const invalid = [
    [moduleEvent('query-cache-policy', key('5', uri))],
    [moduleEvent('select-key', { originalUri: uri, keyId: '5', selectionPhase: 'primary' }),
      moduleEvent('resolve-key', { ...key('5', uri), resolvedKeyId: '6', resolutionPhase: 'query' }),
      moduleEvent('load-source', { ...key('5', uri), resolvedKeyId: '6', resolvedKeyHandle: 'foreign', resolvedUri: uri, sourceUri: uri })],
  ];
  for (const events of invalid) {
    const m = mock([events]), row = runClassValuesCase(m.api, fixture({ expressions: ['C'], support_modules: [{ uri, source: 'class C {}\n', moduleName: 'child' }] }));
    assert.match(row.acquisitionError.message, /ownership mismatch/);
    assert.equal(m.calls.some(call => call[0] === 'completeModuleSource'), false);
    assert.equal(row.payloads['outer-0.status.bin'], undefined);
  }
});

test('shared policy refusal is preserved instead of silently allowing a request', () => {
  const m = mock([[moduleEvent('select-key', { originalUri: 'other:/missing', keyId: '1', selectionPhase: 'runtime-import' })]], { defaultResult: false });
  const row = runClassValuesCase(m.api, fixture({ expressions: ['missing.C'] }));
  assert.match(row.acquisitionError.message, /unconfigured module request/);
  assert.equal(row.payloads['outer-0.stdout.bin'], undefined);
});

test('nested trace consumes throw-once after reentry and callback is guarded before entry', () => {
  const m = mock([[log('outer'), value('outer')], [log('inner'), failure('actual callback failure', { disposition: 'internal-failure', callbackOrigin: 'log-delegate' })], [value('later')]]);
  const row = runClassValuesCase(m.api, fixture({ logger: { mode: 'capture', action: 'throw-once', callback: 'nested()', callback_after: 0 } }));
  assert.equal(row.acquisitionError, null);
  const acknowledgements = m.calls.filter(call => call[0] === 'acknowledgeLog');
  assert.deepEqual(acknowledgements.map(call => [call[1], call[2] == null ? null : Array.from(call[2])]),
    [['inner', Array.from(u('stage56-logger-failure'))], ['outer', null]]);
  assert.equal(m.calls.filter(call => call[0] === 'reenterModuleSource').length, 1);
  assert.deepEqual(row.protocol.slice(0, 8), ['call-enter=outer-0', 'log-enter=0 level=trace depth=0',
    'call-enter=log-callback-0', 'log-enter=1 level=trace depth=1', 'log-throw=1',
    'call-exit=log-callback-0 status=candidate-error', 'log-exit=0', 'call-exit=outer-0 status=success']);
  assert.equal(decoded(row, 'log-callback-0.callback-origin.bin'), 'log-delegate');
  assert.equal(decoded(row, 'log-callback-0.status.bin'), 'candidate-error\n');
});

test('logger threshold counts trace and warning entries together', () => {
  const m = mock([[log('trace'), log('warn', 'warn'), value('outer')], [value('nested')]]);
  const row = runClassValuesCase(m.api, fixture({ expressions: ['C'], logger: { mode: 'capture', action: 'none', callback: 'nested()', callback_after: 1 } }));
  assert.equal(row.acquisitionError, null);
  assert.equal(m.calls.find(call => call[0] === 'reenterModuleSource')[1], 'warn');
  assert.equal(decoded(row, 'log-callback-1.stdout.bin'), 'nested');
  assert.equal(row.protocol.at(-1), 'closed logs=2 warnings=1');
});

test('nested close is performed once, issued logs are acknowledged and later API calls attempted', () => {
  const m = mock([[log('outer'), value('outer')], [log('inner', 'warn'), value('nested')]]);
  const row = runClassValuesCase(m.api, fixture({ logger: { mode: 'capture', action: 'close', callback: 'nested()', callback_after: 0 } }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls.filter(call => call[0] === 'closeFromCallback'), [['closeFromCallback', 'inner']]);
  assert.deepEqual(m.calls.filter(call => call[0] === 'acknowledgeLog').map(call => call[1]), ['inner', 'outer']);
  assert.equal(decoded(row, 'outer-1.status.bin'), 'api-exception\n');
  assert.equal(decoded(row, 'outer-1.stderr.bin'), 'actual closed evaluator');
  assert.equal(row.protocol.includes('logger-close=1'), true);
  assert.equal(row.protocol.includes('logger-close=0'), false);
});

test('all frame UTF16 fields retain isolated surrogates, empty member and empty source lists', () => {
  const first = log('utf16', 'trace', { message: u('\ud800\ud83d\ude00'), moduleUri: u('\udc00'),
    memberName: new Uint16Array(), sourceLines: [], startLine: 0, endColumn: 0xffffffff });
  const second = log('line', 'warn', { sourceLines: [u('\ud800\r\n')] });
  const m = mock([[first, second, value('\udc00')]]), row = runClassValuesCase(m.api, fixture({ expressions: ['C'] }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(row.payloads['log-0.message.code-units.bin'], [0xd8, 0, 0xd8, 0x3d, 0xde, 0]);
  assert.deepEqual(row.payloads['log-0.message.bin'], [0x3f, 0xf0, 0x9f, 0x98, 0x80]);
  assert.equal(decoded(row, 'log-0.member.null.bin'), 'false');
  assert.equal(decoded(row, 'log-1.member.null.bin'), 'true');
  assert.equal(decoded(row, 'log-0.source-count.bin'), '0');
  assert.deepEqual(row.payloads['log-1.source-0.code-units.bin'], [0xd8, 0, 0, 13, 0, 10]);
  assert.deepEqual(row.payloads['outer-0.stdout.bin'], [0x3f]);
});

test('noop passes the disabled logger option without inventing logger callbacks', () => {
  const m = mock([[value('ok')]]), row = runClassValuesCase(m.api, fixture({ expressions: ['C'],
    logger: { mode: 'noop', action: 'none', callback: '', callback_after: 0 } }));
  assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', false, false, false]);
  assert.equal(row.protocol.at(-1), 'closed logs=0 warnings=0');
});

test('unconfigured effects and malformed frames remain acquisition failures', () => {
  for (const event of [{ kind: 'read', token: 'read', uri: 'file:///unknown' }, log('bad', 'trace', { memberName: undefined })]) {
    const m = mock([[event]]), row = runClassValuesCase(m.api, fixture({ expressions: ['C'] }));
    assert.notEqual(row.acquisitionError, null);
    assert.equal(row.payloads['outer-0.status.bin'], undefined);
  }
});

test('owned log buffers remain checked after freeing the session', () => {
  const m = mock([[log('log'), value('ok')]], { corruptOnFree: true });
  const row = runClassValuesCase(m.api, fixture({ expressions: ['C'] }));
  assert.equal(row.architecture.retainedPayloadsStable, false);
});

test('invalid or duplicate module and logger profiles are rejected before construction', () => {
  for (const extra of [{ support_modules: [{ uri: 'file:///classes/main.pkl', source: 'duplicate', moduleName: 'other' }] },
    { logger: { mode: 'noop', action: 'close', callback: '', callback_after: 0 } },
    { logger: { mode: 'capture', action: 'none', callback: '', callback_after: -1 } }]) {
    const m = mock([]), row = runClassValuesCase(m.api, fixture(extra));
    assert.notEqual(row.acquisitionError, null); assert.equal(m.calls.length, 0);
  }
});

test('file primary keys are local, support keys are nonlocal, and pkl primary selection delegates', () => {
  const uri = 'file:///classes/main.pkl';
  const mainEvents = [moduleEvent('select-key', { originalUri: uri, keyId: '7', selectionPhase: 'primary' }),
    moduleEvent('capabilities', { ...key('7', uri), importerOriginalUri: uri, importerResolvedUri: uri }), value('ok')];
  const main = mock([mainEvents]), row = runClassValuesCase(main.api, fixture({ expressions: ['C'] }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(main.calls.find(call => call[0] === 'completeModuleCapabilities').slice(2), [true, true, false, 0]);
  const pkl = mock([[moduleEvent('select-key', { originalUri: 'pkl:stage56Control', keyId: '8', selectionPhase: 'primary' }), value('ok')]]);
  const pklRow = runClassValuesCase(pkl.api, fixture({ uri: 'pkl:stage56Control', expressions: ['C'] }));
  assert.equal(pklRow.acquisitionError, null);
  assert.equal(pkl.calls.some(call => call[0] === 'completeModuleKey'), false);
  assert.equal(pkl.calls.filter(call => call[0] === 'completeDefaultModule').length, 1);
});

test('runtime import of the primary URI does not manufacture a support-module source', () => {
  const uri = 'file:///classes/main.pkl';
  const m = mock([[moduleEvent('select-key', { originalUri: uri, keyId: '9', selectionPhase: 'runtime-import' }), value('cached primary')]]);
  const row = runClassValuesCase(m.api, fixture({ expressions: ['self.C'] }));
  assert.equal(row.acquisitionError, null);
  assert.equal(m.calls.some(call => call[0] === 'completeModuleKey' || call[0] === 'completeModuleSource'), false);
  assert.equal(m.calls.filter(call => call[0] === 'completeDefaultModule').length, 1);
});

test('unowned Host fragment and import-URI requests never reach permissive shared defaults', () => {
  const uri = 'file:///classes/child.pkl';
  for (const event of [moduleEvent('query-fragment-paths', key('99', uri)),
    moduleEvent('resolve-import-uri', { ...key('99', uri), importUri: 'other.pkl' })]) {
    const m = mock([[event]]), row = runClassValuesCase(m.api, fixture({ expressions: ['C'],
      support_modules: [{ uri, source: 'class C {}\n', moduleName: 'child' }] }));
    assert.match(row.acquisitionError.message, /Host ModuleKey descriptor ownership mismatch/);
    assert.equal(m.calls.some(call => call[0] === 'completeDefaultModule'), false);
    assert.equal(m.calls.some(call => call[0] === 'completeModuleFragmentPaths'), false);
  }
});
