"use strict";

const { existsSync } = require("node:fs");
const { resolve } = require("node:path");

function loadNativeAddon() {
  const candidates = [
    process.env.PKL_NAPI_ADDON_PATH,
    resolve(__dirname, "..", "native", `pkl_napi.${process.platform}-${process.arch}.node`),
    resolve(__dirname, "..", "native", "pkl_napi.node"),
  ].filter(Boolean);

  for (const candidate of candidates) {
    if (existsSync(candidate)) {
      return require(candidate);
    }
  }
  throw new Error(
    `Unable to load native @pkl-rs/napi addon for ${process.platform}-${process.arch}. Checked: ${candidates.join(", ")}`,
  );
}

const native = loadNativeAddon();

const PKL_VERSION = native.productPklVersion ? native.productPklVersion() : "0.32.0";
const MAX_U64 = (1n << 64n) - 1n;
const MIN_I64 = -(1n << 63n);
const MAX_I64 = (1n << 63n) - 1n;

class PklError extends Error {
  constructor(message, kind = "evaluation", capability = undefined) {
    super(message);
    this.name = "PklError";
    this.kind = kind;
    this.code = kind;
    if (capability !== undefined) {
      this.capability = capability;
    }
  }
}

function unwrapOutcome(outcome) {
  if (!outcome || typeof outcome !== "object") {
    throw new PklError("Malformed native outcome from pkl-napi.", "evaluation");
  }
  if (!outcome.ok) {
    throw new PklError(
      outcome.message ?? "Unknown Pkl error",
      outcome.kind ?? "evaluation",
      outcome.capability,
    );
  }
  return outcome.value;
}

function capabilities() {
  return {
    vfs: false,
    asyncModuleLoader: true,
    asyncResourceLoader: true,
    nativeFs: true,
    nativeHttp: true,
    externalReaders: true,
    modulePathJar: true,
    projectResolution: true,
    packageCache: true,
    formatter: true,
    repl: true,
    schema: true,
    analyzeImports: true,
    bigint64: true,
    uint8ArrayBytes: true,
    clock64Bit: true,
    platform: process.platform,
    runtime: "napi-node",
    arch: process.arch,
  };
}

class ModuleSource {
  constructor(uri, contents = undefined) {
    if (typeof uri !== "string" || uri.length === 0) {
      throw new PklError("ModuleSource uri must be a non-empty string.", "invalid-argument");
    }
    if (contents !== undefined && typeof contents !== "string") {
      throw new PklError("ModuleSource contents must be a string or undefined.", "invalid-argument");
    }
    this.uri = uri;
    if (contents !== undefined) {
      this.contents = contents;
    }
    Object.freeze(this);
  }

  static text(text, uri = "repl:text") {
    if (typeof text !== "string") {
      throw new PklError("ModuleSource.text requires a string source.", "invalid-argument");
    }
    if (typeof uri !== "string" || uri.length === 0) {
      throw new PklError("ModuleSource.text uri must be a non-empty string.", "invalid-argument");
    }
    return new ModuleSource(uri, text);
  }

  static uri(uri) {
    if (typeof uri !== "string" || uri.length === 0) {
      throw new PklError("ModuleSource.uri requires a non-empty string.", "invalid-argument");
    }
    return new ModuleSource(uri, undefined);
  }

  static path(fsPath) {
    if (typeof fsPath !== "string" || fsPath.length === 0) {
      throw new PklError("ModuleSource.path requires a non-empty string path.", "invalid-argument");
    }
    const uri = unwrapOutcome(native.productPathToFileUri(fsPath));
    return new ModuleSource(uri, undefined);
  }

  static modulePath(modulePath) {
    if (typeof modulePath !== "string" || modulePath.length === 0) {
      throw new PklError(
        "ModuleSource.modulePath requires a non-empty string path.",
        "invalid-argument",
      );
    }
    let uri;
    if (modulePath.startsWith("modulepath:")) {
      uri = modulePath;
    } else if (modulePath.startsWith("/")) {
      uri = `modulepath:${modulePath}`;
    } else {
      uri = `modulepath:/${modulePath}`;
    }
    return new ModuleSource(uri, undefined);
  }
}

function normalizeSource(source) {
  if (source instanceof ModuleSource) {
    return { uri: source.uri, contents: source.contents };
  }
  if (source && typeof source === "object" && typeof source.uri === "string") {
    if (source.contents !== undefined && typeof source.contents !== "string") {
      throw new PklError("ModuleSource.contents must be a string.", "invalid-argument");
    }
    return { uri: source.uri, contents: source.contents };
  }
  throw new PklError("Expected a ModuleSource instance.", "invalid-argument");
}

class PklDuration {
  constructor(value, unit) {
    this.value = value;
    this.unit = unit;
  }
}

class PklDataSize {
  constructor(value, unit) {
    this.value = value;
    this.unit = unit;
  }
}

class PklPair {
  constructor(first, second) {
    this.first = first;
    this.second = second;
  }
}

class PklRegex {
  constructor(pattern) {
    this.pattern = pattern;
  }
}

class PklObject {
  constructor(className, moduleUri, properties = {}, entries = new Map(), elements = []) {
    this.className = className;
    this.moduleUri = moduleUri;
    this.properties = properties;
    this.entries = entries instanceof Map ? entries : new Map(entries);
    this.elements = Array.isArray(elements) ? elements : Array.from(elements);
  }
}

class PklClass {
  constructor(name, moduleUri) {
    this.name = name;
    this.moduleUri = moduleUri;
  }
}

class PklTypeAlias {
  constructor(name, moduleUri) {
    this.name = name;
    this.moduleUri = moduleUri;
  }
}

class PklIntSeq {
  constructor(start, end, step) {
    this.start = BigInt(start);
    this.end = BigInt(end);
    this.step = BigInt(step);
  }
}

class PklFunction {}

