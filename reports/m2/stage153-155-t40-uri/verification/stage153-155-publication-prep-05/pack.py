"""Package only the reviewed finite Stage153-155 evidence roster."""
import gzip
import hashlib
import json
import os
import re
from pathlib import Path
import shutil
import stat
import sys
import tarfile

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

roster_path = Path(sys.argv[1])
expected_roster_sha = sys.argv[2]
OUT = ROOT / 'target/m2/stage153-155-publication-01'
FLOOR = 1275068416

def lexical(path):
    p = Path(path)
    assert isinstance(path, (str, Path)) and '\x00' not in str(path) and '\\' not in str(path)
    assert '..' not in p.parts
    assert all('stage86' not in x.lower() and 'private8' not in x.lower()
               and x != 'stage65-b-native-regressions-evidence.tar.gz.partial' for x in p.parts)
    return p

def sha(p):
    with lexical(p).open('rb') as f:
        return hashlib.file_digest(f, 'sha256').hexdigest()

def validate(row):
    spelling = row['member']
    assert spelling and '\\' not in spelling
    assert all(part not in ('', '.', '..') for part in spelling.split('/'))
    member = lexical(row['member'])
    assert spelling == member.as_posix()
    p = lexical(row['source']['path'])
    assert not member.is_absolute() and p == ROOT / member
    assert not p.name.endswith(('.tar.gz', '.tgz', '.zip', '.tar', '.tar.zst', '.partial'))
    for q in [p, *p.parents]:
        assert not stat.S_ISLNK(q.lstat().st_mode), str(q)
    s = p.stat()
    assert stat.S_ISREG(s.st_mode)
    assert s.st_size == row['source']['bytes']
    assert stat.S_IMODE(s.st_mode) == row['source']['mode']
    return p, s

assert roster_path.is_absolute() and roster_path.is_relative_to(ROOT)
for component in [roster_path, *roster_path.parents]:
    assert not stat.S_ISLNK(lexical(component).lstat().st_mode)
assert stat.S_ISREG(roster_path.lstat().st_mode)
assert sha(roster_path) == expected_roster_sha
roster = json.loads(roster_path.read_bytes())
rows = roster['files']
assert roster['complete'] and not roster.get('draft', False) and not roster.get('mutable_scratch_pending_m1', False) and len(rows) == roster['file_count']
validate_required_roles(roster)
assert len({x['member'] for x in rows}) == len(rows)
assert len({x['source']['path'] for x in rows}) == len(rows)
assert shutil.disk_usage(ROOT).free >= FLOOR + 128 * 1024**2
OUT.mkdir()
archive = OUT / 'evidence.tar.gz'
result = {'complete': False, 'errors': [], 'roster_sha256': expected_roster_sha,
          'roster_path': str(roster_path), 'file_count': len(rows),
          'logical_payload_bytes': sum(x['source']['bytes'] for x in rows),
          'scope': 'Finite raw evidence payload; excludes compiled/cache bodies. Not a hermetic restoration or complete dependency closure.'}
try:
    with archive.open('xb') as raw, gzip.GzipFile(filename='', mode='wb', fileobj=raw, mtime=0) as gz, tarfile.open(fileobj=gz, mode='w|', format=tarfile.PAX_FORMAT) as tf:
        for index, row in enumerate(rows):
            p, before = validate(row)
            assert sha(p) == row['source']['sha256'], str(p)
            info = tf.gettarinfo(str(p), arcname=row['member'])
            assert info.isfile()
            with p.open('rb') as f:
                tf.addfile(info, f)
            after = p.stat()
            assert (before.st_ino, before.st_size, before.st_mtime_ns, before.st_ctime_ns) == (after.st_ino, after.st_size, after.st_mtime_ns, after.st_ctime_ns)
            if index % 32 == 0:
                assert shutil.disk_usage(ROOT).free >= FLOOR
    result.update(complete=True, archive={'path': str(archive), 'bytes': archive.stat().st_size, 'sha256': sha(archive)})
except Exception as e:
    result['errors'].append(repr(e))
(OUT / 'report.json').write_text(json.dumps(result, indent=2) + '\n')
print(json.dumps(result))
raise SystemExit(0 if result['complete'] else 1)
