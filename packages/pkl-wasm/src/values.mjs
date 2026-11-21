import { PklError } from './errors.mjs';

const I64_MIN = -9223372036854775808n;
const I64_MAX = 9223372036854775807n;

export class PklDuration {
  /**
   * @param {number} value
   * @param {string} unit
   */
  constructor(value, unit) {
    this.value = value;
    this.unit = unit;
  }
}

export class PklDataSize {
  /**
   * @param {number} value
   * @param {string} unit
   */
  constructor(value, unit) {
    this.value = value;
    this.unit = unit;
  }
}

export class PklPair {
  /**
   * @param {any} first
   * @param {any} second
   */
  constructor(first, second) {
    this.first = first;
    this.second = second;
  }
}

export class PklRegex {
  /**
   * @param {string} pattern
   */
  constructor(pattern) {
    this.pattern = pattern;
  }
}

export class PklObject {
  /**
   * @param {string} className
   * @param {string} moduleUri
   * @param {Record<string, any>} [properties]
   * @param {Map<any, any> | Iterable<[any, any]>} [entries]
   * @param {any[]} [elements]
   */
  constructor(className, moduleUri, properties = {}, entries = new Map(), elements = []) {
    this.className = className;
    this.moduleUri = moduleUri;
    this.properties = properties;
    this.entries = entries instanceof Map ? entries : new Map(entries);
    this.elements = Array.isArray(elements) ? elements : Array.from(elements);
  }
}

export class PklClass {
  /**
   * @param {string} name
   * @param {string} moduleUri
   */
  constructor(name, moduleUri) {
    this.name = name;
    this.moduleUri = moduleUri;
  }
}

export class PklTypeAlias {
  /**
   * @param {string} name
   * @param {string} moduleUri
   */
  constructor(name, moduleUri) {
    this.name = name;
    this.moduleUri = moduleUri;
  }
}

export class PklIntSeq {
  /**
   * @param {bigint} start
   * @param {bigint} end
   * @param {bigint} step
   */
  constructor(start, end, step) {
    if (typeof start !== 'bigint' || typeof end !== 'bigint' || typeof step !== 'bigint') {
      throw new PklError('PklIntSeq start, end, and step must be BigInt values', 'invalid-argument');
    }
    this.start = start;
    this.end = end;
    this.step = step;
  }
}

export class PklFunction {
  constructor() {}
}

export class PklReference {
  /**
   * @param {any} domain
   * @param {any} data
   * @param {any[]} path
   */
  constructor(domain, data, path) {
    this.domain = domain;
    this.data = data;
    this.path = path;
  }
}

/**
 * Hydrates a Rust-produced tagged value descriptor into public JS/TS PklValue instances.
 * @param {any} raw
 * @returns {any}
 */
export function hydrateValue(raw) {
  if (raw === null || raw === undefined) return null;
  const t = typeof raw;
  if (t === 'boolean' || t === 'bigint' || t === 'number' || t === 'string') {
    return raw;
  }
  if (raw instanceof Uint8Array) {
    return raw;
  }
  if (Array.isArray(raw)) {
    const out = new Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) {
      out[i] = hydrateValue(raw[i]);
    }
    return out;
  }
  if (t === 'object' && typeof raw.__pklTag === 'string') {
    switch (raw.__pklTag) {
      case 'duration':
        return new PklDuration(raw.value, raw.unit);
      case 'dataSize':
        return new PklDataSize(raw.value, raw.unit);
      case 'pair':
        return new PklPair(hydrateValue(raw.first), hydrateValue(raw.second));
      case 'regex':
        return new PklRegex(raw.pattern);
      case 'set': {
        const set = new Set();
        for (let i = 0; i < raw.items.length; i += 1) {
          set.add(hydrateValue(raw.items[i]));
        }
        return set;
      }
      case 'map': {
        const map = new Map();
        for (let i = 0; i < raw.entries.length; i += 1) {
          const pair = raw.entries[i];
          map.set(hydrateValue(pair[0]), hydrateValue(pair[1]));
        }
        return map;
      }
      case 'object': {
        const props = {};
        for (let i = 0; i < raw.properties.length; i += 1) {
          const [k, v] = raw.properties[i];
          Object.defineProperty(props, k, {
            value: hydrateValue(v),
            enumerable: true,
            writable: true,
            configurable: true,
          });
        }
        const entries = new Map();
        for (let i = 0; i < raw.entries.length; i += 1) {
          const [k, v] = raw.entries[i];
          entries.set(hydrateValue(k), hydrateValue(v));
        }
        const elements = new Array(raw.elements.length);
        for (let i = 0; i < raw.elements.length; i += 1) {
          elements[i] = hydrateValue(raw.elements[i]);
        }
        return new PklObject(raw.className, raw.moduleUri, props, entries, elements);
      }
      case 'intSeq':
        return new PklIntSeq(raw.start, raw.end, raw.step);
      case 'class':
        return new PklClass(raw.name, raw.moduleUri);
      case 'typeAlias':
        return new PklTypeAlias(raw.name, raw.moduleUri);
      case 'function':
        return new PklFunction();
      case 'reference':
        return new PklReference(
          hydrateValue(raw.domain),
          hydrateValue(raw.data),
          raw.path.map(hydrateValue),
        );
      default:
        throw new PklError(`Unknown Pkl value tag: ${raw.__pklTag}`, 'decode');
    }
  }
  return raw;
}

