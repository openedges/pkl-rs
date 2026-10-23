// Actual public ModuleSource comparison against sealed Java01–07 controls.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, realpath} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve, dirname, isAbsolute, sep} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {trackedReader, serveAsset, auditRequests} from './compare-modulekey.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';
export {trackedReader, serveAsset, auditRequests};

export const PINS = Object.freeze({
  fixture: 'eda9449570132c0f76ebf2ea67071a94b6b3a53b29fb9a49dfb982e66ad18bb2',
  verifier: '30131893b23f908aebdcf54eec1ac37dd6666a3054ee5e017e77e935068beaaf',
});
const json = value => JSON.stringify(value, null, 2) + '\n';
export const hash = value => createHash('sha256').update(value).digest('hex');
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
function check(ok, message) { if (!ok) throw Error(message); }
function inside(root, rel) {
  check(typeof rel === 'string' && !isAbsolute(rel) && !rel.split(/[\\/]/).includes('..'), 'unsafe declared relative path');
  const path = resolve(root, rel); check(path.startsWith(resolve(root) + sep), 'relative path escapes root'); return path;
}
const expectedCohorts = {java01: [68, 444], java02: [36, 188], java03: [4, 40],
  java04: [8, 104], java05: [8, 48], java06: [8, 80], java07: [8, 40], all944: [140, 944]};

export function parseArguments(argv) {
  const values = [...argv], preflightOnly = values[0] === '--preflight-only';
  if (preflightOnly) values.shift();
  check(values.length === 13,
    'usage: compare-modulekey-extended.mjs [--preflight-only] OUTPUT WASM_DIR ADDON FIXTURES_DIR HOST_METADATA VERIFIER PINNED_PACKAGE_JSON BROWSER_EXECUTABLE BROWSER_SHA256 ARTIFACT_PINS_JSON ARTIFACT_PINS_SHA256 COHORT public');
  const [output, wasm, addon, fixtureRoot, metadata, verifier, packagePath, browserExecutable,
    browserSha256, artifactPins, artifactPinsSha256, cohort, entrypoint] = values;
  check(Object.hasOwn(expectedCohorts, cohort), 'explicit cohort java01..java07|all944 required');
  check(entrypoint === 'public', 'extended944 requires actual public ModuleSource entrypoint');
  check([browserSha256, artifactPinsSha256].every(h => /^[a-f0-9]{64}$/.test(h)), 'explicit SHA256 arguments required');
  return {preflightOnly, cohort, entrypoint, browserSha256, artifactPinsSha256,
    ...Object.fromEntries(Object.entries({output, wasm, addon, fixtureRoot, metadata, verifier,
      packagePath, browserExecutable, artifactPins}).map(([k, v]) => [k, resolve(v)]))};
}

export function operationArguments(profile, operation) {
  check(operation?.kind === 'evaluate' && typeof operation.expression === 'string', 'evaluate operation required');
  const source = profile.sourceProfiles?.[operation.sourceProfileId];
  check(source && typeof source.uri === 'string' && ['uri', 'contents', 'repl'].includes(source.kind), 'declared public source profile required');
  let contents = null;
  if (source.kind !== 'uri') {
    const value = profile.sources?.[source.sourceId];
    check(value?.kind === 'text' && typeof value.text === 'string', 'authored contents must be preloaded text');
    contents = value.text;
  } else check(source.contents === null, 'URI-only profile requires explicit null contents');
  return [source.uri, contents, operation.expression, 0];
}

