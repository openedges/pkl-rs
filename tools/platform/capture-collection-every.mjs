// Logged every() capture, adapted from the pinned collection capture.
// Separate profile: no changes to the fifteen nonlogging collection captures.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { spawnSync, execFileSync } from 'node:child_process';
import { readFile, writeFile, mkdir, readdir, realpath } from 'node:fs/promises';
import { resolve, dirname, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { createReadStream } from 'node:fs';
import { javaUtf8Bytes } from './resource-session.mjs';
import { deriveJavaExecution, CAPTURE_TRACKED_SOURCES as SHARED_SOURCES } from './capture-collection-conversions.mjs';
import { runEveryLoggerSession } from './every-logger-session.mjs';

const script = fileURLToPath(import.meta.url), directory = dirname(script), root = resolve(directory, '../..');
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const shaFile = async path => { const hash = createHash('sha256');
  for await (const chunk of createReadStream(path)) hash.update(chunk);
  return hash.digest('hex'); };
const json = value => JSON.stringify(value, null, 2) + '\n';
const put = (path, value) => writeFile(path, json(value), { flag: 'wx' });
const textUnits = value => Array.from({ length: value.length }, (_, index) => value.charCodeAt(index));
const unitsBytes = value => value.flatMap(unit => [unit >> 8, unit & 255]);
const FIXTURE = 'tests/fixtures/m2-collection-every-oracle';
const VERIFIER = 'tools/collection_predicate_fixtures.py';
const MODULE_URI = 'file:///stage68/collection-every.pkl';
const INPUT_SHA = Object.freeze({
  'cases.json': 'bec8793e348b2c17a4d4d75fbe236eb61ade9a13b4d9b18c825f2db1056396c1',
  'module.pkl': 'b8bbc394b568efe18232055ecd0e77876ca278a697316b04d47d33633a9e1756',
  'queries.tsv': '69dbce55e4fc0f8780d48575799a01614b50ecc88a1493a5fa9c1b23aa30289a',
});
export const CAPTURE_TRACKED_SOURCES = Object.freeze([...new Set([...SHARED_SOURCES,
  'tools/platform/capture-collection-every.mjs',
  'tools/platform/capture-collection-every.test.mjs',
  'tools/platform/every-logger-session.mjs',
  'tools/platform/every-logger-session.test.mjs',
  'tools/collection_every_compare.py',
  'tools/tests/test_collection_every_compare.py',
  'tools/collection_every_fixtures.py',
  'tools/tests/test_collection_predicate_fixtures.py', VERIFIER,
])]);
const common = ['expression.bin', 'expression.code-units.bin', 'result.status.bin',
  'result.stdout.bin', 'result.stdout.code-units.bin', 'result.stderr.bin', 'result.stderr.code-units.bin'];
const terminalMeta = ['candidate.disposition.bin', 'candidate.phase.bin', 'candidate.reason.bin',
  'candidate.callback-origin.bin'];
const apiMeta = ['candidate.api-error-name.bin', 'candidate.callback-origin.bin'];
const logFields = ['level', 'message', 'module-uri', 'member', 'position'];
const javaProvenanceScript = "import json,sys,tarfile; from pathlib import Path; archive=tarfile.open(Path(sys.argv[1])/'oracle-evidence.tar.gz','r:gz'); member=archive.getmember('capture-03/report.json'); assert member.isfile(); data=json.load(archive.extractfile(member)); print(json.dumps({'environment':data['environment'],'sessions':[row['argv'] for row in data['sessions']],'jdk_properties':data['jdk']['properties']},sort_keys=True))";
function decodeExact(bytes, name) {
  if (bytes.subarray(0, 3).equals(Buffer.from([0xef, 0xbb, 0xbf]))) throw Error('UTF8 BOM in ' + name);
  const value = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  if (!Buffer.from(value, 'utf8').equals(bytes)) throw Error('UTF8 round trip differs: ' + name);
  return value;
}
export async function loadEveryFixture(fixture) {
  const path = resolve(fixture);
  const names = ['cases.json', 'module.pkl', 'queries.tsv'];
  const raw = Object.fromEntries(await Promise.all(names.map(async name => [name, await readFile(resolve(path, name))])));
  for (const name of names) if (sha(raw[name]) !== INPUT_SHA[name]) throw Error('canonical every input differs: ' + name);
  const manifest = JSON.parse(decodeExact(raw['cases.json'], 'cases.json'));
  if (manifest.schema !== 1 || manifest.baseline !== 'Pkl 0.32.0' || manifest.uri !== MODULE_URI
      || manifest.repetitions !== 2 || !Array.isArray(manifest.rows) || manifest.rows.length !== 64)
    throw Error('every fixture identity differs');
  const ids = new Set();
  for (const row of manifest.rows) {
    if (typeof row?.id !== 'string' || !/^[a-z0-9-]+$/.test(row.id) || ids.has(row.id)
        || typeof row.expression !== 'string' || !row.expression || /[\r\n\t]/.test(row.expression))
      throw Error('invalid or duplicate every row');
    ids.add(row.id);
  }
  const queries = decodeExact(raw['queries.tsv'], 'queries.tsv');
  if (queries !== manifest.rows.map(row => `string\t${row.id}\t${row.expression}\n`).join(''))
    throw Error('ordered every query bytes differ');
  return { uri: MODULE_URI, source: decodeExact(raw['module.pkl'], 'module.pkl'),
    rows: manifest.rows.map(row => ({ id: row.id, expression: row.expression })),
    inputHashes: Object.fromEntries(names.map(name => [name, sha(raw[name])])) };
}
export function requireSessionCoverage(fixture, observations) {
  if (fixture?.uri !== MODULE_URI || fixture?.rows?.length !== 64 || !Array.isArray(observations)
      || observations.length !== 2 || observations.some((row, index) => row.repeat !== index
        || row.rowCount !== 64 || row.uri !== MODULE_URI)) throw Error('every session coverage differs');
}
async function filesBelow(path) {
  const names = [];
  for (const entry of await readdir(path, { withFileTypes: true })) {
    const child = resolve(path, entry.name);
    if (entry.isDirectory()) names.push(...await filesBelow(child));
    else if (entry.isFile()) names.push(child);
    else throw Error('nonregular acquisition input');
  }
  return names.sort();
}
function loggedFiles(total) {
  if (!Number.isInteger(total) || total < 0 || total > 999) throw Error('invalid logger count');
  return ['logs.count.bin', ...Array.from({ length: total }, (_, index) =>
    logFields.flatMap(field => [`log-${String(index).padStart(3, '0')}.${field}.bin`,
      `log-${String(index).padStart(3, '0')}.${field}.code-units.bin`])).flat()];
}
export async function validateSessionFileSet(path, fixture, target) {
  if (!['napi', 'chromium'].includes(target)) throw Error('unknown target');
  const rootFiles = ['input.json', 'process.stdout.bin', 'process.stderr.bin',
    'candidate-trace.json', 'process.json', ...(target === 'napi' ? ['child.json'] : [])];
  const expected = new Set(rootFiles), protocol = [`session-open mode=string rows=${fixture.rows.length}`];
  const statuses = {};
  for (const [index, row] of fixture.rows.entries()) {
    const prefix = `${String(index).padStart(3, '0')}-${row.id}`;
    const dir = resolve(path, prefix);
    const status = (await readFile(resolve(dir, 'result.status.bin'))).toString('ascii');
    if (!['success\n', 'error\n', 'candidate-error\n', 'api-exception\n'].includes(status))
      throw Error('invalid candidate row status: ' + prefix);
    const clean = status.slice(0, -1);
    statuses[clean] = (statuses[clean] ?? 0) + 1;
    const countRaw = await readFile(resolve(dir, 'logs.count.bin'));
    const countText = decodeExact(countRaw, 'logs.count.bin');
    if (!/^(0|[1-9][0-9]*)\n$/.test(countText))
      throw Error('invalid logger count bytes: ' + prefix);
    const logCount = Number(countText.trim());
    for (const name of [...common, ...(clean === 'api-exception' ? apiMeta : terminalMeta), ...loggedFiles(logCount)])
      expected.add(prefix + '/' + name);
    const units = textUnits(row.expression);
    if (!(await readFile(resolve(dir, 'expression.bin'))).equals(Buffer.from(javaUtf8Bytes(units)))
        || !(await readFile(resolve(dir, 'expression.code-units.bin'))).equals(Buffer.from(unitsBytes(units))))
      throw Error('candidate expression differs: ' + prefix);
    for (const channel of ['stdout', 'stderr']) {
      const raw = await readFile(resolve(dir, `result.${channel}.bin`));
      const be = await readFile(resolve(dir, `result.${channel}.code-units.bin`));
      if (be.length % 2) throw Error('odd UTF16 channel: ' + prefix);
      const values = Array.from({ length: be.length / 2 }, (_, i) => (be[i * 2] << 8) | be[i * 2 + 1]);
      if (!raw.equals(Buffer.from(javaUtf8Bytes(values)))) throw Error('Java UTF8 result channel differs: ' + prefix);
      if ((clean === 'success' && channel === 'stderr' || clean !== 'success' && channel === 'stdout') && raw.length)
        throw Error('nonempty inactive result channel: ' + prefix);
    }
    for (let logIndex = 0; logIndex < logCount; logIndex++) for (const field of logFields) {
      const stem = `log-${String(logIndex).padStart(3, '0')}.${field}`;
      const raw = await readFile(resolve(dir, stem + '.bin'));
      const be = await readFile(resolve(dir, stem + '.code-units.bin'));
      if (be.length % 2) throw Error('odd UTF16 logger channel: ' + prefix);
      const values = Array.from({ length: be.length / 2 }, (_, i) => (be[i * 2] << 8) | be[i * 2 + 1]);
      if (!raw.equals(Buffer.from(javaUtf8Bytes(values)))) throw Error('Java UTF8 logger channel differs: ' + prefix);
    }
    protocol.push(`call-enter=string-${index}-${row.id}`, `call-exit=string-${index}-${row.id} status=${clean}`);
  }
  protocol.push(`session-closed mode=string rows=${fixture.rows.length} public-calls=${fixture.rows.length}`
    + ` success=${statuses.success ?? 0} errors=${fixture.rows.length - (statuses.success ?? 0)}`);
  const actual = new Set((await filesBelow(path)).map(name => name.slice(path.length + 1).replaceAll('\\', '/')));
  if (actual.size !== expected.size || [...actual].some(name => !expected.has(name)))
    throw Error('missing or extra every capture file');
  if (!(await readFile(resolve(path, 'process.stdout.bin'))).equals(Buffer.from(protocol.join('\n') + '\n')))
    throw Error('whole-session candidate protocol differs');
  return { rowCount: fixture.rows.length, statusCounts: statuses, fileCount: actual.size,
    processStdoutSha256: sha(await readFile(resolve(path, 'process.stdout.bin'))),
    processStderrSha256: sha(await readFile(resolve(path, 'process.stderr.bin'))) };
}
async function persist(path, result) {
  for (const row of result.rows) {
    const directory = resolve(path, `${String(row.index).padStart(3, '0')}-${row.id}`);
    await mkdir(directory);
    for (const [name, bytes] of Object.entries(row.payloads)) {
      if (!/^[a-z0-9.-]+\.bin$/.test(name) || !Array.isArray(bytes)
          || bytes.some(byte => !Number.isInteger(byte) || byte < 0 || byte > 255))
        throw Error('invalid captured collection conversion channel');
      await writeFile(resolve(directory, name), Buffer.from(bytes), { flag: 'wx' });
    }
  }
  await put(resolve(path, 'candidate-trace.json'), { trace: result.trace, architecture: result.architecture,
    acquisitionError: result.acquisitionError, boundary: result.boundary });
}

async function child(args) {
  if (args.length !== 3) throw Error('child INPUT OUTPUT ADDON');
  const [input, output, addon] = args.map(value => resolve(value));
  const fixture = JSON.parse(await readFile(input));
  const api = createRequire(resolve(root, 'tools/platform/package.json'))(addon);
  const result = runEveryLoggerSession(api, fixture);
  await persist(output, result);
  await put(resolve(output, 'child.json'), { pid: process.pid, node: process.version, versions: process.versions,
    executable: process.execPath, argv: process.argv, cwd: process.cwd(),
    environment: process.env,
    architecture: result.architecture, acquisitionError: result.acquisitionError });
  process.stdout.write(Buffer.from(result.protocolBytes));
  if (result.acquisitionError != null || result.rows.length !== fixture.rows.length
      || result.architecture.terminalSlots !== 0 || result.architecture.terminalDepth !== 0)
    process.exitCode = 2;
}

export async function capture(args) {
  if (args.length !== 4) throw Error('OUTPUT WASM_DIR NAPI_BINARY FIXTURE_DIR');
  const [out, wasm, addon, fixturePath] = args.map(value => resolve(value));
  await mkdir(out, { recursive: false });
  const tracked = {}, sets = {}, infrastructureErrors = [];
  const track = async path => { tracked[path] = await shaFile(path); };
  const report = { schema: 1, scope: 'Two fresh complete 64-row N-API processes and Chromium contexts, preserving ordered logger events; Java-only exception metadata remains separate.',
    argv: process.argv, cwd: process.cwd(), environment: Object.fromEntries(['PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'PLAYWRIGHT_BROWSERS_PATH', 'PKL_FIXTURE_PYTHON']
      .filter(name => process.env[name] != null).map(name => [name, process.env[name]])),
    fixture: fixturePath, repetitions: 2, orderedRows: null,
    node: { executable: process.execPath, version: process.version, versions: process.versions, platform: process.platform, arch: process.arch },
    inputsBefore: tracked, fileSetsBefore: sets, targets: { napi: [], chromium: [] },
    infrastructureErrors, artifactSourceBinding: 'Not inferred from supplied addon or Wasm artifacts; external build record required.',
    fullPublicApiComparisonPassed: false, performanceClaim: false, complete: false };
  let fixture;
  try {
    if (fixturePath !== resolve(root, FIXTURE)) throw Error('unexpected every fixture path');
    fixture = await loadEveryFixture(fixturePath);
    report.orderedRows = fixture.rows.length;
    for (const path of [fixturePath, wasm]) {
      sets[path] = await filesBelow(path);
      for (const name of sets[path]) await track(name);
    }
    for (const name of [addon, process.execPath, ...CAPTURE_TRACKED_SOURCES.map(path => resolve(root, path))]) await track(name);
    report.node.executableRealpath = await realpath(process.execPath);
    report.node.executableSha256 = tracked[process.execPath];
    report.artifacts = { napi: { path: addon, realpath: await realpath(addon), sha256: tracked[addon] },
      wasm: { directory: wasm, files: sets[wasm].map(name => ({ path: name, sha256: tracked[name] })) } };
    const python = process.env.PKL_FIXTURE_PYTHON ?? 'python3';
    const py = JSON.parse(execFileSync(python, ['-c', 'import json,sys;print(json.dumps({"executable":sys.executable,"version":sys.version}))'], { cwd: root }).toString());
    await track(py.executable);
    report.python = { ...py, executableRealpath: await realpath(py.executable), executableSha256: tracked[py.executable] };
    const verifierArgs = [resolve(root, VERIFIER), 'collection-every', '--fixture', fixturePath];
    const verification = spawnSync(python, verifierArgs,
      { cwd: root, encoding: null, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
    await writeFile(resolve(out, 'fixture-verification.stdout.bin'), verification.stdout ?? Buffer.alloc(0), { flag: 'wx' });
    await writeFile(resolve(out, 'fixture-verification.stderr.bin'), verification.stderr ?? Buffer.alloc(0), { flag: 'wx' });
    report.fixtureVerification = { command: [python, ...verifierArgs],
      exitCode: verification.status, signal: verification.signal, error: verification.error?.message ?? null,
      stdoutSha256: sha(verification.stdout ?? Buffer.alloc(0)), stderrSha256: sha(verification.stderr ?? Buffer.alloc(0)) };
    if (verification.status !== 0 || verification.signal || verification.error) throw Error('canonical fixture verification failed');
    const provenanceCommand = [py.executable, '-c', javaProvenanceScript,
      fixturePath];
    const extraction = spawnSync(provenanceCommand[0], provenanceCommand.slice(1),
      { cwd: root, encoding: null, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
    await writeFile(resolve(out, 'java-provenance.stdout.bin'), extraction.stdout ?? Buffer.alloc(0), { flag: 'wx' });
    await writeFile(resolve(out, 'java-provenance.stderr.bin'), extraction.stderr ?? Buffer.alloc(0), { flag: 'wx' });
    report.javaProvenanceExtraction = { command: provenanceCommand, cwd: root,
      exitCode: extraction.status, signal: extraction.signal, error: extraction.error?.message ?? null,
      stdoutSha256: sha(extraction.stdout ?? Buffer.alloc(0)), stderrSha256: sha(extraction.stderr ?? Buffer.alloc(0)) };
    if (extraction.status !== 0 || extraction.signal || extraction.error || extraction.stderr?.length)
      throw Error('Java provenance extraction failed');
    report.javaExecution = deriveJavaExecution(JSON.parse(extraction.stdout.toString('utf8')));
    // JDK identity preflight and each actual evaluation JVM are separate
    // processes. The pinned preflight is ko-KR; both evaluations override en-US.
    // Preserve both observations, and run Node/Chromium with the actual session locale.
    report.fixtureInputs = fixture.inputHashes;
    report.caseIds = fixture.rows.map(row => row.id);
    const require = createRequire(resolve(root, 'tools/platform/package.json'));
    const { chromium } = require('playwright');
    const packageJson = JSON.parse(await readFile(resolve(root, 'tools/platform/package.json')));
    const lock = JSON.parse(await readFile(resolve(root, 'tools/platform/package-lock.json')));
    const pin = packageJson.devDependencies.playwright;
    if (!/^\d+\.\d+\.\d+$/.test(pin) || require('playwright/package.json').version !== pin
        || require('playwright-core/package.json').version !== pin || lock.packages[''].devDependencies.playwright !== pin
        || lock.packages['node_modules/playwright'].version !== pin || lock.packages['node_modules/playwright-core'].version !== pin)
      throw Error('browser tooling pin mismatch');
    await track(require.resolve('playwright/package.json'));
    await track(require.resolve('playwright-core/package.json'));
    await track(chromium.executablePath());
    report.playwright = { pin, package: require.resolve('playwright/package.json'),
      corePackage: require.resolve('playwright-core/package.json'), browserExecutable: chromium.executablePath(),
      browserExecutableRealpath: await realpath(chromium.executablePath()),
      browserExecutableSha256: tracked[chromium.executablePath()] };
    for (const repeat of [0, 1]) {
      const path = resolve(out, 'napi', `repeat-${repeat}`);
      await mkdir(path, { recursive: true });
      await put(resolve(path, 'input.json'), fixture);
      const command = [script, '--child', resolve(path, 'input.json'), path, addon];
      const childResult = spawnSync(process.execPath, command, { cwd: root, env: report.javaExecution.environment,
        encoding: null, timeout: 180_000, maxBuffer: 8 * 1024 * 1024 });
      for (const channel of ['stdout', 'stderr'])
        await writeFile(resolve(path, `process.${channel}.bin`), childResult[channel] ?? Buffer.alloc(0), { flag: 'wx' });
      const observation = { repeat, uri: fixture.uri, rowCount: fixture.rows.length, directory: path,
        command: [process.execPath, ...command], isolation: 'new-node-child-and-SourceProbe-for-entire-ordered-session',
        exitCode: childResult.status, signal: childResult.signal, error: childResult.error?.message ?? null,
        processStdoutSha256: sha(childResult.stdout ?? Buffer.alloc(0)),
        processStderrSha256: sha(childResult.stderr ?? Buffer.alloc(0)) };
      await put(resolve(path, 'process.json'), observation);
      report.targets.napi.push(observation);
      if (childResult.status === 0 && !childResult.signal && !childResult.error) {
        try { observation.fileSet = await validateSessionFileSet(path, fixture, 'napi'); }
        catch (error) { infrastructureErrors.push({ target: 'napi', repeat, message: error.message }); }
      }
    }
    const routes = new Map([
      ['/session.mjs', resolve(root, 'tools/platform/every-logger-session.mjs')],
      ['/module-policy-session.mjs', resolve(root, 'tools/platform/module-policy-session.mjs')],
      ['/resource-session.mjs', resolve(root, 'tools/platform/resource-session.mjs')],
      // The canonical session's relative imports resolve from /session.mjs.
      ['/pkl_wasm.js', resolve(wasm, 'pkl_wasm.js')],
      ['/pkl_wasm_bg.wasm', resolve(wasm, 'pkl_wasm_bg.wasm')],
    ]);
    report.httpRequests = [];
    const server = createServer(async (request, response) => {
      let pathname = null;
      try {
        pathname = new URL(request.url, 'http://127.0.0.1').pathname;
        if (pathname === '/') { report.httpRequests.push({ path: pathname, status: 200 }); response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>collection every capture</title>'); return; }
        if (pathname === '/favicon.ico') { report.httpRequests.push({ path: pathname, status: 204 }); response.writeHead(204); response.end(); return; }
        const path = routes.get(pathname); if (!path) throw Error('unknown artifact route');
        const body = await readFile(path);
        report.httpRequests.push({ path: pathname, status: 200 });
        response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' });
        response.end(body);
      } catch { report.httpRequests.push({ path: pathname ?? request.url, status: 404 }); response.writeHead(404); response.end(); }
    });
    let browser;
    try {
      await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
      const launchOptions = { headless: true, executablePath: chromium.executablePath(),
        env: report.javaExecution.environment };
      report.playwright.launchOptions = launchOptions;
      const contextOptions = { locale: report.javaExecution.locale,
        timezoneId: report.javaExecution.timezoneId };
      report.playwright.contextOptions = contextOptions;
      browser = await chromium.launch(launchOptions);
      report.playwright.browserVersion = browser.version();
      for (const repeat of [0, 1]) {
        const path = resolve(out, 'chromium', `repeat-${repeat}`);
        await mkdir(path, { recursive: true });
        await put(resolve(path, 'input.json'), fixture);
        const context = await browser.newContext(contextOptions);
        const observation = { repeat, uri: fixture.uri, rowCount: fixture.rows.length, directory: path,
          isolation: 'new-browser-context-and-Wasm-instance-for-entire-ordered-session',
          browserErrors: [], acquisitionError: null };
        try {
          const page = await context.newPage();
          page.on('pageerror', error => observation.browserErrors.push({ message: error.message, stack: error.stack }));
          page.on('crash', () => observation.browserErrors.push({ message: 'page crashed' }));
          page.on('console', message => { if (message.type() === 'error') observation.browserErrors.push({ message: message.text() }); });
          await page.goto(`http://127.0.0.1:${server.address().port}`);
          observation.contextObserved = await page.evaluate(() => ({
            locale: navigator.language, timezoneId: Intl.DateTimeFormat().resolvedOptions().timeZone,
          }));
          if (observation.contextObserved.locale !== contextOptions.locale
              || observation.contextObserved.timezoneId !== contextOptions.timezoneId)
            throw Error('Chromium actual context locale or timezone differs');
          const result = await page.evaluate(async input => {
            const api = await import('/pkl_wasm.js'); await api.default();
            const { runEveryLoggerSession } = await import('/session.mjs');
            return runEveryLoggerSession(api, input);
          }, fixture);
          await persist(path, result);
          // This is the page-returned protocol byte stream, not OS process stdout.
          await writeFile(resolve(path, 'process.stdout.bin'), Buffer.from(result.protocolBytes), { flag: 'wx' });
          observation.protocolOrigin = 'browser-page-returned-UTF8';
          observation.acquisitionError = result.acquisitionError;
          observation.architecture = result.architecture;
        } catch (error) { observation.acquisitionError = errorRecord(error); }
        finally { await context.close(); }
        // Browser console/page errors have no Java-like process stderr channel;
        // preserve them explicitly and fail admission when present.
        await writeFile(resolve(path, 'process.stderr.bin'),
          Buffer.from(observation.browserErrors.map(error => json(error)).join('')), { flag: 'wx' });
        observation.stderrOrigin = 'browser-pageerror-and-console-error-capture';
        await put(resolve(path, 'process.json'), observation);
        report.targets.chromium.push(observation);
        if (observation.acquisitionError == null && !observation.browserErrors.length) {
          try { observation.fileSet = await validateSessionFileSet(path, fixture, 'chromium'); }
          catch (error) { infrastructureErrors.push({ target: 'chromium', repeat, message: error.message }); }
        }
      }
    } finally {
      if (browser) await browser.close();
      if (server.listening) await new Promise(ok => server.close(ok));
    }
  } catch (error) { infrastructureErrors.push(errorRecord(error)); }
  report.inputsAfter = {};
  for (const path of Object.keys(tracked)) {
    try { report.inputsAfter[path] = await shaFile(path); }
    catch { report.inputsAfter[path] = null; }
  }
  report.fileSetsAfter = {};
  for (const path of Object.keys(sets)) {
    try { report.fileSetsAfter[path] = await filesBelow(path); }
    catch { report.fileSetsAfter[path] = null; }
  }
  if (fixture) for (const rows of Object.values(report.targets)) {
    try { requireSessionCoverage(fixture, rows); }
    catch (error) { infrastructureErrors.push({ message: error.message }); }
  }
  report.complete = infrastructureErrors.length === 0
    && JSON.stringify(report.inputsBefore) === JSON.stringify(report.inputsAfter)
    && JSON.stringify(report.fileSetsBefore) === JSON.stringify(report.fileSetsAfter)
    && report.targets.napi.every(row => row.exitCode === 0 && row.signal == null && row.error == null
      && row.fileSet && row.processStderrSha256 === sha(Buffer.alloc(0)))
    && report.targets.chromium.every(row => row.acquisitionError == null && !row.browserErrors.length && row.fileSet
      && row.fileSet.processStderrSha256 === sha(Buffer.alloc(0))
      && row.architecture?.terminalSlots === 0 && row.architecture?.terminalDepth === 0
      && row.architecture?.retainedPayloadsStable === true);
  await put(resolve(out, 'report.json'), report);
  if (!report.complete) process.exitCode = 1;
  return report;
}

const errorRecord = error => ({ name: error?.name ?? null, message: String(error?.message ?? error), stack: error?.stack ?? null });
if (process.argv[1] && resolve(process.argv[1]) === script) {
  if (process.argv[2] === '--child') await child(process.argv.slice(3));
  else await capture(process.argv.slice(2));
}
