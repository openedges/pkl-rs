// Exhaustive Stage 34 comparison. Any mismatch remains a failed comparison.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, relative, sep, extname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';

const script = fileURLToPath(import.meta.url);
const root = resolve(dirname(script), '../..');
const [outputArg, wasmArg, addonArg] = process.argv.slice(2);
if (!outputArg || !wasmArg || !addonArg) throw Error('usage: compare-method-parents.mjs OUTPUT WASM_DIR NAPI_BINARY');
const output = resolve(outputArg);
const addon = resolve(addonArg);
const wasm = resolve(wasmArg);
const fixturePath = resolve(root, 'tools/platform/method-parent-cases.mjs');
const require = createRequire(resolve(root, 'tools/platform/package.json'));
const { chromium } = require('playwright');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';
const repetitions = 2;
const { methodParentCases: cases, methodParentContext: context } = await import(pathToFileURL(fixturePath));
if (!cases.length || new Set(cases.map(row => row.id)).size !== cases.length) throw Error('expected nonempty distinct Stage 34 fixtures');
const caseIds = cases.map(row => row.id);
const packagePath = resolve(root, 'tools/platform/package.json');
const lockPath = resolve(root, 'tools/platform/package-lock.json');
const manifest = JSON.parse(await readFile(packagePath));
const lock = JSON.parse(await readFile(lockPath));
const pin = manifest.devDependencies.playwright;
const playwright = require('playwright/package.json').version;
const playwrightCore = require('playwright-core/package.json').version;
if (!/^\d+\.\d+\.\d+$/.test(pin) || playwright !== pin || playwrightCore !== pin
    || lock.packages[''].devDependencies.playwright !== pin
    || lock.packages['node_modules/playwright'].version !== pin
    || lock.packages['node_modules/playwright-core'].version !== pin) throw Error('Playwright installation differs from manifest/lock pin');
// Refuse an existing evidence directory; never overwrite earlier captures.
await mkdir(output, { recursive: false });
const sourceHashes = {};
for (const fixture of cases) {
  const bytes = Buffer.from(fixture.source, 'utf8');
  if (sha(bytes) !== fixture.sourceSha256) throw Error('fixture source hash differs: ' + fixture.id);
  const original = await readFile(resolve(root, 'tests/fixtures/m2', fixture.id + '.pkl'));
  if (!bytes.equals(original)) throw Error('fixture source differs from selected input: ' + fixture.id);
  if (Object.hasOwn(fixture, 'expectedError') === Object.hasOwn(fixture, 'expectedValue')) throw Error('fixture must have one expected outcome: ' + fixture.id);
  sourceHashes[fixture.id] = sha(bytes);
}
await writeFile(resolve(output, 'fixtures.json'), json({ context, cases }));
await writeFile(resolve(output, 'compare-method-parents.mjs'), await readFile(script));
const artifactHashes = {};
async function recordFile(path) { artifactHashes[relative(root, path)] = sha(await readFile(path)); }
async function recordTree(path) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, entry.name);
    if (entry.isDirectory()) await recordTree(child);
    else if (entry.isFile()) await recordFile(child);
    else throw Error('unexpected artifact entry: ' + child);
  }
}
await recordFile(addon);
await recordTree(wasm);
await recordFile(fixturePath);
await recordFile(packagePath);
await recordFile(lockPath);
await recordFile(script);
const browserErrors = [];
const infrastructureErrors = [];
const report = { schema: 1, scope: `All ${cases.length} selected Stage 34 cases, two fresh evaluator calls per case on Node and Chromium; full UTF-8 values/errors without normalization.`,
  caseCount: cases.length, caseIds, caseIdsSha256: sha(Buffer.from(JSON.stringify(caseIds), 'utf8')),
  argv: process.argv, cwd: process.cwd(), context, repetitions, sourceHashes, artifactHashes,
  fixtureSha256: sha(await readFile(fixturePath)), node: process.version, nodeVersions: process.versions,
  nodeExecutable: process.execPath, nodeExecutableSha256: sha(await readFile(process.execPath)),
  playwright, playwrightCore, playwrightPin: pin, browserErrors, infrastructureErrors, targets: {} };

// The same function runs against N-API and browser Wasm; every failure is captured.
function evaluateAll(api, fixtures, settings, count) {
  const rows = [];
  for (let repetition = 0; repetition < count; repetition++) {
    for (const fixture of fixtures) {
      let status, payload;
      try {
        payload = api.evaluateLanguage(fixture.source, settings.uri, settings.moduleName,
          fixture.expression, settings.powerAssertions);
        status = typeof payload === 'string' ? 'value' : 'harness_error';
        if (status === 'harness_error') payload = 'Unexpected non-string return: ' + String(payload);
      } catch (error) {
        status = error instanceof Error ? 'error' : 'harness_error';
        payload = error instanceof Error ? error.message : String(error);
      }
      rows.push({ id: fixture.id, repetition, status, payload });
    }
  }
  return rows;
}

