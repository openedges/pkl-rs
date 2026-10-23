import test from 'node:test';
import assert from 'node:assert/strict';
import {runModulePreparationCase, compareModulePreparationCase} from './module-preparation-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

const owner = {importerOriginalUri: 'file:///main.pkl', importerResolvedUri: 'file:///main.pkl'};
const cap = {kind: 'capabilities', originalUri: 'file:///main.pkl', ...owner};
const element = {kind: 'has-element', originalUri: 'file:///dep.pkl', ...owner};
const resolve = {kind: 'resolve', originalUri: 'file:///dep.pkl', importUri: 'file:///dep.pkl', ...owner};
const source = {kind: 'load-source', originalUri: 'file:///dep.pkl',
  resolvedUri: 'file:///dep.pkl', sourceUri: 'file:///dep.pkl'};
const units = text => Array.from({length: text.length}, (_, i) => text.charCodeAt(i));
const expected = (status, text, before, after, events) => {
  const stdoutUnits = status === 'success' ? units(text) : [], stderrUnits = status === 'error' ? units(text) : [];
  return {status, stdoutUnits, stderrUnits, stdoutHex: Buffer.from(javaUtf8Bytes(stdoutUnits)).toString('hex'),
    stderrHex: Buffer.from(javaUtf8Bytes(stderrUnits)).toString('hex'), readsBefore: before, readsAfter: after,
    observableEvents: events};
};
const fixture = {
  id: 'module-host-unit', repeat: 0, session: 0, source: 'value = 1', uri: 'file:///main.pkl', moduleName: 'main',
  expressions: ['value', 'denied'],
  moduleProviders: [
    {request: cap, result: {kind: 'capabilities', local: true, hierarchical: true, fragmentPaths: false}, observableEvents: []},
    {request: element, result: {kind: 'has-element', exists: true}, observableEvents: ['module dep']},
    {request: resolve, result: {kind: 'resolve', resolvedUri: 'file:///dep.pkl', sourceUri: 'file:///dep.pkl'}, observableEvents: ['import dep']},
    {request: source, result: {kind: 'source', text: 'value = 1', inferredName: 'dep'}, observableEvents: ['source dep']},
  ],
  readProviders: [
    {uri: 'file:///value.bin', result: {kind: 'resource', resourceUri: 'file:///value.bin', bytes: [65]}, countedReader: true, observableEvents: ['read value']},
    {uri: 'file:///denied.bin', result: {kind: 'security-manager-exception', message: 'denied\ud800'}, countedReader: false, observableEvents: ['policy denied']},
  ],
  readerPlan: [{uri: 'file:///value.bin', result: {kind: 'resource', resourceUri: 'file:///value.bin', bytes: [65]}}],
  expected: [expected('success', 'A', 0, 1, ['module dep', 'import dep', 'source dep', 'read value']),
    expected('error', 'denied\ud800', 1, 1, ['policy denied'])],
};
class Session {
  constructor() { this.views = []; this.methods = []; this.programs = [[cap, element, resolve, source, {read: 'file:///value.bin'}], [{read: 'file:///denied.bin'}]]; }
  startPath(index) { this.index = index; this.step = 0; this.pending = false; }
  poll() {
    const action = this.programs[this.index][this.step];
    if (action) {
      this.pending = true; this.collected = false;
      if (action.read) return {kind: 'read', uri: action.read, token: 'owned-token'};
      return {kind: 'module-request', token: 'owned-token', module: {id: String(this.step), ...action}};
    }
    const view = Uint16Array.from(this.index === 0 ? this.payload : units(this.cause)); this.views.push(view);
    return this.index === 0 ? {kind: 'value', value: view}
      : {kind: 'error', error: view, disposition: 'language-error'};
  }
  collect() { if (this.pending) this.collected = true; }
  finish(token, method) { assert.equal(token, 'owned-token'); assert.equal(this.collected, true); this.pending = false; this.step++; this.methods.push(method); }
  completeModuleCapabilities(token, local, hierarchical, fragmentPaths) { assert.deepEqual([local, hierarchical, fragmentPaths], [true, true, false]); this.finish(token, 'capabilities'); }
  completeModuleHasElement(token, exists) { assert.equal(exists, true); this.finish(token, 'has-element'); }
  completeModuleResolution(token, original, resolved, sourceUri) { assert.deepEqual([original, resolved, sourceUri], Array(3).fill('file:///dep.pkl')); this.finish(token, 'resolve'); }
  completeModuleSource(token, text, name) { assert.deepEqual([text, name], ['value = 1', 'dep']); this.finish(token, 'source'); }
  resolvedReadUri() { return this.programs[this.index][this.step].read; }
  completeReadResource(token, uri, bytes) { assert.equal(uri, 'file:///value.bin'); this.payload = Array.from(bytes); this.finish(token, 'resource'); }
  rejectReadSecurityManager(token, cause) { assert.equal(cause, 'denied\ud800'); this.cause = cause; this.finish(token, 'denied'); }
  rejectModulePreparation(token, hasElement, kind, message) { this.failure = {hasElement, kind, message}; this.finish(token, 'module-failure'); }
  close() {} heapSlots() { return 0; } free() {}
}
const api = Class => ({SourceSession: {forExpressions: () => new Class()}});
const run = (input = fixture, Class = Session) => runModulePreparationCase(api(Class), input);

