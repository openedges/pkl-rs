import { PklError } from './errors.mjs';

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: false, ignoreBOM: true });

function hexDigitValue(code) {
  if (code >= 0x30 && code <= 0x39) return code - 0x30;
  if (code >= 0x41 && code <= 0x46) return code - 0x41 + 10;
  if (code >= 0x61 && code <= 0x66) return code - 0x61 + 10;
  return -1;
}

/**
 * Byte-level `%XX` decoder matching Rust `percent_decode_path` so malformed
 * trailing escapes (e.g. `%00%ZZ`) never skip decoding earlier `%00` / `%2e%2e`.
 * @param {string} raw
 * @returns {string}
 */
function percentDecodeUtf8(raw) {
  if (!raw.includes('%')) return raw;
  const srcBytes = textEncoder.encode(raw);
  const out = new Uint8Array(srcBytes.length);
  let outLen = 0;
  let i = 0;
  while (i < srcBytes.length) {
    if (srcBytes[i] === 0x25 /* '%' */ && i + 2 < srcBytes.length) {
      const hi = hexDigitValue(srcBytes[i + 1]);
      const lo = hexDigitValue(srcBytes[i + 2]);
      if (hi >= 0 && lo >= 0) {
        out[outLen++] = (hi << 4) | lo;
        i += 3;
        continue;
      }
    }
    out[outLen++] = srcBytes[i++];
  }
  return textDecoder.decode(out.subarray(0, outLen));
}

function isWindowsStyleVfsPath(p) {
  return (
    p.startsWith('\\\\') ||
    p.startsWith('//?/') ||
    /^[A-Za-z]:[\\/]/.test(p) ||
    /^\/[A-Za-z]:[\\/]/.test(p)
  );
}

/**
 * Normalizes a POSIX virtual path to an absolute `/...` form resolving `.` and `..`.
 * @param {string} inputPath
 * @param {string} [baseDir]
 * @param {{ preservePosixBackslash?: boolean }} [opts]
 * @returns {string}
 */
export function normalizeVfsPath(inputPath, baseDir = '/') {
  if (typeof inputPath !== 'string' || inputPath.length === 0) {
    throw new PklError('VFS path must be a non-empty string', 'invalid-argument');
  }
  if (inputPath.includes('\0') || (typeof baseDir === 'string' && baseDir.includes('\0'))) {
    throw new PklError('VFS path must not contain NUL bytes', 'invalid-argument');
  }
  const isWindows =
    isWindowsStyleVfsPath(inputPath) ||
    (typeof baseDir === 'string' && isWindowsStyleVfsPath(baseDir));
  let replaced = isWindows ? inputPath.replace(/\\/g, '/') : inputPath;
  if (replaced.startsWith('//?/')) {
    const afterVerbatim = replaced.slice(4);
    if (/^[A-Za-z]:(?:\/|$)/.test(afterVerbatim)) {
      replaced = afterVerbatim;
    }
  }
  let combined;
  if (/^[A-Za-z]:(?:\/|$)/.test(replaced)) {
    combined = `/${replaced}`;
  } else if (replaced.startsWith('/')) {
    combined = replaced;
  } else {
    const normBase = isWindows
      ? baseDir.replace(/\\/g, '/').replace(/\/+$/, '')
      : baseDir.replace(/\/+$/, '');
    combined = `${normBase}/${replaced}`;
  }
  const driveMatch = /^\/([A-Za-z]:)(?:\/(.*))?$/.exec(combined);
  if (driveMatch) {
    const drive = driveMatch[1].toUpperCase();
    const rest = driveMatch[2] ?? '';
    const parts = [];
    for (const seg of rest.split('/')) {
      if (seg === '' || seg === '.') continue;
      if (seg === '..') {
        parts.pop();
      } else {
        parts.push(seg);
      }
    }
    return parts.length === 0 ? `/${drive}` : `/${drive}/${parts.join('/')}`;
  }
  const parts = [];
  for (const seg of combined.split('/')) {
    if (seg === '' || seg === '.') continue;
    if (seg === '..') {
      parts.pop();
    } else {
      parts.push(seg);
    }
  }
  return parts.length === 0 ? '/' : `/${parts.join('/')}`;
}