async function saveTarget(target, rows) {
  const dir = resolve(output, target);
  await mkdir(dir);
  const expectedPairs = new Set(cases.flatMap(fixture => Array.from({ length: repetitions },
    (_, repetition) => `${fixture.id}\u0000${repetition}`)));
  const observedPairs = new Set();
  const results = [];
  for (const row of rows) {
    const fixture = cases.find(fixture => fixture.id === row.id);
    const pair = `${row.id}\u0000${row.repetition}`;
    if (!expectedPairs.has(pair) || observedPairs.has(pair)) throw Error('unexpected or duplicate execution pair: ' + pair);
    observedPairs.add(pair);
    const expectedStatus = Object.hasOwn(fixture, 'expectedError') ? 'error' : 'value';
    const expected = Buffer.from(fixture.expectedError ?? fixture.expectedValue, 'utf8');
    const actual = Buffer.from(row.payload, 'utf8');
    const match = row.status === expectedStatus && actual.equals(expected);
    const prefix = `${row.id}.repeat-${row.repetition}`;
    await writeFile(resolve(dir, prefix + '.expected.bin'), expected);
    await writeFile(resolve(dir, prefix + '.actual.bin'), actual);
    const result = { id: row.id, repetition: row.repetition, status: match ? 'pass' : 'mismatch',
      expectedStatus, actualStatus: row.status, expectedSha256: sha(expected), actualSha256: sha(actual),
      expectedBytes: expected.length, actualBytes: actual.length,
      expectedFile: `${target}/${prefix}.expected.bin`, actualFile: `${target}/${prefix}.actual.bin` };
    await writeFile(resolve(dir, prefix + '.json'), json(result));
    results.push(result);
  }
  const mismatchIds = cases.filter(fixture => results.some(row => row.id === fixture.id && row.status !== 'pass')).map(fixture => fixture.id);
  const complete = observedPairs.size === expectedPairs.size;
  return { complete, executions: rows.length, passingExecutions: results.filter(row => row.status === 'pass').length,
    mismatchingExecutions: results.filter(row => row.status === 'mismatch').length,
    passingCases: complete ? cases.length - mismatchIds.length : 0, mismatchingCases: mismatchIds.length,
    mismatchIds, results };
}

try {
  const native = require(addon);
  report.targets.node = await saveTarget('node', evaluateAll(native, cases, context, repetitions));
} catch (error) { infrastructureErrors.push({ target: 'node', message: error.message, stack: error.stack }); }

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://127.0.0.1');
    if (url.pathname === '/') {
      response.writeHead(200, { 'Content-Type': 'text/html' });
      response.end('<!doctype html><title>Stage 34 raw comparison</title>');
      return;
    }
    const path = resolve(wasm, '.' + decodeURIComponent(url.pathname));
    if (!path.startsWith(wasm + sep)) throw Error('invalid Wasm path');
    response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' });
    response.end(await readFile(path));
  } catch { response.writeHead(404); response.end(); }
});
let browser;
try {
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
  browser = await chromium.launch({ headless: true });
  report.browser = browser.version();
  report.browserExecutable = chromium.executablePath();
  report.browserExecutableSha256 = sha(await readFile(report.browserExecutable));
  const page = await browser.newPage();
  page.on('pageerror', error => browserErrors.push({ type: 'pageerror', message: error.message, stack: error.stack }));
  page.on('crash', () => browserErrors.push({ type: 'crash' }));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const rows = await page.evaluate(async ({ fixtures, settings, count, evaluateSource }) => {
    const api = await import('/pkl_wasm.js');
    await api.default();
    const run = (0, eval)('(' + evaluateSource + ')');
    return run(api, fixtures, settings, count);
  }, { fixtures: cases, settings: context, count: repetitions, evaluateSource: evaluateAll.toString() });
  report.targets.browser = await saveTarget('browser', rows);
} catch (error) { infrastructureErrors.push({ target: 'browser', message: error.message, stack: error.stack }); }
finally {
  if (browser) await browser.close();
  if (server.listening) await new Promise(ok => server.close(ok));
}
// A concurrent build or source edit invalidates this comparison's provenance.
report.stability = { artifactHashes: {}, sourceHashes: {}, changed: [] };
async function finalHash(path) {
  try { return sha(await readFile(resolve(root, path))); }
  catch (error) {
    infrastructureErrors.push({ target: 'all', message: 'Cannot reread comparison input', path, error: error.message });
    return null;
  }
}
for (const [path, expected] of Object.entries(artifactHashes)) {
  const actual = await finalHash(path);
  report.stability.artifactHashes[path] = actual;
  if (actual !== expected) report.stability.changed.push(path);
}
for (const fixture of cases) {
  const path = `tests/fixtures/m2/${fixture.id}.pkl`;
  const actual = await finalHash(path);
  report.stability.sourceHashes[fixture.id] = actual;
  if (actual !== sourceHashes[fixture.id]) report.stability.changed.push(path);
}
if (report.stability.changed.length) infrastructureErrors.push({ target: 'all',
  message: 'Sources or artifacts changed during comparison', paths: report.stability.changed });
report.status = infrastructureErrors.length || browserErrors.length
  || !['node', 'browser'].every(target => report.targets[target]?.complete && report.targets[target].mismatchingExecutions === 0)
  ? 'failed' : 'passed';
await writeFile(resolve(output, 'execution.json'), json(report));
console.log(json({ status: report.status, output, targets: Object.fromEntries(Object.entries(report.targets).map(([target, result]) =>
  [target, { complete: result.complete, executions: result.executions, passingCases: result.passingCases,
    mismatchingCases: result.mismatchingCases, mismatchIds: result.mismatchIds }])), browserErrors, infrastructureErrors }));
if (report.status !== 'passed') process.exitCode = 1;
