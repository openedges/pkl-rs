import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm, copyFile, unlink } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CAPTURE_TRACKED_SOURCES, loadDataSizeFixture, requireSessionCoverage,
  validateSessionFileSet } from './capture-data-size.mjs';

const here = resolve(fileURLToPath(import.meta.url), '..');
const canonical = resolve(here, '../../tests/fixtures/m2-data-size');
const utf16 = text => Buffer.from(Array.from(text).flatMap(char => {
  const units = [];
  for (let i = 0; i < char.length; i++) units.push(char.charCodeAt(i));
  return units.flatMap(unit => [unit >> 8, unit & 255]);
}));
const fixture = { uri: 'file:///stage57/datasize-oracle.pkl', rows: [{ id: 'one', expression: '1.mb' }] };

async function temporary(fn) {
  const path = await mkdtemp(join(tmpdir(), 'data-size-capture-'));
  try { await fn(path); } finally { await rm(path, { recursive: true, force: true }); }
}

async function fakeSession(path, target = 'chromium') {
  const row = join(path, '000-one'); await mkdir(row);
  for (const [name, raw] of Object.entries({
    'expression.bin': Buffer.from('1.mb'), 'expression.code-units.bin': utf16('1.mb'),
    'result.status.bin': Buffer.from('success\n'), 'result.stdout.bin': Buffer.from('1.mb'),
    'result.stdout.code-units.bin': utf16('1.mb'), 'result.stderr.bin': Buffer.alloc(0),
    'result.stderr.code-units.bin': Buffer.alloc(0), 'candidate.disposition.bin': Buffer.alloc(0),
    'candidate.phase.bin': Buffer.alloc(0), 'candidate.reason.bin': Buffer.alloc(0),
    'candidate.callback-origin.bin': Buffer.alloc(0),
  })) await writeFile(join(row, name), raw);
  const protocol = 'session-open mode=string rows=1\ncall-enter=string-0-one\n'
    + 'call-exit=string-0-one status=success\n'
    + 'session-closed mode=string rows=1 public-calls=1 success=1 errors=0 datasize-rows=0 repeated-export-calls=0\n';
  for (const [name, raw] of Object.entries({
    'input.json': '{}\n', 'candidate-trace.json': '{}\n', 'process.json': '{}\n',
    'process.stdout.bin': protocol, 'process.stderr.bin': '', ...(target === 'napi' ? { 'child.json': '{}\n' } : {}),
  })) await writeFile(join(path, name), raw);
}

test('canonical source, ordered TSV and URI are loaded by fixed identities', async () => {
  const selected = await loadDataSizeFixture(canonical);
  assert.equal(selected.uri, 'file:///stage57/datasize-oracle.pkl');
  assert.equal(selected.rows.length, 255);
  assert.deepEqual(selected.rows[0], { id: 'int-b', expression: '7.b' });
  assert.deepEqual(selected.rows.at(-1), { id: 'too-many-arguments', expression: '1.mb.toUnit("kb","mb")' });
  assert.equal(selected.source.includes('marker = "stage57"'), true);
  assert.equal(Object.keys(selected.inputHashes).length, 3);
});

test('mutated, missing and extra authored input cannot enter the canonical capture', async () => {
  await temporary(async path => {
    for (const name of ['cases.json', 'module.pkl', 'queries.tsv'])
      await copyFile(join(canonical, name), join(path, name));
    for (const [name, change] of [['cases.json', b => Buffer.concat([b, Buffer.from(' ')] )],
      ['module.pkl', b => Buffer.concat([b, Buffer.from('changed')])],
      ['queries.tsv', b => Buffer.concat([b, Buffer.from('extra\t1.b\n')])]]) {
      const file = join(path, name), original = await readFile(file);
      await writeFile(file, change(original));
      await assert.rejects(loadDataSizeFixture(path), /canonical input identity differs/);
      await writeFile(file, original);
    }
    await unlink(join(path, 'module.pkl'));
    await assert.rejects(loadDataSizeFixture(path));
  });
});

test('session coverage rejects missing, duplicate and wrong URI/row counts', async () => {
  const selected = await loadDataSizeFixture(canonical);
  const one = { repeat: 0, rowCount: 255, uri: selected.uri };
  const two = { repeat: 1, rowCount: 255, uri: selected.uri };
  assert.doesNotThrow(() => requireSessionCoverage(selected, [one, two]));
  assert.throws(() => requireSessionCoverage(selected, [one]), /missing/);
  assert.throws(() => requireSessionCoverage(selected, [one, one]), /duplicate/);
  assert.throws(() => requireSessionCoverage(selected, [one, { ...two, rowCount: 254 }]), /wrong/);
  assert.throws(() => requireSessionCoverage(selected, [one, { ...two, uri: 'file:///foreign.pkl' }]), /wrong/);
  for (const repeat of ['1', true, null, 2, -1])
    assert.throws(() => requireSessionCoverage(selected, [one, { ...two, repeat }]), /invalid/);
});

test('row file-set, UTF16 and whole-session protocol validation rejects tampering', async () => {
  await temporary(async path => {
    await fakeSession(path);
    const good = await validateSessionFileSet(path, fixture, 'chromium');
    assert.equal(good.rowCount, 1);
    assert.deepEqual(good.statusCounts, { success: 1 });
    const missing = join(path, '000-one/result.stdout.bin');
    const original = await readFile(missing); await unlink(missing);
    await assert.rejects(validateSessionFileSet(path, fixture, 'chromium'));
    await writeFile(missing, original);
    const extra = join(path, '000-one/result.error-class.bin');
    await writeFile(extra, 'fabricated Java metadata');
    await assert.rejects(validateSessionFileSet(path, fixture, 'chromium'), /missing or extra/);
    await unlink(extra);
    const units = join(path, '000-one/result.stdout.code-units.bin');
    const originalUnits = await readFile(units); await writeFile(units, Buffer.from([0, 1]));
    await assert.rejects(validateSessionFileSet(path, fixture, 'chromium'), /UTF8 channel differs/);
    await writeFile(units, originalUnits);
    const protocol = join(path, 'process.stdout.bin');
    const originalProtocol = await readFile(protocol); await writeFile(protocol, Buffer.from('wrong\n'));
    await assert.rejects(validateSessionFileSet(path, fixture, 'chromium'), /protocol differs/);
    await writeFile(protocol, originalProtocol);
    assert.equal((await validateSessionFileSet(path, fixture, 'chromium')).fileCount, 16);
  });
});

test('N-API child identity file is required and transport sources are explicit', async () => {
  await temporary(async path => {
    await fakeSession(path, 'napi');
    assert.equal((await validateSessionFileSet(path, fixture, 'napi')).fileCount, 17);
    await unlink(join(path, 'child.json'));
    await assert.rejects(validateSessionFileSet(path, fixture, 'napi'), /missing or extra/);
  });
  assert.equal(new Set(CAPTURE_TRACKED_SOURCES).size, CAPTURE_TRACKED_SOURCES.length);
  for (const name of ['tools/platform/capture-data-size.mjs', 'tools/platform/data-size-session.mjs',
    'tools/platform/module-policy-session.mjs', 'tools/platform/resource-session.mjs',
    'tools/data_size_fixtures.py', 'tools/data_size_oracle.py', 'tools/platform/package-lock.json'])
    assert.equal(CAPTURE_TRACKED_SOURCES.includes(name), true);
});
