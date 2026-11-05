"""Read only a newly admitted Stage153-155 archive; extract nothing.

Usage: python3 readback.py ARCHIVE ARCHIVE_SHA256 ARCHIVE_BYTES ROSTER ROSTER_SHA256 FRESH_OUTPUT_DIR
Run only after the root pack process is terminal/reaped and the final Stage153-155 roster is immutable.
"""
import gzip
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import tarfile
import time

ROOT = Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage57')

ROLE_NAMES = {'package_uri74_boundary_raw_and_independent_readback', 'same_workspace_raw266_channels_and_reader', 'focused_native_source_regressions', 'm1_clone_capture_raw_channels_and_reader', 'terminal_workspace_guard_inline_source_bookends', 'workspace_aggregate_independent_readback', 'file_uri48_boundary_raw_and_independent_readback', 'default_prechecks_terminal_and_readback', 'final_docs_and_source_audit', 't40_original2_raw_and_separate_cache_controls'}

def validate_required_roles(roster):
    roles = roster['required_future_inputs']
    assert isinstance(roles, list) and len(roles) == len(ROLE_NAMES)
    assert all(isinstance(role, dict) and role.get('required') is True for role in roles)
    assert {role['role'] for role in roles} == ROLE_NAMES
    rows = roster['files']
    by_path = {row['source']['path']: row['source'] for row in rows}
    assert len(by_path) == len(rows)
    for role in roles:
        actual = role['actual_pin']
        assert isinstance(actual, dict) and set(actual) == {'path', 'bytes', 'sha256', 'mode'}
        assert isinstance(actual['path'], str) and actual['path'].endswith('.json')
        assert type(actual['bytes']) is int and actual['bytes'] >= 0
        assert type(actual['mode']) is int and 0 <= actual['mode'] <= 0o7777
        assert isinstance(actual['sha256'], str) and re.fullmatch('[0-9a-f]{64}', actual['sha256'])
        assert by_path.get(actual['path']) == actual



def lexical(value, member=False):
    assert isinstance(value, str) and value and "\\" not in value and "\0" not in value
    parts = value.split('/')
    if not member:
        assert value.startswith('/')
        parts = parts[1:]
    assert all(p not in ('', '.', '..') for p in parts)
    assert all('stage86' not in p.lower() and 'private8' not in p.lower()
               and p != 'stage65-b-native-regressions-evidence.tar.gz.partial' for p in parts)
    pure = PurePosixPath(value)
    assert value == pure.as_posix() and pure.is_absolute() != member
    if member:
        return pure
    path = Path(value)
    assert path.is_relative_to(ROOT)
    return path


def regular(path):
    path = lexical(str(path))
    for parent in reversed(path.parents):
        assert not stat.S_ISLNK(parent.lstat().st_mode), str(parent)
    info = path.lstat()
    assert stat.S_ISREG(info.st_mode), str(path)
    return info


def digest(path):
    with path.open('rb') as stream:
        return hashlib.file_digest(stream, 'sha256').hexdigest()


def identity(path):
    info = regular(path)
    return {'path': str(path), 'bytes': info.st_size, 'sha256': digest(path),
            'mode': stat.S_IMODE(info.st_mode), 'device': info.st_dev,
            'inode': info.st_ino, 'mtime_ns': info.st_mtime_ns,
            'ctime_ns': info.st_ctime_ns}


assert len(sys.argv) == 7, __doc__
archive = lexical(sys.argv[1])
expected_archive_sha = sys.argv[2]
expected_archive_bytes = int(sys.argv[3])
roster_path = lexical(sys.argv[4])
expected_roster_sha = sys.argv[5]
out = lexical(sys.argv[6])
assert re.fullmatch('[0-9a-f]{64}', expected_archive_sha)
assert re.fullmatch('[0-9a-f]{64}', expected_roster_sha)
assert archive.name == 'evidence.tar.gz' and archive.parent.parent == ROOT / 'target/m2'
assert archive.parent == ROOT / 'target/m2/stage153-155-publication-01'
assert out == ROOT / 'target/m2/stage153-155-publication-readback-01'
assert out != archive.parent
out.mkdir()
report = {'complete': False, 'errors': [], 'started_epoch': time.time(),
          'scope': 'One new Stage153-155 archive and its exact final finite roster. No extraction, older archive bodies, source payload replays, or cache walks.',
          'archive_expected': {'path': str(archive), 'bytes': expected_archive_bytes, 'sha256': expected_archive_sha},
          'roster_expected': {'path': str(roster_path), 'sha256': expected_roster_sha}}
