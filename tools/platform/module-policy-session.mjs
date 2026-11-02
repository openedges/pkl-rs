// Transport validation only. Trust, factory selection order and caches stay in Rust.
const uriFields = ['originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri',
  'resolvedUri', 'sourceUri'];
const keyFields = ['keyId', 'keyKind', 'keyHandle', 'resolvedKeyId', 'resolvedKeyHandle', 'selectionPhase', 'resolutionPhase'];
const fields = new Set(['id', 'kind', ...uriFields, ...keyFields]);
const contexts = {
  'check-import': ['originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'],
  'select-key': ['originalUri'],
  'query-cache-policy': ['originalUri'],
  'query-fragment-paths': ['originalUri'],
  capabilities: ['originalUri', 'importerOriginalUri', 'importerResolvedUri'],
  'has-element': ['originalUri', 'importerOriginalUri', 'importerResolvedUri'],
  resolve: ['originalUri', 'importUri', 'importerOriginalUri', 'importerResolvedUri'],
  'resolve-key': ['originalUri'],
  'resolve-import-uri': ['originalUri', 'importUri'],
  'load-source': ['originalUri', 'resolvedUri', 'sourceUri'],
};
const keyKinds = new Set(['standard-library', 'host', 'supplied-text', 'synthetic',
  'standard-file', 'standard-http', 'standard-generic-url']);
const phases = new Set(['runtime-import', 'clause-preparation', 'primary']);
const present = value => value !== null && value !== undefined;
const nonempty = value => typeof value === 'string' && value.length > 0;
// Compare decimal strings without narrowing u64 identities to a JS Number.
const decimal = value => typeof value === 'string' && /^(0|[1-9][0-9]*)$/.test(value)
  && (value.length < 20 || (value.length === 20 && value <= '18446744073709551615'));
export class ModuleTransportError extends Error {}
function invalid() { throw new ModuleTransportError('invalid default-policy module request context'); }

export function validateModuleRequest(event) {
  const request = event?.module;
  if (event?.kind !== 'module-request' || !nonempty(event.token)
      || !request || typeof request !== 'object' || Array.isArray(request)
      || !Object.hasOwn(request, 'id') || !Object.hasOwn(request, 'kind')
      || !decimal(request.id) || typeof request.kind !== 'string' || !Object.hasOwn(contexts, request.kind)
      || Object.keys(request).some(name => !fields.has(name))) invalid();
  const required = contexts[request.kind];
  if (required.some(name => !Object.hasOwn(request, name) || typeof request[name] !== 'string')
      || uriFields.some(name => !required.includes(name) && present(request[name]))) invalid();
  if (request.kind === 'check-import' && request.originalUri !== request.importUri) invalid();
  let requiredKeys = [];
  switch (request.kind) {
    case 'select-key': requiredKeys = ['keyId', 'selectionPhase']; break;
    case 'query-cache-policy':
    case 'query-fragment-paths':
    case 'resolve-import-uri': requiredKeys = ['keyId', 'keyKind', 'keyHandle']; break;
    case 'capabilities':
    case 'has-element':
      if (keyFields.some(name => present(request[name]))) requiredKeys = ['keyId', 'keyKind', 'keyHandle'];
      break;
    case 'resolve-key': requiredKeys = ['keyId', 'keyKind', 'keyHandle', 'resolvedKeyId', 'resolutionPhase']; break;
    case 'resolve':
    case 'load-source':
      // Stage48 artifacts have no descriptors. New artifacts must supply the
      // complete phase-specific descriptor, never a partial legacy/new hybrid.
      if (keyFields.some(name => present(request[name]))) {
        requiredKeys = ['keyId', 'keyKind', 'keyHandle', 'resolvedKeyId'];
        if (request.kind === 'load-source') requiredKeys.push('resolvedKeyHandle');
      }
      break;
  }
  if (requiredKeys.some(name => !Object.hasOwn(request, name) || !present(request[name]))
      || keyFields.some(name => !requiredKeys.includes(name) && present(request[name]))) invalid();
  for (const name of requiredKeys) {
    if (name === 'keyId' || name === 'resolvedKeyId') { if (!decimal(request[name])) invalid(); }
    else if (name === 'keyKind') { if (!keyKinds.has(request[name])) invalid(); }
    else if (name === 'selectionPhase') { if (!phases.has(request[name])) invalid(); }
    else if (name === 'resolutionPhase') { if (!['primary', 'query'].includes(request[name])) invalid(); }
    // Opaque handles may be empty (the shared default StandardLibrary key is).
    else if (typeof request[name] !== 'string') invalid();
  }
  return request;
}

export function completeDefaultModuleRequest(session, event) {
  const request = validateModuleRequest(event);
  if (!['check-import', 'select-key', 'query-cache-policy', 'query-fragment-paths', 'resolve', 'resolve-key', 'resolve-import-uri'].includes(request.kind)) invalid();
  const completed = session.completeDefaultModule(event.token, 0);
  if (typeof completed !== 'boolean') throw new ModuleTransportError('invalid default module completion result');
  return completed;
}

// An exact fixture registry is input data, not a semantic URI/source cache.
// Selection and the later isCached query are deliberately separate completions.
export function completeFixtureModuleKey(session, event, configuredUris) {
  if (!['select-key', 'query-cache-policy', 'query-fragment-paths'].includes(event?.module?.kind)) return false;
  const request = validateModuleRequest(event);
  if (!(configuredUris instanceof Set)) throw new ModuleTransportError('invalid fixture ModuleKey URI registry');
  if (!configuredUris.has(request.originalUri)) return false;
  const handle = `fixture-key:${request.keyId}`;
  if (request.kind === 'select-key') {
    session.completeModuleKey(event.token, request.keyId, request.originalUri, 'host', handle, 0);
  } else {
    if (request.keyKind !== 'host' || request.keyHandle !== handle)
      throw new ModuleTransportError('fixture ModuleKey descriptor does not match its owner');
    if (request.kind === 'query-cache-policy') session.completeModuleCachePolicy(event.token, request.keyId, true, 0);
    else session.completeModuleFragmentPaths(event.token, request.keyId, false, 0);
  }
  return true;
}
