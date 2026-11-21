/**
 * Structured PklError preserving exact raw rendered Pkl diagnostics (lossless UTF-16),
 * error classification (`kind` / `code`), and explicit missing host `capability`.
 */
export class PklError extends Error {
  /**
   * @param {string} message
   * @param {"evaluation"|"security"|"timeout"|"closed"|"cancelled"|"configuration"|"decode"|"io"|"invalid-argument"|"capability-unavailable"} [kind]
   * @param {string} [capability]
   */
  constructor(message, kind = 'evaluation', capability = undefined) {
    super(message);
    this.name = 'PklError';
    this.kind = kind;
    this.code = kind;
    if (capability !== undefined) {
      this.capability = capability;
    }
  }
}

/**
 * Converts a raw JS or Wasm-thrown error into a `PklError` without mutating or trimming `message`.
 * @param {unknown} err
 * @param {PklError["kind"]} [fallbackKind]
 * @returns {PklError}
 */
export function toPklError(err, fallbackKind = 'evaluation') {
  if (err instanceof PklError) {
    return err;
  }
  if (err && typeof err === 'object') {
    const anyErr = /** @type {any} */ (err);
    const kind = typeof anyErr.__pklKind === 'string' ? anyErr.__pklKind : fallbackKind;
    const capability =
      typeof anyErr.__pklCapability === 'string' ? anyErr.__pklCapability : undefined;
    const message =
      typeof anyErr.message === 'string' ? anyErr.message : String(err);
    return new PklError(message, kind, capability);
  }
  return new PklError(String(err), fallbackKind);
}
