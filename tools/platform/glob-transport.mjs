// Shared Node/browser transport validation. URI matching and caches remain in Rust.
const fields = new Set(['id', 'kind', 'uri', 'readerKind', 'readerId', 'readerUri', 'readerScheme', 'readerHandle', 'keyKind']);
const kinds = new Set(['select-resource', 'is-globbable', 'is-hierarchical', 'fragment-paths', 'has-element', 'list-elements']);
const keyKinds = new Set(['standard-library', 'host', 'supplied-text', 'synthetic',
  'standard-file', 'standard-http', 'standard-generic-url']);
const present = value => value !== undefined && value !== null;
const text = value => typeof value === 'string';
export const decimalU64 = value => text(value) && /^(0|[1-9][0-9]*)$/.test(value)
  && (value.length < 20 || value.length === 20 && value <= '18446744073709551615');
export class GlobTransportError extends Error {}
function invalid(reason) { throw new GlobTransportError(reason); }
export function validateGlobRequest(event) {
  const request = event?.glob;
  if (event?.kind !== 'glob-request' || !text(event.token) || event.token.length === 0
      || !request || typeof request !== 'object' || Array.isArray(request)
      || Object.keys(request).some(key => !fields.has(key))
      || !decimalU64(request.id) || !decimalU64(request.readerId) || !kinds.has(request.kind))
    invalid('invalid glob request identity or shape');
  const uriRequired = ['select-resource', 'has-element', 'list-elements'].includes(request.kind);
  if (uriRequired ? !text(request.uri) : present(request.uri)) invalid('invalid glob URI fields');
  if (!['resource', 'module'].includes(request.readerKind)) invalid('invalid glob reader kind');
  if (request.kind === 'select-resource') {
    if (request.readerKind !== 'resource' || ['readerUri', 'readerScheme', 'readerHandle', 'keyKind'].some(key => present(request[key])))
      invalid('invalid unselected resource reader');
  } else if (request.readerKind === 'resource') {
    if (!text(request.readerScheme) || !text(request.readerHandle) || present(request.readerUri) || present(request.keyKind))
      invalid('incomplete resource reader identity');
  } else {
    if (['fragment-paths', 'has-element'].includes(request.kind) || !text(request.readerUri)
        || !text(request.readerHandle) || !keyKinds.has(request.keyKind) || present(request.readerScheme))
      invalid('incomplete module reader identity');
  }
  return request;
}
export function validateElements(elements) {
  if (!Array.isArray(elements)) invalid('element array required');
  for (const element of elements) {
    if (!element || Object.keys(element).some(key => !['name', 'isDirectory'].includes(key))
        || !(element.name instanceof Uint16Array) || typeof element.isDirectory !== 'boolean')
      invalid('lossless name and strict directory boolean required');
  }
  return elements;
}
export function completeGlobElements(session, event, elements, now = 0) {
  const request = validateGlobRequest(event);
  if (request.kind !== 'list-elements') invalid('elements completion outside listing phase');
  session.completeGlobElements(event.token, validateElements(elements), now);
}
export function completeGlobBoolean(session, event, value, now = 0) {
  const request = validateGlobRequest(event);
  if (!['is-globbable', 'is-hierarchical', 'fragment-paths', 'has-element'].includes(request.kind)
      || typeof value !== 'boolean') invalid('boolean completion outside boolean phase');
  session.completeGlobBoolean(event.token, request.kind, value, now);
}
export function completeGlobReader(session, event, scheme, handle, now = 0) {
  const request = validateGlobRequest(event);
  if (request.kind !== 'select-resource' || !text(scheme) || !text(handle)) invalid('invalid reader completion');
  session.completeGlobReader(event.token, request.readerId, scheme, handle, now);
}
export function completeDefaultGlob(session, event, now = 0) {
  validateGlobRequest(event);
  const completed = session.completeDefaultGlob(event.token, now);
  if (typeof completed !== 'boolean') invalid('invalid default glob completion result');
  return completed;
}
