export const PKL_VERSION: "0.32.0";

export interface PklCapabilities {
  vfs: boolean;
  asyncModuleLoader: boolean;
  asyncResourceLoader: boolean;
  nativeFs: boolean;
  nativeHttp: boolean;
  externalReaders: boolean;
  modulePathJar: boolean;
  projectResolution: boolean;
  packageCache: boolean;
  formatter: boolean;
  repl: boolean;
  schema: boolean;
  analyzeImports: boolean;
  bigint64: boolean;
  uint8ArrayBytes: boolean;
  clock64Bit: boolean;
  platform: string;
  runtime: "wasm-browser" | "wasm-node" | "napi-node";
  arch: string;
}

export function capabilities(): PklCapabilities;

export type PklErrorKind =
  | "evaluation"
  | "security"
  | "timeout"
  | "closed"
  | "cancelled"
  | "configuration"
  | "decode"
  | "io"
  | "invalid-argument"
  | "capability-unavailable";

export class PklError extends Error {
  readonly kind: PklErrorKind;
  readonly code: PklErrorKind;
  readonly capability?: keyof PklCapabilities;
  constructor(message: string, kind?: PklErrorKind, capability?: keyof PklCapabilities);
}

export class ModuleSource {
  readonly uri: string;
  readonly contents?: string;
  constructor(uri: string, contents?: string);
  static text(text: string, uri?: string): ModuleSource;
  static uri(uri: string): ModuleSource;
  static path(path: string): ModuleSource;
  static modulePath(path: string): ModuleSource;
}

export class PklDuration {
  readonly value: number;
  readonly unit: string;
  constructor(value: number, unit: string);
}

export class PklDataSize {
  readonly value: number;
  readonly unit: string;
  constructor(value: number, unit: string);
}

export class PklPair<A = PklValue, B = PklValue> {
  readonly first: A;
  readonly second: B;
  constructor(first: A, second: B);
}

export class PklRegex {
  readonly pattern: string;
  constructor(pattern: string);
}

export class PklObject {
  readonly className: string;
  readonly moduleUri: string;
  readonly properties: Record<string, PklValue>;
  readonly entries: Map<PklValue, PklValue>;
  readonly elements: PklValue[];
  constructor(
    className: string,
    moduleUri: string,
    properties?: Record<string, PklValue>,
    entries?: Map<PklValue, PklValue>,
    elements?: PklValue[],
  );
}

export class PklClass {
  readonly name: string;
  readonly moduleUri: string;
  constructor(name: string, moduleUri: string);
}

export class PklTypeAlias {
  readonly name: string;
  readonly moduleUri: string;
  constructor(name: string, moduleUri: string);
}

export class PklIntSeq {
  readonly start: bigint;
  readonly end: bigint;
  readonly step: bigint;
  constructor(start: bigint, end: bigint, step: bigint);
}

export class PklFunction {
  constructor();
}

export class PklReference {
  readonly domain: PklValue;
  readonly data: PklValue;
  readonly path: PklValue[];
  constructor(domain: PklValue, data: PklValue, path?: PklValue[]);
}

export type PklValue =
  | null
  | boolean
  | bigint
  | number
  | string
  | Uint8Array
  | PklValue[]
  | Set<PklValue>
  | Map<PklValue, PklValue>
  | PklDuration
  | PklDataSize
  | PklPair
  | PklRegex
  | PklObject
  | PklClass
  | PklTypeAlias
  | PklIntSeq
  | PklFunction
  | PklReference;

export interface StackFrame {
  moduleUri: string;
  memberName: string | null;
  sourceLines: string[];
  startLine: number;
  startColumn: number;
  endLine: number;
  endColumn: number;
}

export interface PathElement {
  name: string;
  isDirectory: boolean;
}