/**
 * Returns true if `candidate` is equal to or nested beneath `rootDir` on a path-component boundary.
 * @param {string} candidate
 * @param {string} rootDir
 * @returns {boolean}
 */
export function isUnderRootDir(candidate, rootDir) {
  const c = normalizeVfsPath(candidate);
  const r = normalizeVfsPath(rootDir);
  if (r === '/') return c.startsWith('/');
  return c === r || c.startsWith(`${r}/`);
}

/**
 * Percent-encodes a POSIX path matching Pkl `IoUtils.toUri` path rules (preserving `/` and Unicode).
 * @param {string} path
 * @returns {string}
 */
export function encodeUriPath(path) {
  let out = '';
  for (const ch of path) {
    const cp = ch.codePointAt(0);
    if (cp > 0x7f) {
      out += ch;
      continue;
    }
    if (
      (cp >= 0x41 && cp <= 0x5a) ||
      (cp >= 0x61 && cp <= 0x7a) ||
      (cp >= 0x30 && cp <= 0x39) ||
      ch === '/' ||
      ch === '-' ||
      ch === '.' ||
      ch === '_' ||
      ch === '~' ||
      ch === '!' ||
      ch === '$' ||
      ch === '&' ||
      ch === "'" ||
      ch === '(' ||
      ch === ')' ||
      ch === '*' ||
      ch === '+' ||
      ch === ',' ||
      ch === ';' ||
      ch === '=' ||
      ch === ':' ||
      ch === '@'
    ) {
      out += ch;
    } else {
      out += `%${cp.toString(16).toUpperCase().padStart(2, '0')}`;
    }
  }
  return out;
}

/**
 * Converts a POSIX path to a `file://` URI.
 * @param {string} path
 * @param {string} [baseDir]
 * @returns {string}
 */
export function pathToFileUri(path, baseDir = '/') {
  const normalized = normalizeVfsPath(path, baseDir);
  return `file://${encodeUriPath(normalized)}`;
}

/**
 * Decodes a `file:` URI (`file:///...` or `file:/...`) into a normalized POSIX VFS path.
 * Rejects non-empty authorities (`file://host/...`).
 * @param {string} uri
 * @returns {string}
 */
export function fileUriToPath(uri) {
  if (typeof uri !== 'string') {
    throw new PklError('Expected file: URI string', 'invalid-argument');
  }
  const lower = uri.toLowerCase();
  if (!lower.startsWith('file:')) {
    throw new PklError(`Expected file: URI, got \`${uri}\``, 'invalid-argument');
  }
  let rest = uri.slice(5);
  const hashIdx = rest.indexOf('#');
  if (hashIdx >= 0) rest = rest.slice(0, hashIdx);
  const queryIdx = rest.indexOf('?');
  if (queryIdx >= 0) rest = rest.slice(0, queryIdx);
  if (rest.startsWith('//')) {
    const afterSlashes = rest.slice(2);
    const slashIdx = afterSlashes.indexOf('/');
    const authority = slashIdx >= 0 ? afterSlashes.slice(0, slashIdx) : afterSlashes;
    if (authority.length > 0) {
      throw new PklError(`Refusing file URI with non-empty authority: \`${uri}\``, 'security');
    }
    rest = slashIdx >= 0 ? afterSlashes.slice(slashIdx) : '/';
  }
  const decoded = percentDecodeUtf8(rest);
  if (decoded.includes('\0')) {
    throw new PklError(`File URI must not contain NUL bytes: \`${uri}\``, 'invalid-argument');
  }
  if (!decoded.startsWith('/')) {
    throw new PklError(`File URI must have a leading slash: \`${uri}\``, 'invalid-argument');
  }
  return normalizeVfsPath(decoded);
}

/**
 * Compiles a POSIX glob pattern supporting `*`, `**`, `?`, `[...]`, `{a,b}` into a
 * linear-state-space matcher immune to catastrophic backtracking (ReDoS).
 * @param {string} pattern
 * @param {string} [baseDir]
 * @returns {(path: string) => boolean}
 */
