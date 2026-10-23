// Thin constructor transport probe: no Pkl parsing, source rewriting or evaluation.
export function stringUnits(text) {
  if (typeof text !== 'string') throw Error('expected JavaScript string');
  return Array.from({ length: text.length }, (_, index) => text.charCodeAt(index));
}
export function runLosslessConstructorCase(api, fixture, factory) {
  if (!['constructor', 'expressions', 'output'].includes(factory)) throw Error('unknown factory');
  let session;
  try {
    const args = [fixture.source, fixture.uri, fixture.moduleName, fixture.paths,
      'compact', false, false, fixture.powerAssertions ?? false];
    if (factory === 'constructor') session = new api.SourceSession(...args);
    else if (factory === 'expressions') session = api.SourceSession.forExpressions(...args);
    else if (factory === 'output') session = api.SourceSession.forOutput(
      fixture.source, fixture.uri, fixture.moduleName, fixture.paths,
      fixture.arguments, 'compact', false, false, fixture.powerAssertions ?? false);
  } catch (error) {
    return {
      id: fixture.id, factory, kind: 'rejected', isError: error instanceof Error,
      name: typeof error?.name === 'string' ? error.name : null,
      messageUnits: typeof error?.message === 'string' ? stringUnits(error.message) : null,
      thrownType: typeof error,
    };
  }
  const cleanupErrors = [];
  for (const method of ['close', 'collect', 'free']) {
    try { if (typeof session?.[method] === 'function') session[method](); }
    catch (error) { cleanupErrors.push({ method, message: String(error) }); }
  }
  return { id: fixture.id, factory, kind: 'constructed', cleanupErrors };
}
export function compareConstructorResult(fixture, actual) {
  const expected = fixture.expectedMessageUnits;
  const unitsMatch = Array.isArray(expected) && Array.isArray(actual?.messageUnits)
    && expected.length === actual.messageUnits.length
    && expected.every((unit, index) => unit === actual.messageUnits[index]);
  const channels = {
    rejected: actual?.kind === 'rejected', errorObject: actual?.isError === true,
    messageUnits: unitsMatch, id: actual?.id === fixture.id,
  };
  return { status: Object.values(channels).every(Boolean) ? 'pass' : 'mismatch', channels };
}