/**
 * Hydrates all `annotations` arrays inside a `ModuleSchema` tree into `PklObject` instances.
 * @param {any} schema
 * @returns {any}
 */
export function hydrateSchema(schema) {
  if (!schema || typeof schema !== 'object') return schema;
  const hydrateClass = (cls) => {
    if (!cls) return cls;
    cls.annotations = (cls.annotations || []).map(hydrateValue);
    for (const mapKey of ['properties', 'allProperties', 'methods', 'allMethods']) {
      const rec = cls[mapKey];
      if (rec && typeof rec === 'object') {
        for (const key of Object.keys(rec)) {
          rec[key].annotations = (rec[key].annotations || []).map(hydrateValue);
        }
      }
    }
    return cls;
  };
  const hydrateAlias = (alias) => {
    if (!alias) return alias;
    alias.annotations = (alias.annotations || []).map(hydrateValue);
    return alias;
  };
  schema.annotations = (schema.annotations || []).map(hydrateValue);
  schema.moduleClass = hydrateClass(schema.moduleClass);
  for (const mapKey of ['classes', 'allClasses']) {
    const rec = schema[mapKey];
    if (rec && typeof rec === 'object') {
      for (const key of Object.keys(rec)) {
        rec[key] = hydrateClass(rec[key]);
      }
    }
  }
  for (const mapKey of ['typeAliases', 'allTypeAliases']) {
    const rec = schema[mapKey];
    if (rec && typeof rec === 'object') {
      for (const key of Object.keys(rec)) {
        rec[key] = hydrateAlias(rec[key]);
      }
    }
  }
  if (schema.supermodule) {
    schema.supermodule = hydrateSchema(schema.supermodule);
  }
  return schema;
}

/**
 * Dehydrates a public JS/TS PklValue into a lossless Rust descriptor tree for `roundtripValue`.
 * @param {any} value
 * @returns {any}
 */
export function dehydrateValue(value) {
  if (value === null || value === undefined) return null;
  const t = typeof value;
  if (t === 'boolean' || t === 'number' || t === 'string') {
    return value;
  }
  if (t === 'bigint') {
    if (value < I64_MIN || value > I64_MAX) {
      throw new PklError(
        `BigInt ${value} is outside signed 64-bit range (${I64_MIN}..=${I64_MAX})`,
        'invalid-argument',
      );
    }
    return value;
  }
  if (value instanceof Uint8Array) {
    return value;
  }
  if (Array.isArray(value)) {
    return value.map(dehydrateValue);
  }
  if (value instanceof Set) {
    return {
      __pklTag: 'set',
      items: Array.from(value, dehydrateValue),
    };
  }
  if (value instanceof Map) {
    const entries = [];
    for (const [k, v] of value.entries()) {
      entries.push([dehydrateValue(k), dehydrateValue(v)]);
    }
    return { __pklTag: 'map', entries };
  }
  if (value instanceof PklDuration) {
    return { __pklTag: 'duration', value: value.value, unit: value.unit };
  }
  if (value instanceof PklDataSize) {
    return { __pklTag: 'dataSize', value: value.value, unit: value.unit };
  }
  if (value instanceof PklPair) {
    return {
      __pklTag: 'pair',
      first: dehydrateValue(value.first),
      second: dehydrateValue(value.second),
    };
  }
  if (value instanceof PklRegex) {
    return { __pklTag: 'regex', pattern: value.pattern };
  }
  if (value instanceof PklObject) {
    const properties = [];
    for (const [k, v] of Object.entries(value.properties || {})) {
      properties.push([k, dehydrateValue(v)]);
    }
    const entries = [];
    const entryIterable =
      value.entries instanceof Map ? value.entries.entries() : (value.entries || []);
    for (const [k, v] of entryIterable) {
      entries.push([dehydrateValue(k), dehydrateValue(v)]);
    }
    const elements = (value.elements || []).map(dehydrateValue);
    return {
      __pklTag: 'object',
      className: value.className,
      moduleUri: value.moduleUri,
      properties,
      entries,
      elements,
    };
  }
  if (value instanceof PklIntSeq) {
    return {
      __pklTag: 'intSeq',
      start: value.start,
      end: value.end,
      step: value.step,
    };
  }
  if (value instanceof PklClass) {
    return { __pklTag: 'class', name: value.name, moduleUri: value.moduleUri };
  }
  if (value instanceof PklTypeAlias) {
    return { __pklTag: 'typeAlias', name: value.name, moduleUri: value.moduleUri };
  }
  if (value instanceof PklFunction) {
    return { __pklTag: 'function' };
  }
  if (value instanceof PklReference) {
    return {
      __pklTag: 'reference',
      domain: dehydrateValue(value.domain),
      data: dehydrateValue(value.data),
      path: (value.path || []).map(dehydrateValue),
    };
  }
  throw new PklError(
    `Unsupported value type for Pkl conversion: ${Object.prototype.toString.call(value)}`,
    'invalid-argument',
  );
}
