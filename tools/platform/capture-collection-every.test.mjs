import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { CAPTURE_TRACKED_SOURCES, validateSessionFileSet, loadEveryFixture, requireSessionCoverage } from './capture-collection-every.mjs';
import { deriveJavaExecution } from './capture-collection-conversions.mjs';
import { javaUtf8Bytes } from './resource-session.mjs';

const fixturePath = resolve('tests/fixtures/m2-collection-every-oracle');
const fields = ['level', 'message', 'module-uri', 'member', 'position'];
const pair = async (dir, stem, value) => {
  // Array.from splits supplementary characters; use explicit UTF-16 indexing.
  const raw = Array.from({ length: value.length }, (_, i) => value.charCodeAt(i));
  await writeFile(join(dir, stem + '.bin'), Buffer.from(javaUtf8Bytes(raw)));
  await writeFile(join(dir, stem + '.code-units.bin'), Buffer.from(raw.flatMap(n => [n >> 8, n & 255])));
};

async function candidate(logs) {
  const parent = await mkdtemp(join(tmpdir(), 'every-capture-test-'));
  const row = join(parent, '000-one'); await mkdir(row);
  await writeFile(join(parent, 'input.json'), '{}\n');
  await writeFile(join(parent, 'candidate-trace.json'), '{}\n');
  await writeFile(join(parent, 'process.json'), '{}\n');
  await writeFile(join(parent, 'child.json'), '{}\n');
  await writeFile(join(parent, 'process.stderr.bin'), Buffer.alloc(0));
  await writeFile(join(parent, 'process.stdout.bin'), Buffer.from(
    'session-open mode=string rows=1\ncall-enter=string-0-one\n'
    + 'call-exit=string-0-one status=success\n'
    + 'session-closed mode=string rows=1 public-calls=1 success=1 errors=0\n'));
  await pair(row, 'expression', 'List(1).every((x) -> true)');
  await pair(row, 'result.stdout', 'true'); await pair(row, 'result.stderr', '');
  await writeFile(join(row, 'result.status.bin'), 'success\n');
  await writeFile(join(row, 'logs.count.bin'), `${logs.length}\n`);
  for (const [i, record] of logs.entries()) for (const field of fields)
    await pair(row, `log-${String(i).padStart(3, '0')}.${field}`, record[field]);
  for (const field of ['disposition', 'phase', 'reason', 'callback-origin'])
    await writeFile(join(row, `candidate.${field}.bin`), '');
  return { parent, row, fixture: { rows: [{ id: 'one', expression: 'List(1).every((x) -> true)' }] } };
}
const log = { level: 'trace', message: '🙂\ud800', 'module-uri': 'repl:text', member: 'null', position: '1:1-1:1' };
test('dynamic logger event files are admitted only at their own declared count', async t => {
  const { parent, row, fixture } = await candidate([log, log]);
  t.after(() => rm(parent, { recursive: true, force: true }));
  assert.equal((await validateSessionFileSet(parent, fixture, 'napi')).fileCount, 6 + 2 * 10 + 7 + 4 + 1);
  await rm(join(row, 'log-001.member.code-units.bin'));
  await assert.rejects(validateSessionFileSet(parent, fixture, 'napi'));
});
test('zero logs and extra event files have distinct file-set outcomes', async t => {
  const { parent, row, fixture } = await candidate([]);
  t.after(() => rm(parent, { recursive: true, force: true }));
  assert.equal((await validateSessionFileSet(parent, fixture, 'napi')).statusCounts.success, 1);
  await pair(row, 'log-000.message', 'unexpected');
  await assert.rejects(validateSessionFileSet(parent, fixture, 'napi'), /extra every capture file/);
});
test('logger count requires exact ASCII bytes', async t => {
  const { parent, row, fixture } = await candidate([]);
  t.after(() => rm(parent, { recursive: true, force: true }));
  await writeFile(join(row, 'logs.count.bin'), Buffer.from([0xb0, 0x0a]));
  await assert.rejects(validateSessionFileSet(parent, fixture, 'napi'));
});
test('fixture ordered 64 rows and two session coverage stay fixed', async () => {
  const fixture = await loadEveryFixture(fixturePath);
  assert.equal(fixture.uri, 'file:///stage68/collection-every.pkl');
  assert.equal(fixture.rows.length, 64);
  const rows = [0, 1].map(repeat => ({ repeat, uri: fixture.uri, rowCount: 64 }));
  requireSessionCoverage(fixture, rows);
  assert.throws(() => requireSessionCoverage(fixture, rows.slice(0, 1)), /coverage/);
});
test('Java locale is derived from both pinned JVM commands', () => {
  const command = ['java', '-Duser.language=en', '-Duser.country=US', '-Duser.timezone=UTC'];
  const provenance = { environment: { TZ: 'UTC', LANG: 'C' }, sessions: [command, [...command]],
    jdk_properties: { 'user.language': 'ko', 'user.country': 'KR' } };
  assert.equal(deriveJavaExecution(provenance).locale, 'en-US');
  assert.equal(deriveJavaExecution(provenance).preflightLocale, 'ko-KR');
  provenance.sessions[1] = ['java', '-Duser.language=fr', '-Duser.country=FR', '-Duser.timezone=UTC'];
  assert.throws(() => deriveJavaExecution(provenance), /locale or timezone/);
});
test('canonical browser route resolves the local session imports', async () => {
  const source = await readFile(new URL('./capture-collection-every.mjs', import.meta.url), 'utf8');
  const session = await readFile(new URL('./every-logger-session.mjs', import.meta.url), 'utf8');
  assert.match(source, /\['\/module-policy-session\.mjs'/);
  assert.match(source, /\['\/resource-session\.mjs'/);
  assert.match(session, /from '\.\/module-policy-session\.mjs'/);
  assert.match(session, /from '\.\/resource-session\.mjs'/);
  assert.equal(new URL('./module-policy-session.mjs', 'http://127.0.0.1/session.mjs').pathname,
    '/module-policy-session.mjs');
});
test('canonical capture pins its logger transport, comparator, verifier, and tests', () => {
  for (const name of ['tools/platform/capture-collection-every.mjs',
    'tools/platform/capture-collection-every.test.mjs',
    'tools/platform/every-logger-session.mjs',
    'tools/platform/every-logger-session.test.mjs',
    'tools/collection_every_compare.py',
    'tools/tests/test_collection_every_compare.py',
    'tools/collection_predicate_fixtures.py'])
    assert.ok(CAPTURE_TRACKED_SOURCES.includes(name), name);
  assert.ok(CAPTURE_TRACKED_SOURCES.every(name => !name.startsWith('target/m2/')));
});
