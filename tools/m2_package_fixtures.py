#!/usr/bin/env python3
"""Bounded original-package fixture qualification and owned HTTPS test lifecycle.

This does not change original execution readiness or establish evaluator compatibility.
Generated ZIP bodies are inspected only at the explicitly supplied fixture root.
"""
from __future__ import annotations

import argparse
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import shutil
import signal
import ssl
import stat
import struct
import subprocess
import time
import zipfile
import zlib

import compat

ROOT = Path(__file__).resolve().parents[1]
PACKAGES = ('badImportsWithinPackage@1.0.0', 'badMetadataJson@1.0.0',
            'badPackageZipUrl@1.0.0', 'birds@0.5.0', 'fruit@1.0.5')
ALL_PACKAGES = (
    'badChecksum@1.0.0', 'badImportsWithinPackage@1.0.0', 'badMetadataJson@1.0.0',
    'badPackageZipUrl@1.0.0', 'birds@0.5.0', 'birds@0.6.0', 'birds@0.7.0',
    'deprecated@1.0.0', 'fruit@1.0.5', 'fruit@1.1.0',
    'packageContainingWildcardCharacters@1.0.0', 'unlisted@1.0.0',
)
EXPECTED_METADATA = {
    'birds@0.5.0': 'b27206b80f4f227752b6f02143887f3ea41e554542cec38f7b572b987566c4de',
    'birds@0.6.0': 'd18c8c248a64adf4ca77631b713e6a77782b62bc8faf02aa44558676de7c8149',
    'birds@0.7.0': 'bf920484106d197c27c558efc75c67db3da33ccb06d7141605dcfc73c2789ec4',
    'deprecated@1.0.0': '7589f1b802d6b5b93c468fb6399d2235d44d83f27409da2b036455ccb6e07ce5',
    'fruit@1.0.5': '8ff80e5ac882650e817f261c8af024bf0db9b07888c9d18c20017457e04ffe06',
    'fruit@1.1.0': '8d982761d182f2185e4180c82190791d9a60c721cb3393bb2e946fab90131e8c',
}
SERVER_SOURCE = ROOT / 'tools/java/PackageFixtureServer.java'
RECIPE = ROOT / 'tools/fixtures/package-fixture.gradle'
QUALIFIER_SOURCE = Path(__file__).absolute()
MAX_FIXTURE_BYTES = 4 * 1024 * 1024


def safe_path(value):
    """Reject prohibited names lexically, before any filesystem operation."""
    path = Path(value)
    if (path.name.lower() == 'stage65-b-native-regressions-evidence.tar.gz.partial'
            or any('stage86' in part.lower() or 'private8' in part.lower() for part in path.parts)
            or '..' in path.parts):
        raise ValueError('prohibited fixture path')
    if path.suffix.lower() in ('.zip', '.gz', '.tar', '.tgz'):
        for part in path.parts:
            match = re.match(r'stage(\d+)(?:-|$)', part, re.I)
            if match and int(match[1]) <= 136:
                raise ValueError('historical archive bodies are closed')
    return path.absolute()


def regular(path):
    path = safe_path(path)
    for parent in reversed(path.parents):
        if parent.is_symlink():
            raise ValueError('fixture path has a symlink ancestor')
    info = path.lstat()
    if not stat.S_ISREG(info.st_mode):
        raise ValueError('fixture input must be a regular file: ' + str(path))
    return path, info


def pin(path):
    path, info = regular(path)
    digest = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            digest.update(chunk)
    return dict(path=str(path), bytes=info.st_size, sha256=digest.hexdigest(),
                mode=oct(stat.S_IMODE(info.st_mode)))


def read_json(path):
    path, _ = regular(path)
    return json.loads(path.read_bytes())


def write_json(path, value):
    path = safe_path(path)
    with path.open('xb') as output:
        output.write(compat.encoded(value))


def fresh_directory(path):
    path = safe_path(path)
    for parent in reversed(path.parents):
        if parent.is_symlink():
            raise ValueError('fixture output has a symlink ancestor')
    path.mkdir(parents=True, exist_ok=False)
    return path


def validate_pin(record):
    observed = pin(record['path'])
    if any(observed[key] != record[key] for key in ('bytes', 'sha256', 'mode')):
        raise ValueError('fixture provenance changed: ' + record['path'])
    return observed