class PklReference {
  constructor(domain, data, path = []) {
    this.domain = domain;
    this.data = data;
    this.path = Array.isArray(path) ? path : Array.from(path);
  }
}

function definePropSafe(obj, key, value) {
  Object.defineProperty(obj, key, {
    value,
    enumerable: true,
    writable: true,
    configurable: true,
  });
}

function hydrateValue(raw) {
  if (
    raw === null ||
    raw === undefined ||
    typeof raw === "boolean" ||
    typeof raw === "number" ||
    typeof raw === "bigint" ||
    typeof raw === "string"
  ) {
    return raw;
  }
  if (raw instanceof Uint8Array) {
    return raw;
  }
  if (Array.isArray(raw)) {
    return raw.map(hydrateValue);
  }
  if (typeof raw === "object" && typeof raw.__pklTag === "string") {
    switch (raw.__pklTag) {
      case "set": {
        const set = new Set();
        for (const item of raw.items) {
          set.add(hydrateValue(item));
        }
        return set;
      }
      case "map": {
        const map = new Map();
        for (const [k, v] of raw.entries) {
          map.set(hydrateValue(k), hydrateValue(v));
        }
        return map;
      }
      case "duration":
        return new PklDuration(raw.value, raw.unit);
      case "dataSize":
        return new PklDataSize(raw.value, raw.unit);
      case "regex":
        return new PklRegex(raw.pattern);
      case "pair":
        return new PklPair(hydrateValue(raw.first), hydrateValue(raw.second));
      case "object": {
        const properties = {};
        for (const [k, v] of raw.properties) {
          definePropSafe(properties, k, hydrateValue(v));
        }
        const entries = new Map();
        for (const [k, v] of raw.entries) {
          entries.set(hydrateValue(k), hydrateValue(v));
        }
        const elements = raw.elements.map(hydrateValue);
        return new PklObject(raw.className, raw.moduleUri, properties, entries, elements);
      }
      case "class":
        return new PklClass(raw.name, raw.moduleUri);
      case "typeAlias":
        return new PklTypeAlias(raw.name, raw.moduleUri);
      case "intSeq":
        return new PklIntSeq(raw.start, raw.end, raw.step);
      case "function":
        return new PklFunction();
      case "reference":
        return new PklReference(
          hydrateValue(raw.domain),
          hydrateValue(raw.data),
          raw.path.map(hydrateValue),
        );
      default:
        throw new PklError(`Unknown Pkl tag: ${raw.__pklTag}`, "decode");
    }
  }
  return raw;
}

function hydrateSchema(schema) {
  if (!schema || typeof schema !== "object") return schema;
  if (Array.isArray(schema.annotations)) {
    schema.annotations = schema.annotations.map(hydrateValue);
  }
  if (schema.supermodule) {
    hydrateSchema(schema.supermodule);
  }
  const hydrateClass = (cls) => {
    if (!cls) return;
    if (Array.isArray(cls.annotations)) {
      cls.annotations = cls.annotations.map(hydrateValue);
    }
    for (const mapName of ["properties", "allProperties", "methods", "allMethods"]) {
      const m = cls[mapName];
      if (m && typeof m === "object") {
        for (const member of Object.values(m)) {
          if (member && Array.isArray(member.annotations)) {
            member.annotations = member.annotations.map(hydrateValue);
          }
        }
      }
    }
  };
  hydrateClass(schema.moduleClass);
  for (const mapName of ["classes", "allClasses"]) {
    const m = schema[mapName];
    if (m && typeof m === "object") {
      for (const cls of Object.values(m)) {
        hydrateClass(cls);
      }
    }
  }
  for (const mapName of ["typeAliases", "allTypeAliases"]) {
    const m = schema[mapName];
    if (m && typeof m === "object") {
      for (const alias of Object.values(m)) {
        if (alias && Array.isArray(alias.annotations)) {
          alias.annotations = alias.annotations.map(hydrateValue);
        }
      }
    }
  }
  return schema;
}

function dehydrateValue(val) {
  if (val === null) return null;
  if (typeof val === "boolean" || typeof val === "number" || typeof val === "string") {
    return val;
  }
  if (typeof val === "bigint") {
    if (val < MIN_I64 || val > MAX_I64) {
      throw new PklError(
        `BigInt ${val} is outside signed 64-bit integer range (${MIN_I64}..=${MAX_I64}).`,
        "invalid-argument",
      );
    }
    return val;
  }
  if (val instanceof Uint8Array) {
    return new Uint8Array(val.buffer, val.byteOffset, val.byteLength);
  }
  if (Array.isArray(val)) {
    return val.map(dehydrateValue);
  }
  if (val instanceof Set) {
    return { __pklTag: "set", items: Array.from(val, dehydrateValue) };
  }
  if (val instanceof Map) {
    return {
      __pklTag: "map",
      entries: Array.from(val.entries(), ([k, v]) => [dehydrateValue(k), dehydrateValue(v)]),
    };
  }
  if (val instanceof PklDuration) {
    return { __pklTag: "duration", value: val.value, unit: val.unit };
  }
  if (val instanceof PklDataSize) {
    return { __pklTag: "dataSize", value: val.value, unit: val.unit };
  }
  if (val instanceof PklRegex) {
    return { __pklTag: "regex", pattern: val.pattern };
  }
  if (val instanceof PklPair) {
    return {
      __pklTag: "pair",
      first: dehydrateValue(val.first),
      second: dehydrateValue(val.second),
    };
  }
  if (val instanceof PklObject) {
    const properties = Object.entries(val.properties).map(([k, v]) => [k, dehydrateValue(v)]);
    const entries = Array.from(val.entries.entries(), ([k, v]) => [
      dehydrateValue(k),
      dehydrateValue(v),
    ]);
    const elements = val.elements.map(dehydrateValue);
    return {
      __pklTag: "object",
      className: val.className,
      moduleUri: val.moduleUri,
      properties,
      entries,
      elements,
    };
  }
  if (val instanceof PklClass) {
    return { __pklTag: "class", name: val.name, moduleUri: val.moduleUri };
  }
  if (val instanceof PklTypeAlias) {
    return { __pklTag: "typeAlias", name: val.name, moduleUri: val.moduleUri };
  }
  if (val instanceof PklIntSeq) {
    return {
      __pklTag: "intSeq",
      start: dehydrateValue(val.start),
      end: dehydrateValue(val.end),
      step: dehydrateValue(val.step),
    };
  }
  if (val instanceof PklFunction) {
    return { __pklTag: "function" };
  }
  if (val instanceof PklReference) {
    return {
      __pklTag: "reference",
      domain: dehydrateValue(val.domain),
      data: dehydrateValue(val.data),
      path: val.path.map(dehydrateValue),
    };
  }
  if (typeof val === "object") {
    const properties = Object.entries(val).map(([k, v]) => [k, dehydrateValue(v)]);
    return {
      __pklTag: "object",
      className: "Dynamic",
      moduleUri: "pkl:base",
      properties,
      entries: [],
      elements: [],
    };
  }
  throw new PklError(`Unsupported JS value type for Pkl conversion: ${typeof val}`, "invalid-argument");
}

