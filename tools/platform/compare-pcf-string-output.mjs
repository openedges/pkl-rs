// Actual Host UTF-16 input and owned output bytes, using the same driver on both targets.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runPcfStringOutputFixtures } from './pcf-string-output-session.mjs';

const script = fileURLToPath(import.meta.url);
const root = resolve(dirname(script), '../..');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';

export function parsePcfStringOutputFixture(bytes, metadata) {
  if (metadata.schema_version !== 1 || hash(bytes) !== metadata.fixture_sha256
      || !Number.isSafeInteger(metadata.case_count) || metadata.case_count < 1
      || metadata.cases?.length !== metadata.case_count
      || metadata.rejected_or_unverified?.length !== 0)
    throw Error('invalid or changed Java fixture metadata');
  let offset = 0;
  const take = length => {
    if (!Number.isSafeInteger(length) || length < 0 || length > bytes.length - offset)
      throw Error('truncated Java fixture');
    const value = bytes.subarray(offset, offset + length); offset += length; return value;
  };
  const count = () => take(4).readUInt32BE();
  const blob = () => take(count());
  const text = () => {
    const raw = blob(), value = new TextDecoder('utf-8', { fatal: true }).decode(raw);
    if (!Buffer.from(value, 'utf8').equals(raw)) throw Error('source UTF8 roundtrip differs');
    return value;
  };
  if (!take(8).equals(Buffer.from('PKLPCF1\0')) || count() !== metadata.case_count)
    throw Error('invalid Java fixture header');
  const fixtures = [], seen = new Set();
  for (const expected of metadata.cases) {
    const id = text(), source = text(), unitCount = count(), rawUnits = take(unitCount * 2);
    const valueUnits = Array.from({ length: unitCount }, (_, index) => rawUnits.readUInt16BE(index * 2));
    const expectedBytes = blob();
    if (!/^[a-z0-9-]+$/.test(id) || seen.has(id) || id !== expected.id
        || hash(Buffer.from(source, 'utf8')) !== expected.source_sha256
        || unitCount !== expected.value_code_units || hash(rawUnits) !== expected.value_utf16be_sha256
        || expectedBytes.length !== expected.expected_bytes || hash(expectedBytes) !== expected.expected_sha256)
      throw Error('Java fixture row binding differs: ' + id);
    seen.add(id); fixtures.push({ id, source, valueUnits, expectedBytes });
  }
  if (offset !== bytes.length) throw Error('trailing Java fixture bytes');
  return fixtures;
}