function compileGlob(pattern, baseDir = '/') {
  const norm = normalizeVfsPath(pattern, baseDir);
  /** @type {Array<any>} */
  const tokens = [];
  let i = 0;
  while (i < norm.length) {
    const ch = norm[i];
    if (ch === '*') {
      if (norm[i + 1] === '*') {
        if (norm[i + 2] === '/') {
          // Collapse redundant consecutive `**/` segments.
          const last = tokens[tokens.length - 1];
          if (!(last && last.kind === 'lit' && last.ch === '/' && tokens.length >= 3 && tokens[tokens.length - 3].kind === 'branch')) {
            const baseIdx = tokens.length;
            tokens.push({ kind: 'branch', skipTo: baseIdx + 3 });
            tokens.push({ kind: 'anyTree' });
            tokens.push({ kind: 'lit', ch: '/' });
          }
          i += 3;
        } else {
          if (tokens.length === 0 || tokens[tokens.length - 1].kind !== 'anyTree') {
            tokens.push({ kind: 'anyTree' });
          }
          i += 2;
        }
      } else {
        if (tokens.length === 0 || tokens[tokens.length - 1].kind !== 'star') {
          tokens.push({ kind: 'star' });
        }
        i += 1;
      }
    } else if (ch === '?') {
      tokens.push({ kind: 'qmark' });
      i += 1;
    } else if (ch === '[') {
      const close = norm.indexOf(']', i + 1);
      if (close > i) {
        let raw = norm.slice(i + 1, close);
        const negated = raw.startsWith('!') || raw.startsWith('^');
        if (negated) raw = raw.slice(1);
        const chars = new Set();
        const ranges = [];
        for (let k = 0; k < raw.length; k += 1) {
          if (k + 2 < raw.length && raw[k + 1] === '-') {
            ranges.push([raw.charCodeAt(k), raw.charCodeAt(k + 2)]);
            k += 2;
          } else {
            chars.add(raw.charCodeAt(k));
          }
        }
        tokens.push({ kind: 'class', negated, chars, ranges });
        i = close + 1;
      } else {
        tokens.push({ kind: 'lit', ch: '[' });
        i += 1;
      }
    } else if (ch === '{') {
      const close = norm.indexOf('}', i + 1);
      if (close > i) {
        const alts = norm.slice(i + 1, close).split(',');
        tokens.push({ kind: 'alt', alts });
        i = close + 1;
      } else {
        tokens.push({ kind: 'lit', ch: '{' });
        i += 1;
      }
    } else {
      tokens.push({ kind: 'lit', ch });
      i += 1;
    }
  }

  const tLen = tokens.length;
  return (text) => {
    const sLen = text.length;
    const stride = sLen + 1;
    const totalStates = (tLen + 1) * stride;
    if (totalStates > 4_000_000) {
      throw new PklError('Glob pattern or path exceeds complexity limit', 'invalid-argument');
    }
    const visited = new Uint8Array(totalStates);
    const tStack = [0];
    const sStack = [0];
    while (tStack.length > 0) {
      const ti = tStack.pop();
      const si = sStack.pop();
      if (ti === tLen) {
        if (si === sLen) return true;
        continue;
      }
      const key = ti * stride + si;
      if (visited[key] !== 0) continue;
      visited[key] = 1;

      const tok = tokens[ti];
      switch (tok.kind) {
        case 'lit':
          if (si < sLen && text[si] === tok.ch) {
            tStack.push(ti + 1);
            sStack.push(si + 1);
          }
          break;
        case 'qmark':
          if (si < sLen && text[si] !== '/') {
            tStack.push(ti + 1);
            sStack.push(si + 1);
          }
          break;
        case 'class':
          if (si < sLen) {
            const code = text.charCodeAt(si);
            if (tok.negated && code === 47 /* '/' */) {
              break;
            }
            let matched = tok.chars.has(code);
            if (!matched) {
              for (let r = 0; r < tok.ranges.length; r += 1) {
                const [lo, hi] = tok.ranges[r];
                if (code >= lo && code <= hi) {
                  matched = true;
                  break;
                }
              }
            }
            if (tok.negated ? !matched : matched) {
              tStack.push(ti + 1);
              sStack.push(si + 1);
            }
          }
          break;
        case 'star':
          tStack.push(ti + 1);
          sStack.push(si);
          if (si < sLen && text[si] !== '/') {
            tStack.push(ti);
            sStack.push(si + 1);
          }
          break;
        case 'anyTree':
          tStack.push(ti + 1);
          sStack.push(si);
          if (si < sLen) {
            tStack.push(ti);
            sStack.push(si + 1);
          }
          break;
        case 'branch':
          tStack.push(tok.skipTo);
          sStack.push(si);
          tStack.push(ti + 1);
          sStack.push(si);
          break;
        case 'alt':
          for (let a = 0; a < tok.alts.length; a += 1) {
            const opt = tok.alts[a];
            if (text.startsWith(opt, si)) {
              tStack.push(ti + 1);
              sStack.push(si + opt.length);
            }
          }
          break;
      }
    }
    return false;
  };
}

