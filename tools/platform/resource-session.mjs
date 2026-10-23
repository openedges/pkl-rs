// The shared runtime owns URI resolution, Resource construction and caching.
// This Host consumes an explicit Java-backed reader plan without evaluating Pkl.
const same = (left, right) => left.length === right.length
  && left.every((value, index) => value === right[index]);
function vector(value, maximum, name) {
  if (!Array.isArray(value) || value.some(unit => !Number.isInteger(unit) || unit < 0 || unit > maximum))
    throw Error(`invalid ${name}`);
  return value;
}
function message(error) {
  const text = String(error?.message ?? error);
  return {message: text, units: Array.from({length: text.length}, (_, index) => text.charCodeAt(index)),
    stack: error?.stack ?? null};
}
function validate(fixture) {
  for (const name of ['id', 'source', 'uri', 'moduleName'])
    if (typeof fixture?.[name] !== 'string') throw Error(`invalid fixture ${name}`);
  if (!Array.isArray(fixture.expressions) || !fixture.expressions.length
      || fixture.expressions.some(expression => typeof expression !== 'string'))
    throw Error('invalid expression sequence');
  if (!Array.isArray(fixture.readerPlan)) throw Error('invalid reader plan');
  for (const entry of fixture.readerPlan) {
    if (typeof entry?.uri !== 'string' || !entry.result) throw Error('invalid reader plan entry');
    const result = entry.result;
    switch (result.kind) {
      case 'missing': break;
      case 'text': vector(result.units, 65535, 'reader UTF16 units'); break;
      case 'resource':
        if (typeof result.resourceUri !== 'string') throw Error('invalid returned Resource URI');
        vector(result.bytes, 255, 'Resource bytes');
        break;
      case 'io':
      case 'security-manager-exception':
        if (!Object.hasOwn(result, 'message') || (result.message !== null && typeof result.message !== 'string'))
          throw Error('invalid typed reader failure');
        break;
      default: throw Error('unknown typed reader completion');
    }
  }
}

export function runResourceCase(api, fixture, budget = 7) {
  const calls = [], readerUris = [], retained = [];
  const architecture = {collections: 0, terminalSlots: null, ownedResultsStable: false,
    consumedReaderPlan: false};
  let session, constructorError = null, acquisitionError = null;
  try {
    validate(fixture);
    if (!Number.isSafeInteger(budget) || budget < 1) throw Error('invalid polling budget');
    try {
      session = api.SourceSession.forExpressions(fixture.source, fixture.uri, fixture.moduleName,
        fixture.expressions, 'COMPACT', false, false, true);
    } catch (error) {
      constructorError = message(error);
    }
    if (session) {
      let polls = 0;
      for (let index = 0; index < fixture.expressions.length; index++) {
        const call = {index, expression: fixture.expressions[index], readsBefore: readerUris.length,
          readsAfter: null, status: null, stdoutUnits: [], stderrUnits: [], disposition: null};
        session.startPath(index, 0, null);
        let settled = false;
        while (++polls <= 1_000_000) {
          const event = session.poll(0, budget);
          session.collect(); architecture.collections++;
          if (event.kind === 'yield') continue;
          if (event.kind === 'read') {
            if (typeof event.token !== 'string') throw Error('invalid reader token');
            const uri = session.resolvedReadUri(event.token);
            if (typeof uri !== 'string') throw Error('invalid resolved reader URI');
            const entry = fixture.readerPlan[readerUris.length];
            readerUris.push(uri);
            if (!entry || entry.uri !== uri) throw Error('reader URI/order differs from Java reader plan');
            const result = entry.result;
            if (result.kind === 'missing') session.completeReadMissing(event.token, 0);
            else if (result.kind === 'text') {
              const payload = Uint16Array.from(result.units);
              session.completeReadText(event.token, payload, 0);
              payload.fill(0); // The runtime must own its completion payload.
            } else if (result.kind === 'resource') {
              const payload = Uint8Array.from(result.bytes);
              session.completeReadResource(event.token, result.resourceUri, payload, 0);
              payload.fill(0);
            } else if (result.kind === 'io') session.rejectReadIo(event.token, result.message, 0);
            else session.rejectReadSecurityManager(event.token, result.message, 0);
            continue;
          }
          if (event.kind !== 'value' && event.kind !== 'error')
            throw Error(`unexpected Resource session event: ${event.kind}`);
          const view = event.kind === 'value' ? event.value : event.error;
          if (!(view instanceof Uint16Array)) throw Error('expected Uint16Array completion');
          const snapshot = Array.from(view);
          retained.push({view, snapshot});
          call.status = event.kind === 'value' ? 'success' : 'error';
          call[event.kind === 'value' ? 'stdoutUnits' : 'stderrUnits'] = snapshot;
          call.disposition = event.disposition ?? null;
          call.phase = event.phase ?? null;
          call.reason = event.reason ?? null;
          call.readsAfter = readerUris.length;
          calls.push(call);
          settled = true;
          break;
        }
        if (!settled) throw Error('Resource session polling bound exceeded');
      }
      architecture.consumedReaderPlan = readerUris.length === fixture.readerPlan.length;
    }
  } catch (error) {
    acquisitionError = message(error);
  } finally {
    if (session) {
      try {
        session.close();
        session.collect(); architecture.collections++;
        architecture.terminalSlots = session.heapSlots();
      } catch (error) { acquisitionError ??= message(error); }
      try { session.free?.(); } catch (error) { acquisitionError ??= message(error); }
      try {
        architecture.ownedResultsStable = retained.every(({view, snapshot}) => same(Array.from(view), snapshot));
      } catch (error) { acquisitionError ??= message(error); }
    }
  }
  return {id: fixture?.id, repeat: fixture?.repeat, session: fixture?.session,
    calls, readerUris, constructorError, acquisitionError, architecture};
}

