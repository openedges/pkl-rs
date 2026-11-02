#!/usr/bin/env python3
"""Stream and verify one Stage136 gzip tar; never extract archive members."""
import hashlib
import json
import os
import stat
import sys
import zlib
from pathlib import Path, PurePosixPath

MAX_COMPRESSED = 85 * 1024 * 1024
MAX_RAW = 768 * 1024 * 1024
MAX_INVENTORY = 32 * 1024 * 1024
BLOCK = 512


def reject_name(value):
    lower = str(value).lower()
    if ('stage65-b-native-regressions-evidence.tar.gz.partial' in lower
            or 'stage86' in lower or 'private8' in lower):
        raise ValueError('prohibited path')


def safe_name(name):
    reject_name(name)
    if not name or '\x00' in name or name.startswith('/') or '\\' in name:
        raise ValueError('unsafe member name')
    if '..' in name.split('/'):
        raise ValueError('unsafe member traversal')
    return name


class GzipStream:
    def __init__(self, path):
        self.file = os.fdopen(os.dup(path), 'rb') if isinstance(path, int) else open(path, 'rb')
        self.decoder = zlib.decompressobj(31)
        self.pending = b''
        self.buffer = bytearray()
        self.compressed = 0
        self.raw = 0
        self.sha = hashlib.sha256()
        self.done = False

    def fill(self, need):
        while len(self.buffer) < need and not self.done:
            if not self.pending:
                chunk = self.file.read(65536)
                if not chunk:
                    raise ValueError('truncated gzip stream')
                self.compressed += len(chunk)
                if self.compressed > MAX_COMPRESSED:
                    raise ValueError('compressed limit exceeded')
                self.sha.update(chunk)
                self.pending = chunk
            data = self.decoder.decompress(self.pending, min(65536, MAX_RAW - self.raw + 1))
            self.pending = self.decoder.unconsumed_tail
            self.raw += len(data)
            if self.raw > MAX_RAW:
                raise ValueError('raw limit exceeded')
            self.buffer.extend(data)
            if self.decoder.eof:
                if self.decoder.unused_data or self.pending or self.file.read(1):
                    raise ValueError('trailing bytes or concatenated gzip stream')
                self.done = True

    def take(self, count):
        self.fill(count)
        if len(self.buffer) < count:
            raise ValueError('truncated tar stream')
        result = bytes(self.buffer[:count])
        del self.buffer[:count]
        return result

    def finish_zeros(self):
        while True:
            if any(self.buffer):
                raise ValueError('nonzero trailing tar bytes')
            self.buffer.clear()
            if self.done:
                break
            self.fill(1)
        self.file.close()


def number(raw):
    if raw and raw[0] & 0x80:
        raise ValueError('base-256 tar numbers not supported')
    text = raw.rstrip(b'\x00 ').lstrip(b' ')
    if not text:
        return 0
    if any(c not in b'01234567' for c in text):
        raise ValueError('invalid octal tar field')
    return int(text, 8)


def cstring(raw):
    return raw.split(b'\0', 1)[0].decode('utf-8', 'strict')


def header(raw):
    expected = number(raw[148:156])
    actual = sum(raw[:148]) + 8 * 32 + sum(raw[156:])
    if expected != actual:
        raise ValueError('tar header checksum mismatch')
    if raw[257:263] not in (b'ustar\0', b'ustar '):
        raise ValueError('unsupported tar format')
    name = cstring(raw[:100])
    prefix = cstring(raw[345:500])
    if prefix:
        name = prefix + '/' + name
    safe_name(name)
    return name, number(raw[124:136]), number(raw[100:108]), raw[156:157]


def pax_parse(raw):
    result = {}
    offset = 0
    while offset < len(raw):
        space = raw.find(b' ', offset)
        if space < 0 or not raw[offset:space].isdigit():
            raise ValueError('invalid PAX length')
        length = int(raw[offset:space])
        end = offset + length
        if length <= space - offset + 2 or end > len(raw) or raw[end - 1:end] != b'\n':
            raise ValueError('invalid PAX record boundary')
        body = raw[space + 1:end - 1]
        if b'=' not in body:
            raise ValueError('invalid PAX record')
        key, value = body.split(b'=', 1)
        key = key.decode('ascii', 'strict')
        if key not in ('path', 'size') or key in result:
            raise ValueError('unsupported or duplicate PAX key')
        result[key] = value.decode('utf-8', 'strict')
        if key == 'path':
            safe_name(result[key])
        offset = end
    return result


def payload(stream, size, collect=False):
    if size > MAX_RAW:
        raise ValueError('member too large')
    if collect and size > MAX_INVENTORY:
        raise ValueError('metadata too large')
    hasher = hashlib.sha256()
    content = bytearray() if collect else None
    remaining = size
    while remaining:
        chunk = stream.take(min(remaining, 65536))
        hasher.update(chunk)
        if collect:
            content.extend(chunk)
        remaining -= len(chunk)
    padding = (-size) % BLOCK
    if padding and any(stream.take(padding)):
        raise ValueError('nonzero member padding')
    return hasher.hexdigest(), bytes(content) if collect else None


