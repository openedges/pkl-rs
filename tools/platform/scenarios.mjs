import { FakeHostOperations, waitingEvaluation } from './host-adapter.mjs';

// Identical assertions execute inside Node and an actual Chromium page.
export async function scenarios(api) {
  const passed = [];
  const assert = (condition, message) => { if (!condition) throw new Error(message); };
  const eq = (actual, expected) => assert(actual === expected, `${String(actual)} != ${String(expected)}`);
  const throws = (fn, message) => {
    let caught;
    try { fn(); } catch (error) { caught = error; }
    assert(caught instanceof Error, 'expected a JavaScript Error');
    if (message) eq(caught.message, message);
  };
  const check = async (name, fn) => { await fn(); passed.push(name); };
  const clock = () => Math.floor(performance.now());
  const delay = (ms) => new Promise(resolve => setTimeout(resolve, ms));
  const dispose = (probe) => { probe.close(); if (probe.free) probe.free(); };
  const read = probe => {
    const fields = probe.poll(clock(), 100).split('\t');
    eq(fields[0], 'read'); eq(fields[2], '746573743a2f7265736f75726365');
    return fields[1];
  };
  await check('shared-parser-resolver-vm', () => {
    eq(api.evaluate('answer = 40 + 2', 'answer'), 'int\t42');
    eq(api.evaluate('answer = "한글🙂"', 'answer'), 'string\ted959ceab880f09f9982');
    eq(api.evaluate('base = new { n = 1; value = this.n }; child = (base) { n = 3 }; answer = child.value', 'answer'), 'int\t3');
    throws(() => api.evaluate('answer = 1 +', 'answer'));
  });
  // These source cases also execute against Java in the M2 native differential
  // selection. Here they verify the same engine through real JS/Wasm bindings.
  await check('m2-recursive-methods-and-closures', () => {
    eq(api.evaluate('function sum(n) = if (n == 0) 0 else n + sum(n - 1)\nresult = sum(10)', 'result'), 'int\t55');
    eq(api.evaluate('result = let (x = 40) ((y) -> x + y).apply(2)', 'result'), 'int\t42');
    eq(api.evaluate('function add(x: Int, y: Int): Int = x + y\nresult = add(40, 2)', 'result'), 'int\t42');
  });
  await check('m2-lazy-control-and-interpolation', () => {
    eq(api.evaluate('result = 42 ?? missing', 'result'), 'int\t42');
    eq(api.evaluate('result = false && missing', 'result'), 'bool\tfalse');
    eq(api.evaluate('result = "answer: \\(40 + 2)"', 'result'), 'string\t616e737765723a203432');
  });
  await check('utf8-utf16-string-boundary', () => {
    for (const value of ['', '한글🙂e\u0301\0\r\n', '𐐀漢字']) eq(api.roundtripString(value), value);
    for (const value of ['\ud800', '\udfff', 'a\ud800b']) throws(() => api.roundtripString(value), 'unpaired UTF-16 surrogate is unsupported');
    for (const value of [123, null, undefined, {}]) throws(() => api.roundtripString(value));
  });
  await check('lossless-i64-bigint-range', () => {
    for (const value of [0n, -1n, 9007199254740993n, -(1n << 63n), (1n << 63n) - 1n]) eq(api.roundtripInteger(value), value);
    for (const value of [1n << 63n, -(1n << 63n) - 1n, 1n << 100n, -(1n << 100n)]) throws(() => api.roundtripInteger(value), 'BigInt is outside signed 64-bit range');
    throws(() => api.roundtripInteger(42));
  });
  await check('owned-byte-array-boundary', () => {
    const input = new Uint8Array([0, 255, 13, 10, 128, 42]);
    const result = api.roundtripBytes(input);
    eq([...result].join(','), [...input].join(','));
    result[0] = 99; eq(input[0], 0);
    input[1] = 7; eq(result[1], 255);
    for (const value of [[], {}, new Uint16Array([1]), null]) throws(() => api.roundtripBytes(value));
  });
  await check('same-evaluator-nested-read-resume', async () => {
    const probe = new api.HostProbe('add');
    const host = new FakeHostOperations(probe, clock);
    try {
      probe.start(clock(), 10000); const token = read(probe);
      const outer = host.begin(token);
      eq(read(probe), token);
      probe.reenter(token, clock(), 10000);
      const nestedToken = read(probe);
      assert(nestedToken !== token, 'nested read reused parent identity');
      const nested = host.begin(nestedToken);
      eq(host.stats.active, 2); eq(host.stats.resources, 2);
      throws(() => probe.respondInteger(token, 9n, clock()), 'stale or unexpected Host completion');
      await delay(5);
      host.complete(nestedToken, 2n);
      eq((await nested.settled).value, 2n);
      eq(probe.poll(clock(), 100), 'ready\tint\t102');
      eq(read(probe), token);
      eq(host.stats.active, 1); eq(host.stats.resources, 1);
      host.complete(token, 3n);
      eq((await outer.settled).value, 3n);
      throws(() => probe.respondInteger(token, 3n, clock()), 'stale or unexpected Host completion');
      eq(probe.poll(clock(), 100), 'ready\tint\t43');
      eq(host.stats.resources, 0); eq(host.stats.settled, 2); eq(host.stats.aborted, 0);
      eq(host.operations.size, 0); eq(host.stats.timers, 0); eq(host.stats.listeners, 0);
      probe.start(clock(), 10000); const next = read(probe);
      assert(next !== token, 'request identity reused');
      throws(() => probe.respondInteger(token, 2n, clock()), 'stale or unexpected Host completion');
      probe.respondInteger(next, 3n, clock());
      eq(probe.poll(clock(), 100), 'ready\tint\t43');
    } finally { probe.close(); host.drainAborts(); if (probe.free) probe.free(); }
  });
  await check('cross-session-completion-rejected', () => {
    const a = new api.HostProbe('read'), b = new api.HostProbe('read');
    try {
      a.start(clock(), 10000); b.start(clock(), 10000);
      const at = read(a), bt = read(b);
      throws(() => b.respondInteger(at, 1n, clock()), 'stale or unexpected Host completion');
      b.respondInteger(bt, 5n, clock()); eq(b.poll(clock(), 100), 'ready\tint\t5');
    } finally { dispose(a); dispose(b); }
  });
  await check('async-resource-bytes', async () => {
    const probe = new api.HostProbe('read');
    try {
      probe.start(clock(), 10000); const token = read(probe);
      await delay(5);
      probe.respondBytes(token, new Uint8Array([0, 255, 13, 10]), clock());
      eq(probe.poll(clock(), 100), 'ready\tbytes\t00ff0d0a');
    } finally { dispose(probe); }
  });
  await check('host-error-bytes', async () => {
    const probe = new api.HostProbe('read');
    try {
      probe.start(clock(), 10000); const token = read(probe);
      await delay(5); probe.reject(token, 'failed\0\r\n🙂', clock());
      eq(probe.poll(clock(), 100), 'error\tHost\t6661696c6564000d0af09f9982');
    } finally { dispose(probe); }
  });
  for (const action of ['cancel', 'close', 'timeout']) {
    await check(`live-host-${action}-abort-release-settle`, async () => {
      const probe = new api.HostProbe('read');
      let evaluation;
      try {
        evaluation = waitingEvaluation(probe, action === 'timeout' ? 50 : 10000, clock);
        const { host } = evaluation;
        const [operation] = host.operations.values();
        eq(host.stats.active, 1); eq(host.stats.resources, 1);
        assert(!operation.controller.signal.aborted, 'operation aborted before terminal action');
        await delay(5);
        eq(host.stats.active, 1); eq(host.stats.settled, 0);
        if (action !== 'timeout') { probe[action](); evaluation.pump(); }
        const result = await evaluation.result;
        eq(result.split('\t')[1], { cancel: 'Cancelled', close: 'Closed', timeout: 'Timeout' }[action]);
        eq((await operation.settled).error, 'AbortError');
        assert(operation.controller.signal.aborted, 'underlying Host operation was not aborted');
        eq(host.stats.aborted, 1); eq(host.stats.active, 0); eq(host.stats.resources, 0);
        eq(host.operations.size, 0); eq(host.stats.timers, 0); eq(host.stats.listeners, 0);
        eq(host.stats.settled, 1); eq(host.stats.rejected, 1); eq(host.stats.fulfilled, 0);
        eq(probe.drainCancelledReads().length, 0);
        throws(() => host.complete(operation.token, 1n), 'evaluator is closed');
        eq(host.stats.lateRejected, 1); eq(host.stats.settled, 1);
        throws(() => probe.start(clock(), 10000), 'evaluator is closed');
      } finally {
        if (evaluation) { evaluation.stop(); probe.close(); evaluation.host.drainAborts(); }
        dispose(probe);
      }
    });
  }
  await check('nested-terminal-aborts-all-live-host-operations', async () => {
    for (const action of ['cancel', 'close', 'timeout']) {
      const probe = new api.HostProbe('add');
      let evaluation;
      try {
        // The outer deadline also bounds a nested context with a longer deadline.
        evaluation = waitingEvaluation(probe, action === 'timeout' ? 50 : 10000, clock);
        const { host } = evaluation;
        const [outer] = host.operations.values();
        probe.reenter(outer.token, clock(), 10000);
        evaluation.pump();
        const operations = [...host.operations.values()];
        eq(operations.length, 2); eq(host.stats.active, 2); eq(host.stats.resources, 2);
        eq(host.stats.timers, 2); eq(host.stats.listeners, 2);
        await delay(5);
        eq(host.stats.settled, 0);
        if (action !== 'timeout') { probe[action](); evaluation.pump(); }
        eq((await evaluation.result).split('\t')[1], { cancel: 'Cancelled', close: 'Closed', timeout: 'Timeout' }[action]);
        for (const operation of operations) {
          eq((await operation.settled).error, 'AbortError');
          assert(operation.controller.signal.aborted, 'nested terminal left an active Host operation');
          throws(() => host.complete(operation.token, 1n), 'evaluator is closed');
        }
        eq(host.stats.aborted, 2); eq(host.stats.rejected, 2); eq(host.stats.settled, 2);
        eq(host.stats.fulfilled, 0); eq(host.stats.lateRejected, 2);
        eq(host.stats.active, 0); eq(host.stats.resources, 0); eq(host.operations.size, 0);
        eq(host.stats.timers, 0); eq(host.stats.listeners, 0);
        eq(probe.drainCancelledReads().length, 0);
        throws(() => probe.start(clock(), 10000), 'evaluator is closed');
      } finally {
        if (evaluation) { evaluation.stop(); probe.close(); evaluation.host.drainAborts(); }
        dispose(probe);
      }
    }
  });
  await check('cooperative-loop-cancel', async () => {
    const probe = new api.HostProbe('loop');
    try {
      probe.start(clock(), 10000); eq(probe.poll(clock(), 1000), 'yield');
      await delay(5); probe.cancel();
      eq(probe.poll(clock(), 100).split('\t')[1], 'Cancelled');
    } finally { dispose(probe); }
  });
  await check('invalid-clock-budget-boundary', () => {
    const probe = new api.HostProbe('loop');
    try {
      for (const value of [-1, 0.5, NaN, Infinity, 4294967296]) {
        const message = 'expected an integer in 0..=4294967295';
        throws(() => probe.start(value, 100), message);
        throws(() => probe.start(0, value), message);
        throws(() => probe.poll(value, 100), message);
        throws(() => probe.poll(0, value), message);
      }
      probe.start(4294967295, 1);
      eq(probe.poll(4294967295, 1), 'yield');
    } finally { dispose(probe); }
  });
  return passed;
}
