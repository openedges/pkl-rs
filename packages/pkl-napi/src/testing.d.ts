import type { PklValue } from "./index";

export function roundtripValue<T extends PklValue>(val: T): PklValue;
export function roundtripInteger(val: bigint): bigint;
export function roundtripBytes(val: Uint8Array): Uint8Array;
export function roundtripString(val: string): string;

export interface ConversionBenchmarkResult {
  iterations: number;
  totalMs: number;
  elapsedMs: number;
  nativeElapsedMs: number;
  avgUs: number;
  perIterationUs: number;
  nativePerIterationUs: number;
  opsPerSec: number;
  sample: PklValue;
  lastValue: PklValue;
}
export type ConversionTelemetry = ConversionBenchmarkResult;

export function measureConversion(
  iterations?: number,
  sampleValue?: PklValue,
): ConversionBenchmarkResult;
export function measureConversion(
  value: PklValue,
  iterations?: number,
): ConversionBenchmarkResult;
