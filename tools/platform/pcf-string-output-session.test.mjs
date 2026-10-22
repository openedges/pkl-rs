import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { runPcfStringOutputCase } from './pcf-string-output-session.mjs';
import { parsePcfStringOutputFixture } from './compare-pcf-string-output.mjs';

const uri = 'file:///stage40-pcf-string-output.pkl';
const fixture = { id: 'synthetic', source: 'value=read("life:value")', valueUnits: [0xd800, 0, 0xdc00, 0xd83d, 0xde00] };
function fakeApi(events, { mutate = false, mutateOnFree = false, slots = 0 } = {}) {
  const observed = { inputs: [], configured: [] };
  class SourceSession {
    static forOutput(...args) { observed.arguments = args; return new SourceSession(); }
    startOutputBytes() { this.events = events(); }
    poll() { const event = this.events.shift(); this.last = event.bytes ?? event.errorBytes ?? this.last; return event; }
    collect() {}
    respondString(token, units) { observed.inputs.push({ token, units }); }
    respondConfigured(token) { observed.configured.push(token); }
    close() { if (mutate && this.last) this.last[0] ^= 255; }
    free() { if (mutateOnFree && this.last) this.last[0] ^= 255; }
    heapSlots() { return slots; }
  }
  return { api: { SourceSession }, observed };
}

test('passes exact lone-surrogate UTF16 Host values and preserves output byte vectors', () => {
  const { api, observed } = fakeApi(() => [
    { kind: 'read', uri: 'prop:pkl.outputFormat', token: 'configured' },
    { kind: 'read', uri: 'life:value', token: 'value' },
    { kind: 'bytes', bytes: Uint8Array.from([0, 255, 13, 10]) },
  ]);
  const row = runPcfStringOutputCase(api, fixture, uri);
  assert.equal(row.acquisitionError, null);
  assert(observed.inputs[0].units instanceof Uint16Array);
  assert.deepEqual(Array.from(observed.inputs[0].units), fixture.valueUnits);
  assert.deepEqual(observed.configured, ['configured']);
  assert.deepEqual(observed.arguments.slice(0, 4), [fixture.source, uri, 'stage40-pcf-string-output', []]);
  assert.deepEqual(row.completion, { kind: 'bytes', stdout: [0, 255, 13, 10], stderr: [] });
  assert.equal(row.architecture.valueReads, 1);
  assert.equal(row.architecture.terminalSlots, 0);
  assert.equal(row.architecture.ownedBytesStable, true);
});

test('retains complete error bytes without substituting a display error', () => {
  const { api } = fakeApi(() => [{ kind: 'error', errorBytes: Uint8Array.from([255, 0, 10]), error: new Uint16Array([65]) }]);
  const row = runPcfStringOutputCase(api, fixture, uri);
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(row.completion, { kind: 'error', stdout: [], stderr: [255, 0, 10] });
});

test('reports ownership and terminal-root failures independently of captured bytes', () => {
  const { api } = fakeApi(() => [{ kind: 'bytes', bytes: Uint8Array.from([1, 2]) }], { mutate: true, slots: 2 });
  const row = runPcfStringOutputCase(api, fixture, uri);
  assert.deepEqual(row.completion.stdout, [1, 2]);
  assert.equal(row.architecture.ownedBytesStable, false);
  assert.equal(row.architecture.terminalSlots, 2);
  const afterFree = fakeApi(() => [{ kind: 'bytes', bytes: Uint8Array.from([1, 2]) }], { mutateOnFree: true });
  assert.equal(runPcfStringOutputCase(afterFree.api, fixture, uri).architecture.ownedBytesStable, false);
});

test('unknown reads and malformed output transports remain acquisition failures', () => {
  for (const event of [{ kind: 'read', uri: 'file:///unexpected', token: 'x' },
    { kind: 'bytes', bytes: [1] }, { kind: 'error', errorBytes: 'error' }, { kind: 'module' }]) {
    const { api } = fakeApi(() => [event]);
    assert.notEqual(runPcfStringOutputCase(api, fixture, uri).acquisitionError, null);
  }
  assert.notEqual(runPcfStringOutputCase({}, fixture, uri).acquisitionError, null);
});

const hash = bytes => createHash('sha256').update(bytes).digest('hex');
function packed(rows) {
  const parts = [Buffer.from('PKLPCF1\0')];
  const count = n => { const b = Buffer.alloc(4); b.writeUInt32BE(n); parts.push(b); };
  const blob = b => { count(b.length); parts.push(b); };
  count(rows.length);
  const cases = [];
  for (const row of rows) {
    blob(Buffer.from(row.id)); blob(Buffer.from(row.source)); count(row.valueUnits.length);
    const units = Buffer.alloc(2 * row.valueUnits.length);
    row.valueUnits.forEach((unit, index) => units.writeUInt16BE(unit, 2 * index)); parts.push(units);
    const output = Buffer.from([0, 255, 13, 10]); blob(output);
    cases.push({ id: row.id, source_sha256: hash(Buffer.from(row.source)), value_code_units: row.valueUnits.length,
      value_utf16be_sha256: hash(units), expected_bytes: output.length, expected_sha256: hash(output) });
  }
  const bytes = Buffer.concat(parts);
  return { bytes, metadata: { schema_version: 1, fixture_sha256: hash(bytes), case_count: rows.length,
    rejected_or_unverified: [], cases } };
}

test('fixture parser binds every source, UTF16 input and raw output without trailing records', () => {
  const { bytes, metadata } = packed([fixture]);
  const rows = parsePcfStringOutputFixture(bytes, metadata);
  assert.deepEqual(rows[0].valueUnits, fixture.valueUnits);
  assert.deepEqual(rows[0].expectedBytes, Buffer.from([0, 255, 13, 10]));
  for (const changed of [bytes.subarray(0, bytes.length - 1), Buffer.concat([bytes, Buffer.from([0])])]) {
    assert.throws(() => parsePcfStringOutputFixture(changed, { ...metadata, fixture_sha256: hash(changed) }));
  }
  const wrongSource = structuredClone(metadata); wrongSource.cases[0].source_sha256 = '0'.repeat(64);
  assert.throws(() => parsePcfStringOutputFixture(bytes, wrongSource), /row binding/);
  const duplicate = packed([fixture, fixture]);
  assert.throws(() => parsePcfStringOutputFixture(duplicate.bytes, duplicate.metadata), /row binding/);
});

test('complete checked-in192 fixture decodes against its original Java metadata', async () => {
  const directory = new URL('../../tests/fixtures/m2-pcf-string-output/', import.meta.url);
  const bytes = await readFile(new URL('cases.bin', directory));
  const metadata = JSON.parse(await readFile(new URL('metadata.json', directory)));
  const fixtures = parsePcfStringOutputFixture(bytes, metadata);
  assert.equal(fixtures.length, 192);
  assert.equal(metadata.verified_java_evaluations, 384);
  assert(fixtures.some(row => row.valueUnits.includes(0xd800)));
  assert(fixtures.some(row => row.valueUnits.includes(0xdc00)));
  assert(fixtures.some(row => row.valueUnits.includes(0)));
});