export function validateExtendedFacts(bundle, metadata, cohort) {
  check(bundle?.schema === 1 && bundle.profile === 'modulekey-extended944-v1'
    && bundle.javaApiCalls === 944 && bundle.javaOrderedEvents === 10124
    && bundle.fixtures?.length === 140 && same(bundle.scopeLedger,
      {ordinary: 836, 'intent-not-reached-diagnostic': 100, 'internal-boundary': 8}), 'exact original944 scope');
  check(metadata?.schema === 1 && metadata.profile === bundle.profile && metadata.profiles?.length === 35, 'exact extended Host profiles');
  const profiles = new Map();
  for (const profile of metadata.profiles) {
    check(typeof profile.id === 'string' && !profiles.has(profile.id), 'unique provider profile');
    check(profile.initial?.cached === true && profile.initial.customFactory === true
      && profile.initial.sourceState === 'good' && profile.initial.policy === 'allow', 'authored initial profile');
    check(profile.sources && profile.sourceProfiles && Array.isArray(profile.customModules)
      && Array.isArray(profile.fileModules), 'preloaded source/key tables required');
    const requested = new Set();
    for (const plan of [...profile.customModules, ...profile.fileModules]) {
      check(['requestedUri', 'selectedUri', 'resolvedUri', 'sourceUri', 'sourceId', 'inferredName']
        .every(k => typeof plan[k] === 'string') && ['host', 'synthetic'].includes(plan.keyKind)
        && ['true', 'false', 'dynamic'].includes(plan.cacheMode)
        && ['fixed', 'state-at-load', 'state-at-factory'].includes(plan.sourceMode)
        && typeof plan.fragmentPaths === 'boolean' && !requested.has(plan.requestedUri), 'configured key table structure');
      requested.add(plan.requestedUri);
      if (plan.sourceMode === 'fixed') check(profile.sources[plan.sourceId]?.kind === 'text', 'fixed source lookup');
      if (plan.sourceMode === 'state-at-factory') check(plan.keyKind === 'synthetic', 'actual Synthetic factory source snapshot');
    }
    profiles.set(profile.id, profile);
  }
  const identities = new Set(), scopes = {ordinary: 0, 'intent-not-reached-diagnostic': 0, 'internal-boundary': 0};
  let total = 0, events = 0;
  for (const fixture of bundle.fixtures) {
    const profile = profiles.get(fixture.providerProfileId), key = [fixture.id, fixture.repeat, fixture.session].join('/');
    check(profile && fixture.id === profile.id && !identities.has(key) && [0, 1].includes(fixture.repeat)
      && [0, 1].includes(fixture.session) && Object.hasOwn(expectedCohorts, fixture.cohort)
      && fixture.cohort !== 'all944', 'actual Java session identity');
    identities.add(key);
    const evaluated = [];
    for (const operation of fixture.operations) {
      if (operation.kind === 'evaluate') { operationArguments(profile, operation); evaluated.push(operation); }
      else if (operation.kind === 'cache') check(['on', 'off'].includes(operation.value), 'authored cache state');
      else if (operation.kind === 'factory') check(['custom', 'builtin'].includes(operation.value), 'authored factory state');
      else if (operation.kind === 'source') check(profile.sources[operation.value], 'declared source state');
      else if (operation.kind === 'policy') check(operation.value === 'allow'
        || profile.policyRules.some(rule => rule.mode === operation.value), 'declared policy state');
      else check(operation.kind === 'gc' && (!Object.hasOwn(operation, 'value') || operation.value === ''), 'zero-argument GC operation required');
    }
    check(same(evaluated.map(o => o.expression), fixture.expressions)
      && evaluated.length === fixture.expected.length, 'operation/expected API accounting');
    for (const [i, expected] of fixture.expected.entries()) {
      check(expected.sourceProfileId === evaluated[i].sourceProfileId && Object.hasOwn(scopes, expected.scope), 'expected source/scope join');
      check(expected.scope !== 'internal-boundary' || expected.errorClass === 'org.pkl.core.PklBugException', 'internal oracle class retained');
      scopes[expected.scope]++;
    }
    total += fixture.expected.length; events += fixture.orderedJavaEvents.length;
  }
  check(total === 944 && events === 10124 && same(scopes, bundle.scopeLedger), 'complete original raw ledger');
  check(Object.hasOwn(expectedCohorts, cohort), 'explicit cohort');
  const selected = bundle.fixtures.filter(f => cohort === 'all944' || f.cohort === cohort);
  const [sessions, apis] = expectedCohorts[cohort];
  check(selected.length === sessions && selected.reduce((n, f) => n + f.expected.length, 0) === apis, 'exact selected cohort accounting');
  return {profiles, selected, unexecuted: bundle.fixtures.filter(f => !selected.includes(f))};
}

