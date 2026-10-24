import test from 'node:test';
import assert from 'node:assert/strict';
import { runFrameTransformerCase, validateFrame } from './frame-transformer-session.mjs';
const u = value => Uint16Array.from({ length: value.length }, (_, i) => value.charCodeAt(i));
const frame = (uri = 'pkl:ref') => ({ moduleUri: u(uri), memberName: null, sourceLines: [u('old()')],
  startLine: 3, startColumn: 2, endLine: 3, endColumn: 6 });
const transform = token => ({ kind: 'transform-frame', token, frame: frame() });
const log = (token, level = 'warn') => ({ kind: 'log', token, log: { ...frame('custom'), level, message: u('old') } });
const value = text => ({ kind: 'value', value: u(text) });
const failure = text => ({ kind: 'error', error: u(text), disposition: 'internal-failure', phase: 'evaluation' });
const profile = extra => ({ mode: 'identity', action: 'none', callback: '', callback_after: 0, ...extra });
const fixture = extra => ({ id: 'transport', source: 'exact source\r\n', uri: 'file:///exact.pkl', operation: 'expression',
  expressions: ['old()'], transformer: profile(), logger: profile({ mode: 'capture' }),
  replacement: { moduleUri: [0xd800], memberName: [0xdc00], sourceLines: [],
    startLine: 20, startColumn: 4, endLine: 99, endColumn: 6 }, ...extra });
function mock(programs, { corrupt = false, invalidTransformToken = false } = {}) {
  const calls = [], contexts = [], payloads = []; let next = 0;
  const session = {
    evaluateModuleSource(...args) { calls.push(['evaluateModuleSource', ...args]); contexts.push([...programs[next++]]); },
    reenterModuleSource(...args) { calls.push(['reenterModuleSource', ...args]); contexts.push([...programs[next++]]); },
    poll(now, budget) {
      calls.push(['poll', now, budget]); const event = contexts.at(-1)?.shift();
      if (!event) throw Error('mock call did not settle');
      if (event.frame) payloads.push(event.frame.moduleUri);
      if (event.value) payloads.push(event.value);
      if (['value', 'error'].includes(event.kind)) contexts.pop(); return event;
    },
    completeFrameTransform(...args) { calls.push(['completeFrameTransform', ...args]); if (invalidTransformToken) throw Error('actual stale transform token'); },
    acknowledgeLog(...args) { calls.push(['acknowledgeLog', ...args]); },
    setFrameTransformer(...args) { calls.push(['setFrameTransformer', ...args]); },
    completeModuleKey(...args) { calls.push(['completeModuleKey', ...args]); },
    completeModuleCachePolicy(...args) { calls.push(['completeModuleCachePolicy', ...args]); },
    completeModuleFragmentPaths(...args) { calls.push(['completeModuleFragmentPaths', ...args]); },
    completeDefaultModule(...args) { calls.push(['completeDefaultModule', ...args]); return true; },
    close() {}, collect() {}, heapSlots() { return 0; }, contextDepth() { return contexts.length; },
    free() { if (corrupt) payloads.forEach(value => value.fill(0)); },
  };
  return { calls, api: { SourceSession: { moduleEvaluator(...args) { calls.push(['moduleEvaluator', ...args]); return session; } } } };
}
const decode = (row, name) => Buffer.from(row.payloads[name]).toString('utf8');