function parseTimeoutMs(value) {
  if (value === undefined || value === null) return undefined;
  if (typeof value === "bigint") {
    if (value < 0n || value > MAX_U64) {
      throw new PklError(
        `timeoutMs must be an unsigned 64-bit integer (0..=${MAX_U64}), got ${value}.`,
        "invalid-argument",
      );
    }
    return value;
  }
  if (typeof value === "number") {
    if (!Number.isFinite(value) || !Number.isInteger(value) || !Number.isSafeInteger(value) || value < 0) {
      throw new PklError(
        `timeoutMs must be a non-negative safe integer or unsigned 64-bit BigInt, got ${value}.`,
        "invalid-argument",
      );
    }
    return BigInt(value);
  }
  throw new PklError(
    `timeoutMs must be a number or bigint, got ${typeof value}.`,
    "invalid-argument",
  );
}

function toKeyValueList(input, fieldName) {
  if (input === undefined || input === null) return undefined;
  const out = [];
  if (input instanceof Map) {
    for (const [k, v] of input.entries()) {
      if (v === undefined) continue;
      out.push({ key: String(k), value: String(v) });
    }
    return out;
  }
  if (typeof input === "object" && !Array.isArray(input)) {
    for (const [k, v] of Object.entries(input)) {
      if (v === undefined) continue;
      out.push({ key: k, value: String(v) });
    }
    return out;
  }
  throw new PklError(`${fieldName} must be an object or Map.`, "invalid-argument");
}

function decodeU16(arr) {
  if (!arr || arr.length === 0) return "";
  const CHUNK = 8192;
  if (arr.length <= CHUNK) {
    return String.fromCharCode(...arr);
  }
  let out = "";
  for (let i = 0; i < arr.length; i += CHUNK) {
    out += String.fromCharCode(...arr.subarray(i, Math.min(i + CHUNK, arr.length)));
  }
  return out;
}

function decodeBridgeFrame(frame) {
  if (!frame) return undefined;
  return {
    moduleUri: decodeU16(frame.moduleUriU16),
    memberName: frame.memberNameU16 ? decodeU16(frame.memberNameU16) : null,
    sourceLines: (frame.sourceLinesU16 || []).map(decodeU16),
    startLine: frame.startLine,
    startColumn: frame.startColumn,
    endLine: frame.endLine,
    endColumn: frame.endColumn,
  };
}

function normalizeColorOption(color) {
  if (color === undefined || color === null) return undefined;
  if (typeof color === "boolean") return color ? "always" : "never";
  if (typeof color === "string") {
    const lower = color.toLowerCase();
    if (lower === "never" || lower === "auto" || lower === "always") {
      return lower;
    }
  }
  throw new PklError(
    `Invalid color option "${color}". Expected boolean or "never" | "auto" | "always".`,
    "invalid-argument",
  );
}

