// Browser-safe capability adapter. The engine prepares import URIs, owns caches,
// compiles sources and evaluates modules; this Host performs exact bundle lookups.
class TransportError extends Error {}
const failureKinds = new Set(['not-found', 'io', 'denied', 'unsupported']);
function failure(entry) {
  if (!entry || !failureKinds.has(entry.kind) || !Object.hasOwn(entry, 'message')
      || (typeof entry.message !== 'string' && !(entry.kind === 'io' && entry.message === null)))
    throw Error('invalid module Host failure');
  return entry;
}
function resolution(entry) {
  if (!entry || typeof entry.importUri !== 'string') throw Error('invalid module resolution');
  if (entry.resolves != null) {
    if (!Array.isArray(entry.resolves) || !entry.resolves.length) throw Error('empty module resolve plan');
    for (const leaf of entry.resolves) {
      if (leaf.resolves != null) throw Error('nested module resolve plans are invalid');
      resolution({...leaf, importUri: entry.importUri});
    }
  } else if (entry.failure) failure(entry.failure);
  else if (typeof entry.resolvedUri !== 'string' || typeof entry.sourceUri !== 'string')
    throw Error('invalid resolved/source identity');
  return entry;
}
function source(entry) {
  if (!entry || typeof entry.resolvedUri !== 'string') throw Error('invalid module source');
  if (entry.loads != null) {
    if (!Array.isArray(entry.loads) || !entry.loads.length) throw Error('empty module source completion plan');
    for (const leaf of entry.loads) {
      if (leaf.loads != null) throw Error('nested module source completion plans are invalid');
      source({ ...leaf, resolvedUri: entry.resolvedUri });
    }
  } else if (entry.failure) failure(entry.failure);
  else if (typeof entry.text !== 'string' || typeof entry.inferredName !== 'string')
    throw Error('invalid source text/name');
  if (entry.closeOnLoad != null && typeof entry.closeOnLoad !== 'boolean') throw Error('invalid source close flag');
  if (entry.callbackPaths != null && (!Array.isArray(entry.callbackPaths)
      || entry.callbackPaths.some(index => !Number.isSafeInteger(index) || index < 0)))
    throw Error('invalid source callback paths');
  if (entry.callbackTimes != null && (!Number.isSafeInteger(entry.callbackTimes) || entry.callbackTimes < 0))
    throw Error('invalid source callback count');
  return entry;
}
function resource(entry) {
  if (!entry || typeof entry.uri !== 'string') throw Error('invalid resource URI');
  if (entry.reads != null) {
    if (!Array.isArray(entry.reads) || !entry.reads.length) throw Error('empty resource read plan');
    for (const leaf of entry.reads) {
      if (leaf.reads != null) throw Error('nested resource read plans are invalid');
      resource({...leaf, uri: entry.uri});
    }
  } else {
    if (Object.hasOwn(entry, 'failure') === Object.hasOwn(entry, 'valueUnits'))
      throw Error('resource must provide one completion');
    if (entry.failure) {
      const f = entry.failure;
      if (!Object.hasOwn(f, 'message') || !['io','diagnostic'].includes(f.kind)
          || (typeof f.message !== 'string' && !(f.kind === 'io' && f.message === null)))
        throw Error('invalid resource failure');
    } else if (!Array.isArray(entry.valueUnits)
        || entry.valueUnits.some(unit => !Number.isInteger(unit) || unit < 0 || unit > 65535))
      throw Error('invalid resource UTF16 code unit');
  }
  return entry;
}
function table(entries, validate, key) {
  const result = new Map();
  for (const entry of entries ?? []) {
    validate(entry);
    if (result.has(entry[key])) throw Error('duplicate bundle identity: ' + entry[key]);
    result.set(entry[key], entry);
  }
  return result;
}
function units(value, name) {
  if (!(value instanceof Uint16Array)) throw new TransportError(name + ' must be Uint16Array');
  return Array.from(value);
}
function bytes(value, name) {
  if (!(value instanceof Uint8Array)) throw new TransportError(name + ' must be Uint8Array');
  return Array.from(value);
}
function thrown(error) { return { message: String(error?.message ?? error), stack: error?.stack ?? null }; }