/**
 * Pure in-memory POSIX VirtualFileSystem with optional `rootDir` confinement,
 * owned `Uint8Array` storage, directory listing, and glob matching.
 */
export class VirtualFileSystem {
  /**
   * @param {Record<string, string | Uint8Array>} [initialFiles]
   * @param {{ rootDir?: string }} [options]
   */
  constructor(initialFiles = undefined, options = undefined) {
    /** @type {string | null} */
    this._rootDir =
      options && typeof options.rootDir === 'string' && options.rootDir.length > 0
        ? normalizeVfsPath(options.rootDir)
        : null;
    /** @type {Map<string, Uint8Array>} */
    this._files = new Map();
    /** @type {Set<string>} */
    this._dirs = new Set(['/']);

    if (this._rootDir) {
      this._ensureParentDirs(this._rootDir);
      this._dirs.add(this._rootDir);
    }

    if (initialFiles && typeof initialFiles === 'object') {
      for (const [path, content] of Object.entries(initialFiles)) {
        this.write(path, content);
      }
    }
  }

  /** @returns {string | null} */
  get rootDir() {
    return this._rootDir;
  }

  /**
   * Resolves a path relative to `rootDir` (or `/` if unset).
   * Note: `rootDir` confinement for Pkl module/resource access is enforced by `pkl-runtime`'s
   * `SecurityPolicy` so callers can stage fixtures outside `rootDir` to test confinement parity.
   * @param {string} path
   * @returns {string}
   */
  _resolve(path) {
    return normalizeVfsPath(path, this._rootDir ?? '/');
  }

  /**
   * @param {string} path
   */
  _ensureParentDirs(path) {
    const ancestors = [];
    let cur = path;
    while (cur !== '/') {
      const idx = cur.lastIndexOf('/');
      cur = idx <= 0 ? '/' : cur.slice(0, idx);
      if (this._files.has(cur)) {
        throw new PklError(`Cannot create directory \`${cur}\`: file already exists`, 'io');
      }
      ancestors.push(cur);
    }
    for (let i = 0; i < ancestors.length; i += 1) {
      this._dirs.add(ancestors[i]);
    }
  }

  /**
   * Writes a file (UTF-8 string or owned copy of Uint8Array), creating parent directories.
   * @param {string} path
   * @param {string | Uint8Array} content
   */
  write(path, content) {
    const norm = this._resolve(path);
    if (norm === '/' || /^\/[A-Za-z]:$/.test(norm)) {
      throw new PklError(`Cannot write file at root directory \`${norm}\``, 'io');
    }
    if (this._dirs.has(norm)) {
      throw new PklError(`Cannot overwrite directory \`${norm}\` with a file`, 'io');
    }
    let bytes;
    if (typeof content === 'string') {
      bytes = textEncoder.encode(content);
    } else if (content instanceof Uint8Array) {
      bytes = new Uint8Array(content);
    } else {
      throw new PklError('VFS file content must be string or Uint8Array', 'invalid-argument');
    }
    this._ensureParentDirs(norm);
    this._files.set(norm, bytes);
  }

  /**
   * Creates a directory and any missing parent directories.
   * @param {string} path
   */
  mkdir(path) {
    const norm = this._resolve(path);
    if (this._files.has(norm)) {
      throw new PklError(`Cannot create directory \`${norm}\`: file already exists`, 'io');
    }
    this._ensureParentDirs(norm);
    this._dirs.add(norm);
  }

