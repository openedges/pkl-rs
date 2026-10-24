"""Record and verify raw native DataSize string sessions against pinned Java."""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time

import data_size_fixtures
import m2_source_inputs

ROOT = Path(__file__).resolve().parents[1]
COMMON = ('expression.bin', 'expression.code-units.bin', 'result.status.bin',
          'result.stdout.bin', 'result.stdout.code-units.bin',
          'result.stderr.bin', 'result.stderr.code-units.bin')
JAVA_ONLY = ('result.error-class.bin', 'result.error-class.code-units.bin')
EVENT_METADATA = {'candidate.' + name + '.bin' for name in
                  ('disposition', 'phase', 'reason', 'callback-origin')}
API_METADATA = {'candidate.api-error-kind.bin'}
MANDATORY = {'tools/data_size_compare.py', 'tools/data_size_oracle.py',
             'tools/data_size_fixtures.py', 'tools/java/DataSizeSplitOracle.java',
             'tools/tests/test_data_size_compare.py', 'tools/tests/test_data_size_fixtures.py',
             'tools/m2_source_inputs.py', 'crates/pkl-runtime/examples/data_size_fixture_probe.rs'}
EMBEDDED_INPUTS = {'crates/pkl-runtime/src/stdlib/' + name for name in
                   ('base.pkl', 'ref.pkl', 'json.pkl', 'test.pkl', 'modules.txt')}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def same_json(left, right):
    if type(left) is not type(right):
        return False
    if isinstance(left, dict):
        return left.keys() == right.keys() and all(same_json(left[k], right[k]) for k in left)
    if isinstance(left, list):
        return len(left) == len(right) and all(same_json(a, b) for a, b in zip(left, right))
    return left == right


def digest(data):
    return hashlib.sha256(data).hexdigest()


def identity(path):
    data = Path(path).read_bytes()
    return {'sha256': digest(data), 'bytes': len(data)}


def read(path):
    return json.loads(Path(path).read_bytes())


def put(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def safe(root, name):
    path = Path(name)
    require(not path.is_absolute() and path.parts and '..' not in path.parts
            and '\\' not in name and '\0' not in name, 'unsafe relative path')
    return root / path


def files(root):
    result = {}
    for path in root.rglob('*'):
        require(not path.is_symlink(), 'symlink in capture: ' + str(path))
        if path.is_file():
            result[path.relative_to(root).as_posix()] = identity(path)
    return dict(sorted(result.items()))


def source_names(root):
    names = set(m2_source_inputs.source_paths(root)) | MANDATORY
    fixture = root / 'tests/fixtures/m2-data-size'
    names.update(p.relative_to(root).as_posix() for p in fixture.rglob('*') if p.is_file())
    return sorted(names)


@contextmanager
def authority(fixture):
    verdict = data_size_fixtures.verify(fixture)
    with tempfile.TemporaryDirectory(prefix='pkl-data-size-comparison-') as directory:
        root = Path(directory)
        with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as archive:
            for member in archive:
                if not member.name.startswith('capture-03/'):
                    continue
                require(member.isfile(), 'nonregular Java archive member')
                path = safe(root, member.name)
                path.parent.mkdir(parents=True, exist_ok=True)
                require(not path.exists(), 'duplicate Java archive member')
                path.write_bytes(archive.extractfile(member).read())
        yield root / 'capture-03', verdict


def required_build_inputs(root):
    paths = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
             'crates/pkl-runtime/Cargo.toml', 'crates/pkl-runtime/examples/data_size_fixture_probe.rs'}
    paths.update(EMBEDDED_INPUTS)
    paths.update(p.relative_to(root).as_posix() for p in (root / '.cargo').rglob('*') if p.is_file())
    paths.update(p.relative_to(root).as_posix() for p in (root / 'crates/pkl-runtime/src').rglob('*')
                 if p.is_file() and (p.suffix in {'.rs', '.pkl'} or p.name == 'modules.txt'))
    return paths


