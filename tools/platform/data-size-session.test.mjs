import test from 'node:test';
import assert from 'node:assert/strict';
import { runDataSizeSession } from './data-size-session.mjs';

const u = text => Uint16Array.from({ length: text.length }, (_, index) => text.charCodeAt(index));
const value = text => ({ kind: 'value', value: u(text) });
const failure = (text, disposition = 'language-error') => ({ kind: 'error', error: u(text),
  disposition, phase: 'evaluation', reason: 'observed' });
const fixture = rows => ({ uri: 'file:///stage57/datasize-oracle.pkl', source: 'marker = "stage57"\r\n',
  rows: rows.map(([id, expression]) => ({ id, expression })) });
const decoded = (result, index, name) => Buffer.from(result.rows[index].payloads[name]).toString('utf8');
let request = 0;
const moduleEvent = (kind, fields) => ({ kind: 'module-request', token: 'token-' + (++request),
  module: { id: String(request), kind, ...fields } });

function mock(programs, { corruptOnFree = false, defaultResult = true, startError = null, pollError = null,
  finalSlots = 0 } = {}) {
  const calls = [], owned = [];
  let next = 0, events = [], closed = false;
  const session = {
    evaluateModuleSource(...args) {
      calls.push(['evaluateModuleSource', ...args]);
      if (startError) throw startError;
      events = [...programs[next++]];
    },
    poll(...args) {
      calls.push(['poll', ...args]);
      if (pollError) throw pollError;
      const event = events.shift();
      if (!event) throw Error('unsettled mock query');
      if (event.value instanceof Uint16Array) owned.push(event.value);
      if (event.error instanceof Uint16Array) owned.push(event.error);
      return event;
    },
    collect() { calls.push(['collect']); },
    completeDefaultModule(...args) { calls.push(['completeDefaultModule', ...args]); return defaultResult; },
    close() { calls.push(['close']); closed = true; },
    heapSlots() { return finalSlots; }, contextDepth() { return closed ? 0 : 1; },
    free() { calls.push(['free']); if (corruptOnFree) owned.forEach(view => view.fill(0)); },
  };
  for (const method of ['completeModuleKey', 'completeModuleCachePolicy',
    'completeModuleFragmentPaths', 'completeModuleCapabilities'])
    session[method] = (...args) => calls.push([method, ...args]);
  return { calls, api: { SourceSession: { moduleEvaluator(...args) {
    calls.push(['moduleEvaluator', ...args]); return session;
  } } } };
}

test('one evaluator handles ordered queries and exact Java string protocol with owned UTF16', () => {
  const input = fixture([['first', '1.mb'], ['second', '2.mb.value']]);
  const m = mock([[{ kind: 'yield' }, value('1.mb\r\n\ud800')], [failure('Pkl Error\n\udc00')]]);
  const result = runDataSizeSession(m.api, input);
  assert.equal(result.acquisitionError, null);
  assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', false, false, false]);
  assert.deepEqual(m.calls.filter(call => call[0] === 'evaluateModuleSource'), input.rows.map(row =>
    ['evaluateModuleSource', input.uri, input.source, row.expression, 0, null]));
  assert.deepEqual(result.protocol, [
    'session-open mode=string rows=2', 'call-enter=string-0-first', 'call-exit=string-0-first status=success',
    'call-enter=string-1-second', 'call-exit=string-1-second status=error',
    'session-closed mode=string rows=2 public-calls=2 success=1 errors=1 datasize-rows=0 repeated-export-calls=0',
  ]);
  assert.equal(Buffer.from(result.protocolBytes).toString(), result.protocol.join('\n') + '\n');
  assert.equal(decoded(result, 0, 'result.stdout.bin'), '1.mb\r\n?');
  assert.deepEqual(result.rows[0].payloads['result.stdout.code-units.bin'].slice(-2), [0xd8, 0]);
  assert.equal(decoded(result, 1, 'result.stderr.bin'), 'Pkl Error\n?');
  assert.equal(decoded(result, 1, 'result.status.bin'), 'error\n');
  assert.equal(result.rows.flatMap(row => Object.keys(row.payloads)).some(name => name.includes('error-class')), false);
  assert.equal(result.architecture.terminalSlots, 0);
  assert.equal(result.architecture.terminalDepth, 0);
  assert.equal(result.architecture.retainedPayloadsStable, true);
  assert.equal(m.calls.filter(call => call[0] === 'moduleEvaluator').length, 1);
  assert.equal(m.calls.filter(call => call[0] === 'close').length, 1);
});

