// Compare every raw trace field and full callback protocol on both JS targets.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, relative, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runTraceCase } from './trace-session.mjs';

const script = fileURLToPath(import.meta.url);
const root = resolve(dirname(script), '../..');
const args = process.argv.slice(2);
if (args.length !== 4) throw Error('usage: compare-trace-sessions.mjs OUTPUT WASM_DIR NAPI_BINARY JAVA_ACQUISITION');
const [output, wasm, addon, oracleRoot] = args.map(path => resolve(path));
const require = createRequire(resolve(root, 'tools/platform/package.json'));
const { chromium } = require('playwright');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';
const reportBytes = await readFile(resolve(oracleRoot, 'report.json'));
const oracle = JSON.parse(reportBytes);
if (oracle.schema !== 1 || oracle.repetitions !== 2 || !oracle.cases.length)
  throw Error('expected nonempty source-driven trace acquisition');
const cases = oracle.cases.map(row => ({ ...row }));
const fixtures = [];
for (const fixture of cases) {
  const source = await readFile(resolve(root, fixture.source));
  if (sha(source) !== fixture.origin.input_sha256) throw Error('original source changed: ' + fixture.id);
  const sourceText = source.toString('utf8');
  if (!Buffer.from(sourceText, 'utf8').equals(source)) throw Error('source is not lossless UTF-8: ' + fixture.id);
  fixtures.push({ ...fixture, sourceText });
}
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
async function record(path) { hashes[path] = sha(await readFile(path)); }
async function recordTree(path) {
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, entry.name);
    if (entry.isDirectory()) await recordTree(child);
    else if (entry.isFile()) await record(child);
    else throw Error('unexpected artifact entry: ' + child);
  }
}
await record(addon);
await recordTree(wasm);
for (const path of [script, resolve(dirname(script), 'trace-session.mjs'), resolve(oracleRoot, 'report.json'),
  resolve(root, 'tools/platform/package.json'), resolve(root, 'tools/platform/package-lock.json')]) await record(path);
for (const fixture of fixtures) await record(resolve(root, fixture.source));
await writeFile(resolve(output, 'fixtures.json'), json(fixtures));
const report = { schema: 1, scope: 'Complete source-session trace payloads, two fresh sessions per input on Node and Chromium. Java oracle errors remain failures.',
  argv: process.argv, cwd: process.cwd(), oracleReportSha256: sha(reportBytes), repetitions: oracle.repetitions,
  excludedComparisonChannels: { 'error-class.bin': 'JVM exception class names remain retained in the Java acquisition as provenance. Status, complete error bytes, callback protocol and all recorded log fields are compared.' },
  selectedCases: fixtures.length, node: process.version, nodeExecutable: process.execPath,
  nodeExecutableSha256: sha(await readFile(process.execPath)), playwright, playwrightCore, playwrightPin: pin,
  artifactAndInputHashes: hashes, targets: {}, infrastructureErrors: [], browserErrors: [] };

async function compare(target, rows) {
  const results = [];
  const expectedPairs = new Set(fixtures.flatMap(fixture => [0, 1].map(repeat => fixture.id + ':' + repeat)));
  const seen = new Set();
  for (const row of rows) {
    const pair = row.id + ':' + row.repeat;
    if (!expectedPairs.has(pair) || seen.has(pair)) throw Error('unexpected or duplicate result: ' + pair);
    seen.add(pair);
    const java = resolve(oracleRoot, 'cases', row.id, String(row.repeat), 'java');
    const origin = oracle.results.find(result => result.id === row.id && result.repeat === row.repeat);
    if (!origin) throw Error('missing Java execution: ' + pair);
    const expected = {};
    for (const entry of await readdir(java, { withFileTypes: true })) {
      if (entry.isFile() && entry.name.endsWith('.bin') && !entry.name.endsWith('.error-class.bin')) {
        const path = resolve(java, entry.name);
        const bytes = await readFile(path);
        const recorded = origin.comparisons.find(item => item.name === entry.name)?.expected_sha256;
        if (recorded !== sha(bytes)) throw Error('Java payload binding mismatch: ' + path);
        expected[entry.name] = bytes;
        await record(path);
      }
    }
    const dest = resolve(output, target, row.id, String(row.repeat));
    await mkdir(dest, { recursive: true });
    const names = [...new Set([...Object.keys(expected), ...Object.keys(row.payloads)])].sort();
    const comparisons = [];
    for (const name of names) {
      if (!/^[a-z0-9.-]+\.bin$/.test(name)) throw Error('unsafe payload name');
      const left = expected[name];
      const right = Object.hasOwn(row.payloads, name) ? Buffer.from(row.payloads[name]) : null;
      if (left) await writeFile(resolve(dest, 'java.' + name), left);
      if (right) await writeFile(resolve(dest, name), right);
      comparisons.push({ name, expectedSha256: left ? sha(left) : null, actualSha256: right ? sha(right) : null,
        rawEqual: left != null && right != null && left.equals(right) });
    }
    const javaExecuted = origin.processes.java.status === 'executed' && origin.processes.java.exit_code === 0
      && origin.missing_calls.java.length === 0;
    const oracleErrors = Object.entries(expected).filter(([name, bytes]) => name.endsWith('.status.bin')
      && bytes.equals(Buffer.from('oracle-error\n'))).map(([name]) => name);
    const status = row.acquisitionError || !javaExecuted ? 'acquisition-failed'
      : oracleErrors.length ? 'oracle-error' : comparisons.length && comparisons.every(item => item.rawEqual) ? 'pass' : 'mismatch';
    await writeFile(resolve(dest, 'architecture.json'), json(row.architecture));
    const result = { id: row.id, repeat: row.repeat, status, oracleErrors, acquisitionError: row.acquisitionError,
      architecture: row.architecture, comparisons };
    await writeFile(resolve(dest, 'result.json'), json(result));
    results.push(result);
  }
  return { complete: seen.size === expectedPairs.size, results,
    statusCounts: results.reduce((counts, row) => ({ ...counts, [row.status]: (counts[row.status] ?? 0) + 1 }), {}) };
}

