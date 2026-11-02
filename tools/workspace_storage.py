#!/usr/bin/env python3
"""Opt-in rustc wrapper that transparently compresses linked Mach-O executables.

Set RUSTC_WRAPPER to this executable, PKL_STORAGE_TARGET to an isolated Cargo
target directory, and PKL_STORAGE_RECEIPTS to a separate fresh directory.
PKL_STORAGE_MIN_FREE_MIB optionally sets the copy floor (512..65536 MiB;
default 1024 MiB), and is recorded as a separate execution condition.
The wrapper requires Cargo's rustc JSON artifact notifications and fails closed
when an explicit test or binary link produces none. It does not change which tests Cargo runs.
"""
import hashlib
import json
import os
import shlex
from pathlib import Path
import shutil
import stat
import struct
import subprocess
import sys
import tempfile
import threading
import time
import uuid

FLOOR = 1024**3
DITTO = '/usr/bin/ditto'
CODESIGN = '/usr/bin/codesign'
XATTR = '/usr/bin/xattr'


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def plain_path(path):
    """Reject symlinks in the path, including ancestors, before file access."""
    if not path.is_absolute() or '..' in path.parts or '\n' in str(path):
        raise ValueError('absolute path without traversal or newlines required')
    if any(part == 'stage65-b-native-regressions-evidence.tar.gz.partial'
           or part.lower().startswith('stage86') or 'private8' in part.lower()
           for part in path.parts):
        raise ValueError('excluded path before metadata')
    current = Path(path.anchor)
    for part in path.parts[1:]:
        current /= part
        if stat.S_ISLNK(current.lstat().st_mode):
            raise ValueError('symlink path component: '+str(current))
    return path


def regular(path):
    path = plain_path(path)
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode) or info.st_nlink != 1:
        raise ValueError('artifact must be regular with exactly one link')
    return info


def macho_executable(path):
    with path.open('rb') as stream:
        header = stream.read(16)
    if len(header) < 16:
        return False
    endian = {b'\xcf\xfa\xed\xfe': '<', b'\xfe\xed\xfa\xcf': '>',
              b'\xce\xfa\xed\xfe': '<', b'\xfe\xed\xfa\xce': '>'}.get(header[:4])
    return endian is not None and struct.unpack(endian+'I', header[12:16])[0] == 2


def xattrs(path):
    listed = subprocess.run([XATTR, str(path)], capture_output=True, check=True).stdout
    names = listed.decode('utf-8').splitlines()
    if len(names) != len(set(names)) or any(not name or any(ord(c) < 32 for c in name) for name in names):
        raise ValueError('ambiguous xattr name listing')
    attrs = {}
    for name in sorted(names):
        if name == 'com.apple.decmpfs':
            continue  # APFS compression representation, not user metadata.
        encoded = subprocess.run([XATTR, '-px', name, str(path)], capture_output=True, check=True).stdout
        try:
            value = bytes.fromhex(encoded.decode('ascii'))
        except (UnicodeDecodeError, ValueError) as error:
            raise ValueError('invalid xattr hex for '+name) from error
        attrs[name] = hashlib.sha256(value).hexdigest()
    return attrs


def metadata(path):
    info = regular(path)
    attrs = xattrs(path)
    acl = subprocess.run(['/bin/ls', '-lde', str(path)], capture_output=True, check=True).stdout.splitlines()[1:]
    return {'sha256': digest(path), 'bytes': info.st_size, 'mode': stat.S_IMODE(info.st_mode),
            'uid': info.st_uid, 'gid': info.st_gid, 'inode': info.st_ino,
            'nlink': info.st_nlink, 'blocks': info.st_blocks, 'flags': getattr(info, 'st_flags', 0),
            'xattrs': attrs, 'acl_sha256': hashlib.sha256(b'\n'.join(acl)).hexdigest()}


