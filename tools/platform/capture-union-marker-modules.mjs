// Separate-module parser diagnostic capture. Each Java module/repetition has
// its own N-API child or Chromium context/Wasm instance and SourceSession.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawnSync, execFileSync } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir, realpath } from 'node:fs/promises';
import { createReadStream } from 'node:fs';
import { resolve, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runStringSession } from './data-size-session.mjs';
import { javaUtf8Bytes } from './resource-session.mjs';

const SCRIPT = fileURLToPath(import.meta.url), DIR = dirname(SCRIPT), ROOT = resolve(DIR, '../..');
const URI = 'file:///stage66/union-marker-diagnostics.pkl';
const PROGRAMS_SHA256 = '9aff6bb7a1704df308ebd0f0639683bcbdfacaab4439f6392a54ac7aa00b7b71';
const sha = raw => createHash('sha256').update(raw).digest('hex');
const shaFile = async path => { const h = createHash('sha256'); for await (const bytes of createReadStream(path)) h.update(bytes); return h.digest('hex'); };
const json = value => JSON.stringify(value, null, 2) + '\n';
const put = (path, value) => writeFile(path, json(value), { flag: 'wx' });
const errorRecord = error => ({ name: error?.name ?? null, message: String(error?.message ?? error), stack: error?.stack ?? null });
const units = value => Array.from({ length: value.length }, (_, i) => value.charCodeAt(i));
const unitBytes = values => values.flatMap(unit => [unit >> 8, unit & 255]);
const COMMON = ['expression.bin', 'expression.code-units.bin', 'result.status.bin',
  'result.stdout.bin', 'result.stdout.code-units.bin', 'result.stderr.bin', 'result.stderr.code-units.bin'];
const META = ['candidate.disposition.bin', 'candidate.phase.bin', 'candidate.reason.bin', 'candidate.callback-origin.bin'];
const API_META = ['candidate.api-error-name.bin', 'candidate.callback-origin.bin'];
export const CAPTURE_TRACKED_SOURCES = Object.freeze([
  'tools/platform/capture-union-marker-modules.mjs', 'tools/platform/capture-union-marker-modules.test.mjs',
  'tools/platform/data-size-session.mjs', 'tools/platform/data-size-session.test.mjs',
  'tools/platform/module-policy-session.mjs', 'tools/platform/resource-session.mjs',
  'tools/platform/package.json', 'tools/platform/package-lock.json',
  'tools/union_marker_oracle.py', 'tools/union_marker_fixtures.py',
  'tools/tests/test_union_marker_fixtures.py', 'tools/union_marker_portable_compare.py',
  'tools/tests/test_union_marker_portable_compare.py',
  'tools/amendment_construction_oracle.py', 'tools/collection_conversions_fixtures.py',
  'tools/collection_conversions_oracle.py', 'tools/collection_conversions_portable_compare.py',
  'tools/collection_conversions_compare.py', 'tools/data_size_portable_compare.py',
  'tools/data_size_compare.py', 'tools/data_size_fixtures.py', 'tools/data_size_oracle.py',
  'tools/java/CollectionConversionsOracle.java', 'tools/m1_platforms.py',
  'tools/m1_semantics.py', 'tools/m2_source_inputs.py',
]);
const decode = raw => { const text = new TextDecoder('utf-8', { fatal: true }).decode(raw);
  if (!Buffer.from(text, 'utf8').equals(raw)) throw Error('authored UTF8 bytes differ'); return text; };
