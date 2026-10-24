// Raw observations only: comparison against independently pinned Java is separate.
// Main oracle cases each start a fresh JVM. Match that isolation with one Node
// process or new browser context/Wasm instance per case/repetition.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawnSync, execFileSync } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runDeprecationCase } from './deprecation-session.mjs';

const script = fileURLToPath(import.meta.url), directory = dirname(script);
const root = resolve(directory, '../..');
const sha = data => createHash('sha256').update(data).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';
const put = (path, value) => writeFile(path, json(value), { flag: 'wx' });

async function persist(out, row) {
  for (const [name, bytes] of Object.entries(row.payloads)) {
    if (!/^[a-z0-9.-]+\.bin$/.test(name) || !Array.isArray(bytes)
        || bytes.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255)) throw Error('invalid captured channel');
    await writeFile(resolve(out, name), Buffer.from(bytes), { flag: 'wx' });
  }
  await writeFile(resolve(out, 'protocol.bin'), Buffer.from(row.protocolBytes), { flag: 'wx' });
  await put(resolve(out, 'candidate-trace.json'), { trace: row.trace, architecture: row.architecture,
    acquisitionError: row.acquisitionError, protocol: row.protocol });
}

export function requirePairs(cases, results) {
  const expected = new Set(cases.flatMap(row => [0, 1].map(repeat => row.id + '\0' + repeat)));
  const seen = new Set();
  for (const row of results) {
    const key = row.id + '\0' + row.repeat;
    if (!expected.has(key) || seen.has(key)) throw Error('unexpected or duplicate capture pair');
    seen.add(key);
  }
  if (seen.size !== expected.size) throw Error('missing capture pair');
}

async function child(args) {
  if (args.length !== 3) throw Error('child INPUT OUTPUT ADDON');
  const [input, output, addon] = args.map(path => resolve(path));
  const fixture = JSON.parse(await readFile(input));
  const api = createRequire(resolve(directory, 'package.json'))(addon);
  const row = runDeprecationCase(api, fixture);
  await persist(output, row);
  await put(resolve(output, 'child.json'), { id: row.id, node: process.version, versions: process.versions,
    executable: process.execPath, argv: process.argv, cwd: process.cwd(),
    architecture: row.architecture, acquisitionError: row.acquisitionError });
  if (row.acquisitionError != null) process.exitCode = 1;
}