def codesign(path):
    result = subprocess.run([CODESIGN, '--verify', '--strict', str(path)], capture_output=True)
    if result.returncode:
        raise ValueError('codesign verification failed for '+str(path))
    return {'argv': [CODESIGN, '--verify', '--strict', str(path)], 'exit_code': result.returncode,
            'stdout_sha256': hashlib.sha256(result.stdout).hexdigest(),
            'stderr_sha256': hashlib.sha256(result.stderr).hexdigest()}


def copy_compressed(source, temporary, timeout, target, floor):
    command = [DITTO, '--noclone', '--hfsCompression', str(source), str(temporary)]
    started = time.monotonic()
    # ditto output is retained in the receipt without polluting rustc's streams.
    with tempfile.TemporaryFile() as stdout, tempfile.TemporaryFile() as stderr:
        child = subprocess.Popen(command, stdout=stdout, stderr=stderr)
        try:
            while child.poll() is None:
                if time.monotonic()-started > timeout:
                    raise TimeoutError('compression deadline exceeded')
                if shutil.disk_usage(target).free < floor:
                    raise OSError('compression disk floor crossed')
                time.sleep(.05)
            if child.returncode:
                raise OSError('ditto failed: '+str(child.returncode))
        finally:
            if child.poll() is None:
                child.kill(); child.wait()
        stdout.seek(0); stderr.seek(0)
        return {'argv': command, 'exit_code': child.returncode,
                'stdout_sha256': hashlib.sha256(stdout.read()).hexdigest(),
                'stderr_sha256': hashlib.sha256(stderr.read()).hexdigest(),
                'elapsed_seconds': time.monotonic()-started}


def compress_product(path, target, timeout, floor=FLOOR):
    path = Path(path)
    if not path.is_absolute():
        path = Path.cwd()/path
    if not path.is_relative_to(target):
        raise ValueError('linked artifact outside isolated target directory')
    info = regular(path)
    if not macho_executable(path):
        return {'path': str(path), 'action': 'skip-non-executable',
                'bytes': info.st_size, 'mode': stat.S_IMODE(info.st_mode)}
    before = metadata(path)
    signed_before = codesign(path)
    if shutil.disk_usage(target).free < floor + before['bytes']:
        raise OSError('insufficient disk space for copy above floor')
    name = '.'+path.name+'.pkl-storage-'+uuid.uuid4().hex
    temporary = path.with_name(name)
    fd = os.open(temporary, os.O_CREAT | os.O_EXCL | os.O_WRONLY, 0o600)
    os.close(fd)
    try:
        copied = copy_compressed(path, temporary, timeout, target, floor)
        after = metadata(temporary)
        stable_fields = ('sha256', 'bytes', 'mode', 'uid', 'gid', 'xattrs', 'acl_sha256')
        if any(before[field] != after[field] for field in stable_fields):
            raise ValueError('compression changed content or non-compression metadata')
        # UF_COMPRESSED is the only allowed flag transition. An unchanged
        # uncompressed copy is not evidence of a successful compressed output.
        if (before['flags'] & ~0x20) != (after['flags'] & ~0x20) or not after['flags'] & 0x20:
            raise ValueError('unexpected file flags or compression not applied')
        signed_after = codesign(temporary)
        # The artifact cannot change while the temporary copy is prepared.
        current = metadata(path)
        if current != before:
            raise ValueError('original artifact changed during compression')
        if shutil.disk_usage(target).free < floor:
            raise OSError('compression disk floor crossed before replacement')
        os.replace(temporary, path)
        return {'path': str(path), 'action': 'compressed', 'before': before, 'after': after,
                'codesign_before': signed_before, 'codesign_after': signed_after, 'copy': copied}
    finally:
        if temporary.exists():
            temporary.unlink()


