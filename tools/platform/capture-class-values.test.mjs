import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { createHash } from 'node:crypto';
import { CAPTURE_TRACKED_SOURCES, METADATA_KIND, METADATA_TRACKED_SOURCES, captureTrackedSources, loadClassCases, requirePairs } from './capture-class-values.mjs';

const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const source = (name, bytes, uri = 'file:///classes/main.pkl') => ({ source: 'inputs/' + name, source_sha256: sha(bytes), uri });
const manifest = cases => ({ schema: 1, baseline: 'Pkl 0.32.0', repetitions: 2, cases });
const row = fields => ({ id: 'first', expressions: ['C'], support_modules: [],
  logger: { mode: 'capture', action: 'none', callback: '', callback_after: 0 }, ...fields });
async function directory(fn) {
  const path = await mkdtemp(join(tmpdir(), 'class-capture-'));
  try { await mkdir(join(path, 'inputs')); await fn(path); } finally { await rm(path, { recursive: true, force: true }); }
}

test('capture coverage rejects missing/duplicate/foreign pairs and coerced repetition identities', () => {
  const cases = [{ id: 'first' }, { id: 'second' }];
  const complete = cases.flatMap(row => [0, 1].map(repeat => ({ id: row.id, repeat })));
  assert.doesNotThrow(() => requirePairs(cases, complete));
  assert.throws(() => requirePairs(cases, complete.slice(1)), /missing/);
  assert.throws(() => requirePairs(cases, [...complete, complete[0]]), /duplicate/);
  assert.throws(() => requirePairs(cases, [...complete.slice(1), { id: 'foreign', repeat: 0 }]), /unexpected/);
  for (const repeat of ['0', false, true, null, -1, 2, NaN])
    assert.throws(() => requirePairs(cases, [{ id: 'first', repeat }, ...complete.slice(1)]), /invalid capture repetition/);
  assert.throws(() => requirePairs([{ id: 'first' }, { id: 'first' }], complete), /invalid capture selection/);
  assert.throws(() => requirePairs([], []), /invalid capture selection/);
});

test('primary and support sources preserve exact CRLF bytes and original path/hash identities', async () => {
  await directory(async path => {
    const main = Buffer.from('class C {}\r\n'), child = Buffer.from('module child\r\nclass D {}\r\n');
    await writeFile(join(path, 'inputs/main.pkl'), main); await writeFile(join(path, 'inputs/child.pkl'), child);
    const input = row({ ...source('main.pkl', main), support_modules: [source('child.pkl', child, 'file:///classes/child.pkl')] });
    const result = await loadClassCases(path, manifest([input]));
    assert.deepEqual(Buffer.from(result[0].source), main);
    assert.deepEqual(Buffer.from(result[0].support_modules[0].source), child);
    assert.equal(result[0].sourcePath, 'inputs/main.pkl');
    assert.equal(result[0].source_sha256, sha(main));
    assert.equal(result[0].moduleName, 'main'); assert.equal(result[0].support_modules[0].moduleName, 'child');
    assert.equal(input.source, 'inputs/main.pkl');
  });
});

test('a changed support payload cannot enter the public evaluator with an old source identity', async () => {
  await directory(async path => {
    const bytes = Buffer.from('class C {}\n'); await writeFile(join(path, 'inputs/main.pkl'), bytes);
    await writeFile(join(path, 'inputs/child.pkl'), 'changed\n');
    const input = row({ ...source('main.pkl', bytes), support_modules: [source('child.pkl', bytes, 'file:///classes/child.pkl')] });
    await assert.rejects(loadClassCases(path, manifest([input])), /source identity differs/);
  });
});

test('invalid UTF8 and BOM-stripped round trips are rejected instead of normalizing authored bytes', async () => {
  await directory(async path => {
    for (const bytes of [Buffer.from([0xff]), Buffer.from([0xef, 0xbb, 0xbf, 0x61])]) {
      await writeFile(join(path, 'inputs/main.pkl'), bytes);
      await assert.rejects(loadClassCases(path, manifest([row(source('main.pkl', bytes))])));
    }
  });
});

test('duplicate case/module identities and traversal inputs are rejected before capture', async () => {
  await directory(async path => {
    const bytes = Buffer.from('class C {}\n'); await writeFile(join(path, 'inputs/main.pkl'), bytes);
    const input = row(source('main.pkl', bytes));
    await assert.rejects(loadClassCases(path, manifest([input, input])), /invalid case identity/);
    await assert.rejects(loadClassCases(path, manifest([{ ...input, support_modules: [source('main.pkl', bytes)] }])), /duplicate authored module URI/);
    for (const name of ['inputs/../outside.pkl', 'inputs/..\\outside.pkl', '/outside.pkl', 'inputs/main.pkl\0'])
      await assert.rejects(loadClassCases(path, manifest([{ ...input, source: name }])), /invalid authored source identity/);
    await assert.rejects(loadClassCases(path, { ...manifest([input]), repetitions: '2' }), /unknown main fixture profile/);
  });
});

test('tracked-source contract includes every directly used transport and offline verification dependency', () => {
  assert.equal(new Set(CAPTURE_TRACKED_SOURCES).size, CAPTURE_TRACKED_SOURCES.length);
  for (const name of ['tools/platform/class-values-session.mjs', 'tools/platform/capture-class-values.mjs',
    'tools/platform/module-policy-session.mjs', 'tools/platform/resource-session.mjs',
    'tools/class_values_fixtures.py', 'tools/class_values_oracle.py', 'tools/class_values_exports.py',
    'tools/frame_transformer_oracle.py', 'tools/platform/package.json', 'tools/platform/package-lock.json'])
    assert.equal(CAPTURE_TRACKED_SOURCES.includes(name), true);
});


test('supplemental verification tracks both new helpers only for its explicit authority kind', () => {
  assert.deepEqual(captureTrackedSources(manifest([])), [...CAPTURE_TRACKED_SOURCES]);
  assert.deepEqual(captureTrackedSources({ ...manifest([]), kind: METADATA_KIND }),
    [...CAPTURE_TRACKED_SOURCES, ...METADATA_TRACKED_SOURCES]);
  assert.deepEqual([...METADATA_TRACKED_SOURCES].sort(),
    ['tools/class_metadata_fixtures.py', 'tools/class_metadata_oracle.py']);
  for (const kind of ['unknown', '', null, false, 0, [], {}])
    assert.throws(() => captureTrackedSources({ kind }), /unknown Class fixture/);
});
