import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile, mkdir, mkdtemp, writeFile, rm} from 'node:fs/promises';
import {createServer} from 'node:http';
import {resolve, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArguments, validateExtendedFacts, operationArguments, compareRawRows,
  auditExtendedObservations, combineSessionComparison, aggregateComparisons,
  auditRequests, serveAsset, trackedReader, hash, PINS} from './compare-modulekey-extended.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const golden = resolve(process.env.MODULEKEY_EXTENDED_FIXTURES ?? resolve(repo, 'tests/fixtures/m2-modulekey-extended'));
const bundle = JSON.parse(await readFile(resolve(golden, 'fixtures.json')));
const metadata = JSON.parse(await readFile(resolve(golden, 'host-metadata.json')));
const argv = ['output', 'wasm', 'addon', golden, resolve(golden, 'host-metadata.json'), 'verifier',
  'package.json', 'chrome', '1'.repeat(64), 'artifact-pins.json', '2'.repeat(64), 'all944', 'public'];

test('driver requires explicit public entry, original cohort and browser/artifact identities', () => {
  assert.equal(parseArguments(['--preflight-only', ...argv]).preflightOnly, true);
  for (const [index, value] of [[8, 'invalid'], [10, ''], [11, 'all212'], [12, 'provided-primary']]) {
    const invalid = [...argv]; invalid[index] = value; assert.throws(() => parseArguments(invalid));
  }
  assert.throws(() => parseArguments(argv.slice(0, -1)), /usage:/);
});

test('authentic fixture retains every original session, intent failure and internal boundary', async () => {
  assert.equal(hash(await readFile(resolve(golden, 'manifest.json'))), PINS.fixture);
  const all = validateExtendedFacts(bundle, metadata, 'all944');
  assert.equal(all.selected.length, 140); assert.equal(all.unexecuted.length, 0);
  assert.equal(all.profiles.size, 35);
  for (const [cohort, count] of Object.entries({java01: 444, java02: 188, java03: 40, java04: 104,
    java05: 48, java06: 80, java07: 40})) {
    const selected = validateExtendedFacts(bundle, metadata, cohort);
    assert.equal(selected.selected.reduce((n, f) => n + f.expected.length, 0), count);
    assert.equal([...selected.selected, ...selected.unexecuted].length, 140);
  }
});

test('session/source joins reject unknown profiles, duplicate identities and invalid state operations', () => {
  for (const mutate of [b => { b.fixtures[0].providerProfileId = 'missing'; },
    b => { b.fixtures[1] = structuredClone(b.fixtures[0]); },
    b => { b.fixtures[0].operations[0].sourceProfileId = 'missing'; },
    b => { b.fixtures[0].operations.unshift({kind: 'source', value: 'undefined'}); },
    b => { b.scopeLedger.ordinary++; }]) {
    const changed = structuredClone(bundle); mutate(changed);
    assert.throws(() => validateExtendedFacts(changed, metadata, 'all944'));
  }
  const changed = structuredClone(metadata); changed.profiles[1].id = changed.profiles[0].id;
  assert.throws(() => validateExtendedFacts(bundle, changed, 'all944'), /unique provider/);
});

test('public args preserve URI-only null versus empty contents and per-call URI', () => {
  const p = {sources: {empty: {kind: 'text', text: ''}}, sourceProfiles: {
    uri: {kind: 'uri', uri: 'memo:/same', contents: null},
    contents: {kind: 'contents', uri: 'memo:/same', sourceId: 'empty'},
    repl: {kind: 'repl', uri: 'repl:text', sourceId: 'empty'},
  }};
  assert.deepEqual(operationArguments(p, {kind: 'evaluate', sourceProfileId: 'uri', expression: 'x'}), ['memo:/same', null, 'x', 0]);
  assert.deepEqual(operationArguments(p, {kind: 'evaluate', sourceProfileId: 'contents', expression: 'x'}), ['memo:/same', '', 'x', 0]);
  assert.deepEqual(operationArguments(p, {kind: 'evaluate', sourceProfileId: 'repl', expression: 'x'}), ['repl:text', '', 'x', 0]);
  const malformed = structuredClone(p); malformed.sourceProfiles.uri.contents = '';
  assert.throws(() => operationArguments(malformed, {kind: 'evaluate', sourceProfileId: 'uri', expression: 'x'}));
});

