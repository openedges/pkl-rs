import test from 'node:test';
import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, mkdtemp, rm} from 'node:fs/promises';
import {resolve, dirname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArguments, validateHostFacts, validateFactoryFacts, auditRequests, auditObservations, trackedReader, serveAsset, hash, PINS} from './compare-modulekey.mjs';

const repo = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const golden = resolve(process.env.MODULEKEY_FIXTURES ?? resolve(repo, 'tests/fixtures/m2-modulekey-spi'));
const sidecar = resolve(process.env.MODULEKEY_HOST_METADATA ?? resolve(repo, 'tests/fixtures/m2-modulekey-host/host-metadata.json'));
const bundle = JSON.parse(await readFile(resolve(golden, 'fixtures.json')));
const authored = JSON.parse(await readFile(resolve(golden, 'original/cases.json')));
const metadata = JSON.parse(await readFile(sidecar));
const argv = ['output', 'wasm', 'addon', golden, sidecar, 'verifier', 'package.json', 'chrome',
  '1'.repeat(64), 'artifact-pins.json', '2'.repeat(64), 'spi132', 'provided-primary'];

test('driver requires explicit bounded cohort, entrypoint, browser pin and artifact pin', () => {
  const parsed = parseArguments(['--preflight-only', ...argv]);
  assert.equal(parsed.preflightOnly, true); assert.equal(parsed.entrypoint, 'provided-primary');
  assert.equal(parsed.browserSha256, '1'.repeat(64)); assert.equal(parsed.addon, resolve('addon'));
  for (const [index, value] of [[8, 'not-a-sha'], [10, ''], [11, 'all'], [12, 'implicit']]) {
    const invalid = [...argv]; invalid[index] = value; assert.throws(() => parseArguments(invalid));
  }
  assert.throws(() => parseArguments(argv.slice(0, -1)), /usage:/);
});

test('authentic source-derived Host table joins all original cases while selecting exactly spi132', async () => {
  assert.equal(hash(await readFile(resolve(golden, 'manifest.json'))), PINS.fixture);
  assert.equal(hash(await readFile(sidecar)), PINS.sidecar);
  const result = validateHostFacts(bundle, authored, metadata, 'spi132');
  assert.equal(result.selected.length, 20); assert.equal(result.unexecuted.length, 16);
  assert.equal(result.selected.reduce((n, f) => n + f.expected.length, 0), 132);
  assert.equal(result.unexecuted.reduce((n, f) => n + f.expected.length, 0), 80);
});

test('a sidecar cannot rename, redirect, change sources, or alter custom cache snapshots', () => {
  for (const [field, value] of [['inferredName', 'hidden'], ['selectedUri', 'pkl:base'],
    ['resolvedUri', 'memo:/redirect.pkl'], ['sourceUri', 'memo:/source.pkl'],
    ['sourceText', 'tag = "replacement"'], ['cacheMode', 'false'], ['fragmentPaths', true]]) {
    const changed = structuredClone(metadata); changed.profiles[0].customModules[0][field] = value;
    assert.throws(() => validateHostFacts(bundle, authored, changed, 'spi132'), /custom provider|source-derived custom/);
  }
});

test('file bindings cannot redirect URI, inferred name or capabilities and must have exact coverage', () => {
  for (const [field, value] of [['resolvedUri', 'file:///replacement.pkl'], ['inferredName', 'replacement'],
    ['sourcePath', 'outside.pkl'], ['isCached', false], ['fragmentPaths', true]]) {
    const changed = structuredClone(metadata); changed.profiles[0].fileModules[0][field] = value;
    assert.throws(() => validateHostFacts(bundle, authored, changed, 'spi132'), /source-derived file/);
  }
  const missing = structuredClone(metadata); missing.profiles[0].fileModules.pop();
  assert.throws(() => validateHostFacts(bundle, authored, missing, 'spi132'), /file binding coverage/);
  const duplicate = structuredClone(metadata); duplicate.profiles[1].id = duplicate.profiles[0].id;
  assert.throws(() => validateHostFacts(bundle, authored, duplicate, 'spi132'), /duplicate Host profile/);
});

test('transport audit rejects replayed IDs/tokens and u64 narrowing or ordering errors', () => {
  const row = {calls: [{}, {}], requests: [
    {callIndex: 0, token: 'token-a', module: {id: '9007199254740993'}},
    {callIndex: 1, token: 'token-b', module: {id: '9007199254740994'}},
  ]};
  assert.equal(auditRequests(row).passed, true);
  for (const change of [{id: '9007199254740993'}, {id: '9007199254740992'},
    {id: '18446744073709551616'}, {id: 9007199254740994}]) {
    const changed = structuredClone(row); Object.assign(changed.requests[1].module, change);
    assert.equal(auditRequests(changed).passed, false);
  }
  const token = structuredClone(row); token.requests[1].token = token.requests[0].token;
  assert.equal(auditRequests(token).passed, false);
  const index = structuredClone(row); index.requests[1].callIndex = 2;
  assert.equal(auditRequests(index).passed, false);
});

