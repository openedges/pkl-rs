"""Acquire and verify complete native collection conversion string sessions against Java."""
from __future__ import annotations

import argparse
from collections import Counter
import json
from pathlib import Path
import platform
import shutil
import sys
import tarfile

import collection_conversion_profiles as profiles
from data_size_compare import (COMMON, EVENT_METADATA, API_METADATA, files, identity,
                               put, read, require, run_native, verify_process_boundaries)

ROOT = Path(__file__).resolve().parents[1]
TOOLS = tuple(sorted(profiles.PROFILE_SOURCES | {
    'tools/collection_conversions_compare.py',
    'tools/tests/test_collection_conversions_compare.py',
    'tools/data_size_compare.py', 'tools/data_size_fixtures.py',
    'tools/data_size_oracle.py', 'tools/m2_source_inputs.py',
}))
EMBEDDED_INPUTS = {'crates/pkl-runtime/src/stdlib/' + name for name in
                   ('base.pkl', 'ref.pkl', 'json.pkl', 'test.pkl', 'math.pkl', 'modules.txt')}


def build_inputs(root):
    names = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
             'crates/pkl-runtime/Cargo.toml',
             'crates/pkl-runtime/examples/string_session_probe.rs',
             'crates/pkl-runtime/examples/data_size_fixture_probe.rs',
             'crates/pkl-runtime/examples/support/string_session.rs'}
    names.update(EMBEDDED_INPUTS)
    for directory in ('crates/pkl-runtime/src', '.cargo'):
        names.update(p.relative_to(root).as_posix() for p in (root / directory).rglob('*') if p.is_file())
    return names


def verify_build(build):
    before, after = read(build / 'source-before.json'), read(build / 'source-after.json')
    report = read(build / 'build.json')
    require(before == after and report['source_stable'] is True, 'build input changed')
    require(files(build / 'sources') == before, 'captured build source differs')
    require(build_inputs(build / 'sources').issubset(before), 'build source closure incomplete')
    require(report['complete'] is True and report['exit_code'] == 0, 'build incomplete or failed')
    require(report['argv'] == ['cargo', 'build', '-p', 'pkl-runtime', '--example',
                              'string_session_probe', '--example', 'data_size_fixture_probe', '--locked'],
            'unexpected build command')
    require(report['env']['CARGO_INCREMENTAL'] == '0'
            and report['env']['CARGO_PROFILE_DEV_DEBUG'] == '0', 'build profile differs')
    for channel in ('stdout', 'stderr'):
        require(identity(build / f'build.{channel}.bin') == report[channel], 'build log differs')
    for name in ('string_session_probe', 'data_size_fixture_probe'):
        require(identity(build / name) == report['artifacts'][name], 'built artifact differs')
    return report['artifacts']


def java_files(fixture):
    profile = profiles.profile_for_fixture(fixture)
    authority = profiles.authority_for_fixture(fixture)
    authority.verify(fixture)
    with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as archive:
        return {m.name.removeprefix(profile['archive_prefix'] + '/'): archive.extractfile(m).read()
                for m in archive}