function buildNativeOptions(options = {}) {
  if (options !== undefined && options !== null && typeof options !== "object") {
    throw new PklError("EvaluatorOptions must be an object.", "invalid-argument");
  }
  const opts = options || {};
  const nativeOpts = {};

  if (opts.allowedModules !== undefined) {
    if (!Array.isArray(opts.allowedModules)) {
      throw new PklError("allowedModules must be an array of strings.", "invalid-argument");
    }
    nativeOpts.allowedModules = opts.allowedModules.map(String);
  }
  if (opts.allowedResources !== undefined) {
    if (!Array.isArray(opts.allowedResources)) {
      throw new PklError("allowedResources must be an array of strings.", "invalid-argument");
    }
    nativeOpts.allowedResources = opts.allowedResources.map(String);
  }
  if (opts.rootDir !== undefined && opts.rootDir !== null) {
    nativeOpts.rootDir = String(opts.rootDir);
  }
  if (opts.workingDir !== undefined && opts.workingDir !== null) {
    nativeOpts.workingDir = String(opts.workingDir);
  }
  if (opts.modulePaths !== undefined) {
    if (!Array.isArray(opts.modulePaths)) {
      throw new PklError("modulePaths must be an array of paths.", "invalid-argument");
    }
    nativeOpts.modulePaths = opts.modulePaths.map(String);
  }
  if (opts.properties !== undefined) {
    nativeOpts.properties = toKeyValueList(opts.properties, "properties");
  }
  if (opts.env !== undefined) {
    nativeOpts.env = toKeyValueList(opts.env, "env");
  } else if (opts.project === undefined && opts.projectDir === undefined) {
    // Default to current Node process.env when neither explicit env nor project overrides it.
    nativeOpts.env = toKeyValueList(process.env, "env");
  }
  if (opts.outputFormat !== undefined && opts.outputFormat !== null) {
    nativeOpts.outputFormat = String(opts.outputFormat);
  }
  if (opts.traceMode !== undefined && opts.traceMode !== null) {
    if (opts.traceMode !== "compact" && opts.traceMode !== "pretty") {
      throw new PklError(
        `Invalid traceMode "${opts.traceMode}". Expected "compact" or "pretty".`,
        "invalid-argument",
      );
    }
    nativeOpts.traceMode = opts.traceMode;
  }
  if (opts.powerAssertions !== undefined) {
    nativeOpts.powerAssertions = Boolean(opts.powerAssertions);
  }
  const color = normalizeColorOption(opts.color);
  if (color !== undefined) {
    nativeOpts.color = color;
  }
  const timeoutMs = parseTimeoutMs(opts.timeoutMs);
  if (timeoutMs !== undefined) {
    nativeOpts.timeoutMs = timeoutMs;
  }
  if (opts.noCache === true || opts.cacheDir === null) {
    nativeOpts.noCache = true;
  } else if (opts.cacheDir !== undefined) {
    nativeOpts.cacheDir = String(opts.cacheDir);
  }
  if (opts.projectDir !== undefined && opts.projectDir !== null) {
    nativeOpts.projectDir = String(opts.projectDir);
  }
  if (opts.testMode !== undefined) {
    nativeOpts.testMode = Boolean(opts.testMode);
  }

  if (opts.http !== undefined && opts.http !== null) {
    const http = opts.http;
    const nativeHttp = {};
    const certFiles = [];
    const certPems = [];
    if (Array.isArray(http.caCertificates)) {
      for (const cert of http.caCertificates) {
        if (cert instanceof Uint8Array) {
          certPems.push(new Uint8Array(cert.buffer, cert.byteOffset, cert.byteLength));
        } else if (typeof cert === "string") {
          if (cert.includes("-----BEGIN")) {
            certPems.push(Buffer.from(cert, "utf8"));
          } else {
            certFiles.push(cert);
          }
        } else {
          throw new PklError(
            "http.caCertificates entries must be file paths, PEM strings, or Uint8Array.",
            "invalid-argument",
          );
        }
      }
    }
    if (Array.isArray(http.caCertificateFiles)) {
      for (const filePath of http.caCertificateFiles) {
        if (typeof filePath !== "string") {
          throw new PklError(
            "http.caCertificateFiles entries must be file path strings.",
            "invalid-argument",
          );
        }
        certFiles.push(filePath);
      }
    }
    if (Array.isArray(http.caCertificatePems)) {
      for (const pem of http.caCertificatePems) {
        if (pem instanceof Uint8Array) {
          certPems.push(new Uint8Array(pem.buffer, pem.byteOffset, pem.byteLength));
        } else if (typeof pem === "string") {
          certPems.push(Buffer.from(pem, "utf8"));
        } else {
          throw new PklError(
            "http.caCertificatePems entries must be PEM strings or Uint8Array.",
            "invalid-argument",
          );
        }
      }
    }
    if (certFiles.length > 0) nativeHttp.caCertificateFiles = certFiles;
    if (certPems.length > 0) nativeHttp.caCertificatePems = certPems;
    if (http.proxy !== undefined && http.proxy !== null) {
      if (http.proxy.address !== undefined && http.proxy.address !== null) {
        nativeHttp.proxyAddress = String(http.proxy.address);
      }
      if (Array.isArray(http.proxy.noProxy)) {
        nativeHttp.noProxy = http.proxy.noProxy.map(String);
      }
    }
    if (http.rewrites !== undefined && http.rewrites !== null) {
      nativeHttp.rewrites = toKeyValueList(http.rewrites, "http.rewrites");
    }
    if (http.headers !== undefined && http.headers !== null) {
      const entries = [];
      for (const [urlPattern, hdrMap] of Object.entries(http.headers)) {
        if (!hdrMap || typeof hdrMap !== "object") continue;
        for (const [name, value] of Object.entries(hdrMap)) {
          entries.push({ urlPattern, name, value: String(value) });
        }
      }
      nativeHttp.headers = entries;
    }
    if (http.testPort !== undefined && http.testPort !== null) {
      nativeHttp.testPort = Number(http.testPort);
    }
    nativeOpts.http = nativeHttp;
  }

  if (opts.externalModuleReaders && typeof opts.externalModuleReaders === "object") {
    nativeOpts.externalModuleReaders = Object.entries(opts.externalModuleReaders).map(
      ([scheme, cfg]) => ({
        scheme: scheme.replace(/:$/, ""),
        executable: String(cfg.executable),
        arguments: Array.isArray(cfg.arguments) ? cfg.arguments.map(String) : [],
      }),
    );
  }
  if (opts.externalResourceReaders && typeof opts.externalResourceReaders === "object") {
    nativeOpts.externalResourceReaders = Object.entries(opts.externalResourceReaders).map(
      ([scheme, cfg]) => ({
        scheme: scheme.replace(/:$/, ""),
        executable: String(cfg.executable),
        arguments: Array.isArray(cfg.arguments) ? cfg.arguments.map(String) : [],
      }),
    );
  }

  const moduleReaders = Array.isArray(opts.moduleReaders) ? opts.moduleReaders : [];
  if (moduleReaders.length > 0) {
    nativeOpts.moduleReaders = moduleReaders.map((r, index) => {
      if (!r || typeof r.scheme !== "string" || typeof r.read !== "function") {
        throw new PklError(
          `moduleReaders[${index}] must have string scheme and read() function.`,
          "invalid-argument",
        );
      }
      return {
        index,
        scheme: r.scheme.replace(/:$/, ""),
        isLocal: Boolean(r.isLocal),
        isGlobbable: Boolean(r.isGlobbable),
        hasHierarchicalUris: Boolean(r.hasHierarchicalUris),
        isCached: r.isCached === undefined ? true : Boolean(r.isCached),
        hasHasElement: typeof r.hasElement === "function",
        hasListElements: typeof r.listElements === "function",
      };
    });
  }

  const resourceReaders = Array.isArray(opts.resourceReaders) ? opts.resourceReaders : [];
  if (resourceReaders.length > 0) {
    nativeOpts.resourceReaders = resourceReaders.map((r, index) => {
      if (!r || typeof r.scheme !== "string" || typeof r.read !== "function") {
        throw new PklError(
          `resourceReaders[${index}] must have string scheme and read() function.`,
          "invalid-argument",
        );
      }
      return {
        index,
        scheme: r.scheme.replace(/:$/, ""),
        isGlobbable: Boolean(r.isGlobbable),
        hasHierarchicalUris: Boolean(r.hasHierarchicalUris),
        hasHasElement: typeof r.hasElement === "function",
        hasListElements: typeof r.listElements === "function",
      };
    });
  }

  const logger = opts.logger || null;
  if (logger !== null) {
    nativeOpts.hasCustomLogger = true;
    nativeOpts.hasLoggerTrace = typeof logger.trace === "function";
    nativeOpts.hasLoggerWarn = typeof logger.warn === "function";
  }

  const frameTransformer = typeof opts.frameTransformer === "function" ? opts.frameTransformer : null;
  if (frameTransformer) {
    nativeOpts.hasFrameTransformer = true;
  }

  const securityManager = opts.securityManager || null;
  if (securityManager) {
    nativeOpts.hasSecurityResolveModule = typeof securityManager.checkResolveModule === "function";
    nativeOpts.hasSecurityImportModule = typeof securityManager.checkImportModule === "function";
    nativeOpts.hasSecurityReadResource = typeof securityManager.checkReadResource === "function";
    nativeOpts.hasSecurityResolveResource =
      typeof securityManager.checkResolveResource === "function";
  }

  const hasCallbacks = Boolean(
    logger ||
      frameTransformer ||
      (securityManager &&
        (nativeOpts.hasSecurityResolveModule ||
          nativeOpts.hasSecurityImportModule ||
          nativeOpts.hasSecurityReadResource ||
          nativeOpts.hasSecurityResolveResource)) ||
      moduleReaders.length > 0 ||
      resourceReaders.length > 0,
  );

  let nativeProject = undefined;
  if (opts.project !== undefined && opts.project !== null) {
    if (opts.project instanceof Project && opts.project._native) {
      nativeProject = opts.project._native;
    } else {
      throw new PklError("options.project must be a Project instance.", "invalid-argument");
    }
  }

  let bridgeTarget = null;
  const setBridgeTarget = (target) => {
    bridgeTarget = target;
  };

  const bridgeCallback = hasCallbacks
    ? (req) => {
        void (async () => {
          const target = bridgeTarget;
          if (!target) return;
          try {
            const payload = await dispatchBridgeRequest(req, {
              logger,
              frameTransformer,
              securityManager,
              moduleReaders,
              resourceReaders,
            });
            target.completeBridgeCallback(req.id, { ok: true, ...payload });
          } catch (err) {
            const isSecurity = err instanceof PklError && err.kind === "security";
            const isIo = err instanceof PklError && err.kind === "io";
            const msg = err instanceof Error ? err.message : String(err);
            target.completeBridgeCallback(req.id, {
              ok: false,
              errorKind: isSecurity ? "security" : isIo ? "io" : "evaluation",
              errorMessage: msg,
            });
          }
        })();
      }
    : undefined;

  return {
    nativeOpts,
    nativeProject,
    bridgeCallback,
    setBridgeTarget,
    signal: opts.signal,
  };
}

