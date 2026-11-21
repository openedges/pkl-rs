// SPDX-License-Identifier: Apache-2.0
// Deterministic lossless canonical serializers for M6 differential parity against M6OracleProbe.java.

export function encodeUtf16Hex(str) {
  let out = "";
  for (let i = 0; i < str.length; i++) {
    out += str.charCodeAt(i).toString(16).padStart(4, "0");
  }
  return out;
}

export function encodeBytesHex(u8) {
  let out = "";
  for (let i = 0; i < u8.length; i++) {
    out += u8[i].toString(16).padStart(2, "0");
  }
  return out;
}

export function formatFloatCanonical(n) {
  if (Number.isNaN(n)) return "NaN";
  if (!Number.isFinite(n)) return n > 0 ? "Infinity" : "-Infinity";
  if (Object.is(n, -0)) return "-0.0";
  // Match Java Double.toString for integer-valued floats like 1.0 -> "1.0"
  const s = String(n);
  if (/^-?\d+$/.test(s)) return s + ".0";
  return s;
}

export function serializeCanonicalValue(val, api) {
  if (val === null || val === undefined) return "null";
  if (typeof val === "boolean") return `bool:${val}`;
  if (typeof val === "bigint") return `int:${val.toString(10)}`;
  if (typeof val === "number") return `float:${formatFloatCanonical(val)}`;
  if (typeof val === "string") return `str:${encodeUtf16Hex(val)}`;
  if (val instanceof Uint8Array) return `bytes:${encodeBytesHex(val)}`;

  if (api?.PklDuration && val instanceof api.PklDuration) {
    return `duration:${formatFloatCanonical(val.value)}:${val.unit}`;
  }
  if (api?.PklDataSize && val instanceof api.PklDataSize) {
    return `datasize:${formatFloatCanonical(val.value)}:${val.unit}`;
  }
  if (api?.PklRegex && val instanceof api.PklRegex) {
    return `regex:${encodeUtf16Hex(val.pattern)}`;
  }
  if (api?.PklPair && val instanceof api.PklPair) {
    return `pair(${serializeCanonicalValue(val.first, api)}, ${serializeCanonicalValue(val.second, api)})`;
  }
  if (api?.PklIntSeq && val instanceof api.PklIntSeq) {
    return `intseq(${val.start.toString(10)},${val.end.toString(10)},${val.step.toString(10)})`;
  }
  if (api?.PklClass && val instanceof api.PklClass) {
    return `class(${val.name}|${val.moduleUri})`;
  }
  if (api?.PklTypeAlias && val instanceof api.PklTypeAlias) {
    return `typealias(${val.name}|${val.moduleUri})`;
  }
  if (api?.PklFunction && val instanceof api.PklFunction) {
    return "function()";
  }
  if (api?.PklObject && val instanceof api.PklObject) {
    const props = val.properties || {};
    const keys = Object.keys(props).sort();
    const parts = keys.map((k) => `${k}=${serializeCanonicalValue(props[k], api)}`);
    return `object(${val.className}|${val.moduleUri}){${parts.join(", ")}}`;
  }
  if (Array.isArray(val)) {
    return `list[${val.map((x) => serializeCanonicalValue(x, api)).join(", ")}]`;
  }
  if (val instanceof Set) {
    const parts = [];
    for (const elem of val) parts.push(serializeCanonicalValue(elem, api));
    return `set[${parts.join(", ")}]`;
  }
  if (val instanceof Map) {
    const parts = [];
    for (const [k, v] of val.entries()) {
      parts.push(`${serializeCanonicalValue(k, api)} => ${serializeCanonicalValue(v, api)}`);
    }
    return `map{${parts.join(", ")}}`;
  }
  throw new Error(`Unsupported PklValue runtime representation: ${Object.prototype.toString.call(val)}`);
}

export function serializeCanonicalSchema(schema) {
  const lines = [];
  const modName = schema.moduleName;
  const isAmend = Boolean(schema.isAmend);
  const supermod = schema.supermodule ?? null;
  const isExtend = supermod !== null && !isAmend;
  lines.push(`module=${modName}`);
  lines.push(`isAmend=${isAmend}`);
  lines.push(`isExtend=${isExtend}`);
  lines.push(`supermodule=${supermod === null ? "none" : supermod.moduleName}`);
  lines.push(`docComment=${schema.docComment == null ? "none" : String(schema.docComment).replace(/\n/g, "\\n")}`);

  const modCls = schema.moduleClass || {};
  const modProps = Object.keys(modCls.properties || {}).sort();
  lines.push(`properties=${modProps.join(",")}`);
  const modMethods = Object.keys(modCls.methods || {}).sort();
  lines.push(`methods=${modMethods.join(",")}`);

  const classesObj = schema.classes || {};
  const classNames = Object.keys(classesObj).sort();
  for (const cname of classNames) {
    const cls = classesObj[cname];
    const mods = new Set(cls.modifiers || []);
    const isAbs = typeof cls.isAbstract === "boolean" ? cls.isAbstract : mods.has("abstract");
    const isOpen = typeof cls.isOpen === "boolean" ? cls.isOpen : mods.has("open");
    const cprops = Object.keys(cls.properties || {}).sort();
    const cmeths = Object.keys(cls.methods || {}).sort();
    lines.push(`class=${cname}|abstract=${isAbs}|open=${isOpen}|props=${cprops.join(",")}|methods=${cmeths.join(",")}`);
  }

  const aliases = Object.keys(schema.typeAliases || {}).sort();
  lines.push(`typeAliases=${aliases.join(",")}`);
  const imports = Object.keys(schema.imports || {}).sort();
  lines.push(`imports=${imports.join(",")}`);
  return lines.join("\n") + "\n";
}