export interface ModuleReader {
  scheme: string;
  isLocal?: boolean;
  isGlobbable?: boolean;
  hasHierarchicalUris?: boolean;
  isCached?: boolean;
  read(uri: string): string | Promise<string>;
  listElements?(uri: string): PathElement[] | Promise<PathElement[]>;
  hasElement?(uri: string): boolean | Promise<boolean>;
}

export interface ResourceReader {
  scheme: string;
  isGlobbable?: boolean;
  hasHierarchicalUris?: boolean;
  read(
    uri: string,
  ): Uint8Array | string | null | undefined | Promise<Uint8Array | string | null | undefined>;
  listElements?(uri: string): PathElement[] | Promise<PathElement[]>;
  hasElement?(uri: string): boolean | Promise<boolean>;
}

export interface SecurityManager {
  checkResolveModule?(uri: string): void | Promise<void>;
  checkImportModule?(importingUri: string, importedUri: string): void | Promise<void>;
  checkReadResource?(uri: string): void | Promise<void>;
  checkResolveResource?(uri: string): void | Promise<void>;
}

export interface Logger {
  trace?(msg: string, frame?: StackFrame): void | Promise<void>;
  warn?(msg: string, frame?: StackFrame): void | Promise<void>;
}

export interface HttpOptions {
  caCertificates?: (string | Uint8Array)[];
  caCertificateFiles?: string[];
  caCertificatePems?: (string | Uint8Array)[];
  proxy?: { address?: string; noProxy?: string[] };
  rewrites?: Record<string, string>;
  headers?: Record<string, Record<string, string>>;
  testPort?: number;
}

export interface ExternalReaderSpec {
  executable: string;
  arguments?: string[];
}

export interface EvaluatorOptions {
  allowedModules?: string[];
  allowedResources?: string[];
  rootDir?: string;
  workingDir?: string;
  properties?: Record<string, string> | Map<string, string>;
  env?: Record<string, string> | Map<string, string>;
  outputFormat?: string;
  traceMode?: "compact" | "pretty";
  powerAssertions?: boolean;
  color?: boolean | "never" | "auto" | "always";
  timeoutMs?: number | bigint;
  signal?: AbortSignal;
  logger?: Logger;
  frameTransformer?: (frame: StackFrame) => StackFrame | Promise<StackFrame>;
  securityManager?: SecurityManager;
  moduleReaders?: ModuleReader[];
  resourceReaders?: ResourceReader[];
  modulePaths?: string[];
  projectDir?: string;
  project?: Project;
  cacheDir?: string | null;
  noCache?: boolean;
  testMode?: boolean;
  http?: HttpOptions;
  externalModuleReaders?: Record<string, ExternalReaderSpec>;
  externalResourceReaders?: Record<string, ExternalReaderSpec>;
}

export interface CallOptions {
  timeoutMs?: number | bigint;
  signal?: AbortSignal;
}

export interface TestOptions extends CallOptions {
  overwrite?: boolean;
  reporter?: "spec" | "minimal" | "junit";
}

export interface SourceLocation {
  startLine: number;
  endLine: number;
}

export interface TypeParameter {
  name: string;
  variance: "invariant" | "covariant" | "contravariant";
}

export interface PClassInfo {
  moduleName: string;
  simpleName: string;
  qualifiedName: string;
  displayName: string;
  moduleUri: string;
  isModuleClass: boolean;
}

export interface TypeAliasInfo {
  moduleName: string;
  simpleName: string;
  qualifiedName: string;
  displayName: string;
}

export type PType =
  | { kind: "unknown" }
  | { kind: "nothing" }
  | { kind: "module" }
  | { kind: "stringLiteral"; literal: string }
  | { kind: "class"; classInfo: PClassInfo; typeArguments: PType[] }
  | { kind: "nullable"; baseType: PType }
  | { kind: "union"; elementTypes: PType[] }
  | { kind: "function"; parameterTypes: PType[]; returnType: PType }
  | { kind: "constrained"; baseType: PType; constraints: string[] }
  | { kind: "alias"; aliasInfo: TypeAliasInfo; typeArguments: PType[]; aliasedType: PType }
  | { kind: "typeVariable"; typeParameter: TypeParameter };