def bounded_files(root):
    """Enumerate only a finite fixture tree; reject symlinks and oversized data."""
    root = safe_path(root)
    for parent in (root, *root.parents):
        if parent.is_symlink():
            raise ValueError('fixture tree has a symlink ancestor')
    if root.is_symlink() or not root.is_dir():
        raise ValueError('fixture root is not a regular directory')
    result = {}
    pending = [root]
    total = 0
    visited = 0
    while pending:
        directory = pending.pop()
        for child in directory.iterdir():
            child = safe_path(child)
            visited += 1
            info = child.lstat()
            if stat.S_ISDIR(info.st_mode):
                pending.append(child)
            elif stat.S_ISREG(info.st_mode):
                total += info.st_size
                result[child.relative_to(root).as_posix()] = pin(child)
            else:
                raise ValueError('non-regular fixture tree entry')
            if total > MAX_FIXTURE_BYTES or visited > 128:
                raise ValueError('fixture tree exceeds finite pilot bounds')
    return result


def inspect_generated(packages_root, preparation_manifest):
    root = safe_path(packages_root)
    manifest_path = safe_path(preparation_manifest)
    preparation = read_json(manifest_path)
    if (preparation.get('package_names') != list(PACKAGES)
            or preparation.get('upstream_revision') != 'e51fb2f91774265666f94980ec630db794a4db2d'
            or preparation.get('input_count') != 19 or len(preparation.get('inputs', [])) != 19):
        raise ValueError('preparation is not the reviewed five-package input set')
    inputs = []
    package_files = {name: {} for name in PACKAGES}
    metadata_sources = {}
    source_base = ROOT / 'external/pkl/pkl-commons-test/src/main/files/packages'
    staged_base = manifest_path.parent / 'inputs/packages'
    expected_inputs = set()
    for entry in preparation['inputs']:
        source, staged = entry['source'], entry['staged']
        source_path = safe_path(source['path'])
        staged_path = safe_path(staged['path'])
        relative = source_path.relative_to(source_base)
        if (relative.parts[0] not in PACKAGES or staged_path != staged_base / relative
                or relative.as_posix() in expected_inputs):
            raise ValueError('preparation input escaped the finite source/staging boundary')
        expected_inputs.add(relative.as_posix())
        inputs.extend((validate_pin(source), validate_pin(staged)))
        if source['sha256'] != staged['sha256']:
            raise ValueError('staged fixture differs from original source')
        name = relative.parts[0]
        if relative == Path(name) / (name + '.json'):
            metadata_sources[name] = source_path.read_bytes()
        elif len(relative.parts) >= 3 and relative.parts[1] == 'package':
            package_files[name][Path(*relative.parts[2:]).as_posix()] = source_path.read_bytes()
        else:
            raise ValueError('unexpected original package input')
    for base in (source_base, staged_base):
        observed = {name + '/' + relative for name in PACKAGES
                    for relative in bounded_files(base / name)}
        if observed != expected_inputs:
            raise ValueError('original/staged input membership changed')
    recipe_pins = [validate_pin(row) for row in preparation['recipe_files']]
    recipe_pins.extend(validate_pin(row['source']) for row in preparation['upstream_recipe_and_notices'])
    expected_names = {f'{name}/{name}{suffix}' for name in PACKAGES
                      for suffix in ('.zip', '.json', '.json.sha256')}
    files = bounded_files(root)
    if set(files) != expected_names:
        raise ValueError('generated fixture membership differs: missing or extra files')
    for name in PACKAGES:
        archive_path = root / name / (name + '.zip')
        zip_hash = files[f'{name}/{name}.zip']['sha256']
        metadata = (root / name / (name + '.json')).read_bytes()
        # Gradle Copy's line filter emits LF on this qualified non-Windows host.
        expected = b''.join(line.replace(b'$computedChecksum', zip_hash.encode(), 1) + b'\n'
                            for line in metadata_sources[name].splitlines())
        actual_hash = compat.digest(metadata)
        if metadata != expected:
            raise ValueError('generated metadata differs from canonical source filtering: ' + name)
        if (root / name / (name + '.json.sha256')).read_bytes() != actual_hash.encode():
            raise ValueError('metadata checksum sidecar differs: ' + name)
        if name in EXPECTED_METADATA and actual_hash != EXPECTED_METADATA[name]:
            raise ValueError('pinned metadata checksum mismatch: ' + name)
        with zipfile.ZipFile(archive_path) as archive:
            entries = archive.infolist()
            names = [entry.filename for entry in entries]
            wanted = package_files[name]
            directories = {parent.as_posix() + '/' for item in wanted
                           for parent in Path(item).parents if parent != Path('.')}
            if len(names) != len(set(names)) or set(names) != set(wanted) | directories:
                raise ValueError('generated ZIP membership differs: ' + name)
            for entry in entries:
                if entry.is_dir():
                    continue
                expected_body = wanted[entry.filename]
                if entry.file_size != len(expected_body) or archive.read(entry) != expected_body:
                    raise ValueError('generated ZIP payload differs: ' + name)
    return dict(schema=1, kind='qualified-package-fixtures', package_names=list(PACKAGES),
                fixture_root=str(root), preparation_manifest=pin(manifest_path),
                qualifier=pin(QUALIFIER_SOURCE), reusable_recipe=pin(RECIPE),
                inputs=inputs, recipes=recipe_pins, files=files,
                expected_metadata_sha256=dict(EXPECTED_METADATA),
                scope='Five generated fixtures independently qualified against pinned originals; '
                      'no evaluator execution, compatibility or manifest-readiness claim.')