// Java String.getBytes(UTF_8) replaces unmatched surrogates with '?'. Keep the
// original UTF16 channel too; TextEncoder alone cannot verify that boundary.
export function javaUtf8Bytes(units) {
  vector(units, 65535, 'output UTF16 units');
  let text = '';
  for (let index = 0; index < units.length; index++) {
    const unit = units[index], next = units[index + 1];
    if (unit >= 0xd800 && unit <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
      text += String.fromCharCode(unit, next); index++;
    } else text += unit >= 0xd800 && unit <= 0xdfff ? '?' : String.fromCharCode(unit);
  }
  return Array.from(new TextEncoder().encode(text));
}
function byteHex(units) {
  return javaUtf8Bytes(units).map(byte => byte.toString(16).padStart(2, '0')).join('');
}
export function compareResourceCase(fixture, actual) {
  if (!Array.isArray(fixture.expected) || fixture.expected.length !== fixture.expressions.length)
    throw Error('missing expected Java API calls');
  const checks = {
    identity: actual.id === fixture.id && actual.repeat === fixture.repeat && actual.session === fixture.session,
    acquisition: actual.constructorError === null && actual.acquisitionError === null,
    calls: actual.calls.length === fixture.expected.length,
    readerTrace: same(actual.readerUris, fixture.readerPlan.map(entry => entry.uri)),
    consumedReaderPlan: actual.architecture.consumedReaderPlan === true,
    terminalSlots: actual.architecture.terminalSlots === 0,
    ownedResultsStable: actual.architecture.ownedResultsStable === true,
  };
  const comparisons = fixture.expected.map((expected, index) => {
    if (!['success', 'error'].includes(expected.status)) throw Error('invalid Java API status');
    for (const channel of ['stdout', 'stderr']) {
      vector(expected[channel + 'Units'], 65535, `expected ${channel} units`);
      if (!/^(?:[0-9a-f]{2})*$/.test(expected[channel + 'Hex'])
          || byteHex(expected[channel + 'Units']) !== expected[channel + 'Hex'])
        throw Error(`inconsistent Java ${channel} UTF16/UTF8 evidence`);
    }
    for (const field of ['readsBefore', 'readsAfter'])
      if (!Number.isSafeInteger(expected[field]) || expected[field] < 0) throw Error('invalid Java reader count');
    const call = actual.calls[index];
    if (!call) return {index, status: 'missing-call', checks: {present: false}};
    const channels = {
      index: call.index === index && call.expression === fixture.expressions[index],
      status: call.status === expected.status,
      stdoutUnits: same(call.stdoutUnits, expected.stdoutUnits),
      stderrUnits: same(call.stderrUnits, expected.stderrUnits),
      stdoutBytes: byteHex(call.stdoutUnits) === expected.stdoutHex,
      stderrBytes: byteHex(call.stderrUnits) === expected.stderrHex,
      readsBefore: call.readsBefore === expected.readsBefore,
      readsAfter: call.readsAfter === expected.readsAfter,
      disposition: call.status === 'success' || call.disposition === 'language-error',
    };
    return {index, status: Object.values(channels).every(Boolean) ? 'pass' : 'mismatch', checks: channels};
  });
  return {checks, comparisons, passed: Object.values(checks).every(Boolean)
    && comparisons.every(comparison => comparison.status === 'pass')};
}