export interface PClassProperty {
  name: string;
  simpleName: string;
  moduleName: string;
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  type: PType;
}

export interface PClassMethod {
  name: string;
  simpleName: string;
  moduleName: string;
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  typeParameters: TypeParameter[];
  parameters: Record<string, PType>;
  returnType: PType;
}

export interface PClassSchema extends PClassInfo {
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  typeParameters: TypeParameter[];
  superclass: PClassInfo | null;
  supertype: PType | null;
  properties: Record<string, PClassProperty>;
  allProperties: Record<string, PClassProperty>;
  methods: Record<string, PClassMethod>;
  allMethods: Record<string, PClassMethod>;
}

export interface TypeAliasSchema extends TypeAliasInfo {
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  typeParameters: TypeParameter[];
  aliasedType: PType;
}

export interface ModuleSchema {
  moduleUri: string;
  moduleName: string;
  shortModuleName: string;
  isAmend: boolean;
  isExtend: boolean;
  docComment: string | null;
  annotations: PklObject[];
  supermodule: ModuleSchema | null;
  moduleClass: PClassSchema;
  classes: Record<string, PClassSchema>;
  allClasses: Record<string, PClassSchema>;
  typeAliases: Record<string, TypeAliasSchema>;
  allTypeAliases: Record<string, TypeAliasSchema>;
  imports: Record<string, string>;
}

export interface TestFailure {
  kind: string;
  message: string;
}

export interface TestError {
  message: string;
  rendered: string;
}

export interface TestResult {
  name: string;
  totalAsserts: number;
  failed: boolean;
  isExampleWritten: boolean;
  failures: TestFailure[];
  errors: TestError[];
}

export interface TestSectionResults {
  name: "facts" | "examples";
  failed: boolean;
  totalTests: number;
  totalFailures: number;
  totalAsserts: number;
  results: TestResult[];
}

export interface TestResults {
  moduleName: string;
  displayUri: string;
  failed: boolean;
  totalTests: number;
  totalFailures: number;
  totalAsserts: number;
  totalAssertsFailed: number;
  facts: TestSectionResults;
  examples: TestSectionResults;
  logs: string;
  error: TestError | null;
  rendered: string;
}

export interface ProjectPackageInfo {
  name: string;
  baseUri: string;
  version: string;
  uri: string;
  packageZipUrl: string;
  description: string | null;
  authors: string[];
  website: string | null;
  documentation: string | null;
  sourceCode: string | null;
  sourceCodeUrlScheme: string | null;
  license: string | null;
  issueTracker: string | null;
  apiTests: string[];
  exclude: string[];
}

export interface ProjectEvaluatorSettings {
  allowedModules: string[] | null;
  allowedResources: string[] | null;
  rootDir: string | null;
  moduleCacheDir: string | null;
  noCache: boolean | null;
  traceMode: string | null;
  color: string | null;
  timeoutMs: bigint | null;
  env: Record<string, string> | null;
  externalProperties: Record<string, string> | null;
  modulePath: string[] | null;
}

export class Project {
  readonly projectDir: string;
  readonly projectFileUri: string;
  readonly projectBaseUri: string;
  readonly package: ProjectPackageInfo | null;
  readonly evaluatorSettings: ProjectEvaluatorSettings | null;
  readonly tests: string[];
  static load(
    dirOrFile: string,
    options?: {
      env?: Record<string, string> | Map<string, string>;
      properties?: Record<string, string> | Map<string, string>;
    },
  ): Promise<Project>;
  static loadSync(
    dirOrFile: string,
    options?: {
      env?: Record<string, string> | Map<string, string>;
      properties?: Record<string, string> | Map<string, string>;
    },
  ): Project;
}