export function compareRawRows(fixture, actual) {
  const calls = actual.calls ?? [], rows = [];
  for (const [i, expected] of fixture.expected.entries()) {
    const call = calls[i];
    const match = !!call && call.index === i && call.status === expected.status && call.statusRawHex === expected.statusRawHex
      && call.sourceProfileId === expected.sourceProfileId && ['stdout', 'stderr'].every(channel =>
      same(call[channel + 'Units'], expected[channel + 'Units'])
      && Buffer.from(javaUtf8Bytes(call[channel + 'Units'])).toString('hex') === expected[channel + 'Hex']);
    rows.push({index: i, scope: expected.scope, rawBytesMatch: match});
  }
  return {rows, callCountMatches: calls.length === fixture.expected.length,
    allRawMatch: calls.length === fixture.expected.length && rows.every(r => r.rawBytesMatch),
    matched: rows.filter(r => r.rawBytesMatch).length,
    mismatchOrMissing: rows.filter(r => !r.rawBytesMatch).length,
    ordinaryMatched: rows.filter(r => r.scope === 'ordinary' && r.rawBytesMatch).length,
    diagnosticMatched: rows.filter(r => r.scope === 'intent-not-reached-diagnostic' && r.rawBytesMatch).length,
    internalBoundaryMatched: rows.filter(r => r.scope === 'internal-boundary' && r.rawBytesMatch).length};
}

