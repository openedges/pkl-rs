import test from 'node:test';
import assert from 'node:assert/strict';
import { runLosslessConstructorCase, compareConstructorResult, stringUnits } from './lossless-constructor-session.mjs';
const fixture = { id: 'surrogate-message', source: 'x = 1\n', uri: 'file:///source.pkl',
  moduleName: 'source', paths: ['x'], arguments: ['eval', 'source.pkl'],
  expectedMessageUnits: [0xd800, 0x78, 0xdc00] };
function api(message) {
  class SourceSession {
    constructor() { throw Error(message); }
    static forExpressions() { throw Error(message); }
    static forOutput() { throw Error(message); }
  }
  return { SourceSession };
}
test('all three constructor boundaries retain exact unpaired UTF16', () => {
  for (const factory of ['constructor', 'expressions', 'output']) {
    const actual = runLosslessConstructorCase(api('\ud800x\udc00'), fixture, factory);
    assert.equal(compareConstructorResult(fixture, actual).status, 'pass');
  }
});
test('question mark and different surrogate cannot pass exact message units', () => {
  for (const text of ['?x?', '\udc00x\ud800', '\ufffdx\ufffd']) {
    const actual = runLosslessConstructorCase(api(text), fixture, 'expressions');
    assert.equal(compareConstructorResult(fixture, actual).status, 'mismatch');
  }
});
test('unexpected construction is retained separately from a thrown error', () => {
  const calls = [];
  class SourceSession { close() { calls.push('close'); } collect() { calls.push('collect'); } free() { calls.push('free'); } }
  const actual = runLosslessConstructorCase({ SourceSession }, fixture, 'constructor');
  assert.equal(actual.kind, 'constructed');
  assert.deepEqual(calls, ['close', 'collect', 'free']);
  assert.equal(compareConstructorResult(fixture, actual).status, 'mismatch');
});
test('non-Error thrown value and missing message do not count as errors', () => {
  for (const thrown of [{ message: '\ud800x\udc00' }, Error()]) {
    class SourceSession { constructor() { throw thrown; } }
    assert.equal(compareConstructorResult(fixture, runLosslessConstructorCase({ SourceSession }, fixture, 'constructor')).status, 'mismatch');
  }
  assert.deepEqual(stringUnits('\ud800x\udc00'), fixture.expectedMessageUnits);
});

test('invalid driver factory is rejected before any evaluator call', () => {
  assert.throws(() => runLosslessConstructorCase(api('unknown factory'), fixture, 'missing'), /unknown factory/);
});
