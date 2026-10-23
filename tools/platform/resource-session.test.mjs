import test from 'node:test';
import assert from 'node:assert/strict';
import {runResourceCase, compareResourceCase, javaUtf8Bytes} from './resource-session.mjs';

const units = text => Array.from({length: text.length}, (_, index) => text.charCodeAt(index));
const hex = values => Buffer.from(javaUtf8Bytes(values)).toString('hex');
const expected = (status, value, before, after) => {
  const stdoutUnits = status === 'success' ? value : [], stderrUnits = status === 'error' ? value : [];
  return {status, stdoutUnits, stderrUnits, stdoutHex: hex(stdoutUnits), stderrHex: hex(stderrUnits),
    readsBefore: before, readsAfter: after};
};
const fixture = {id: 'reader-sequence', repeat: 0, session: 0, source: 'module source',
  uri: 'file:///main.pkl', moduleName: 'main', expressions: ['value', 'cached', 'absent', 'retry'],
  readerPlan: [
    {uri: 'res:/text', result: {kind: 'text', units: [65, 0xd800]}},
    {uri: 'res:/missing', result: {kind: 'missing'}},
    {uri: 'res:/retry', result: {kind: 'io', message: null}},
  ], expected: [expected('success', [65, 0xd800], 0, 1), expected('success', [65, 0xd800], 1, 1),
    expected('success', units('null'), 1, 2), expected('error', units('failed\n'), 2, 3)]};

class Session {
  startPath(index) { this.index = index; this.pending = index !== 1; }
  poll() {
    if (this.pending) { this.pending = false; return {kind: 'read', token: 'token'}; }
    return this.index === 3
      ? {kind: 'error', error: Uint16Array.from(units('failed\n')), disposition: 'language-error'}
      : {kind: 'value', value: Uint16Array.from(this.index === 2 ? units('null') : this.cached)};
  }
  resolvedReadUri() { return ['res:/text', null, 'res:/missing', 'res:/retry'][this.index]; }
  completeReadText(token, payload) { assert.equal(token, 'token'); this.cached = Array.from(payload); }
  completeReadMissing(token) { assert.equal(token, 'token'); }
  rejectReadIo(token, cause) { assert.equal(token, 'token'); assert.equal(cause, null); }
  close() {} collect() {} free() {} heapSlots() { return 0; }
}
const api = Class => ({SourceSession: {forExpressions: () => new Class()}});
const run = (Class = Session, input = fixture) => runResourceCase(api(Class), input);

test('full raw channels, cache reads and typed Missing/I/O preserve their boundaries', () => {
  const result = run();
  assert.equal(result.acquisitionError, null);
  assert.deepEqual(result.calls.map(call => call.readsAfter), [1, 1, 2, 3]);
  assert.equal(compareResourceCase(fixture, result).passed, true);
});
test('Java UTF8 replacement and full UTF16 remain independent comparisons', () => {
  assert.deepEqual(javaUtf8Bytes([0xd800, 0x20, 0xdc00, 0xd83d, 0xde00]), [63, 32, 63, 240, 159, 152, 128]);
  const result = run();
  result.calls[0].stdoutUnits = [65, 63];
  const comparison = compareResourceCase(fixture, result).comparisons[0];
  assert.equal(comparison.checks.stdoutBytes, true);
  assert.equal(comparison.checks.stdoutUnits, false);
  assert.equal(comparison.status, 'mismatch');
});
test('trailing newline changes remain raw mismatches', () => {
  const result = run(); result.calls[3].stderrUnits.pop();
  assert.equal(compareResourceCase(fixture, result).comparisons[3].status, 'mismatch');
});
test('reader counts and missing API calls cannot silently pass', () => {
  const result = run(); result.calls[1].readsAfter++;
  assert.equal(compareResourceCase(fixture, result).comparisons[1].checks.readsAfter, false);
  result.calls.pop();
  assert.equal(compareResourceCase(fixture, result).comparisons[3].status, 'missing-call');
});
test('extra API calls and unconsumed reader plans fail', () => {
  const result = run(); result.calls.push({...result.calls[0]});
  assert.equal(compareResourceCase(fixture, result).checks.calls, false);
  const input = {...fixture, readerPlan: [...fixture.readerPlan, fixture.readerPlan[0]]};
  assert.equal(compareResourceCase(input, run(Session, input)).passed, false);
});
test('wrong reader URI is recorded and never receives the expected completion', () => {
  class WrongUri extends Session {
    resolvedReadUri() { return 'res:/wrong'; }
    completeReadText() { assert.fail('wrong URI must not consume a payload'); }
  }
  const result = run(WrongUri);
  assert.deepEqual(result.readerUris, ['res:/wrong']);
  assert.match(result.acquisitionError.message, /URI\/order/);
  assert.equal(compareResourceCase(fixture, result).passed, false);
});
test('transport rejects untyped arrays, unsupported errors and retained heap slots', () => {
  class ArrayResult extends Session { poll() { return {kind: 'value', value: [1]}; } }
  assert.match(run(ArrayResult).acquisitionError.message, /Uint16Array/);
  class Unsupported extends Session {
    poll() { const event = super.poll(); if (event.kind === 'error') event.disposition = 'unsupported'; return event; }
  }
  assert.equal(compareResourceCase(fixture, run(Unsupported)).comparisons[3].checks.disposition, false);
  class Leaked extends Session { heapSlots() { return 1; } }
  assert.equal(compareResourceCase(fixture, run(Leaked)).checks.terminalSlots, false);
});
test('input payload aliasing and unstable output views are detected', () => {
  class BorrowedInput extends Session { completeReadText(token, payload) { this.cached = payload; } }
  assert.equal(compareResourceCase(fixture, run(BorrowedInput)).passed, false);
  class BorrowedOutput extends Session {
    poll() { const event = super.poll(); if (event.value) { this.views ??= []; this.views.push(event.value); } return event; }
    close() { for (const view of this.views) view.fill(0); }
  }
  assert.equal(run(BorrowedOutput).architecture.ownedResultsStable, false);
});
test('typed Resource URI and owned bytes reach the Host completion intact', () => {
  const input = {...fixture, expressions: ['value'], readerPlan: [{uri: 'res:/bytes',
    result: {kind: 'resource', resourceUri: 'res:/returned\ud800', bytes: [65, 0, 255]}}]};
  class ResourceSession extends Session {
    resolvedReadUri() { return 'res:/bytes'; }
    completeReadResource(token, uri, payload) {
      assert.equal(uri, 'res:/returned\ud800'); assert.ok(payload instanceof Uint8Array);
      assert.deepEqual(Array.from(payload), [65, 0, 255]); this.cached = [49];
    }
  }
  assert.equal(run(ResourceSession, input).acquisitionError, null);
});
test('output arrays must remain owned after the binding instance is freed', () => {
  class ReleasedOutput extends Session {
    poll() { const event = super.poll(); if (event.value) { this.views ??= []; this.views.push(event.value); } return event; }
    free() { for (const view of this.views) view.fill(0); }
  }
  assert.equal(compareResourceCase(fixture, run(ReleasedOutput)).checks.ownedResultsStable, false);
});
test('invalid fixtures never enter the runtime and inconsistent oracle channels fail', () => {
  class Unreachable { constructor() { assert.fail('must validate before constructing'); } }
  const invalid = {...fixture, readerPlan: [{uri: 'res:/x', result: {kind: 'text', units: [65536]}}]};
  assert.match(run(Unreachable, invalid).acquisitionError.message, /UTF16/);
  const bad = structuredClone(fixture); bad.expected[0].stdoutHex = '41efbfbd';
  assert.throws(() => compareResourceCase(bad, run()), /inconsistent Java/);
});
test('constructor failures preserve unmatched surrogate error units', () => {
  const result = runResourceCase({SourceSession: {forExpressions() { throw Error('bad\ud800'); }}}, fixture);
  assert.deepEqual(result.constructorError.units, [98, 97, 100, 0xd800]);
  assert.equal(compareResourceCase(fixture, result).passed, false);
});

