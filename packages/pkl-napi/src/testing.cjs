"use strict";

const pkg = require("./index.cjs");

const TESTING_INTERNALS = Symbol.for("@openedges/pkl-napi/testing-internals");
const internals = pkg[TESTING_INTERNALS];
if (!internals) {
  throw new Error("Missing @openedges/pkl-napi testing internals");
}

const { native, unwrapOutcome, dehydrateValue, hydrateValue } = internals;
const { PklError, PklObject, PklDuration, PklDataSize, PklPair } = pkg;

const MIN_I64 = -(1n << 63n);
const MAX_I64 = (1n << 63n) - 1n;

function roundtripValue(val) {
  const dehydrated = dehydrateValue(val);
  const raw = unwrapOutcome(native.productRoundtripValue(dehydrated));
  return hydrateValue(raw);
}

function roundtripInteger(val) {
  if (typeof val !== "bigint") {
    throw new PklError("roundtripInteger expects a BigInt.", "invalid-argument");
  }
  return unwrapOutcome(native.productRoundtripInteger(val));
}

function roundtripBytes(val) {
  if (!(val instanceof Uint8Array)) {
    throw new PklError("roundtripBytes expects a Uint8Array.", "invalid-argument");
  }
  return native.roundtripBytes(new Uint8Array(val.buffer, val.byteOffset, val.byteLength));
}

function roundtripString(val) {
  if (typeof val !== "string") {
    throw new PklError("roundtripString expects a string.", "invalid-argument");
  }
  return native.roundtripString(val);
}

function defaultBenchmarkPayload() {
  return new PklObject(
    "Bench#Payload",
    "repl:bench",
    {
      intMin: MIN_I64,
      intMax: MAX_I64,
      floatVal: 3.141592653589793,
      text: "hello-pkl-🚀-utf16",
      bytes: new Uint8Array([0, 1, 2, 127, 128, 255]),
      duration: new PklDuration(5, "s"),
      dataSize: new PklDataSize(16, "mib"),
      pair: new PklPair("left", 42n),
      list: [1n, 2n, 3n, true, null],
      set: new Set(["a", "b", "c"]),
      map: new Map([
        ["k1", 100n],
        ["k2", "v2"],
      ]),
    },
    new Map(),
    [],
  );
}

function measureConversion(valOrIterations = 1000, maybeSecond = undefined) {
  let value;
  let iterations;
  if (
    typeof valOrIterations === "number" &&
    (maybeSecond === undefined || Number.isInteger(valOrIterations))
  ) {
    iterations = valOrIterations;
    value = maybeSecond !== undefined ? maybeSecond : defaultBenchmarkPayload();
  } else {
    value = valOrIterations;
    iterations = maybeSecond === undefined ? 1000 : maybeSecond;
  }
  if (!Number.isInteger(iterations) || iterations < 1 || iterations > 1_000_000) {
    throw new PklError(
      `iterations must be an integer between 1 and 1000000, got ${iterations}.`,
      "invalid-argument",
    );
  }
  const dehydrated = dehydrateValue(value);
  const startNs = process.hrtime.bigint();
  const nativeStats = unwrapOutcome(native.productMeasureConversion(dehydrated, iterations));
  const hydrated = hydrateValue(nativeStats.lastValue);
  const totalElapsedMs = Number(process.hrtime.bigint() - startNs) / 1e6;
  const perIterationUs = (totalElapsedMs * 1000) / nativeStats.iterations;
  return {
    iterations: nativeStats.iterations,
    totalMs: totalElapsedMs,
    elapsedMs: totalElapsedMs,
    nativeElapsedMs: nativeStats.elapsedMs,
    avgUs: perIterationUs,
    perIterationUs,
    nativePerIterationUs: nativeStats.perIterationUs,
    opsPerSec: totalElapsedMs > 0 ? (nativeStats.iterations * 1000) / totalElapsedMs : Infinity,
    sample: hydrated,
    lastValue: hydrated,
  };
}

module.exports = {
  roundtripValue,
  roundtripInteger,
  roundtripBytes,
  roundtripString,
  measureConversion,
};