test('actual API failures and unsupported disposition remain distinct from language errors', () => {
  const m = mock([[failure('unsupported method', 'unsupported')], [value('unused')]], { pollError: null });
  const input = fixture([['unsupported', '1.mb.unknown']]);
  const candidate = runDataSizeSession(m.api, input);
  assert.equal(candidate.acquisitionError, null);
  assert.equal(decoded(candidate, 0, 'result.status.bin'), 'candidate-error\n');
  assert.equal(decoded(candidate, 0, 'candidate.disposition.bin'), 'unsupported');
  assert.equal(decoded(candidate, 0, 'result.stderr.bin'), 'unsupported method');
  const thrown = mock([], { startError: Object.assign(Error('real public API failure'), { name: 'ApiError' }) });
  const actual = runDataSizeSession(thrown.api, fixture([['api', '1.mb']]));
  assert.equal(actual.acquisitionError, null);
  assert.equal(decoded(actual, 0, 'result.status.bin'), 'api-exception\n');
  assert.equal(decoded(actual, 0, 'candidate.api-error-name.bin'), 'ApiError');
  assert.equal(decoded(actual, 0, 'result.stderr.bin'), 'real public API failure');
  assert.equal(Object.hasOwn(actual.rows[0].payloads, 'candidate.disposition.bin'), false);
});

test('primary Host descriptor, cache policy and capabilities are owned across queries', () => {
  const uri = 'file:///stage57/datasize-oracle.pkl', key = '18446744073709551615';
  const descriptor = { originalUri: uri, keyId: key, keyKind: 'host', keyHandle: `fixture-key:${key}` };
  const m = mock([[
    moduleEvent('select-key', { originalUri: uri, keyId: key, selectionPhase: 'primary' }),
    moduleEvent('query-cache-policy', descriptor),
    moduleEvent('query-fragment-paths', descriptor),
    moduleEvent('capabilities', { ...descriptor, importerOriginalUri: uri, importerResolvedUri: uri }),
    value('1.mb'),
  ], [moduleEvent('check-import', { originalUri: 'pkl:base', importUri: 'pkl:base',
    importerOriginalUri: uri, importerResolvedUri: uri }), value('2.mb')]]);
  const result = runDataSizeSession(m.api, fixture([['one', '1.mb'], ['two', '2.mb']]));
  assert.equal(result.acquisitionError, null);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleKey').slice(2), [key, uri, 'host', `fixture-key:${key}`, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleCachePolicy').slice(2), [key, true, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleFragmentPaths').slice(2), [key, false, 0]);
  assert.deepEqual(m.calls.find(call => call[0] === 'completeModuleCapabilities').slice(2), [true, true, false, 0]);
  assert.equal(m.calls.filter(call => call[0] === 'completeDefaultModule').length, 1);
});

test('unowned Host requests and unexpected effects fail acquisition before false row success', () => {
  const uri = 'file:///stage57/datasize-oracle.pkl';
  for (const event of [moduleEvent('query-cache-policy', { originalUri: uri, keyId: '42',
    keyKind: 'host', keyHandle: 'fixture-key:42' }), { kind: 'read', token: 'r', uri: 'file:///no' }]) {
    const m = mock([[event, value('false success')]]);
    const result = runDataSizeSession(m.api, fixture([['one', '1.mb']]));
    assert.notEqual(result.acquisitionError, null);
    assert.equal(result.rows[0].status, null);
    assert.equal(result.protocol.some(line => line.startsWith('session-closed')), false);
  }
});

test('closed heap and owned buffers are checked; malformed and duplicate query input is rejected', () => {
  const row = fixture([['one', '1.mb']]);
  assert.match(runDataSizeSession(mock([[value('1.mb')]], { finalSlots: 1 }).api, row).acquisitionError.message,
    /nonzero state/);
  assert.match(runDataSizeSession(mock([[value('1.mb')]], { corruptOnFree: true }).api, row).acquisitionError.message,
    /owned UTF16/);
  for (const bad of [fixture([['one', '1.mb'], ['one', '2.mb']]), fixture([['bad/id', '1.mb']]),
    { ...row, uri: 'https://example.invalid/x.pkl' }]) {
    const m = mock([]), result = runDataSizeSession(m.api, bad);
    assert.notEqual(result.acquisitionError, null);
    assert.equal(m.calls.length, 0);
  }
});