async function dispatchBridgeRequest(req, hooks) {
  switch (req.op) {
    case "log": {
      const msg = decodeU16(req.messageU16);
      const frame = decodeBridgeFrame(req.frame);
      if (req.level === "trace" && hooks.logger && typeof hooks.logger.trace === "function") {
        await hooks.logger.trace(msg, frame);
      } else if (req.level === "warn" && hooks.logger && typeof hooks.logger.warn === "function") {
        await hooks.logger.warn(msg, frame);
      }
      return {};
    }
    case "transformFrame": {
      const frame = decodeBridgeFrame(req.frame);
      const transformed = await hooks.frameTransformer(frame);
      if (!transformed || typeof transformed !== "object") {
        return {};
      }
      return {
        frame: {
          moduleUri: String(transformed.moduleUri ?? frame.moduleUri),
          memberName:
            transformed.memberName === undefined
              ? frame.memberName
              : transformed.memberName === null
                ? undefined
                : String(transformed.memberName),
          sourceLines: Array.isArray(transformed.sourceLines)
            ? transformed.sourceLines.map(String)
            : frame.sourceLines,
          startLine: Number(transformed.startLine ?? frame.startLine),
          startColumn: Number(transformed.startColumn ?? frame.startColumn),
          endLine: Number(transformed.endLine ?? frame.endLine),
          endColumn: Number(transformed.endColumn ?? frame.endColumn),
        },
      };
    }
    case "checkResolveModule": {
      await hooks.securityManager.checkResolveModule(decodeU16(req.uriU16));
      return {};
    }
    case "checkImportModule": {
      await hooks.securityManager.checkImportModule(
        decodeU16(req.uriU16),
        decodeU16(req.uri2U16),
      );
      return {};
    }
    case "checkReadResource": {
      await hooks.securityManager.checkReadResource(decodeU16(req.uriU16));
      return {};
    }
    case "checkResolveResource": {
      await hooks.securityManager.checkResolveResource(decodeU16(req.uriU16));
      return {};
    }
    case "moduleRead": {
      const reader = hooks.moduleReaders[req.readerIndex];
      const text = await reader.read(decodeU16(req.uriU16));
      if (text === null || text === undefined) {
        return { text: undefined };
      }
      return { text: String(text) };
    }
    case "moduleHasElement": {
      const reader = hooks.moduleReaders[req.readerIndex];
      const exists = await reader.hasElement(decodeU16(req.uriU16));
      return { boolValue: Boolean(exists) };
    }
    case "moduleListElements": {
      const reader = hooks.moduleReaders[req.readerIndex];
      const elements = await reader.listElements(decodeU16(req.uriU16));
      return {
        elements: Array.isArray(elements)
          ? elements.map((el) => ({
              name: String(el.name),
              isDirectory: Boolean(el.isDirectory),
            }))
          : [],
      };
    }
    case "resourceRead": {
      const reader = hooks.resourceReaders[req.readerIndex];
      const value = await reader.read(decodeU16(req.uriU16));
      if (value === null || value === undefined) {
        return { missing: true };
      }
      if (value instanceof Uint8Array) {
        return { bytes: new Uint8Array(value.buffer, value.byteOffset, value.byteLength) };
      }
      if (typeof value === "string") {
        return { text: value };
      }
      if (typeof value === "object" && value.bytes instanceof Uint8Array) {
        return {
          bytes: new Uint8Array(value.bytes.buffer, value.bytes.byteOffset, value.bytes.byteLength),
        };
      }
      if (typeof value === "object" && typeof value.text === "string") {
        return { text: value.text };
      }
      throw new PklError(
        `ResourceReader.read must return Uint8Array, string, null, or undefined.`,
        "invalid-argument",
      );
    }
    case "resourceHasElement": {
      const reader = hooks.resourceReaders[req.readerIndex];
      const exists = await reader.hasElement(decodeU16(req.uriU16));
      return { boolValue: Boolean(exists) };
    }
    case "resourceListElements": {
      const reader = hooks.resourceReaders[req.readerIndex];
      const elements = await reader.listElements(decodeU16(req.uriU16));
      return {
        elements: Array.isArray(elements)
          ? elements.map((el) => ({
              name: String(el.name),
              isDirectory: Boolean(el.isDirectory),
            }))
          : [],
      };
    }
    default:
      throw new PklError(`Unknown bridge callback op: ${req.op}`, "evaluation");
  }
}