test('HTTP server hashes the actual served copy before headers and preserves empty404 on read failure', async () => {
  const parent = resolve(repo, 'target/m2/stage49-modulekey-driver-tests-private'); await mkdir(parent, {recursive: true});
  const temp = await mkdtemp(resolve(parent, 'http-')), asset = resolve(temp, 'modulekey-session.mjs');
  await writeFile(asset, 'export const value = 1;\n'); const tracker = trackedReader(), ledger = [];
  const server = createServer((req, res) => { void serveAsset(req, res, new Map([['/helper.mjs', asset]]), tracker.read, ledger); });
  await new Promise(done => server.listen(0, '127.0.0.1', done));
  const origin = `http://127.0.0.1:${server.address().port}`;
  try {
    const first = await fetch(origin + '/helper.mjs'); assert.equal(first.status, 200);
    assert.equal(await first.text(), 'export const value = 1;\n');
    assert.equal(tracker.hashes[asset], hash(await readFile(asset))); assert.equal(ledger[0].sha256, tracker.hashes[asset]);
    await writeFile(asset, 'export const value = 2;\n');
    const changed = await fetch(origin + '/helper.mjs'); assert.equal(changed.status, 404); assert.equal(await changed.text(), '');
    const missing = await fetch(origin + '/absent.js'); assert.equal(missing.status, 404); assert.equal(await missing.text(), '');
    assert.deepEqual(await tracker.changed(), [asset]);
  } finally { await new Promise(done => server.close(done)); await rm(temp, {recursive: true}); }
});


test('all212 and factory80 selections preserve the entire ledger without implicit exclusions', () => {
  const all = validateHostFacts(bundle, authored, metadata, 'all212');
  assert.equal(all.selected.length, 36); assert.equal(all.unexecuted.length, 0);
  const factory = validateHostFacts(bundle, authored, metadata, 'factory80');
  assert.equal(factory.selected.length, 16); assert.equal(factory.unexecuted.length, 20);
  for (const cohort of ['spi132', 'factory80', 'all212']) {
    const args = [...argv]; args[11] = cohort; assert.equal(parseArguments(args).cohort, cohort);
  }
});

test('factory IO and exact NoLoader misses derive from authenticated helper inputs and actual registry trace', async () => {
  const helper = (await readFile(resolve(golden, 'original/AdmissionProbe.java'))).toString('utf8');
  validateFactoryFacts(bundle, metadata, helper);
  for (const mutate of [m => { m.profiles[0].factoryFailures[0].message = 'replacement'; },
    m => { m.profiles[0].factoryFailures[0].scheme = 'THROWING'; },
    m => { m.profiles[0].noLoaderUris = ['noLoader:x']; },
    m => { m.profiles.find(p => p.noLoaderUris.length).noLoaderUris = []; }]) {
    const changed = structuredClone(metadata); mutate(changed);
    assert.throws(() => validateFactoryFacts(bundle, changed, helper), /factory failure|NoLoader/);
  }
  assert.throws(() => validateFactoryFacts(bundle, metadata, helper.replace('stage48-factory-create', 'replacement')), /IOException rule/);
});


test('actual observation stream preserves default false versus consumed completion and joins API/results', () => {
  const call = {index: 0, status: 'error', stdoutUnits: [], stderrUnits: [55]};
  const events = [
    {kind: 'constructor-call', method: 'forExpressions'}, {kind: 'constructor-return'},
    {kind: 'api-call', method: 'startPath', args: [0, 0]}, {kind: 'api-started', method: 'startPath'},
    {kind: 'module-request', token: 't', module: {id: '1'}},
    {kind: 'host-call', method: 'completeDefaultModule', args: ['t', 0]},
    {kind: 'host-return', method: 'completeDefaultModule', returnedUndefined: false, value: false},
    {kind: 'host-call', method: 'rejectModuleKey', args: ['t', false, 'no-loader', null, 0]},
    {kind: 'host-return', method: 'rejectModuleKey', returnedUndefined: true, value: null},
    {kind: 'api-result', call: structuredClone(call)}, {kind: 'close-call'}, {kind: 'close-return', terminalSlots: 0},
  ];
  const row = {profile: 'provided-primary', calls: [call], requests: [{callIndex: 0, token: 't', module: {id: '1'}}],
    observations: events.map((e, sequence) => ({sequence, callIndex: sequence >= 10 ? 1 : 0, ...e})),
    architecture: {terminalSlots: 0}, constructorError: null, acquisitionError: null};
  const f = {expressions: ['unused'], operations: [{kind: 'evaluate'}]};
  assert.equal(auditObservations(row, f).passed, true);
  for (const mutate of [r => { r.observations[6].value = true; }, r => { r.observations[7].args[0] = 'foreign'; },
    r => { r.observations[6].returnedUndefined = true; }, r => { r.observations[9].call.stderrUnits = []; },
    r => { r.observations[5].sequence = 9; }, r => { r.observations.pop(); }]) {
    const changed = structuredClone(row); mutate(changed); assert.equal(auditObservations(changed, f).passed, false);
  }
});