export async function capture(args) {
  if (args.length !== 4) throw Error('OUTPUT WASM_DIR NAPI_BINARY FIXTURE_DIR');
  const [out, wasm, addon, fixture] = args.map(path => resolve(path));
  const verification = execFileSync(process.env.PKL_FIXTURE_PYTHON ?? 'python3',
    [resolve(root, 'tools/deprecation_fixtures.py'), '--fixture', fixture], { cwd: root });
  const manifest = JSON.parse(await readFile(resolve(fixture, 'cases.json')));
  if (manifest.schema !== 1 || manifest.repetitions !== 2) throw Error('unknown main fixture profile');
  const tracked = {}, sets = {};
  const track = async path => { tracked[path] = sha(await readFile(path)); };
  async function tree(path) {
    const names = [];
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = resolve(path, entry.name);
      if (entry.isDirectory()) names.push(...await tree(child));
      else if (entry.isFile()) names.push(child);
      else throw Error('nonregular acquisition input');
    }
    return names.sort();
  }
  for (const path of [fixture, wasm]) {
    sets[path] = await tree(path);
    for (const file of sets[path]) await track(file);
  }
  for (const path of [script, addon, process.execPath, resolve(root, 'tools/deprecation_fixtures.py'),
    resolve(root, 'tools/m2_deprecation.py'), ...['deprecation-session.mjs', 'module-policy-session.mjs',
      'resource-session.mjs', 'package.json', 'package-lock.json'].map(name => resolve(directory, name))]) await track(path);
  const require = createRequire(resolve(directory, 'package.json'));
  const { chromium } = require('playwright');
  const packageJson = JSON.parse(await readFile(resolve(directory, 'package.json')));
  const lock = JSON.parse(await readFile(resolve(directory, 'package-lock.json')));
  const pin = packageJson.devDependencies.playwright;
  if (!/^\d+\.\d+\.\d+$/.test(pin) || require('playwright/package.json').version !== pin
      || require('playwright-core/package.json').version !== pin || lock.packages[''].devDependencies.playwright !== pin
      || lock.packages['node_modules/playwright'].version !== pin || lock.packages['node_modules/playwright-core'].version !== pin)
    throw Error('browser tooling pin mismatch');
  const cases = [];
  for (const row of manifest.cases) {
    if (!/^[a-z0-9-]+$/.test(row.id)) throw Error('invalid case identity');
    const bytes = await readFile(resolve(fixture, row.source));
    const source = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    if (sha(bytes) !== row.source_sha256 || !Buffer.from(source).equals(bytes)) throw Error('source identity differs');
    cases.push({ ...row, sourcePath: row.source, source });
  }
  await mkdir(out, { recursive: false });
  await writeFile(resolve(out, 'fixture-verification.stdout.bin'), verification, { flag: 'wx' });
  const environment = Object.fromEntries(['PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'TZ', 'TERM',
    'PLAYWRIGHT_BROWSERS_PATH', 'PKL_FIXTURE_PYTHON'].filter(name => process.env[name] != null).map(name => [name, process.env[name]]));
  const report = { schema: 1, scope: 'Main Java-backed sessions with fresh candidate process or Wasm instance per case/repetition. This report records acquisition; compatibility is evaluated separately.',
    argv: process.argv, cwd: process.cwd(), environment, fixtureManifestSha256: tracked[resolve(fixture, 'manifest.json')],
    caseIds: cases.map(row => row.id), repetitions: 2, node: { executable: process.execPath, version: process.version, versions: process.versions },
    playwright: pin, inputsBefore: tracked, fileSetsBefore: sets, targets: { napi: [], chromium: [] }, infrastructureErrors: [] };
  for (const row of cases) for (const repeat of [0, 1]) {
    const path = resolve(out, 'napi', row.id, String(repeat));
    await mkdir(path, { recursive: true });
    const input = resolve(path, 'input.json'); await put(input, row);
    const command = [script, '--child', input, path, addon];
    const processResult = spawnSync(process.execPath, command, { cwd: root, env: environment, encoding: null,
      timeout: 60_000, maxBuffer: 8 * 1024 * 1024 });
    for (const channel of ['stdout', 'stderr']) await writeFile(resolve(path, 'process.' + channel + '.bin'), processResult[channel] ?? Buffer.alloc(0), { flag: 'wx' });
    const observation = { id: row.id, repeat, directory: path, command: [process.execPath, ...command],
      exitCode: processResult.status, signal: processResult.signal, error: processResult.error?.message ?? null };
    await put(resolve(path, 'process.json'), observation);
    report.targets.napi.push(observation);
    console.log('napi', row.id, repeat, processResult.status);
  }
  const routes = new Map([
    ['/session.mjs', resolve(directory, 'deprecation-session.mjs')],
    ['/module-policy-session.mjs', resolve(directory, 'module-policy-session.mjs')],
    ['/resource-session.mjs', resolve(directory, 'resource-session.mjs')],
    ['/pkl_wasm.js', resolve(wasm, 'pkl_wasm.js')],
    ['/pkl_wasm_bg.wasm', resolve(wasm, 'pkl_wasm_bg.wasm')],
  ]);
  const server = createServer(async (request, response) => {
    try {
      const pathname = new URL(request.url, 'http://127.0.0.1').pathname;
      if (pathname === '/') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>Logger capture</title>'); return; }
      const path = routes.get(pathname); if (!path) throw Error('unknown artifact route');
      response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' });
      response.end(await readFile(path));
    } catch { response.writeHead(404); response.end(); }
  });
  let browser;
  try {
    await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
    browser = await chromium.launch({ headless: true });
    await track(chromium.executablePath());
    report.browser = { version: browser.version(), executable: chromium.executablePath(), sha256: tracked[chromium.executablePath()] };
    for (const row of cases) for (const repeat of [0, 1]) {
      const path = resolve(out, 'chromium', row.id, String(repeat)); await mkdir(path, { recursive: true });
      await put(resolve(path, 'input.json'), row);
      const context = await browser.newContext();
      const observation = { id: row.id, repeat, directory: path, isolation: 'new-browser-context-and-wasm-instance', browserErrors: [] };
      try {
        const page = await context.newPage();
        page.on('pageerror', error => observation.browserErrors.push({ message: error.message, stack: error.stack }));
        page.on('crash', () => observation.browserErrors.push({ message: 'page crashed' }));
        await page.goto(`http://127.0.0.1:${server.address().port}`);
        const result = await page.evaluate(async fixture => {
          const api = await import('/pkl_wasm.js'); await api.default();
          const { runDeprecationCase } = await import('/session.mjs');
          return runDeprecationCase(api, fixture);
        }, row);
        await persist(path, result);
        observation.acquisitionError = result.acquisitionError;
        observation.architecture = result.architecture;
      } catch (error) { observation.acquisitionError = { message: error.message, stack: error.stack }; }
      finally { await context.close(); }
      await put(resolve(path, 'process.json'), observation);
      report.targets.chromium.push(observation);
      console.log('chromium', row.id, repeat, observation.acquisitionError == null ? 'captured' : 'failed');
    }
  } catch (error) { report.infrastructureErrors.push({ message: error.message, stack: error.stack }); }
  finally {
    if (browser) await browser.close();
    if (server.listening) await new Promise(ok => server.close(ok));
  }
  report.inputsAfter = {};
  for (const path of Object.keys(tracked)) {
    try { report.inputsAfter[path] = sha(await readFile(path)); }
    catch { report.inputsAfter[path] = null; }
  }
  report.fileSetsAfter = {};
  for (const path of Object.keys(sets)) report.fileSetsAfter[path] = await tree(path);
  for (const rows of Object.values(report.targets)) {
    try { requirePairs(cases, rows); }
    catch (error) { report.infrastructureErrors.push({ message: error.message }); }
  }
  report.complete = report.infrastructureErrors.length === 0
    && JSON.stringify(report.inputsBefore) === JSON.stringify(report.inputsAfter)
    && JSON.stringify(report.fileSetsBefore) === JSON.stringify(report.fileSetsAfter)
    && report.targets.napi.every(row => row.exitCode === 0 && row.error == null && row.signal == null)
    && report.targets.chromium.every(row => row.acquisitionError == null && !row.browserErrors.length);
  await put(resolve(out, 'report.json'), report);
  if (!report.complete) process.exitCode = 1;
  return report;
}

if (process.argv[1] && resolve(process.argv[1]) === script) {
  if (process.argv[2] === '--child') await child(process.argv.slice(3));
  else await capture(process.argv.slice(2));
}
