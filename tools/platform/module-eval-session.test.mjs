// Synthetic driver logic checks, not Java compatibility observations.
import test from 'node:test';
import assert from 'node:assert/strict';
import { evaluateModuleFixtures } from './module-eval-session.mjs';
import { compareModulePayload } from './compare-module-eval.mjs';

const fixture = { id: 'synthetic', sourceText: 'x = (', uri: 'file:///same%20physical/source.pkl',
  moduleName: 'source', arguments: ['eval', '--no-cache', '--no-project', '--settings', 'pkl:settings', 'original/source.pkl'] };
const expected = { stdout: Buffer.alloc(0), stderr: Buffer.from('error\n\n'), exitCode: 1 };
const outcome = () => ({ phase: 'parsing', disposition: 'language-error', reason: null,
  stdout: new Uint8Array(), stderr: new Uint8Array(expected.stderr), exitCode: 1 });

test('passes original source, physical URI, module name and argv without replacing the module with an expression', () => {
  const calls = [];
  const rows = evaluateModuleFixtures({ evaluateModuleFrontend(...args) { calls.push(args); return outcome(); } }, [fixture]);
  assert.equal(calls.length, 2);
  for (const call of calls) assert.deepEqual(call, [fixture.sourceText, fixture.uri, fixture.moduleName, fixture.arguments]);
  assert.deepEqual(rows.map(row => row.repeat), [0, 1]);
  assert.deepEqual(rows[0].result.stderr, [...expected.stderr]);
});

test('compares complete byte vectors including final newlines and process-result field', () => {
  assert.equal(compareModulePayload(expected, outcome(), 'ordinary-language-error').status, 'pass');
  const missingNewline = { ...outcome(), stderr: new Uint8Array(Buffer.from('error\n')) };
  assert.equal(compareModulePayload(expected, missingNewline, 'ordinary-language-error').status, 'mismatch');
  assert.equal(compareModulePayload(expected, { ...outcome(), exitCode: 0 }, 'ordinary-language-error').status, 'mismatch');
});

test('matching unsupported/internal/oracle failures cannot count as ordinary compatibility passes', () => {
  const unsupported = compareModulePayload(expected, { ...outcome(), disposition: 'unsupported' }, 'ordinary-language-error');
  assert.equal(unsupported.rawMatch, true); assert.equal(unsupported.status, 'unsupported');
  const internal = compareModulePayload(expected, { ...outcome(), disposition: 'internal-failure' }, 'ordinary-language-error');
  assert.equal(internal.rawMatch, true); assert.equal(internal.status, 'non-language-failure');
  const oracle = compareModulePayload(expected, outcome(), 'internal-error');
  assert.equal(oracle.rawMatch, true); assert.equal(oracle.status, 'oracle-error');
});

test('invalid transport and thrown exceptions remain per-call acquisition failures', () => {
  const rows = evaluateModuleFixtures({ evaluateModuleFrontend() { return { ...outcome(), stderr: 'error\n\n' }; } }, [fixture]);
  assert.equal(rows.length, 2); assert(rows.every(row => row.result === null && row.acquisitionError));
  const thrown = evaluateModuleFixtures({ evaluateModuleFrontend() { throw Error('transport'); } }, [fixture]);
  assert(thrown.every(row => row.acquisitionError.message === 'transport'));
  assert.equal(compareModulePayload(expected, null, 'ordinary-language-error').status, 'acquisition-failed');
});

test('rejects self-consistent acquisition rows with the wrong physical URI, argv or deterministic environment', async () => {
  const { assertModuleProcessContext } = await import('./compare-module-eval.mjs');
  const { createHash } = await import('node:crypto');
  const { resolve } = await import('node:path');
  const { pathToFileURL } = await import('node:url');
  const row = { id: 'synthetic-context', input: 'original/source.pkl', upstream_case: {
    cwd: '.', command: ['eval', '--no-cache', '--no-project', '--settings', 'pkl:settings', 'original/source.pkl'],
    environment: { LANG: 'C.UTF-8', LC_ALL: 'C.UTF-8', NO_COLOR: '1', TERM: 'dumb', TZ: 'UTC' } } };
  const profile = { workspace: '/synthetic/workspace', artifact: '/synthetic/pkl.jar', jdk: { executable: '/synthetic/java' },
    environment: { PATH: '/usr/bin:/bin', HOME: '/synthetic/profile/home', TMPDIR: '/synthetic/profile/tmp',
      TMP: '/synthetic/profile/tmp', TEMP: '/synthetic/profile/tmp', LANG: 'C', LC_ALL: 'C', NO_COLOR: '1', TERM: 'dumb', TZ: 'UTC' } };
  const lock = { jvm_options: ['-Dfile.encoding=UTF-8'] };
  profile.command = [profile.jdk.executable, ...lock.jvm_options, `-Duser.home=${profile.environment.HOME}`,
    `-Djava.io.tmpdir=${profile.environment.TMPDIR}`, '-cp', profile.artifact, 'org.pkl.cli.Main'];
  const state = resolve(profile.workspace, '.state', createHash('sha256').update(row.id).digest('hex').slice(0, 24));
  const full = { ...profile.environment, ...row.upstream_case.environment, HOME: state + '/home', TMPDIR: state + '/tmp', TMP: state + '/tmp', TEMP: state + '/tmp' };
  const record = { cwd: profile.workspace, moduleUri: pathToFileURL(resolve(profile.workspace, row.input)).href,
    command: profile.command.map(arg => arg.startsWith('-Duser.home=') ? `-Duser.home=${full.HOME}`
      : arg.startsWith('-Djava.io.tmpdir=') ? `-Djava.io.tmpdir=${full.TMPDIR}` : arg).concat(row.upstream_case.command),
    environment: Object.fromEntries(Object.entries(full).filter(([key]) => key !== 'PATH')), fullAllowlistedEnvironment: full };
  assert.doesNotThrow(() => assertModuleProcessContext(row, profile, record, lock));
  for (const change of [copy => { copy.moduleUri = 'file:///different/source.pkl'; }, copy => { copy.cwd = '/different'; },
    copy => { copy.command.push('--test-mode'); }, copy => { copy.environment.LANG = 'C'; },
    copy => { copy.fullAllowlistedEnvironment.TMPDIR = '/different/tmp'; }]) {
    const forged = structuredClone(record); change(forged);
    assert.throws(() => assertModuleProcessContext(row, profile, forged, lock));
  }
});