def artifact_lines(data):
    result = []
    for line in data.splitlines():
        try:
            row = json.loads(line)
        except (UnicodeDecodeError, ValueError):
            continue
        if isinstance(row, dict) and row.get('$message_type') == 'artifact' and row.get('emit') == 'link':
            if not isinstance(row.get('artifact'), str):
                raise ValueError('malformed rustc link artifact notification')
            result.append(row['artifact'])
    return list(dict.fromkeys(result))


def needs_executable_link(args):
    if any(arg == '--print' or arg.startswith('--print=') or arg in ('-vV', '-V', '--version') for arg in args):
        return False
    types = []
    for index, arg in enumerate(args):
        if arg.startswith('--crate-type='):
            types.extend(arg[len('--crate-type='):].split(','))
        elif arg == '--crate-type' and index+1 < len(args):
            types.extend(args[index+1].split(','))
    if '--test' not in args and 'bin' not in types:
        return False
    emits = []
    for index, arg in enumerate(args):
        if arg.startswith('--emit='):
            emits.extend(arg[7:].split(','))
        elif arg == '--emit' and index+1 < len(args):
            emits.extend(args[index+1].split(','))
    return not emits or any(item.split('=', 1)[0] == 'link' for item in emits)


def storage_floor(environment):
    value = environment.get('PKL_STORAGE_MIN_FREE_MIB', '1024')
    if not value.isascii() or not value.isdigit():
        raise ValueError('PKL_STORAGE_MIN_FREE_MIB must be an integer from 512 to 65536')
    mib = int(value)
    if not 512 <= mib <= 65536:
        raise ValueError('PKL_STORAGE_MIN_FREE_MIB must be an integer from 512 to 65536')
    return mib * 1024**2


def jobserver_fds(environment):
    """Validate Cargo's POSIX jobserver authorization without changing it.

    Numeric descriptors are passed only after checking that both pipe ends are
    open. FIFO authorization is path-based and requires no inherited descriptor.
    Conflicting or malformed authorizations fail before starting the compiler.
    """
    flags = environment.get('CARGO_MAKEFLAGS', '')
    try:
        words = shlex.split(flags)
    except ValueError as error:
        raise ValueError('invalid CARGO_MAKEFLAGS quoting') from error
    authorizations = []
    for word in words:
        for option in ('--jobserver-auth', '--jobserver-fds'):
            if word == option or word.startswith(option+'='):
                if word == option:
                    raise ValueError('jobserver authorization requires =value')
                value = word[len(option)+1:]
                if option == '--jobserver-auth' and value.startswith('fifo:'):
                    if not value[5:]:
                        raise ValueError('empty jobserver FIFO path')
                    authorizations.append(('fifo', value[5:]))
                else:
                    parts = value.split(',')
                    if (len(parts) != 2 or any(not part.isascii() or not part.isdigit()
                                               for part in parts)):
                        raise ValueError('invalid numeric jobserver authorization')
                    read_fd, write_fd = map(int, parts)
                    if read_fd < 3 or write_fd < 3 or read_fd == write_fd:
                        raise ValueError('invalid jobserver descriptor pair')
                    authorizations.append(('pipe', (read_fd, write_fd)))
                break
    if not authorizations:
        return {'kind': 'absent', 'fds': []}, ()
    if any(item != authorizations[0] for item in authorizations[1:]):
        raise ValueError('conflicting jobserver authorizations')
    kind, value = authorizations[0]
    if kind == 'fifo':
        return {'kind': 'fifo', 'fds': []}, ()
    for fd in value:
        try:
            mode = os.fstat(fd).st_mode
        except (OSError, OverflowError) as error:
            raise ValueError('closed or invalid jobserver descriptor '+str(fd)) from error
        if not stat.S_ISFIFO(mode):
            raise ValueError('jobserver descriptor is not a pipe: '+str(fd))
    return {'kind': 'pipe', 'fds': list(value),
            'inheritable': [os.get_inheritable(fd) for fd in value]}, value