export function auditExtendedObservations(actual, fixture, profile) {
  const failures = [], stream = actual.observations;
  if (!Array.isArray(stream)) return {passed: false, failures: ['missing-observations']};
  const evaluated = fixture.operations.filter(op => op.kind === 'evaluate');
  let constructed = false, constructing = false, active = null, started = false;
  let pending = null, host = null, gc = false, closing = false, closed = false, results = 0;
  const requests = [], operations = [], counts = {};
  for (const [sequence, event] of stream.entries()) {
    counts[event.kind] = (counts[event.kind] ?? 0) + 1;
    if (event.sequence !== sequence || !Number.isSafeInteger(event.callIndex)
      || event.callIndex < 0 || event.callIndex > actual.calls.length) failures.push('sequence/index');
    switch (event.kind) {
      case 'constructor-call':
        if (sequence !== 0 || constructing || constructed || event.callIndex !== 0
          || event.method !== 'moduleEvaluator') failures.push('constructor-order'); constructing = true; break;
      case 'constructor-return': if (!constructing) failures.push('constructor-pair'); constructing = false; constructed = true; break;
      case 'constructor-throw': if (!constructing || !actual.constructorError) failures.push('constructor-throw'); constructing = false; break;
      case 'api-call':
        if (!constructed || active != null || closed || closing || event.callIndex !== results
          || event.method !== 'evaluateModuleSource' || !evaluated[results]
          || !same(event.args, operationArguments(profile, evaluated[results]))) failures.push('api-input/order');
        active = results; started = false; operations.push({kind: 'evaluate'}); break;
      case 'api-started': if (active == null || started || event.callIndex !== active
        || event.method !== 'evaluateModuleSource') failures.push('api-start'); started = true; break;
      case 'module-request':
        if (active == null || !started || pending || host || event.callIndex !== active) failures.push('request-order');
        pending = event.token; requests.push({callIndex: event.callIndex, token: event.token, module: event.module}); break;
      case 'host-call': if (!pending || host || event.args?.[0] !== pending || event.callIndex !== active) failures.push('completion-order'); host = event.method; break;
      case 'host-return':
        if (!host || event.method !== host || event.callIndex !== active || typeof event.returnedUndefined !== 'boolean'
          || (event.returnedUndefined && event.value !== null)) failures.push('completion-return');
        if (host === 'completeDefaultModule') {
          if (event.returnedUndefined || typeof event.value !== 'boolean') failures.push('default-return-contract');
          if (event.value === true) pending = null;
        } else pending = null;
        host = null; break;
      case 'host-throw': if (host !== event.method || !actual.acquisitionError) failures.push('completion-throw'); host = null; break;
      case 'api-result':
        if (active !== results || !started || pending || host || event.callIndex !== results
          || !same(event.call, actual.calls[results])) failures.push('api-result');
        results++; active = null; started = false; break;
      case 'explicit-gc-call': if (!constructed || closed || closing || active != null || gc || event.callIndex !== results) failures.push('gc-order'); gc = true; operations.push({kind: 'gc'}); break;
      case 'explicit-gc-return': if (!gc) failures.push('gc-pair'); gc = false; break;
      case 'cache-config':
      case 'source-config':
      case 'factory-config':
      case 'policy-config': {
        if (!constructed || closed || closing || active != null || gc || event.callIndex !== results) failures.push('configuration-order');
        const kind = {'cache-config': 'cache', 'source-config': 'source', 'factory-config': 'factory', 'policy-config': 'policy'}[event.kind];
        if (kind === 'cache' || kind === 'factory') {
          const value = kind === 'cache' ? event.cached : event.customFactory;
          if (typeof value !== 'boolean') failures.push('configuration-value');
          operations.push({kind, value: kind === 'cache' ? (value ? 'on' : 'off') : (value ? 'custom' : 'builtin')});
        } else {
          const value = kind === 'source' ? event.state : event.mode;
          if (typeof value !== 'string') failures.push('configuration-value'); operations.push({kind, value});
        }
        break;
      }
      case 'acquisition-error': if (!actual.acquisitionError) failures.push('unreported-acquisition-error'); active = null; pending = null; host = null; gc = false; break;
      case 'close-call': if (!constructed || closing || closed || active != null || host || gc || event.callIndex !== results) failures.push('close-order'); closing = true; break;
      case 'close-return': if (!closing || event.terminalSlots !== actual.architecture.terminalSlots) failures.push('close-pair'); closing = false; closed = true; break;
      case 'close-throw': if (!closing || !actual.acquisitionError) failures.push('close-throw'); closing = false; closed = true; break;
      case 'free-throw': if (!closed || event.callIndex !== results || !actual.acquisitionError) failures.push('unreported-free-failure'); break;
      default: failures.push('unknown-observation-kind');
    }
  }
  const planned = fixture.operations.map(op => {
    if (op.kind === 'evaluate') return {kind: 'evaluate'};
    if (op.kind === 'gc') {
      if (Object.hasOwn(op, 'value') && op.value !== '') failures.push('nonempty-gc-operand');
      return {kind: 'gc'};
    }
    return {...op};
  });
  if (!same(requests, actual.requests) || results !== actual.calls.length) failures.push('raw-stream-join');
  if (!actual.constructorError && !actual.acquisitionError
    && (!same(operations, planned) || !closed || pending || host || active != null)) failures.push('incomplete-stream');
  return {passed: failures.length === 0, failures, diagnosticCounts: counts,
    scope: 'Actual diagnostic stream consistency; not original Java10124 events/counter parity.'};
}

export function combineSessionComparison(fixture, actual, profile, helperComparison) {
  const raw = compareRawRows(fixture, actual), transport = auditRequests(actual);
  const observations = auditExtendedObservations(actual, fixture, profile);
  const cleanupPassed = actual.architecture?.terminalSlots === 0
    && actual.architecture.ownedResultsStable === true && actual.architecture.releasedHostKeys === true;
  const observationIntegrityPassed = transport.passed && observations.passed;
  const acquisitionIntegrityPassed = helperComparison.acquisitionIntegrityPassed === true
    && !actual.constructorError && !actual.acquisitionError && raw.callCountMatches
    && cleanupPassed && observationIntegrityPassed;
  return {...helperComparison, raw, transport, observations, cleanupPassed,
    helperAcquisitionIntegrityPassed: helperComparison.acquisitionIntegrityPassed,
    observationIntegrityPassed, acquisitionIntegrityPassed, rawApiPassed: raw.allRawMatch,
    ordinaryCompatibilityPassed: helperComparison.ordinaryCompatibilityPassed === true && acquisitionIntegrityPassed};
}