def verify_build(build, sources):
    before, after = read(build / 'source-before.json'), read(build / 'source-after.json')
    require(before == after and isinstance(before, dict), 'build source changed')
    require(required_build_inputs(sources).issubset(before), 'build input closure incomplete')
    for name, record in before.items():
        require(identity(safe(sources, name)) == record, 'build source differs: ' + name)
    report, artifact = read(build / 'build.json'), read(build / 'artifact.json')
    expected = ['cargo', 'build', '-p', 'pkl-runtime', '--example', 'data_size_fixture_probe', '--locked']
    require(report['argv'][-1] in [expected, expected + ['--release']], 'unknown native build command')
    require(report['env'].get('CARGO_INCREMENTAL') == '0'
            and report['env'].get('CARGO_PROFILE_DEV_DEBUG') == '0', 'unrecorded native build profile')
    require(artifact['source_before_sha256'] == identity(build / 'source-before.json')['sha256']
            and artifact['source_after_sha256'] == identity(build / 'source-after.json')['sha256'],
            'artifact build manifest binding differs')
    require(identity(build / 'data_size_fixture_probe') == {k: artifact[k] for k in ('sha256', 'bytes')},
            'recorded executable differs')
    for index, command in enumerate(report['argv']):
        require((build / f'{index}.exit').read_bytes() == b'0\n', 'native build/check failed')
        require((build / f'{index}.stdout').is_file() and (build / f'{index}.stderr').is_file(),
                'native build raw output missing')
        require(isinstance(command, list) and all(isinstance(x, str) for x in command), 'invalid build argv')
    return {'artifact': identity(build / 'data_size_fixture_probe'), 'source_count': len(before),
            'build_manifest': identity(build / 'build.json'), 'scope': 'Recorded native build, not portable artifacts'}