def compare(output, processes):
    fixture = output / 'fixture'
    java = java_files(fixture)
    profile = profiles.profile_for_fixture(fixture)
    cases = read(fixture / 'cases.json')['rows']
    recorded_output = Path(read(output / 'report.json')['output'])
    require(recorded_output.is_absolute(), 'recorded capture path differs')
    environment = json.loads(java['provenance.json'])['environment']
    verify_process_boundaries(processes)
    rows, sessions = [], []
    for repeat, process in enumerate(processes):
        folder = output / 'native' / f'repeat-{repeat}'
        expected_dirs = {f'{i:03d}-{row["id"]}' for i, row in enumerate(cases)}
        require({p.name for p in folder.iterdir() if p.is_dir()} == expected_dirs,
                'candidate row directory coverage differs')
        require({p.name for p in folder.iterdir() if p.is_file()} == {
            'candidate-trace.tsv', 'process.stdout.bin', 'process.stderr.bin'}, 'candidate session file set differs')
        require(process['exit_code'] == 0 and process['timed_out'] is False
                and process['launch_error'] is None, 'candidate process incomplete')
        require(process['command'] == [str(recorded_output / 'build/string_session_probe'),
                str(recorded_output / 'native' / f'repeat-{repeat}'),
                str(recorded_output / 'fixture'), profile['module_uri']]
                and process['cwd'] == str(recorded_output / 'fixture'), 'candidate command differs')
        require(process['environment'] == environment, 'candidate environment differs from Java')
        require(type(process['timeout_seconds']) in (int, float)
                and process['timeout_seconds'] == 120, 'candidate timeout differs')
        counts = Counter()
        protocol = [f'session-open mode=string rows={len(cases)}']
        for index, row in enumerate(cases):
            name = f'{index:03d}-{row["id"]}'
            candidate = folder / name
            status = (candidate / 'result.status.bin').read_bytes()
            require(status in (b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'),
                    'invalid candidate status')
            metadata = API_METADATA if status == b'api-exception\n' else EVENT_METADATA
            require({p.name for p in candidate.iterdir()} == set(COMMON) | metadata,
                    'candidate raw channel set differs')
            if status != b'api-exception\n':
                disposition = (candidate / 'candidate.disposition.bin').read_bytes()
                require((status != b'error\n' or disposition == b'language-error')
                        and (status != b'candidate-error\n' or disposition != b'language-error')
                        and (status != b'success\n' or disposition == b''), 'candidate disposition differs')
            mismatches = [channel for channel in COMMON if (candidate / channel).read_bytes()
                          != java[f'repeat-{repeat}/{name}/{channel}']]
            rows.append({'repeat': repeat, 'id': row['id'], 'status': 'mismatch' if mismatches else 'pass',
                         'different_channels': mismatches})
            clean = status.decode('ascii')[:-1]
            counts[clean] += 1
            protocol.extend([f'call-enter=string-{index}-{row["id"]}',
                             f'call-exit=string-{index}-{row["id"]} status={clean}'])
        protocol.append(f'session-closed mode=string rows={len(cases)} public-calls={len(cases)} '
                        f'success={counts["success"]} errors={len(cases)-counts["success"]}')
        require((folder / 'process.stdout.bin').read_bytes() == ('\n'.join(protocol) + '\n').encode(),
                'candidate complete protocol differs from actual rows')
        differences = []
        for channel in ('stdout', 'stderr'):
            path = folder / f'process.{channel}.bin'
            require(identity(path) == process[channel], 'recorded process bytes differ')
            if path.read_bytes() != java[f'repeat-{repeat}/process.{channel}.bin']:
                differences.append(channel)
        trace = (folder / 'candidate-trace.tsv').read_bytes().splitlines()
        require(trace.count(b'terminal-heap-slots\t0') == trace.count(b'terminal-context-depth\t0') == 1,
                'terminal lifecycle observation missing')
        sessions.append({'repeat': repeat, 'different_process_channels': differences,
                         'candidate_status_counts': dict(counts)})
    counts = dict(Counter(row['status'] for row in rows))
    return {'counts': counts, 'rows': rows, 'sessions': sessions,
            'strict_string_passed': counts == {'pass': len(cases)*profile['repetitions']}
            and all(not session['different_process_channels'] for session in sessions)}


def acquire(output, build, fixture=profiles.ROOT / profiles.PROFILES[
        'file:///stage62/collection-conversions.pkl']['fixture']):
    require(not output.exists(), 'fresh output required')
    profile = profiles.profile_for_fixture(fixture)
    verify_build(build)
    before = read(build / 'source-before.json')
    require(all(identity(ROOT / name) == record for name, record in before.items()),
            'current sources differ from built artifact')
    output.mkdir(parents=True)
    shutil.copytree(build, output / 'build')
    shutil.copytree(fixture, output / 'fixture')
    for name in TOOLS:
        dest = output / 'tools' / Path(name).name
        dest.parent.mkdir(exist_ok=True)
        shutil.copy2(ROOT / name, dest)
    tools_before = {name: identity(ROOT / name) for name in TOOLS}
    java = java_files(output / 'fixture')
    environment = json.loads(java['provenance.json'])['environment']
    report = {'schema': 1, 'complete': False, 'scope': 'Native string-only collection conversion; full API and M2 remain required.',
              'host': platform.uname()._asdict(), 'python': sys.version,
              'output': str(output), 'profile_uri': profile['module_uri'],
              'tools_before': tools_before, 'processes': [],
              'full_public_api_passed': False, 'full_m2_passed': False, 'performance_claim': False}
    put(output / 'report.json', report)
    uri = profile['module_uri']
    for repeat in range(profile['repetitions']):
        folder = output / 'native' / f'repeat-{repeat}'
        folder.mkdir(parents=True)
        report['processes'].append(run_native(output / 'build/string_session_probe', folder,
                                              output / 'fixture', uri, environment, repeat, 120))
        put(output / 'report.json', report)
    report['comparison'] = compare(output, report['processes'])
    report['source_after'] = {name: identity(ROOT / name) for name in before}
    report['tools_after'] = {name: identity(ROOT / name) for name in TOOLS}
    report['complete'] = True
    put(output / 'report.json', report)
    put(output / 'inventory.json', files(output))
    return verify(output)


def verify(output):
    report = read(output / 'report.json')
    require(report['schema'] == 1 and report['complete'] is True, 'incomplete acquisition')
    actual = files(output)
    actual.pop('inventory.json')
    require(actual == read(output / 'inventory.json'), 'capture file identity differs')
    verify_build(output / 'build')
    require(report['profile_uri'] == profiles.profile_for_fixture(output / 'fixture')['module_uri'],
            'captured profile URI differs')
    require(report['source_after'] == read(output / 'build/source-before.json'), 'candidate source changed')
    require(report['tools_before'] == report['tools_after']
            == {name: identity(output / 'tools' / Path(name).name) for name in TOOLS}, 'capture tools changed')
    verdict = compare(output, report['processes'])
    require(verdict == report['comparison'], 'stored comparison differs')
    require(all(report[key] is False for key in ('full_public_api_passed', 'full_m2_passed', 'performance_claim')),
            'unsupported broader claim')
    return {'integrity_passed': True, 'strict_string_passed': verdict['strict_string_passed'],
            'counts': verdict['counts'], 'mismatches': [row for row in verdict['rows'] if row['status'] != 'pass']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--acquire', type=Path)
    mode.add_argument('--verify', type=Path)
    parser.add_argument('--build', type=Path)
    parser.add_argument('--fixture', type=Path, default=profiles.ROOT / profiles.PROFILES[
        'file:///stage62/collection-conversions.pkl']['fixture'])
    parser.add_argument('--gate', choices=('integrity', 'string'), default='string')
    args = parser.parse_args()
    if args.acquire:
        parser.error('--build required for acquisition') if args.build is None else None
        verdict = acquire(args.acquire.resolve(), args.build.resolve(), args.fixture.resolve())
    else:
        verdict = verify(args.verify.resolve())
    print(json.dumps(verdict, indent=2))
    raise SystemExit(0 if verdict['integrity_passed' if args.gate == 'integrity' else 'strict_string_passed'] else 1)