export function aggregateComparisons(sessions) {
  return {rawApiPassed: sessions.every(s => s.rawApiPassed),
    acquisitionIntegrityPassed: sessions.every(s => s.acquisitionIntegrityPassed),
    observationIntegrityPassed: sessions.every(s => s.observationIntegrityPassed),
    ordinaryCompatibilityPassed: sessions.every(s => s.ordinaryCompatibilityPassed),
    rawByteMatches: sessions.reduce((n, s) => n + s.raw.matched, 0),
    rawByteMismatchOrMissing: sessions.reduce((n, s) => n + s.raw.mismatchOrMissing, 0),
    intendedOrdinaryRawMatched: sessions.reduce((n, s) => n + s.raw.ordinaryMatched, 0),
    intendedOrdinaryCompatible: sessions.reduce((n, s) => n + (s.acquisitionIntegrityPassed
      ? (s.cases ?? []).filter(c => c.ordinaryCompatible).length : 0), 0),
    diagnosticOnlyRawMatched: sessions.reduce((n, s) => n + s.raw.diagnosticMatched, 0),
    internalBoundaryRawMatched: sessions.reduce((n, s) => n + s.raw.internalBoundaryMatched, 0),
    internalBoundaryCount: sessions.reduce((n, s) => n + s.raw.rows.filter(c => c.scope === 'internal-boundary').length, 0)};
}

