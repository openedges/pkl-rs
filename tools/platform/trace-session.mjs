// The same host driver runs against native N-API and browser Wasm SourceSession.
// Values, error rendering, caches and continuations remain in the shared engine.
export function runTraceCase(api, fixture, budget = 32) {
  const payloads = {}, protocol = [], retained = [];
  const architecture = { collections: 0, logs: 0, reads: 0, terminalSlots: null,
    sessionCreated: false, setupFailed: false, retainedEventBytesStable: true };
  let session, loggerDepth = 0, readerDepth = 0;
  const units = text => Uint16Array.from({ length: text.length }, (_, index) => text.charCodeAt(index));
  // Java UTF-8 capture replaces isolated UTF-16 surrogates with '?'. Raw units
  // are captured independently, so this encoding boundary cannot hide loss.
  const javaUtf8 = value => {
    const bytes = [];
    const input = typeof value === 'string' ? units(value) : value;
    for (let index = 0; index < input.length; index++) {
      let point = input[index];
      if (point >= 0xD800 && point <= 0xDBFF) {
        const low = input[index + 1];
        if (low >= 0xDC00 && low <= 0xDFFF) {
          point = 0x10000 + ((point - 0xD800) << 10) + low - 0xDC00;
          index++;
        } else { bytes.push(0x3F); continue; }
      } else if (point >= 0xDC00 && point <= 0xDFFF) { bytes.push(0x3F); continue; }
      if (point < 0x80) bytes.push(point);
      else if (point < 0x800) bytes.push(0xC0 | (point >> 6), 0x80 | (point & 63));
      else if (point < 0x10000) bytes.push(0xE0 | (point >> 12), 0x80 | ((point >> 6) & 63), 0x80 | (point & 63));
      else bytes.push(0xF0 | (point >> 18), 0x80 | ((point >> 12) & 63), 0x80 | ((point >> 6) & 63), 0x80 | (point & 63));
    }
    return bytes;
  };
  const write = (name, value) => { payloads[name] = javaUtf8(value); };
  const writeUnits = (name, value) => {
    if (!(value instanceof Uint16Array)) throw Error('expected lossless Uint16Array event field: ' + name);
    const bytes = [];
    for (const unit of value) bytes.push(unit >> 8, unit & 255);
    payloads[name] = bytes;
    retained.push({ value, expected: Array.from(value) });
  };
  const saveLog = (index, log) => {
    if (!Object.hasOwn(log, 'memberName') || (log.memberName !== null && !(log.memberName instanceof Uint16Array)))
      throw Error('memberName must be explicit null or Uint16Array');
    const prefix = `log-${index}`;
    write(prefix + '.level.bin', log.level);
    write(prefix + '.message.bin', log.message);
    writeUnits(prefix + '.message.code-units.bin', log.message);
    write(prefix + '.message.length.bin', String(log.message.length));
    write(prefix + '.uri.bin', log.moduleUri);
    writeUnits(prefix + '.uri.code-units.bin', log.moduleUri);
    write(prefix + '.member.bin', log.memberName === null ? 'null' : log.memberName);
    write(prefix + '.member.null.bin', String(log.memberName === null));
    writeUnits(prefix + '.member.code-units.bin', log.memberName === null ? new Uint16Array() : log.memberName);
    write(prefix + '.position.bin', [log.startLine, log.startColumn, log.endLine, log.endColumn].join(':'));
    log.sourceLines.forEach((line, index) => {
      write(prefix + `.source-${index}.bin`, line);
      writeUnits(prefix + `.source-${index}.code-units.bin`, line);
    });
  };
  const paths = [...fixture.expressions];
  const add = path => path ? paths.push(path) - 1 : null;
  const loggerPath = add(fixture.logger_callback);
  const readerPath = add(fixture.reader_callback);
  const afterClosePath = fixture.action === 'close-reenter' ? add('stable') : null;
  const errorEvent = error => ({ kind: 'error', error: units(error instanceof Error ? error.message : String(error)) });
  const saveResult = (prefix, event) => {
    if (!['value', 'error'].includes(event.kind)) throw Error('expected completed expression, got ' + event.kind);
    if (!((event.kind === 'value' ? event.value : event.error) instanceof Uint16Array))
      throw Error('completion must contain lossless UTF-16 units');
    const status = event.kind === 'value' ? 'success' : 'error';
    write(prefix + '.status.bin', status + '\n');
    write(prefix + '.stdout.bin', event.kind === 'value' ? event.value : '');
    write(prefix + '.stderr.bin', event.kind === 'error' ? event.error : '');
    protocol.push(`result=${prefix} status=${status}`);
  };
  function capture(index, prefix, parent = null) {
    let event;
    try {
      if (parent == null) session.startPath(index, 0);
      else session.reenterPath(parent, index, 0);
    } catch (error) { event = errorEvent(error); }
    // Transport failures while driving an active evaluation are infrastructure
    // failures, not fabricated language-error completions.
    if (!event) event = evaluate();
    saveResult(prefix, event);
  }
  function evaluate() {
    for (let polls = 0; polls < 100_000; polls++) {
      const event = session.poll(0, budget);
      session.collect();
      architecture.collections++;
      if (event.kind === 'yield') continue;
      if (['module', 'value', 'error'].includes(event.kind)) return event;
      if (event.kind === 'read') {
        const index = architecture.reads++;
        protocol.push(`read-enter=${index} uri=${event.uri}`);
        readerDepth++;
        try {
          if (readerDepth === 1 && readerPath != null) capture(readerPath, `read-callback-${index}`, event.token);
        } finally { readerDepth--; }
        protocol.push(`read-exit=${index}`);
        session.respondString(event.token, units('resource'), 0);
      } else if (event.kind === 'log') {
        const index = architecture.logs++;
        protocol.push(`log-enter=${index} depth=${loggerDepth}`);
        saveLog(index, event.log);
        loggerDepth++;
        try {
          if (loggerDepth === 1 && loggerPath != null) capture(loggerPath, `log-callback-${index}`, event.token);
          if (fixture.action.startsWith('close')) session.closeFromCallback(event.token);
          if (fixture.action === 'close-reenter') capture(afterClosePath, `post-close-callback-${index}`, event.token);
          session.acknowledgeLog(event.token,
            fixture.action === 'throw' ? units('stage37-logger-failure') : null, 0);
        } finally { loggerDepth--; }
        if (fixture.action !== 'throw') protocol.push(`log-exit=${index}`);
      } else throw Error('unexpected source event: ' + event.kind);
    }
    throw Error('source logger instruction budget exceeded');
  }
  let acquisitionError = null;
  try {
    if (typeof api.SourceSession !== 'function') throw Error('SourceSession constructor is unavailable');
    for (const name of ['startModule', 'startPath', 'reenterPath', 'poll', 'collect', 'respondString',
      'acknowledgeLog', 'closeFromCallback', 'close', 'heapSlots', 'setupFailed']) {
      if (typeof api.SourceSession.prototype[name] !== 'function') throw Error('SourceSession method is unavailable: ' + name);
    }
    let compileError;
    try {
      session = new api.SourceSession(fixture.sourceText, 'file:///stage37-trace.pkl', 'stage37-trace',
        paths, fixture.mode, fixture.action !== 'noop', false, false);
      architecture.sessionCreated = true;
    } catch (error) {
      // The current development constructor returns a rendered Pkl diagnostic.
      // Missing bindings, JS TypeErrors and traps are not source diagnostics.
      if (!(error instanceof Error) || !error.message.startsWith('–– Pkl Error ––\n')) throw error;
      compileError = errorEvent(error);
      architecture.setupFailed = true;
    }
    if (compileError) {
      fixture.expressions.forEach((_, index) => saveResult(`outer-${index}`, compileError));
    } else {
      fixture.expressions.forEach((_, index) => {
        // Module setup and the first expression belong to one admitted call.
        // Callback close must allow that existing call to finish.
        capture(index, `outer-${index}`);
        if (index === 0) architecture.setupFailed = session.setupFailed();
      });
    }
    protocol.push(`logs=${architecture.logs} reads=${architecture.reads}`);
  } catch (error) {
    acquisitionError = { message: String(error?.message ?? error), stack: error?.stack ?? null };
  } finally {
    if (session) {
      try {
        session.close();
        architecture.terminalSlots = session.heapSlots();
        for (const { value, expected } of retained) {
          if (value.length !== expected.length || expected.some((unit, index) => unit !== value[index]))
            architecture.retainedEventBytesStable = false;
        }
        if (typeof session.free === 'function') session.free();
      } catch (error) {
        const cleanup = { message: String(error?.message ?? error), stack: error?.stack ?? null };
        acquisitionError = acquisitionError ? { ...acquisitionError, cleanup } : cleanup;
      }
    }
    write('protocol.bin', protocol.length ? protocol.join('\n') + '\n' : '');
    write('process-stderr.bin', '');
  }
  return { payloads, architecture, acquisitionError };
}