export async function comparePcfStringOutput(args) {
  if (![3, 4].includes(args.length))
    throw Error('usage: compare-pcf-string-output.mjs OUTPUT WASM_DIR NAPI_BINARY [FIXTURE_DIR]');
  const [output, wasm, addon] = args.slice(0, 3).map(path => resolve(path));
  const fixtureDir = resolve(args[3] ?? resolve(root, 'tests/fixtures/m2-pcf-string-output'));
  const bytes = await readFile(resolve(fixtureDir, 'cases.bin'));
  const metadataBytes = await readFile(resolve(fixtureDir, 'metadata.json'));
  const metadata = JSON.parse(metadataBytes);
  if (metadata.uri !== 'file:///stage40-pcf-string-output.pkl') throw Error('unexpected Java source URI');
  const fixtures = parsePcfStringOutputFixture(bytes, metadata);
  const transportFixtures = fixtures.map(({ id, source, valueUnits }) => ({ id, source, valueUnits }));
  const require = createRequire(resolve(root, 'tools/platform/package.json'));
  const { chromium } = require('playwright');
  const manifest = JSON.parse(await readFile(resolve(root, 'tools/platform/package.json')));
  const lock = JSON.parse(await readFile(resolve(root, 'tools/platform/package-lock.json')));
  const pin = manifest.devDependencies.playwright;
  const playwright = require('playwright/package.json').version;
  const playwrightCore = require('playwright-core/package.json').version;
  if (!/^\d+\.\d+\.\d+$/.test(pin) || playwright !== pin || playwrightCore !== pin
      || lock.packages[''].devDependencies.playwright !== pin
      || lock.packages['node_modules/playwright'].version !== pin
      || lock.packages['node_modules/playwright-core'].version !== pin) throw Error('Playwright pin mismatch');
  await mkdir(output, { recursive: false });
  const hashes = {};
  const record = async path => { hashes[path] = hash(await readFile(path)); };
  async function tree(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = resolve(path, entry.name);
      if (entry.isDirectory()) await tree(child);
      else if (entry.isFile()) await record(child);
      else throw Error('unexpected artifact entry: ' + child);
    }
  }
  for (const path of [script, resolve(dirname(script), 'pcf-string-output-session.mjs'),
    resolve(fixtureDir, 'cases.bin'), resolve(fixtureDir, 'metadata.json'), addon,
    resolve(root, 'tools/platform/package.json'), resolve(root, 'tools/platform/package-lock.json')]) await record(path);
  await tree(wasm);
  await writeFile(resolve(output, 'fixtures.json'), json(transportFixtures));
  for (const path of [script, resolve(dirname(script), 'pcf-string-output-session.mjs')])
    await writeFile(resolve(output, path === script ? 'compare-pcf-string-output.mjs' : 'pcf-string-output-session.mjs'), await readFile(path));
  const report = { schema: 1, scope: 'Complete fixed Java actual-output String matrix through shared SourceSession Host UTF16/byte transport. Fresh evaluator per case and repetition on actual Node and Chromium; no formatter-derived expectations.',
    argv: process.argv, cwd: process.cwd(), uri: metadata.uri, cases: fixtures.length, repetitions: 2,
    oracleAcquisition: metadata.acquisition, fixtureSha256: hash(bytes), metadataSha256: hash(metadataBytes),
    node: process.version, nodeExecutable: process.execPath, nodeExecutableSha256: hash(await readFile(process.execPath)),
    playwright, playwrightCore, playwrightPin: pin, artifactAndInputHashes: hashes,
    targets: {}, infrastructureErrors: [], browserErrors: [] };
  const expectedPairs = fixtures.flatMap(fixture => [0, 1].map(repeat => fixture.id + ':' + repeat));
  async function save(target, rows) {
    if (rows.length !== expectedPairs.length || rows.some((row, index) => row.id + ':' + row.repeat !== expectedPairs[index]))
      throw Error('missing, reordered or duplicate case repetition');
    const results = [];
    for (const row of rows) {
      const fixture = fixtures.find(fixture => fixture.id === row.id);
      const directory = resolve(output, target, row.id, String(row.repeat));
      await mkdir(directory, { recursive: true });
      await writeFile(resolve(directory, 'java.stdout.bin'), fixture.expectedBytes);
      await writeFile(resolve(directory, 'java.stderr.bin'), Buffer.alloc(0));
      const actual = row.completion;
      const stdout = actual ? Buffer.from(actual.stdout) : null, stderr = actual ? Buffer.from(actual.stderr) : null;
      if (stdout) await writeFile(resolve(directory, 'stdout.bin'), stdout);
      if (stderr) await writeFile(resolve(directory, 'stderr.bin'), stderr);
      const channels = { stdout: stdout !== null && stdout.equals(fixture.expectedBytes),
        stderr: stderr !== null && stderr.length === 0, completion: actual?.kind === 'bytes' };
      const rawMatch = Object.values(channels).every(Boolean);
      const architectureMatch = row.architecture.valueReads === 1 && row.architecture.terminalSlots === 0
        && row.architecture.ownedBytesStable && row.architecture.inputUnitsStable;
      const status = row.acquisitionError ? 'acquisition-failed' : !rawMatch ? 'mismatch'
        : !architectureMatch ? 'architecture-failed' : 'pass';
      const result = { id: row.id, repeat: row.repeat, status, channels, rawMatch, architectureMatch,
        architecture: row.architecture, acquisitionError: row.acquisitionError, actualKind: actual?.kind ?? null,
        expectedStdoutSha256: hash(fixture.expectedBytes), actualStdoutSha256: stdout ? hash(stdout) : null,
        actualStderrSha256: stderr ? hash(stderr) : null };
      await writeFile(resolve(directory, 'result.json'), json(result)); results.push(result);
    }
    return { complete: true, executions: results.length, results,
      statusCounts: results.reduce((counts, row) => ({ ...counts, [row.status]: (counts[row.status] ?? 0) + 1 }), {}) };
  }
  try { report.targets.node = await save('node', runPcfStringOutputFixtures(require(addon), transportFixtures, metadata.uri)); }
  catch (error) { report.infrastructureErrors.push({ target: 'node', message: error.message, stack: error.stack }); }
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, 'http://127.0.0.1');
      if (url.pathname === '/') { response.end('<!doctype html><title>PCF String output verification</title>'); return; }
      const helper = url.pathname === '/pcf-string-output-session.mjs';
      const path = helper ? resolve(dirname(script), 'pcf-string-output-session.mjs') : resolve(wasm, '.' + decodeURIComponent(url.pathname));
      if (!helper && !path.startsWith(wasm + sep)) throw Error('invalid Wasm path');
      response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' });
      response.end(await readFile(path));
    } catch { response.writeHead(404); response.end(); }
  });
  await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
  let browser;
  try {
    const executable = chromium.executablePath();
    report.browserExecutable = executable; report.browserExecutableSha256 = hash(await readFile(executable));
    browser = await chromium.launch({ headless: true }); report.browser = browser.version();
    const page = await browser.newPage();
    page.on('pageerror', error => report.browserErrors.push({ message: error.message, stack: error.stack }));
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    const rows = await page.evaluate(async ({ fixtures, uri }) => {
      const api = await import('/pkl_wasm.js'); await api.default();
      const { runPcfStringOutputFixtures } = await import('/pcf-string-output-session.mjs');
      return runPcfStringOutputFixtures(api, fixtures, uri);
    }, { fixtures: transportFixtures, uri: metadata.uri });
    report.targets.browser = await save('browser', rows);
  } catch (error) { report.infrastructureErrors.push({ target: 'browser', message: error.message, stack: error.stack }); }
  finally { if (browser) await browser.close(); await new Promise(resolve => server.close(resolve)); }
  report.changedInputs = [];
  for (const [path, digest] of Object.entries(hashes)) if (hash(await readFile(path)) !== digest) report.changedInputs.push(path);
  report.compatibilityComplete = ['node', 'browser'].every(target => report.targets[target]?.complete
    && report.targets[target].results.every(row => row.status === 'pass'))
    && !report.infrastructureErrors.length && !report.browserErrors.length && !report.changedInputs.length;
  await writeFile(resolve(output, 'execution.json'), json(report));
  console.log(json({ targets: Object.fromEntries(Object.entries(report.targets).map(([name, value]) => [name, value.statusCounts])), compatibilityComplete: report.compatibilityComplete }));
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === script) {
  const report = await comparePcfStringOutput(process.argv.slice(2));
  process.exitCode = report.compatibilityComplete ? 0 : 1;
}