test('module effects and denied reads use exact providers while counted reads retain ownership checks', () => {
  const result = run();
  assert.equal(result.acquisitionError, null);
  assert.deepEqual(result.calls.map(call => [call.readsBefore, call.readsAfter]), [[0, 1], [1, 1]]);
  assert.deepEqual(result.readerUris, ['file:///value.bin']);
  assert.equal(result.moduleRequests.length, 6);
  assert.equal(result.architecture.pendingModuleOrDeniedReadCollections, 5);
  assert.equal(compareModulePreparationCase(fixture, result).passed, true);
});
test('module tables are reusable unordered input facts without a consume-all requirement', () => {
  const input = structuredClone(fixture);
  input.moduleProviders.reverse();
  input.moduleProviders.push({request: {...element, originalUri: 'file:///unused.pkl'},
    result: {kind: 'has-element', exists: false}, observableEvents: ['unused']});
  assert.equal(compareModulePreparationCase(input, run(input)).passed, true);
});
test('duplicate module/read identities and incomplete importer context are rejected before execution', () => {
  let input = structuredClone(fixture); input.moduleProviders.push(input.moduleProviders[0]);
  assert.throws(() => run(input), /duplicate module/);
  input = structuredClone(fixture); input.readProviders.push(input.readProviders[0]);
  assert.throws(() => run(input), /duplicate read/);
  input = structuredClone(fixture); delete input.moduleProviders[0].request.importerResolvedUri;
  assert.throws(() => run(input), /incomplete module/);
});
test('unknown module/read requests never fall back to missing or a default capability', () => {
  let input = structuredClone(fixture); input.moduleProviders.splice(1, 1);
  let result = run(input); assert.match(result.acquisitionError.message, /unconfigured exact module/);
  assert.equal(compareModulePreparationCase(input, result).passed, false);
  input = structuredClone(fixture); input.readProviders.pop();
  result = run(input); assert.match(result.acquisitionError.message, /unconfigured exact File read/);
  assert.equal(compareModulePreparationCase(input, result).passed, false);
});
test('module preparation rejection keeps its capability/element channel and nullable UTF16 cause', () => {
  for (const request of [cap, element]) for (const message of [null, 'cause\ud800']) {
    const input = structuredClone(fixture);
    input.expressions = ['value']; input.readerPlan = []; input.readProviders = [];
    input.moduleProviders = [{request, result: {kind: 'failure', errorKind: 'security-manager-exception', message}, observableEvents: ['denied module']}];
    class Rejected extends Session {
      constructor() { super(); this.programs = [[request]]; this.payload = [65]; }
      rejectModulePreparation(token, hasElement, kind, cause) {
        assert.equal(hasElement, request.kind === 'has-element'); assert.equal(kind, 'security-manager-exception'); assert.equal(cause, message);
        super.rejectModulePreparation(token, hasElement, kind, cause);
      }
    }
    assert.equal(run(input, Rejected).acquisitionError, null);
  }
});
test('wrong generated request IDs or URI context are preserved as acquisition failures', () => {
  class BadId extends Session { poll() { const event = super.poll(); if (event.module) event.module.id = 7; return event; } }
  assert.match(run(fixture, BadId).acquisitionError.message, /decimal module request ID/);
  class WrongOwner extends Session { poll() { const event = super.poll(); if (event.module) event.module.importerResolvedUri = 'file:///other.pkl'; return event; } }
  assert.match(run(fixture, WrongOwner).acquisitionError.message, /unconfigured exact module/);
});
test('observable policy order and complete event bytes are compared separately from output equality', () => {
  const result = run(); result.calls[0].observableEvents.reverse();
  let compared = compareModulePreparationCase(fixture, result);
  assert.equal(compared.comparisons[0].checks.stdoutBytes, true);
  assert.equal(compared.comparisons[0].checks.observableEvents, false);
  assert.equal(compared.passed, false);
  const another = run(); another.calls[1].observableEvents[0] += '\n';
  assert.equal(compareModulePreparationCase(fixture, another).passed, false);
});
test('changed provider bytes or counted-reader status cannot redefine the ordered reader plan', () => {
  let input = structuredClone(fixture); input.readProviders[0].result.bytes[0]++;
  assert.throws(() => run(input), /reader plan differs/);
  input = structuredClone(fixture); input.readProviders[1].countedReader = true;
  assert.throws(() => run(input), /invalid File Resource/);
});
test('retained output ownership is still checked after the actual session is freed', () => {
  class Borrowed extends Session { free() { for (const view of this.views) view.fill(0); } }
  assert.equal(compareModulePreparationCase(fixture, run(fixture, Borrowed)).checks.ownedResultsStable, false);
});
