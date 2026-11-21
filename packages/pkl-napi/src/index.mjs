import { createRequire } from "node:module";

const require = createRequire(import.meta.url);
const pkg = require("./index.cjs");

export const {
  PKL_VERSION,
  capabilities,
  PklError,
  ModuleSource,
  PklDuration,
  PklDataSize,
  PklPair,
  PklRegex,
  PklObject,
  PklClass,
  PklTypeAlias,
  PklIntSeq,
  PklFunction,
  PklReference,
  Evaluator,
  Project,
  ReplSession,
  createEvaluator,
  createEvaluatorSync,
  createRepl,
  analyzeImportGraph,
  format,
  decodePklBinary,
  monotonicClockMs,
  roundtripValue,
  roundtripInteger,
  roundtripBytes,
  roundtripString,
  measureConversion,
} = pkg;

export default pkg;