test('raw comparator independently checks original JavaUTF8, status bytes and source association', () => {
  const f = {expected: [{status: 'success', statusRawHex: '737563636573730a', sourceProfileId: 'p',
    stdoutUnits: [0xd800], stderrUnits: [], stdoutHex: '3f', stderrHex: '', scope: 'intent-not-reached-diagnostic'}]};
  const row = {calls: [{index: 0, status: 'success', statusRawHex: '737563636573730a', sourceProfileId: 'p', stdoutUnits: [0xd800], stderrUnits: []}]};
  assert.equal(compareRawRows(f, row).allRawMatch, true);
  assert.equal(compareRawRows(f, row).diagnosticMatched, 1);
  for (const mutate of [r => { r.calls[0].sourceProfileId = 'other'; },
    r => { r.calls[0].statusRawHex = '73756363657373'; }, r => { r.calls.push({...r.calls[0]}); }]) {
    const changed = structuredClone(row); mutate(changed); assert.equal(compareRawRows(f, changed).allRawMatch, false);
  }
  const wrongEncoder = structuredClone(f); wrongEncoder.expected[0].stdoutHex = 'efbfbd';
  assert.equal(compareRawRows(wrongEncoder, row).allRawMatch, false);
});

function observedFixture() {
  const profile = {sources: {one: {kind: 'text', text: ''}}, sourceProfiles: {
    a: {kind: 'uri', uri: 'memo:/a', contents: null}, b: {kind: 'contents', uri: 'memo:/b', sourceId: 'one'},
  }};
  const fixture = {operations: [{kind: 'source', value: 'one'}, {kind: 'factory', value: 'builtin'},
    {kind: 'cache', value: 'off'}, {kind: 'policy', value: 'allow'},
    {kind: 'evaluate', sourceProfileId: 'a', expression: 'x'}, {kind: 'gc'},
    {kind: 'evaluate', sourceProfileId: 'b', expression: 'y'}]};
  const calls = [0, 1].map(index => ({index, sourceProfileId: index === 0 ? 'a' : 'b',
    status: 'success', statusRawHex: '737563636573730a', stdoutUnits: [55], stderrUnits: []}));
  fixture.expected = calls.map(call => ({...structuredClone(call), stdoutHex: '37', stderrHex: '', scope: 'ordinary'}));
  const observations = [], emit = (callIndex, kind, fields = {}) => observations.push({sequence: observations.length, callIndex, kind, ...fields});
  emit(0, 'constructor-call', {method: 'moduleEvaluator'}); emit(0, 'constructor-return');
  emit(0, 'source-config', {state: 'one'}); emit(0, 'factory-config', {customFactory: false});
  emit(0, 'cache-config', {cached: false}); emit(0, 'policy-config', {mode: 'allow'});
  emit(0, 'api-call', {method: 'evaluateModuleSource', args: ['memo:/a', null, 'x', 0]});
  emit(0, 'api-started', {method: 'evaluateModuleSource'});
  emit(0, 'module-request', {token: 't', module: {id: '9007199254740993'}});
  emit(0, 'host-call', {method: 'completeDefaultModule', args: ['t', 0]});
  emit(0, 'host-return', {method: 'completeDefaultModule', returnedUndefined: false, value: false});
  emit(0, 'host-call', {method: 'completeImportUri', args: ['t', 'k', 'memo:/resolved', 0]});
  emit(0, 'host-return', {method: 'completeImportUri', returnedUndefined: true, value: null});
  emit(0, 'api-result', {call: structuredClone(calls[0])});
  emit(1, 'explicit-gc-call'); emit(1, 'explicit-gc-return');
  emit(1, 'api-call', {method: 'evaluateModuleSource', args: ['memo:/b', '', 'y', 0]});
  emit(1, 'api-started', {method: 'evaluateModuleSource'}); emit(1, 'api-result', {call: structuredClone(calls[1])});
  emit(2, 'close-call'); emit(2, 'close-return', {terminalSlots: 0});
  return {fixture, profile, actual: {calls, observations, requests: [{callIndex: 0, token: 't', module: {id: '9007199254740993'}}],
    constructorError: null, acquisitionError: null,
    architecture: {terminalSlots: 0, ownedResultsStable: true, releasedHostKeys: true}}};
}

test('actual observation audit joins configuration order, contents, default false and API results', () => {
  const {fixture, profile, actual} = observedFixture();
  assert.equal(auditExtendedObservations(actual, fixture, profile).passed, true);
  for (const mutate of [r => { r.observations[2].state = 'other'; },
    r => { r.observations[3].customFactory = true; }, r => { r.observations[5].mode = 'deny-repl'; },
    r => { r.observations[10].value = true; }, r => { r.observations[16].args[1] = null; },
    r => { r.observations[13].call.stdoutUnits = []; }, r => { r.observations[11].args[0] = 'foreign'; },
    r => { r.observations.pop(); }]) {
    const changed = structuredClone(actual); mutate(changed);
    assert.equal(auditExtendedObservations(changed, fixture, profile).passed, false);
  }
});

