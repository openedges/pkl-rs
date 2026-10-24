// Fresh public expression sessions on N-API and actual Chromium Wasm.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { execFileSync } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, relative, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runParameterizedCase, javaUtf8, compareParameterizedPayload } from './parameterized-object-session.mjs';

const script = fileURLToPath(import.meta.url);
const root = resolve(dirname(script), '../..');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';

export function admitPairs(cases, rows) {
  const expected = new Set(cases.flatMap(row => [0, 1].map(repeat => `${row.id}\0${repeat}`)));
  const observed = new Set();
  for (const row of rows) {
    const key = `${row.id}\0${row.repeat}`;
    if (!expected.has(key) || observed.has(key)) throw Error('unexpected or duplicate execution pair');
    observed.add(key);
  }
  if (observed.size !== expected.size) throw Error('missing fresh execution pair');
}

export async function compareParameterizedObjects(args) {
  if (args.length !== 4) throw Error('usage: compare-parameterized-objects.mjs OUTPUT WASM_DIR NAPI_BINARY FIXTURE_DIR');
  const [output, wasm, addon, fixture] = args.map(value => resolve(value));
  // Authenticate the independent Java authorities before admitting any case.
  const python = process.env.PKL_FIXTURE_PYTHON ?? 'python3';
  const verification = execFileSync(python, [resolve(root, 'tools/parameterized_object_fixtures.py'), fixture], { cwd: root });
  const declarations = JSON.parse(await readFile(resolve(fixture, 'cases.json')));
  const boundaries = JSON.parse(await readFile(resolve(fixture, 'oracle-boundaries.json')));
  const manifest = JSON.parse(await readFile(resolve(fixture, 'manifest.json')));
  const tracked = {};
  const wasmFiles = [];
  async function track(path) { tracked[path] = hash(await readFile(path)); }
  async function tree(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = resolve(path, entry.name);
      if (entry.isDirectory()) await tree(child);
      else if (entry.isFile()) { wasmFiles.push(child); await track(child); }
      else throw Error('nonregular artifact entry');
    }
  }
  for (const name of Object.keys(manifest.files)) await track(resolve(fixture, name));
  await track(resolve(fixture, 'manifest.json'));
  await track(script);
  await track(resolve(root, 'tools/parameterized_object_fixtures.py'));
  for (const name of ['parameterized-object-session.mjs', 'module-policy-session.mjs', 'package.json', 'package-lock.json'])
    await track(resolve(dirname(script), name));
  await track(addon); await tree(wasm);
  await track(process.execPath);
  const require = createRequire(resolve(root, 'tools/platform/package.json'));
  const { chromium } = require('playwright');
  const packageManifest = JSON.parse(await readFile(resolve(dirname(script), 'package.json')));
  const lock = JSON.parse(await readFile(resolve(dirname(script), 'package-lock.json')));
  const pin = packageManifest.devDependencies.playwright;
  if (!/^\d+\.\d+\.\d+$/.test(pin) || require('playwright/package.json').version !== pin
      || require('playwright-core/package.json').version !== pin
      || lock.packages[''].devDependencies.playwright !== pin
      || lock.packages['node_modules/playwright'].version !== pin
      || lock.packages['node_modules/playwright-core'].version !== pin) throw Error('Playwright pin mismatch');
  const cases = [];
  for (const row of declarations) {
    const bytes = await readFile(resolve(fixture, row.source));
    const source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    if (hash(bytes) !== row.source_sha256 || !Buffer.from(source).equals(bytes)) throw Error('source identity differs');
    const resources = [];
    for (const resource of row.resources) {
      const payload = await readFile(resolve(fixture, resource.source));
      if (hash(payload) !== resource.sha256) throw Error('resource identity differs');
      resources.push({ uri: resource.source_uri, bytes: Array.from(payload) });
    }
    const channels = row.observations[0].channels;
    cases.push({ id: row.id, source, uri: row.source_uri, moduleName: row.module_name,
      expression: row.expression, powerAssertions: row.power_assertions, resources,
      expected: { exitCode: row.observations[0].exit_code,
        stdout: Array.from(await readFile(resolve(fixture, channels.stdout.path))),
        stderr: Array.from(await readFile(resolve(fixture, channels.stderr.path))) } });
  }
  await mkdir(output, { recursive: false });
  await writeFile(resolve(output, 'fixture-verification.stdout.bin'), verification);
  const report = { schema: 1, scope: 'Exact public SourceSession.forExpressions output bytes; two fresh sessions per ordinary case/target. Host traces are candidate observations without Java event-order expectations.',
    argv: process.argv, cwd: process.cwd(), environment: Object.fromEntries(['LANG', 'LC_ALL', 'TZ', 'TERM', 'PKL_FIXTURE_PYTHON', 'PLAYWRIGHT_BROWSERS_PATH'].filter(key => process.env[key] != null).map(key => [key, process.env[key]])),
    fixtureManifestSha256: tracked[resolve(fixture, 'manifest.json')], caseIds: cases.map(row => row.id), repetitions: 2,
    profile: { method: 'SourceSession.forExpressions', traceMode: 'compact', hostLogger: false, hostFrameTransformer: false, powerAssertions: false },
    oracleBoundaries: boundaries.map(row => ({ id: row.id, status: 'oracle-error', candidateExecution: 'not-requested', classification: row.classification })),
    node: { version: process.version, versions: process.versions, executable: process.execPath, sha256: hash(await readFile(process.execPath)) },
    playwright: pin, targets: Object.fromEntries(['napi', 'chromium'].map(target => [target,
      { complete: false, expectedExecutions: 2 * cases.length, executions: 0, passingExecutions: 0, results: [] }])),
    infrastructureErrors: [], browserErrors: [], inputHashes: tracked };

  async function save(target, rows) {
    admitPairs(cases, rows);
    const results = [];
    for (const row of rows) {
      const entry = cases.find(value => value.id === row.id);
      const directory = resolve(output, target, row.id, 'repeat-' + row.repeat);
      await mkdir(directory, { recursive: true });
      const comparison = compareParameterizedPayload(entry.expected, row);
      const channels = {};
      for (const name of ['stdout', 'stderr']) {
        const bytes = Buffer.from(javaUtf8(row[name + 'Units']));
        await writeFile(resolve(directory, name + '.bin'), bytes);
        const units = Buffer.alloc(row[name + 'Units'].length * 2);
        row[name + 'Units'].forEach((unit, index) => units.writeUInt16BE(unit, index * 2));
        await writeFile(resolve(directory, name + '.utf16be.bin'), units);
        channels[name] = { path: relative(output, resolve(directory, name + '.bin')), sha256: hash(bytes), bytes: bytes.length };
      }
      await writeFile(resolve(directory, 'trace.json'), json({ requests: row.trace, architecture: row.architecture, acquisitionError: row.acquisitionError }));
      results.push({ id: row.id, repeat: row.repeat, ...comparison, exitCode: row.exitCode,
        disposition: row.disposition, phase: row.phase, reason: row.reason, channels,
        tracePath: relative(output, resolve(directory, 'trace.json')) });
    }
    report.targets[target] = { complete: true, executions: results.length,
      passingExecutions: results.filter(row => row.status === 'pass').length, results };
  }
  try {
    const api = require(addon);
    await save('napi', cases.flatMap(fixture => [0, 1].map(repeat => ({ ...runParameterizedCase(api, fixture), repeat }))));
  } catch (error) { report.infrastructureErrors.push({ target: 'napi', message: error.message, stack: error.stack }); }

  const routes = new Map([
    ['/session.mjs', resolve(dirname(script), 'parameterized-object-session.mjs')],
    ['/module-policy-session.mjs', resolve(dirname(script), 'module-policy-session.mjs')],
  ]);
  const server = createServer(async (request, response) => {
    try {
      const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
      if (pathname === '/') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>Expression comparison</title>'); return; }
      const path = routes.get(pathname) ?? resolve(wasm, '.' + decodeURIComponent(pathname));
      if (!routes.has(pathname) && !path.startsWith(wasm + sep)) throw Error('invalid Wasm path');
      response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' });
      response.end(await readFile(path));
    } catch { response.writeHead(404); response.end(); }
  });
  let browser;
  try {
    await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
    browser = await chromium.launch({ headless: true });
    report.browser = { version: browser.version(), executable: chromium.executablePath(), sha256: hash(await readFile(chromium.executablePath())) };
    const page = await browser.newPage();
    page.on('pageerror', error => report.browserErrors.push({ message: error.message, stack: error.stack }));
    page.on('crash', () => report.browserErrors.push({ message: 'page crashed' }));
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    const rows = await page.evaluate(async fixtures => {
      const api = await import('/pkl_wasm.js'); await api.default();
      const { runParameterizedCase } = await import('/session.mjs');
      return fixtures.flatMap(fixture => [0, 1].map(repeat => ({ ...runParameterizedCase(api, fixture), repeat })));
    }, cases);
    await save('chromium', rows);
  } catch (error) { report.infrastructureErrors.push({ target: 'chromium', message: error.message, stack: error.stack }); }
  finally {
    if (browser) await browser.close();
    if (server.listening) await new Promise(ok => server.close(ok));
  }
  report.changedInputs = [];
  async function fileSet(directory) {
    const files = [];
    for (const entry of await readdir(directory, { withFileTypes: true })) {
      const path = resolve(directory, entry.name);
      if (entry.isDirectory()) files.push(...await fileSet(path));
      else if (entry.isFile()) files.push(path);
      else throw Error('nonregular acquisition input');
    }
    return files.sort();
  }
  try {
    const actual = await fileSet(wasm);
    const expected = [...wasmFiles].sort();
    if (actual.length !== expected.length || actual.some((name, index) => name !== expected[index])) report.changedInputs.push(wasm + '#file-set');
    const actualFixture = await fileSet(fixture);
    const expectedFixture = [...Object.keys(manifest.files).map(name => resolve(fixture, name)), resolve(fixture, 'manifest.json')].sort();
    if (actualFixture.length !== expectedFixture.length || actualFixture.some((name, index) => name !== expectedFixture[index])) report.changedInputs.push(fixture + '#file-set');
  } catch (error) { report.changedInputs.push(error.message); }
  for (const [path, expected] of Object.entries(tracked)) {
    try { if (hash(await readFile(path)) !== expected) report.changedInputs.push(path); }
    catch { report.changedInputs.push(path); }
  }
  report.status = !report.infrastructureErrors.length && !report.browserErrors.length && !report.changedInputs.length
    && ['napi', 'chromium'].every(target => report.targets[target]?.executions === 2 * cases.length
      && report.targets[target].passingExecutions === 2 * cases.length) ? 'passed' : 'failed';
  await writeFile(resolve(output, 'report.json'), json(report));
  console.log(json({ status: report.status, cases: cases.length, targets: Object.fromEntries(Object.entries(report.targets)
    .map(([key, value]) => [key, { executions: value.executions, passingExecutions: value.passingExecutions }])), oracleBoundaries: boundaries.length,
    infrastructureErrors: report.infrastructureErrors, browserErrors: report.browserErrors, changedInputs: report.changedInputs }));
  if (report.status !== 'passed') process.exitCode = 1;
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === script) await compareParameterizedObjects(process.argv.slice(2));
