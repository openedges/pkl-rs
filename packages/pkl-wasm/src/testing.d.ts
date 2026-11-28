import type { PklValue } from "./index.js";

export interface ConversionTelemetry {
  iterations: number;
  totalMs: number;
  elapsedMs: number;
  avgUs: number;
  perIterationUs: number;
  opsPerSec: number;
  sample: PklValue;
  lastValue: PklValue;
}
export type ConversionBenchmarkResult = ConversionTelemetry;

export declare function roundtripValue<T extends PklValue>(value: T): PklValue;
export declare function roundtripInteger(value: bigint): bigint;
export declare function roundtripBytes(value: Uint8Array): Uint8Array;
export declare function roundtripString(value: string): string;
export declare function measureConversion(
  iterations?: number,
  sampleValue?: PklValue,
): ConversionTelemetry;
export declare function measureConversion(
  sampleValue: PklValue,
  iterations?: number,
): ConversionTelemetry;
