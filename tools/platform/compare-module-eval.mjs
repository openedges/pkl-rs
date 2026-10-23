// Compare explicit shared module frontend/output byte APIs to original CLI payloads.
// This is not native CLI process/files compatibility or complete module eval.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, relative, basename, sep, extname } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { createHash } from 'node:crypto';
import { evaluateModuleFixtures } from './module-eval-session.mjs';

const script = fileURLToPath(import.meta.url);
const root = resolve(dirname(script), '../..');
const hash = bytes => createHash('sha256').update(bytes).digest('hex');
const json = value => JSON.stringify(value, null, 2) + '\n';

function ordered(value) {
  if (Array.isArray(value)) return value.map(ordered);
  if (value && typeof value === 'object') return Object.fromEntries(Object.keys(value).sort().map(key => [key, ordered(value[key])]));
  return value;
}
const same = (left, right) => JSON.stringify(ordered(left)) === JSON.stringify(ordered(right));
const compatHash = value => hash(Buffer.from(JSON.stringify(ordered(value), null, 2) + '\n'));

// Validate the independently declared original contract, not just two copies of
// an acquisition record that could agree on an incorrect URI or command.
export function assertModuleProcessContext(row, profile, process, lock) {
  const state = resolve(profile.workspace, '.state', hash(Buffer.from(row.id)).slice(0, 24));
  const home = resolve(state, 'home'), temporary = resolve(state, 'tmp');
  const prefix = [profile.jdk.executable, ...lock.jvm_options,
    `-Duser.home=${profile.environment.HOME}`, `-Djava.io.tmpdir=${profile.environment.TMPDIR}`,
    '-cp', profile.artifact, 'org.pkl.cli.Main'];
  if (!same(prefix, profile.command)) throw Error('pinned Java prefix differs');
  const command = prefix.map(arg => arg.startsWith('-Duser.home=') ? `-Duser.home=${home}`
    : arg.startsWith('-Djava.io.tmpdir=') ? `-Djava.io.tmpdir=${temporary}` : arg).concat(row.upstream_case.command);
  const environment = { ...profile.environment, ...row.upstream_case.environment,
    HOME: home, TMPDIR: temporary, TMP: temporary, TEMP: temporary };
  const recorded = Object.fromEntries([...new Set([...Object.keys(row.upstream_case.environment),
    'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'NO_COLOR'])]
    .filter(key => Object.hasOwn(environment, key)).map(key => [key, environment[key]]));
  if (process.cwd !== resolve(profile.workspace, row.upstream_case.cwd)
      || process.moduleUri !== pathToFileURL(resolve(profile.workspace, row.input)).href
      || !same(process.command, command) || !same(process.environment, recorded)
      || !same(process.fullAllowlistedEnvironment, environment)) throw Error('original Java execution context differs: ' + row.id);
}

export function validateModuleFilesystem(row, baseline, process, result, capturedFiles) {
  for (const resource of [{ path: row.input, kind: 'file', sha256: row.input_sha256 }, ...row.upstream_case.resources]) {
    if (!same(baseline[resource.path], { kind: resource.kind ?? 'file', sha256: resource.sha256 }))
      throw Error('original resource baseline differs: ' + resource.path);
  }
  if (process.workspaceBeforeSha256 !== compatHash(baseline) || !same(process.result, result))
    throw Error('workspace before/result binding differs');
  const after = { ...baseline };
  for (const [name, change] of Object.entries(result.workspace_changes)) {
    if (name.startsWith('/') || name.split('/').includes('..') || !same(change.before, baseline[name] ?? null))
      throw Error('workspace transition differs: ' + name);
    if (change.after == null) delete after[name]; else after[name] = change.after;
    if (change.after != null && change.after.kind !== 'directory'
        && result.files['workspace/' + name] !== change.after.sha256)
      throw Error('workspace changed bytes missing: ' + name);
  }
  if (process.workspaceAfterSha256 !== compatHash(after)) throw Error('workspace after binding differs');
  const actualFiles = Object.fromEntries(Object.entries(capturedFiles).map(([name, bytes]) => [name, hash(bytes)]));
  if (!same(actualFiles, result.files)) throw Error('captured Java files differ');
}

export function moduleOracleOutcome(result, files, stderr) {
  if (result.timed_out) return 'timeout';
  if (Object.keys(result.workspace_changes).length || Object.keys(files).length) return 'unexpected-workspace-write';
  if (result.exit_code === 0) return 'value';
  const prefix = Buffer.from('–– Pkl Error ––\n');
  return result.exit_code === 1 && stderr.subarray(0, prefix.length).equals(prefix)
    ? 'ordinary-language-error' : 'internal-or-process-error';
}

export function compareModulePayload(expected, actual, oracleOutcome, options = {}) {
  const channels = actual ? {
    stdout: Buffer.from(actual.stdout).equals(expected.stdout),
    stderr: Buffer.from(actual.stderr).equals(expected.stderr),
    exitCode: actual.exitCode === expected.exitCode,
  } : { stdout: false, stderr: false, exitCode: false };
  const rawMatch = Object.values(channels).every(Boolean);
  // An oracle bug, timeout, unsupported implementation, or transport failure
  // never becomes an ordinary compatibility pass because raw bytes coincide.
  const outputSuccess = options.operation === 'output-bytes' && actual?.exitCode === 0 && actual.disposition == null;
  const upstreamExpectedExitMatch = options.upstreamExpectedExit == null ? null
    : options.upstreamExpectedExit.includes(expected.exitCode);
  const status = !actual ? 'acquisition-failed' : oracleOutcome !== 'ordinary-language-error' && oracleOutcome !== 'value'
    ? 'oracle-error' : upstreamExpectedExitMatch === false ? 'oracle-contract-mismatch'
    : actual.disposition !== 'language-error' && !outputSuccess ? actual.disposition === 'unsupported' ? 'unsupported' : 'non-language-failure'
    : rawMatch ? 'pass' : 'mismatch';
  return { channels, rawMatch, status, upstreamExpectedExitMatch };
}

export async function compareModuleEval(args) {
  let operation = 'frontend';
  if (args[0] === '--operation') {
    operation = args[1]; args = args.slice(2);
    if (!['frontend', 'output-bytes'].includes(operation)) throw Error('unknown module operation');
  }
  const outputOperation = operation === 'output-bytes';
  const preflightOnly = args.length === 2 && args[0] === '--verify-oracle';
  if (!preflightOnly && (args.length !== 4 || args.some(arg => !arg)))
    throw Error('usage: compare-module-eval.mjs [--operation frontend|output-bytes] OUTPUT WASM_DIR NAPI_BINARY JAVA_ACQUISITION | [--operation frontend|output-bytes] --verify-oracle JAVA_ACQUISITION');
  const [output, wasm, addon, oracleRoot] = preflightOnly ? [null, null, null, resolve(args[1])] : args.map(arg => resolve(arg));
  const require = createRequire(resolve(root, 'tools/platform/package.json'));
  const chromium = preflightOnly ? null : require('playwright').chromium;
  const manifestPath = resolve(root, 'tools/platform/package.json');
  const lockPath = resolve(root, 'tools/platform/package-lock.json');
  const manifest = JSON.parse(await readFile(manifestPath));
  const lock = JSON.parse(await readFile(lockPath));
  const pin = manifest.devDependencies.playwright;
  const playwright = preflightOnly ? null : require('playwright/package.json').version;
  const playwrightCore = preflightOnly ? null : require('playwright-core/package.json').version;
  if (!/^\d+\.\d+\.\d+$/.test(pin) || (!preflightOnly && (playwright !== pin || playwrightCore !== pin))
      || lock.packages[''].devDependencies.playwright !== pin
      || lock.packages['node_modules/playwright'].version !== pin
      || lock.packages['node_modules/playwright-core'].version !== pin) throw Error('Playwright pin mismatch');
  const originalPath = resolve(oracleRoot, outputOperation ? 'original30.json' : 'original72.json');
  const oracleReportPath = resolve(oracleRoot, 'report.json');
  const original = JSON.parse(await readFile(originalPath));
  const oracle = JSON.parse(await readFile(oracleReportPath));
  const profilePath = resolve(oracleRoot, 'profile.json');
  const profile = JSON.parse(await readFile(profilePath));
  const oracleLockPath = resolve(root, 'oracle.lock.json');
  const oracleLock = JSON.parse(await readFile(oracleLockPath));
  const registryPath = resolve(root, 'compatibility/m2-module-eval.json');
  const registry = JSON.parse(await readFile(registryPath));
  const corpusPath = resolve(root, 'compatibility/corpus.json');
  const corpus = JSON.parse(await readFile(corpusPath));
  const closurePath = resolve(oracleRoot, 'closure.json');
  const closure = JSON.parse(await readFile(closurePath));
  const inventoryPath = resolve(oracleRoot, 'inventory.json');
  const inventory = JSON.parse(await readFile(inventoryPath));
  const phaseLinkPath = resolve(oracleRoot, 'phase-link.json');
  const phaseLink = outputOperation ? null : JSON.parse(await readFile(phaseLinkPath));
  const phaseReportPath = phaseLink?.reportPath;
  const phaseReport = outputOperation ? null : JSON.parse(await readFile(phaseReportPath));
  if (!outputOperation && (hash(await readFile(phaseReportPath)) !== phaseLink.reportSha256
      || phaseReport.caseCount !== original.length || phaseReport.artifactSha256 !== profile.artifactSha256
      || phaseReport.originalCliProfileSha256 !== hash(await readFile(profilePath))))
    throw Error('supplemental phase observation binding differs');
  const phaseById = new Map((phaseReport?.records ?? []).map(row => [row.id, row]));

  const baselinePath = resolve(oracleRoot, 'workspace-baseline.json');
  const baseline = JSON.parse(await readFile(baselinePath));
  const acquisitionManifestPath = resolve(oracleRoot, 'manifest.json');
  const acquisitionManifest = JSON.parse(await readFile(acquisitionManifestPath));
  if (!same(acquisitionManifest.sources, corpus.sources)) throw Error('Java original resource-set declaration differs');
  for (const source of corpus.sources) {
    if (compatHash(source.resources) !== source.resource_set_sha256) throw Error('resource inventory hash differs');
    for (const resource of source.resources) {
      if (!same(baseline[resource.path], { kind: resource.kind, sha256: resource.sha256 }))
        throw Error('original resource-set baseline differs: ' + resource.path);
    }
  }
  if (closure.status !== 'closed' || closure.reportSha256 !== hash(await readFile(oracleReportPath))
      || closure.inventorySha256 !== hash(await readFile(inventoryPath))
      || !Array.isArray(inventory.files) || closure.files !== inventory.files.length)
    throw Error('Java acquisition closure differs');
  const inventoryNames = new Set();
  for (const entry of inventory.files) {
    const path = resolve(oracleRoot, entry.path);
    if (!path.startsWith(oracleRoot + sep) || inventoryNames.has(entry.path)) throw Error('unsafe/duplicate Java inventory entry');
    inventoryNames.add(entry.path);
    const bytes = await readFile(path);
    if (bytes.length !== (entry.bytes ?? entry.size) || hash(bytes) !== entry.sha256) throw Error('Java closure payload differs: ' + entry.path);
  }
  if (profile.lockSha256 !== hash(await readFile(oracleLockPath)) || profile.artifactSha256 !== oracleLock.artifact.sha256
      || !same(profile.reference, oracleLock.source) || hash(await readFile(profile.artifact)) !== oracleLock.artifact.sha256
      || hash(await readFile(profile.jdk.executable)) !== profile.jdk.executable_sha256
      || hash(await readFile(resolve(profile.jdk.properties['java.home'], 'lib/modules'))) !== profile.jdk.runtime_modules_sha256
      || hash(await readFile(resolve(profile.jdk.properties['java.home'], 'release'))) !== profile.jdk.release_sha256
      || Object.entries(oracleLock.jdk.properties).some(([key, value]) => profile.jdk.properties[key] !== value))
    throw Error('pinned Java oracle identity differs');
  const registered = registry.cases?.slice(outputOperation ? 72 : 0, outputOperation ? 102 : 72);
  if (!Array.isArray(registered) || !same(original.map(row => row.id), registered.map(row => row.id)))
    throw Error('original IDs differ from reviewed registry');
  const corpusById = new Map(corpus.cases.map(row => [row.id, row]));
  for (const [index, row] of original.entries()) {
    const declaration = registered[index];
    if (row.input_sha256 !== declaration.input_sha256 || compatHash(row.upstream_case) !== declaration.upstream_case_sha256
        || !same(row.upstream_case, corpusById.get(row.id)) || row.upstream_case.command.at(-1) !== row.input)
      throw Error('original upstream contract differs: ' + row.id);
  }
  if (!Array.isArray(original) || original.length !== (outputOperation ? 30 : 72) || new Set(original.map(row => row.id)).size !== original.length
      || oracle.caseCount !== original.length || oracle.processCount !== 2 * original.length
      || !oracle.workspaceRestored || !oracle.artifactStable) throw Error('expected closed complete original CLI acquisition');
  const hashes = {};
  async function record(path) {
    if (path.startsWith(oracleRoot + sep) && path !== closurePath && path !== inventoryPath
        && !inventoryNames.has(relative(oracleRoot, path).split(sep).join('/')))
      throw Error('required Java payload missing from closure: ' + path);
    hashes[path] = hash(await readFile(path));
  }
  async function capturedFiles(directory) {
    const result = {};
    async function visit(path, prefix = '') {
      let entries;
      try { entries = await readdir(path, { withFileTypes: true }); }
      catch (error) { if (error.code === 'ENOENT' && path === directory) return; throw error; }
      for (const entry of entries) {
        const child = resolve(path, entry.name), name = prefix + entry.name;
        if (entry.isDirectory()) await visit(child, name + '/');
        else if (entry.isFile()) { result[name] = await readFile(child); await record(child); }
        else throw Error('unexpected Java capture entry: ' + child);
      }
    }
    await visit(directory); return result;
  }
  async function tree(path) {
    for (const entry of await readdir(path, { withFileTypes: true })) {
      const child = resolve(path, entry.name);
      if (entry.isDirectory()) await tree(child);
      else if (entry.isFile()) await record(child);
      else throw Error('unexpected artifact entry: ' + child);
    }
  }
  const fixtures = [];
  const expected = new Map();
  for (const [index, row] of original.entries()) {
    const sourcePath = resolve(oracleRoot, 'inputs', row.input);
    if (!sourcePath.startsWith(resolve(oracleRoot, 'inputs') + sep)) throw Error('unsafe original input path');
    const bytes = await readFile(sourcePath);
    if (hash(bytes) !== row.input_sha256 || !(await readFile(resolve(root, row.input))).equals(bytes))
      throw Error('original source changed: ' + row.id);
    const sourceText = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
    if (!Buffer.from(sourceText, 'utf8').equals(bytes)) throw Error('source UTF8 roundtrip differs: ' + row.id);
    const directory = hash(Buffer.from(row.id, 'utf8')).slice(0, 24);
    const phase = phaseById.get(row.id);
    if (!outputOperation && (!phase || phase.input !== row.input || phase.inputSha256 !== row.input_sha256
        || phase.outcome !== 'org.pkl.parser.ParserError')) throw Error('supplemental phase input differs: ' + row.id);
    const expectedFrontendPhase = outputOperation ? null : phase.throwSite?.includes('org.pkl.parser.Lexer.') ? 'lexing'
      : phase.throwSite?.includes('org.pkl.parser.ParserImpl.') ? 'parsing' : null;
    if (!outputOperation && expectedFrontendPhase === null) throw Error('unknown supplemental parser throw site');
    let uri;
    for (const repeat of [0, 1]) {
      const caseRoot = resolve(oracleRoot, 'cases', directory, String(repeat));
      const processPath = resolve(caseRoot, 'process.json');
      const process = JSON.parse(await readFile(processPath));
      const resultPath = resolve(caseRoot, 'result.json');
      const result = JSON.parse(await readFile(resultPath));
      assertModuleProcessContext(row, profile, process, oracleLock);
      const reportRecord = oracle.records.find(item => item.id === row.id && item.repeat === repeat);
      if (process.id !== row.id || process.repeat !== repeat || process.index !== index
          || process.sourceSha256 !== row.input_sha256 || JSON.stringify(process.originalArguments) !== JSON.stringify(row.upstream_case.command)
          || JSON.stringify(process) !== JSON.stringify(reportRecord)
          || JSON.stringify(result) !== JSON.stringify(process.result)) throw Error('Java process binding mismatch: ' + row.id);
      if (uri != null && uri !== process.moduleUri) throw Error('repeat URI differs: ' + row.id);
      uri = process.moduleUri;
      if (!outputOperation && phase.moduleUri !== uri) throw Error('supplemental phase URI differs: ' + row.id);
      const files = await capturedFiles(resolve(caseRoot, 'files'));
      validateModuleFilesystem(row, baseline, process, result, files);
      const payload = { exitCode: result.exit_code, declaredOracleOutcome: process.outcome, files,
        upstreamExpectedExit: row.upstream_case.expected_exit };
      for (const channel of ['stdout', 'stderr']) {
        const path = resolve(caseRoot, channel + '.bin'); payload[channel] = await readFile(path);
        if (hash(payload[channel]) !== result.sha256[channel]) throw Error('Java channel checksum mismatch: ' + path);
        await record(path);
      }
      payload.oracleOutcome = moduleOracleOutcome(result, files, payload.stderr);
      await record(processPath); await record(resultPath);
      expected.set(row.id + ':' + repeat, payload);
    }
    fixtures.push({ id: row.id, operation, directory, expectedFrontendPhase, sourceText, sourceSha256: row.input_sha256, uri,
      moduleName: basename(row.input, '.pkl'), arguments: row.upstream_case.command });
    await record(sourcePath); await record(resolve(root, row.input));
  }
  if (preflightOnly) {
    const result = { preflightOnly: true, oracleVerified: true, caseCount: fixtures.length, processCount: expected.size,
      oracleReportSha256: hash(await readFile(oracleReportPath)), closureSha256: hash(await readFile(closurePath)),
      operation, inventoryFilesVerified: inventory.files.length, supplementalPhaseReportSha256: phaseLink?.reportSha256 ?? null,
      artifactAndInputHashes: hashes, scope: 'Offline original Java evidence validation only; no candidate API, process or browser execution.' };
    console.log(json(result)); return result;
  }
  await mkdir(output, { recursive: false });
  for (const path of [script, resolve(dirname(script), 'module-eval-session.mjs'), originalPath, oracleReportPath,
    profilePath, closurePath, inventoryPath, ...(!outputOperation ? [phaseLinkPath, phaseReportPath] : []), baselinePath, acquisitionManifestPath, registryPath, corpusPath, oracleLockPath, manifestPath, lockPath, addon]) await record(path);
  await tree(wasm);
  await writeFile(resolve(output, 'fixtures.json'), json(fixtures));
  await writeFile(resolve(output, 'compare-module-eval.mjs'), await readFile(script));
  await writeFile(resolve(output, 'module-eval-session.mjs'), await readFile(resolve(dirname(script), 'module-eval-session.mjs')));
  const report = { schema: 1, operation, scope: outputOperation
    ? 'Original30 actual module-output byte-vector API on Node/Chromium, two calls each. Whole stdout/stderr/exitCode compared to original CLI; native process/files remain separate obligations. Unexpected original Java exit status cannot pass even when raw channels match.'
    : 'Original72 shared frontend byte-vector API on actual Node/Chromium, two calls each. Whole stdout/stderr/exitCode compared to original CLI; native process/files and successful output evaluation remain separate obligations.',
    excludedNativeChannels: ['native process execution and OS exit status', 'generated files and workspace mutations'],
    argv: process.argv, cwd: process.cwd(), oracleReportPath, oracleReportSha256: hash(await readFile(oracleReportPath)),
    phaseObservation: outputOperation ? null : { reportPath: phaseReportPath, reportSha256: phaseLink.reportSha256, scope: 'Supplemental Parser API phase; independent of raw CLI comparison status.' },
    caseCount: fixtures.length, repetitions: 2, node: process.version, nodeVersions: process.versions,
    nodeExecutable: process.execPath, nodeExecutableSha256: hash(await readFile(process.execPath)),
    playwright, playwrightCore, playwrightPin: pin, artifactAndInputHashes: hashes, targets: {}, infrastructureErrors: [], browserErrors: [] };
  async function save(target, rows) {
    const seen = new Set(), results = [];
    for (const row of rows) {
      const key = row.id + ':' + row.repeat;
      if (!expected.has(key) || seen.has(key)) throw Error('unexpected/duplicate portable call: ' + key);
      seen.add(key);
      const fixture = fixtures.find(item => item.id === row.id);
      const origin = expected.get(key);
      if (row.operation !== operation) throw Error('portable operation differs: ' + key);
      const compared = compareModulePayload(origin, row.result, origin.oracleOutcome,
        { operation, upstreamExpectedExit: outputOperation ? origin.upstreamExpectedExit : null });
      const directory = resolve(output, target, fixture.directory, String(row.repeat));
      await mkdir(directory, { recursive: true });
      const channels = {};
      for (const channel of ['stdout', 'stderr']) {
        await writeFile(resolve(directory, 'java.' + channel + '.bin'), origin[channel]);
        if (row.result) await writeFile(resolve(directory, channel + '.bin'), Buffer.from(row.result[channel]));
        channels[channel] = { expectedSha256: hash(origin[channel]), actualSha256: row.result ? hash(Buffer.from(row.result[channel])) : null,
          rawEqual: compared.channels[channel] };
      }
      for (const [name, bytes] of Object.entries(origin.files)) {
        const path = resolve(directory, 'java.files', name);
        if (!path.startsWith(resolve(directory, 'java.files') + sep)) throw Error('unsafe Java generated filename');
        await mkdir(dirname(path), { recursive: true }); await writeFile(path, bytes);
      }
      const result = { id: row.id, repeat: row.repeat, operation, uri: fixture.uri, status: compared.status, rawMatch: compared.rawMatch,
        upstreamExpectedExit: outputOperation ? origin.upstreamExpectedExit : null,
        upstreamExpectedExitMatch: compared.upstreamExpectedExitMatch,
        oracleOutcome: origin.oracleOutcome, acquisitionError: row.acquisitionError,
        declaredOracleOutcome: origin.declaredOracleOutcome,
        excludedJavaFiles: Object.fromEntries(Object.entries(origin.files).map(([name, bytes]) => [name, hash(bytes)])),
        expectedExitCode: origin.exitCode, actualExitCode: row.result?.exitCode ?? null,
        exitCodeEqual: compared.channels.exitCode, phase: row.result?.phase ?? null,
        expectedFrontendPhase: fixture.expectedFrontendPhase,
        phaseMatch: outputOperation ? null : row.result?.phase === fixture.expectedFrontendPhase,
        disposition: row.result?.disposition ?? null, reason: row.result?.reason ?? null,
        dispositionPresent: row.result?.dispositionPresent ?? null, dispositionKind: row.result?.dispositionKind ?? null,
        reasonPresent: row.result?.reasonPresent ?? null, reasonKind: row.result?.reasonKind ?? null, channels };
      await writeFile(resolve(directory, 'result.json'), json(result)); results.push(result);
    }
    return { complete: seen.size === expected.size, executions: results.length,
      statusCounts: results.reduce((acc, item) => ({ ...acc, [item.status]: (acc[item.status] ?? 0) + 1 }), {}), results };
  }
  try { report.targets.node = await save('node', evaluateModuleFixtures(require(addon), fixtures)); }
  catch (error) { report.infrastructureErrors.push({ target: 'node', message: error.message, stack: error.stack }); }
  const server = createServer(async (request, response) => {
    try {
      const url = new URL(request.url, 'http://127.0.0.1');
      if (url.pathname === '/') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>Module frontend verification</title>'); return; }
      const path = url.pathname === '/module-eval-session.mjs' ? resolve(dirname(script), 'module-eval-session.mjs')
        : resolve(wasm, '.' + decodeURIComponent(url.pathname));
      if (url.pathname !== '/module-eval-session.mjs' && !path.startsWith(wasm + sep)) throw Error('unsafe Wasm path');
      const assetBytes = await readFile(path); response.writeHead(200, { 'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript' }); response.end(assetBytes);
    } catch { if (!response.headersSent) { response.writeHead(404); response.end(); } else response.destroy(); }
  });
  let browser;
  try {
    await new Promise((ok, fail) => { server.once('error', fail); server.listen(0, '127.0.0.1', ok); });
    report.browserExecutable = chromium.executablePath();
    report.browserExecutableSha256 = hash(await readFile(report.browserExecutable));
    report.browserLaunchOptions = {headless: true, executablePath: report.browserExecutable};
    browser = await chromium.launch(report.browserLaunchOptions);
    report.browser = browser.version();
    const page = await browser.newPage();
    page.on('pageerror', error => report.browserErrors.push({ type: 'pageerror', message: error.message, stack: error.stack }));
    page.on('crash', () => report.browserErrors.push({ type: 'crash' }));
    await page.goto(`http://127.0.0.1:${server.address().port}`);
    const rows = await page.evaluate(async fixtures => {
      const api = await import('/pkl_wasm.js'); await api.default();
      const { evaluateModuleFixtures } = await import('/module-eval-session.mjs');
      return evaluateModuleFixtures(api, fixtures);
    }, fixtures);
    report.targets.browser = await save('browser', rows);
  } catch (error) { report.infrastructureErrors.push({ target: 'browser', message: error.message, stack: error.stack }); }
  finally { if (browser) await browser.close(); if (server.listening) await new Promise(ok => server.close(ok)); }
  report.changedInputs = [];
  for (const [path, expectedHash] of Object.entries(hashes)) if (hash(await readFile(path)) !== expectedHash) report.changedInputs.push(path);
  report.compatibilityComplete = !report.infrastructureErrors.length && !report.browserErrors.length && !report.changedInputs.length
    && ['node', 'browser'].every(target => report.targets[target]?.complete && report.targets[target].results.every(row => row.status === 'pass'));
  report.phaseObservationsMatched = outputOperation ? null : ['node', 'browser'].every(target => report.targets[target]?.complete
    && report.targets[target].results.every(row => row.phaseMatch));
  await writeFile(resolve(output, 'execution.json'), json(report));
  console.log(json({ compatibilityComplete: report.compatibilityComplete, phaseObservationsMatched: report.phaseObservationsMatched,
    targets: Object.fromEntries(Object.entries(report.targets).map(([target, value]) => [target, value.statusCounts])),
    infrastructureErrors: report.infrastructureErrors, browserErrors: report.browserErrors, changedInputs: report.changedInputs }));
  return report;
}
if (process.argv[1] && resolve(process.argv[1]) === script) {
  const report = await compareModuleEval(process.argv.slice(2));
  if (!report.preflightOnly && !report.compatibilityComplete) process.exitCode = 1;
}
