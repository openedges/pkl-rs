import { PklError } from './errors.mjs';

const I64_MIN = -9223372036854775808n;
const I64_MAX = 9223372036854775807n;
const FAST_UTF8_DECODER = new TextDecoder('utf-8', { fatal: true, ignoreBOM: true });
const FAST_MODULE_URI_RE =
  /^[A-Za-z][A-Za-z0-9+.-]*:(?:\/\/\/[A-Za-z0-9\-._~/:$&'()*+,;=]*|\/(?!\/)[A-Za-z0-9\-._~/$&'()*+,;=]*|[A-Za-z0-9\-._~$&'()*+,;=][A-Za-z0-9\-._~/$&'()*+,;=]*)$/;
const VALID_DURATION_UNITS = new Set(['ns', 'us', 'ms', 's', 'min', 'h', 'd']);
const VALID_DATA_SIZE_UNITS = new Set([
  'b', 'kb', 'mb', 'gb', 'tb', 'pb', 'kib', 'mib', 'gib', 'tib', 'pib',
]);
const BLANK_OR_CTRL_RE = /^[\s\x00-\x20]*$/;

export const DECODE_FALLBACK = Symbol('pkl.decode.fallback');

const DECODE_STATE = new Int32Array(1);
const INTERN_OFF = new Int32Array(64);
const INTERN_LEN = new Int32Array(64);
const INTERN_STR = new Array(64);
const KEY_CACHE = new Map();
const SHORT_UTF8_CODES = new Uint16Array(64);

/**
 * Pure-JS RFC 3629 strict UTF-8 decoder for short strings (`<= 64` bytes).
 * Avoids `Uint8Array.subarray()` + WebIDL `TextDecoder.decode()` boundary crossings
 * (notably expensive in SpiderMonkey) while rejecting overlong sequences, surrogates,
 * out-of-range scalars, and truncated bytes via `DECODE_FALLBACK`.
 */
function fastDecodeShortUtf8(bytes, pos, end) {
  let outLen = 0;
  let i = pos;
  while (i < end) {
    const b0 = bytes[i++];
    if (b0 < 0x80) {
      SHORT_UTF8_CODES[outLen++] = b0;
    } else if (b0 >= 0xc2 && b0 <= 0xdf) {
      if (i >= end) return DECODE_FALLBACK;
      const b1 = bytes[i++];
      if ((b1 & 0xc0) !== 0x80) return DECODE_FALLBACK;
      SHORT_UTF8_CODES[outLen++] = ((b0 & 0x1f) << 6) | (b1 & 0x3f);
    } else if (b0 >= 0xe0 && b0 <= 0xef) {
      if (i + 1 >= end) return DECODE_FALLBACK;
      const b1 = bytes[i++];
      const b2 = bytes[i++];
      if ((b1 & 0xc0) !== 0x80 || (b2 & 0xc0) !== 0x80) return DECODE_FALLBACK;
      if (b0 === 0xe0 && b1 < 0xa0) return DECODE_FALLBACK;
      if (b0 === 0xed && b1 >= 0xa0) return DECODE_FALLBACK;
      SHORT_UTF8_CODES[outLen++] = ((b0 & 0x0f) << 12) | ((b1 & 0x3f) << 6) | (b2 & 0x3f);
    } else if (b0 >= 0xf0 && b0 <= 0xf4) {
      if (i + 2 >= end) return DECODE_FALLBACK;
      const b1 = bytes[i++];
      const b2 = bytes[i++];
      const b3 = bytes[i++];
      if ((b1 & 0xc0) !== 0x80 || (b2 & 0xc0) !== 0x80 || (b3 & 0xc0) !== 0x80) {
        return DECODE_FALLBACK;
      }
      if (b0 === 0xf0 && b1 < 0x90) return DECODE_FALLBACK;
      if (b0 === 0xf4 && b1 > 0x8f) return DECODE_FALLBACK;
      const cp =
        (((b0 & 0x07) << 18) | ((b1 & 0x3f) << 12) | ((b2 & 0x3f) << 6) | (b3 & 0x3f)) -
        0x10000;
      SHORT_UTF8_CODES[outLen++] = 0xd800 | (cp >> 10);
      SHORT_UTF8_CODES[outLen++] = 0xdc00 | (cp & 0x3ff);
    } else {
      return DECODE_FALLBACK;
    }
  }
  return String.fromCharCode.apply(null, SHORT_UTF8_CODES.subarray(0, outLen));
}

function internPropKey(s) {
  let k = KEY_CACHE.get(s);
  if (k !== undefined) return k;
  const tmp = { [s]: 0 };
  k = Object.keys(tmp)[0];
  if (KEY_CACHE.size < 256) KEY_CACHE.set(k, k);
  return k;
}

function fastReadStr(bytes, view, len, pos, asKey) {
  if (pos >= len) return DECODE_FALLBACK;
  const b = bytes[pos++];
  let n;
  if (b >= 0xa0 && b <= 0xbf) {
    n = b & 0x1f;
  } else if (b === 0xd9) {
    if (pos >= len) return DECODE_FALLBACK;
    n = bytes[pos++];
  } else if (b === 0xda) {
    if (pos + 2 > len) return DECODE_FALLBACK;
    n = view.getUint16(pos, false);
    pos += 2;
  } else if (b === 0xdb) {
    if (pos + 4 > len) return DECODE_FALLBACK;
    n = view.getUint32(pos, false);
    pos += 4;
  } else {
    return DECODE_FALLBACK;
  }
  const end = pos + n;
  if (end > len) return DECODE_FALLBACK;
  DECODE_STATE[0] = end;
  if (n === 0) return '';
  if (n <= 24) {
    const slot = (bytes[pos] * 31 + bytes[end - 1] * 17 + n * 7) & 63;
    if (INTERN_LEN[slot] === n) {
      const prev = INTERN_OFF[slot];
      let eq = true;
      for (let k = 0; k < n; k += 1) {
        if (bytes[prev + k] !== bytes[pos + k]) {
          eq = false;
          break;
        }
      }
      if (eq) return INTERN_STR[slot];
    }
    let ascii = true;
    for (let i = pos; i < end; i += 1) {
      if (bytes[i] >= 0x80) {
        ascii = false;
        break;
      }
    }
    if (ascii) {
      let s;
      if (n === 1) s = String.fromCharCode(bytes[pos]);
      else if (n === 2) s = String.fromCharCode(bytes[pos], bytes[pos + 1]);
      else if (n === 3) s = String.fromCharCode(bytes[pos], bytes[pos + 1], bytes[pos + 2]);
      else if (n === 4) {
        s = String.fromCharCode(bytes[pos], bytes[pos + 1], bytes[pos + 2], bytes[pos + 3]);
      } else if (n === 5) {
        s = String.fromCharCode(
          bytes[pos],
          bytes[pos + 1],
          bytes[pos + 2],
          bytes[pos + 3],
          bytes[pos + 4],
        );
      } else if (n === 6) {
        s = String.fromCharCode(
          bytes[pos],
          bytes[pos + 1],
          bytes[pos + 2],
          bytes[pos + 3],
          bytes[pos + 4],
          bytes[pos + 5],
        );
      } else if (n === 7) {
        s = String.fromCharCode(
          bytes[pos],
          bytes[pos + 1],
          bytes[pos + 2],
          bytes[pos + 3],
          bytes[pos + 4],
          bytes[pos + 5],
          bytes[pos + 6],
        );
      } else if (n === 8) {
        s = String.fromCharCode(
          bytes[pos],
          bytes[pos + 1],
          bytes[pos + 2],
          bytes[pos + 3],
          bytes[pos + 4],
          bytes[pos + 5],
          bytes[pos + 6],
          bytes[pos + 7],
        );
      } else {
        s = '';
        for (let i = pos; i < end; i += 1) s += String.fromCharCode(bytes[i]);
      }
      if (asKey) s = internPropKey(s);
      INTERN_OFF[slot] = pos;
      INTERN_LEN[slot] = n;
      INTERN_STR[slot] = s;
      return s;
    }
  }
  if (n <= 64) {
    const decoded = fastDecodeShortUtf8(bytes, pos, end);
    if (decoded === DECODE_FALLBACK) return DECODE_FALLBACK;
    return asKey ? internPropKey(decoded) : decoded;
  }
  try {
    return FAST_UTF8_DECODER.decode(bytes.subarray(pos, end));
  } catch {
    return DECODE_FALLBACK;
  }
}

