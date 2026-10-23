// Actual supplied-primary ModuleKey comparison. Java raw and full lifecycle gates stay distinct.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, lstat, realpath} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve, dirname, relative, isAbsolute, sep, extname} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {runModuleKeyCase, compareModuleKeyCase} from './modulekey-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

export const PINS = Object.freeze({
  fixture: 'ebd3591361efc0a03e98e38695b120e9c625d9c09d0438fc641de420114aab97',
  verifier: '14b95a5cf0b9268b7072a0070d1b2f289c00f67c8212ca0b5ace3aa5174306c8',
  sidecar: '1ba947f9d3935cf3424f85e9dc19a6e220f5f8cf482206ad42e087ef9ac0b3d7',
});
const SOURCE_PINS = Object.freeze({
  'pkl-core/src/main/java/org/pkl/core/util/IoUtils.java': 'e0d14b69e64185c9ef4b771648173d5584b9ab910e717244a6ec889ca43c9e10',
  'pkl-core/src/main/java/org/pkl/core/module/ModuleKeys.java': '137c1635b1bbc0894f30171355c50719a6c0c15b40e92aa44b51104f20103a05',
  'pkl-core/src/main/java/org/pkl/core/module/ModuleKey.java': '4cada0267fa1013d602b5594b04bbb8f150e942af48d01ae94398f9b3f7915f9',
  'pkl-core/src/main/java/org/pkl/core/runtime/ReaderBase.java': '940086123ed49c72568dcd809cfd0d3bf8627de5bf5b55d8a61fae9f4fc71a62',
  'pkl-core/src/main/java/org/pkl/core/runtime/ModuleCache.java': '96eb6429a710d0b4db09664b6d84a0cb25a17a3206983183f66a65382ae69d82',
  'pkl-core/src/main/java/org/pkl/core/runtime/ModuleResolver.java': 'f0f7d42b9b9f85362efd31ec3631df123f192e864e0d86a647502a7485c25db0',
  'pkl-core/src/main/java/org/pkl/core/module/ModuleKeyFactories.java': '6360af26c7011bcd24d28ca9acb90dc817f9e93bc19e1c82a987db52a17ec08d',
});
const json = value => JSON.stringify(value, null, 2) + '\n';
export const hash = value => createHash('sha256').update(value).digest('hex');
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
function check(ok, message) { if (!ok) throw Error(message); }
function inside(root, rel) {
  check(typeof rel === 'string' && !isAbsolute(rel) && !rel.split(/[\\/]/).includes('..'), 'unsafe declared relative path');
  const path = resolve(root, rel); check(path.startsWith(resolve(root) + sep), 'relative path escapes root'); return path;
}
export function parseArguments(argv) {
  const values = [...argv], preflightOnly = values[0] === '--preflight-only';
  if (preflightOnly) values.shift();
  check(values.length === 13,
    'usage: compare-modulekey.mjs [--preflight-only] OUTPUT WASM_DIR ADDON FIXTURES_DIR HOST_METADATA VERIFIER PINNED_PACKAGE_JSON BROWSER_EXECUTABLE BROWSER_SHA256 ARTIFACT_PINS_JSON ARTIFACT_PINS_SHA256 COHORT ENTRYPOINT');
  const [output, wasm, addon, fixtureRoot, metadata, verifier, packagePath, browserExecutable,
    browserSha256, artifactPins, artifactPinsSha256, cohort, entrypoint] = values;
  check(['spi132', 'factory80', 'all212'].includes(cohort), 'explicit cohort spi132|factory80|all212 required');
  check(['provided-primary', 'public'].includes(entrypoint), 'explicit ModuleSource entrypoint required');
  check([browserSha256, artifactPinsSha256].every(h => /^[a-f0-9]{64}$/.test(h)), 'explicit SHA256 arguments required');
  return {preflightOnly, cohort, entrypoint, browserSha256, artifactPinsSha256,
    ...Object.fromEntries(Object.entries({output, wasm, addon, fixtureRoot, metadata, verifier,
      packagePath, browserExecutable, artifactPins}).map(([key, path]) => [key, resolve(path)]))};
}
export function trackedReader() {
  const hashes = {};
  async function read(path) {
    path = resolve(path); check((await lstat(path)).isFile(), 'regular declared input required: ' + path);
    const bytes = await readFile(path), digest = hash(bytes);
    check(hashes[path] == null || hashes[path] === digest, 'input changed during acquisition: ' + path);
    hashes[path] = digest; return bytes;
  }
  return {hashes, read, async changed() {
    const changed = [];
    for (const [path, digest] of Object.entries(hashes)) {
      try { if (!(await lstat(path)).isFile() || hash(await readFile(path)) !== digest) changed.push(path); }
      catch { changed.push(path); }
    }
    return changed;
  }};
}
// Reconstruction is bounded to the selected original helper inputs; this is not a URI resolver.
export function validateHostFacts(bundle, authored, metadata, cohort) {
  check(bundle.schema === 1 && bundle.javaApiCalls === 212 && bundle.javaOrderedEvents === 2248
    && Array.isArray(bundle.fixtures) && bundle.fixtures.length === 36, 'exact original212 fixture accounting');
  check(metadata.schema === 2 && metadata.fixtureManifestSha256 === PINS.fixture
    && metadata.fixtureVerifierSha256 === PINS.verifier && metadata.profiles.length === 9, 'sidecar fixture/source identity');
  const byId = new Map();
  for (const profile of metadata.profiles) {
    check(!byId.has(profile.id), 'duplicate Host profile'); byId.set(profile.id, profile);
    const fixture = bundle.fixtures.find(f => f.id === profile.id), c = authored.find(c => c.id === profile.id);
    check(fixture && c, 'unknown Host profile');
    check(profile.customModules.length === fixture.moduleProviders.length, 'custom provider coverage differs');
    const uris = new Set();
    for (let i = 0; i < fixture.moduleProviders.length; i++) {
      const original = fixture.moduleProviders[i], actual = profile.customModules[i], uri = original.selectedUri;
      check(uri === 'pkl:base' || /^memo:\/\/?.+\.pkl$/.test(uri), 'unimplemented bounded inferred-name input');
      const inferred = uri === 'pkl:base' ? 'base' : uri.slice(uri.lastIndexOf('/') + 1, -4);
      for (const [name, value] of Object.entries(original)) check(same(actual[name], value), 'custom provider fact changed: ' + name);
      check(actual.sourceUri === uri && actual.inferredName === inferred && actual.fragmentPaths === false
        && actual.local === true && actual.hierarchical === true && actual.isGlobbable === false,
      'source-derived custom URI/name/capability mismatch');
      check(!uris.has(actual.requestedUri), 'duplicate custom provider'); uris.add(actual.requestedUri);
    }
    const inputs = c.inputPaths.filter(p => p.endsWith('.pkl'));
    check(profile.fileModules.length === inputs.length, 'file binding coverage differs');
    for (let i = 0; i < inputs.length; i++) {
      const rel = inputs[i], actual = profile.fileModules[i], uri = pathToFileURL(inside(c.workspace, rel)).href;
      check(actual.requestedUri === uri && actual.selectedUri === uri && actual.resolvedUri === uri
        && actual.sourceUri === uri && actual.originalAbsolutePath === inside(c.workspace, rel)
        && actual.sourcePath === `original/workspace/${c.id}/${rel}`
        && actual.inferredName === rel.slice(rel.lastIndexOf('/') + 1, -4)
        && actual.local === true && actual.hierarchical === true && actual.fragmentPaths === false
        && actual.isGlobbable === true && actual.isCached === true,
      'source-derived file URI/name/capability mismatch');
    }
  }
  check(new Set(bundle.fixtures.map(f => f.id)).size === byId.size, 'missing Host profile');
  check(['spi132', 'factory80', 'all212'].includes(cohort), 'explicit cohort required for Host facts');
  const selected = bundle.fixtures.filter(f => cohort === 'all212' || f.cohort === cohort);
  const expected = {spi132: [20, 132], factory80: [16, 80], all212: [36, 212]}[cohort];
  check(selected.length === expected[0] && selected.reduce((n, f) => n + f.expected.length, 0) === expected[1],
    'exact selected cohort accounting');
  return {profiles: byId, selected, unexecuted: bundle.fixtures.filter(f => !selected.includes(f))};
}
export function validateFactoryFacts(bundle, metadata, helperSource) {
  const rule = 'if(uri.getScheme().equals("throwing"))throw new IOException("stage48-factory-create");';
  const proof = metadata.factoryFailureProof;
  check(proof?.sourcePath === 'original/AdmissionProbe.java'
    && proof.sha256 === '7509222bebfbbea487ae20548671d827a68bff364a7f10e5df30bb9dcd6a6346'
    && proof.exactRuleUtf8Hex === Buffer.from(rule).toString('hex')
    && helperSource.split(rule).length === 2 && helperSource.indexOf(rule) < helperSource.indexOf('Plan p=plans.get(uri);'),
  'authenticated factory IOException rule');
  for (const profile of metadata.profiles) {
    check(same(profile.factoryFailures, [{scheme: 'throwing', errorKind: 'io', message: 'stage48-factory-create'}]), 'factory failure input facts changed');
    const fixtures = bundle.fixtures.filter(f => f.id === profile.id), uriSet = new Set();
    for (const f of fixtures) {
      const literals = [...f.source.matchAll(/\bimport\s*\(?\s*"([^"\n]*)"/g)].map(m => m[1]);
      const events = f.orderedEvents.map(e => Buffer.from(e.utf8Hex, 'hex').toString('utf8'));
      for (const uri of literals.filter(uri => uri.startsWith('noLoader:'))) {
        const starts = events.flatMap((e, i) => e === 'factory-custom ' + uri ? [i] : []);
        check(starts.length > 0 && profile.customModules.length === 0, 'NoLoader source and actual custom registry miss');
        for (const start of starts) {
          const next = events.findIndex((e, i) => i > start && e.startsWith('factory-custom '));
          const attempt = events.slice(start, next < 0 ? undefined : next), indices = [];
          for (const e of attempt) {
            const match = /^factory-(\d+) (.*)$/.exec(e);
            if (match && match[2] === uri) indices.push(Number(match[1]));
            check(!e.startsWith('factory-hit-') || !e.includes(' ' + uri + ' '), 'NoLoader default registry unexpectedly returned key');
            check(e !== 'factory-custom-hit ' + uri, 'NoLoader custom registry unexpectedly returned key');
          }
          check(indices.length > 0 && indices.every((n, i) => n === i), 'complete actual empty factory attempt');
        }
        uriSet.add(uri);
      }
    }
    check(same(profile.noLoaderUris, [...uriSet].sort()), 'exact source/trace-derived NoLoader URI set');
  }
}
export function auditRequests(actual) {
  const failures = [], tokens = new Set(), ids = new Set(); let previous = -1n;
  for (const row of actual.requests ?? []) {
    const id = row.module?.id;
    if (typeof id !== 'string' || !/^(0|[1-9][0-9]*)$/.test(id) || BigInt(id) > 18446744073709551615n
      || ids.has(id) || BigInt(id) <= previous) failures.push('request-id');
    else { ids.add(id); previous = BigInt(id); }
    if (typeof row.token !== 'string' || !row.token || tokens.has(row.token)) failures.push('request-token');
    tokens.add(row.token);
    if (!Number.isSafeInteger(row.callIndex) || row.callIndex < 0 || row.callIndex >= actual.calls.length)
      failures.push('request-api-index');
  }
  return {passed: failures.length === 0, failures};
}
// Validate the recorded diagnostic stream, not a fabricated Java callback/counter stream.
export function auditObservations(actual, fixture) {
  const failures = [], stream = actual.observations;
  if (!Array.isArray(stream)) return {passed: false, failures: ['missing-observations']};
  let constructed = false, constructorPending = false, active = null, activeMethod = null, started = false;
  let pending = null, host = null, gc = false, closing = false, closed = false, results = 0;
  const requests = [], operations = [], counts = {};
  for (const [index, event] of stream.entries()) {
    counts[event.kind] = (counts[event.kind] ?? 0) + 1;
    if (event.sequence !== index || !Number.isSafeInteger(event.callIndex)
      || event.callIndex < 0 || event.callIndex > actual.calls.length) failures.push('sequence/index');
    switch (event.kind) {
      case 'constructor-call':
        if (index !== 0 || constructorPending || constructed || event.callIndex !== 0
          || event.method !== (actual.profile === 'public' ? 'moduleEvaluator' : 'forExpressions')) failures.push('constructor-order');
        constructorPending = true; break;
      case 'constructor-return':
        if (!constructorPending) failures.push('constructor-pair');
        constructorPending = false; constructed = true; break;
      case 'constructor-throw':
        if (!constructorPending || !actual.constructorError) failures.push('constructor-throw');
        constructorPending = false; break;
      case 'api-call': {
        if (!constructed || active != null || closing || closed || event.callIndex !== results) failures.push('api-order');
        const publicEntry = actual.profile === 'public';
        const args = publicEntry ? [fixture.uri, null, fixture.expressions[results], 0] : [results, 0];
        if (event.method !== (publicEntry ? 'evaluateModuleSource' : 'startPath') || !same(event.args, args)) failures.push('api-input');
        active = results; activeMethod = event.method; started = false; operations.push({kind: 'evaluate'}); break;
      }
      case 'api-started':
        if (active == null || started || event.callIndex !== active || event.method !== activeMethod) failures.push('api-start'); started = true; break;
      case 'module-request':
        if (active == null || !started || pending || host || event.callIndex !== active) failures.push('request-order');
        pending = event.token; requests.push({callIndex: event.callIndex, token: event.token, module: event.module}); break;
      case 'host-call':
        if (!pending || host || event.args?.[0] !== pending || event.callIndex !== active) failures.push('completion-token/order');
        host = event.method; break;
      case 'host-return':
        if (!host || event.method !== host || event.callIndex !== active || typeof event.returnedUndefined !== 'boolean'
          || (event.returnedUndefined && event.value !== null)) failures.push('completion-return');
        if (host === 'completeDefaultModule') {
          if (event.returnedUndefined || typeof event.value !== 'boolean') failures.push('default-return-contract');
          if (event.value === true) pending = null;
        } else pending = null;
        host = null; break;
      case 'host-throw':
        if (event.method !== host || !actual.acquisitionError) failures.push('completion-throw'); host = null; break;
      case 'api-result':
        if (active !== results || !started || pending || host || event.callIndex !== results
          || !same(event.call, actual.calls[results])) failures.push('api-result');
        results++; active = null; activeMethod = null; started = false; break;
      case 'explicit-gc-call':
        if (active != null || gc || event.callIndex !== results) failures.push('gc-order'); gc = true; operations.push({kind: 'gc'}); break;
      case 'explicit-gc-return':
        if (!gc) failures.push('gc-pair'); gc = false; break;
      case 'cache-config':
        if (active != null || event.callIndex !== results || typeof event.cached !== 'boolean') failures.push('cache-order');
        operations.push({kind: 'cache', cached: event.cached}); break;
      case 'acquisition-error':
        if (!actual.acquisitionError) failures.push('unreported-acquisition-error');
        active = null; pending = null; host = null; gc = false; break;
      case 'close-call':
        if (!constructed || closing || closed || active != null || host || gc || event.callIndex !== results) failures.push('close-order'); closing = true; break;
      case 'close-return':
        if (!closing || event.terminalSlots !== actual.architecture.terminalSlots) failures.push('close-pair');
        closing = false; closed = true; break;
      case 'close-throw':
        if (!closing || !actual.acquisitionError) failures.push('close-throw'); closing = false; closed = true; break;
      default: failures.push('unknown-observation-kind');
    }
  }
  const planned = fixture.operations.map(op => op.kind === 'cache'
    ? {kind: 'cache', cached: op.value === 'on'} : {kind: op.kind});
  if (!same(requests, actual.requests) || results !== actual.calls.length) failures.push('raw-stream-join');
  if (!actual.constructorError && !actual.acquisitionError && (!same(operations, planned) || !closed || pending || host || active != null)) failures.push('incomplete-stream');
  return {passed: failures.length === 0, failures, diagnosticCounts: counts,
    scope: 'Actual selected observation consistency only; not original Java2248 event/counter parity.'};
}
export async function serveAsset(request, response, assets, read, ledger) {
  let pathname = request.url;
  try {
    pathname = new URL(request.url, 'http://127.0.0.1').pathname;
    if (pathname === '/') { response.end('<!doctype html><title>ModuleKey verification</title>'); return; }
    const path = assets.get(pathname);
    if (!path) { response.writeHead(404); response.end(); ledger.push({path: pathname, status: 404}); return; }
    const bytes = await read(path); // Hash the actual served copy before committing HTTP headers.
    response.writeHead(200, {'Content-Type': extname(path) === '.wasm' ? 'application/wasm' : 'text/javascript'});
    response.end(bytes); ledger.push({path: pathname, file: path, sha256: hash(bytes), bytes: bytes.length, status: 200});
  } catch (error) {
    ledger.push({path: pathname, error: String(error.message ?? error), status: 404});
    if (!response.headersSent) { response.writeHead(404); response.end(); } else response.destroy();
  }
}
async function verifyFixture(args, output, phase, read) {
  const identify = spawnSync('python3', ['-I', '-c', 'import sys;print(sys.executable)'], {encoding: 'utf8'});
  check(identify.status === 0 && identify.stdout.endsWith('\n'), 'Python executable identification failed');
  const python = await realpath(identify.stdout.slice(0, -1)); await read(python);
  const command = [python, '-I', '-B', args.verifier, '--fixtures', args.fixtureRoot];
  const startedAt = new Date().toISOString();
  const result = spawnSync(command[0], command.slice(1), {encoding: null,
    env: {...process.env, PYTHONDONTWRITEBYTECODE: '1'}});
  await writeFile(resolve(output, `${phase}.stdout.bin`), result.stdout ?? Buffer.alloc(0));
  await writeFile(resolve(output, `${phase}.stderr.bin`), result.stderr ?? Buffer.alloc(0));
  const record = {command, startedAt, finishedAt: new Date().toISOString(), pid: result.pid ?? null,
    parentPid: process.pid, exitCode: result.status, signal: result.signal, error: result.error?.message ?? null};
  await writeFile(resolve(output, `${phase}.process.json`), json(record));
  check(result.status === 0 && !result.signal && !result.error, 'original Java fixture verifier failed'); return record;
}
export async function main(argv) {
  const args = parseArguments(argv), tracker = trackedReader(), {read, hashes} = tracker;
  await mkdir(args.output, {recursive: false});
  const report = {schema: 1, startedAt: new Date().toISOString(), argv, cwd: process.cwd(),
    pid: process.pid, parentPid: process.ppid, preflightOnly: args.preflightOnly,
    node: process.version, nodeExecutable: process.execPath,
    host: {platform: os.platform(), release: os.release(), architecture: os.arch(), hostname: os.hostname()},
    artifactAndInputHashes: hashes, targets: {}, infrastructureErrors: [], browserErrors: [], httpAssets: [],
    fullCompatibilityPassed: false, fullLifecyclePassed: false,
    entrypoint: args.entrypoint,
    fullLifecycleGate: {passed: false, comparison: 'Full Java ordered events and counter bytes are not compared by this driver.',
      execution: args.entrypoint === 'public' ? 'Public ModuleSource operations selected; actual support and completion are recorded per session.' : 'Provided primary text; public primary/repl entrypoint is unexecuted.'},
    scope: 'Explicit ' + args.cohort + ' cohort. All212 original Java API and2248event ledger retained; unselected APIs listed separately. Full public event/counter equality remains failed. Configured file tables are a bounded virtual Host, not actual File secure-path callback parity.'};
  let browser, server;
  try {
    check(hash(await read(args.verifier)) === PINS.verifier, 'literal fixture verifier source pin');
    check(hash(await read(resolve(args.fixtureRoot, 'manifest.json'))) === PINS.fixture, 'literal fixture manifest pin');
    check(hash(await read(args.metadata)) === PINS.sidecar, 'literal Host sidecar pin');
    report.preflight = await verifyFixture(args, args.output, 'preflight', read);
    const manifest = JSON.parse(await read(resolve(args.fixtureRoot, 'manifest.json')));
    for (const [rel, row] of Object.entries(manifest.files)) {
      const b = await read(inside(args.fixtureRoot, rel)); check(hash(b) === row.sha256 && b.length === row.bytes, 'fixture payload hash');
    }
    const bundleBytes = await read(resolve(args.fixtureRoot, 'fixtures.json')), bundle = JSON.parse(bundleBytes);
    const metadataBytes = await read(args.metadata), metadata = JSON.parse(metadataBytes);
    check(same(Object.keys(metadata.sourceProof).sort(), Object.keys(SOURCE_PINS).sort()), 'exact source proof coverage');
    for (const [rel, digest] of Object.entries(SOURCE_PINS)) {
      const proof = metadata.sourceProof[rel]; check(proof.sha256 === digest && proof.copySha256 === digest, 'literal source proof pin');
      check(hash(await read(inside(dirname(args.metadata), proof.copy))) === digest, 'copied pinned source proof');
    }
    const authored = JSON.parse(await read(resolve(args.fixtureRoot, 'original/cases.json')));
    const helperSource = await read(resolve(args.fixtureRoot, 'original/AdmissionProbe.java'));
    check(hash(helperSource) === '7509222bebfbbea487ae20548671d827a68bff364a7f10e5df30bb9dcd6a6346', 'pinned original factory observer source');
    validateFactoryFacts(bundle, metadata, helperSource.toString('utf8'));
    const {profiles, selected, unexecuted} = validateHostFacts(bundle, authored, metadata, args.cohort);
    for (const profile of profiles.values()) for (const file of profile.fileModules) {
      const b = await read(inside(args.fixtureRoot, file.sourcePath));
      check(hash(b) === file.sourceSha256 && b.toString('utf8') === file.sourceText, 'Host file source text/hash differs from original');
    }
    check(hash(await read(args.artifactPins)) === args.artifactPinsSha256, 'explicit artifact manifest pin');
    const artifactProof = JSON.parse(await read(args.artifactPins));
    check(artifactProof.schema === 1 && Object.keys(artifactProof.sourceBindings ?? {}).length > 0, 'declared frozen build/source binding required');
    for (const [path, digest] of Object.entries(artifactProof.files ?? {}))
      check(artifactProof.sourceBindings[path] == null || artifactProof.sourceBindings[path] === digest, 'conflicting artifact/source metadata hashes');
    const required = [args.addon, resolve(args.wasm, 'pkl_wasm.js'), resolve(args.wasm, 'pkl_wasm_bg.wasm')];
    for (const path of required) check(/^[a-f0-9]{64}$/.test(artifactProof.files?.[path]), 'missing explicit binding artifact hash');
    for (const [path, digest] of Object.entries({...artifactProof.files, ...artifactProof.sourceBindings})) {
      check(isAbsolute(path) && /^[a-f0-9]{64}$/.test(digest) && hash(await read(path)) === digest, 'frozen artifact/source metadata hash');
    }
    check(hash(await read(args.browserExecutable)) === args.browserSha256, 'explicit browser executable pin');
    const script = fileURLToPath(import.meta.url), helperNames = ['modulekey-session.mjs', 'resource-session.mjs', 'module-policy-session.mjs'];
    for (const path of [script, process.execPath, args.packagePath, resolve(dirname(args.packagePath), 'package-lock.json')]) await read(path);
    const assets = new Map([['/pkl_wasm.js', required[1]], ['/pkl_wasm_bg.wasm', required[2]]]);
    for (const name of helperNames) {
      const bytes = await read(resolve(dirname(script), name)), copy = resolve(args.output, name);
      await writeFile(copy, bytes); await read(copy); assets.set('/' + name, copy);
    }
    await writeFile(resolve(args.output, 'fixtures.json'), bundleBytes);
    await writeFile(resolve(args.output, 'host-metadata.json'), metadataBytes);
    report.selectedApiCallsPerTarget = selected.reduce((n, f) => n + f.expected.length, 0); report.javaApiCalls = 212; report.javaOrderedEvents = 2248;
    report.unexecuted = unexecuted.map(f => ({id: f.id, repeat: f.repeat, session: f.session, apiCalls: f.expected.length, reason: 'outside explicit ' + args.cohort + ' cohort'}));
    report.artifactProof = artifactProof; report.browserExecutable = args.browserExecutable;
    report.browserExecutableSha256 = args.browserSha256; report.browserOsPid = null;
    report.browserOsPidQualification = 'Playwright launch does not expose the browser OS PID; exact executablePath/hash and browser version are recorded.';
    const pkg = JSON.parse(await read(args.packagePath));
    const lock = JSON.parse(await read(resolve(dirname(args.packagePath), 'package-lock.json'))), pin = pkg.devDependencies.playwright;
    check(/^\d+\.\d+\.\d+$/.test(pin) && lock.packages[''].devDependencies.playwright === pin
      && lock.packages['node_modules/playwright'].version === pin
      && lock.packages['node_modules/playwright-core'].version === pin, 'pinned browser package/lock required');
    report.playwrightPin = pin;
    if (!args.preflightOnly) {
      const require = createRequire(args.packagePath);
      const {chromium} = require('playwright');
      check(require('playwright/package.json').version === pin && require('playwright-core/package.json').version === pin, 'pinned browser runtime required');
      for (const path of Object.keys(require.cache)) await read(path);
      const save = async (target, rows) => {
        await writeFile(resolve(args.output, target + '.raw.json'), json(rows));
        check(rows.length === selected.length && rows.every((r, i) => same([r.id, r.repeat, r.session], [selected[i].id, selected[i].repeat, selected[i].session])), 'missing or reordered actual sessions');
        const sessions = [];
        for (let i = 0; i < rows.length; i++) {
          const row = rows[i], fixture = selected[i], dir = resolve(args.output, target, String(i)); await mkdir(dir, {recursive: true});
          await writeFile(resolve(dir, 'actual.json'), json(row)); await writeFile(resolve(dir, 'expected.json'), json(fixture.expected));
          await writeFile(resolve(dir, 'observations.json'), json(row.observations ?? null));
          for (const call of row.calls) {
            await writeFile(resolve(dir, `${call.index}.status.bin`), call.status + '\n');
            for (const channel of ['stdout', 'stderr']) {
              const units = call[channel + 'Units'], bytes = Buffer.alloc(units.length * 2); units.forEach((u, j) => bytes.writeUInt16BE(u, 2 * j));
              await writeFile(resolve(dir, `${call.index}.${channel}.utf16be.bin`), bytes);
              await writeFile(resolve(dir, `${call.index}.${channel}.java-utf8.bin`), Buffer.from(javaUtf8Bytes(units)));
            }
          }
          const transport = auditRequests(row), observations = auditObservations(row, fixture);
          const comparison = {...compareModuleKeyCase(fixture, row), transport, observations};
          comparison.rawApiPassed &&= transport.passed && observations.passed; await writeFile(resolve(dir, 'comparison.json'), json(comparison)); sessions.push(comparison);
        }
        return {complete: true, sessions, actualApiCalls: rows.reduce((n, r) => n + r.calls.length, 0), rawApiPassed: sessions.every(s => s.rawApiPassed), fullLifecyclePassed: false};
      };
      try { const api = require(args.addon); report.targets.node = await save('node', selected.map(f => runModuleKeyCase(api, f, profiles.get(f.id), 7, {entrypoint: args.entrypoint}))); }
      catch (error) { report.infrastructureErrors.push({target: 'node', message: error.message, stack: error.stack}); }
      server = createServer((req, res) => { void serveAsset(req, res, assets, read, report.httpAssets); });
      await new Promise(done => server.listen(0, '127.0.0.1', done));
      const origin = `http://127.0.0.1:${server.address().port}`, missing = await fetch(origin + '/__missing__.js');
      const body = Buffer.from(await missing.arrayBuffer()); report.serverMissingAssetProbe = {status: missing.status, bodyHex: body.toString('hex')};
      check(missing.status === 404 && body.length === 0, 'missing HTTP asset empty404 required');
      report.browserLaunchOptions = {headless: true, executablePath: args.browserExecutable};
      browser = await chromium.launch(report.browserLaunchOptions); report.browser = browser.version();
      const page = await browser.newPage(); page.on('pageerror', error => report.browserErrors.push({message: error.message, stack: error.stack}));
      await page.goto(origin);
      const rows = await page.evaluate(async ({fixtures, profiles, entrypoint}) => {
        const api = await import('/pkl_wasm.js'); await api.default(); const {runModuleKeyCase} = await import('/modulekey-session.mjs');
        return fixtures.map(f => runModuleKeyCase(api, f, profiles.find(p => p.id === f.id), 7, {entrypoint}));
      }, {fixtures: selected, profiles: [...profiles.values()], entrypoint: args.entrypoint});
      report.targets.browser = await save('browser', rows);
      for (const path of Object.keys(require.cache)) await read(path);
    }
    report.postflight = await verifyFixture(args, args.output, 'postflight', read);
  } catch (error) { report.infrastructureErrors.push({target: 'driver', message: error.message, stack: error.stack}); }
  finally {
    if (browser) try { await browser.close(); } catch (error) { report.infrastructureErrors.push({target: 'browser-close', message: error.message}); }
    if (server) await new Promise(done => server.close(done));
  }
  report.changedInputs = await tracker.changed(); report.finishedAt = new Date().toISOString();
  report.acquisitionIntegrityPassed = report.infrastructureErrors.length === 0 && report.browserErrors.length === 0 && report.changedInputs.length === 0;
  report.rawApiPassed = !args.preflightOnly && report.acquisitionIntegrityPassed && ['node', 'browser'].every(t => report.targets[t]?.rawApiPassed);
  report.selectedContractPassed = false; // The original full public lifecycle gate is intentionally not implemented by this profile.
  await writeFile(resolve(args.output, 'execution.json'), json(report));
  return {report, exitCode: args.preflightOnly ? (report.acquisitionIntegrityPassed ? 0 : 1) : 1};
}
if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).then(({report, exitCode}) => {
    console.log(json({rawApiPassed: report.rawApiPassed, acquisitionIntegrityPassed: report.acquisitionIntegrityPassed,
      fullLifecyclePassed: report.fullLifecyclePassed, selectedContractPassed: report.selectedContractPassed})); process.exitCode = exitCode;
  }).catch(error => { console.error(error.stack); process.exitCode = 1; });
}
