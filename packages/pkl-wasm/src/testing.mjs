import { toPklError, PklError } from './errors.mjs';
import { ensureInitSync, getWasmBindings } from './runtime.mjs';
import {
  PklDataSize,
  PklDuration,
  PklObject,
  dehydrateValue,
  hydrateValue,
} from './values.mjs';

/**
 * Lossless JS <-> Rust value round-trip helper across the Wasm boundary.
 * Exposed via `@openedges/pkl-wasm/testing` for internal tests and benchmarks.
 * @param {any} value
 * @returns {any}
 */
export function roundtripValue(value) {
  const wasm = getWasmBindings();
  const dehydrated = dehydrateValue(value);
  try {
    const raw = wasm.wasmRoundtripValue(dehydrated);
    return hydrateValue(raw);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Lossless signed 64-bit BigInt round-trip helper (`i64::MIN..=i64::MAX`).
 * @param {bigint} value
 * @returns {bigint}
 */
export function roundtripInteger(value) {
  const wasm = getWasmBindings();
  try {
    return wasm.wasmValidateInt64(value);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Lossless owned `Uint8Array` round-trip helper.
 * @param {Uint8Array} value
 * @returns {Uint8Array}
 */
export function roundtripBytes(value) {
  const wasm = getWasmBindings();
  if (!(value instanceof Uint8Array)) {
    throw new PklError('expected Uint8Array', 'invalid-argument');
  }
  try {
    return wasm.roundtripBytes(value);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Lossless UTF-16 string round-trip helper (preserves unpaired surrogates and astral scalars).
 * @param {string} value
 * @returns {string}
 */
export function roundtripString(value) {
  const wasm = getWasmBindings();
  if (typeof value !== 'string') {
    throw new PklError('expected string', 'invalid-argument');
  }
  try {
    return wasm.roundtripString(value);
  } catch (err) {
    throw toPklError(err, 'invalid-argument');
  }
}

/**
 * Measures JS <-> Rust Wasm value conversion round-trip overhead for benchmark telemetry.
 * @param {number} [iterations]
 * @param {any} [sampleValue]
 */
export function measureConversion(iterationsOrValue = 1000, secondArg = undefined) {
  ensureInitSync();
  let iterations;
  let sampleValue;
  if (
    typeof iterationsOrValue === 'number' &&
    (secondArg === undefined || Number.isInteger(iterationsOrValue))
  ) {
    iterations = iterationsOrValue;
    sampleValue = secondArg;
  } else {
    sampleValue = iterationsOrValue;
    iterations = secondArg === undefined ? 1000 : secondArg;
  }
  if (!Number.isInteger(iterations) || iterations <= 0 || iterations > 1_000_000) {
    throw new PklError(
      `iterations must be an integer between 1 and 1000000, got ${iterations}`,
      'invalid-argument',
    );
  }
  const payload =
    sampleValue !== undefined
      ? sampleValue
      : new PklObject(
          'Benchmark#Payload',
          'file:///bench.pkl',
          {
            id: 9223372036854775807n,
            ratio: 3.141592653589793,
            label: 'hello-🚀-pkl',
            active: true,
            raw: new Uint8Array([0, 1, 127, 128, 255]),
            dur: new PklDuration(250, 'ms'),
            size: new PklDataSize(64, 'mib'),
            tags: ['a', 'b', 'c'],
          },
          new Map([['k', 42n]]),
          ['elem0'],
        );

  const now = () =>
    typeof performance !== 'undefined' && typeof performance.now === 'function'
      ? performance.now()
      : Date.now();
  const t0 = now();
  let last;
  for (let i = 0; i < iterations; i += 1) {
    last = roundtripValue(payload);
  }
  const totalMs = Math.max(0, now() - t0);
  const avgUs = (totalMs * 1000) / iterations;
  return {
    iterations,
    totalMs,
    elapsedMs: totalMs,
    avgUs,
    perIterationUs: avgUs,
    opsPerSec: totalMs > 0 ? (iterations * 1000) / totalMs : Infinity,
    sample: last,
    lastValue: last,
  };
}
