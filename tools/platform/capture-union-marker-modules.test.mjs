import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, cpSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { javaUtf8Bytes } from './resource-session.mjs';
import { loadModules, deriveJavaExecution, verifySessionFiles, CAPTURE_TRACKED_SOURCES } from './capture-union-marker-modules.mjs';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '../..');
const fixture = resolve(root, 'tests/fixtures/m2-union-marker-diagnostics');
const put = (root, name, value) => { const path = resolve(root, name); mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, value); };
const be = value => Buffer.from(Array.from(value, character => [character.charCodeAt(0) >> 8, character.charCodeAt(0) & 255]).flat());

test('forty distinct module bytes and shared URI remain separate', async () => {
  const modules = await loadModules(fixture);
  assert.equal(modules.length, 40);
  assert.equal(new Set(modules.map(value => value.id)).size, 40);
  assert.equal(new Set(modules.map(value => value.uri)).size, 1);
  assert.notEqual(modules[0].source, modules[1].source);
  assert.ok(modules[39].source.includes('\r\n'));
  assert.deepEqual(modules[0].rows, [{ id: 'nonunion-int', expression: 'result' }]);
  assert.ok(CAPTURE_TRACKED_SOURCES.includes('tools/union_marker_portable_compare.py'));
});

test('authored module mutation and extra module are rejected by selection', async () => {
  const temporary = mkdtempSync(resolve(tmpdir(), 'pkl-union-modules-'));
  try {
    cpSync(fixture, temporary, { recursive: true });
    writeFileSync(resolve(temporary, 'modules/00-nonunion-int.pkl'), 'typealias T = *String\nresult = 1\n');
    await assert.rejects(loadModules(temporary), /authored module bytes differ/);
    cpSync(resolve(fixture, 'modules/00-nonunion-int.pkl'), resolve(temporary, 'modules/00-nonunion-int.pkl'));
    writeFileSync(resolve(temporary, 'modules/extra.pkl'), 'result = 1\n');
    await assert.rejects(loadModules(temporary), /authored module file set differs/);
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});

test('all eighty Java commands must carry one common evaluation locale', () => {
  const sessions = Array.from({ length: 80 }, () => ({ command: ['java', '-Duser.language=en', '-Duser.country=US', '-Duser.timezone=UTC'] }));
  const provenance = { environment: { TZ: 'UTC' }, sessions,
    jdk: { properties: { 'user.language': 'ko', 'user.country': 'KR' } } };
  assert.equal(deriveJavaExecution(provenance).locale, 'en-US');
  sessions[79] = { command: ['java', '-Duser.language=ko', '-Duser.country=KR', '-Duser.timezone=UTC'] };
  assert.throws(() => deriveJavaExecution(provenance), /evaluation environment differs/);
  sessions.pop();
  assert.throws(() => deriveJavaExecution(provenance), /invalid Java execution provenance/);
});

test('one-module raw session requires exact channels and whole protocol', async () => {
  const temporary = mkdtempSync(resolve(tmpdir(), 'pkl-union-session-'));
  const module = { id: 'nonunion-int' }, folder = `000-${module.id}`;
  try {
    const stderr = '–– Pkl Error ––\nOnly type unions can have a default marker (*).\n';
    put(temporary, 'input.json', '{}\n'); put(temporary, 'process.json', '{}\n');
    put(temporary, 'candidate-trace.json', '{}\n'); put(temporary, 'child.json', '{}\n');
    put(temporary, 'process.stderr.bin', Buffer.alloc(0));
    put(temporary, 'process.stdout.bin', `session-open mode=string rows=1\ncall-enter=string-0-${module.id}\ncall-exit=string-0-${module.id} status=error\nsession-closed mode=string rows=1 public-calls=1 success=0 errors=1\n`);
    put(temporary, `${folder}/expression.bin`, Buffer.from('result'));
    put(temporary, `${folder}/expression.code-units.bin`, be('result'));
    put(temporary, `${folder}/result.status.bin`, 'error\n');
    put(temporary, `${folder}/result.stdout.bin`, Buffer.alloc(0));
    put(temporary, `${folder}/result.stdout.code-units.bin`, Buffer.alloc(0));
    put(temporary, `${folder}/result.stderr.bin`, Buffer.from(javaUtf8Bytes(Array.from(stderr, c => c.charCodeAt(0)))));
    put(temporary, `${folder}/result.stderr.code-units.bin`, be(stderr));
    for (const name of ['disposition', 'phase', 'reason', 'callback-origin'])
      put(temporary, `${folder}/candidate.${name}.bin`, name === 'disposition' ? 'language-error' : '');
    assert.equal((await verifySessionFiles(temporary, module, 'napi')).status, 'error');
    put(temporary, `${folder}/extra.bin`, 'x');
    await assert.rejects(verifySessionFiles(temporary, module, 'napi'), /session file set differs/);
  } finally { rmSync(temporary, { recursive: true, force: true }); }
});
