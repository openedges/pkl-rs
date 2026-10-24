// Protocol regression tests using scripted transports, not Java compatibility evidence.
import assert from 'node:assert/strict';
import test from 'node:test';
import {runCollectionTransformCase, compareCollectionTransformCase} from './collection-transform-session.mjs';

const view = text => Uint16Array.from(Array.from(text, ch => ch.charCodeAt(0)));
const value = text => ({kind: 'value', value: view(text)});
function fixture(operations = [{kind: 'evaluate', value: 'answer'}]) {
  return {id: 'protocol-case', source: 'answer = 7\n', uri: 'file:///fixture/input.pkl', moduleName: 'input',
    readerRules: [], operations};
}
function mock(scripts, options = {}) {
  const calls = [], outputs = [];
  let queue = [], index = 0, closed = false;
  const session = {
    evaluateModuleSource(...args) {
      calls.push(['evaluateModuleSource', ...args]);
      if (closed) throw Error('closed evaluator');
      queue = scripts[index++].slice();
    },
    poll() {
      const event = queue.shift(); assert.ok(event, 'scripted event must exist');
      if (event.value) outputs.push(event.value);
      if (event.error) outputs.push(event.error);
      return event;
    },
    collect() { calls.push(['collect']); },
    close() { closed = true; calls.push(['close']); },
    heapSlots() { return options.slots ?? 0; },
    free() { if (options.mutateOnFree) outputs[0][0] = 0; calls.push(['free']); },
    completeDefaultModule(...args) { calls.push(['completeDefaultModule', ...args]); return true; },
  };
  for (const method of ['rejectReadIo', 'rejectReadSecurityManager', 'completeReadText', 'acknowledgeLog', 'completeModuleKey',
    'completeModuleCachePolicy', 'completeModuleFragmentPaths', 'completeModuleResolvedKey', 'completeModuleSource'])
    session[method] = (...args) => calls.push([method, ...args]);
  return {calls, api: {SourceSession: {moduleEvaluator(...args) {
    calls.push(['moduleEvaluator', ...args]); return session;
  }}}};
}
function expectedSimple(input) {
  return {...input, expectedEffects: [], expected: [{index: 0, expression: 'answer', scope: 'ordinary',
    status: 'success', statusRawHex: '737563636573730a', stdoutUnits: [55], stdoutHex: '37',
    stderrUnits: [], stderrHex: '', readsBefore: 0, readsAfter: 0, logsBefore: 0, logsAfter: 0}]};
}

test('public evaluator flags and exact raw result survive collect/close/free', () => {
  const input = fixture(), transport = mock([[value('7')]]);
  const actual = runCollectionTransformCase(transport.api, input);
  assert.deepEqual(transport.calls[0], ['moduleEvaluator', 'compact', true, false, false]);
  assert.deepEqual(transport.calls[1], ['evaluateModuleSource', input.uri, null, 'answer', 0, 8000]);
  const compared = compareCollectionTransformCase(expectedSimple(input), actual);
  assert.equal(compared.acquisitionIntegrityPassed, true);
  assert.equal(compared.ordinaryCompatible, 1);
  assert.equal(compared.fullJavaEventsCountersPassed, false);
});

test('reader responses use authored rules and actual attempt, never expected response order', () => {
  const input = fixture([{kind: 'evaluate', value: 'first'}, {kind: 'evaluate', value: 'second'}]);
  input.readerRules = [{uri: 'ct:/x', kind: 'io-first', value: 'actual-cause'}];
  input.readerPlan = [{uri: 'ct:/wrong', result: {kind: 'text', units: [0]}}];
  const failure = {kind: 'error', error: view('failure'), disposition: 'language-error'};
  const read = {kind: 'read', uri: 'ct:/x', token: '9007199254740993'};
  const transport = mock([[read, failure], [read, value('7')]]);
  const actual = runCollectionTransformCase(transport.api, input);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(actual.effects.map(effect => [effect.attempt, effect.result]), [
    [1, {kind: 'io', message: 'actual-cause'}],
    [2, {kind: 'text', units: Array.from('recovered', ch => ch.charCodeAt(0))}],
  ]);
  assert.equal(transport.calls.find(call => call[0] === 'rejectReadIo')[1], read.token);
  assert.deepEqual(transport.calls.find(call => call[0] === 'completeReadText'),
    ['completeReadText', read.token, view('recovered'), 0]);
  assert.deepEqual(actual.calls.map(call => [call.readsBefore, call.readsAfter]), [[0, 1], [1, 2]]);
});

test('unconfigured reads fail acquisition and still clean up', () => {
  const transport = mock([[{kind: 'read', uri: 'ct:/unknown', token: 'x'}]]);
  const actual = runCollectionTransformCase(transport.api, fixture());
  assert.match(actual.acquisitionError, /unconfigured reader/);
  assert.equal(actual.calls.length, 0);
  assert.equal(actual.architecture.terminalSlots, 0);
  assert.equal(transport.calls.filter(call => call[0] === 'completeReadText').length, 0);
});