function fastReadArrayLen(bytes, view, len, pos) {
  if (pos >= len) return -1;
  const b = bytes[pos++];
  if (b >= 0x90 && b <= 0x9f) {
    DECODE_STATE[0] = pos;
    return b & 0x0f;
  }
  if (b === 0xdc) {
    if (pos + 2 > len) return -1;
    DECODE_STATE[0] = pos + 2;
    return view.getUint16(pos, false);
  }
  if (b === 0xdd) {
    if (pos + 4 > len) return -1;
    DECODE_STATE[0] = pos + 4;
    return view.getUint32(pos, false);
  }
  return -1;
}

function fastReadMapLen(bytes, view, len, pos) {
  if (pos >= len) return -1;
  const b = bytes[pos++];
  if (b >= 0x80 && b <= 0x8f) {
    DECODE_STATE[0] = pos;
    return b & 0x0f;
  }
  if (b === 0xde) {
    if (pos + 2 > len) return -1;
    DECODE_STATE[0] = pos + 2;
    return view.getUint16(pos, false);
  }
  if (b === 0xdf) {
    if (pos + 4 > len) return -1;
    DECODE_STATE[0] = pos + 4;
    return view.getUint32(pos, false);
  }
  return -1;
}

function fastReadInt64(bytes, view, len, pos) {
  if (pos >= len) return DECODE_FALLBACK;
  const b = bytes[pos++];
  if (b <= 0x7f) {
    DECODE_STATE[0] = pos;
    return BigInt(b);
  }
  if (b >= 0xe0) {
    DECODE_STATE[0] = pos;
    return BigInt(b - 256);
  }
  if (b === 0xcc) {
    if (pos >= len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 1;
    return BigInt(bytes[pos]);
  }
  if (b === 0xcd) {
    if (pos + 2 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 2;
    return BigInt(view.getUint16(pos, false));
  }
  if (b === 0xce) {
    if (pos + 4 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 4;
    return BigInt(view.getUint32(pos, false));
  }
  if (b === 0xcf) {
    if (pos + 8 > len) return DECODE_FALLBACK;
    const v = view.getBigUint64(pos, false);
    if (v > I64_MAX) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 8;
    return v;
  }
  if (b === 0xd0) {
    if (pos >= len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 1;
    return BigInt(view.getInt8(pos));
  }
  if (b === 0xd1) {
    if (pos + 2 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 2;
    return BigInt(view.getInt16(pos, false));
  }
  if (b === 0xd2) {
    if (pos + 4 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 4;
    return BigInt(view.getInt32(pos, false));
  }
  if (b === 0xd3) {
    if (pos + 8 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 8;
    return view.getBigInt64(pos, false);
  }
  return DECODE_FALLBACK;
}

function fastReadFloat(bytes, view, len, pos) {
  if (pos >= len) return DECODE_FALLBACK;
  const b = bytes[pos++];
  if (b === 0xca) {
    if (pos + 4 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 4;
    return view.getFloat32(pos, false);
  }
  if (b === 0xcb) {
    if (pos + 8 > len) return DECODE_FALLBACK;
    DECODE_STATE[0] = pos + 8;
    return view.getFloat64(pos, false);
  }
  return DECODE_FALLBACK;
}

function fastDecodeVal(bytes, view, len, depth) {
  let pos = DECODE_STATE[0];
  if (depth > 64 || pos >= len) return DECODE_FALLBACK;
  const b = bytes[pos];
  if (b <= 0x7f) {
    DECODE_STATE[0] = pos + 1;
    return BigInt(b);
  }
  if (b >= 0xe0) {
    DECODE_STATE[0] = pos + 1;
    return BigInt(b - 256);
  }
  if (b >= 0xa0 && b <= 0xbf) {
    return fastReadStr(bytes, view, len, pos, false);
  }
  if (b === 0xc0) {
    DECODE_STATE[0] = pos + 1;
    return null;
  }
  if (b === 0xc2) {
    DECODE_STATE[0] = pos + 1;
    return false;
  }
  if (b === 0xc3) {
    DECODE_STATE[0] = pos + 1;
    return true;
  }
  if (b === 0xca || b === 0xcb) {
    return fastReadFloat(bytes, view, len, pos);
  }
  if (b >= 0xcc && b <= 0xd3) {
    return fastReadInt64(bytes, view, len, pos);
  }
  if (b === 0xd9 || b === 0xda || b === 0xdb) {
    return fastReadStr(bytes, view, len, pos, false);
  }
  if ((b >= 0x90 && b <= 0x9f) || b === 0xdc || b === 0xdd) {
    const alen = fastReadArrayLen(bytes, view, len, pos);
    pos = DECODE_STATE[0];
    if (alen < 1 || pos >= len) return DECODE_FALLBACK;
    const code = bytes[pos++];
    if (code > 0x7f) return DECODE_FALLBACK;

    if (code === 0x01) {
      if (alen !== 4) return DECODE_FALLBACK;
      const className = fastReadStr(bytes, view, len, pos, false);
      if (
        className === DECODE_FALLBACK ||
        className.length === 0 ||
        ((className.charCodeAt(0) <= 0x20 || className.charCodeAt(0) >= 0x80) &&
          BLANK_OR_CTRL_RE.test(className))
      ) {
        return DECODE_FALLBACK;
      }
      pos = DECODE_STATE[0];
      const moduleUri = fastReadStr(bytes, view, len, pos, false);
      if (
        moduleUri === DECODE_FALLBACK ||
        (moduleUri !== 'pkl:base' && !FAST_MODULE_URI_RE.test(moduleUri))
      ) {
        return DECODE_FALLBACK;
      }
      pos = DECODE_STATE[0];
      const mcount = fastReadArrayLen(bytes, view, len, pos);
      pos = DECODE_STATE[0];
      if (mcount < 0 || mcount * 3 > len - pos) return DECODE_FALLBACK;
      const props = {};
      const entries = new Map();
      const elements = [];
      const nextDepth = depth + 1;
      for (let i = 0; i < mcount; i += 1) {
        if (pos + 2 > len) return DECODE_FALLBACK;
        if (bytes[pos++] !== 0x93) return DECODE_FALLBACK;
        const mcode = bytes[pos++];
        if (mcode === 0x10) {
          const k = fastReadStr(bytes, view, len, pos, true);
          if (k === DECODE_FALLBACK) return DECODE_FALLBACK;
          pos = DECODE_STATE[0];
          if (pos >= len) return DECODE_FALLBACK;
          const vb = bytes[pos];
          let v;
          if (vb === 0xd3) {
            if (pos + 9 > len) return DECODE_FALLBACK;
            v = view.getBigInt64(pos + 1, false);
            pos += 9;
          } else if (vb === 0xcf) {
            if (pos + 9 > len) return DECODE_FALLBACK;
            v = view.getBigUint64(pos + 1, false);
            if (v > I64_MAX) return DECODE_FALLBACK;
            pos += 9;
          } else if (vb === 0xc2) {
            v = false;
            pos += 1;
          } else if (vb === 0xc3) {
            v = true;
            pos += 1;
          } else if (vb === 0xc0) {
            v = null;
            pos += 1;
          } else if (vb <= 0x7f) {
            v = BigInt(vb);
            pos += 1;
          } else if (vb >= 0xe0) {
            v = BigInt(vb - 256);
            pos += 1;
          } else if (vb >= 0xa0 && vb <= 0xbf) {
            v = fastReadStr(bytes, view, len, pos, false);
            if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
            pos = DECODE_STATE[0];
          } else if (
            pos + 4 <= len &&
            vb === 0x92 &&
            bytes[pos + 1] === 0x0f &&
            bytes[pos + 2] === 0xc4
          ) {
            const blen = bytes[pos + 3];
            const p = pos + 4;
            if (p + blen > len) return DECODE_FALLBACK;
            if (blen <= 16) {
              v = new Uint8Array(blen);
              for (let j = 0; j < blen; j += 1) v[j] = bytes[p + j];
            } else {
              v = bytes.slice(p, p + blen);
            }
            pos = p + blen;
          } else {
            DECODE_STATE[0] = pos;
            v = fastDecodeVal(bytes, view, len, nextDepth);
            if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
            pos = DECODE_STATE[0];
          }
          if (k === '__proto__') {
            Object.defineProperty(props, k, {
              value: v,
              enumerable: true,
              writable: true,
              configurable: true,
            });
          } else {
            props[k] = v;
          }
        } else if (mcode === 0x11) {
          DECODE_STATE[0] = pos;
          const k = fastDecodeVal(bytes, view, len, nextDepth);
          if (k === DECODE_FALLBACK) return DECODE_FALLBACK;
          const v = fastDecodeVal(bytes, view, len, nextDepth);
          if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
          pos = DECODE_STATE[0];
          entries.set(k, v);
        } else if (mcode === 0x12) {
          const idx = fastReadInt64(bytes, view, len, pos);
          if (idx === DECODE_FALLBACK) return DECODE_FALLBACK;
          const v = fastDecodeVal(bytes, view, len, nextDepth);
          if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
          pos = DECODE_STATE[0];
          elements.push(v);
        } else {
          return DECODE_FALLBACK;
        }
      }
      DECODE_STATE[0] = pos;
      return new PklObject(className, moduleUri, props, entries, elements);
    }

    if (code === 0x04 || code === 0x05) {
      if (alen !== 2) return DECODE_FALLBACK;
      const count = fastReadArrayLen(bytes, view, len, pos);
      if (count < 0 || count > len - DECODE_STATE[0]) return DECODE_FALLBACK;
      const out = new Array(count);
      const nextDepth = depth + 1;
      for (let i = 0; i < count; i += 1) {
        const item = fastDecodeVal(bytes, view, len, nextDepth);
        if (item === DECODE_FALLBACK) return DECODE_FALLBACK;
        out[i] = item;
      }
      return out;
    }

    if (code === 0x02 || code === 0x03) {
      if (alen !== 2) return DECODE_FALLBACK;
      const count = fastReadMapLen(bytes, view, len, pos);
      if (count < 0 || count * 2 > len - DECODE_STATE[0]) return DECODE_FALLBACK;
      const out = new Map();
      const nextDepth = depth + 1;
      for (let i = 0; i < count; i += 1) {
        const k = fastDecodeVal(bytes, view, len, nextDepth);
        if (k === DECODE_FALLBACK) return DECODE_FALLBACK;
        const v = fastDecodeVal(bytes, view, len, nextDepth);
        if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
        out.set(k, v);
      }
      return out;
    }

    if (code === 0x06) {
      if (alen !== 2) return DECODE_FALLBACK;
      const count = fastReadArrayLen(bytes, view, len, pos);
      if (count < 0 || count > len - DECODE_STATE[0]) return DECODE_FALLBACK;
      const out = new Set();
      const nextDepth = depth + 1;
      for (let i = 0; i < count; i += 1) {
        const item = fastDecodeVal(bytes, view, len, nextDepth);
        if (item === DECODE_FALLBACK) return DECODE_FALLBACK;
        out.add(item);
      }
      return out;
    }

    if (code === 0x0f) {
      if (alen !== 2 || pos >= len) return DECODE_FALLBACK;
      const bb = bytes[pos++];
      let blen;
      if (bb === 0xc4) {
        if (pos >= len) return DECODE_FALLBACK;
        blen = bytes[pos++];
      } else if (bb === 0xc5) {
        if (pos + 2 > len) return DECODE_FALLBACK;
        blen = view.getUint16(pos, false);
        pos += 2;
      } else if (bb === 0xc6) {
        if (pos + 4 > len) return DECODE_FALLBACK;
        blen = view.getUint32(pos, false);
        pos += 4;
      } else {
        return DECODE_FALLBACK;
      }
      if (pos + blen > len) return DECODE_FALLBACK;
      let owned;
      if (blen <= 16) {
        owned = new Uint8Array(blen);
        for (let j = 0; j < blen; j += 1) owned[j] = bytes[pos + j];
      } else {
        owned = bytes.slice(pos, pos + blen);
      }
      DECODE_STATE[0] = pos + blen;
      return owned;
    }

    if (code === 0x07) {
      if (alen !== 3) return DECODE_FALLBACK;
      const val = fastReadFloat(bytes, view, len, pos);
      if (val === DECODE_FALLBACK) return DECODE_FALLBACK;
      const unit = fastReadStr(bytes, view, len, DECODE_STATE[0], false);
      if (unit === DECODE_FALLBACK || !VALID_DURATION_UNITS.has(unit)) {
        return DECODE_FALLBACK;
      }
      return new PklDuration(val, unit);
    }

    if (code === 0x08) {
      if (alen !== 3) return DECODE_FALLBACK;
      const val = fastReadFloat(bytes, view, len, pos);
      if (val === DECODE_FALLBACK) return DECODE_FALLBACK;
      const unit = fastReadStr(bytes, view, len, DECODE_STATE[0], false);
      if (unit === DECODE_FALLBACK || !VALID_DATA_SIZE_UNITS.has(unit)) {
        return DECODE_FALLBACK;
      }
      return new PklDataSize(val, unit);
    }

    if (code === 0x09) {
      if (alen !== 3) return DECODE_FALLBACK;
      DECODE_STATE[0] = pos;
      const nextDepth = depth + 1;
      const first = fastDecodeVal(bytes, view, len, nextDepth);
      if (first === DECODE_FALLBACK) return DECODE_FALLBACK;
      const second = fastDecodeVal(bytes, view, len, nextDepth);
      if (second === DECODE_FALLBACK) return DECODE_FALLBACK;
      return new PklPair(first, second);
    }

    return DECODE_FALLBACK;
  }
  return DECODE_FALLBACK;
}

/**
 * Single-pass pure-JS fast-path decoder for valid lossless `pkl-binary` payloads.
 * Produces identical lossless JS values (`BigInt`, owned non-aliasing `Uint8Array`,
 * `PklObject`, `Map`, `Set`, etc.) and returns `DECODE_FALLBACK` on any edge tag,
 * strict validation requirement, invalid UTF-8/surrogate, or malformed input so the
 * Rust decoder handles exact error paths/messages and all special tags.
 *
 * @param {Uint8Array} bytes
 * @param {boolean} [strict]
 * @returns {any}
 */
export function tryDecodePklBinaryFast(bytes, strict = false) {
  if (strict || !(bytes instanceof Uint8Array) || bytes.byteLength === 0) {
    return DECODE_FALLBACK;
  }
  const len = bytes.byteLength;
  const view = new DataView(bytes.buffer, bytes.byteOffset, len);
  DECODE_STATE[0] = 0;
  INTERN_LEN.fill(-1);
  const out = fastDecodeVal(bytes, view, len, 0);
  if (out === DECODE_FALLBACK || DECODE_STATE[0] !== len) {
    return DECODE_FALLBACK;
  }
  return out;
}

const COLUMNAR_MAGIC = 0x504b4c01;
const MAX_COLUMNAR_DEPTH = 64;
const IS_LITTLE_ENDIAN = new Uint8Array(new Uint32Array([1]).buffer)[0] === 1;
let canUseNewFunction = true;
const SHAPE_PROPS_FACTORIES = new Map();
const COLUMNAR_TABLE_READERS = new Map();
let columnarAlignBuf = null;
const COL_STATE = new Float64Array(7);

function isValidColumnarSection(off, count, elemSize, align, byteLen) {
  return (
    off >= 80 &&
    off <= byteLen &&
    (off & (align - 1)) === 0 &&
    count * elemSize <= byteLen - off
  );
}

function getShapePropsFactory(keys) {
  if (!canUseNewFunction || keys.length === 0 || keys.length > 32) return null;
  for (let i = 0; i < keys.length; i += 1) {
    if (keys[i] === '__proto__') return null;
  }
  const sig = JSON.stringify(keys);
  let fn = SHAPE_PROPS_FACTORIES.get(sig);
  if (fn !== undefined) return fn;
  try {
    const params = new Array(keys.length);
    const fields = new Array(keys.length);
    for (let i = 0; i < keys.length; i += 1) {
      params[i] = `v${i}`;
      fields[i] = `${JSON.stringify(keys[i])}:v${i}`;
    }
    fn = new Function(params.join(','), `return {${fields.join(',')}};`);
  } catch {
    canUseNewFunction = false;
    return null;
  }
  if (SHAPE_PROPS_FACTORIES.size < 256) SHAPE_PROPS_FACTORIES.set(sig, fn);
  return fn;
}

function getColumnarTableReader(keys, colTypes, usePreSliced = false) {
  if (!canUseNewFunction || keys.length === 0 || keys.length > 32) return null;
  for (let i = 0; i < keys.length; i += 1) {
    if (keys[i] === '__proto__') return null;
  }
  const sig = `${JSON.stringify(keys)}|${colTypes.join(',')}|${usePreSliced ? 1 : 0}`;
  let fn = COLUMNAR_TABLE_READERS.get(sig);
  if (fn !== undefined) return fn;
  try {
    const lines = [
      'const out = new Array(count);',
      'let bi = st[0], si = st[1], soff = st[2], bo = st[3], bli = st[4], fli = st[5], bbi = st[6];',
      'for (let i = 0; i < count; i += 1) {',
    ];
    const propExprs = new Array(keys.length);
    for (let c = 0; c < keys.length; c += 1) {
      const t = colTypes[c];
      const k = JSON.stringify(keys[c]);
      if (t === 1) {
        propExprs[c] = `${k}: bigs[bi++]`;
      } else if (t === 2) {
        propExprs[c] = `${k}: bools[bbi++] !== 0`;
      } else if (t === 3) {
        if (usePreSliced) {
          propExprs[c] = `${k}: valStrs[si++]`;
        } else {
          lines.push(
            `  const l${c} = lens[si++]; const s${c} = bigStr.slice(soff, soff + l${c}); soff += l${c};`,
          );
          propExprs[c] = `${k}: s${c}`;
        }
      } else if (t === 4) {
        lines.push(
          `  const bn${c} = binLens[bli++]; let u${c}; if (bn${c} <= 16) { u${c} = new Uint8Array(bn${c}); for (let j = 0; j < bn${c}; j += 1) u${c}[j] = bins[bo + j]; } else { u${c} = bins.slice(bo, bo + bn${c}); } bo += bn${c};`,
        );
        propExprs[c] = `${k}: u${c}`;
      } else if (t === 5) {
        propExprs[c] = `${k}: floats[fli++]`;
      } else if (t === 6) {
        propExprs[c] = `${k}: null`;
      } else {
        return null;
      }
    }
    lines.push(
      `  out[i] = new PklObject(cls, uri, {${propExprs.join(',')}});`,
    );
    lines.push('}');
    lines.push(
      'st[0] = bi; st[1] = si; st[2] = soff; st[3] = bo; st[4] = bli; st[5] = fli; st[6] = bbi;',
    );
    lines.push('return out;');
    fn = new Function(
      'PklObject',
      'cls',
      'uri',
      'count',
      'bigs',
      'bools',
      'floats',
      'lens',
      'bigStr',
      'bins',
      'binLens',
      'st',
      'valStrs',
      lines.join('\n'),
    );
  } catch {
    canUseNewFunction = false;
    return null;
  }
  if (COLUMNAR_TABLE_READERS.size < 256) COLUMNAR_TABLE_READERS.set(sig, fn);
  return fn;
}

const COL_READ = {
  ops: null,
  bigs: null,
  floats: null,
  bools: null,
  strLens: null,
  valStrs: null,
  binLens: null,
  binBytes: null,
  shapes: null,
  bigStr: '',
  op: 0,
  bi: 0,
  fli: 0,
  bbi: 0,
  si: 0,
  soff: 0,
  bli: 0,
  bo: 0,
  opsLen: 0,
  bigsLen: 0,
  floatsLen: 0,
  boolsLen: 0,
  strLensLen: 0,
  bigStrLen: 0,
  binLensLen: 0,
  binBytesLen: 0,
  shapesLen: 0,
};

function readColumnarVal(depth) {
  const c = COL_READ;
  if (depth > MAX_COLUMNAR_DEPTH || c.op >= c.opsLen) return DECODE_FALLBACK;
  const tag = c.ops[c.op++];
  if (tag === 0) return null;
  if (tag === 1) {
    if (c.bi >= c.bigsLen) return DECODE_FALLBACK;
    return c.bigs[c.bi++];
  }
  if (tag === 2) {
    if (c.fli >= c.floatsLen) return DECODE_FALLBACK;
    return c.floats[c.fli++];
  }
  if (tag === 3) return false;
  if (tag === 4) return true;
  if (tag === 5) {
    if (c.si >= c.strLensLen) return DECODE_FALLBACK;
    if (c.valStrs !== null) {
      return c.valStrs[c.si++];
    }
    const l = c.strLens[c.si++];
    if (c.soff + l > c.bigStrLen) return DECODE_FALLBACK;
    const s = c.bigStr.slice(c.soff, c.soff + l);
    c.soff += l;
    return s;
  }
  if (tag === 6) {
    if (c.bli >= c.binLensLen) return DECODE_FALLBACK;
    const n = c.binLens[c.bli++];
    if (c.bo + n > c.binBytesLen) return DECODE_FALLBACK;
    let owned;
    if (n <= 16) {
      owned = new Uint8Array(n);
      for (let j = 0; j < n; j += 1) owned[j] = c.binBytes[c.bo + j];
    } else {
      owned = c.binBytes.slice(c.bo, c.bo + n);
    }
    c.bo += n;
    return owned;
  }
  if (tag === 7) {
    if (c.op >= c.opsLen) return DECODE_FALLBACK;
    const n = c.ops[c.op++];
    if (n < 0 || n > c.opsLen - c.op) return DECODE_FALLBACK;
    const arr = new Array(n);
    const nextDepth = depth + 1;
    for (let i = 0; i < n; i += 1) {
      const item = readColumnarVal(nextDepth);
      if (item === DECODE_FALLBACK) return DECODE_FALLBACK;
      arr[i] = item;
    }
    return arr;
  }
  if (tag === 10) {
    if (c.op >= c.opsLen) return DECODE_FALLBACK;
    const shIdx = c.ops[c.op++];
    if (shIdx < 0 || shIdx >= c.shapesLen) return DECODE_FALLBACK;
    const sh = c.shapes[shIdx];
    const keys = sh.keys;
    const klen = keys.length;
    const nextDepth = depth + 1;
    let props;
    if (sh.fn !== null) {
      if (klen === 4) {
        const a0 = readColumnarVal(nextDepth);
        const a1 = readColumnarVal(nextDepth);
        const a2 = readColumnarVal(nextDepth);
        const a3 = readColumnarVal(nextDepth);
        if (
          a0 === DECODE_FALLBACK ||
          a1 === DECODE_FALLBACK ||
          a2 === DECODE_FALLBACK ||
          a3 === DECODE_FALLBACK
        ) {
          return DECODE_FALLBACK;
        }
        props = sh.fn(a0, a1, a2, a3);
      } else if (klen === 5) {
        const a0 = readColumnarVal(nextDepth);
        const a1 = readColumnarVal(nextDepth);
        const a2 = readColumnarVal(nextDepth);
        const a3 = readColumnarVal(nextDepth);
        const a4 = readColumnarVal(nextDepth);
        if (
          a0 === DECODE_FALLBACK ||
          a1 === DECODE_FALLBACK ||
          a2 === DECODE_FALLBACK ||
          a3 === DECODE_FALLBACK ||
          a4 === DECODE_FALLBACK
        ) {
          return DECODE_FALLBACK;
        }
        props = sh.fn(a0, a1, a2, a3, a4);
      } else {
        const vals = new Array(klen);
        for (let i = 0; i < klen; i += 1) {
          const v = readColumnarVal(nextDepth);
          if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
          vals[i] = v;
        }
        props = sh.fn.apply(null, vals);
      }
    } else {
      props = {};
      for (let i = 0; i < klen; i += 1) {
        const v = readColumnarVal(nextDepth);
        if (v === DECODE_FALLBACK) return DECODE_FALLBACK;
        const k = keys[i];
        if (k === '__proto__') {
          Object.defineProperty(props, k, {
            value: v,
            enumerable: true,
            writable: true,
            configurable: true,
          });
        } else {
          props[k] = v;
        }
      }
    }
    return new PklObject(sh.cls, sh.uri, props);
  }
  if (tag === 12) {
    if (c.op + 2 > c.opsLen) return DECODE_FALLBACK;
    const shIdx = c.ops[c.op++];
    const count = c.ops[c.op++];
    if (shIdx < 0 || shIdx >= c.shapesLen || count < 0) return DECODE_FALLBACK;
    const sh = c.shapes[shIdx];
    const keys = sh.keys;
    const klen = keys.length;
    if (c.op + klen > c.opsLen) return DECODE_FALLBACK;
    const colOpStart = c.op;
    c.op += klen;
    let needBigs = 0;
    let needBools = 0;
    let needStrs = 0;
    let needBins = 0;
    let needFloats = 0;
    for (let col = 0; col < klen; col += 1) {
      const t = c.ops[colOpStart + col];
      if (t === 1) needBigs += count;
      else if (t === 2) needBools += count;
      else if (t === 3) needStrs += count;
      else if (t === 4) needBins += count;
      else if (t === 5) needFloats += count;
      else if (t !== 6) return DECODE_FALLBACK;
    }
    if (
      count > 2_000_000 ||
      c.bi + needBigs > c.bigsLen ||
      c.bbi + needBools > c.boolsLen ||
      c.si + needStrs > c.strLensLen ||
      c.bli + needBins > c.binLensLen ||
      c.fli + needFloats > c.floatsLen
    ) {
      return DECODE_FALLBACK;
    }
    if (needBins > 0) {
      let totalBinBytes = 0;
      for (let i = 0; i < needBins; i += 1) totalBinBytes += c.binLens[c.bli + i];
      if (c.bo + totalBinBytes > c.binBytesLen) return DECODE_FALLBACK;
    }
    const usePreSliced = c.valStrs !== null;
    let reader = usePreSliced ? sh.tableReaderPre : sh.tableReader;
    const readerOpStart = usePreSliced ? sh.tableOpStartPre : sh.tableOpStart;
    if (reader === null || reader === undefined || readerOpStart !== colOpStart) {
      const colTypes = new Array(klen);
      for (let col = 0; col < klen; col += 1) colTypes[col] = c.ops[colOpStart + col];
      reader = getColumnarTableReader(keys, colTypes, usePreSliced);
      if (usePreSliced) {
        sh.tableReaderPre = reader;
        sh.tableOpStartPre = colOpStart;
      } else {
        sh.tableReader = reader;
        sh.tableOpStart = colOpStart;
      }
    }
    if (reader !== null) {
      COL_STATE[0] = c.bi;
      COL_STATE[1] = c.si;
      COL_STATE[2] = c.soff;
      COL_STATE[3] = c.bo;
      COL_STATE[4] = c.bli;
      COL_STATE[5] = c.fli;
      COL_STATE[6] = c.bbi;
      const out = reader(
        PklObject,
        sh.cls,
        sh.uri,
        count,
        c.bigs,
        c.bools,
        c.floats,
        c.strLens,
        c.bigStr,
        c.binBytes,
        c.binLens,
        COL_STATE,
        c.valStrs,
      );
      c.bi = COL_STATE[0];
      c.si = COL_STATE[1];
      c.soff = COL_STATE[2];
      c.bo = COL_STATE[3];
      c.bli = COL_STATE[4];
      c.fli = COL_STATE[5];
      c.bbi = COL_STATE[6];
      if ((!usePreSliced && c.soff > c.bigStrLen) || c.bo > c.binBytesLen) return DECODE_FALLBACK;
      return out;
    }
    const out = new Array(count);
    for (let i = 0; i < count; i += 1) {
      const props = {};
      for (let col = 0; col < klen; col += 1) {
        const t = c.ops[colOpStart + col];
        let v;
        if (t === 1) v = c.bigs[c.bi++];
        else if (t === 2) v = c.bools[c.bbi++] !== 0;
        else if (t === 3) {
          if (usePreSliced) {
            v = c.valStrs[c.si++];
          } else {
            const l = c.strLens[c.si++];
            if (c.soff + l > c.bigStrLen) return DECODE_FALLBACK;
            v = c.bigStr.slice(c.soff, c.soff + l);
            c.soff += l;
          }
        } else if (t === 4) {
          const bn = c.binLens[c.bli++];
          if (c.bo + bn > c.binBytesLen) return DECODE_FALLBACK;
          if (bn <= 16) {
            v = new Uint8Array(bn);
            for (let j = 0; j < bn; j += 1) v[j] = c.binBytes[c.bo + j];
          } else {
            v = c.binBytes.slice(c.bo, c.bo + bn);
          }
          c.bo += bn;
        } else if (t === 5) v = c.floats[c.fli++];
        else if (t === 6) v = null;
        else return DECODE_FALLBACK;
        const k = keys[col];
        if (k === '__proto__') {
          Object.defineProperty(props, k, {
            value: v,
            enumerable: true,
            writable: true,
            configurable: true,
          });
        } else {
          props[k] = v;
        }
      }
      out[i] = new PklObject(sh.cls, sh.uri, props);
    }
    return out;
  }
  if (tag === 13) {
    if (c.fli >= c.floatsLen || c.si >= c.strLensLen) return DECODE_FALLBACK;
    const val = c.floats[c.fli++];
    let unit;
    if (c.valStrs !== null) {
      unit = c.valStrs[c.si++];
    } else {
      const l = c.strLens[c.si++];
      if (c.soff + l > c.bigStrLen) return DECODE_FALLBACK;
      unit = c.bigStr.slice(c.soff, c.soff + l);
      c.soff += l;
    }
    return new PklDuration(val, unit);
  }
  if (tag === 14) {
    if (c.fli >= c.floatsLen || c.si >= c.strLensLen) return DECODE_FALLBACK;
    const val = c.floats[c.fli++];
    let unit;
    if (c.valStrs !== null) {
      unit = c.valStrs[c.si++];
    } else {
      const l = c.strLens[c.si++];
      if (c.soff + l > c.bigStrLen) return DECODE_FALLBACK;
      unit = c.bigStr.slice(c.soff, c.soff + l);
      c.soff += l;
    }
    return new PklDataSize(val, unit);
  }
  if (tag === 15) {
    const nextDepth = depth + 1;
    const first = readColumnarVal(nextDepth);
    if (first === DECODE_FALLBACK) return DECODE_FALLBACK;
    const second = readColumnarVal(nextDepth);
    if (second === DECODE_FALLBACK) return DECODE_FALLBACK;
    return new PklPair(first, second);
  }
  if (tag === 16) {
    if (c.si >= c.strLensLen) return DECODE_FALLBACK;
    let pat;
    if (c.valStrs !== null) {
      pat = c.valStrs[c.si++];
    } else {
      const l = c.strLens[c.si++];
      if (c.soff + l > c.bigStrLen) return DECODE_FALLBACK;
      pat = c.bigStr.slice(c.soff, c.soff + l);
      c.soff += l;
    }
    return new PklRegex(pat);
  }
  if (tag === 17) {
    if (c.bi + 3 > c.bigsLen) return DECODE_FALLBACK;
    const start = c.bigs[c.bi++];
    const end = c.bigs[c.bi++];
    const step = c.bigs[c.bi++];
    return new PklIntSeq(start, end, step);
  }
  if (tag === 18) {
    return new PklFunction();
  }
  return DECODE_FALLBACK;
}

/**
 * Unpacks a Rust-produced compact columnar value buffer directly from an `ArrayBuffer` slice
 * without allocating an intermediate `Uint8Array` wrapper.
 *
 * @param {ArrayBufferLike} rawAb
 * @param {number} rawBase
 * @param {number} rawByteLen
 * @param {any} [viewCache]
 * @returns {any}
 */
export function unpackColumnarSlice(rawAb, rawBase, rawByteLen, viewCache = null) {
  if (
    !IS_LITTLE_ENDIAN ||
    !rawAb ||
    !Number.isInteger(rawBase) ||
    !Number.isInteger(rawByteLen) ||
    rawBase < 0 ||
    rawByteLen < 80 ||
    rawBase + rawByteLen > rawAb.byteLength
  ) {
    return DECODE_FALLBACK;
  }
  let ab = rawAb;
  let base = rawBase;
  const byteLen = rawByteLen;
  let alignedCopyOversized = false;
  let canCacheViews = viewCache !== null;
  if ((rawBase & 7) !== 0) {
    canCacheViews = false;
    if (columnarAlignBuf === null || columnarAlignBuf.byteLength < byteLen) {
      columnarAlignBuf = new Uint8Array(Math.max(byteLen, 4096));
    }
    columnarAlignBuf.set(new Uint8Array(rawAb, rawBase, byteLen), 0);
    ab = columnarAlignBuf.buffer;
    base = 0;
    alignedCopyOversized = columnarAlignBuf.byteLength > 262144;
  }
  try {
    return unpackColumnarAligned(ab, base, byteLen, canCacheViews ? viewCache : null);
  } finally {
    if (alignedCopyOversized) {
      columnarAlignBuf = null;
    }
  }
}

function unpackColumnarAligned(ab, base, byteLen, viewCache) {
  const canCacheViews = viewCache !== null;

  let ops;
  let bigs;
  let floats;
  let bools;
  let strLens;
  let valStrs = null;
  let binLens;
  let binBytes;
  let shapes;
  let shapeStrCount;
  let rootOp;
  let bigStr;
  let soff;

  if (
    canCacheViews &&
    viewCache.ab === ab &&
    viewCache.base === base &&
    viewCache.len === byteLen &&
    viewCache.hdr[0] === COLUMNAR_MAGIC &&
    viewCache.hdr[19] === viewCache.gen &&
    viewCache.gen !== 0
  ) {
    ops = viewCache.ops;
    floats = viewCache.floats;
    bools = viewCache.bools;
    strLens = viewCache.strLens;
    binLens = viewCache.binLens;
    binBytes = viewCache.binBytes;
    shapes = viewCache.shapes;
    shapeStrCount = viewCache.shapeStrCount;
    rootOp = viewCache.rootOp;
    bigStr = viewCache.valBigStr;
    soff = 0;
    if (viewCache.bigVals === null) {
      const rawBigs = viewCache.bigs;
      const bLen = rawBigs.length;
      const cachedBigs = new Array(bLen);
      for (let i = 0; i < bLen; i += 1) cachedBigs[i] = rawBigs[i];
      viewCache.bigVals = cachedBigs;
    }
    bigs = viewCache.bigVals;
    if (viewCache.valStrs === null) {
      const vCount = strLens.length - shapeStrCount;
      const cachedStrs = new Array(vCount);
      let pos = 0;
      const bsLen = bigStr.length;
      let ok = true;
      for (let i = 0; i < vCount; i += 1) {
        const l = strLens[shapeStrCount + i];
        if (pos + l > bsLen) {
          ok = false;
          break;
        }
        cachedStrs[i] = bigStr.slice(pos, pos + l);
        pos += l;
      }
      viewCache.valStrs = ok ? cachedStrs : false;
    }
    if (viewCache.valStrs !== false) {
      valStrs = viewCache.valStrs;
    }
  } else {
    const header = new Uint32Array(ab, base, 20);
    if (header[0] !== COLUMNAR_MAGIC) {
      return DECODE_FALLBACK;
    }
    if (
      !isValidColumnarSection(header[1], header[2], 4, 4, byteLen) ||
      !isValidColumnarSection(header[3], header[4], 8, 8, byteLen) ||
      !isValidColumnarSection(header[5], header[6], 8, 8, byteLen) ||
      !isValidColumnarSection(header[7], header[8], 1, 1, byteLen) ||
      !isValidColumnarSection(header[9], header[10], 4, 4, byteLen) ||
      !isValidColumnarSection(header[11], header[12], 1, 1, byteLen) ||
      !isValidColumnarSection(header[13], header[14], 4, 4, byteLen) ||
      !isValidColumnarSection(header[15], header[16], 1, 1, byteLen)
    ) {
      return DECODE_FALLBACK;
    }
    ops = new Int32Array(ab, base + header[1], header[2]);
    bigs = new BigInt64Array(ab, base + header[3], header[4]);
    floats = new Float64Array(ab, base + header[5], header[6]);
    bools = new Uint8Array(ab, base + header[7], header[8]);
    strLens = new Uint32Array(ab, base + header[9], header[10]);
    const strUtf8 = new Uint8Array(ab, base + header[11], header[12]);
    binLens = new Uint32Array(ab, base + header[13], header[14]);
    binBytes = new Uint8Array(ab, base + header[15], header[16]);
    const shapeCount = header[17];
    shapeStrCount = header[18];

    if (shapeStrCount > strLens.length || shapeCount * 3 > ops.length) {
      return DECODE_FALLBACK;
    }

    if (strUtf8.byteLength === 0) {
      bigStr = '';
    } else {
      try {
        bigStr = FAST_UTF8_DECODER.decode(strUtf8);
      } catch {
        return DECODE_FALLBACK;
      }
    }

    soff = 0;
    const bigStrTotalLen = bigStr.length;
    const shapeStrs = new Array(shapeStrCount);
    for (let i = 0; i < shapeStrCount; i += 1) {
      const l = strLens[i];
      if (soff + l > bigStrTotalLen) return DECODE_FALLBACK;
      shapeStrs[i] = bigStr.slice(soff, soff + l);
      soff += l;
    }

    let opIdx = 0;
    const opsTotalLen = ops.length;
    shapes = new Array(shapeCount);
    for (let s = 0; s < shapeCount; s += 1) {
      if (opIdx + 3 > opsTotalLen) return DECODE_FALLBACK;
      const clsIdx = ops[opIdx++];
      const uriIdx = ops[opIdx++];
      const klen = ops[opIdx++];
      if (
        clsIdx < 0 ||
        clsIdx >= shapeStrCount ||
        uriIdx < 0 ||
        uriIdx >= shapeStrCount ||
        klen < 0 ||
        klen > opsTotalLen - opIdx
      ) {
        return DECODE_FALLBACK;
      }
      const cls = shapeStrs[clsIdx];
      const uri = shapeStrs[uriIdx];
      const keys = new Array(klen);
      for (let k = 0; k < klen; k += 1) {
        const keyIdx = ops[opIdx++];
        if (keyIdx < 0 || keyIdx >= shapeStrCount) return DECODE_FALLBACK;
        keys[k] = shapeStrs[keyIdx];
      }
      shapes[s] = { cls, uri, keys, fn: getShapePropsFactory(keys), tableReader: null, tableSig: '' };
    }
    rootOp = opIdx;

    if (canCacheViews && header[19] !== 0) {
      viewCache.ab = ab;
      viewCache.base = base;
      viewCache.len = byteLen;
      viewCache.gen = header[19];
      viewCache.hdr = header;
      viewCache.ops = ops;
      viewCache.bigs = bigs;
      viewCache.floats = floats;
      viewCache.bools = bools;
      viewCache.strLens = strLens;
      viewCache.binLens = binLens;
      viewCache.binBytes = binBytes;
      viewCache.shapes = shapes;
      viewCache.shapeStrCount = shapeStrCount;
      viewCache.rootOp = rootOp;
      viewCache.valBigStr = soff === 0 ? bigStr : bigStr.slice(soff);
      viewCache.bigVals = null;
      viewCache.valStrs = null;
    }
  }

  const c = COL_READ;
  c.ops = ops;
  c.bigs = bigs;
  c.floats = floats;
  c.bools = bools;
  c.strLens = strLens;
  c.valStrs = valStrs;
  c.binLens = binLens;
  c.binBytes = binBytes;
  c.shapes = shapes;
  c.bigStr = bigStr;
  c.op = rootOp;
  c.bi = 0;
  c.fli = 0;
  c.bbi = 0;
  c.si = valStrs !== null ? 0 : shapeStrCount;
  c.soff = soff;
  c.bli = 0;
  c.bo = 0;
  c.opsLen = ops.length;
  c.bigsLen = bigs.length;
  c.floatsLen = floats.length;
  c.boolsLen = bools.length;
  c.strLensLen = valStrs !== null ? valStrs.length : strLens.length;
  c.bigStrLen = bigStr.length;
  c.binLensLen = binLens.length;
  c.binBytesLen = binBytes.length;
  c.shapesLen = shapes.length;

  let root;
  try {
    root = readColumnarVal(0);
  } finally {
    c.ops = null;
    c.bigs = null;
    c.floats = null;
    c.bools = null;
    c.strLens = null;
    c.valStrs = null;
    c.binLens = null;
    c.binBytes = null;
    c.shapes = null;
    c.bigStr = '';
  }
  if (root === DECODE_FALLBACK || c.op !== c.opsLen || c.soff > c.bigStrLen || c.bo > c.binBytesLen) {
    return DECODE_FALLBACK;
  }
  return root;
}

/**
 * Unpacks a Rust-produced compact columnar value buffer into fresh owned lossless JS/TS
 * `PklValue` instances (`PklObject`, `Array`, `Uint8Array`, `BigInt`, `string`, etc.).
 * Returns `DECODE_FALLBACK` on any invalid header or unexpected opcode.
 *
 * @param {Uint8Array} bytes
 * @param {any} [viewCache]
 * @returns {any}
 */
export function unpackColumnarBuffer(bytes, viewCache = null) {
  if (!(bytes instanceof Uint8Array)) {
    return DECODE_FALLBACK;
  }
  return unpackColumnarSlice(bytes.buffer, bytes.byteOffset, bytes.byteLength, viewCache);
}

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

const SYM_ENTRIES = Symbol('pklEntries');
const SYM_ELEMENTS = Symbol('pklElements');

export class PklObject {
  /**
   * @param {string} className
   * @param {string} moduleUri
   * @param {Record<string, any>} [properties]
   * @param {Map<any, any> | Iterable<[any, any]>} [entries]
   * @param {any[]} [elements]
   */
  constructor(className, moduleUri, properties = {}, entries = undefined, elements = undefined) {
    this.className = className;
    this.moduleUri = moduleUri;
    this.properties = properties;
    if (entries !== undefined) {
      if (entries instanceof Map) {
        this[SYM_ENTRIES] = entries;
      } else if (entries !== null && typeof entries[Symbol.iterator] === 'function') {
        this[SYM_ENTRIES] = new Map(entries);
      } else {
        throw new PklError(
          'PklObject entries must be a Map or iterable of [key, value] pairs',
          'invalid-argument',
        );
      }
    }
    if (elements !== undefined) {
      if (Array.isArray(elements)) {
        this[SYM_ELEMENTS] = elements;
      } else if (elements !== null && typeof elements[Symbol.iterator] === 'function') {
        this[SYM_ELEMENTS] = Array.from(elements);
      } else {
        throw new PklError('PklObject elements must be an Array or iterable', 'invalid-argument');
      }
    }
  }

  get entries() {
    let m = this[SYM_ENTRIES];
    if (m === undefined) {
      m = new Map();
      if (Object.isExtensible(this)) {
        this[SYM_ENTRIES] = m;
      }
    }
    return m;
  }

  set entries(val) {
    if (!(val instanceof Map)) {
      throw new PklError('PklObject.entries must be a Map', 'invalid-argument');
    }
    this[SYM_ENTRIES] = val;
  }

  get elements() {
    let a = this[SYM_ELEMENTS];
    if (a === undefined) {
      a = [];
      if (Object.isExtensible(this)) {
        this[SYM_ELEMENTS] = a;
      }
    }
    return a;
  }

  set elements(val) {
    if (!Array.isArray(val)) {
      throw new PklError('PklObject.elements must be an Array', 'invalid-argument');
    }
    this[SYM_ELEMENTS] = val;
  }

  toJSON() {
    const rawEntries = this[SYM_ENTRIES];
    const rawElements = this[SYM_ELEMENTS];
    return {
      className: this.className,
      moduleUri: this.moduleUri,
      properties: this.properties,
      entries: rawEntries !== undefined ? Array.from(rawEntries.entries()) : [],
      elements: rawElements !== undefined ? rawElements : [],
    };
  }

  static _peekEntries(obj) {
    const m = obj[SYM_ENTRIES];
    return m === undefined ? null : m;
  }

  static _peekElements(obj) {
    const a = obj[SYM_ELEMENTS];
    return a === undefined ? null : a;
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
   * @param {any[]} [path]
   */
  constructor(domain, data, path = []) {
    this.domain = domain;
    this.data = data;
    if (Array.isArray(path)) {
      this.path = path;
    } else if (path !== null && path !== undefined && typeof path[Symbol.iterator] === 'function') {
      this.path = Array.from(path);
    } else {
      throw new PklError('PklReference path must be an Array or iterable', 'invalid-argument');
    }
  }
}

const MAX_HYDRATE_DEPTH = 256;
const MAX_DEHYDRATE_DEPTH = 128;

/**
 * Hydrates a Rust-produced tagged value descriptor into public JS/TS PklValue instances.
 * @param {any} raw
 * @param {number} [depth]
 * @returns {any}
 */
export function hydrateValue(raw, depth = 0) {
  if (depth > MAX_HYDRATE_DEPTH) {
    throw new PklError('Maximum value nesting depth exceeded', 'decode');
  }
  if (raw === null || raw === undefined) return null;
  const t = typeof raw;
  if (t === 'boolean' || t === 'bigint' || t === 'number' || t === 'string') {
    return raw;
  }
  if (raw instanceof Uint8Array) {
    return raw;
  }
  const nextDepth = depth + 1;
  if (Array.isArray(raw)) {
    const out = new Array(raw.length);
    for (let i = 0; i < raw.length; i += 1) {
      out[i] = hydrateValue(raw[i], nextDepth);
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
        return new PklPair(hydrateValue(raw.first, nextDepth), hydrateValue(raw.second, nextDepth));
      case 'regex':
        return new PklRegex(raw.pattern);
      case 'set': {
        const set = new Set();
        for (let i = 0; i < raw.items.length; i += 1) {
          set.add(hydrateValue(raw.items[i], nextDepth));
        }
        return set;
      }
      case 'map': {
        const map = new Map();
        for (let i = 0; i < raw.entries.length; i += 1) {
          const pair = raw.entries[i];
          map.set(hydrateValue(pair[0], nextDepth), hydrateValue(pair[1], nextDepth));
        }
        return map;
      }
      case 'object': {
        const props = {};
        for (let i = 0; i < raw.properties.length; i += 1) {
          const [k, v] = raw.properties[i];
          Object.defineProperty(props, k, {
            value: hydrateValue(v, nextDepth),
            enumerable: true,
            writable: true,
            configurable: true,
          });
        }
        const entries = new Map();
        for (let i = 0; i < raw.entries.length; i += 1) {
          const [k, v] = raw.entries[i];
          entries.set(hydrateValue(k, nextDepth), hydrateValue(v, nextDepth));
        }
        const elements = new Array(raw.elements.length);
        for (let i = 0; i < raw.elements.length; i += 1) {
          elements[i] = hydrateValue(raw.elements[i], nextDepth);
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
          hydrateValue(raw.domain, nextDepth),
          hydrateValue(raw.data, nextDepth),
          raw.path.map((item) => hydrateValue(item, nextDepth)),
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
  const hydrateAnnList = (list) => (list || []).map((item) => hydrateValue(item, 0));
  const hydrateClass = (cls) => {
    if (!cls) return cls;
    cls.annotations = hydrateAnnList(cls.annotations);
    for (const mapKey of ['properties', 'allProperties', 'methods', 'allMethods']) {
      const rec = cls[mapKey];
      if (rec && typeof rec === 'object') {
        for (const key of Object.keys(rec)) {
          rec[key].annotations = hydrateAnnList(rec[key].annotations);
        }
      }
    }
    return cls;
  };
  const hydrateAlias = (alias) => {
    if (!alias) return alias;
    alias.annotations = hydrateAnnList(alias.annotations);
    return alias;
  };
  schema.annotations = hydrateAnnList(schema.annotations);
  schema.moduleClass = hydrateClass(schema.moduleClass);
  for (const mapKey of ['classes', 'allClasses']) {
    const rec = schema[mapKey];
    if (rec && typeof rec === 'object') {
      for (const key of Object.keys(rec)) {
        hydrateClass(rec[key]);
      }
    }
  }
  for (const mapKey of ['typeAliases', 'allTypeAliases']) {
    const rec = schema[mapKey];
    if (rec && typeof rec === 'object') {
      for (const key of Object.keys(rec)) {
        hydrateAlias(rec[key]);
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
 * @param {number} [depth]
 * @param {Set<any> | null} [seen]
 * @returns {any}
 */
export function dehydrateValue(value, depth = 0, seen = null) {
  if (depth > MAX_DEHYDRATE_DEPTH) {
    throw new PklError('Maximum value nesting depth exceeded', 'invalid-argument');
  }
  if (value === null) return null;
  if (value === undefined) {
    throw new PklError('Unsupported value type for Pkl conversion: [object Undefined]', 'invalid-argument');
  }
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
  if (value instanceof PklDuration) {
    return { __pklTag: 'duration', value: value.value, unit: value.unit };
  }
  if (value instanceof PklDataSize) {
    return { __pklTag: 'dataSize', value: value.value, unit: value.unit };
  }
  if (value instanceof PklRegex) {
    return { __pklTag: 'regex', pattern: value.pattern };
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

  const active = seen || new Set();
  if (active.has(value)) {
    throw new PklError('Circular reference detected during Pkl value conversion', 'invalid-argument');
  }
  active.add(value);
  const nextDepth = depth + 1;
  try {
    if (Array.isArray(value)) {
      const out = new Array(value.length);
      for (let i = 0; i < value.length; i += 1) {
        out[i] = dehydrateValue(value[i], nextDepth, active);
      }
      return out;
    }
    if (value instanceof Set) {
      const items = [];
      for (const item of value) {
        items.push(dehydrateValue(item, nextDepth, active));
      }
      return {
        __pklTag: 'set',
        items,
      };
    }
    if (value instanceof Map) {
      const entries = [];
      for (const [k, v] of value.entries()) {
        entries.push([
          dehydrateValue(k, nextDepth, active),
          dehydrateValue(v, nextDepth, active),
        ]);
      }
      return { __pklTag: 'map', entries };
    }
    if (value instanceof PklPair) {
      return {
        __pklTag: 'pair',
        first: dehydrateValue(value.first, nextDepth, active),
        second: dehydrateValue(value.second, nextDepth, active),
      };
    }
    if (value instanceof PklObject) {
      const properties = [];
      for (const [k, v] of Object.entries(value.properties || {})) {
        properties.push([k, dehydrateValue(v, nextDepth, active)]);
      }
      const entries = [];
      const rawEntries = PklObject._peekEntries(value);
      if (rawEntries !== null) {
        const entryIterable = rawEntries instanceof Map ? rawEntries.entries() : rawEntries;
        for (const [k, v] of entryIterable) {
          entries.push([
            dehydrateValue(k, nextDepth, active),
            dehydrateValue(v, nextDepth, active),
          ]);
        }
      }
      const rawElements = PklObject._peekElements(value);
      const elements = rawElements
        ? rawElements.map((item) => dehydrateValue(item, nextDepth, active))
        : [];
      return {
        __pklTag: 'object',
        className: value.className,
        moduleUri: value.moduleUri,
        properties,
        entries,
        elements,
      };
    }
    if (value instanceof PklReference) {
      return {
        __pklTag: 'reference',
        domain: dehydrateValue(value.domain, nextDepth, active),
        data: dehydrateValue(value.data, nextDepth, active),
        path: (value.path || []).map((item) => dehydrateValue(item, nextDepth, active)),
      };
    }
  } finally {
    active.delete(value);
  }
  throw new PklError(
    `Unsupported value type for Pkl conversion: ${Object.prototype.toString.call(value)}`,
    'invalid-argument',
  );
}