  /**
   * Reads file bytes as a fresh owned `Uint8Array`.
   * @param {string} path
   * @returns {Uint8Array}
   */
  read(path) {
    const norm = this._resolve(path);
    if (this._dirs.has(norm)) {
      throw new PklError('Is a directory', 'io');
    }
    const bytes = this._files.get(norm);
    if (!bytes) {
      throw new PklError(`File not found in VFS: \`${norm}\``, 'io');
    }
    return new Uint8Array(bytes);
  }

  /**
   * Reads a file as UTF-8 text.
   * @param {string} path
   * @returns {string}
   */
  readText(path) {
    return textDecoder.decode(this.read(path));
  }

  /**
   * Returns true if a file or directory exists at `path`.
   * @param {string} path
   * @returns {boolean}
   */
  has(path) {
    try {
      const norm = this._resolve(path);
      return this._files.has(norm) || this._dirs.has(norm);
    } catch {
      return false;
    }
  }

  /**
   * @param {string} path
   * @returns {boolean}
   */
  isDirectory(path) {
    try {
      const norm = this._resolve(path);
      return this._dirs.has(norm);
    } catch {
      return false;
    }
  }

  /**
   * @param {string} path
   * @returns {boolean}
   */
  isFile(path) {
    try {
      const norm = this._resolve(path);
      return this._files.has(norm);
    } catch {
      return false;
    }
  }

  /**
   * Deletes a file or directory (recursively removing children under a directory).
   * @param {string} path
   * @returns {boolean}
   */
  delete(path) {
    const norm = this._resolve(path);
    if (norm === '/' || (this._rootDir !== null && norm === this._rootDir)) {
      throw new PklError(`Cannot delete root directory \`${norm}\``, 'io');
    }
    if (this._files.delete(norm)) {
      return true;
    }
    if (this._dirs.delete(norm)) {
      const prefix = `${norm}/`;
      for (const key of Array.from(this._files.keys())) {
        if (key.startsWith(prefix)) this._files.delete(key);
      }
      for (const key of Array.from(this._dirs.values())) {
        if (key.startsWith(prefix)) this._dirs.delete(key);
      }
      return true;
    }
    return false;
  }

  /**
   * Lists immediate child entries in a directory (`{ name, isDirectory }[]`), sorted by name.
   * @param {string} [path]
   * @returns {{ name: string, isDirectory: boolean }[]}
   */
  list(path = '/') {
    const norm = this._resolve(path);
    if (!this._dirs.has(norm)) {
      return [];
    }
    const prefix = norm === '/' ? '/' : `${norm}/`;
    /** @type {Map<string, boolean>} */
    const children = new Map();
    for (const filePath of this._files.keys()) {
      if (filePath.startsWith(prefix)) {
        const rest = filePath.slice(prefix.length);
        if (rest.length > 0 && !rest.includes('/')) {
          children.set(rest, false);
        }
      }
    }
    for (const dirPath of this._dirs.values()) {
      if (dirPath !== norm && dirPath.startsWith(prefix)) {
        const rest = dirPath.slice(prefix.length);
        if (rest.length > 0 && !rest.includes('/')) {
          children.set(rest, true);
        }
      }
    }
    return Array.from(children.entries())
      .sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))
      .map(([name, isDirectory]) => ({ name, isDirectory }));
  }

  /**
   * Returns newline-terminated directory listing text matching Pkl `FileResource` directory reads.
   * @param {string} path
   * @returns {Uint8Array}
   */
  readDirectoryResourceBytes(path) {
    const entries = this.list(path);
    entries.sort((a, b) => {
      const la = a.name.toLowerCase();
      const lb = b.name.toLowerCase();
      if (la !== lb) return la < lb ? -1 : 1;
      return a.name < b.name ? -1 : a.name > b.name ? 1 : 0;
    });
    const text = entries.map((e) => `${e.name}\n`).join('');
    return textEncoder.encode(text);
  }

  /**
   * Matches all files (and directories if matching) against a glob pattern, returning sorted paths.
   * @param {string} pattern
   * @returns {string[]}
   */
  glob(pattern) {
    const matcher = compileGlob(pattern, this._rootDir ?? '/');
    const matches = [];
    for (const filePath of this._files.keys()) {
      if (this._rootDir && !isUnderRootDir(filePath, this._rootDir)) continue;
      if (matcher(filePath)) matches.push(filePath);
    }
    matches.sort();
    return matches;
  }
}