test('rejects a self-consistent forged baseline and mismatched raw generated files', async () => {
  const { validateModuleFilesystem, moduleOracleOutcome } = await import('./compare-module-eval.mjs');
  const { createHash } = await import('node:crypto');
  const sha = bytes => createHash('sha256').update(bytes).digest('hex');
  const row = { input: 'original/source.pkl', input_sha256: 'a'.repeat(64), upstream_case: {
    resources: [{ path: 'original/source.err', sha256: 'e'.repeat(64) }] } };
  // Keys are deliberately already in compat's canonical order.
  const baseline = { 'original/source.err': { kind: 'file', sha256: 'e'.repeat(64) },
    'original/source.pkl': { kind: 'file', sha256: 'a'.repeat(64) } };
  const encodedHash = value => sha(Buffer.from(JSON.stringify(value, null, 2) + '\n'));
  const result = { exit_code: 1, timed_out: false, files: {}, workspace_changes: {} };
  const process = { workspaceBeforeSha256: encodedHash(baseline), workspaceAfterSha256: encodedHash(baseline), result };
  assert.doesNotThrow(() => validateModuleFilesystem(row, baseline, process, result, {}));
  const forgedBaseline = structuredClone(baseline); forgedBaseline[row.input].sha256 = 'b'.repeat(64);
  const forgedProcess = { ...process, workspaceBeforeSha256: encodedHash(forgedBaseline), workspaceAfterSha256: encodedHash(forgedBaseline) };
  assert.throws(() => validateModuleFilesystem(row, forgedBaseline, forgedProcess, result, {}), /original resource/);
  const withFile = { ...result, files: { 'generated/result': sha(Buffer.from('exact')) } };
  assert.throws(() => validateModuleFilesystem(row, baseline, { ...process, result: withFile }, withFile,
    { 'generated/result': Buffer.from('different') }), /captured Java files/);
  const ordinary = Buffer.from('–– Pkl Error ––\nerror\n');
  assert.equal(moduleOracleOutcome(result, {}, ordinary), 'ordinary-language-error');
  for (const [metadata, files] of [[{ ...result, timed_out: true }, {}], [{ ...result, exit_code: 2 }, {}],
    [{ ...result, workspace_changes: { unexpected: {} } }, {}], [result, { 'generated/unexpected': Buffer.from('bytes') }]]) {
    const derived = moduleOracleOutcome(metadata, files, ordinary);
    assert.notEqual(derived, 'ordinary-language-error');
    assert.equal(compareModulePayload(expected, outcome(), derived).status, 'oracle-error');
  }
});

test('retains optional Node versus null Wasm reason metadata independently of display reason and raw bytes', () => {
  const values = [Object.fromEntries(Object.entries(outcome()).filter(([key]) => key !== 'reason')),
    { ...outcome(), reason: undefined }, { ...outcome(), reason: null }, { ...outcome(), reason: 'dependency' }];
  let index = 0;
  const rows = evaluateModuleFixtures({ evaluateModuleFrontend() { return values[index++]; } }, [fixture], 4);
  assert.deepEqual(rows.map(row => row.result.reasonKind), ['absent', 'undefined', 'null', 'string']);
  assert.deepEqual(rows.map(row => row.result.reasonPresent), [false, true, true, true]);
  assert.deepEqual(rows.map(row => row.result.reason), [null, null, null, 'dependency']);
  assert(rows.every(row => compareModulePayload(expected, row.result, 'ordinary-language-error').status === 'pass'));
});
