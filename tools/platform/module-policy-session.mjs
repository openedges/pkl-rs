// Transport validation only. Trust levels and bundled-key admission stay in Rust.
const context = ['originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'];
const fields = new Set(['id', 'kind', ...context, 'resolvedUri', 'sourceUri']);

export function completeDefaultModuleRequest(session, event) {
  const request = event?.module;
  if (event?.kind !== 'module-request' || typeof event.token !== 'string' || !event.token.length
      || typeof request?.id !== 'string' || !/^[0-9]+$/.test(request.id)
      || !['check-import', 'resolve'].includes(request.kind)
      || context.some(name => typeof request[name] !== 'string')
      || Object.keys(request).some(name => !fields.has(name))
      || request.resolvedUri != null || request.sourceUri != null)
    throw Error('invalid default-policy module request context');
  const completed = session.completeDefaultModule(event.token, 0);
  if (typeof completed !== 'boolean') throw Error('invalid default module completion result');
  return completed;
}
