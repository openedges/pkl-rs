// Exact expression session transport. This Host supplies captured files only;
// request traces are candidate observations, never synthesized Java events.
import { completeDefaultModuleRequest } from './module-policy-session.mjs';

const errorRecord = error => ({ message: String(error?.message ?? error), stack: error?.stack ?? null });
const textUnits = text => Array.from({ length: text.length }, (_, index) => text.charCodeAt(index));
const sameBytes = (left, right) => left.length === right.length && left.every((byte, index) => byte === right[index]);

export function runParameterizedCase(api, fixture, budget = 17) {
  const result = { id: fixture.id, exitCode: 2, disposition: 'adapter-failure', phase: null,
    reason: null, stdoutUnits: [], stderrUnits: [], trace: [], acquisitionError: null,
    constructorClassified: false,
    architecture: { sessionConstructed: false, polls: 0, collections: 0, terminalSlots: null, terminalDepth: null, ownedResultStable: false } };
  let session, retained;
  try {
    if (fixture.powerAssertions !== false || typeof fixture.expression !== 'string'
        || !Number.isSafeInteger(budget) || budget < 1) throw Error('invalid exact expression profile');
    const resources = new Map();
    for (const resource of fixture.resources) {
      if (resources.has(resource.uri) || typeof resource.uri !== 'string' || !Array.isArray(resource.bytes)
          || resource.bytes.some(value => !Number.isInteger(value) || value < 0 || value > 255))
        throw Error('invalid exact resource table');
      resources.set(resource.uri, resource.bytes);
    }
    if (typeof api?.SourceSession?.forExpressions !== 'function') throw Error('SourceSession.forExpressions is unavailable');
    session = api.SourceSession.forExpressions(fixture.source, fixture.uri, fixture.moduleName,
      [fixture.expression], 'compact', false, false, false);
    result.architecture.sessionConstructed = true;
    session.startPath(0, 0, null);
    let settled = false;
    while (++result.architecture.polls <= 200_000) {
      const event = session.poll(0, budget);
      session.collect(); result.architecture.collections++;
      if (event.kind === 'yield') continue;
      if (event.kind === 'module-request') {
        result.trace.push({ kind: 'module-request', token: event.token, module: { ...event.module } });
        if (!completeDefaultModuleRequest(session, event)) throw Error('unconfigured default-policy module request');
        continue;
      }
      if (event.kind === 'read') {
        const uri = session.resolvedReadUri(event.token);
        result.trace.push({ kind: 'read', token: event.token, requestedUri: event.uri, resolvedUri: uri });
        if (!resources.has(uri)) throw Error('unconfigured exact resource URI: ' + uri);
        const bytes = Uint8Array.from(resources.get(uri));
        session.completeReadResource(event.token, uri, bytes, 0);
        bytes.fill(0);
        continue;
      }
      if (event.kind !== 'value' && event.kind !== 'error') throw Error('unexpected session event: ' + event.kind);
      const view = event.kind === 'value' ? event.value : event.error;
      if (!(view instanceof Uint16Array)) throw Error('expected owned UTF16 terminal channel');
      retained = { view, units: Array.from(view) };
      result[event.kind === 'value' ? 'stdoutUnits' : 'stderrUnits'] = retained.units;
      result.exitCode = event.kind === 'value' ? 0 : 1;
      result.disposition = event.kind === 'value' ? null : event.disposition ?? null;
      result.phase = event.phase ?? null;
      result.reason = event.reason ?? null;
      result.trace.push({ kind: event.kind, disposition: result.disposition, phase: result.phase, reason: result.reason });
      settled = true;
      break;
    }
    if (!settled) throw Error('expression session polling bound exceeded');
  } catch (error) {
    result.acquisitionError = errorRecord(error);
    result.exitCode = 2;
    result.disposition = 'adapter-failure';
    if (!result.stderrUnits.length) result.stderrUnits = textUnits(result.acquisitionError.message);
    if (!session && typeof api?.evaluateModuleFrontend === 'function') {
      // This classification pass does not execute the guest expression. Admit
      // it only when the complete original constructor diagnostic survives a
      // strict UTF8/UTF16 identity comparison; never infer a disposition from text.
      try {
        const frontend = api.evaluateModuleFrontend(fixture.source, fixture.uri, fixture.moduleName,
          ['eval', '--no-project', fixture.uri]);
        const diagnostic = frontend?.stderr instanceof Uint8Array
          ? textUnits(new TextDecoder('utf-8', { fatal: true }).decode(frontend.stderr)) : [];
        const exact = diagnostic.length === result.stderrUnits.length
          && diagnostic.every((unit, index) => unit === result.stderrUnits[index]);
        if (frontend.exitCode !== 0 && exact && ['language-error', 'unsupported', 'limit-exceeded', 'internal-failure'].includes(frontend.disposition)) {
          result.constructorClassified = true;
          result.exitCode = frontend.disposition === 'language-error' ? 1 : 2;
          result.disposition = frontend.disposition;
          result.phase = frontend.phase;
          result.reason = frontend.reason ?? null;
          result.trace.push({ kind: 'constructor-error', disposition: result.disposition, phase: result.phase, reason: result.reason });
          result.acquisitionError = null;
        }
      } catch (classificationError) {
        result.trace.push({ kind: 'constructor-classification-failed', error: errorRecord(classificationError) });
      }
    }
  } finally {
    if (session) {
      try {
        session.close(); session.collect(); result.architecture.collections++;
        result.architecture.terminalSlots = session.heapSlots();
        result.architecture.terminalDepth = session.contextDepth();
        session.free?.();
        result.architecture.ownedResultStable = retained != null
          && retained.units.length === retained.view.length
          && retained.units.every((unit, index) => unit === retained.view[index]);
      } catch (error) {
        result.acquisitionError = { ...result.acquisitionError, cleanup: errorRecord(error) };
      }
    }
  }
  return result;
}