class Project {
  constructor(nativeProject) {
    this._native = nativeProject;
    const info = nativeProject.info();
    this.projectDir = info.projectDir;
    this.projectFileUri = info.projectFileUri;
    this.projectBaseUri = info.projectBaseUri;
    this.package = info.package;
    this.evaluatorSettings = info.evaluatorSettings;
    this.tests = info.tests;
    Object.freeze(this);
  }

  static async load(dirOrFile, options = undefined) {
    if (typeof dirOrFile !== "string" || dirOrFile.length === 0) {
      throw new PklError("Project.load requires a directory or file path.", "invalid-argument");
    }
    const env = options?.env !== undefined ? toKeyValueList(options.env, "env") : undefined;
    const props =
      options?.properties !== undefined
        ? toKeyValueList(options.properties, "properties")
        : undefined;
    const outcome = await native.NativeProject.load(dirOrFile, env, props);
    return new Project(unwrapOutcome(outcome));
  }

  static loadSync(dirOrFile, options = undefined) {
    if (typeof dirOrFile !== "string" || dirOrFile.length === 0) {
      throw new PklError("Project.loadSync requires a directory or file path.", "invalid-argument");
    }
    const env = options?.env !== undefined ? toKeyValueList(options.env, "env") : undefined;
    const props =
      options?.properties !== undefined
        ? toKeyValueList(options.properties, "properties")
        : undefined;
    const outcome = native.NativeProject.loadSync(dirOrFile, env, props);
    return new Project(unwrapOutcome(outcome));
  }
}

class Evaluator {
  constructor(nativeEval, signal = undefined) {
    this._native = nativeEval;
    this._closed = false;
    this._closePromise = null;
    this._nextCallId = 1n;
    this._signal = signal || null;
    this._onAbort = null;
    if (this._signal && !this._signal.aborted) {
      this._onAbort = () => {
        this.cancel();
      };
      this._signal.addEventListener("abort", this._onAbort, { once: true });
    }
  }

  get isClosed() {
    return this._closed || this._native.isClosed;
  }

  cancel() {
    if (!this._closed) {
      this._native.cancel();
    }
  }

  close() {
    if (this._closed) {
      return this._closePromise || Promise.resolve();
    }
    this._closed = true;
    if (this._signal && this._onAbort) {
      this._signal.removeEventListener("abort", this._onAbort);
      this._onAbort = null;
    }
    const closedImmediately = this._native.closeNow();
    if (closedImmediately) {
      this._closePromise = Promise.resolve();
      return this._closePromise;
    }
    this._closePromise = this._native.closeAsync();
    return this._closePromise;
  }

  async _runAsync(callOptions, invoke) {
    if (this.isClosed) {
      throw new PklError("Evaluator has been closed.", "closed");
    }
    const callSignal = callOptions?.signal;
    if (this._signal?.aborted || callSignal?.aborted) {
      throw new PklError("Evaluation was cancelled.", "cancelled");
    }
    const timeoutMs = parseTimeoutMs(callOptions?.timeoutMs);
    const callId = this._nextCallId++;
    let onCallAbort = null;
    if (callSignal) {
      onCallAbort = () => {
        this._native.cancelCall(callId);
      };
      callSignal.addEventListener("abort", onCallAbort, { once: true });
    }
    try {
      const outcome = await invoke(timeoutMs, callId);
      return unwrapOutcome(outcome);
    } finally {
      if (callSignal && onCallAbort) {
        callSignal.removeEventListener("abort", onCallAbort);
      }
    }
  }

