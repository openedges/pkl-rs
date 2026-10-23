// Acquire typed module preparation and Resource sessions on actual Node and Chromium.
// Expected channels come exclusively from verified pinned-Java captures.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, readdir} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve, dirname, sep, extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {runModulePreparationCase, compareModulePreparationCase} from './module-preparation-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

const args = process.argv.slice(2);
if (args.length !== 5)
  throw Error('usage: compare-module-preparation.mjs OUTPUT WASM_DIR ADDON FIXTURES_DIR PINNED_PACKAGE_JSON');
const [output, wasm, addon, fixtureRoot, packagePath] = args.map(path => resolve(path));
const script = fileURLToPath(import.meta.url), helper = resolve(dirname(script), 'module-preparation-session.mjs');
const resourceHelper = resolve(dirname(script), 'resource-session.mjs');
const policyHelper = resolve(dirname(script), 'module-policy-session.mjs');
const verifier = resolve(dirname(script), '../module_portable_fixtures.py');
const json = value => JSON.stringify(value, null, 2) + '\n';
const hash = value => createHash('sha256').update(value).digest('hex');
const hashes = {};
async function read(path) {
  const data = await readFile(path), digest = hash(data);
  if (hashes[path] != null && hashes[path] !== digest) throw Error('input changed during acquisition: ' + path);
  hashes[path] = digest;
  return data;
}
async function tree(path) {
  for (const entry of await readdir(path, {withFileTypes: true})) {
    const child = resolve(path, entry.name);
    if (entry.isDirectory()) await tree(child);
    else if (entry.isFile()) await read(child);
    else throw Error('unexpected non-file artifact/fixture: ' + child);
  }
}
const unitBytes = units => {
  const bytes = Buffer.alloc(units.length * 2);
  units.forEach((unit, index) => bytes.writeUInt16BE(unit, index * 2));
  return bytes;
};
await mkdir(output, {recursive: false});
const startedAt = new Date().toISOString();
const command = ['python3', verifier, '--fixtures', fixtureRoot];
await read(verifier);
const checked = spawnSync(command[0], command.slice(1), {encoding: null});
await writeFile(resolve(output, 'preflight.stdout.bin'), checked.stdout ?? Buffer.alloc(0));
await writeFile(resolve(output, 'preflight.stderr.bin'), checked.stderr ?? Buffer.alloc(0));
const preflight = {command, exitCode: checked.status, signal: checked.signal, error: checked.error?.message ?? null};
await writeFile(resolve(output, 'preflight.process.json'), json(preflight));
if (checked.status !== 0) throw Error('Java module preparation fixture verification failed');
await tree(fixtureRoot);
const bundle = JSON.parse(await read(resolve(fixtureRoot, 'fixtures.json')));
const fixtures = bundle.fixtures;
if (bundle.schema !== 1 || !Array.isArray(fixtures) || !fixtures.length) throw Error('invalid module preparation fixture bundle');
const identities = fixtures.map(fixture => JSON.stringify([fixture.id, fixture.repeat, fixture.session]));
if (new Set(identities).size !== fixtures.length) throw Error('duplicate module preparation session identity');
const require = createRequire(packagePath), {chromium} = require('playwright');
const manifest = JSON.parse(await read(packagePath));
const lock = JSON.parse(await read(resolve(dirname(packagePath), 'package-lock.json')));
const pin = manifest.devDependencies.playwright;
if (!/^\d+\.\d+\.\d+$/.test(pin) || require('playwright/package.json').version !== pin
    || require('playwright-core/package.json').version !== pin
    || lock.packages[''].devDependencies.playwright !== pin
    || lock.packages['node_modules/playwright'].version !== pin
    || lock.packages['node_modules/playwright-core'].version !== pin)
  throw Error('unpinned browser tooling');
for (const path of [script, helper, resourceHelper, policyHelper, addon, process.execPath]) await read(path);
await tree(wasm);
await writeFile(resolve(output, 'fixtures.json'), json(bundle));
await writeFile(resolve(output, 'module-preparation-session.mjs'), await read(helper));
await writeFile(resolve(output, 'resource-session.mjs'), await read(resourceHelper));
await writeFile(resolve(output, 'module-policy-session.mjs'), await read(policyHelper));
await writeFile(resolve(output, 'compare-module-preparation.mjs'), await read(script));
const report = {schema: 1, startedAt, preflight, scope:
  'Typed module preparation sessions on actual Node/Chromium. Complete status/stdout/stderr UTF16 and Java UTF8, reader counts, observable policy-event projections and owned payloads. Unordered provider facts are not Java-observed SPI request order. No actual browser filesystem, CLI, full language or performance claim.',
  host: {platform: os.platform(), release: os.release(), architecture: os.arch(), hostname: os.hostname()},
  node: process.version, nodeExecutable: process.execPath, playwrightPin: pin,
  argv: process.argv, cwd: process.cwd(), selectedApiCallsPerTarget: fixtures.reduce((n, fixture) => n + fixture.expressions.length, 0),
  javaApiCalls: bundle.javaApiCalls, unexecutedApiBoundaries: bundle.unexecutedApiBoundaries,
  artifactSourceBinding: 'Supplied artifacts; source binding requires their separately recorded immutable build.',
  artifactAndInputHashes: hashes, targets: {}, infrastructureErrors: [], browserErrors: []};