try {
  const api = require(addon);
  const rows = fixtures.flatMap(fixture => [0, 1].map(repeat => ({ id: fixture.id, repeat, ...runTraceCase(api, fixture) })));
  report.targets.node = await compare('node', rows);
} catch (error) { report.infrastructureErrors.push({ target: 'node', message: error.message, stack: error.stack }); }

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://127.0.0.1');
    if (url.pathname === '/') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>Trace session verification</title>'); return; }
    let path;
    if (url.pathname === '/trace-session.mjs') path = resolve(dirname(script), 'trace-session.mjs');
    else {
      path = resolve(wasm, '.' + decodeURIComponent(url.pathname));
      if (!path.startsWith(wasm + sep)) throw Error('invalid Wasm path');
    }
    const assetBytes = await readFile(path); response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' });
    response.end(assetBytes);
  } catch { if (!response.headersSent) { response.writeHead(404); response.end(); } else response.destroy(); }
});
let browser;
try {
  await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
  report.browserExecutable = chromium.executablePath();
  report.browserExecutableSha256 = sha(await readFile(report.browserExecutable));
  report.browserLaunchOptions = {headless: true, executablePath: report.browserExecutable};
  browser = await chromium.launch(report.browserLaunchOptions);
  report.browser = browser.version();
  const page = await browser.newPage();
  page.on('pageerror', error => report.browserErrors.push({ type: 'pageerror', message: error.message, stack: error.stack }));
  page.on('crash', () => report.browserErrors.push({ type: 'crash' }));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const rows = await page.evaluate(async fixtures => {
    const api = await import('/pkl_wasm.js');
    await api.default();
    const { runTraceCase } = await import('/trace-session.mjs');
    return fixtures.flatMap(fixture => [0, 1].map(repeat => ({ id: fixture.id, repeat, ...runTraceCase(api, fixture) })));
  }, fixtures);
  report.targets.browser = await compare('browser', rows);
} catch (error) { report.infrastructureErrors.push({ target: 'browser', message: error.message, stack: error.stack }); }
finally {
  if (browser) await browser.close();
  if (server.listening) await new Promise(ok => server.close(ok));
}
report.changedInputs = [];
for (const [path, expected] of Object.entries(hashes)) {
  if (sha(await readFile(path)) !== expected) report.changedInputs.push(relative(root, path));
}
report.compatibilityComplete = !report.infrastructureErrors.length && !report.browserErrors.length && !report.changedInputs.length
  && ['node', 'browser'].every(target => report.targets[target]?.complete && report.targets[target].results.every(row => row.status === 'pass'));
report.architecturePassed = ['node', 'browser'].every(target => report.targets[target]?.complete
  && report.targets[target].results.every(row => !row.acquisitionError && row.architecture.retainedEventBytesStable
    && (row.architecture.sessionCreated === true ? row.architecture.terminalSlots === 0
      : row.architecture.sessionCreated === false && row.architecture.setupFailed === true && row.architecture.terminalSlots === null)))
  && !report.infrastructureErrors.length && !report.browserErrors.length && !report.changedInputs.length;
await writeFile(resolve(output, 'execution.json'), json(report));
console.log(json({ compatibilityComplete: report.compatibilityComplete, architecturePassed: report.architecturePassed,
  targets: Object.fromEntries(Object.entries(report.targets).map(([key, value]) => [key, value.statusCounts])),
  infrastructureErrors: report.infrastructureErrors, browserErrors: report.browserErrors, changedInputs: report.changedInputs }));
if (!report.compatibilityComplete) process.exitCode = 1;