def classify_row(reference, candidate):
    expected = {p.name for p in reference.iterdir() if p.is_file()}
    require(expected == set(COMMON) | set(JAVA_ONLY), 'Java row channel set differs')
    actual = {p.name for p in candidate.iterdir()} if candidate.is_dir() else set()
    differences = []
    for channel in COMMON:
        left, right = reference / channel, candidate / channel
        if not right.is_file() or right.read_bytes() != left.read_bytes():
            differences.append({'channel': channel, 'java': identity(left),
                                'candidate': identity(right) if right.is_file() else None})
    status = (candidate / 'result.status.bin').read_bytes() if (candidate / 'result.status.bin').is_file() else None
    metadata = API_METADATA if status == b'api-exception\n' else EVENT_METADATA
    layout = []
    if actual != set(COMMON) | metadata:
        layout.append({'missing': sorted((set(COMMON) | metadata) - actual),
                       'extra': sorted(actual - (set(COMMON) | metadata))})
    if status not in {b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'}:
        layout.append('unknown or missing candidate status')
    if status == b'error\n' and (candidate / 'candidate.disposition.bin').is_file():
        if (candidate / 'candidate.disposition.bin').read_bytes() != b'language-error':
            layout.append('ordinary error disposition differs')
    if status == b'candidate-error\n' and (candidate / 'candidate.disposition.bin').is_file():
        if (candidate / 'candidate.disposition.bin').read_bytes() == b'language-error':
            layout.append('candidate error mislabeled as ordinary error')
    if status == b'success\n' and (candidate / 'candidate.disposition.bin').is_file():
        if (candidate / 'candidate.disposition.bin').read_bytes():
            layout.append('successful value has an error disposition')
    return {'status': 'pass' if not differences and not layout else 'mismatch',
            'differences': differences, 'layout_issues': layout}


def compare_sessions(java, output, cases, processes):
    rows, sessions = [], []
    require(len(processes) == 2 and same_json([p['repeat'] for p in processes], [0, 1]), 'process selection differs')
    for repeat in range(2):
        reference, candidate = java / 'string' / f'repeat-{repeat}', output / 'native' / f'repeat-{repeat}'
        expected_dirs = {f'{i:03}-{row["id"]}' for i, row in enumerate(cases['rows'])}
        actual_dirs = {p.name for p in candidate.iterdir() if p.is_dir()}
        issues = []
        if expected_dirs != actual_dirs:
            issues.append('session row directory set differs')
        top_files = {p.name for p in candidate.iterdir() if p.is_file()}
        if top_files != {'candidate-trace.tsv', 'process.stdout.bin', 'process.stderr.bin'}:
            issues.append('session process/trace channel set differs')
        process = processes[repeat]
        if process['exit_code'] != 0 or process['timed_out'] or process['launch_error'] is not None:
            issues.append('candidate process did not complete successfully')
        for channel in ('stdout', 'stderr'):
            name = f'process.{channel}.bin'
            require(identity(candidate / name) == process[channel], 'process channel identity differs')
            if (candidate / name).read_bytes() != (reference / name).read_bytes():
                issues.append('raw process ' + channel + ' differs')
        trace = candidate / 'candidate-trace.tsv'
        lines = trace.read_bytes().splitlines() if trace.is_file() else []
        if lines.count(b'terminal-heap-slots\t0') != 1 or lines.count(b'terminal-context-depth\t0') != 1:
            issues.append('terminal zero-state observation missing')
        for index, row in enumerate(cases['rows']):
            directory = f'{index:03}-{row["id"]}'
            verdict = classify_row(reference / directory, candidate / directory)
            rows.append({'repeat': repeat, 'id': row['id'], 'directory': directory, **verdict})
        sessions.append({'repeat': repeat, 'issues': issues})
    counts = dict(Counter(row['status'] for row in rows))
    return {'rows': rows, 'sessions': sessions, 'counts': counts,
            'strict_string_passed': counts == {'pass': len(cases['rows']) * 2} and not any(s['issues'] for s in sessions)}


def verify_process_boundaries(processes):
    require(len(processes) == 2 and same_json([p['repeat'] for p in processes], [0, 1]), 'process selection differs')
    executed = [p for p in processes if p['launch_error'] is None]
    require(all(type(p['pid']) is int and p['pid'] > 0 for p in executed), 'invalid process identity')
    require(len({p['pid'] for p in executed}) == len(executed), 'process identities are not distinct')
    for process in processes:
        require(all(type(process[k]) in (int, float) and math.isfinite(process[k])
                    for k in ['started_epoch', 'finished_epoch']), 'invalid process times')
        require(process['finished_epoch'] >= process['started_epoch'], 'reversed process times')
    require(processes[0]['finished_epoch'] <= processes[1]['started_epoch'], 'ordered process sessions overlap')


def run_native(artifact, output, fixture, uri, environment, repeat, timeout):
    command = [str(artifact.resolve()), str(output.resolve()), str(fixture.resolve()), uri]
    start = time.time()
    stdout = stderr = b''
    process = None
    timed_out, error = False, None
    try:
        process = subprocess.Popen(command, cwd=fixture, env=environment, start_new_session=True,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            stdout, stderr = process.communicate()
    except OSError as failure:
        error = str(failure)
    (output / 'process.stdout.bin').write_bytes(stdout)
    (output / 'process.stderr.bin').write_bytes(stderr)
    return {'repeat': repeat, 'command': command, 'cwd': str(fixture.resolve()), 'environment': environment,
            'pid': process.pid if process else None, 'exit_code': process.returncode if process else None,
            'timed_out': timed_out, 'timeout_seconds': timeout, 'launch_error': error,
            'started_epoch': start, 'finished_epoch': time.time(),
            'stdout': identity(output / 'process.stdout.bin'), 'stderr': identity(output / 'process.stderr.bin')}


def acquire(output, fixture, build, timeout=120):
    output, fixture, build = output.resolve(), fixture.resolve(), build.resolve()
    require(not output.exists() and not output.is_symlink() and math.isfinite(timeout)
            and timeout > 0, 'new output directory and finite positive timeout required')
    with authority(fixture) as (java, authority_verdict):
        output.mkdir(parents=True, exist_ok=False)
        shutil.copytree(fixture, output / 'fixture')
        names = source_names(ROOT)
        before = {}
        for name in names:
            source, dest = safe(ROOT, name), safe(output / 'sources', name)
            dest.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, dest)
            before[name] = identity(dest)
        put(output / 'sources.json', before)
        shutil.copytree(build, output / 'build')
        binding = verify_build(output / 'build', output / 'sources')
        cases = read(output / 'fixture/cases.json')
        provenance = read(java / 'provenance.json')
        report = {'schema': 1, 'scope': 'Native common string-only DataSize comparison; public export and other targets remain required.',
                  'complete': False, 'host': platform.uname()._asdict(), 'python': sys.version,
                  'argv': sys.argv, 'workspace': str(ROOT), 'output': str(output), 'authority': authority_verdict,
                  'build_binding': binding, 'source_manifest': identity(output / 'sources.json'),
                  'sources_before': before, 'processes': [], 'comparison': None,
                  'public_export_status': 'not-executed-required', 'full_public_api_comparison_passed': False,
                  'other_targets_status': 'not-executed-required', 'full_m2_complete': False,
                  'performance_targets_achieved': False}
        put(output / 'report.json', report)
        for repeat in range(2):
            folder = output / 'native' / f'repeat-{repeat}'
            folder.mkdir(parents=True, exist_ok=False)
            report['processes'].append(run_native(output / 'build/data_size_fixture_probe', folder,
                output / 'fixture', cases['module_uri'], provenance['environment'], repeat, timeout))
            put(output / 'report.json', report)
        report['comparison'] = compare_sessions(java, output, cases, report['processes'])
        after_names = source_names(ROOT)
        after = {name: identity(ROOT / name) if (ROOT / name).is_file() else None for name in set(names) | set(after_names)}
        report['source_changes'] = sorted(name for name in set(before) | set(after) if before.get(name) != after.get(name))
        report['sources_after'] = after
        report['build_stable'] = verify_build(output / 'build', output / 'sources') == binding
        report['fixture_stable'] = files(fixture) == files(output / 'fixture')
        report['complete'] = True
        put(output / 'report.json', report)
        put(output / 'inventory.json', files(output))
    return verify_report(output)


def verify_report(output):
    output = output.resolve()
    report = read(output / 'report.json')
    require(type(report['schema']) is int and report['schema'] == 1 and report['complete'] is True, 'incomplete capture')
    actual = files(output)
    del actual['inventory.json']
    require(same_json(actual, read(output / 'inventory.json')), 'packet file inventory differs')
    require(same_json(files(output / 'sources'), report['sources_before'])
            and same_json(report['sources_before'], read(output / 'sources.json')), 'source snapshots differ')
    require(identity(output / 'sources.json') == report['source_manifest'], 'source manifest identity differs')
    require(source_names(output / 'sources') == sorted(report['sources_before']), 'source closure differs')
    expected_changes = sorted(name for name in report['sources_before'].keys() | report['sources_after'].keys()
                              if report['sources_before'].get(name) != report['sources_after'].get(name))
    require(report['source_changes'] == expected_changes, 'source stability accounting differs')
    binding = verify_build(output / 'build', output / 'sources')
    require(binding == report['build_binding'] and report['build_stable'] is True, 'build binding differs')
    require(report['fixture_stable'] is True, 'fixture changed during capture')
    with authority(output / 'fixture') as (java, verdict):
        require(verdict == report['authority'], 'authority verdict differs')
        cases = read(output / 'fixture/cases.json')
        environment = read(java / 'provenance.json')['environment']
        verify_process_boundaries(report['processes'])
        for process in report['processes']:
            require(process['environment'] == environment and process['command'][3] == cases['module_uri'], 'execution inputs differ')
            require(process['command'][0] == str(Path(report['output']) / 'build/data_size_fixture_probe')
                    and process['command'][1] == str(Path(report['output']) / 'native' / f'repeat-{process["repeat"]}')
                    and process['command'][2] == str(Path(report['output']) / 'fixture')
                    and process['cwd'] == str(Path(report['output']) / 'fixture'), 'execution path binding differs')
            require(type(process['timed_out']) is bool and math.isfinite(process['timeout_seconds']) and process['timeout_seconds'] > 0
                    and process['finished_epoch'] >= process['started_epoch'], 'process status/timing differs')
            require((process['pid'] is None) == (process['launch_error'] is not None)
                    and (process['exit_code'] is None) == (process['launch_error'] is not None), 'process launch accounting differs')
            require(process['pid'] is None or (type(process['pid']) is int and process['pid'] > 0), 'invalid process identity')
            require(process['exit_code'] is None or type(process['exit_code']) is int, 'invalid process exit status')
        comparison = compare_sessions(java, output, cases, report['processes'])
        require(same_json(comparison, report['comparison']), 'comparison accounting differs')
    require(report['public_export_status'] == report['other_targets_status'] == 'not-executed-required'
            and report['full_public_api_comparison_passed'] is False and report['full_m2_complete'] is False
            and report['performance_targets_achieved'] is False, 'unexecuted boundary promoted')
    return {'integrity': 'passed', 'counts': comparison['counts'],
            'strict_string_passed': comparison['strict_string_passed'] and not expected_changes,
            'full_public_api_comparison_passed': False, 'other_targets_status': 'not-executed-required'}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--output', type=Path)
    mode.add_argument('--verify-report', type=Path)
    mode.add_argument('--verify-string', type=Path)
    mode.add_argument('--verify-full', type=Path)
    parser.add_argument('--fixture', type=Path, default=data_size_fixtures.DEFAULT)
    parser.add_argument('--build-record', type=Path)
    parser.add_argument('--timeout', type=float, default=120)
    args = parser.parse_args()
    if args.output:
        require(args.build_record is not None, 'a recorded native build is required')
        result = acquire(args.output, args.fixture, args.build_record, args.timeout)
    else:
        result = verify_report(args.verify_report or args.verify_string or args.verify_full)
    print(json.dumps(result, sort_keys=True))
    if args.verify_full or (args.verify_string and not result['strict_string_passed']):
        return 1
    return 0


if __name__ == '__main__':
    raise SystemExit(main())