// This is Java's UTF8 writer boundary, not text normalization. Preserve the
// original candidate UTF16 channel independently in the acquisition record.
export function javaUtf8(units) {
  let value = '';
  for (let index = 0; index < units.length; index++) {
    const unit = units[index], next = units[index + 1];
    if (unit >= 0xd800 && unit <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
      value += String.fromCharCode(unit, next); index++;
    } else value += unit >= 0xd800 && unit <= 0xdfff ? '?' : String.fromCharCode(unit);
  }
  return Array.from(new TextEncoder().encode(value));
}

export function compareParameterizedPayload(expected, actual) {
  const noSessionError = actual.constructorClassified === true && actual.exitCode === 1
    && actual.disposition === 'language-error' && actual.architecture.sessionConstructed === false
    && actual.architecture.terminalSlots === null && actual.architecture.terminalDepth === null;
  const checks = {
    constructorShape: !actual.constructorClassified || noSessionError,
    exitCode: actual.exitCode === expected.exitCode,
    stdout: sameBytes(javaUtf8(actual.stdoutUnits), expected.stdout),
    stderr: sameBytes(javaUtf8(actual.stderrUnits), expected.stderr),
    acquisition: actual.acquisitionError == null,
    disposition: actual.exitCode === 0 || (actual.exitCode === 1 && actual.disposition === 'language-error'),
    terminalSlots: noSessionError || actual.architecture.terminalSlots === 0,
    terminalDepth: noSessionError || actual.architecture.terminalDepth === 0,
    ownedResultStable: noSessionError || actual.architecture.ownedResultStable === true,
  };
  return { status: Object.values(checks).every(Boolean) ? 'pass'
    : actual.acquisitionError != null ? 'acquisition-failed'
    : !checks.disposition ? 'non-language-failure' : 'mismatch', checks };
}