test('unknown runtime error classification preserves raw observation and fails acquisition', () => {
  const transport = mock([[{kind: 'error', error: view('failure'), disposition: 'future-error'}]]);
  const actual = runCollectionTransformCase(transport.api, fixture());
  assert.deepEqual(actual.calls[0].stderrUnits, Array.from('failure', ch => ch.charCodeAt(0)));
  assert.match(actual.acquisitionError, /classification/);
});

test('matching error bytes with a runtime limit cannot pass ordinary compatibility', () => {
  const input = expectedSimple(fixture());
  Object.assign(input.expected[0], {status: 'error', statusRawHex: '6572726f720a',
    stdoutUnits: [], stdoutHex: '', stderrUnits: [55], stderrHex: '37'});
  const transport = mock([[{kind: 'error', error: view('7'), disposition: 'limit-exceeded'}]]);
  const actual = runCollectionTransformCase(transport.api, input);
  const compared = compareCollectionTransformCase(input, actual);
  assert.equal(compared.rawByteMatches, 1);
  assert.equal(compared.ordinaryCompatible, 0);
  actual.calls[0].disposition = 'unknown';
  assert.equal(compareCollectionTransformCase(input, actual).acquisitionIntegrityPassed, false);
});

test('post-close calls remain observed API rejections, without invented Java close events', () => {
  const input = fixture([{kind: 'evaluate', value: 'answer'}, {kind: 'close'},
    {kind: 'evaluate', value: 'answer'}, {kind: 'close'}]);
  const transport = mock([[value('7')]]), actual = runCollectionTransformCase(transport.api, input);
  assert.deepEqual(actual.calls.map(call => call.status), ['success', 'api-error']);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(actual.effects, []);
  assert.equal(actual.observations.some(event => event.kind === 'reader-close'), false);
});

test('borrowed output mutation after free is detected', () => {
  const transport = mock([[value('7')]], {mutateOnFree: true});
  const input = expectedSimple(fixture()), actual = runCollectionTransformCase(transport.api, input);
  assert.equal(actual.architecture.ownedResultsStable, false);
  assert.equal(compareCollectionTransformCase(input, actual).acquisitionIntegrityPassed, false);
});

test('module key identities and resolved owner are preserved as decimal strings', () => {
  const input = fixture(), id = '18446744073709551614', resolved = '18446744073709551615';
  const request = (kind, fields) => ({kind: 'module-request', token: `t-${kind}`, module: {id: '1', kind, ...fields}});
  const key = {originalUri: input.uri, keyId: id, keyKind: 'host', keyHandle: `fixture-key:${id}`};
  const scripts = [[
    request('select-key', {originalUri: input.uri, keyId: id, selectionPhase: 'primary'}),
    request('query-cache-policy', key),
    request('resolve-key', {...key, resolvedKeyId: resolved, resolutionPhase: 'primary'}),
    request('load-source', {...key, resolvedKeyId: resolved, resolvedKeyHandle: `collection-resolved:${resolved}`,
      resolvedUri: input.uri, sourceUri: input.uri}), value('7'),
  ]];
  const transport = mock(scripts), actual = runCollectionTransformCase(transport.api, input);
  assert.equal(actual.acquisitionError, null);
  const completion = transport.calls.find(call => call[0] === 'completeModuleResolvedKey');
  assert.equal(completion[2], id); assert.equal(completion[3], resolved);
  assert.equal(transport.calls.find(call => call[0] === 'completeModuleSource')[2], input.source);
  const forged = structuredClone(scripts);
  forged[0][3].module.resolvedKeyHandle = 'foreign';
  assert.match(runCollectionTransformCase(mock(forged).api, input).acquisitionError, /provider/);
});

test('altered Java UTF16/UTF8 and unknown scope are rejected independently', () => {
  const input = expectedSimple(fixture()), actual = runCollectionTransformCase(mock([[value('7')]]).api, input);
  input.expected[0].stdoutHex = '38';
  assert.throws(() => compareCollectionTransformCase(input, actual), /UTF16\/UTF8/);
  input.expected[0].stdoutHex = '37'; input.expected[0].scope = 'skip';
  assert.throws(() => compareCollectionTransformCase(input, actual), /scope/);
});

test('malformed module requests are retained before strict validation without responding', () => {
  const event = {kind: 'module-request', token: 'pending', module: {kind: 'select-key', originalUri: 'file:///fixture/input.pkl'}};
  const transport = mock([[event]]), actual = runCollectionTransformCase(transport.api, fixture());
  assert.match(actual.acquisitionError, /module request context/);
  assert.deepEqual(actual.moduleRequests, [{token: 'pending', request: event.module}]);
  assert.equal(transport.calls.some(call => call[0].startsWith('completeModule')), false);
});

const deniedResource = uri => `Refusing to read resource \`${uri}\` because it does not match any entry in the resource allowlist (\`--allowed-resources\`).`;