def qualify_generated(packages_root, preparation_manifest, output, generation_report=None):
    output = fresh_directory(output)
    try:
        record = inspect_generated(packages_root, preparation_manifest)
        if generation_report is not None:
            record['generation_report'] = pin(generation_report)
        write_json(output / 'manifest.json', record)
        return output / 'manifest.json'
    except Exception as error:
        write_json(output / 'failure.json', dict(status='qualification-failed', error=str(error)))
        raise


def load_verified_manifest(path):
    recorded = read_json(path)
    validate_pin(recorded['preparation_manifest'])
    current = inspect_generated(recorded['fixture_root'], recorded['preparation_manifest']['path'])
    if {key: value for key, value in recorded.items() if key != 'generation_report'} != current:
        raise ValueError('qualified fixture manifest or bytes changed')
    if 'generation_report' in recorded:
        validate_pin(recorded['generation_report'])
    return recorded


def run_process(command, output, name, environment, timeout=60):
    directory = fresh_directory(output / name)
    write_json(directory / 'invocation.json', dict(argv=[str(x) for x in command],
               cwd=str(output), environment=environment, timeout_seconds=timeout))
    try:
        captured = compat.capture([str(x) for x in command], output, environment, timeout,
                                  directory / 'generated')
    except compat.CaptureTimeout as error:
        captured = error.result
    except Exception as error:
        write_json(directory / 'failure.json', dict(error=str(error)))
        raise
    compat.store_capture(directory, captured)
    if captured['timed_out'] or captured['exit_code'] != 0:
        raise RuntimeError(name + ' failed; raw capture retained in ' + str(directory))
    return captured


