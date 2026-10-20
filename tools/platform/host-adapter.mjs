// Experimental platform I/O adapter. Rust owns evaluation and terminal state;
// each outstanding fake read owns a real timer and an AbortController here.
export class FakeHostOperations {
  constructor(probe, clock) {
    this.probe = probe;
    this.clock = clock;
    this.operations = new Map();
    this.stats = { started: 0, active: 0, resources: 0, timers: 0, listeners: 0, aborted: 0, settled: 0, fulfilled: 0, rejected: 0, lateRejected: 0 };
  }

  begin(token) {
    if (this.operations.has(token)) return this.operations.get(token);
    const controller = new AbortController();
    let resolve, reject, timer, released = false;
    const promise = new Promise((ok, fail) => { resolve = ok; reject = fail; });
    const release = () => {
      if (released) throw new Error('Host resource released twice');
      released = true;
      clearTimeout(timer);
      controller.signal.removeEventListener('abort', abort);
      this.operations.delete(token);
      this.stats.active--;
      this.stats.resources--;
      this.stats.timers--;
      this.stats.listeners--;
    };
    const operation = { token, controller, promise, finish: value => { release(); resolve(value); } };
    operation.settled = promise.then(
      value => { this.stats.settled++; this.stats.fulfilled++; return { value }; },
      error => { this.stats.settled++; this.stats.rejected++; return { error: error.name }; },
    );
    const abort = () => {
      this.stats.aborted++;
      release();
      const error = new Error('Host read aborted');
      error.name = 'AbortError';
      reject(error);
    };
    controller.signal.addEventListener('abort', abort, { once: true });
    this.stats.started++;
    this.stats.active++;
    this.stats.resources++;
    this.stats.timers++;
    this.stats.listeners++;
    // The operation remains genuinely live until completion or an abort signal.
    timer = setTimeout(() => this.complete(token, 1n), 60000);
    this.operations.set(token, operation);
    return operation;
  }

  complete(token, value) {
    try { this.probe.respondInteger(token, value, this.clock()); }
    catch (error) { this.stats.lateRejected++; throw error; }
    this.operations.get(token).finish(value);
  }

  drainAborts() {
    const tokens = this.probe.drainCancelledReads();
    for (const token of tokens) {
      const operation = this.operations.get(token);
      if (!operation || operation.controller.signal.aborted) throw new Error('invalid or duplicate Host abort signal');
      operation.controller.abort();
    }
    return tokens;
  }
}

// Terminal-lifecycle test driver: reads remain pending until cancel/close/timeout.
// Successful nested evaluations are driven explicitly by the reentry scenario;
// this helper is not a general nested-evaluation Promise adapter.
export function waitingEvaluation(probe, timeout, clock) {
  const host = new FakeHostOperations(probe, clock);
  let settle, fail, timer, terminal;
  const result = new Promise((resolve, reject) => { settle = resolve; fail = reject; });
  const pump = () => {
    if (terminal !== undefined) return terminal;
    try {
      const state = probe.poll(clock(), 100);
      host.drainAborts();
      if (state.startsWith('read\t')) host.begin(state.split('\t')[1]);
      else if (state.startsWith('ready\t') || state.startsWith('error\t')) {
        terminal = state;
        clearInterval(timer);
        settle(state);
      }
      return state;
    } catch (error) { clearInterval(timer); fail(error); throw error; }
  };
  probe.start(clock(), timeout);
  pump();
  if (terminal === undefined) timer = setInterval(pump, 2);
  return { host, result, pump, stop: () => clearInterval(timer) };
}
