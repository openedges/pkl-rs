// Host-only transport shared by Node and Chromium. Rendering stays in the VM.
export function runPcfStringOutputCase(api, fixture, uri, budget = 7) {
  let session, completion = null, retainedOutput = null;
  const suppliedUnits = Uint16Array.from(fixture.valueUnits);
  const architecture = { valueReads: 0, configuredReads: [], collections: 0,
    terminalSlots: null, ownedBytesStable: true, inputUnitsStable: true };
  let acquisitionError = null;
  try {
    if (typeof api?.SourceSession?.forOutput !== 'function') throw Error('SourceSession.forOutput is unavailable');
    session = api.SourceSession.forOutput(fixture.source, uri, 'stage40-pcf-string-output', [],
      ['eval', '--no-cache', '--no-project', '--settings', 'pkl:settings', 'stage40-pcf-string-output.pkl'],
      'COMPACT', false, false, false);
    session.startOutputBytes(0);
    for (let polls = 0; polls < 100_000; polls++) {
      const event = session.poll(0, budget);
      session.collect();
      architecture.collections++;
      if (event.kind === 'yield') continue;
      if (event.kind === 'read') {
        if (event.uri === 'life:value') {
          architecture.valueReads++;
          session.respondString(event.token, suppliedUnits, 0);
        } else if (event.uri.startsWith('prop:') || event.uri.startsWith('env:')) {
          architecture.configuredReads.push(event.uri);
          session.respondConfigured(event.token, 0);
        } else throw Error('unexpected resource URI: ' + event.uri);
        continue;
      }
      if (event.kind === 'bytes') {
        if (!(event.bytes instanceof Uint8Array)) throw Error('output bytes must be Uint8Array');
        retainedOutput = event.bytes;
        completion = { kind: 'bytes', stdout: Array.from(event.bytes), stderr: [] };
      } else if (event.kind === 'error') {
        if (!(event.errorBytes instanceof Uint8Array)) throw Error('output diagnostics must be Uint8Array');
        retainedOutput = event.errorBytes;
        completion = { kind: 'error', stdout: [], stderr: Array.from(event.errorBytes) };
      } else throw Error('unexpected output event: ' + event.kind);
      break;
    }
    if (!completion) throw Error('output instruction polling bound exceeded');
  } catch (error) {
    acquisitionError = { message: String(error?.message ?? error), stack: error?.stack ?? null };
  } finally {
    if (session) {
      try {
        session.close();
        session.collect();
        architecture.terminalSlots = session.heapSlots();
        if (typeof session.free === 'function') session.free();
        const expected = completion?.kind === 'bytes' ? completion.stdout : completion?.stderr;
        if (retainedOutput && (retainedOutput.length !== expected.length
            || expected.some((byte, index) => byte !== retainedOutput[index]))) architecture.ownedBytesStable = false;
        architecture.inputUnitsStable = suppliedUnits.length === fixture.valueUnits.length
          && fixture.valueUnits.every((unit, index) => unit === suppliedUnits[index]);
      } catch (error) {
        const cleanup = { message: String(error?.message ?? error), stack: error?.stack ?? null };
        acquisitionError = acquisitionError ? { ...acquisitionError, cleanup } : cleanup;
      }
    }
  }
  return { completion, architecture, acquisitionError };
}

export function runPcfStringOutputFixtures(api, fixtures, uri, repetitions = 2) {
  return fixtures.flatMap(fixture => Array.from({ length: repetitions }, (_, repeat) => ({
    id: fixture.id, repeat, ...runPcfStringOutputCase(api, fixture, uri),
  })));
}