test('transform completion and Logger acknowledgment are separate issued steps', () => {
  const m = mock([[transform('transform'), log('logger'), value('ok')]]);
  const row = runFrameTransformerCase(m.api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', true, true, false]);
  assert.deepEqual(m.calls.filter(x => ['completeFrameTransform', 'acknowledgeLog'].includes(x[0])).map(x => x.slice(0, 2)),
    [['completeFrameTransform', 'transform'], ['acknowledgeLog', 'logger']]);
  assert.deepEqual(row.protocol, ['call-enter=outer-0', 'transform-enter=0 depth=0', 'transform-exit=0',
    'log-enter=0 level=warn depth=0', 'log-exit=0', 'call-exit=outer-0 status=success', 'closed transforms=1 logs=1 warnings=1']);
  assert.equal(decode(row, 'transform-0.input.uri.bin'), 'pkl:ref');
  assert.equal(row.architecture.collections, row.architecture.polls + 1);
});

test('whole replacement preserves independent fields, empty lines and isolated surrogates', () => {
  const m = mock([[transform('t'), value('ok')]]);
  const row = runFrameTransformerCase(m.api, fixture({ transformer: profile({ mode: 'full' }) }));
  assert.equal(row.acquisitionError, null);
  const output = m.calls.find(x => x[0] === 'completeFrameTransform')[2];
  assert.deepEqual(output.moduleUri, Uint16Array.of(0xd800));
  assert.deepEqual(output.memberName, Uint16Array.of(0xdc00));
  assert.deepEqual(output.sourceLines, []); assert.equal(output.endLine, 99);
  assert.deepEqual(row.payloads['transform-0.output.uri.code-units.bin'], [0xd8, 0]);
  assert.deepEqual(row.payloads['transform-0.output.uri.bin'], [0x3f]);
});

test('numbered callback indexes every transform and does not rewrite the input', () => {
  const m = mock([[transform('a'), transform('b'), value('ok')]]);
  const row = runFrameTransformerCase(m.api, fixture({ transformer: profile({ mode: 'numbered' }) }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(row.payloads['transform-1.output.uri.code-units.bin'], [0xd8, 0, 0, 35, 0, 49]);
  assert.equal(decode(row, 'transform-1.input.uri.bin'), 'pkl:ref');
});

test('noop Logger still runs the real custom transformer without logger acknowledgments', () => {
  const m = mock([[transform('a'), value('ok')]]);
  const row = runFrameTransformerCase(m.api, fixture({ logger: profile({ mode: 'noop' }) }));
  assert.equal(row.acquisitionError, null); assert.deepEqual(m.calls[0], ['moduleEvaluator', 'compact', false, true, false]);
  assert.equal(m.calls.filter(x => x[0] === 'completeFrameTransform').length, 1);
  assert.equal(m.calls.some(x => x[0] === 'acknowledgeLog'), false);
  assert.equal(row.protocol.at(-1), 'closed transforms=1 logs=0 warnings=0');
});

test('default and built-in identity use actual engine policy with no synthetic transformer calls', () => {
  for (const mode of ['default', 'builtin-identity']) {
    const m = mock([[log('a'), value('ok')]]); const row = runFrameTransformerCase(m.api, fixture({ transformer: profile({ mode }) }));
    assert.equal(row.acquisitionError, null); assert.equal(m.calls[0][3], false);
    assert.equal(m.calls.some(x => x[0] === 'setFrameTransformer'), mode === 'builtin-identity');
    assert.equal(m.calls.some(x => x[0] === 'completeFrameTransform'), false);
  }
});

test('independent callbacks reenter at both stages and count nested callback depth', () => {
  const m = mock([[transform('outer-t'), log('outer-l'), value('outer')],
    [transform('inner-t'), log('inner-l'), value('inner-transform')], [transform('deep-t'), log('deep-l'), value('inner-log')]]);
  const row = runFrameTransformerCase(m.api, fixture({ transformer: profile({ callback: 'nestedTransform()' }),
    logger: profile({ mode: 'capture', callback: 'nestedLogger()' }) }));
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(m.calls.filter(x => x[0] === 'reenterModuleSource').map(x => x[1]), ['outer-t', 'inner-l']);
  assert.ok(row.protocol.includes('transform-enter=2 depth=2'));
  assert.ok(row.protocol.includes('log-enter=1 level=warn depth=2'));
  assert.equal(decode(row, 'transform-callback-0.stdout.bin'), 'inner-transform');
  assert.equal(decode(row, 'log-callback-0.stdout.bin'), 'inner-log');
});

test('throw-once sends actual failure completion then permits retry without inventing Java class', () => {
  const m = mock([[transform('bad'), failure('actual failure')], [transform('retry'), value('ok')]]);
  const row = runFrameTransformerCase(m.api, fixture({ expressions: ['old()', 'old()'], transformer: profile({ action: 'throw-once' }) }));
  assert.equal(row.acquisitionError, null);
  const calls = m.calls.filter(x => x[0] === 'completeFrameTransform');
  assert.equal(calls[0][2], null); assert.deepEqual(calls[0][3], u('stage55-transformer-failure'));
  assert.equal(calls[1][3], undefined); assert.ok(row.protocol.includes('transform-throw=0'));
  assert.equal(decode(row, 'outer-0.status.bin'), 'candidate-error\n');
  assert.equal(Object.keys(row.payloads).some(name => name.endsWith('error-class.bin')), false);
});

test('typed callback origin is separate metadata and preserves the actual error channels', () => {
  for (const origin of ['log-transformer', 'log-delegate', 'diagnostic-transformer', 'diagnostic-renderer']) {
    const error = { ...failure('actual \ud800 callback message'), callbackOrigin: origin };
    const m = mock([[error], [value('ok')]]);
    const row = runFrameTransformerCase(m.api, fixture({ expressions: ['first()', 'next()'] }));
    assert.equal(row.acquisitionError, null);
    assert.equal(decode(row, 'outer-0.callback-origin.bin'), origin);
    assert.deepEqual(row.payloads['outer-0.stderr.code-units.bin'],
      Array.from(u('actual \ud800 callback message')).flatMap(unit => [unit >> 8, unit & 255]));
    assert.equal(decode(row, 'outer-0.status.bin'), 'candidate-error\n');
    assert.equal(decode(row, 'outer-1.callback-origin.bin'), '');
    assert.equal(Object.keys(row.payloads).some(name => name.endsWith('error-class.bin')), false);
  }
});

test('unknown callback origin is a protocol failure and cannot be presented as compatible', () => {
  const m = mock([[{ ...failure('actual failure'), callbackOrigin: 'invented-java-class' }]]);
  const row = runFrameTransformerCase(m.api, fixture());
  assert.match(row.acquisitionError.message, /invalid callback origin metadata/);
});

test('logger threshold counts trace entries and reports deliberate throw distinctly', () => {
  const m = mock([[log('trace', 'trace'), log('warn'), value('outer')], [value('inner')]]);
  const row = runFrameTransformerCase(m.api, fixture({ logger: profile({ mode: 'capture', action: 'throw-once', callback: 'nested()', callback_after: 1 }) }));
  assert.equal(row.acquisitionError, null);
  assert.equal(m.calls.find(x => x[0] === 'reenterModuleSource')[1], 'warn');
  assert.ok(row.protocol.includes('log-throw=1'));
});

test('nested same-stage callback consumes throw-once before the outer callback resumes', () => {
  const m = mock([[transform('outer'), value('outer')], [transform('inner'), failure('nested failure')]]);
  const row = runFrameTransformerCase(m.api, fixture({ transformer: profile({ action: 'throw-once', callback: 'nested()' }) }));
  assert.equal(row.acquisitionError, null);
  const completions = m.calls.filter(x => x[0] === 'completeFrameTransform');
  assert.deepEqual(completions.map(x => x[1]), ['inner', 'outer']);
  assert.equal(completions[0][2], null); assert.deepEqual(completions[0][3], u('stage55-transformer-failure'));
  assert.equal(completions[1][3], undefined);
  assert.ok(row.protocol.includes('transform-throw=1'));
  assert.ok(row.protocol.includes('transform-exit=0'));
});

test('stale transform rejection remains actual acquisition failure and does not create a terminal pass', () => {
  const m = mock([[transform('stale'), value('unreached')]], { invalidTransformToken: true });
  const row = runFrameTransformerCase(m.api, fixture());
  assert.equal(row.acquisitionError.message, 'actual stale transform token');
  assert.equal(row.payloads['outer-0.status.bin'], undefined);
});

test('retained transform payload corruption after session free is detected', () => {
  const m = mock([[transform('t'), value('owned')]], { corrupt: true });
  assert.equal(runFrameTransformerCase(m.api, fixture()).architecture.retainedPayloadsStable, false);
});

test('invalid full frame text and numeric fields cannot pass validation', () => {
  for (const bad of [{ moduleUri: [65] }, { memberName: undefined }, { sourceLines: [[65]] },
    { startLine: -1 }, { endColumn: 0x100000000 }, { startColumn: NaN }, { endLine: 1.2 }])
    assert.throws(() => validateFrame({ ...frame(), ...bad }), /invalid owned/);
  assert.doesNotThrow(() => validateFrame({ ...frame(), sourceLines: [] }));
});