def run(argv, environment):
    if len(argv) < 2:
        raise ValueError('expected rustc executable and arguments')
    target = plain_path(Path(environment['PKL_STORAGE_TARGET']))
    receipts = plain_path(Path(environment['PKL_STORAGE_RECEIPTS']))
    if not target.is_dir() or not receipts.is_dir() or receipts.is_relative_to(target) or target.is_relative_to(receipts):
        raise ValueError('target and receipt directories must be separate')
    timeout = int(environment.get('PKL_STORAGE_COPY_SECONDS', '120'))
    if not 1 <= timeout <= 1800:
        raise ValueError('copy deadline must be 1..1800 seconds')
    floor = storage_floor(environment)
    identity = str(uuid.uuid4())
    stdout_path = receipts/(identity+'.stdout.bin')
    stderr_path = receipts/(identity+'.stderr.bin')
    receipt_path = receipts/(identity+'.json')
    report = {'argv': argv[1:], 'cwd': str(Path.cwd()), 'target': str(target),
              'started_epoch': time.time(), 'artifacts': [], 'errors': [], 'complete': False,
              'performance_claim': False, 'minimum_free_bytes': floor}
    code = 1
    compiler_exit_code = None
    try:
        rustc_path = shutil.which(argv[1])
        if rustc_path is None:
            raise ValueError('rustc executable not found')
        report['tools'] = {name: {'path': str(Path(path).resolve()), 'sha256': digest(Path(path).resolve())}
                           for name, path in [('rustc', rustc_path), ('ditto', DITTO), ('codesign', CODESIGN)]}
        report['jobserver'], fds = jobserver_fds(environment)
        with stdout_path.open('xb') as stdout_file, stderr_path.open('xb') as stderr_file:
            child = subprocess.Popen(argv[1:], stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                     env=environment, pass_fds=fds)
            failures = []
            def pump(source, sink, destination):
                try:
                    while chunk := source.read1(65536):
                        sink.write(chunk); sink.flush()
                        destination.write(chunk); destination.flush()
                except BaseException as error:
                    failures.append(repr(error))
                    # An undrained child pipe can otherwise deadlock wait().
                    if child.poll() is None:
                        child.kill()
            threads = [threading.Thread(target=pump, args=(child.stdout, stdout_file, sys.stdout.buffer)),
                       threading.Thread(target=pump, args=(child.stderr, stderr_file, sys.stderr.buffer))]
            for thread in threads: thread.start()
            code = child.wait()
            compiler_exit_code = code
            for thread in threads: thread.join()
            if failures: raise RuntimeError('stream forwarding failed: '+repr(failures))
        if code == 0:
            links = artifact_lines(stderr_path.read_bytes())
            if needs_executable_link(argv[2:]) and not links:
                raise ValueError('successful executable link lacked rustc JSON artifact notification')
            for link in links:
                report['artifacts'].append(compress_product(link, target, timeout, floor))
            if needs_executable_link(argv[2:]) and not any(item['action'] == 'compressed' for item in report['artifacts']):
                raise ValueError('successful executable link lacked a Mach-O executable artifact')
        report['complete'] = True
    except BaseException as error:
        report['errors'].append(repr(error))
        if code == 0: code = 1
    finally:
        report['compiler_exit_code'] = compiler_exit_code
        for name, path in [('stdout', stdout_path), ('stderr', stderr_path)]:
            try:
                if path.is_file():
                    report[name] = {'path': str(path), 'bytes': path.stat().st_size, 'sha256': digest(path)}
            except OSError as error:
                report['errors'].append('stream receipt '+name+': '+repr(error))
                report['complete'] = False
                if code == 0: code = 1
        report['finished_epoch'] = time.time()
        with receipt_path.open('x') as output:
            json.dump(report, output, indent=2); output.write('\n')
    return code if code >= 0 else 128-code


def main():
    try:
        return run(sys.argv, os.environ)
    except (KeyError, OSError, ValueError) as error:
        print('workspace_storage: '+str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    raise SystemExit(main())
