export declare const PKL_VERSION: "0.32.0";

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

export declare class PklError extends Error {
  readonly name: "PklError";
  readonly kind: PklErrorKind;
  readonly code: PklErrorKind;
  readonly capability?: keyof PklCapabilities;
  constructor(message: string, kind?: PklErrorKind, capability?: keyof PklCapabilities);
}

export declare class ModuleSource {
  readonly uri: string;
  readonly contents?: string;
  constructor(uri: string, contents?: string);
  static text(text: string, uri?: string): ModuleSource;
  static uri(uri: string): ModuleSource;
  static path(path: string): ModuleSource;
  static modulePath(path: string): ModuleSource;
}

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
  hasFragmentPaths?: boolean;
  read(uri: string): string | Uint8Array | null | undefined | Promise<string | Uint8Array | null | undefined>;
  listElements?(uri: string): PathElement[] | Promise<PathElement[]>;
  hasElement?(uri: string): boolean | Promise<boolean>;
}

export interface ResourceReader {
  scheme: string;
  isGlobbable?: boolean;
  hasHierarchicalUris?: boolean;
  read(uri: string): Uint8Array | string | null | undefined | Promise<Uint8Array | string | null | undefined>;
  listElements?(uri: string): PathElement[] | Promise<PathElement[]>;
  hasElement?(uri: string): boolean | Promise<boolean>;
}

export interface HttpFetchResponse {
  status: number;
  headers?: Record<string, string>;
  bytes: Uint8Array;
}

export interface SecurityManager {
  checkImportModule?(importingUri: string, importedUri: string): void | Promise<void>;
  checkResolveModule?(uri: string): void | Promise<void>;
  checkReadResource?(uri: string): void | Promise<void>;
  checkResolveResource?(uri: string): void | Promise<void>;
}

export interface Logger {
  trace?(message: string, frame?: StackFrame): void | Promise<void>;
  warn?(message: string, frame?: StackFrame): void | Promise<void>;
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
  testMode?: boolean;
  timeoutMs?: number | bigint;
  signal?: AbortSignal;
  logger?: Logger;
  frameTransformer?: (frame: StackFrame) => StackFrame | Promise<StackFrame>;
  securityManager?: SecurityManager;
  moduleReaders?: ModuleReader[];
  resourceReaders?: ResourceReader[];
  vfs?: VirtualFileSystem;
  httpFetch?: (
    url: string,
    init?: { method?: string; headers?: Record<string, string>; signal?: AbortSignal },
  ) => Promise<HttpFetchResponse>;
  packageTransport?: (url: string, init?: { signal?: AbortSignal }) => Promise<Uint8Array>;
  platform?: { os?: string; osVersion?: string; arch?: string };
}

export interface CallOptions {
  timeoutMs?: number | bigint;
  signal?: AbortSignal;
}

export declare class PklDuration {
  readonly value: number;
  readonly unit: string;
  constructor(value: number, unit: string);
}

export declare class PklDataSize {
  readonly value: number;
  readonly unit: string;
  constructor(value: number, unit: string);
}

export declare class PklPair<A = PklValue, B = PklValue> {
  readonly first: A;
  readonly second: B;
  constructor(first: A, second: B);
}

export declare class PklRegex {
  readonly pattern: string;
  constructor(pattern: string);
}

export declare class PklObject {
  readonly className: string;
  readonly moduleUri: string;
  readonly properties: Record<string, PklValue>;
  readonly entries: Map<PklValue, PklValue>;
  readonly elements: PklValue[];
  constructor(
    className: string,
    moduleUri: string,
    properties?: Record<string, PklValue>,
    entries?: Map<PklValue, PklValue> | Iterable<[PklValue, PklValue]>,
    elements?: Iterable<PklValue>,
  );
  toJSON(): {
    className: string;
    moduleUri: string;
    properties: Record<string, PklValue>;
    entries: [PklValue, PklValue][];
    elements: PklValue[];
  };
}

export declare class PklClass {
  readonly name: string;
  readonly moduleUri: string;
  constructor(name: string, moduleUri: string);
}

export declare class PklTypeAlias {
  readonly name: string;
  readonly moduleUri: string;
  constructor(name: string, moduleUri: string);
}

export declare class PklIntSeq {
  readonly start: bigint;
  readonly end: bigint;
  readonly step: bigint;
  constructor(start: bigint, end: bigint, step: bigint);
}

export declare class PklFunction {
  constructor();
}

