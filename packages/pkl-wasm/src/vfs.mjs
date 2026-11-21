import { PklError } from './errors.mjs';

const textEncoder = new TextEncoder();
const textDecoder = new TextDecoder('utf-8', { fatal: false });

/**
 * Normalizes a POSIX virtual path to an absolute `/...` form resolving `.` and `..`.
 * @param {string} inputPath
 * @param {string} [baseDir]
 * @returns {string}
 */
export function normalizeVfsPath(inputPath, baseDir = '/') {
  if (typeof inputPath !== 'string' || inputPath.length === 0) {
    throw new PklError('VFS path must be a non-empty string', 'invalid-argument');
  }
  const replaced = inputPath.replace(/\\/g, '/');
  const combined = replaced.startsWith('/')
    ? replaced
    : `${baseDir.replace(/\\/g, '/').replace(/\/+$/, '')}/${replaced}`;
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
  let decoded;
  try {
    decoded = decodeURIComponent(rest);
  } catch {
    decoded = rest;
  }
  if (!decoded.startsWith('/')) {
    throw new PklError(`File URI must have a leading slash: \`${uri}\``, 'invalid-argument');
  }
  return normalizeVfsPath(decoded);
}

/**
 * Compiles a POSIX glob pattern supporting `*`, `**`, `?`, `[...]`, `{a,b}` into a RegExp.
 * @param {string} pattern
 * @returns {RegExp}
 */
function compileGlob(pattern) {
  const norm = normalizeVfsPath(pattern);
  let regex = '^';
  let i = 0;
  while (i < norm.length) {
    const ch = norm[i];
    if (ch === '*') {
      if (norm[i + 1] === '*') {
        if (norm[i + 2] === '/') {
          regex += '(?:.*/)?';
          i += 3;
        } else {
          regex += '.*';
          i += 2;
        }
      } else {
        regex += '[^/]*';
        i += 1;
      }
    } else if (ch === '?') {
      regex += '[^/]';
      i += 1;
    } else if (ch === '[') {
      const close = norm.indexOf(']', i + 1);
      if (close > i) {
        let cls = norm.slice(i + 1, close);
        if (cls.startsWith('!')) cls = `^${cls.slice(1)}`;
        regex += `[${cls}]`;
        i = close + 1;
      } else {
        regex += '\\[';
        i += 1;
      }
    } else if (ch === '{') {
      const close = norm.indexOf('}', i + 1);
      if (close > i) {
        const alts = norm
          .slice(i + 1, close)
          .split(',')
          .map((alt) => alt.replace(/[.*+?^${}()|[\]\\]/g, '\\$&'));
        regex += `(?:${alts.join('|')})`;
        i = close + 1;
      } else {
        regex += '\\{';
        i += 1;
      }
    } else if ('\\^$.|+()'.includes(ch)) {
      regex += `\\${ch}`;
      i += 1;
    } else {
      regex += ch;
      i += 1;
    }
  }
  regex += '$';
  return new RegExp(regex);
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
    let cur = path;
    while (cur !== '/') {
      const idx = cur.lastIndexOf('/');
      cur = idx <= 0 ? '/' : cur.slice(0, idx);
      this._dirs.add(cur);
    }
  }

  /**
   * Writes a file (UTF-8 string or owned copy of Uint8Array), creating parent directories.
   * @param {string} path
   * @param {string | Uint8Array} content
   */
  write(path, content) {
    const norm = this._resolve(path);
    if (norm === '/') {
      throw new PklError('Cannot write file at root directory `/`', 'io');
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
    if (norm === '/') {
      throw new PklError('Cannot delete root directory `/`', 'io');
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
    const re = compileGlob(pattern);
    const matches = [];
    for (const filePath of this._files.keys()) {
      if (this._rootDir && !isUnderRootDir(filePath, this._rootDir)) continue;
      if (re.test(filePath)) matches.push(filePath);
    }
    matches.sort();
    return matches;
  }
}