try:
    roster_before = identity(roster_path)
    assert roster_before['sha256'] == expected_roster_sha
    roster = json.loads(roster_path.read_bytes())
    assert roster['complete'] and not roster.get('draft', False) and not roster.get('mutable_scratch_pending_m1', False)
    validate_required_roles(roster)
    rows = roster['files']
    assert len(rows) == roster['file_count'] and rows
    expected = {}
    sources = set()
    for row in rows:
        name = row['member']
        lexical(name, member=True)
        source = row['source']
        source_path = lexical(source['path'])
        assert source_path == ROOT / name
        assert not source_path.name.endswith(('.tar.gz', '.tgz', '.zip', '.tar', '.tar.zst', '.partial'))
        assert name not in expected and source['path'] not in sources
        assert isinstance(source['bytes'], int) and source['bytes'] >= 0
        assert isinstance(source['mode'], int) and 0 <= source['mode'] <= 0o7777
        assert re.fullmatch('[0-9a-f]{64}', source['sha256'])
        expected[name] = source
        sources.add(source['path'])
    logical_bytes = sum(row['source']['bytes'] for row in rows)
    assert logical_bytes == roster['logical_payload_bytes']
    archive_before = identity(archive)
    assert archive_before['sha256'] == expected_archive_sha
    assert archive_before['bytes'] == expected_archive_bytes
    seen = set()
    verified_bytes = 0
    with archive.open('rb') as raw, gzip.GzipFile(fileobj=raw, mode='rb') as gz:
        with tarfile.open(fileobj=gz, mode='r:') as tf:
            for index, member in enumerate(tf):
                name = member.name
                lexical(name, member=True)
                assert index < len(rows) and name == rows[index]['member'], name
                assert name in expected and name not in seen, name
                source = expected[name]
                assert member.type in (tarfile.REGTYPE, tarfile.AREGTYPE)
                assert member.isfile() and not member.linkname and not member.sparse
                assert member.size == source['bytes'] and member.mode == source['mode'], name
                # Effective PAX/GNU names have already passed the same canonical
                # check. Metadata extension records are not payload members.
                stream = tf.extractfile(member)
                assert stream is not None
                size = 0
                sha = hashlib.sha256()
                with stream:
                    while True:
                        block = stream.read(1024 * 1024)
                        if not block:
                            break
                        size += len(block)
                        sha.update(block)
                assert size == source['bytes'] and sha.hexdigest() == source['sha256'], name
                verified_bytes += size
                seen.add(name)
            assert seen == set(expected) and verified_bytes == logical_bytes
            # TarFile stops at the first zero header. Require its second zero
            # block plus all remaining record padding, consume gzip to validate
            # CRC/ISIZE, and reject appended nonzero tar data without extraction.
            assert gz.tell() == tf.offset + tarfile.BLOCKSIZE
            trailing = tarfile.BLOCKSIZE
            while True:
                block = gz.read(1024 * 1024)
                if not block:
                    break
                assert not block.strip(b'\0'), 'Nonzero data after tar end marker'
                trailing += len(block)
            # Valid zero end padding includes 9216 bytes; it is not fixed at1024.
            assert trailing >= 2 * tarfile.BLOCKSIZE
            assert gz.tell() % tarfile.RECORDSIZE == 0
            report['tar_logical_bytes'] = gz.tell()
            report['zero_end_padding_bytes'] = trailing
    archive_after = identity(archive)
    roster_after = identity(roster_path)
    assert archive_after == archive_before
    assert roster_after == roster_before
    report.update(complete=True, archive=archive_after, roster=roster_after,
                  member_count=len(seen), logical_payload_bytes=verified_bytes,
                  exact_member_order_and_set=True, duplicate_members=False,
                  all_members_regular=True, modes_lengths_and_stream_sha256_match=True,
                  canonical_forbidden_lexical_checks=True,
                  gzip_crc_isize_consumed=True, raw_archive_checksum_verified_twice=True,
                  archive_and_roster_bookends_stable=True,
                  claim='Finite raw evidence integrity only; not hermetic restoration, complete dependencies, compatibility completion, or performance qualification.')
except Exception as error:
    import traceback
    report['errors'].append(repr(error))
    report['traceback'] = traceback.format_exc()
finally:
    report['reader'] = identity(Path(__file__).absolute())
    report['finished_epoch'] = time.time()
    (out / 'report.json').write_text(json.dumps(report, indent=2) + '\n')
print(json.dumps({'complete': report['complete'], 'errors': report['errors'],
                  'report': identity(out / 'report.json')}))
raise SystemExit(0 if report['complete'] else 1)
