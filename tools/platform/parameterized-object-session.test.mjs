import test from 'node:test';
import assert from 'node:assert/strict';
import { runParameterizedCase, compareParameterizedPayload, javaUtf8 } from './parameterized-object-session.mjs';

const fixture = { id: 'case', source: 'value = 7\n', uri: 'file:///original/input.pkl', moduleName: 'input', expression: 'value', powerAssertions: false, resources: [] };
const units = value => Uint16Array.from(Array.from(value, char => char.charCodeAt(0)));
function api(events, options = {}) {
  const calls = [];
  return { calls, SourceSession: { forExpressions(...args) {
    calls.push(['constructor', args]);
    let index = 0;
    return {
      startPath(...args) { calls.push(['start', args]); },
      poll() { return events[index++]; }, collect() {}, close() {},
      heapSlots() { return options.slots ?? 0; }, contextDepth() { return 0; },
      resolvedReadUri() { return options.resolvedUri; },
      completeReadResource(token, uri, bytes) { calls.push(['resource', token, uri, Array.from(bytes)]); },
      completeDefaultModule(token) { calls.push(['module', token]); return true; },
      free() { options.onFree?.(); },
    };
  } } };
}

test('fresh sessions retain the exact arbitrary expression and Java profile', () => {
  const binding = api([{ kind: 'value', value: units('7') }]);
  for (let repeat = 0; repeat < 2; repeat++) {
    const result = runParameterizedCase(binding, fixture);
    assert.equal(compareParameterizedPayload({ exitCode: 0, stdout: [55], stderr: [] }, result).status, 'pass');
  }
  const constructors = binding.calls.filter(call => call[0] === 'constructor');
  assert.equal(constructors.length, 2);
  assert.deepEqual(constructors[0][1], [fixture.source, fixture.uri, fixture.moduleName, ['value'], 'compact', false, false, false]);
});

test('unsupported diagnostics remain nonpasses even with identical full bytes', () => {
  const result = runParameterizedCase(api([{ kind: 'error', error: units('bad'), disposition: 'unsupported' }]), fixture);
  assert.equal(compareParameterizedPayload({ exitCode: 1, stdout: [], stderr: [98, 97, 100] }, result).status, 'non-language-failure');
});

test('ordinary errors compare complete raw whitespace', () => {
  const result = runParameterizedCase(api([{ kind: 'error', error: units('bad\n'), disposition: 'language-error' }]), fixture);
  assert.equal(compareParameterizedPayload({ exitCode: 1, stdout: [], stderr: [98, 97, 100] }, result).status, 'mismatch');
});

test('exact captured resource URI and bytes use actual Resource completion', () => {
  const uri = 'file:///original/gate.txt';
  const binding = api([{ kind: 'read', token: 'r', uri }, { kind: 'value', value: units('go') }], { resolvedUri: uri });
  const result = runParameterizedCase(binding, { ...fixture, resources: [{ uri, bytes: [103, 111] }] });
  assert.equal(result.acquisitionError, null);
  assert.deepEqual(binding.calls.find(call => call[0] === 'resource'), ['resource', 'r', uri, [103, 111]]);
  assert.equal(result.trace[0].resolvedUri, uri);
});

test('relative read operand completes through its exact declared resolved URI', () => {
  const uri = 'file:///original/gate.txt';
  const binding = api([{ kind: 'read', token: 'r', uri: 'gate.txt' }, { kind: 'value', value: units('go') }], { resolvedUri: uri });
  const result = runParameterizedCase(binding, { ...fixture, resources: [{ uri, bytes: [103, 111] }] });
  assert.equal(result.acquisitionError, null);
  assert.equal(result.exitCode, 0);
  assert.deepEqual(binding.calls.find(call => call[0] === 'resource'), ['resource', 'r', uri, [103, 111]]);
  assert.deepEqual(result.trace[0], { kind: 'read', token: 'r', requestedUri: 'gate.txt', resolvedUri: uri });
});

test('undeclared resolved resource cannot be replaced by an authored relative match', () => {
  const declared = 'file:///original/gate.txt';
  const binding = api([{ kind: 'read', token: 'r', uri: 'gate.txt' }], { resolvedUri: 'file:///different/gate.txt' });
  const result = runParameterizedCase(binding, { ...fixture, resources: [{ uri: declared, bytes: [103, 111] }] });
  assert.equal(result.exitCode, 2);
  assert.match(result.acquisitionError.message, /unconfigured exact resource URI/);
  assert.equal(binding.calls.some(call => call[0] === 'resource'), false);
  assert.equal(result.trace[0].requestedUri, 'gate.txt');
  assert.equal(result.trace[0].resolvedUri, 'file:///different/gate.txt');
});

test('module requests delegate to the existing shared default policy', () => {
  const uri = 'pkl:base';
  const binding = api([{ kind: 'module-request', token: 'm', module: { id: '1', kind: 'check-import', originalUri: uri, importUri: uri, importerOriginalUri: fixture.uri, importerResolvedUri: fixture.uri } }, { kind: 'value', value: units('7') }]);
  const result = runParameterizedCase(binding, fixture);
  assert.equal(result.exitCode, 0);
  assert.deepEqual(binding.calls.find(call => call[0] === 'module'), ['module', 'm']);
});

test('unclassified constructor text is preserved as an acquisition failure', () => {
  const result = runParameterizedCase({ SourceSession: { forExpressions() { throw Error('bad\ud800'); } } }, fixture);
  assert.equal(result.exitCode, 2);
  assert.deepEqual(result.stderrUnits, [98, 97, 100, 0xd800]);
  assert.deepEqual(javaUtf8(result.stderrUnits), [98, 97, 100, 63]);
});

test('constructor classification requires complete original diagnostic identity', () => {
  for (const message of ['bad', 'bad\n']) {
    const binding = { SourceSession: { forExpressions() { throw Error(message); } },
      evaluateModuleFrontend() { return { exitCode: 1, disposition: 'language-error', phase: 'parsing', stderr: Uint8Array.from([98, 97, 100]) }; } };
    const result = runParameterizedCase(binding, fixture);
    assert.equal(result.constructorClassified, message === 'bad');
    assert.equal(result.exitCode, message === 'bad' ? 1 : 2);
  }
});

test('closed session ownership and leaked slots are separate admission checks', () => {
  const view = units('7');
  const result = runParameterizedCase(api([{ kind: 'value', value: view }], { slots: 1, onFree() { view.fill(0); } }), fixture);
  const compared = compareParameterizedPayload({ exitCode: 0, stdout: [55], stderr: [] }, result);
  assert.equal(compared.checks.stdout, true);
  assert.equal(compared.checks.terminalSlots, false);
  assert.equal(compared.checks.ownedResultStable, false);
  assert.equal(compared.status, 'mismatch');
});

test('power assertions cannot silently override the captured Java profile', () => {
  const binding = api([]);
  const result = runParameterizedCase(binding, { ...fixture, powerAssertions: true });
  assert.equal(result.exitCode, 2);
  assert.equal(binding.calls.length, 0);
});

test('a constructor marker cannot bypass successful session lifecycle checks', () => {
  const result = runParameterizedCase(api([{ kind: 'value', value: units('7') }]), fixture);
  result.constructorClassified = true;
  const compared = compareParameterizedPayload({ exitCode: 0, stdout: [55], stderr: [] }, result);
  assert.equal(compared.checks.constructorShape, false);
  assert.notEqual(compared.status, 'pass');
});