async function filesBelow(path) {
  const result = [];
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, entry.name);
    if (entry.isDirectory()) result.push(...await filesBelow(child));
    else if (entry.isFile()) result.push(child);
    else throw Error('nonregular acquisition input');
  }
  return result.sort();
}
export async function loadModules(fixture) {
  const manifestRaw = await readFile(resolve(fixture, 'programs.json'));
  if (sha(manifestRaw) !== PROGRAMS_SHA256) throw Error('authored module manifest differs');
  const manifest = JSON.parse(decode(manifestRaw));
  if (manifest.schema !== 1 || manifest.baseline !== 'Pkl 0.32.0' || manifest.module_uri !== URI
      || manifest.repetitions !== 2 || !Array.isArray(manifest.programs) || manifest.programs.length !== 40)
    throw Error('unknown separate-module fixture');
  const seen = new Set(), modules = [];
  for (const [index, program] of manifest.programs.entries()) {
    if (typeof program.id !== 'string' || !/^[a-z0-9-]+$/.test(program.id) || seen.has(program.id)
        || program.module_file !== `modules/${String(index).padStart(2, '0')}-${program.id}.pkl`
        || program.query !== 'result' || !/^[a-f0-9]{64}$/.test(program.module_sha256)
        || !Number.isSafeInteger(program.module_bytes)) throw Error('invalid authored module identity');
    seen.add(program.id);
    const raw = await readFile(resolve(fixture, program.module_file));
    if (raw.length !== program.module_bytes || sha(raw) !== program.module_sha256) throw Error('authored module bytes differ');
    modules.push({ index, id: program.id, moduleFile: program.module_file,
      moduleSha256: program.module_sha256, uri: URI, source: decode(raw), rows: [{ id: program.id, expression: 'result' }] });
  }
  const actualModules = await filesBelow(resolve(fixture, 'modules'));
  const expectedModules = modules.map(module => resolve(fixture, module.moduleFile)).sort();
  if (JSON.stringify(actualModules) !== JSON.stringify(expectedModules)) throw Error('authored module file set differs');
  return modules;
}
export function deriveJavaExecution(provenance) {
  const environment = provenance?.environment, sessions = provenance?.sessions;
  if (!environment || typeof environment !== 'object' || Array.isArray(environment)
      || !Array.isArray(sessions) || sessions.length !== 80
      || sessions.some(row => !Array.isArray(row.command) || row.command.some(value => typeof value !== 'string')))
    throw Error('invalid Java execution provenance');
  const read = (command, name) => {
    const values = command.filter(value => value.startsWith(`-Duser.${name}=`));
    if (values.length !== 1 || values[0].length <= `-Duser.${name}=`.length) throw Error('Java override differs');
    return values[0].slice(`-Duser.${name}=`.length);
  };
  const overrides = sessions.map(({ command }) => Object.fromEntries(['language', 'country', 'timezone']
    .map(name => [name, read(command, name)])));
  if (overrides.some(value => JSON.stringify(value) !== JSON.stringify(overrides[0]))
      || !/^[a-z]{2,3}$/.test(overrides[0].language) || !/^[A-Z]{2}$/.test(overrides[0].country)
      || environment.TZ !== overrides[0].timezone || Object.values(environment).some(value => typeof value !== 'string'))
    throw Error('Java evaluation environment differs');
  const properties = provenance.jdk?.properties;
  if (typeof properties?.['user.language'] !== 'string'
      || typeof properties?.['user.country'] !== 'string') throw Error('Java preflight identity differs');
  return { environment, locale: `${overrides[0].language}-${overrides[0].country}`,
    timezoneId: overrides[0].timezone, sessionOverrides: overrides[0],
    preflightLocale: `${properties['user.language']}-${properties['user.country']}` };
}
async function persist(path, result) {
  if (result.rows.length !== 1) throw Error('one public query per module required');
  const row = result.rows[0], folder = resolve(path, `000-${row.id}`); await mkdir(folder);
  for (const [name, bytes] of Object.entries(row.payloads)) {
    if (!/^[a-z0-9.-]+\.bin$/.test(name) || !Array.isArray(bytes)
        || bytes.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255))
      throw Error('invalid candidate payload');
    await writeFile(resolve(folder, name), Buffer.from(bytes), { flag: 'wx' });
  }
  await put(resolve(path, 'candidate-trace.json'), { trace: result.trace, architecture: result.architecture,
    acquisitionError: result.acquisitionError, boundary: result.boundary });
}
export async function verifySessionFiles(path, module, target) {
  if (!['napi', 'chromium'].includes(target)) throw Error('unknown target');
  const folder = resolve(path, `000-${module.id}`), status = (await readFile(resolve(folder, 'result.status.bin'))).toString('ascii');
  if (!['success\n', 'error\n', 'candidate-error\n', 'api-exception\n'].includes(status)) throw Error('invalid status');
  const expected = new Set(['input.json', 'process.stdout.bin', 'process.stderr.bin', 'process.json',
    'candidate-trace.json', ...(target === 'napi' ? ['child.json'] : []),
    ...[...COMMON, ...(status === 'api-exception\n' ? API_META : META)].map(name => `000-${module.id}/${name}`)]);
  const actual = new Set((await filesBelow(path)).map(name => name.slice(path.length + 1).replaceAll('\\', '/')));
  if (actual.size !== expected.size || [...actual].some(name => !expected.has(name))) throw Error('session file set differs');
  const expression = units('result');
  if (!(await readFile(resolve(folder, 'expression.bin'))).equals(Buffer.from(javaUtf8Bytes(expression)))
      || !(await readFile(resolve(folder, 'expression.code-units.bin'))).equals(Buffer.from(unitBytes(expression))))
    throw Error('query expression differs');
  for (const channel of ['stdout', 'stderr']) {
    const raw = await readFile(resolve(folder, `result.${channel}.bin`));
    const be = await readFile(resolve(folder, `result.${channel}.code-units.bin`));
    if (be.length % 2) throw Error('odd UTF16 channel');
    const values = Array.from({ length: be.length / 2 }, (_, i) => (be[i * 2] << 8) | be[i * 2 + 1]);
    if (!raw.equals(Buffer.from(javaUtf8Bytes(values)))) throw Error('Java UTF8 channel differs');
    if ((status === 'success\n' && channel === 'stderr' || status !== 'success\n' && channel === 'stdout') && raw.length)
      throw Error('inactive result channel nonempty');
  }
  const success = Number(status === 'success\n');
  const protocol = `session-open mode=string rows=1\ncall-enter=string-0-${module.id}\n`
    + `call-exit=string-0-${module.id} status=${status.trimEnd()}\n`
    + `session-closed mode=string rows=1 public-calls=1 success=${success} errors=${1 - success}\n`;
  if (!(await readFile(resolve(path, 'process.stdout.bin'))).equals(Buffer.from(protocol))
      || (await readFile(resolve(path, 'process.stderr.bin'))).length)
    throw Error('whole-session protocol differs');
  return { status: status.trimEnd(), fileCount: actual.size,
    stdoutSha256: sha(await readFile(resolve(path, 'process.stdout.bin'))),
    stderrSha256: sha(await readFile(resolve(path, 'process.stderr.bin'))) };
}
async function child(args) {
  if (args.length !== 3) throw Error('child INPUT OUTPUT ADDON');
  const [input, output, addon] = args.map(value => resolve(value));
  const module = JSON.parse(await readFile(input));
  const api = createRequire(resolve(DIR, 'package.json'))(addon);
  const result = runStringSession(api, module);
  await persist(output, result);
  await put(resolve(output, 'child.json'), { pid: process.pid, executable: process.execPath,
    node: process.version, versions: process.versions, argv: process.argv,
    cwd: process.cwd(), environment: process.env, architecture: result.architecture,
    acquisitionError: result.acquisitionError });
  process.stdout.write(Buffer.from(result.protocolBytes));
  if (result.acquisitionError || result.architecture.terminalSlots !== 0
      || result.architecture.terminalDepth !== 0) process.exitCode = 2;
}
export async function capture(args) {
  if (args.length !== 4) throw Error('OUTPUT WASM_DIR NAPI_BINARY FIXTURE_DIR');
  const [out, wasm, addon, fixture] = args.map(value => resolve(value));
  await mkdir(out, { recursive: false });
  const tracked = {}, sets = {}, problems = [], track = async path => { tracked[path] = await shaFile(path); };
  const report = { schema: 1, scope: 'Forty separate authored modules, two fresh SourceSessions per module on each portable target; Java-only exception metadata remains separate.',
    argv: process.argv, cwd: process.cwd(), fixture, repetitions: 2, moduleUri: URI,
    environment: Object.fromEntries(['PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'PLAYWRIGHT_BROWSERS_PATH', 'PKL_FIXTURE_PYTHON']
      .filter(name => process.env[name] != null).map(name => [name, process.env[name]])),
    node: { executable: process.execPath, version: process.version, versions: process.versions, platform: process.platform, arch: process.arch },
    inputsBefore: tracked, fileSetsBefore: sets, targets: { napi: [], chromium: [] },
    infrastructureErrors: problems, httpRequests: [], fullPublicApiComparisonPassed: false,
    artifactSourceBinding: 'Supplied addon and Wasm source identity requires independent M1/native build records.',
    performanceClaim: false, complete: false };
  let modules;
  try {
    modules = await loadModules(fixture);
    for (const path of [fixture, wasm]) { sets[path] = await filesBelow(path); for (const name of sets[path]) await track(name); }
    for (const path of [addon, process.execPath, ...CAPTURE_TRACKED_SOURCES.map(name => resolve(ROOT, name))]) await track(path);
    report.node.executableRealpath = await realpath(process.execPath);
    report.node.executableSha256 = tracked[process.execPath];
    report.artifacts = { napi: { path: addon, realpath: await realpath(addon), sha256: tracked[addon] },
      wasm: { directory: wasm, files: sets[wasm].map(path => ({ path, sha256: tracked[path] })) } };
    const python = process.env.PKL_FIXTURE_PYTHON ?? 'python3';
    const py = JSON.parse(execFileSync(python, ['-c', 'import json,sys;print(json.dumps({"executable":sys.executable,"version":sys.version}))'], { cwd: ROOT }).toString());
    await track(py.executable);
    report.python = { ...py, executableRealpath: await realpath(py.executable), executableSha256: tracked[py.executable] };
    const verification = spawnSync(python, [resolve(ROOT, 'tools/union_marker_fixtures.py'), '--fixture', fixture],
      { cwd: ROOT, encoding: null, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
    await writeFile(resolve(out, 'fixture-verification.stdout.bin'), verification.stdout ?? Buffer.alloc(0), { flag: 'wx' });
    await writeFile(resolve(out, 'fixture-verification.stderr.bin'), verification.stderr ?? Buffer.alloc(0), { flag: 'wx' });
    report.fixtureVerification = { command: [python, resolve(ROOT, 'tools/union_marker_fixtures.py'), '--fixture', fixture],
      exitCode: verification.status, signal: verification.signal, error: verification.error?.message ?? null,
      stdoutSha256: sha(verification.stdout ?? Buffer.alloc(0)), stderrSha256: sha(verification.stderr ?? Buffer.alloc(0)) };
    if (verification.status !== 0 || verification.signal || verification.error || verification.stderr?.length)
      throw Error('canonical fixture verification failed');
    const extractionScript = "import json,sys,tarfile; from pathlib import Path; p=Path(sys.argv[1]); a=tarfile.open(p/'oracle-evidence.tar.gz','r:gz'); d=json.load(a.extractfile(a.getmember('capture-01/provenance.json'))); print(json.dumps({'environment':d['environment'],'sessions':d['sessions'],'jdk':d['jdk']},sort_keys=True))";
    const extraction = spawnSync(py.executable, ['-c', extractionScript, fixture],
      { cwd: ROOT, encoding: null, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
    await writeFile(resolve(out, 'java-provenance.stdout.bin'), extraction.stdout ?? Buffer.alloc(0), { flag: 'wx' });
    await writeFile(resolve(out, 'java-provenance.stderr.bin'), extraction.stderr ?? Buffer.alloc(0), { flag: 'wx' });
    report.javaProvenanceExtraction = { command: [py.executable, '-c', extractionScript, fixture], cwd: ROOT,
      exitCode: extraction.status, signal: extraction.signal, error: extraction.error?.message ?? null,
      stdoutSha256: sha(extraction.stdout ?? Buffer.alloc(0)), stderrSha256: sha(extraction.stderr ?? Buffer.alloc(0)) };
    if (extraction.status !== 0 || extraction.signal || extraction.error || extraction.stderr?.length) throw Error('Java provenance extraction failed');
    report.javaExecution = deriveJavaExecution(JSON.parse(extraction.stdout.toString('utf8')));
    report.modules = modules.map(({ index, id, moduleFile, moduleSha256 }) => ({ index, id, moduleFile, moduleSha256 }));
    const require = createRequire(resolve(DIR, 'package.json'));
    const { chromium } = require('playwright');
    const packageJson = JSON.parse(await readFile(resolve(DIR, 'package.json')));
    const lock = JSON.parse(await readFile(resolve(DIR, 'package-lock.json')));
    const pin = packageJson.devDependencies.playwright;
    if (!/^\d+\.\d+\.\d+$/.test(pin) || require('playwright/package.json').version !== pin
        || require('playwright-core/package.json').version !== pin || lock.packages[''].devDependencies.playwright !== pin
        || lock.packages['node_modules/playwright'].version !== pin || lock.packages['node_modules/playwright-core'].version !== pin)
      throw Error('browser tooling pin mismatch');
    await track(require.resolve('playwright/package.json')); await track(require.resolve('playwright-core/package.json'));
    await track(chromium.executablePath());
    report.playwright = { pin, package: require.resolve('playwright/package.json'), corePackage: require.resolve('playwright-core/package.json'),
      browserExecutable: chromium.executablePath(), browserExecutableRealpath: await realpath(chromium.executablePath()),
      browserExecutableSha256: tracked[chromium.executablePath()] };
    for (const module of modules) for (const repeat of [0, 1]) {
      const path = resolve(out, 'napi', 'programs', `${String(module.index).padStart(2, '0')}-${module.id}`, `repeat-${repeat}`);
      await mkdir(path, { recursive: true }); await put(resolve(path, 'input.json'), module);
      const command = [process.execPath, SCRIPT, '--child', resolve(path, 'input.json'), path, addon];
      const result = spawnSync(command[0], command.slice(1), { cwd: ROOT, env: report.javaExecution.environment,
        encoding: null, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
      for (const channel of ['stdout', 'stderr']) await writeFile(resolve(path, `process.${channel}.bin`), result[channel] ?? Buffer.alloc(0), { flag: 'wx' });
      const observation = { index: module.index, id: module.id, repeat, uri: URI, directory: path, command,
        isolation: 'new-node-child-Wasm-uninvolved-and-new-SourceSession', exitCode: result.status,
        signal: result.signal, error: result.error?.message ?? null,
        processStdoutSha256: sha(result.stdout ?? Buffer.alloc(0)), processStderrSha256: sha(result.stderr ?? Buffer.alloc(0)) };
      await put(resolve(path, 'process.json'), observation); report.targets.napi.push(observation);
      if (result.status === 0 && !result.signal && !result.error) try { observation.fileSet = await verifySessionFiles(path, module, 'napi'); }
      catch (error) { problems.push({ target: 'napi', index: module.index, repeat, message: error.message }); }
    }
    const routes = new Map([['/session.mjs', resolve(DIR, 'data-size-session.mjs')],
      ['/module-policy-session.mjs', resolve(DIR, 'module-policy-session.mjs')],
      ['/resource-session.mjs', resolve(DIR, 'resource-session.mjs')],
      ['/pkl_wasm.js', resolve(wasm, 'pkl_wasm.js')], ['/pkl_wasm_bg.wasm', resolve(wasm, 'pkl_wasm_bg.wasm')]]);
    let activeSession = null;
    const server = createServer(async (request, response) => {
      let pathname = null;
      try {
        pathname = new URL(request.url, 'http://127.0.0.1').pathname;
        if (pathname === '/') { report.httpRequests.push({ session: activeSession, path: pathname, status: 200 });
          response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>union marker capture</title>'); return; }
        if (pathname === '/favicon.ico') { report.httpRequests.push({ session: activeSession, path: pathname, status: 204 }); response.writeHead(204); response.end(); return; }
        const file = routes.get(pathname); if (!file) throw Error('unknown artifact route');
        const body = await readFile(file); report.httpRequests.push({ session: activeSession, path: pathname, status: 200 });
        response.writeHead(200, { 'Content-Type': extname(file) === '.wasm' ? 'application/wasm' : 'text/javascript' }); response.end(body);
      } catch { report.httpRequests.push({ session: activeSession, path: pathname ?? request.url, status: 404 }); response.writeHead(404); response.end(); }
    });
    let browser;
    try {
      await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
      const launchOptions = { headless: true, executablePath: chromium.executablePath(), env: report.javaExecution.environment };
      const contextOptions = { locale: report.javaExecution.locale, timezoneId: report.javaExecution.timezoneId };
      report.playwright.launchOptions = launchOptions; report.playwright.contextOptions = contextOptions;
      browser = await chromium.launch(launchOptions); report.playwright.browserVersion = browser.version();
      for (const module of modules) for (const repeat of [0, 1]) {
        const path = resolve(out, 'chromium', 'programs', `${String(module.index).padStart(2, '0')}-${module.id}`, `repeat-${repeat}`);
        await mkdir(path, { recursive: true }); await put(resolve(path, 'input.json'), module);
        const context = await browser.newContext(contextOptions);
        const session = `${module.index}:${module.id}:${repeat}`; activeSession = session;
        const observation = { index: module.index, id: module.id, repeat, uri: URI, directory: path, session,
          isolation: 'new-browser-context-Wasm-instance-and-SourceSession', browserErrors: [], acquisitionError: null };
        try {
          const page = await context.newPage();
          page.on('pageerror', error => observation.browserErrors.push(errorRecord(error)));
          page.on('crash', () => observation.browserErrors.push({ message: 'page crashed' }));
          page.on('console', message => { if (message.type() === 'error') observation.browserErrors.push({ message: message.text() }); });
          await page.goto(`http://127.0.0.1:${server.address().port}`);
          observation.contextObserved = await page.evaluate(() => ({ locale: navigator.language,
            timezoneId: Intl.DateTimeFormat().resolvedOptions().timeZone }));
          if (observation.contextObserved.locale !== contextOptions.locale
              || observation.contextObserved.timezoneId !== contextOptions.timezoneId) throw Error('Chromium context identity differs');
          const result = await page.evaluate(async input => { const api = await import('/pkl_wasm.js'); await api.default();
            const { runStringSession } = await import('/session.mjs'); return runStringSession(api, input); }, module);
          await persist(path, result);
          await writeFile(resolve(path, 'process.stdout.bin'), Buffer.from(result.protocolBytes), { flag: 'wx' });
          observation.protocolOrigin = 'browser-page-returned-UTF8';
          observation.acquisitionError = result.acquisitionError; observation.architecture = result.architecture;
        } catch (error) { observation.acquisitionError = errorRecord(error); }
        finally { await context.close(); activeSession = null; }
        await writeFile(resolve(path, 'process.stderr.bin'), Buffer.from(observation.browserErrors.map(json).join('')), { flag: 'wx' });
        observation.stderrOrigin = 'browser-pageerror-and-console-error-capture';
        await put(resolve(path, 'process.json'), observation); report.targets.chromium.push(observation);
        if (!observation.acquisitionError && !observation.browserErrors.length) try {
          observation.fileSet = await verifySessionFiles(path, module, 'chromium');
        } catch (error) { problems.push({ target: 'chromium', index: module.index, repeat, message: error.message }); }
      }
    } finally { if (browser) await browser.close(); if (server.listening) await new Promise(ok => server.close(ok)); }
  } catch (error) { problems.push(errorRecord(error)); }
  report.inputsAfter = {};
  for (const path of Object.keys(tracked)) try { report.inputsAfter[path] = await shaFile(path); }
  catch { report.inputsAfter[path] = null; }
  report.fileSetsAfter = {};
  for (const path of Object.keys(sets)) try { report.fileSetsAfter[path] = await filesBelow(path); }
  catch { report.fileSetsAfter[path] = null; }
  report.complete = problems.length === 0 && JSON.stringify(report.inputsBefore) === JSON.stringify(report.inputsAfter)
    && JSON.stringify(report.fileSetsBefore) === JSON.stringify(report.fileSetsAfter)
    && report.targets.napi.length === 80 && report.targets.chromium.length === 80
    && report.targets.napi.every(row => row.exitCode === 0 && !row.signal && !row.error && row.fileSet)
    && report.targets.chromium.every(row => !row.acquisitionError && !row.browserErrors.length && row.fileSet
      && row.architecture?.terminalSlots === 0 && row.architecture?.terminalDepth === 0
      && row.architecture?.retainedPayloadsStable === true);
  await put(resolve(out, 'report.json'), report);
  if (!report.complete) process.exitCode = 1;
  return report;
}
if (process.argv[1] && resolve(process.argv[1]) === SCRIPT) {
  if (process.argv[2] === '--child') await child(process.argv.slice(3));
  else await capture(process.argv.slice(2));
}