  _runSync(callOptions, invoke) {
    if (this.isClosed) {
      throw new PklError("Evaluator has been closed.", "closed");
    }
    if (this._signal?.aborted || callOptions?.signal?.aborted) {
      throw new PklError("Evaluation was cancelled.", "cancelled");
    }
    const timeoutMs = parseTimeoutMs(callOptions?.timeoutMs);
    return unwrapOutcome(invoke(timeoutMs));
  }

  async evaluateOutputText(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    return this._runAsync(callOptions, (t, id) =>
      this._native.evaluateOutputText(uri, contents, t, id),
    );
  }

  evaluateOutputTextSync(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    return this._runSync(callOptions, (t) =>
      this._native.evaluateOutputTextSync(uri, contents, t),
    );
  }

  async evaluateOutputBytes(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    return this._runAsync(callOptions, (t, id) =>
      this._native.evaluateOutputBytes(uri, contents, t, id),
    );
  }

  evaluateOutputBytesSync(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    return this._runSync(callOptions, (t) =>
      this._native.evaluateOutputBytesSync(uri, contents, t),
    );
  }

  async evaluateOutputFiles(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const entries = await this._runAsync(callOptions, (t, id) =>
      this._native.evaluateOutputFiles(uri, contents, t, id),
    );
    const out = new Map();
    for (const entry of entries) {
      out.set(entry.path, entry.bytes);
    }
    return out;
  }

  evaluateOutputFilesSync(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const entries = this._runSync(callOptions, (t) =>
      this._native.evaluateOutputFilesSync(uri, contents, t),
    );
    const out = new Map();
    for (const entry of entries) {
      out.set(entry.path, entry.bytes);
    }
    return out;
  }

  async evaluateOutputValue(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const raw = await this._runAsync(callOptions, (t, id) =>
      this._native.evaluateOutputValue(uri, contents, t, id),
    );
    return hydrateValue(raw);
  }

  evaluateOutputValueSync(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const raw = this._runSync(callOptions, (t) =>
      this._native.evaluateOutputValueSync(uri, contents, t),
    );
    return hydrateValue(raw);
  }

  async evaluateExpression(source, expr, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    if (typeof expr !== "string") {
      throw new PklError("Expression must be a string.", "invalid-argument");
    }
    return this._runAsync(callOptions, (t, id) =>
      this._native.evaluateExpression(uri, contents, expr, t, id),
    );
  }

  evaluateExpressionSync(source, expr, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    if (typeof expr !== "string") {
      throw new PklError("Expression must be a string.", "invalid-argument");
    }
    return this._runSync(callOptions, (t) =>
      this._native.evaluateExpressionSync(uri, contents, expr, t),
    );
  }

  async evaluateExpressionValue(source, expr, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    if (typeof expr !== "string") {
      throw new PklError("Expression must be a string.", "invalid-argument");
    }
    const raw = await this._runAsync(callOptions, (t, id) =>
      this._native.evaluateExpressionValue(uri, contents, expr, t, id),
    );
    return hydrateValue(raw);
  }

  evaluateExpressionValueSync(source, expr, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    if (typeof expr !== "string") {
      throw new PklError("Expression must be a string.", "invalid-argument");
    }
    const raw = this._runSync(callOptions, (t) =>
      this._native.evaluateExpressionValueSync(uri, contents, expr, t),
    );
    return hydrateValue(raw);
  }

  async evaluateExpressionPklBinary(source, expr, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    if (typeof expr !== "string") {
      throw new PklError("Expression must be a string.", "invalid-argument");
    }
    return this._runAsync(callOptions, (t, id) =>
      this._native.evaluateExpressionPklBinary(uri, contents, expr, t, id),
    );
  }

  evaluateExpressionPklBinarySync(source, expr, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    if (typeof expr !== "string") {
      throw new PklError("Expression must be a string.", "invalid-argument");
    }
    return this._runSync(callOptions, (t) =>
      this._native.evaluateExpressionPklBinarySync(uri, contents, expr, t),
    );
  }

  async evaluateSchema(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const schema = await this._runAsync(callOptions, (t, id) =>
      this._native.evaluateSchema(uri, contents, t, id),
    );
    return hydrateSchema(schema);
  }

  evaluateSchemaSync(source, callOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const schema = this._runSync(callOptions, (t) =>
      this._native.evaluateSchemaSync(uri, contents, t),
    );
    return hydrateSchema(schema);
  }

  async evaluateTest(source, testOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const overwrite = Boolean(testOptions?.overwrite);
    const reporter = testOptions?.reporter ?? "spec";
    return this._runAsync(testOptions, (t, id) =>
      this._native.evaluateTest(uri, contents, overwrite, reporter, t, id),
    );
  }

  evaluateTestSync(source, testOptions = undefined) {
    const { uri, contents } = normalizeSource(source);
    const overwrite = Boolean(testOptions?.overwrite);
    const reporter = testOptions?.reporter ?? "spec";
    return this._runSync(testOptions, (t) =>
      this._native.evaluateTestSync(uri, contents, overwrite, reporter, t),
    );
  }
}

function createEvaluatorSync(options = undefined) {
  const { nativeOpts, nativeProject, bridgeCallback, setBridgeTarget, signal } =
    buildNativeOptions(options);
  const nativeEval = unwrapOutcome(
    native.NativeEvaluator.create(nativeOpts, nativeProject, bridgeCallback),
  );
  setBridgeTarget(nativeEval);
  return new Evaluator(nativeEval, signal);
}

async function createEvaluator(options = undefined) {
  return createEvaluatorSync(options);
}

class ReplSession {
  constructor(nativeRepl, signal = undefined) {
    this._native = nativeRepl;
    this._closed = false;
    this._closePromise = null;
    this._signal = signal || null;
    this._onAbort = null;
    if (this._signal && !this._signal.aborted) {
      this._onAbort = () => {
        this._native.cancel();
      };
      this._signal.addEventListener("abort", this._onAbort, { once: true });
    }
  }