export function runModuleLoaderCase(api, fixture, budget = 7) {
  let session;
  const results = [], requests = [], reads = [], callbacks = [], logs = [], retained = [];
  const architecture = { collections: 0, terminalSlots: null, ownedResultsStable: true,
    cancelledModules: [], cancelledReads: [] };
  let acquisitionError = null, polls = 0;
  try {
    if (!Number.isSafeInteger(budget) || budget <= 0) throw Error('invalid polling budget');
    const resolutions = table(fixture.resolutions, resolution, 'importUri');
    const sources = table(fixture.sources, source, 'resolvedUri');
    const resources = table(fixture.resources, resource, 'uri');
    const callbackCounts = new Map(), loadCounts = new Map(), resolveCounts = new Map(), readCounts = new Map();
    const paths = fixture.paths ?? [];
    if (fixture.logger && (!Array.isArray(fixture.logger.callbackPaths ?? [])
        || (fixture.logger.callbackPaths ?? []).some(index => !Number.isSafeInteger(index) || index < 0)))
      throw Error('invalid logger callback paths');
    if (!Array.isArray(fixture.actions) || fixture.actions.length === 0) throw Error('empty module actions');
    if (fixture.operation === 'output-bytes') {
      session = api.SourceSession.forOutput(fixture.source, fixture.uri, fixture.moduleName, paths,
        fixture.arguments, 'COMPACT', fixture.logger != null, false, fixture.powerAssertions ?? false);
    } else if (fixture.operation == null || fixture.operation === 'expressions') {
      if (typeof api.SourceSession.forExpressions !== 'function') throw new TransportError('parsed expression session binding is unavailable');
      session = api.SourceSession.forExpressions(fixture.source, fixture.uri, fixture.moduleName, paths,
        'COMPACT', fixture.logger != null, false, fixture.powerAssertions ?? false);
    } else throw Error('unknown module operation');
    const remember = (view, expected) => { retained.push({ view, expected }); return expected; };
    const metadata = event => {
      const output = fixture.operation === 'output-bytes';
      if (!output && event.kind !== 'error') return {};
      const dispositions = { 1: 'language-error', 2: 'unsupported', 3: 'limit-exceeded', 4: 'internal-failure' };
      if (event.kind === 'error' && !Object.values(dispositions).includes(event.disposition))
        throw new TransportError('invalid shared failure disposition');
      if (output && (!Number.isInteger(event.exitCode)
          || (event.kind === 'bytes' ? event.exitCode !== 0 || event.disposition != null
            : ![1, 2, 3, 4].includes(event.exitCode) || event.disposition !== dispositions[event.exitCode])))
        throw new TransportError('invalid shared module completion metadata');
      if (typeof event.phase !== 'string' || (event.reason != null && typeof event.reason !== 'string'))
        throw new TransportError('invalid shared module completion metadata');
      return { ...(output ? { exitCode: event.exitCode } : {}), phase: event.phase,
        disposition: event.disposition ?? null, reason: event.reason ?? null,
        dispositionPresent: Object.hasOwn(event, 'disposition'), reasonPresent: Object.hasOwn(event, 'reason'),
        dispositionKind: !Object.hasOwn(event, 'disposition') ? 'absent' : event.disposition === null ? 'null' : typeof event.disposition,
        reasonKind: !Object.hasOwn(event, 'reason') ? 'absent' : event.reason === null ? 'null' : typeof event.reason };
    };
    const settle = depth => {
      if (depth > 32) throw new TransportError('module callback depth bound exceeded');
      while (++polls <= 1_000_000) {
        const event = session.poll(0, budget);
        session.collect(); architecture.collections++;
        if (event.kind === 'yield') continue;
        if (event.kind === 'module-request') {
          const request = event.module;
          if (typeof event.token !== 'string' || !request || typeof request.id !== 'string'
              || !/^[0-9]+$/.test(request.id) || typeof request.originalUri !== 'string')
            throw new TransportError('invalid typed module request');
          requests.push({ token: event.token, ...request });
          if (request.kind === 'resolve') {
            if (typeof request.importUri !== 'string' || typeof request.importerOriginalUri !== 'string'
                || typeof request.importerResolvedUri !== 'string') throw new TransportError('incomplete resolver context');
            let entry = resolutions.get(request.importUri);
            if (entry?.resolves) {
              const index = resolveCounts.get(request.importUri) ?? 0;
              if (index >= entry.resolves.length) throw new TransportError('module resolve plan exhausted');
              resolveCounts.set(request.importUri, index + 1);
              entry = {...entry, ...entry.resolves[index], resolves: undefined};
            }
            if (!entry) session.rejectModuleResolution(event.token, 'not-found', '', 0);
            else if (entry.originalUri != null && entry.originalUri !== request.originalUri)
              throw new TransportError('module original identity differs from fixture');
            else if (entry.failure) session.rejectModuleResolution(event.token, entry.failure.kind, entry.failure.message, 0);
            else session.completeModuleResolution(event.token, request.originalUri, entry.resolvedUri, entry.sourceUri, 0);
          } else if (request.kind === 'load-source') {
            if (typeof request.resolvedUri !== 'string' || typeof request.sourceUri !== 'string')
              throw new TransportError('incomplete source context');
            let entry = sources.get(request.resolvedUri);
            let callbackKey = entry;
            if (entry?.loads) {
              const index = loadCounts.get(request.resolvedUri) ?? 0;
              if (index >= entry.loads.length) throw new TransportError('module source completion plan exhausted');
              loadCounts.set(request.resolvedUri, index + 1);
              callbackKey = entry.loads[index];
              entry = { ...entry, ...entry.loads[index], loads: undefined, resolvedUri: request.resolvedUri };
            }
            if (!entry) session.rejectModuleSource(event.token, 'not-found', '', 0);
            else {
              if (entry.sourceUri != null && entry.sourceUri !== request.sourceUri)
                throw new TransportError('module source identity differs from fixture');
              const count = callbackCounts.get(callbackKey) ?? 0;
              if (entry.callbackPaths && count < (entry.callbackTimes ?? 1)) {
                callbackCounts.set(callbackKey, count + 1);
                for (const index of entry.callbackPaths) {
                  session.reenterPath(event.token, index, 0);
                  const callback = { token: event.token, path: index, result: null };
                  callbacks.push(callback);
                  callback.result = settle(depth + 1);
                }
              }
              if (entry.closeOnLoad) session.closeFromCallback(event.token);
              if (entry.failure) session.rejectModuleSource(event.token, entry.failure.kind, entry.failure.message, 0);
              else session.completeModuleSource(event.token, entry.text, entry.inferredName, 0);
            }
          } else throw new TransportError('unknown module request kind');
          continue;
        }
        if (event.kind === 'log') {
          if (!fixture.logger || typeof event.token !== 'string' || !event.log) throw new TransportError('unconfigured logger event');
          const log = event.log;
          if (!Object.hasOwn(log, 'memberName') || (log.memberName !== null && !(log.memberName instanceof Uint16Array))
              || !Array.isArray(log.sourceLines) || typeof log.level !== 'string'
              || ['startLine','startColumn','endLine','endColumn'].some(key => !Number.isSafeInteger(log[key]) || log[key] < 0))
            throw new TransportError('invalid logger frame metadata');
          logs.push({ token: event.token, level: log.level, message: units(log.message, 'log message'),
            moduleUri: units(log.moduleUri, 'log module URI'),
            memberName: log.memberName === null ? null : units(log.memberName, 'log member name'),
            sourceLines: log.sourceLines.map(line => units(line, 'log source line')),
            startLine: log.startLine, startColumn: log.startColumn, endLine: log.endLine, endColumn: log.endColumn });
          for (const index of fixture.logger.callbackPaths ?? []) {
            session.reenterPath(event.token, index, 0);
            const callback = { token: event.token, path: index, result: null };
            callbacks.push(callback);
            callback.result = settle(depth + 1);
          }
          if (fixture.logger.close) session.closeFromCallback(event.token);
          const failure = fixture.logger.failureUnits;
          if (failure != null && (!Array.isArray(failure) || failure.some(unit => !Number.isInteger(unit) || unit < 0 || unit > 65535)))
            throw new TransportError('invalid logger failure UTF16 units');
          session.acknowledgeLog(event.token, failure == null ? undefined : Uint16Array.from(failure), 0);
          continue;
        }
        if (event.kind === 'read') {
          if (typeof event.uri !== 'string' || typeof event.token !== 'string')
            throw new TransportError('invalid resource request');
          reads.push({uri: event.uri, token: event.token});
          let entry = resources.get(event.uri);
          if (entry?.reads) {
            const index = readCounts.get(event.uri) ?? 0;
            if (index >= entry.reads.length) throw new TransportError('resource read plan exhausted');
            readCounts.set(event.uri, index + 1);
            entry = {...entry, ...entry.reads[index], reads: undefined};
          }
          if (entry?.failure) {
            if (entry.failure.kind === 'io') session.rejectReadIo(event.token, entry.failure.message, 0);
            else session.reject(event.token, entry.failure.message, 0);
          } else if (entry) session.respondString(event.token, Uint16Array.from(entry.valueUnits), 0);
          else if (event.uri.startsWith('prop:') || event.uri.startsWith('env:')) session.respondConfigured(event.token, 0);
          else throw new TransportError('unconfigured resource: ' + event.uri);
          continue;
        }
        if (event.kind === 'value') return { kind: 'value', valueUnits: remember(event.value, units(event.value, 'value')) };
        if (event.kind === 'bytes') return { ...metadata(event), kind: 'bytes', bytes: remember(event.bytes, bytes(event.bytes, 'bytes')) };
        if (event.kind === 'error') {
          if (event.errorBytes != null) return { ...metadata(event), kind: 'error', errorBytes: remember(event.errorBytes, bytes(event.errorBytes, 'errorBytes')) };
          return { ...metadata(event), kind: 'error', errorUnits: remember(event.error, units(event.error, 'error')) };
        }
        throw new TransportError('unexpected module session event: ' + event.kind);
      }
      throw new TransportError('module polling bound exceeded');
    };
    for (let index = 0; index < fixture.actions.length; index++) {
      const action = fixture.actions[index];
      if (action.kind === 'set-source') { const entry = source(action.source); sources.set(entry.resolvedUri, entry); loadCounts.delete(entry.resolvedUri); continue; }
      if (action.kind === 'set-resolution') { const entry = resolution(action.resolution); resolutions.set(entry.importUri, entry); resolveCounts.delete(entry.importUri); continue; }
      if (action.kind === 'close') { session.close(); continue; }
      if (action.kind !== 'path' && action.kind !== 'output-bytes') throw Error('unknown module action');
      try {
        if (action.kind === 'path') session.startPath(action.index, 0);
        else session.startOutputBytes(0);
        results.push({ actionIndex: index, ...settle(0) });
      } catch (error) {
        if (error instanceof TransportError) throw error;
        results.push({ actionIndex: index, kind: 'thrown', error: thrown(error) });
      }
    }
  } catch (error) { acquisitionError = thrown(error); }
  finally {
    if (session) {
      try {
        session.close(); session.collect();
        architecture.terminalSlots = session.heapSlots();
        architecture.cancelledModules = session.drainCancelledModules();
        architecture.cancelledReads = session.drainCancelledReads();
        if (typeof session.free === 'function') session.free();
        architecture.ownedResultsStable = retained.every(({ view, expected }) => view.length === expected.length
          && expected.every((unit, index) => unit === view[index]));
      } catch (error) { acquisitionError = acquisitionError ? { ...acquisitionError, cleanup: thrown(error) } : thrown(error); }
    }
  }
  return { id: fixture.id, results, requests, reads, callbacks, logs, architecture, acquisitionError };
}
