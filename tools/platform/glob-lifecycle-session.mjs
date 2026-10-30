// Actual binding/session checks. This is transport evidence, not Java parity.
import {runGlobFixture} from './glob-session.mjs';
import {validateGlobRequest, completeGlobReader, completeGlobBoolean, completeGlobElements, completeDefaultGlob} from './glob-transport.mjs';
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const assert = (condition, message) => { if (!condition) throw Error(message); };
function rejects(action) { let rejected = false; try { action(); } catch { rejected = true; } assert(rejected, 'invalid operation was admitted'); }
function next(session, now = 0) {
  for (let count = 0; count < 100_000; count++) {
    const event = session.poll(now, 1); session.collect();
    if (event.kind === 'yield') continue;
    if (event.kind === 'module-request') { assert(session.completeDefaultModule(event.token, now), 'unexpected module Host effect'); continue; }
    return event;
  }
  throw Error('glob lifecycle poll bound');
}
function make(api, timeout = null) {
  const session = api.SourceSession.forExpressions('marker = true\n', 'repl:text', 'globTransport',
    ['read*("custom:**").keys.toList().join("|")', 'read*("custom:inner*").keys.toList().join("|")'],
    'compact', false, false, false);
  session.startPath(0, 0, timeout);
  const event = next(session); validateGlobRequest(event);
  assert(event.glob.kind === 'select-resource', 'expected resource selection');
  return {session, event};
}
function listing(session, selected) {
  completeGlobReader(session, selected, 'custom', 'transport-reader');
  let event = next(session); assert(event.glob.kind === 'is-globbable', 'globbability order');
  completeGlobBoolean(session, event, true);
  event = next(session); assert(event.glob.kind === 'is-hierarchical', 'hierarchy order');
  completeGlobBoolean(session, event, false);
  event = next(session); assert(event.glob.kind === 'list-elements', 'listing order');
  return event;
}
// Codec observations use the exact production binding decoder and encoder.
// They do not establish URI acceptance or evaluator support for these names.
function codecContract(api, id) {
  const result = {id, evidenceKind: 'production-codec', executed: false, passed: false};
  try {
    const units = [[0x61], [0xd800], [0xdc00], [0xd83d, 0xde00], [0x61, 0, 0xdc00, 0x62]];
    const elements = units.map((name, index) => ({name: new Uint16Array(name), isDirectory: index % 2 === 1}));
    result.executed = true;
    const records = api.globTransportCodec(elements);
    assert(records.length === elements.length, 'codec record count');
    result.observation = records.map((record, index) => {
      const descriptor = validateGlobRequest(record);
      assert(descriptor.id === '18446744073709551615' && descriptor.readerId === '9007199254740993', 'u64 descriptor narrowed');
      assert(descriptor.kind === 'list-elements' && descriptor.uri === 'codec:**' && descriptor.readerKind === 'resource' && descriptor.readerScheme === 'codec' && descriptor.readerHandle === 'codec-reader', 'descriptor transfer changed');
      assert(record.token === `codec:${index}` && record.reason === (elements[index].isDirectory ? 'directory' : 'file'), 'typed element flag/order changed');
      assert(record.value instanceof Uint16Array && same(Array.from(record.value), units[index]), 'UTF16 completion/event units changed');
      return {glob: descriptor, token: record.token, reason: record.reason, units: Array.from(record.value)};
    });
    result.passed = true;
  } catch (error) { result.failure = String(error.message ?? error); }
  return result;
}