async function verifyFixture(args, output, phase, read) {
  const identify = spawnSync('python3', ['-I', '-c', 'import sys;print(sys.executable)'], {encoding: 'utf8'});
  check(identify.status === 0 && identify.stdout.endsWith('\n'), 'Python executable identification failed');
  const python = await realpath(identify.stdout.slice(0, -1)); await read(python);
  const command = [python, '-I', '-B', args.verifier, '--fixtures', args.fixtureRoot];
  const startedAt = new Date().toISOString();
  const result = spawnSync(command[0], command.slice(1), {encoding: null, env: {...process.env, PYTHONDONTWRITEBYTECODE: '1'}});
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
    fullLifecycleGate: {passed: false, comparison: 'Full Java10124 ordered events and counter bytes are not compared by this driver.',
      execution: args.entrypoint === 'public' ? 'Public ModuleSource operations selected; actual support and completion are recorded per session.' : 'Provided primary text; public primary/repl entrypoint is unexecuted.'},
    scope: 'Explicit ' + args.cohort + ' cohort. All944 original Java API and10124event ledger retained; unselected APIs listed separately. Full public event/counter equality remains failed. Configured file tables are a bounded virtual Host, not actual File secure-path callback parity.'};
  let browser, server;
  try {
    check(hash(await read(args.verifier)) === PINS.verifier, 'literal fixture verifier source pin');
    check(hash(await read(resolve(args.fixtureRoot, 'manifest.json'))) === PINS.fixture, 'literal fixture manifest pin');
    check(hash(await read(args.metadata)) === hash(await read(resolve(args.fixtureRoot, 'host-metadata.json'))), 'Host metadata must be the authenticated fixture copy');
    report.preflight = await verifyFixture(args, args.output, 'preflight', read);
    const manifest = JSON.parse(await read(resolve(args.fixtureRoot, 'manifest.json')));
    for (const [rel, row] of Object.entries(manifest.files)) {
      const b = await read(inside(args.fixtureRoot, rel)); check(hash(b) === row.sha256 && b.length === row.bytes, 'fixture payload hash');
    }
    const bundleBytes = await read(resolve(args.fixtureRoot, 'fixtures.json')), bundle = JSON.parse(bundleBytes);
    const metadataBytes = await read(args.metadata), metadata = JSON.parse(metadataBytes);
    const {profiles, selected, unexecuted} = validateExtendedFacts(bundle, metadata, args.cohort);
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
    const script = fileURLToPath(import.meta.url), helperNames = ['modulekey-extended-session.mjs', 'resource-session.mjs', 'module-policy-session.mjs', 'modulekey-session.mjs', 'compare-modulekey.mjs'];
    for (const path of [script, process.execPath, args.packagePath, resolve(dirname(args.packagePath), 'package-lock.json')]) await read(path);
    const assets = new Map([['/pkl_wasm.js', required[1]], ['/pkl_wasm_bg.wasm', required[2]]]);
    for (const name of helperNames) {
      const bytes = await read(resolve(dirname(script), name)), copy = resolve(args.output, name);
      await writeFile(copy, bytes); await read(copy); assets.set('/' + name, copy);
    }
    await writeFile(resolve(args.output, 'fixtures.json'), bundleBytes);
    await writeFile(resolve(args.output, 'host-metadata.json'), metadataBytes);
    report.selectedApiCallsPerTarget = selected.reduce((n, f) => n + f.expected.length, 0); report.javaApiCalls = 944; report.javaOrderedEvents = 10124;
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
      const {runModuleKeyExtendedCase, compareModuleKeyExtendedCase} = await import(pathToFileURL(resolve(dirname(script), 'modulekey-extended-session.mjs')).href);
      const save = async (target, rows) => {
        await writeFile(resolve(args.output, target + '.raw.json'), json(rows));
        check(rows.length === selected.length && rows.every((r, i) => same([r.id, r.repeat, r.session], [selected[i].id, selected[i].repeat, selected[i].session])), 'missing or reordered actual sessions');
        const sessions = [];
        for (let i = 0; i < rows.length; i++) {
          const row = rows[i], fixture = selected[i], dir = resolve(args.output, target, String(i)); await mkdir(dir, {recursive: true});
          await writeFile(resolve(dir, 'actual.json'), json(row)); await writeFile(resolve(dir, 'expected.json'), json(fixture.expected));
          await writeFile(resolve(dir, 'observations.json'), json(row.observations ?? null));
          for (const call of row.calls) {
            check(typeof call.statusRawHex === 'string' && /^(?:[a-f0-9]{2})*$/.test(call.statusRawHex), 'actual raw status encoding');
            await writeFile(resolve(dir, `${call.index}.status.bin`), Buffer.from(call.statusRawHex, 'hex'));
            await writeFile(resolve(dir, `${call.index}.source-profile.bin`), call.sourceProfileId);
            for (const channel of ['stdout', 'stderr']) {
              const units = call[channel + 'Units'], bytes = Buffer.alloc(units.length * 2); units.forEach((u, j) => bytes.writeUInt16BE(u, 2 * j));
              await writeFile(resolve(dir, `${call.index}.${channel}.utf16be.bin`), bytes);
              await writeFile(resolve(dir, `${call.index}.${channel}.java-utf8.bin`), Buffer.from(javaUtf8Bytes(units)));
            }
          }
          const comparison = combineSessionComparison(fixture, row, profiles.get(fixture.providerProfileId),
            compareModuleKeyExtendedCase(fixture, row));
          await writeFile(resolve(dir, 'comparison.json'), json(comparison)); sessions.push(comparison);
        }
        return {complete: true, sessions, actualApiCalls: rows.reduce((n, r) => n + r.calls.length, 0),
          ...aggregateComparisons(sessions), fullLifecyclePassed: false};
      };
      try { const api = require(args.addon); report.targets.node = await save('node', selected.map(f => runModuleKeyExtendedCase(api, f, profiles.get(f.providerProfileId), 7))); }
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
        const api = await import('/pkl_wasm.js'); await api.default(); const {runModuleKeyExtendedCase} = await import('/modulekey-extended-session.mjs');
        return fixtures.map(f => runModuleKeyExtendedCase(api, f, profiles.find(p => p.id === f.providerProfileId), 7));
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
  report.toolIntegrityPassed = report.infrastructureErrors.length === 0 && report.browserErrors.length === 0 && report.changedInputs.length === 0;
  report.acquisitionIntegrityPassed = report.toolIntegrityPassed && (args.preflightOnly
    || ['node', 'browser'].every(t => report.targets[t]?.complete && report.targets[t]?.acquisitionIntegrityPassed));
  report.rawApiPassed = !args.preflightOnly && ['node', 'browser'].every(t => report.targets[t]?.complete && report.targets[t]?.rawApiPassed);
  report.ordinaryCompatibilityPassed = !args.preflightOnly && report.acquisitionIntegrityPassed
    && ['node', 'browser'].every(t => report.targets[t]?.ordinaryCompatibilityPassed);
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
