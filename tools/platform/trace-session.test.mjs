import test from 'node:test';
import assert from 'node:assert/strict';
import { runTraceCase } from './trace-session.mjs';

const units = text => Uint16Array.from(text, char => char.charCodeAt(0));
const fixture = {
  sourceText: 'output { text="value" }', operation: 'output-bytes', calls: 2,
  resource_profile: 'preconfigured-plus-life',
  mode: 'COMPACT', action: 'capture', logger_callback: '', reader_callback: '',
};
function fakeApi(events, mutateOnClose = false) {
  const observed = { factories: 0, outputStarts: 0, pathStarts: 0, configured: [] };
  class SourceSession {
    static forOutput(...args) {
      observed.factories++;
      observed.factoryArguments = args;
      return new SourceSession();
    }
    startModule() { throw Error('no separate module initialization'); }
    startPath() { observed.pathStarts++; this.queue = [...events()]; }
    startOutputBytes() { observed.outputStarts++; this.queue = [...events()]; }
    reenterPath() { throw Error('no callback configured'); }
    reenterOutputBytes() { throw Error('no callback configured'); }
    respondConfigured(token) { observed.configured.push(token); }
    respondString() { throw Error('no custom read configured'); }
    acknowledgeLog() { throw Error('no log configured'); }
    closeFromCallback() { throw Error('no log configured'); }
    poll() {
      const event = this.queue.shift();
      if (event.bytes) this.last = event.bytes;
      return event;
    }
    collect() {}
    setupFailed() { return false; }
    heapSlots() { return 0; }
    close() { if (mutateOnClose && this.last) this.last[0] ^= 255; }
  }
  return { api: { SourceSession }, observed };
}

test('actual output dispatch preserves arbitrary bytes and configured reads', () => {
  const { api, observed } = fakeApi(() => [
    { kind: 'read', uri: 'prop:pkl.outputFormat', token: 'configured' },
    { kind: 'bytes', bytes: Uint8Array.from([0, 255, 65, 13, 10]) },
  ]);
  const result = runTraceCase(api, fixture, 1);
  assert.equal(result.acquisitionError, null);
  assert.equal(observed.factories, 1);
  assert.equal(observed.outputStarts, 2);
  assert.equal(observed.pathStarts, 0);
  assert.deepEqual(observed.factoryArguments[3], []);
  assert.deepEqual(observed.configured, ['configured', 'configured']);
  assert.deepEqual(result.payloads['outer-0.stdout.bin'], [0, 255, 65, 13, 10]);
  assert.equal(result.architecture.reads, 0);
  assert.equal(result.architecture.retainedEventBytesStable, true);
});

test('output diagnostics use raw bytes rather than a display string', () => {
  const { api } = fakeApi(() => [{ kind: 'error', errorBytes: Uint8Array.from([255, 0, 13, 10]), error: units('wrong display') }]);
  const result = runTraceCase(api, fixture);
  assert.equal(result.acquisitionError, null);
  assert.deepEqual(result.payloads['outer-0.stderr.bin'], [255, 0, 13, 10]);
  assert.deepEqual(result.payloads['outer-0.stdout.bin'], []);
});

test('expression remains the default and uses its original string boundary', () => {
  const { api, observed } = fakeApi(() => [{ kind: 'value', value: Uint16Array.from([0xd800]) }]);
  const result = runTraceCase(api, { sourceText: 'result=1', expressions: ['result'], mode: 'COMPACT', action: 'capture', logger_callback: '', reader_callback: '' });
  assert.equal(result.acquisitionError, null);
  assert.equal(observed.factories, 0);
  assert.equal(observed.outputStarts, 0);
  assert.equal(observed.pathStarts, 1);
  assert.deepEqual(result.payloads['outer-0.stdout.bin'], [63]);
});

test('post-close byte mutation is detected independently of payload equality', () => {
  const { api } = fakeApi(() => [{ kind: 'bytes', bytes: Uint8Array.from([1, 2]) }], true);
  const result = runTraceCase(api, fixture);
  assert.deepEqual(result.payloads['outer-1.stdout.bin'], [1, 2]);
  assert.equal(result.architecture.retainedEventBytesStable, false);
});

test('missing output bindings and wrong byte types stay acquisition failures', () => {
  for (const event of [{ kind: 'bytes', bytes: [1, 2] }, { kind: 'bytes' }]) {
    const { api } = fakeApi(() => [event]);
    assert.notEqual(runTraceCase(api, fixture).acquisitionError, null);
  }
  const { api } = fakeApi(() => []);
  api.SourceSession.forOutput = undefined;
  assert.notEqual(runTraceCase(api, fixture).acquisitionError, null);
});

test('output reader profile and explicit call count cannot silently change', () => {
  const { api, observed } = fakeApi(() => [{ kind: 'bytes', bytes: new Uint8Array() }]);
  for (const altered of [
    { resource_profile: undefined }, { resource_profile: 'life-only' },
    { calls: 0 }, { calls: 1.5 }, { calls: 1025 }, { expressions: ['result'] },
  ]) assert.throws(() => runTraceCase(api, { ...fixture, ...altered }));
  assert.equal(observed.factories, 0);
});