export function runGlobLifecycleContracts(api, moduleFixtureFiles) {
  const results = [];
  function run(id, body, timeout = null) {
    let state;
    const result = {id, executed: false, passed: false, observation: null, failure: null, cleanupFailure: null};
    try { state = make(api, timeout); result.executed = true; result.observation = body(state.session, state.event); result.passed = true; }
    catch (error) { result.failure = String(error.message ?? error); }
    finally {
      if (state) try { state.session.close(); state.session.collect(); result.terminalSlots = state.session.heapSlots(); assert(result.terminalSlots === 0, 'retained slots after close'); state.session.free?.(); }
      catch (error) { result.cleanupFailure = String(error.message ?? error); result.passed = false; }
    }
    results.push(result);
  }
  run('transport-00', (session, event) => structuredClone(validateGlobRequest(event)));
  // Successful high-ID transfer requires the separate actual-binding codec probe.
  results.push(codecContract(api, 'transport-01'));
  run('transport-02', (session, event) => {
    const id = event.glob.readerId; const current = listing(session, event);
    assert(current.glob.readerId === id && current.glob.readerScheme === 'custom' && current.glob.readerHandle === 'transport-reader', 'resource identity changed');
    return structuredClone(current.glob);
  });
  // This runs an actual retained raw-UTF16 import fixture. It does not forge
  // ModuleKey records; every descriptor comes from the production session.
  const moduleResult = {id: 'transport-03', evidenceKind: 'real-session', executed: false, passed: false};
  try {
    assert(moduleFixtureFiles, 'retained import fixture is required');
    moduleResult.executed = true;
    const capture = runGlobFixture(api.SourceSession, moduleFixtureFiles, 'production16384');
    assert(capture.complete, 'actual module fixture did not complete');
    const keys = new Map(); let globReaders = 0, rawLoads = 0;
    for (const effect of capture.effects) {
      if (effect.direction === 'completion' && effect.method === 'completeModuleKey') {
        const [,keyId,uri,kind,handle] = effect.arguments;
        assert(typeof keyId === 'string' && !keys.has(keyId), 'duplicate/nondecimal module identity');
        keys.set(keyId, {uri,kind,handle});
      }
      if (effect.direction !== 'request') continue;
      const event = effect.event;
      if (event.glob?.readerKind === 'module') {
        validateGlobRequest(event); const key = keys.get(event.glob.readerId);
        assert(key && key.uri === event.glob.readerUri && key.handle === event.glob.readerHandle && key.kind === event.glob.keyKind, 'glob module identity changed');
        globReaders++;
      }
      if (event.module?.kind === 'load-source') {
        const request = event.module, key = keys.get(request.keyId);
        assert(key && key.uri === request.originalUri && key.handle === request.keyHandle, 'loaded module identity changed');
        assert(typeof request.resolvedKeyId === 'string' && /^[0-9]+$/.test(request.resolvedKeyId), 'resolved key ID narrowed');
        if (request.originalUri === 'gmod:' + String.fromCharCode(0xd800)) rawLoads++;
      }
    }
    assert(globReaders > 0 && rawLoads === 1, 'raw UTF16 module key/load identity witness missing');
    moduleResult.observation = capture;
    moduleResult.passed = true;
  } catch (error) { moduleResult.failure = String(error.message ?? error); }
  results.push(moduleResult);
  run('transport-04', (session, event) => {
    const original = event.token; completeGlobReader(session, event, 'custom', 'transport-reader');
    const pending = next(session); rejects(() => session.completeGlobReader(original, event.glob.readerId, 'custom', 'transport-reader', 0));
    assert(next(session).token === pending.token, 'stale token consumed current operation'); return {original, current: pending.token};
  });
  run('transport-05', (session, event) => {
    rejects(() => session.completeGlobBoolean(event.token, 'is-globbable', true, 0));
    assert(next(session).token === event.token, 'wrong variant consumed pending operation'); return {token: event.token};
  });
  run('transport-06', (session, event) => {
    rejects(() => session.completeGlobReader(event.token, '18446744073709551615', 'custom', 'wrong', 0));
    assert(next(session).token === event.token, 'wrong reader consumed pending operation'); return {attempted: '18446744073709551615', pending: event.glob.readerId};
  });
  run('transport-07', (session, event) => {
    completeGlobReader(session, event, 'custom', 'transport-reader'); const pending = next(session);
    rejects(() => session.rejectGlob(pending.token, 'reader-not-found', null, 0));
    assert(next(session).token === pending.token, 'wrong phase failure consumed pending operation'); return {token: pending.token};
  });
  run('transport-08', (session, event) => {
    const pending = listing(session, event);
    const names = [[0xd800], [0xdfff], [0x78,0xd800,0x79], [0xd83d,0xde00]];
    completeGlobElements(session, pending, names.map(name => ({name: new Uint16Array(name), isDirectory: false})));
    const value = next(session); assert(value.kind === 'value', 'real raw-UTF16 enumeration failed');
    // GlobResolver sorts same-kind elements by Java String.compareTo UTF16.
    // Completion/codec input order remains the original names array above.
    const expectedNames = [names[2], names[0], names[3], names[1]];
    const expected = [];
    for (const [index,name] of expectedNames.entries()) {
      if (index) expected.push(0x7c);
      expected.push(...[0x63,0x75,0x73,0x74,0x6f,0x6d,0x3a], ...name);
    }
    assert(same(Array.from(value.value), expected), 'actual raw URI key units or order changed');
    return {evidenceKind: 'real-session', names, keyUnits: Array.from(value.value)};
  });
  run('transport-09', (session, event) => {
    const pending = listing(session, event);
    rejects(() => session.completeGlobElements(pending.token, [{name: new Uint16Array([97]), isDirectory: 1}], 0));
    assert(next(session).token === pending.token, 'nonboolean directory flag consumed operation'); return {token: pending.token};
  });
  run('transport-10', (session, event) => {
    const pending = listing(session, event); completeGlobElements(session, pending, [{name: new Uint16Array([97]), isDirectory: false}]);
    const value = next(session); assert(value.kind === 'value' && value.value instanceof Uint16Array, 'lossless terminal payload required');
    const snapshot = Array.from(value.value); session.collect(); session.close(); session.collect();
    assert(same(Array.from(value.value), snapshot), 'retained output mutated after collection/close'); return {units: snapshot};
  });
  run('transport-11', (session, event) => {
    const pending = listing(session, event); const name = new Uint16Array([97]);
    completeGlobElements(session, pending, [{name, isDirectory: false}]); name[0] = 98;
    const value = next(session); assert(value.kind === 'value', 'successful detached input expected');
    const units = Array.from(value.value); assert(same(units, [99,117,115,116,111,109,58,97]), 'input was retained by reference'); return {units};
  });
  for (const [id, method] of [['transport-12', 'close'], ['transport-13', 'cancel']]) run(id, (session, event) => {
    session[method](); const cancelled = session.drainCancelledGlobs(); assert(same(cancelled, [event.token]), 'glob absent from abort inventory'); return {cancelled};
  });
  run('transport-14', (session, event) => {
    assert(next(session, 1).kind === 'error', 'deadline should terminate evaluation');
    const cancelled = session.drainCancelledGlobs(); assert(same(cancelled, [event.token]), 'deadline glob inventory'); return {cancelled};
  }, 1);
  run('transport-15', (session, event) => {
    session.reenterPath(event.token, 1, 0, null); const inner = next(session); validateGlobRequest(inner);
    assert(inner.token !== event.token, 'nested token reused'); session.cancel();
    const cancelled = session.drainCancelledGlobs(); assert(same(cancelled, [event.token, inner.token]), 'nested abort order'); return {cancelled};
  });
  run('transport-16', (session, event) => {
    session.cancel(); const first = session.drainCancelledGlobs(), second = session.drainCancelledGlobs();
    assert(same(first, [event.token]) && same(second, []), 'abort inventory must drain once'); return {first, second};
  });
  run('transport-17', (session, event) => {
    rejects(() => session.setGlobTestMode(true)); assert(next(session).token === event.token, 'busy profile setter changed pending operation'); return {token: event.token};
  });
  run('transport-18', (session, event) => {
    completeGlobElements(session, listing(session, event), []); const value = next(session);
    assert(value.kind === 'value' && value.value.length === 0, 'empty listing did not produce empty keys'); return {units: Array.from(value.value)};
  });
  run('transport-19', (session, event) => {
    completeGlobReader(session, event, 'custom', 'transport-reader'); const pending = next(session);
    const message = String.fromCharCode(0x65e5, 0xd800, 0, 0xdc00); session.rejectGlob(pending.token, 'io', message, 0);
    const failure = next(session); assert(failure.kind === 'error' && failure.error instanceof Uint16Array, 'lossless error payload required');
    // Full units are compared across actual native/NAPI/Wasm captures; no message normalization.
    return {inputUnits: Array.from({length: message.length}, (_, i) => message.charCodeAt(i)), errorUnits: Array.from(failure.error), disposition: failure.disposition, requiredCrossTargetComparison: true, fullErrorEqualityEstablished: false};
  });
  run('transport-20', (session, event) => {
    completeGlobReader(session, event, 'custom', 'transport-reader'); const pending = next(session);
    rejects(() => session.completeGlobBoolean(pending.token, 'is-globbable', 1, 0));
    assert(next(session).token === pending.token, 'nonboolean completion consumed pending operation'); return {token: pending.token};
  });
  run('transport-21', (session, event) => {
    rejects(() => session.completeGlobReader(event.token, event.glob.readerId, undefined, undefined, 0));
    assert(next(session).token === event.token, 'missing reader fields consumed pending operation'); return {token: event.token};
  });
  run('transport-22', (session, event) => {
    const invalid = structuredClone(event); invalid.glob.kind = 'unknown'; rejects(() => validateGlobRequest(invalid));
    assert(next(session).token === event.token, 'transport validation changed pending state'); return {unknownRejected: true};
  });
  run('transport-23', (session, event) => {
    // SelectResource has an explicit default ReaderNotFound completion. A
    // custom ListElements request is the actual unavailable-I/O phase.
    const pending = listing(session, event);
    assert(completeDefaultGlob(session, pending) === false, 'unavailable custom listing accepted as default completion');
    assert(next(session).token === pending.token, 'default fallback consumed unavailable listing'); return {unavailableRemainedPending: true};
  });
  return {scope: 'typed transport and lifecycle; not Java parity', results, localPredicatesComplete: results.length === 24 && results.every(row => row.executed && row.passed), complete: false, pending: ['complete transport19 error bytes across all3 targets', 'native typed API counterparts and compile-time-only rejection qualifications']};
}
