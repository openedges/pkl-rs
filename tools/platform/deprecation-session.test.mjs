import test from 'node:test';
import assert from 'node:assert/strict';
import { runDeprecationCase } from './deprecation-session.mjs';

const u = value => Uint16Array.from({ length: value.length }, (_, i) => value.charCodeAt(i));
const value = text => ({ kind: 'value', value: u(text) });
const failure = (text, disposition = 'language-error') => ({ kind: 'error', error: u(text), disposition, phase: 'evaluation' });
const warning = (token, message = 'old', level = 'warn') => ({ kind: 'log', token,
  log: { level, message: u(message), moduleUri: u('file:///exact.pkl'), memberName: null,
    sourceLines: [u('old()')], startLine: 1, startColumn: 1, endLine: 1, endColumn: 5 } });
const fixture = extra => ({ id: 'transport', source: 'exact source\r\n', uri: 'file:///exact.pkl',
  action: 'capture', callback: '', callback_after: 0, expressions: ['old()', 'old()'], ...extra });

function mock(programs, { corruptOnFree = false } = {}) {
  const calls = [], frames = [], retained = [];
  let next = 0, closing = false;
  const session = {
    evaluateModuleSource(...args) {
      calls.push(['evaluateModuleSource', ...args]);
      if (closing) throw Error('actual closed evaluator');
      frames.push([...programs[next++]]);
    },
    reenterModuleSource(...args) {
      calls.push(['reenterModuleSource', ...args]);
      frames.push([...programs[next++]]);
    },
    poll(now, budget) {
      calls.push(['poll', now, budget]);
      const event = frames.at(-1)?.shift();
      if (!event) throw Error('mock call did not settle');
      if (['value', 'error'].includes(event.kind)) { frames.pop(); retained.push(event.value ?? event.error); }
      return event;
    },
    acknowledgeLog(...args) { calls.push(['acknowledgeLog', ...args]); },
    completeModuleKey(...args) { calls.push(['completeModuleKey', ...args]); },
    completeModuleCachePolicy(...args) { calls.push(['completeModuleCachePolicy', ...args]); },
    completeModuleFragmentPaths(...args) { calls.push(['completeModuleFragmentPaths', ...args]); },
    completeDefaultModule(...args) { calls.push(['completeDefaultModule', ...args]); return true; },
    closeFromCallback(...args) { calls.push(['closeFromCallback', ...args]); closing = true; },
    close() { calls.push(['close']); closing = true; },
    collect() {}, heapSlots() { return 0; }, contextDepth() { return frames.length; },
    free() { if (corruptOnFree) retained.forEach(view => view.fill(0)); },
  };
  const api = { SourceSession: { moduleEvaluator(...args) { calls.push(['moduleEvaluator', ...args]); return session; } } };
  return { api, calls };
}
const decoded = (row, name) => Buffer.from(row.payloads[name]).toString('utf8');