  get isClosed() {
    return this._closed || this._native.isClosed;
  }

  _checkOpen() {
    if (this.isClosed) {
      throw new PklError("REPL session has been closed.", "closed");
    }
    if (this._signal?.aborted) {
      throw new PklError("Evaluation was cancelled.", "cancelled");
    }
  }

  async eval(input, options = undefined) {
    this._checkOpen();
    if (typeof input !== "string") {
      throw new PklError("ReplSession.eval input must be a string.", "invalid-argument");
    }
    const force = Boolean(options?.force);
    const id = options?.id !== undefined ? String(options.id) : undefined;
    return unwrapOutcome(await this._native.eval(input, force, id));
  }

  async load(uriOrSource, options = undefined) {
    this._checkOpen();
    let uri;
    if (typeof uriOrSource === "string") {
      uri = uriOrSource;
    } else if (uriOrSource && typeof uriOrSource === "object" && typeof uriOrSource.uri === "string") {
      uri = uriOrSource.uri;
    } else {
      throw new PklError(
        "ReplSession.load requires a URI string or ModuleSource.",
        "invalid-argument",
      );
    }
    const id = options?.id !== undefined ? String(options.id) : undefined;
    return unwrapOutcome(await this._native.load(uri, id));
  }

  async complete(prefix, options = undefined) {
    this._checkOpen();
    if (typeof prefix !== "string") {
      throw new PklError("ReplSession.complete prefix must be a string.", "invalid-argument");
    }
    const id = options?.id !== undefined ? String(options.id) : undefined;
    return unwrapOutcome(await this._native.complete(prefix, id));
  }

  async reset(options = undefined) {
    this._checkOpen();
    const id = options?.id !== undefined ? String(options.id) : undefined;
    unwrapOutcome(await this._native.reset(id));
  }

  close() {
    if (this._closed) {
      return this._closePromise || Promise.resolve();
    }
    this._closed = true;
    if (this._signal && this._onAbort) {
      this._signal.removeEventListener("abort", this._onAbort);
      this._onAbort = null;
    }
    if (this._native.close()) {
      this._closePromise = Promise.resolve();
      return this._closePromise;
    }
    this._closePromise = this._native.closeAsync();
    return this._closePromise;
  }
}

async function createRepl(options = undefined) {
  const { nativeOpts, nativeProject, bridgeCallback, setBridgeTarget, signal } =
    buildNativeOptions(options);
  const nativeRepl = unwrapOutcome(
    native.NativeReplSession.create(nativeOpts, nativeProject, bridgeCallback),
  );
  setBridgeTarget(nativeRepl);
  return new ReplSession(nativeRepl, signal);
}

async function analyzeImportGraph(uris, options = undefined) {
  if (!Array.isArray(uris)) {
    throw new PklError("analyzeImportGraph uris must be an array.", "invalid-argument");
  }
  const normalized = uris.map((item) => {
    if (typeof item === "string") return item;
    if (item && typeof item === "object" && typeof item.uri === "string") return item.uri;
    throw new PklError(
      "analyzeImportGraph entries must be URI strings or ModuleSource instances.",
      "invalid-argument",
    );
  });
  const { nativeOpts, nativeProject, bridgeCallback, setBridgeTarget, signal } =
    buildNativeOptions(options);
  if (signal?.aborted) {
    throw new PklError("Import graph analysis was cancelled.", "cancelled");
  }
  const analyzer = unwrapOutcome(
    native.NativeAnalyzer.create(nativeOpts, nativeProject, bridgeCallback),
  );
  setBridgeTarget(analyzer);
  let onAbort = null;
  if (signal) {
    onAbort = () => analyzer.cancel();
    signal.addEventListener("abort", onAbort, { once: true });
  }
  try {
    return unwrapOutcome(await analyzer.importGraph(normalized));
  } finally {
    if (signal && onAbort) {
      signal.removeEventListener("abort", onAbort);
    }
  }
}

function format(source, options = undefined) {
  if (typeof source !== "string") {
    throw new PklError("format source must be a string.", "invalid-argument");
  }
  const grammarVersion = options?.grammarVersion;
  if (grammarVersion !== undefined && grammarVersion !== 1 && grammarVersion !== 2) {
    throw new PklError(
      `Invalid grammarVersion "${grammarVersion}". Expected 1 or 2.`,
      "invalid-argument",
    );
  }
  return unwrapOutcome(native.productFormat(source, grammarVersion));
}

function decodePklBinary(bytes) {
  if (!(bytes instanceof Uint8Array)) {
    throw new PklError("decodePklBinary expects a Uint8Array.", "invalid-argument");
  }
  const raw = unwrapOutcome(
    native.productDecodePklBinary(new Uint8Array(bytes.buffer, bytes.byteOffset, bytes.byteLength)),
  );
  return hydrateValue(raw);
}

function monotonicClockMs() {
  return native.productMonotonicClockMs();
}

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

function measureConversion(valOrIterations = 1000, maybeIterations = undefined) {
  let value;
  let iterations;
  if (typeof valOrIterations === "number" && maybeIterations === undefined) {
    value = defaultBenchmarkPayload();
    iterations = valOrIterations;
  } else {
    value = valOrIterations;
    iterations = maybeIterations === undefined ? 1000 : maybeIterations;
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
  return {
    iterations: nativeStats.iterations,
    elapsedMs: totalElapsedMs,
    nativeElapsedMs: nativeStats.elapsedMs,
    perIterationUs: (totalElapsedMs * 1000) / nativeStats.iterations,
    nativePerIterationUs: nativeStats.perIterationUs,
    lastValue: hydrated,
  };
}

module.exports = {
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
};