def inventory(data):
    def pairs(items):
        obj = {}
        for key, value in items:
            if key in obj:
                raise ValueError('duplicate JSON key')
            obj[key] = value
        return obj
    result = json.loads(data.decode('utf-8', 'strict'), object_pairs_hook=pairs)
    if not isinstance(result, dict) or set(result) != {'schema', 'files'} or result['schema'] != 1 or not isinstance(result['files'], list):
        raise ValueError('invalid inventory schema')
    seen = {'STAGE136_INVENTORY.json'}
    for item in result['files']:
        if not isinstance(item, dict) or set(item) != {'path', 'bytes', 'sha256', 'mode'}:
            raise ValueError('invalid inventory member')
        name = item['path']
        if not isinstance(name, str):
            raise ValueError('invalid inventory path')
        safe_name(name)
        if PurePosixPath(name).as_posix() != name or name in seen:
            raise ValueError('noncanonical or duplicate inventory path')
        seen.add(name)
        if type(item['bytes']) is not int or not 0 <= item['bytes'] <= MAX_RAW:
            raise ValueError('invalid inventory size')
        if type(item['mode']) is not int or not 0 <= item['mode'] <= 0o7777:
            raise ValueError('invalid inventory mode')
        digest = item['sha256']
        if not isinstance(digest, str) or len(digest) != 64 or any(c not in '0123456789abcdef' for c in digest):
            raise ValueError('invalid inventory digest')
    return result['files']


def verify(path):
    reject_name(path)
    st = os.fstat(path) if isinstance(path, int) else os.stat(path, follow_symlinks=False)
    if not stat.S_ISREG(st.st_mode) or st.st_size > MAX_COMPRESSED:
        raise ValueError('archive is not a bounded regular file')
    stream = GzipStream(path)
    files = None
    count = source_bytes = 0
    pending_pax = None
    while True:
        raw = stream.take(BLOCK)
        if raw == bytes(BLOCK):
            if stream.take(BLOCK) != bytes(BLOCK):
                raise ValueError('missing second tar end block')
            if pending_pax is not None:
                raise ValueError('orphan PAX header')
            break
        name, size, mode, kind = header(raw)
        if kind == b'x':
            if pending_pax is not None:
                raise ValueError('consecutive PAX headers')
            _, body = payload(stream, size, True)
            pending_pax = pax_parse(body)
            continue
        if kind not in (b'0', b'\0'):
            raise ValueError('nonregular tar member')
        if pending_pax is not None:
            name = pending_pax.get('path', name)
            if 'size' in pending_pax:
                value = pending_pax['size']
                if not value.isascii() or not value.isdigit():
                    raise ValueError('invalid PAX size')
                size = int(value)
            pending_pax = None
        safe_name(name)
        if count == 0:
            if name != 'STAGE136_INVENTORY.json':
                raise ValueError('inventory must be first logical member')
            _, body = payload(stream, size, True)
            files = inventory(body)
        else:
            if count > len(files):
                raise ValueError('unexpected extra member')
            expected = files[count - 1]
            if name != expected['path'] or size != expected['bytes'] or mode != expected['mode']:
                raise ValueError('member name, size, order, or mode mismatch')
            value, _ = payload(stream, size)
            if value != expected['sha256']:
                raise ValueError('member digest mismatch')
            source_bytes += size
        count += 1
    if files is None or count != len(files) + 1:
        raise ValueError('missing inventory members')
    stream.finish_zeros()
    if stream.raw % BLOCK:
        raise ValueError('tar length is not block aligned')
    after = os.fstat(path) if isinstance(path, int) else os.stat(path, follow_symlinks=False)
    if (st.st_dev, st.st_ino, st.st_size, st.st_mtime_ns) != (after.st_dev, after.st_ino, after.st_size, after.st_mtime_ns):
        raise ValueError('archive changed during readback')
    return {'archive_sha256': stream.sha.hexdigest(), 'compressed_bytes': stream.compressed,
            'tar_bytes': stream.raw, 'logical_members': count, 'source_members': len(files),
            'source_bytes': source_bytes}


def main():
    if len(sys.argv) != 3:
        raise SystemExit('usage: readback.py ARCHIVE FRESH_OUTPUT_DIRECTORY')
    reject_name(sys.argv[1]); reject_name(sys.argv[2])
    archive = Path(sys.argv[1]).absolute()
    out = Path(sys.argv[2]).absolute()
    out.mkdir(parents=True, exist_ok=False)
    script = Path(__file__).read_bytes()
    result = {'complete': False, 'archive_integrity_valid': False,
              'script_sha256': hashlib.sha256(script).hexdigest(),
              'scope': 'Archive structure and manifest integrity only; no compatibility or source-closure claim.'}
    try:
        result.update(verify(archive))
        result.update(complete=True, archive_integrity_valid=True)
    except Exception as error:
        result['error'] = repr(error)
    (out / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))
    return 0 if result['archive_integrity_valid'] else 1


if __name__ == '__main__':
    sys.exit(main())