test('resource policy rejects before configured reader effects and permits later recovery', () => {
  const input = fixture([{kind: 'evaluate', value: 'denied'}, {kind: 'evaluate', value: 'recovered'}]);
  input.readerRules = [{uri: 'prop:pkl.outputFormat', kind: 'text', value: 'must-not-read'},
    {uri: 'ct:/ok', kind: 'text', value: 'reader-value'}];
  const rejection = {kind: 'read', uri: 'prop:pkl.outputFormat', token: '18446744073709551615'};
  const transport = mock([[rejection, {kind: 'error', error: view('security'), disposition: 'language-error'}],
    [{kind: 'read', uri: 'ct:/ok', token: 'ok'}, value('7')]]);
  const actual = runCollectionTransformCase(transport.api, input);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(transport.calls.filter(call => call[0] === 'rejectReadSecurityManager'),
    [['rejectReadSecurityManager', rejection.token, deniedResource(rejection.uri), 0]]);
  assert.deepEqual(actual.calls.map(call => [call.readsBefore, call.readsAfter, call.logsBefore, call.logsAfter]),
    [[0, 0, 0, 0], [0, 1, 0, 0]]);
  assert.deepEqual(actual.effects.map(effect => [effect.kind, effect.uri, effect.attempt]), [['read', 'ct:/ok', 1]]);
  assert.deepEqual(actual.resourcePolicyRequests, [
    {callIndex: 0, uri: rejection.uri, allowed: false, message: deniedResource(rejection.uri), token: rejection.token},
    {callIndex: 1, uri: 'ct:/ok', allowed: true, message: '', token: 'ok'},
  ]);
  assert.equal(transport.calls.filter(call => call[0] === 'completeReadText').length, 1);
  assert.equal(transport.calls.some(call => call[0] === 'rejectReadIo'), false);
  assert.equal(actual.observations.some(event => event.kind === 'resource-policy'), false);
  assert.deepEqual(JSON.parse(JSON.stringify(actual)).resourcePolicyRequests, actual.resourcePolicyRequests);
  assert.equal(actual.architecture.terminalSlots, 0);
});

test('resource allowlist uses lookingAt prefix semantics, including newline and exact case', () => {
  const uris = ['ct:', 'ct:/line\nrest', 'CT:/case', 'prefixct:/not-prefix'];
  const input = fixture(uris.map(uri => ({kind: 'evaluate', value: uri})));
  input.readerRules = uris.map(uri => ({uri, kind: 'text', value: 'configured'}));
  const transport = mock(uris.map((uri, index) => [{kind: 'read', uri, token: String(index)}, value('7')]));
  const actual = runCollectionTransformCase(transport.api, input);
  assert.equal(actual.acquisitionError, null);
  assert.deepEqual(actual.resourcePolicyRequests.map(row => row.allowed), [true, true, false, false]);
  assert.deepEqual(actual.resourcePolicyRequests.map(row => row.message), ['', '', deniedResource(uris[2]), deniedResource(uris[3])]);
  assert.deepEqual(actual.effects.map(row => row.uri), uris.slice(0, 2));
  assert.deepEqual(actual.calls.map(row => row.readsAfter), [1, 2, 2, 2]);
  assert.equal(transport.calls.filter(call => call[0] === 'rejectReadSecurityManager').length, 2);
});

test('allowed unconfigured ct request stays an acquisition error with its policy record', () => {
  const request = {kind: 'read', uri: 'ct:/unconfigured', token: 'unknown'};
  const transport = mock([[request]]), actual = runCollectionTransformCase(transport.api, fixture());
  assert.match(actual.acquisitionError, /unconfigured reader/);
  assert.deepEqual(actual.resourcePolicyRequests, [{callIndex: 0, uri: request.uri,
    allowed: true, message: '', token: request.token}]);
  assert.deepEqual(actual.effects, []);
  assert.equal(transport.calls.some(call => ['completeReadText', 'rejectReadSecurityManager', 'rejectReadIo'].includes(call[0])), false);
  assert.equal(actual.architecture.providerEntriesReleased, true);
});

test('missing typed security completion fails acquisition and keeps denied request', () => {
  const request = {kind: 'read', uri: 'prop:pkl.outputFormat', token: 'denied'};
  const transport = mock([[request]]), construct = transport.api.SourceSession.moduleEvaluator;
  transport.api.SourceSession.moduleEvaluator = (...args) => {
    const session = construct(...args); delete session.rejectReadSecurityManager; return session;
  };
  const actual = runCollectionTransformCase(transport.api, fixture());
  assert.match(actual.acquisitionError, /rejectReadSecurityManager/);
  assert.equal(actual.resourcePolicyRequests[0].allowed, false);
  assert.deepEqual(actual.effects, []);
  assert.equal(actual.calls.length, 0);
  assert.equal(actual.architecture.terminalSlots, 0);
});