class FixtureServer:
    """One owned JDK HTTPS fixture process; callers retain original authored URIs."""
    def __init__(self, manifest_path, oracle_metadata, output):
        self.manifest_path = safe_path(manifest_path)
        self.metadata = oracle_metadata
        self.output = safe_path(output)
        self.process = None
        self.streams = []
        self.record = {}

    def __enter__(self):
        if self.manifest_path.is_dir():
            self.fixture = dict(fixture_root=str(self.manifest_path))
        else:
            self.fixture = load_verified_manifest(self.manifest_path)
        self.output = fresh_directory(self.output)
        try:
            self._start()
            return self
        except BaseException as error:
            self.record['startup_error'] = str(error)
            self.close()
            raise

    def _start(self):
        java = safe_path(self.metadata['java'])
        identity = self.metadata['jdk']
        if 'properties' in identity and 'java.home' in identity.get('properties', {}):
            home = safe_path(identity['properties']['java.home'])
            java_bin = home / 'bin/java' if (home / 'bin/java').is_file() else java
        else:
            home = java.parent.parent
            java_bin = java
        for path, expected in ((java_bin, identity.get('executable_sha256')),
                               (home / 'lib/modules', identity.get('runtime_modules_sha256')),
                               (home / 'release', identity.get('release_sha256'))):
            if path.exists() and expected and pin(path)['sha256'] != expected:
                raise ValueError('pinned JDK identity changed')
        environment = dict(self.metadata['environment'])
        for key in ('JAVA_TOOL_OPTIONS', 'JDK_JAVA_OPTIONS', '_JAVA_OPTIONS', 'CLASSPATH'):
            environment.pop(key, None)
        temporary = fresh_directory(self.output / 'tmp')
        environment.update(TMPDIR=str(temporary), TMP=str(temporary), TEMP=str(temporary))
        classes = fresh_directory(self.output / 'classes')
        self.ca_path = self.output / 'localhost.pem'
        key_store = self.output / 'localhost.p12'
        keytool = home / 'bin/keytool'
        fixture_info = pin(self.manifest_path) if self.manifest_path.is_file() else dict(path=str(self.manifest_path))
        self.record = dict(schema=1, fixture_manifest=fixture_info,
                           java=pin(java_bin), javac=pin(home / 'bin/javac'), keytool=pin(keytool),
                           server_source=pin(SERVER_SOURCE), environment=environment,
                           scope='Local HTTPS fixture lifecycle only; no evaluator compatibility claim.')
        run_process([keytool, '-genkeypair', '-keyalg', 'RSA', '-alias', 'integ_tests',
                     '-keystore', key_store, '-storepass', 'password', '-dname', 'CN=localhost'],
                    self.output, 'generate-keys', environment)
        run_process([keytool, '-exportcert', '-alias', 'integ_tests', '-storepass', 'password',
                     '-keystore', key_store, '-rfc', '-file', self.ca_path],
                    self.output, 'export-certificate', environment)
        run_process([home / 'bin/javac', '-encoding', 'UTF-8', '-d', classes, SERVER_SOURCE],
                    self.output, 'compile-server', environment)
        self.record.update(certificate=pin(self.ca_path), keystore=pin(key_store),
                           compiled_classes=bounded_files(classes))
        self.request_log = self.output / 'requests.jsonl'
        readiness = self.output / 'ready.json'
        command = [str(java_bin), '-Xmx128m', '-cp', str(classes), 'PackageFixtureServer',
                   self.fixture['fixture_root'], str(key_store), str(readiness), str(self.request_log)]
        self.record.update(argv=command, cwd=str(self.output))
        write_json(self.output / 'server-invocation.json', self.record)
        self.streams = [(self.output / ('server.' + name + '.bin')).open('xb')
                        for name in ('stdout', 'stderr')]
        self.process = subprocess.Popen(command, cwd=self.output, env=environment,
                                        stdin=subprocess.PIPE, stdout=self.streams[0],
                                        stderr=self.streams[1], start_new_session=True)
        deadline = time.monotonic() + 20
        while not readiness.exists():
            if self.process.poll() is not None:
                raise RuntimeError('fixture server exited before readiness')
            if time.monotonic() >= deadline:
                raise RuntimeError('fixture server readiness timed out')
            time.sleep(0.05)
        ready = read_json(readiness)
        if set(ready) != {'port'} or type(ready['port']) is not int or not 0 < ready['port'] < 65536:
            raise ValueError('invalid fixture server readiness')
        self.port = ready['port']
        self.record['port'] = self.port

    def close(self):
        if self.process is not None:
            try:
                if self.process.stdin:
                    self.process.stdin.close()
                self.process.wait(timeout=5)
            except (BrokenPipeError, subprocess.TimeoutExpired):
                self.record['forced_shutdown'] = True
                try:
                    os.killpg(self.process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                self.process.wait(timeout=5)
            self.record['exit_code'] = self.process.returncode
            self.process = None
        for stream in self.streams:
            stream.close()
        self.streams = []
        for name in ('server.stdout.bin', 'server.stderr.bin', 'requests.jsonl'):
            if (self.output / name).exists():
                self.record[name] = pin(self.output / name)
        try:
            if not self.manifest_path.is_dir():
                load_verified_manifest(self.manifest_path)
            self.record['fixture_postcheck'] = 'pass'
        except Exception as error:
            self.record['fixture_postcheck'] = str(error)
        if not (self.output / 'server-result.json').exists():
            write_json(self.output / 'server-result.json', self.record)

    def __exit__(self, kind, value, traceback):
        self.close()
        if kind is None and (self.record.get('exit_code') != 0
                             or self.record.get('forced_shutdown')
                             or self.record.get('fixture_postcheck') != 'pass'):
            raise RuntimeError('fixture shutdown or postcheck failed; evidence retained')


def _build_reproducible_gradle_zip(source_dir: Path) -> bytes:
    """Build a reproducible ZIP byte-identical to Gradle's Zip task.

    Matches Apple Pkl 0.32.0 `pkl-commons-test.gradle.kts` (`preserveFileTimestamps = false`,
    `reproducibleFileOrder = true`): DOS timestamp 1980-02-01 00:00:00 (`0x0041`/`0x0000`),
    UTF-8 flag (`0x0800`), Unix creator `0x0314`, directory mode `0o040755` + DOS `0x10`
    deflated empty stream (`b'\\x03\\x00'`), file mode `0o100644` DEFLATE level 6, sorted
    lexicographically by POSIX relative path. Zero Gradle or network dependency.
    """
    source_dir = safe_path(source_dir)
    if source_dir.is_symlink() or not source_dir.is_dir():
        raise ValueError(f'package source directory must be a regular directory: {source_dir}')
    entries: list[tuple[str, bool, bytes]] = []
    for current, dirs, files in os.walk(source_dir, followlinks=False):
        cur = Path(current)
        for name in dirs:
            path = cur / name
            if path.is_symlink():
                raise ValueError(f'symlink entry not permitted in package fixture: {path}')
            rel = path.relative_to(source_dir).as_posix() + '/'
            entries.append((rel, True, b''))
        for name in files:
            path = cur / name
            info = path.lstat()
            if not stat.S_ISREG(info.st_mode):
                raise ValueError(f'non-regular entry not permitted in package fixture: {path}')
            rel = path.relative_to(source_dir).as_posix()
            entries.append((rel, False, path.read_bytes()))
    entries.sort(key=lambda item: item[0])

    dos_time = 0x0000
    dos_date = 0x0041  # 1980-02-01 00:00:00 (Gradle CONSTANT_TIME_FOR_ZIP_ENTRIES)
    flags = 0x0800     # UTF-8 filename encoding
    method = 8         # DEFLATE
    out = bytearray()
    central = bytearray()
    for name, is_dir, data in entries:
        name_bytes = name.encode('utf-8')
        offset = len(out)
        if is_dir:
            comp = b'\x03\x00'
            crc = 0
            uncomp_len = 0
            comp_len = 2
            ext_attr = (0o040755 << 16) | 0x10
        else:
            compressor = zlib.compressobj(6, zlib.DEFLATED, -zlib.MAX_WBITS)
            comp = compressor.compress(data) + compressor.flush()
            crc = zlib.crc32(data) & 0xFFFFFFFF
            uncomp_len = len(data)
            comp_len = len(comp)
            ext_attr = 0o100644 << 16
        out += struct.pack(
            '<IHHHHHIIIHH',
            0x04034B50, 20, flags, method, dos_time, dos_date,
            crc, comp_len, uncomp_len, len(name_bytes), 0,
        )
        out += name_bytes
        out += comp
        central += struct.pack(
            '<IHHHHHHIIIHHHHHII',
            0x02014B50, 0x0314, 20, flags, method, dos_time, dos_date,
            crc, comp_len, uncomp_len, len(name_bytes), 0, 0, 0, 0, ext_attr, offset,
        )
        central += name_bytes

    cd_offset = len(out)
    out += central
    out += struct.pack(
        '<IHHHHIIH',
        0x06054B50, 0, 0, len(entries), len(entries), len(central), cd_offset, 0,
    )
    return bytes(out)


def build_upstream_packages(stage_dir, java_home=None):
    """Build all 12 upstream test packages into a staged fixture root without Gradle."""
    del java_home  # Unused; pure-Python reproducible ZIP builder has zero JVM/Gradle dependency.
    stage_dir = safe_path(stage_dir)
    stage_dir.mkdir(parents=True, exist_ok=True)
    src_base = ROOT / 'external/pkl/pkl-commons-test/src/main/files/packages'

    for name in ALL_PACKAGES:
        pkg_src = src_base / name
        zip_bytes = _build_reproducible_gradle_zip(pkg_src / 'package')
        zip_sha = hashlib.sha256(zip_bytes).hexdigest()
        source_json = (pkg_src / f'{name}.json').read_bytes()
        # Match Gradle Copy's LF line filter verbatim (see inspect_generated).
        metadata_bytes = b''.join(
            line.replace(b'$computedChecksum', zip_sha.encode('ascii'), 1) + b'\n'
            for line in source_json.splitlines()
        )
        meta_sha = hashlib.sha256(metadata_bytes).hexdigest()

        target = stage_dir / name
        if target.exists():
            shutil.rmtree(target)
        target.mkdir(parents=True, exist_ok=False)
        (target / f'{name}.zip').write_bytes(zip_bytes)
        (target / f'{name}.json').write_bytes(metadata_bytes)
        (target / f'{name}.json.sha256').write_bytes(meta_sha.encode('ascii'))

    for pkg_name, expected_sha in EXPECTED_METADATA.items():
        actual_sha = hashlib.sha256((stage_dir / pkg_name / f'{pkg_name}.json').read_bytes()).hexdigest()
        if actual_sha != expected_sha:
            raise ValueError(
                f'{pkg_name} metadata checksum mismatch: expected {expected_sha}, got {actual_sha}'
            )
    return stage_dir


def check_server(server):
    """Exercise the real TLS handler, retaining every response including mismatches."""
    context = ssl.create_default_context(cafile=str(server.ca_path))
    checks = []
    for name in PACKAGES:
        checks.extend([('GET', '/' + name, 200, None, f'{name}/{name}.json'),
                       ('GET', f'/{name}/{name}.zip', 200, None, f'{name}/{name}.zip')])
    checks.extend([('GET', '/missing@1.0.0', 404, None, None),
                   ('HEAD', '/birds@0.5.0', 405, None, None),
                   ('GET', '/HTTP301/birds@0.5.0', 301, '/birds@0.5.0', None),
                   ('GET', '/HTTP307/birds@0.5.0', 307, '/birds@0.5.0', None)])
    directory = fresh_directory(server.output / 'route-checks')
    rows = []
    for index, (method, path, status, location, file) in enumerate(checks):
        row = dict(method=method, path=path, expected_status=status, expected_location=location)
        connection = http.client.HTTPSConnection('localhost', server.port, context=context, timeout=10)
        try:
            connection.request(method, path)
            response = connection.getresponse()
            body = response.read(MAX_FIXTURE_BYTES + 1)
            (directory / f'{index}.body.bin').write_bytes(body)
            expected = (Path(server.fixture['fixture_root']) / file).read_bytes() if file else b''
            row.update(status=response.status, headers=response.getheaders(), bytes=len(body),
                       body_sha256=compat.digest(body),
                       passed=response.status == status and response.getheader('Location') == location
                       and body == expected)
        except Exception as error:
            row.update(passed=False, error=str(error))
        finally:
            connection.close()
        rows.append(row)
    report = dict(passed=all(row['passed'] for row in rows), checks=rows)
    write_json(directory / 'report.json', report)
    if not report['passed']:
        raise RuntimeError('fixture route checks failed; all response observations retained')
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--qualify', type=Path, help='explicit newly generated five-package root')
    mode.add_argument('--check-server', type=Path, help='qualified fixture manifest')
    parser.add_argument('--preparation-manifest', type=Path)
    parser.add_argument('--generation-report', type=Path)
    parser.add_argument('--oracle-metadata', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    if args.qualify:
        if not args.preparation_manifest:
            parser.error('--qualify requires --preparation-manifest')
        print(qualify_generated(args.qualify, args.preparation_manifest, args.output, args.generation_report))
    else:
        if not args.oracle_metadata:
            parser.error('--check-server requires --oracle-metadata')
        with FixtureServer(args.check_server, read_json(args.oracle_metadata), args.output) as server:
            check_server(server)
        print(args.output / 'server-result.json')


if __name__ == '__main__':
    main()