test('default module requests retain their actual context without changing the Java reader plan', () => {
  class WithModules extends Session {
    startPath(index) { super.startPath(index); this.moduleStep = 0; }
    poll() {
      if (this.index === 0 && this.moduleStep < 2) return {kind: 'module-request', token: 'module-token',
        module: {id: String(this.moduleStep), kind: this.moduleStep === 0 ? 'check-import' : 'resolve',
          originalUri: 'pkl:json', importUri: 'pkl:json', importerOriginalUri: 'file:///main.pkl',
          importerResolvedUri: 'file:///resolved.pkl'}};
      return super.poll();
    }
    completeDefaultModule(token, now) {
      assert.equal(token, 'module-token'); assert.equal(now, 0); this.moduleStep++; return true;
    }
  }
  const result = run(WithModules);
  assert.equal(compareResourceCase(fixture, result).passed, true);
  assert.equal(result.modulePolicy, 'pkl-0.32.0-default');
  assert.deepEqual(result.moduleRequests.map(row => row.module.kind), ['check-import', 'resolve']);
  assert.equal(result.moduleRequests[0].module.importerResolvedUri, 'file:///resolved.pkl');
  assert.deepEqual(result.readerUris, ['res:/text', 'res:/missing', 'res:/retry']);
});

test('unsupported default module requests and stale-token errors remain acquisition failures', () => {
  for (const mode of ['unsupported', 'stale']) {
    class Unhandled extends Session {
      poll() { return {kind: 'module-request', token: 'module-token', module: {id: '0', kind: 'check-import',
        originalUri: 'pkl:json', importUri: 'pkl:json', importerOriginalUri: 'file:///main.pkl',
        importerResolvedUri: 'file:///main.pkl'}}; }
      completeDefaultModule() { if (mode === 'stale') throw Error('stale module token'); return false; }
    }
    const result = run(Unhandled);
    assert.match(result.acquisitionError.message, mode === 'stale' ? /stale module token/ : /unsupported/);
    assert.equal(result.moduleRequests.length, 1);
    assert.equal(result.calls.length, 0);
    assert.equal(compareResourceCase(fixture, result).passed, false);
  }
});

test('missing or contradictory module event context fails before default-policy completion', () => {
  const request = {id: '0', kind: 'check-import', originalUri: 'pkl:json', importUri: 'pkl:json',
    importerOriginalUri: 'file:///main.pkl', importerResolvedUri: 'file:///resolved.pkl'};
  const invalid = ['originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'].map(name => {
    const copy = {...request}; delete copy[name]; return copy;
  });
  invalid.push({...request, kind: 'load-source'}, {...request, sourceUri: 'file:///forged.pkl'},
    {...request, resolvedUri: 'pkl:json'}, {...request, unexpected: 'metadata'});
  for (const module of invalid) {
    class BadContext extends Session {
      poll() { return {kind: 'module-request', token: 'module-token', module}; }
      completeDefaultModule() { assert.fail('invalid context must not consume a token'); }
    }
    const result = run(BadContext);
    assert.match(result.acquisitionError.message, /module request context/);
    assert.deepEqual(result.moduleRequests[0].module, module);
    assert.equal(result.calls.length, 0);
  }
});
