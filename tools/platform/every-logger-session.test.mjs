import test from 'node:test';
import assert from 'node:assert/strict';
import { runEveryLoggerSession } from './every-logger-session.mjs';

const units = text => Uint16Array.from({ length: text.length }, (_, i) => text.charCodeAt(i));
const decode = bytes => new TextDecoder().decode(Uint8Array.from(bytes));
const fixture = { uri: 'file:///stage68/collection-every.pkl', source: 'result = 1',
  rows: [{ id: 'one', expression: 'List(1).every((x) -> trace(x) > 0)' },
    { id: 'empty', expression: 'List().every((x) -> false)' }] };
function log(token, message, position = 37) {
  return { kind: 'log', token, log: { level: 'trace', message: units(message),
    moduleUri: units('repl:text'), memberName: null, sourceLines: [],
    startLine: 1, startColumn: position, endLine: 1, endColumn: position } };
}
function mock(firstEvents) {
  const calls = [], acknowledgments = [];
  const session = {
    events: [],
    evaluateModuleSource(...args) { calls.push(args); this.events = calls.length === 1
      ? [...firstEvents] : [{ kind: 'value', value: units('true') }]; },
    poll() { return this.events.shift() ?? { kind: 'yield' }; },
    collect() {},
    acknowledgeLog(...args) { acknowledgments.push(args); },
    close() {}, heapSlots() { return 0; }, contextDepth() { return 0; }, free() {},
  };
  const api = { SourceSession: { moduleEvaluator(...args) {
    assert.deepEqual(args, ['compact', true, false, false]); return session;
  } } };
  return { api, calls, acknowledgments };
}

test('captures ordered raw logger fields and explicit zero-log row', () => {
  const source = mock([log('first', 'x = 🙂\ud800'), log('second', 'x = 1', 39),
    { kind: 'value', value: units('true') }]);
  const result = runEveryLoggerSession(source.api, fixture);
  assert.equal(result.acquisitionError, null);
  assert.equal(result.architecture.retainedPayloadsStable, true);
  assert.deepEqual(source.acknowledgments, [['first', undefined, 0], ['second', undefined, 0]]);
  assert.equal(decode(result.rows[0].payloads['logs.count.bin']), '2\n');
  assert.equal(decode(result.rows[1].payloads['logs.count.bin']), '0\n');
  assert.deepEqual(result.rows[0].payloads['log-000.message.code-units.bin'].slice(-2), [0xd8, 0]);
  assert.deepEqual(result.rows[0].payloads['log-000.message.bin'].slice(-5), [240, 159, 153, 130, 63]);
  assert.equal(decode(result.rows[0].payloads['log-000.member.bin']), 'null');
  assert.equal(decode(result.rows[0].payloads['log-000.position.bin']), '1:37-1:37');
  assert.equal(decode(result.rows[0].payloads['log-001.position.bin']), '1:39-1:39');
  assert.equal(result.rows[0].status, 'success');
  assert.equal(result.rows[1].status, 'success');
  assert.deepEqual(source.calls.map(args => args.slice(0, 3)), fixture.rows.map(row =>
    [fixture.uri, fixture.source, row.expression]));
});

test('invalid logger metadata fails acquisition before acknowledgement', () => {
  const event = log('bad', 'x'); delete event.log.memberName;
  const source = mock([event, { kind: 'value', value: units('true') }]);
  const result = runEveryLoggerSession(source.api, fixture);
  assert.match(result.acquisitionError.message, /invalid owned logger event/);
  assert.deepEqual(source.acknowledgments, []);
});

test('logger token and position must be valid', () => {
  for (const event of [log('', 'x'), log('t', 'x', -1)]) {
    const source = mock([event, { kind: 'value', value: units('true') }]);
    const result = runEveryLoggerSession(source.api, fixture);
    assert.match(result.acquisitionError.message, /invalid owned logger event/);
    assert.deepEqual(source.acknowledgments, []);
  }
});
