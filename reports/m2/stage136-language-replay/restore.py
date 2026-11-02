#!/usr/bin/env python3
"""Restore validated Stage136 content-addressed parts to a fresh external directory."""
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import tarfile

PREP = Path(__file__).resolve().parent
ROOT = PREP.parents[2]
CHUNK_LIMIT = 8 * 1024 * 1024
INDEX_LIMIT = 512 * 1024 * 1024
STAGING = '.__stage136_restore_blobs__'
spec = importlib.util.spec_from_file_location('stage136_reader', PREP / 'restore_reader.py')
reader = importlib.util.module_from_spec(spec)
spec.loader.exec_module(reader)


def digest_ok(value):
    return isinstance(value, str) and len(value) == 64 and all(c in '0123456789abcdef' for c in value)


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError('duplicate JSON key')
        result[key] = value
    return result


def name_parts(name):
    reader.safe_name(name)
    p = PurePosixPath(name)
    if p.as_posix() != name or any(x in ('', '.', '..') for x in name.split('/')):
        raise ValueError('noncanonical path')
    if p.parts[0] == STAGING:
        raise ValueError('reserved staging path')
    return p.parts


def inspect(parts, expected_index):
    """Verify all parts and index/chunk identities before creating the output."""
    slices = {}
    blobs = {}
    part_pins = []
    for part_fd in parts:
        os.lseek(part_fd, 0, os.SEEK_SET)
        checked = reader.verify(part_fd)
        part_pins.append(checked)
        os.lseek(part_fd, 0, os.SEEK_SET)
        with os.fdopen(os.dup(part_fd), 'rb') as source, tarfile.open(fileobj=source, mode='r|gz') as tf:
            for member in tf:
                name = member.name
                if name == 'STAGE136_INVENTORY.json':
                    continue
                if name.startswith('index/'):
                    suffix = name[6:]
                    if len(suffix) != 6 or not suffix.isascii() or not suffix.isdigit() or suffix in slices or member.size > CHUNK_LIMIT:
                        raise ValueError('invalid or duplicate index slice')
                    slices[suffix] = tf.extractfile(member).read()
                    if sum(map(len, slices.values())) > INDEX_LIMIT:
                        raise ValueError('index size limit')
                elif name.startswith('blobs/'):
                    key = name[6:]
                    if not digest_ok(key) or key in blobs or member.size > CHUNK_LIMIT:
                        raise ValueError('invalid or duplicate blob')
                    h = hashlib.sha256()
                    total = 0
                    stream = tf.extractfile(member)
                    while True:
                        block = stream.read(1024 * 1024)
                        if not block:
                            break
                        h.update(block)
                        total += len(block)
                    if h.hexdigest() != key or total != member.size:
                        raise ValueError('blob digest or size differs')
                    blobs[key] = total
                else:
                    raise ValueError('unexpected archive member')
    if not slices or sorted(slices) != [f'{i:06d}' for i in range(len(slices))]:
        raise ValueError('incomplete or unordered index slices')
    raw = b''.join(slices[f'{i:06d}'] for i in range(len(slices)))
    if hashlib.sha256(raw).hexdigest() != expected_index:
        raise ValueError('index SHA-256 differs from supplied identity')
    index = json.loads(raw, object_pairs_hook=unique_object)
    if not isinstance(index, dict) or index.get('schema') != 1 or index.get('chunk_bytes') != CHUNK_LIMIT or not isinstance(index.get('files'), list):
        raise ValueError('invalid restoration index')
    files = index['files']
    seen = set()
    used = set()
    for row in files:
        if not isinstance(row, dict) or set(row) != {'path', 'bytes', 'sha256', 'mode', 'chunks'}:
            raise ValueError('invalid file record')
        names = name_parts(row['path'])
        if row['path'] in seen:
            raise ValueError('duplicate restoration path')
        seen.add(row['path'])
        if type(row['bytes']) is not int or row['bytes'] < 0 or type(row['mode']) is not int or not 0 <= row['mode'] <= 0o777 or not digest_ok(row['sha256']) or not isinstance(row['chunks'], list):
            raise ValueError('invalid size, mode, hash, or chunks')
        total = 0
        for chunk in row['chunks']:
            if not isinstance(chunk, dict) or set(chunk) != {'sha256', 'bytes'} or not digest_ok(chunk['sha256']) or type(chunk['bytes']) is not int or not 0 < chunk['bytes'] <= CHUNK_LIMIT or blobs.get(chunk['sha256']) != chunk['bytes']:
                raise ValueError('missing or invalid chunk')
            total += chunk['bytes']
            used.add(chunk['sha256'])
        if total != row['bytes'] or (not row['bytes'] and row['chunks']):
            raise ValueError('file chunk lengths differ')
    for name in seen:
        components = name.split('/')
        if any('/'.join(components[:i]) in seen for i in range(1, len(components))):
            raise ValueError('file overlaps a parent directory')
    if used != set(blobs):
        raise ValueError('unused or missing blob')
    return files, blobs, part_pins