export class Evaluator {
  readonly isClosed: boolean;
  evaluateOutputText(source: ModuleSource, options?: CallOptions): Promise<string>;
  evaluateOutputTextSync(source: ModuleSource, options?: CallOptions): string;
  evaluateOutputBytes(source: ModuleSource, options?: CallOptions): Promise<Uint8Array>;
  evaluateOutputBytesSync(source: ModuleSource, options?: CallOptions): Uint8Array;
  evaluateOutputFiles(source: ModuleSource, options?: CallOptions): Promise<Map<string, Uint8Array>>;
  evaluateOutputFilesSync(source: ModuleSource, options?: CallOptions): Map<string, Uint8Array>;
  evaluateOutputValue(source: ModuleSource, options?: CallOptions): Promise<PklValue>;
  evaluateOutputValueSync(source: ModuleSource, options?: CallOptions): PklValue;
  evaluateExpression(source: ModuleSource, expr: string, options?: CallOptions): Promise<string>;
  evaluateExpressionSync(source: ModuleSource, expr: string, options?: CallOptions): string;
  evaluateExpressionValue(
    source: ModuleSource,
    expr: string,
    options?: CallOptions,
  ): Promise<PklValue>;
  evaluateExpressionValueSync(source: ModuleSource, expr: string, options?: CallOptions): PklValue;
  evaluateExpressionPklBinary(
    source: ModuleSource,
    expr: string,
    options?: CallOptions,
  ): Promise<Uint8Array>;
  evaluateExpressionPklBinarySync(
    source: ModuleSource,
    expr: string,
    options?: CallOptions,
  ): Uint8Array;
  evaluateSchema(source: ModuleSource, options?: CallOptions): Promise<ModuleSchema>;
  evaluateSchemaSync(source: ModuleSource, options?: CallOptions): ModuleSchema;
  evaluateTest(source: ModuleSource, options?: TestOptions): Promise<TestResults>;
  evaluateTestSync(source: ModuleSource, options?: TestOptions): TestResults;
  cancel(): void;
  close(): Promise<void>;
}

export function createEvaluator(options?: EvaluatorOptions): Promise<Evaluator>;
export function createEvaluatorSync(options?: EvaluatorOptions): Evaluator;

export interface ReplResponse {
  kind: "evalSuccess" | "evalError" | "incompleteInput" | "invalidRequest" | "internalError";
  result?: string;
  message?: string;
}

export class ReplSession {
  readonly isClosed: boolean;
  eval(input: string, options?: { force?: boolean; id?: string }): Promise<ReplResponse[]>;
  load(uriOrSource: string | ModuleSource, options?: { id?: string }): Promise<ReplResponse[]>;
  reset(options?: { id?: string }): Promise<void>;
  complete(prefix: string, options?: { id?: string }): Promise<string[]>;
  close(): Promise<void>;
}

export function createRepl(options?: EvaluatorOptions): Promise<ReplSession>;

export interface ImportGraph {
  imports: Record<string, string[]>;
  resolvedImports: Record<string, string>;
}

export function analyzeImportGraph(
  uris: (string | ModuleSource)[],
  options?: EvaluatorOptions,
): Promise<ImportGraph>;

export function format(source: string, options?: { grammarVersion?: 1 | 2 }): string;
export function decodePklBinary(bytes: Uint8Array): PklValue;
export function monotonicClockMs(): bigint;

export function roundtripValue<T extends PklValue>(val: T): PklValue;
export function roundtripInteger(val: bigint): bigint;
export function roundtripBytes(val: Uint8Array): Uint8Array;
export function roundtripString(val: string): string;

export interface ConversionBenchmarkResult {
  iterations: number;
  elapsedMs: number;
  nativeElapsedMs: number;
  perIterationUs: number;
  nativePerIterationUs: number;
  lastValue: PklValue;
}

export function measureConversion(iterations?: number): ConversionBenchmarkResult;
export function measureConversion(value: PklValue, iterations?: number): ConversionBenchmarkResult;