test('complete source/factory/policy/API sequence accepts authored empty GC operand and rejects nonempty', () => {
  const {fixture, profile, actual} = observedFixture();
  fixture.operations[5].value = '';
  assert.equal(auditExtendedObservations(actual, fixture, profile).passed, true);
  for (const value of ['unexpected', ' ', '\n']) {
    fixture.operations[5].value = value;
    const audit = auditExtendedObservations(actual, fixture, profile);
    assert.equal(audit.passed, false); assert.equal(audit.failures.includes('nonempty-gc-operand'), true);
  }
  const original = structuredClone(bundle);
  const operation = original.fixtures.find(f => f.cohort === 'java03').operations.find(op => op.kind === 'gc');
  assert.equal(operation.value, '');
  operation.value = 'not-zero-argument';
  assert.throws(() => validateExtendedFacts(original, metadata, 'all944'), /zero-argument GC/);
});

test('matching completed raw APIs remain separate from acquisition and cleanup failures', () => {
  const {fixture, profile, actual} = observedFixture();
  const helper = {rawApiPassed: true, acquisitionIntegrityPassed: true, ordinaryCompatibilityPassed: true,
    cases: fixture.expected.map((e, index) => ({index, ordinaryCompatible: true}))};
  assert.equal(combineSessionComparison(fixture, actual, profile, helper).acquisitionIntegrityPassed, true);
  for (const mutate of [r => { r.acquisitionError = {message: 'after completed APIs'}; },
    r => { r.architecture.terminalSlots = 1; r.observations.at(-1).terminalSlots = 1; },
    r => { r.architecture.ownedResultsStable = false; }, r => { r.architecture.releasedHostKeys = false; }]) {
    const changed = structuredClone(actual); mutate(changed);
    const comparison = combineSessionComparison(fixture, changed, profile, helper);
    assert.equal(comparison.rawApiPassed, true); assert.equal(comparison.raw.matched, 2);
    assert.equal(comparison.acquisitionIntegrityPassed, false); assert.equal(comparison.ordinaryCompatibilityPassed, false);
    const total = aggregateComparisons([comparison]);
    assert.equal(total.rawApiPassed, true); assert.equal(total.acquisitionIntegrityPassed, false);
    assert.equal(total.intendedOrdinaryCompatible, 0); assert.equal(total.rawByteMatches, 2);
  }
});

test('free failure is an explicit observed acquisition failure with retained raw matches', () => {
  const {fixture, profile, actual} = observedFixture();
  actual.acquisitionError = {message: 'free failed'};
  actual.observations.push({sequence: actual.observations.length, callIndex: 2, kind: 'free-throw', message: 'free failed'});
  assert.equal(auditExtendedObservations(actual, fixture, profile).passed, true);
  const comparison = combineSessionComparison(fixture, actual, profile,
    {rawApiPassed: true, acquisitionIntegrityPassed: false, ordinaryCompatibilityPassed: false, cases: []});
  assert.equal(comparison.rawApiPassed, true); assert.equal(comparison.acquisitionIntegrityPassed, false);
  assert.equal(comparison.observations.failures.includes('unknown-observation-kind'), false);
});

test('typed request identity audit retains u64 strings and rejects replay/foreign API association', () => {
  const {actual} = observedFixture(); assert.equal(auditRequests(actual).passed, true);
  for (const mutate of [r => { r.requests[0].module.id = 9007199254740993; },
    r => { r.requests.push(structuredClone(r.requests[0])); }, r => { r.requests[0].callIndex = 2; }]) {
    const changed = structuredClone(actual); mutate(changed); assert.equal(auditRequests(changed).passed, false);
  }
});

test('HTTP response guard hashes actual copied assets before headers and retains empty404', async () => {
  const privateRoot = resolve(repo, 'target/m2/stage50-modulekey-extended-driver-tests-private');
  await mkdir(privateRoot, {recursive: true}); const temp = await mkdtemp(resolve(privateRoot, 'http-'));
  const file = resolve(temp, 'helper.mjs'); await writeFile(file, 'export const n=1;\n');
  const tracker = trackedReader(), ledger = [];
  const server = createServer((req, res) => { void serveAsset(req, res, new Map([['/helper.mjs', file]]), tracker.read, ledger); });
  await new Promise(done => server.listen(0, '127.0.0.1', done)); const base = `http://127.0.0.1:${server.address().port}`;
  try {
    const response = await fetch(base + '/helper.mjs'); assert.equal(response.status, 200); await response.arrayBuffer();
    assert.equal(ledger[0].sha256, hash(await readFile(file)));
    await writeFile(file, 'export const n=2;\n'); const changed = await fetch(base + '/helper.mjs');
    assert.equal(changed.status, 404); assert.equal(await changed.text(), '');
    const missing = await fetch(base + '/unknown'); assert.equal(missing.status, 404); assert.equal(await missing.text(), '');
    assert.deepEqual(await tracker.changed(), [file]);
  } finally { await new Promise(done => server.close(done)); await rm(temp, {recursive: true}); }
});
