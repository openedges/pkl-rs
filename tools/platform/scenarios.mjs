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
  await check('async-same-stack-resume-reentry-duplicate', async () => {
    const probe = new api.HostProbe('add');
    try {
      probe.start(clock(), 10000); const token = read(probe);
      eq(read(probe), token);
      throws(() => probe.start(clock(), 10000), 'evaluation is already active');
      await delay(5);
      probe.respondInteger(token, 2n, clock());
      throws(() => probe.respondInteger(token, 2n, clock()), 'stale or unexpected Host completion');
      eq(probe.poll(clock(), 100), 'ready\tint\t42');
      probe.start(clock(), 10000); const next = read(probe);
      assert(next !== token, 'request identity reused');
      throws(() => probe.respondInteger(token, 2n, clock()), 'stale or unexpected Host completion');
      probe.respondInteger(next, 3n, clock());
      eq(probe.poll(clock(), 100), 'ready\tint\t43');
    } finally { dispose(probe); }
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
  await check('cancel-close-late-callback', async () => {
    for (const action of ['cancel', 'close']) {
      const probe = new api.HostProbe('read');
      try {
        probe.start(clock(), 10000); const token = read(probe);
        await delay(5); probe[action]();
        eq(probe.poll(clock(), 100).split('\t')[1], action === 'cancel' ? 'Cancelled' : 'Closed');
        throws(() => probe.respondInteger(token, 1n, clock()), 'evaluator is closed');
        throws(() => probe.start(clock(), 10000), 'evaluator is closed');
      } finally { dispose(probe); }
    }
  });
  await check('real-timer-waiting-timeout', async () => {
    const probe = new api.HostProbe('read');
    try {
      const start = clock(); probe.start(start, 50); const token = read(probe);
      await delay(70);
      eq(probe.poll(clock(), 100).split('\t')[1], 'Timeout');
      throws(() => probe.respondInteger(token, 1n, clock()), 'evaluator is closed');
    } finally { dispose(probe); }
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