async function save(target, rows) {
  await writeFile(resolve(output, target + '.raw.json'), json(rows));
  if (rows.length !== fixtures.length || rows.some((row, index) =>
    JSON.stringify([row.id, row.repeat, row.session]) !== identities[index]))
    throw Error('missing, extra or reordered module preparation sessions');
  const sessions = [];
  for (let index = 0; index < rows.length; index++) {
    const directory = resolve(output, target, String(index));
    await mkdir(directory, {recursive: true});
    await writeFile(resolve(directory, 'actual.json'), json(rows[index]));
    await writeFile(resolve(directory, 'expected.json'), json(fixtures[index].expected));
    for (const call of rows[index].calls) {
      for (const channel of ['stdout', 'stderr']) {
        await writeFile(resolve(directory, `${call.index}.${channel}.utf16be.bin`), unitBytes(call[channel + 'Units']));
        await writeFile(resolve(directory, `${call.index}.${channel}.java-utf8.bin`), Buffer.from(javaUtf8Bytes(call[channel + 'Units'])));
      }
    }
    const result = {id: rows[index].id, repeat: rows[index].repeat, session: rows[index].session,
      ...compareModulePreparationCase(fixtures[index], rows[index])};
    await writeFile(resolve(directory, 'comparison.json'), json(result));
    sessions.push(result);
  }
  const rawApiCounts = {};
  for (const session of sessions) for (const call of session.comparisons)
    rawApiCounts[call.status] = (rawApiCounts[call.status] ?? 0) + 1;
  return {complete: true, sessions, rawApiCounts, selectedContractPassed: sessions.every(session => session.passed)};
}
try {
  const api = require(addon);
  report.targets.node = await save('node', fixtures.map(fixture => runModulePreparationCase(api, fixture)));
} catch (error) { report.infrastructureErrors.push({target: 'node', message: error.message, stack: error.stack}); }
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://127.0.0.1');
    if (url.pathname === '/') { response.end('<!doctype html><title>Module preparation verification</title>'); return; }
    const isHelper = ['/resource-session.mjs', '/module-preparation-session.mjs', '/module-policy-session.mjs'].includes(url.pathname);
    const path = isHelper ? resolve(output, url.pathname.slice(1)) : resolve(wasm, '.' + decodeURIComponent(url.pathname));
    if (!isHelper && !path.startsWith(wasm + sep)) throw Error('invalid artifact path');
    const assetBytes = await read(path); response.writeHead(200, {'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript'});
    response.end(assetBytes);
  } catch { if (!response.headersSent) { response.writeHead(404); response.end(); } else response.destroy(); }
});
await new Promise(done => server.listen(0, '127.0.0.1', done));
let browser;
try {
  const missingAssetPath = '/__pkl_verification_missing_asset__.js';
  const missingAssetResponse = await fetch(`http://127.0.0.1:${server.address().port}${missingAssetPath}`);
  const missingAssetBody = Buffer.from(await missingAssetResponse.arrayBuffer());
  report.serverMissingAssetProbe = {path: missingAssetPath, status: missingAssetResponse.status,
    bodyHex: missingAssetBody.toString('hex')};
  if (missingAssetResponse.status !== 404 || missingAssetBody.length !== 0)
    throw Error('missing artifact must return an empty 404 response');
  report.browserExecutable = chromium.executablePath();
  report.browserExecutableSha256 = hash(await read(report.browserExecutable));
  report.browserLaunchOptions = {headless: true, executablePath: report.browserExecutable};
  browser = await chromium.launch(report.browserLaunchOptions);
  report.browser = browser.version();
  const page = await browser.newPage();
  page.on('pageerror', error => report.browserErrors.push({message: error.message, stack: error.stack}));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const rows = await page.evaluate(async fixtures => {
    const api = await import('/pkl_wasm.js'); await api.default();
    const {runModulePreparationCase} = await import('/module-preparation-session.mjs');
    return fixtures.map(fixture => runModulePreparationCase(api, fixture));
  }, fixtures);
  report.targets.browser = await save('browser', rows);
} catch (error) { report.infrastructureErrors.push({target: 'browser', message: error.message, stack: error.stack}); }
finally { if (browser) await browser.close(); await new Promise(done => server.close(done)); }
report.changedInputs = [];
for (const [path, digest] of Object.entries(hashes))
  if (hash(await readFile(path)) !== digest) report.changedInputs.push(path);
report.selectedContractPassed = ['node', 'browser'].every(target => report.targets[target]?.selectedContractPassed)
  && !report.infrastructureErrors.length && !report.browserErrors.length && !report.changedInputs.length;
report.fullCompatibilityPassed = false;
report.remainingScope = 'Unexecuted API boundaries, whole original programs, other Host capabilities, full platform and M2 completion gates remain separate obligations.';
report.finishedAt = new Date().toISOString();
await writeFile(resolve(output, 'execution.json'), json(report));
console.log(json({targets: Object.fromEntries(Object.entries(report.targets).map(([target, result]) =>
  [target, {rawApiCounts: result.rawApiCounts, selectedContractPassed: result.selectedContractPassed}])),
  selectedContractPassed: report.selectedContractPassed, fullCompatibilityPassed: report.fullCompatibilityPassed}));
process.exitCode = report.selectedContractPassed ? 0 : 1;