test('each exact query uses the public module evaluator with original source bytes', () => {
  const m = mock([[value('a')], [value('b')]]);
  const row = runDeprecationCase(m.api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', true, false, false]);
  assert.deepEqual(m.calls.filter(x => x[0] === 'evaluateModuleSource'),
    Array.from({ length: 2 }, () => ['evaluateModuleSource', 'file:///exact.pkl', 'exact source\r\n', 'old()', 0, null]));
  assert.equal(decoded(row, 'outer-1.stdout.bin'), 'b');
  assert.equal(row.architecture.terminalDepth, 0);
});

test('a nested warning does not recursively invoke the once-per-session callback', () => {
  const m = mock([[warning('outer'), value('a')], [warning('inner'), value('nested')], [warning('later'), value('b')]]);
  const row = runDeprecationCase(m.api, fixture({ callback: 'different()' }));
  assert.equal(row.acquisitionError, null);
  assert.equal(m.calls.filter(x => x[0] === 'reenterModuleSource').length, 1);
  assert.deepEqual(m.calls.filter(x => x[0] === 'acknowledgeLog').map(x => x[1]), ['inner', 'outer', 'later']);
  assert.deepEqual(row.protocol.slice(0, 7), ['call-enter=outer-0', 'log-enter=0 level=warn depth=0',
    'call-enter=log-callback-0', 'log-enter=1 level=warn depth=1', 'log-exit=1',
    'call-exit=log-callback-0 status=success', 'log-exit=0']);
});

test('callback threshold counts warnings and excludes trace events', () => {
  const m = mock([[warning('t', 'trace', 'trace'), warning('w0'), value('a')],
    [warning('w1'), value('b')], [value('nested')]]);
  const row = runDeprecationCase(m.api, fixture({ callback: 'nested()', callback_after: 1 }));
  assert.equal(row.acquisitionError, null);
  assert.equal(m.calls.find(x => x[0] === 'reenterModuleSource')[1], 'w1');
  assert.equal(decoded(row, 'log-callback-2.stdout.bin'), 'nested');
});

test('callback close still acknowledges its issued log and records actual later API failures', () => {
  const m = mock([[warning('issued'), value('completed active call')]]);
  const row = runDeprecationCase(m.api, fixture({ action: 'close-reenter' }));
  assert.equal(row.acquisitionError, null);
  assert.equal(decoded(row, 'post-close-callback-0.status.bin'), 'api-exception\n');
  assert.equal(decoded(row, 'post-close-callback-0.stderr.bin'), 'actual closed evaluator');
  assert.equal(decoded(row, 'outer-1.status.bin'), 'api-exception\n');
  assert.deepEqual(m.calls.filter(x => x[0] === 'acknowledgeLog').map(x => x[1]), ['issued']);
  assert.equal(row.architecture.terminalDepth, 0);
  assert.equal(Object.keys(row.payloads).some(name => name.endsWith('error-class.bin')), false);
});

test('throw-once acknowledges a real logger failure and preserves candidate disposition', () => {
  const m = mock([[warning('first'), failure('logger failed', 'internal-failure')], [warning('retry'), value('ok')]]);
  const row = runDeprecationCase(m.api, fixture({ action: 'throw-once' }));
  const acknowledgements = m.calls.filter(x => x[0] === 'acknowledgeLog');
  assert.deepEqual(acknowledgements[0][2], u('stage54-logger-failure'));
  assert.equal(acknowledgements[1][2], undefined);
  assert.equal(decoded(row, 'outer-0.status.bin'), 'candidate-error\n');
  assert.equal(row.protocol.includes('log-exit=0'), false);
  assert.equal(decoded(row, 'outer-1.stdout.bin'), 'ok');
});

test('unconfigured effects stay acquisition failures', () => {
  const m = mock([[{ kind: 'read', token: 'read', uri: 'file:///unconfigured' }]]);
  const row = runDeprecationCase(m.api, fixture({ expressions: ['read()'] }));
  assert.match(row.acquisitionError.message, /unexpected event: read/);
  assert.equal(row.payloads['outer-0.status.bin'], undefined);
});

test('raw UTF16 and Java UTF8 are both preserved including isolated surrogates', () => {
  const m = mock([[warning('utf16', '\ud800\ud83d\ude00'), value('\udc00')]]);
  const row = runDeprecationCase(m.api, fixture({ expressions: ['old()'] }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(row.payloads['log-0.message.code-units.bin'], [0xd8, 0, 0xd8, 0x3d, 0xde, 0]);
  assert.deepEqual(row.payloads['log-0.message.bin'], [0x3f, 0xf0, 0x9f, 0x98, 0x80]);
  assert.deepEqual(row.payloads['outer-0.stdout.bin'], [0x3f]);
});

test('primary key selection supplies only the exact configured source URI', () => {
  const event = { kind: 'module-request', token: 'key', module: { id: '1', kind: 'select-key',
    originalUri: 'file:///exact.pkl', keyId: '4', selectionPhase: 'primary' } };
  const m = mock([[event, value('ok')]]);
  const row = runDeprecationCase(m.api, fixture({ expressions: ['old()'] }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls.find(x => x[0] === 'completeModuleKey'),
    ['completeModuleKey', 'key', '4', 'file:///exact.pkl', 'host', 'fixture-key:4', 0]);
});

test('noop uses the real disabled logger option', () => {
  const m = mock([[value('ok')]]);
  const row = runDeprecationCase(m.api, fixture({ action: 'noop', expressions: ['old()'] }));
  assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', false, false, false]);
  assert.equal(row.protocol.at(-1), 'closed logs=0 warnings=0');
});

test('supplied-text wrapper policy uses its own key instead of the registered primary Host key', () => {
  const events = [
    { kind: 'module-request', token: 'primary', module: { id: '1', kind: 'select-key',
      originalUri: 'file:///exact.pkl', keyId: '1', selectionPhase: 'primary' } },
    { kind: 'module-request', token: 'wrapper', module: { id: '2', kind: 'query-fragment-paths',
      originalUri: 'file:///exact.pkl', keyId: '2', keyKind: 'supplied-text', keyHandle: '' } },
    value('amended output'),
  ];
  const m = mock([events]);
  const row = runDeprecationCase(m.api, fixture({ expressions: ['output.text'] }));
  assert.equal(row.acquisitionError, null);
  assert.equal(decoded(row, 'outer-0.stdout.bin'), 'amended output');
  assert.deepEqual(m.calls.filter(call => call[0] === 'completeDefaultModule'),
    [['completeDefaultModule', 'wrapper', 0]]);
  assert.equal(m.calls.some(call => call[0] === 'completeModuleFragmentPaths'), false);
});

test('owned buffer corruption after freeing a session is detected', () => {
  const m = mock([[value('not empty')]], { corruptOnFree: true });
  const row = runDeprecationCase(m.api, fixture({ expressions: ['old()'] }));
  assert.equal(row.architecture.retainedPayloadsStable, false);
});

test('invalid callback threshold is rejected before evaluator construction', () => {
  const m = mock([]);
  const row = runDeprecationCase(m.api, fixture({ callback_after: -1 }));
  assert.match(row.acquisitionError.message, /invalid deprecation session profile/);
  assert.equal(m.calls.length, 0);
});