export declare class PklReference {
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

export interface SourceLocation {
  startLine: number;
  endLine: number;
}

export interface PClassInfo {
  moduleName: string;
  simpleName: string;
  moduleUri: string;
  qualifiedName: string;
  displayName: string;
  isModuleClass: boolean;
}

export interface TypeAliasInfo {
  moduleName: string;
  simpleName: string;
  qualifiedName: string;
  displayName: string;
}

export interface TypeParameter {
  name: string;
  variance: "invariant" | "covariant" | "contravariant";
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

export interface PPropertySchema {
  name: string;
  simpleName: string;
  moduleName: string;
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  type: PType;
}
export type PClassProperty = PPropertySchema;

export interface PMethodSchema {
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
export type PClassMethod = PMethodSchema;

export interface PClassSchema {
  info: PClassInfo;
  moduleName: string;
  simpleName: string;
  qualifiedName: string;
  displayName: string;
  moduleUri: string;
  isModuleClass: boolean;
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  typeParameters: TypeParameter[];
  superclass: PClassInfo | null;
  supertype: PType | null;
  properties: Record<string, PPropertySchema>;
  allProperties: Record<string, PPropertySchema>;
  methods: Record<string, PMethodSchema>;
  allMethods: Record<string, PMethodSchema>;
}

export interface PTypeAliasSchema {
  info: TypeAliasInfo;
  moduleName: string;
  simpleName: string;
  qualifiedName: string;
  displayName: string;
  docComment: string | null;
  sourceLocation: SourceLocation;
  modifiers: string[];
  annotations: PklObject[];
  typeParameters: TypeParameter[];
  aliasedType: PType;
}
export type TypeAliasSchema = PTypeAliasSchema;

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
  typeAliases: Record<string, PTypeAliasSchema>;
  allTypeAliases: Record<string, PTypeAliasSchema>;
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
  failures: TestFailure[];
  errors: TestError[];
  isExampleWritten: boolean;
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

export interface EvaluateTestOptions extends CallOptions {
  overwrite?: boolean;
  reporter?: "spec" | "minimal" | "junit";
}
export type TestOptions = EvaluateTestOptions;

export declare class VirtualFileSystem {
  readonly rootDir: string | null;
  constructor(initialFiles?: Record<string, string | Uint8Array>, options?: { rootDir?: string });
  write(path: string, content: string | Uint8Array): void;
  mkdir(path: string): void;
  read(path: string): Uint8Array;
  readText(path: string): string;
  has(path: string): boolean;
  isDirectory(path: string): boolean;
  isFile(path: string): boolean;
  delete(path: string): boolean;
  list(path?: string): PathElement[];
  glob(pattern: string): string[];
}

export declare class Evaluator {
  private constructor();
  readonly isClosed: boolean;
  evaluateOutputText(source: ModuleSource, options?: CallOptions): Promise<string>;
  evaluateOutputBytes(source: ModuleSource, options?: CallOptions): Promise<Uint8Array>;
  evaluateOutputFiles(source: ModuleSource, options?: CallOptions): Promise<Map<string, Uint8Array>>;
  evaluateOutputValue(source: ModuleSource, options?: CallOptions): Promise<PklValue>;
  evaluateExpression(source: ModuleSource, expr: string, options?: CallOptions): Promise<string>;
  evaluateExpressionValue(source: ModuleSource, expr: string, options?: CallOptions): Promise<PklValue>;
  evaluateExpressionPklBinary(source: ModuleSource, expr: string, options?: CallOptions): Promise<Uint8Array>;
  evaluateSchema(source: ModuleSource, options?: CallOptions): Promise<ModuleSchema>;
  evaluateTest(source: ModuleSource, options?: EvaluateTestOptions): Promise<TestResults>;
  cancel(): void;
  close(): Promise<void>;
}

export interface ReplResponse {
  kind:
    | "evalSuccess"
    | "evalError"
    | "incompleteInput"
    | "invalidRequest"
    | "internalError"
    | "completion";
  result?: string;
  message?: string;
  members?: string[];
}

export declare class ReplSession {
  private constructor();
  readonly isClosed: boolean;
  eval(
    input: string,
    options?: { force?: boolean; evalDefinitions?: boolean; id?: string },
  ): Promise<ReplResponse[]>;
  load(uriOrSource: string | ModuleSource, options?: { id?: string }): Promise<ReplResponse[]>;
  reset(options?: { id?: string }): Promise<void>;
  complete(prefix: string, options?: { id?: string }): Promise<string[]>;
  cancel(): void;
  close(): Promise<void>;
}

export interface ImportGraph {
  imports: Record<string, string[]>;
  resolvedImports: Record<string, string>;
}

export declare function init(
  input?: RequestInfo | URL | Response | BufferSource | WebAssembly.Module,
): Promise<void>;
export declare function capabilities(): PklCapabilities;
export declare function createEvaluator(options?: EvaluatorOptions): Promise<Evaluator>;
export declare function decodePklBinary(bytes: Uint8Array, options?: { strict?: boolean }): PklValue;
export declare function format(
  source: string,
  options?: { grammarVersion?: 1 | 2 | null },
): string;
export declare function createRepl(options?: EvaluatorOptions): Promise<ReplSession>;
export declare function analyzeImportGraph(
  uris: (string | ModuleSource)[],
  options?: EvaluatorOptions,
): Promise<ImportGraph>;
