#!/usr/bin/env python3
"""Fresh native SourceProbe replay of forty sealed Java parser modules."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import platform
import shutil
import sys
import tarfile

import collection_conversions_compare as native_build
from data_size_compare import (COMMON, EVENT_METADATA, API_METADATA, identity,
                               run_native, verify_process_boundaries)
import union_marker_fixtures as authority
from union_marker_oracle import MODULE_URI, PROGRAMS_SHA256

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-union-marker-diagnostics'
TOOLS = (
    'tools/union_marker_compare.py', 'tools/tests/test_union_marker_compare.py',
    'tools/union_marker_fixtures.py', 'tools/union_marker_oracle.py',
    'tools/amendment_construction_oracle.py', 'tools/collection_conversions_fixtures.py',
    'tools/collection_conversions_oracle.py', 'tools/collection_conversions_compare.py',
    'tools/collection_conversion_profiles.py', 'tools/data_size_compare.py',
    'tools/data_size_fixtures.py', 'tools/data_size_oracle.py', 'tools/m2_source_inputs.py',
    'tools/java/CollectionConversionsOracle.java',
)


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read(path):
    return json.loads(Path(path).read_bytes())


def put(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def exact_files(root):
    files = set()
    for path in root.rglob('*'):
        need(not path.is_symlink(), 'symlink in native capture')
        if path.is_file():
            files.add(path.relative_to(root).as_posix())
        else:
            need(path.is_dir(), 'nonregular native capture entry')
    return files


def java_authority(fixture):
    verdict = authority.verify(fixture)
    programs = read(fixture / 'programs.json')['programs']
    need(len(programs) == 40 and sha((fixture / 'programs.json').read_bytes()) == PROGRAMS_SHA256,
         'canonical authored modules differ')
    with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as archive:
        files = {member.name.removeprefix('capture-01/'): archive.extractfile(member).read()
                 for member in archive}
    provenance = json.loads(files['provenance.json'])
    need(provenance['complete'] is True and provenance['stable'] is True and
         len(provenance['sessions']) == 80 and len({s['pid'] for s in provenance['sessions']}) == 80,
         'Java separate-module authority incomplete')
    return programs, files, provenance, verdict


def input_files(output, fixture, programs):
    folders = []
    for index, program in enumerate(programs):
        folder = output / 'inputs' / f"{index:02d}-{program['id']}"
        folder.mkdir(parents=True)
        raw = (fixture / program['module_file']).read_bytes()
        need(sha(raw) == program['module_sha256'] and len(raw) == program['module_bytes'],
             'authored module bytes changed')
        (folder / 'module.pkl').write_bytes(raw)
        (folder / 'queries.tsv').write_bytes(f"string\t{program['id']}\tresult\n".encode())
        folders.append(folder)
    return folders


def compare_row(reference, folder):
    status_file = folder / 'result.status.bin'
    status = status_file.read_bytes() if status_file.is_file() else None
    expected = set(COMMON) | (API_METADATA if status == b'api-exception\n' else EVENT_METADATA)
    actual = {p.name for p in folder.iterdir()} if folder.is_dir() else set()
    missing = sorted(expected - actual)
    extra = sorted(actual - expected)
    differences = [name for name in COMMON
                   if not (folder / name).is_file() or (folder / name).read_bytes() != reference[name]]
    issues = []
    if missing or extra:
        issues.append({'missing': missing, 'extra': extra})
    if status not in (b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'):
        issues.append('invalid candidate status')
    if status == b'error\n' and (folder / 'candidate.disposition.bin').is_file() and (
            folder / 'candidate.disposition.bin').read_bytes() != b'language-error':
        issues.append('ordinary error disposition differs')
    if status == b'candidate-error\n' and (folder / 'candidate.disposition.bin').is_file() and (
            folder / 'candidate.disposition.bin').read_bytes() == b'language-error':
        issues.append('candidate error mislabeled ordinary')
    if status == b'success\n' and (folder / 'candidate.disposition.bin').is_file() and (
            folder / 'candidate.disposition.bin').read_bytes():
        issues.append('success has error disposition')
    return {'status': 'pass' if not differences and not issues else 'mismatch',
            'different_channels': differences, 'issues': issues}


def verify_global_order(processes):
    need(len(processes) == 80, 'native process coverage differs')
    need(all(first['finished_epoch'] <= second['started_epoch']
             for first, second in zip(processes, processes[1:])),
         'ordered native module sessions overlap')


def compare(output, build, fixture, programs, java, provenance, processes):
    verify_global_order(processes)
    rows, sessions = [], []
    all_pids = set()
    for index, program in enumerate(programs):
        pair = processes[index * 2:index * 2 + 2]
        verify_process_boundaries(pair)
        for repeat, process in enumerate(pair):
            name = f"{index:02d}-{program['id']}"
            folder = output / 'native' / name / f'repeat-{repeat}'
            input_folder = output / 'inputs' / name
            java_base = f'programs/{name}/repeat-{repeat}/'
            row_base = java_base + f"000-{program['id']}/"
            need(process['index'] == index and process['id'] == program['id'] and
                 process['repeat'] == repeat and process['command'] ==
                 [str((build / 'string_session_probe').resolve()), str(folder.resolve()),
                  str(input_folder.resolve()), MODULE_URI] and
                 process['cwd'] == str(input_folder.resolve()) and
                 process['environment'] == provenance['environment'] and
                 process['exit_code'] == 0 and process['timed_out'] is False and
                 process['launch_error'] is None and process['timeout_seconds'] == 120 and
                 process['pid'] not in all_pids,
                 'native executable/argv/env/fresh process differs')
            all_pids.add(process['pid'])
            need(exact_files(folder) == {'process.stdout.bin', 'process.stderr.bin',
                                         'candidate-trace.tsv'} |
                 {f"000-{program['id']}/{name}" for name in exact_files(folder / f"000-{program['id']}" )},
                 'native session file set differs')
            candidate = folder / f"000-{program['id']}"
            reference = {name: java[row_base + name] for name in COMMON}
            verdict = compare_row(reference, candidate)
            rows.append({'index': index, 'id': program['id'], 'repeat': repeat, **verdict})
            issues = []
            for channel in ('stdout', 'stderr'):
                raw = (folder / f'process.{channel}.bin').read_bytes()
                need(identity(folder / f'process.{channel}.bin') == process[channel],
                     'native OS raw process hash differs')
                if raw != java[java_base + f'process.{channel}.bin']:
                    issues.append('process.' + channel + ' differs')
            trace = (folder / 'candidate-trace.tsv').read_bytes().splitlines()
            need(trace.count(b'terminal-heap-slots\t0') ==
                 trace.count(b'terminal-context-depth\t0') == 1,
                 'native terminal lifetime proof missing')
            sessions.append({'index': index, 'id': program['id'], 'repeat': repeat,
                             'issues': issues})
    counts = dict(Counter(row['status'] for row in rows))
    return {'rows': rows, 'sessions': sessions, 'counts': counts,
            'strict_string_passed': counts == {'pass': 80} and
                                    all(not session['issues'] for session in sessions)}


def acquire(output, build, fixture=FIXTURE):
    output, build, fixture = (Path(path).resolve() for path in (output, build, fixture))
    need(not output.exists() and not output.is_symlink(), 'fresh native output required')
    native_build.verify_build(build)
    sources = read(build / 'source-before.json')
    need(all(identity(ROOT / name) == record for name, record in sources.items()),
         'current runtime sources differ from frozen native build')
    programs, java, provenance, authority_verdict = java_authority(fixture)
    tools_before = {name: identity(ROOT / name) for name in TOOLS}
    output.mkdir(parents=True)
    shutil.copytree(fixture, output / 'fixture')
    inputs = input_files(output, fixture, programs)
    for name in TOOLS:
        destination = output / 'tools' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, destination)
    report = {'schema': 1, 'complete': False, 'scope': 'Native complete common string channels for forty separate parser modules; full API/M2 remains required.',
              'host': platform.uname()._asdict(), 'python': sys.version,
              'build': str(build), 'artifact': identity(build / 'string_session_probe'),
              'fixture': str(fixture), 'module_uri': MODULE_URI,
              'authority': authority_verdict, 'tools_before': tools_before,
              'build_sources_before': sources, 'processes': [],
              'producer_error_class': 'Java exception-class, toString and cause fields remain producer-only; native string SourceProbe has seven common channels.',
              'full_public_api_passed': False, 'full_m2_passed': False,
              'performance_claim': False}
    put(output / 'report.json', report)
    for index, program in enumerate(programs):
        for repeat in (0, 1):
            folder = output / 'native' / f"{index:02d}-{program['id']}" / f'repeat-{repeat}'
            folder.mkdir(parents=True)
            process = run_native(build / 'string_session_probe', folder, inputs[index], MODULE_URI,
                                 provenance['environment'], repeat, 120)
            process.update(index=index, id=program['id'])
            report['processes'].append(process)
            put(output / 'report.json', report)
            if process['exit_code'] != 0 or process['timed_out'] or process['launch_error']:
                raise ValueError('native child failed; raw attempt retained')
    report['comparison'] = compare(output, build, output / 'fixture', programs, java,
                                   provenance, report['processes'])
    report['build_sources_after'] = {name: identity(ROOT / name) for name in sources}
    report['tools_after'] = {name: identity(ROOT / name) for name in TOOLS}
    report['artifact_after'] = identity(build / 'string_session_probe')
    report['complete'] = (report['build_sources_before'] == report['build_sources_after'] and
                          report['tools_before'] == report['tools_after'] and
                          report['artifact'] == report['artifact_after'])
    put(output / 'report.json', report)
    inventory = {name: identity(output / name) for name in exact_files(output)}
    put(output / 'inventory.json', inventory)
    if not report['complete']:
        raise ValueError('native source changed; raw attempt retained')
    return verify(output)


def verify(output):
    output = Path(output).resolve()
    report = read(output / 'report.json')
    need(report['schema'] == 1 and report['complete'] is True and
         report['fixture'] == str(FIXTURE.resolve()) and report['module_uri'] == MODULE_URI and
         report['producer_error_class'].startswith('Java exception-class') and
         report['full_public_api_passed'] is False and report['full_m2_passed'] is False and
         report['performance_claim'] is False,
         'native capture completion/scope differs')
    names = exact_files(output)
    need('inventory.json' in names, 'native inventory missing')
    names.remove('inventory.json')
    need(read(output / 'inventory.json') == {name: identity(output / name) for name in names},
         'native capture full file identity differs')
    build = Path(report['build'])
    native_build.verify_build(build)
    need(report['artifact'] == report['artifact_after'] == identity(build / 'string_session_probe') and
         report['build_sources_before'] == report['build_sources_after'] ==
         read(build / 'source-before.json') and
         all(identity(ROOT / name) == record for name, record in report['build_sources_before'].items()),
         'native artifact/source binding differs')
    need(report['tools_before'] == report['tools_after'] ==
         {name: identity(ROOT / name) for name in TOOLS} and
         all(identity(output / 'tools' / name) == report['tools_before'][name] for name in TOOLS),
         'native comparator source closure differs')
    programs, java, provenance, authority_verdict = java_authority(output / 'fixture')
    need(report['authority'] == authority_verdict and len(report['processes']) == 80,
         'native Java authority or process coverage differs')
    expected_inputs = input_files_check(output, programs)
    need(expected_inputs == {name for name in names if name.startswith('inputs/')},
         'native source/query input set differs')
    expected = {'report.json'} | expected_inputs
    expected.update('fixture/' + name for name in exact_files(output / 'fixture'))
    expected.update('tools/' + name for name in TOOLS)
    for index, program in enumerate(programs):
        for repeat in (0, 1):
            prefix = f"native/{index:02d}-{program['id']}/repeat-{repeat}/"
            expected.update(prefix + name for name in exact_files(output / prefix))
    need(names == expected, 'native top-level file set differs')
    observed = compare(output, build, output / 'fixture', programs, java,
                       provenance, report['processes'])
    need(report['comparison'] == observed, 'native raw replay report differs')
    return report


def input_files_check(output, programs):
    names = set()
    for index, program in enumerate(programs):
        path = output / 'inputs' / f"{index:02d}-{program['id']}"
        module = path / 'module.pkl'
        queries = path / 'queries.tsv'
        need(module.read_bytes() == (output / 'fixture' / program['module_file']).read_bytes() and
             queries.read_bytes() == f"string\t{program['id']}\tresult\n".encode(),
             'native copied module/query bytes differ')
        names.update({module.relative_to(output).as_posix(), queries.relative_to(output).as_posix()})
    return names


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--build', type=Path)
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    parser.add_argument('--verify', type=Path)
    parser.add_argument('--gate', choices=('integrity', 'string'), default='string')
    args = parser.parse_args()
    need(bool(args.verify) != bool(args.output), 'choose exactly acquisition or offline verification')
    if args.verify:
        report = verify(args.verify)
    else:
        need(args.build is not None, 'native build required')
        report = acquire(args.output, args.build, args.fixture)
    print(json.dumps({'complete': report['complete'],
                      'comparison': report['comparison']['counts'],
                      'strict_string_passed': report['comparison']['strict_string_passed'],
                      'full_public_api_passed': False}, sort_keys=True))
    raise SystemExit(0 if args.gate == 'integrity' or
                     report['comparison']['strict_string_passed'] else 1)


if __name__ == '__main__':
    main()