def open_dir(parent, name):
    try:
        os.mkdir(name, 0o700, dir_fd=parent)
    except FileExistsError:
        pass
    return os.open(name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent)


def open_absolute(path, directory=False):
    path = path.absolute()
    if '..' in path.parts or not path.is_absolute():
        raise ValueError('unsafe absolute path')
    current = os.open('/', os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for i, component in enumerate(path.parts[1:]):
            flags = os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK
            if i < len(path.parts[1:]) - 1 or directory:
                flags |= os.O_DIRECTORY
            nxt = os.open(component, flags, dir_fd=current)
            os.close(current)
            current = nxt
        result = os.fstat(current)
        if directory and not stat.S_ISDIR(result.st_mode):
            raise ValueError('expected directory')
        if not directory and not stat.S_ISREG(result.st_mode):
            raise ValueError('part is not regular')
        return current
    except BaseException:
        os.close(current)
        raise


def repository_roots(*paths):
    """Find Git worktree roots containing the utility, cwd or destination."""
    roots = set()
    for path in paths:
        path = path.absolute()
        for candidate in (path, *path.parents):
            try:
                os.lstat(candidate / '.git')
            except (FileNotFoundError, NotADirectoryError):
                continue
            roots.add(candidate)
    return roots


def restore(output, parts, expected_index, expected_parts=None):
    reader.reject_name(str(output))
    for part in parts:
        reader.reject_name(str(part))
    if not digest_ok(expected_index):
        raise ValueError('expected index SHA-256 is required')
    if not parts or len(set(map(str, parts))) != len(parts):
        raise ValueError('supply distinct archive parts')
    if expected_parts is not None and (len(expected_parts) != len(parts) or any(not digest_ok(x) for x in expected_parts)):
        raise ValueError('expected part SHA-256 roster differs')
    if not hasattr(os, 'O_NOFOLLOW') or not hasattr(os, 'O_DIRECTORY') or not hasattr(os, 'O_NONBLOCK'):
        raise ValueError('safe directory-relative opens unavailable')
    if '..' in output.parts:
        raise ValueError('parent traversal is not allowed')
    output = output.absolute()
    if output.name in ('', '.', '..'):
        raise ValueError('destination must be fresh')
    parent_fd = open_absolute(output.parent, directory=True)
    if any(output == repo or output.is_relative_to(repo) for repo in repository_roots(PREP, Path.cwd(), output.parent)):
        os.close(parent_fd)
        raise ValueError('destination must be outside the repository')
    part_fds = []
    try:
        for part in parts:
            part_fds.append(open_absolute(part))
        identities = [(os.fstat(fd).st_dev, os.fstat(fd).st_ino) for fd in part_fds]
        if len(set(identities)) != len(identities):
            raise ValueError('duplicate archive part identity')
        files, blobs, part_pins = inspect(part_fds, expected_index)
        if expected_parts is not None and [x['archive_sha256'] for x in part_pins] != expected_parts:
            raise ValueError('archive part SHA-256 differs from supplied identities')
        try:
            os.mkdir(output.name, 0o700, dir_fd=parent_fd)
        except FileExistsError as error:
            raise ValueError('destination must be fresh') from error
        root_fd = os.open(output.name, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=parent_fd)
    except BaseException:
        for fd in part_fds:
            os.close(fd)
        raise
    finally:
        os.close(parent_fd)
    try:
        os.mkdir(STAGING, 0o700, dir_fd=root_fd)
        staging_fd = os.open(STAGING, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=root_fd)
        try:
            # A second scan stages each checked blob exactly once; it rehashes bytes.
            staged = set()
            for part_fd in part_fds:
                os.lseek(part_fd, 0, os.SEEK_SET)
                with os.fdopen(os.dup(part_fd), 'rb') as source, tarfile.open(fileobj=source, mode='r|gz') as tf:
                    for member in tf:
                        if not member.name.startswith('blobs/'):
                            continue
                        if not member.isfile():
                            raise ValueError('nonregular blob member')
                        key = member.name[6:]
                        if key not in blobs or key in staged:
                            raise ValueError('blob namespace changed')
                        fd = os.open(key, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=staging_fd)
                        h = hashlib.sha256()
                        total = 0
                        with os.fdopen(fd, 'wb') as dst:
                            stream = tf.extractfile(member)
                            while True:
                                block = stream.read(1024 * 1024)
                                if not block:
                                    break
                                dst.write(block)
                                h.update(block)
                                total += len(block)
                        if h.hexdigest() != key or total != blobs[key]:
                            raise ValueError('blob changed during restore')
                        staged.add(key)
            if staged != set(blobs):
                raise ValueError('blob roster changed')
            for row in files:
                names = name_parts(row['path'])
                current = os.dup(root_fd)
                try:
                    for component in names[:-1]:
                        nxt = open_dir(current, component)
                        os.close(current)
                        current = nxt
                    fd = os.open(names[-1], os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW, 0o600, dir_fd=current)
                    h = hashlib.sha256()
                    total = 0
                    with os.fdopen(fd, 'wb') as dst:
                        for chunk in row['chunks']:
                            src_fd = os.open(chunk['sha256'], os.O_RDONLY | os.O_NOFOLLOW, dir_fd=staging_fd)
                            with os.fdopen(src_fd, 'rb') as src:
                                while True:
                                    block = src.read(1024 * 1024)
                                    if not block:
                                        break
                                    dst.write(block)
                                    h.update(block)
                                    total += len(block)
                        if total != row['bytes'] or h.hexdigest() != row['sha256']:
                            raise ValueError('restored file identity differs: ' + row['path'])
                        os.fchmod(dst.fileno(), row['mode'])
                finally:
                    os.close(current)
            # Recheck the same open file descriptors after restoration; a
            # concurrent archive change must not inherit the first-pass pin.
            for part_fd, original in zip(part_fds, part_pins):
                os.lseek(part_fd, 0, os.SEEK_SET)
                current = reader.verify(part_fd)
                if (current['archive_sha256'], current['compressed_bytes']) != (original['archive_sha256'], original['compressed_bytes']):
                    raise ValueError('archive changed during restore')
            # Remove only the digest-named files this process created.
            for key in staged:
                os.unlink(key, dir_fd=staging_fd)
        finally:
            os.close(staging_fd)
        os.rmdir(STAGING, dir_fd=root_fd)
    finally:
        os.close(root_fd)
        for fd in part_fds:
            os.close(fd)
    return {'files': len(files), 'blobs': len(blobs), 'part_sha256': [x['archive_sha256'] for x in part_pins], 'index_sha256': expected_index, 'destination': str(output)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--index-sha256', required=True, help='Expected complete index SHA-256 from the checkpoint descriptor')
    parser.add_argument('--part-sha256', action='append', help='Expected archive SHA-256; repeat once per part, in the supplied part order')
    parser.add_argument('output', type=Path, help='Fresh directory outside the repository; parent must exist')
    parser.add_argument('parts', type=Path, nargs='+', help='All Stage136 archive parts')
    args = parser.parse_args()
    print(json.dumps(restore(args.output, args.parts, args.index_sha256, args.part_sha256), indent=2))


if __name__ == '__main__':
    main()
