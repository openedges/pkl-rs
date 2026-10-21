#!/usr/bin/env python3
"""Acquire ordered class-publication calls in one evaluator/module per sequence."""
import argparse
from datetime import datetime, timezone
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import benchmark
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path('compatibility/m2-class-lifecycle')
REPETITIONS = 2


def source_paths(root=ROOT, java_only=False):
    fixed = ['references.lock', 'oracle.lock.json', 'tools/m2_class_lifecycle.py',
             'tools/java/M2ClassLifecycleProbe.java', 'tools/java/M2ClassLifecycleHostProbe.java', 'tools/java/M2ClassIdentityProbe.java',
             'tools/benchmark.py', 'tools/oracle.py',
             'tools/tests/test_m2_class_lifecycle.py']
    paths = [root / path for path in fixed]
    if any(not path.is_file() for path in paths):
        raise ValueError('missing required lifecycle source')
    paths.extend(path for path in sorted((root / FIXTURES).rglob('*')) if path.is_file())
    if not java_only:
        paths.extend(root / path for path in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'])
        paths.extend(path for path in sorted((root / 'crates').rglob('*')) if path.is_file())
    return paths


def manifest_at(root):
    manifest = json.loads((root / FIXTURES / 'manifest.json').read_bytes())
    rows = manifest['sequences']
    if manifest.get('schema') != 1 or not rows or len({r['id'] for r in rows}) != len(rows):
        raise ValueError('empty, duplicate or unsupported sequence manifest')
    if manifest['uri'] != 'file:///m2-platform.pkl' or manifest['power_assertions'] is not False:
        raise ValueError('probe transport context changed')
    for row in rows:
        for key in ('source', 'expressions'):
            relative = Path(row[key])
            if relative.is_absolute() or '..' in relative.parts:
                raise ValueError('unsafe fixture path')
        calls = (root / FIXTURES / row['expressions']).read_text().splitlines()
        if not calls or any(not c or any(not (part and all(ch.isalnum() or ch == '_' for ch in part))
                                          for part in c.split('.')) for c in calls):
            raise ValueError('expressions must be nonempty supported property paths')
    return manifest


def inventory(directory):
    return {p.relative_to(directory).as_posix(): oracle.sha256(p)
            for p in sorted(directory.rglob('*')) if p.is_file()
            and p != directory / 'report.json' and not p.is_relative_to(directory / 'build')}


def channel(result, directory, name):
    data = (directory / (name + '.bin')).read_bytes()
    identity = result[name]
    if len(data) != identity['bytes'] or benchmark.digest(data) != identity['sha256']:
        raise ValueError('process capture identity changed')
    return data


def observe_process(directory, result, calls):
    stdout = channel(result, directory, 'stdout')
    stderr = channel(result, directory, 'stderr')
    protocol = ''.join(f'call={i} completed\n' for i in range(len(calls))).encode() + b'closed\n'
    executed = result['status'] == 'executed' and result['exit_code'] == 0
    if not executed or stdout != protocol or stderr:
        return {'status': 'acquisition-failed', 'calls': [], 'reason': 'process failed or incomplete ordered protocol'}
    outcomes = []
    expected_files = {f'{i}.{name}.bin' for i in range(len(calls))
                      for name in ('stdout', 'stderr', 'status', 'error-class')}
    actual_files = {p.name for p in directory.glob('*.bin') if p.name not in {'stdout.bin', 'stderr.bin'}}
    if actual_files != expected_files:
        raise ValueError('missing or extra per-call captures')
    for i, expression in enumerate(calls):
        data = {name: (directory / f'{i}.{name}.bin').read_bytes()
                for name in ('stdout', 'stderr', 'status', 'error-class')}
        if data['status'] not in {b'success\n', b'error\n', b'oracle-error\n'}:
            raise ValueError('unknown per-call result kind')
        if (data['status'] == b'success\n' and data['stderr']) or (data['status'] != b'success\n' and (not data['stderr'] or data['stdout'])):
            raise ValueError('inconsistent per-call channels')
        outcomes.append({'index': i, 'expression': expression,
                         'status': data['status'].decode().removesuffix('\n'),
                         'channels': {name: {'bytes': len(value), 'sha256': benchmark.digest(value)}
                                      for name, value in data.items()}})
    return {'status': 'executed', 'calls': outcomes}


def observe_pair(directory, row, calls, languages):
    result = {lang: observe_process(directory / lang, row[lang], calls) for lang in languages}
    comparisons = []
    if languages == ['java', 'rust']:
        for index, expression in enumerate(calls):
            executed = all(result[lang]['status'] == 'executed' for lang in languages)
            equal = executed and all((directory / 'java' / f'{index}.{name}.bin').read_bytes() ==
                                     (directory / 'rust' / f'{index}.{name}.bin').read_bytes()
                                     for name in ('status', 'stdout', 'stderr'))
            valid = executed and all(result[lang]['calls'][index]['status'] != 'oracle-error' for lang in languages)
            comparisons.append({'index': index, 'expression': expression, 'raw_equal': equal,
                                'status': 'pass' if valid and equal else ('mismatch' if executed else 'unexecuted')})
    return {'processes': result, 'comparisons': comparisons}


def contract_passes(report):
    return (not report.get('acquisition_error') and report.get('languages') == ['java', 'rust']
            and bool(report.get('cases')) and all(row['observations']['comparisons'] and
                all(call['status'] == 'pass' for call in row['observations']['comparisons']) for row in report['cases']))


def checked_command(directory, relative, result, expected_command):
    if result != json.loads((directory / relative / 'result.json').read_bytes()):
        raise ValueError('process identity differs from its retained result')
    if result['command'] != [str(arg) for arg in expected_command] or result['cwd'] != report_workspace(directory):
        raise ValueError('unexpected lifecycle command/source/working directory')
    for name in ('stdout', 'stderr'):
        if result[name]['path'] != str(Path(json.loads((directory / 'report.json').read_bytes())['output_directory']) / relative / (name + '.bin')):
            raise ValueError('capture path differs from command identity')
        channel(result, directory / relative, name)


def report_workspace(directory):
    return json.loads((directory / 'report.json').read_bytes())['workspace']


def check_report(directory):
    directory = Path(directory)
    report = json.loads((directory / 'report.json').read_bytes())
    if report.get('schema') != 1 or report.get('acquisition_error'):
        raise ValueError('failed or unsupported lifecycle acquisition report')
    if report.get('languages') not in (['java'], ['java', 'rust']):
        raise ValueError('unknown lifecycle languages')
    if not report.get('source_stability_before_execution') or not report.get('source_stability_after_execution'):
        raise ValueError('candidate sources changed during acquisition')
    if not report.get('sources') or not report.get('artifacts') or report.get('repetitions') != REPETITIONS:
        raise ValueError('missing sources/artifacts/repetition contract')
    for row in report['sources']:
        if oracle.sha256(directory / row['snapshot']) != row['sha256']:
            raise ValueError('source snapshot changed')
    for row in report['artifacts']:
        if (oracle.sha256(directory / row['path']) != row['sha256']
                or (directory / row['path']).stat().st_size != row['bytes']):
            raise ValueError('artifact changed')
    if inventory(directory) != report['evidence_sha256']:
        raise ValueError('evidence inventory changed')
    snapshot_root = directory / 'sources'
    expected_sources = {p.relative_to(snapshot_root).as_posix() for p in source_paths(snapshot_root, report['languages'] == ['java'])}
    if {row['path'] for row in report['sources']} != expected_sources or len(report['sources']) != len(expected_sources):
        raise ValueError('missing, duplicate or unexpected source identities')
    for row in report['sources']:
        if row['snapshot'] != 'sources/' + row['path'] or row['bytes'] != (snapshot_root / row['path']).stat().st_size:
            raise ValueError('source identity path/size mismatch')
    manifest = manifest_at(snapshot_root)
    metadata = report['oracle']
    if metadata != json.loads((directory / 'oracle.json').read_bytes()):
        raise ValueError('oracle metadata changed')
    lock = json.loads((snapshot_root / 'oracle.lock.json').read_bytes())
    if (metadata['lock_sha256'] != oracle.sha256(snapshot_root / 'oracle.lock.json')
            or metadata['version'] != lock['version'] or metadata['source'] != lock['source']
            or metadata['artifact_sha256'] != lock['artifact']['sha256']
            or metadata['release']['commitId'] != lock['artifact']['commit_id']
            or any(metadata['jdk']['properties'].get(key) != value for key, value in lock['jdk']['properties'].items())):
        raise ValueError('oracle artifact/source/JDK is not pinned')
    cache = Path(report['workspace']) / '.cache/oracle'
    expected_environment = {'PATH': os.defpath, 'HOME': str(cache / 'home'), 'TMPDIR': str(cache / 'tmp'),
                            'TMP': str(cache / 'tmp'), 'TEMP': str(cache / 'tmp'), 'LC_ALL': 'C', 'LANG': 'C',
                            'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1', 'COLUMNS': '80'}
    expected_oracle_command = [metadata['java'], *lock['jvm_options'],
                               '-Duser.home=' + expected_environment['HOME'],
                               '-Djava.io.tmpdir=' + expected_environment['TMPDIR'],
                               '-cp', str(cache / lock['artifact']['filename']), 'org.pkl.cli.Main']
    if (metadata.get('profile') != 'reference' or metadata['command'] != expected_oracle_command
            or metadata['environment'] != expected_environment
            or metadata['artifact'] != str(cache / lock['artifact']['filename'])):
        raise ValueError('oracle execution conditions changed')
    for key in ('executable_sha256', 'runtime_modules_sha256', 'release_sha256'):
        digest = metadata['jdk'][key]
        if len(digest) != 64 or any(ch not in '0123456789abcdef' for ch in digest):
            raise ValueError('missing Java runtime identity')
    if report.get('compatibility_complete') is not False or report.get('comparison_channels') != ['status', 'stdout', 'stderr']:
        raise ValueError('unsupported compatibility claim or comparison channels')
    original = Path(report['output_directory'])
    if not original.is_absolute():
        raise ValueError('recorded evidence directory must be absolute')
    classes = original / 'classes'
    original_sources = original / 'sources'
    javac = [str(Path(metadata['java']).with_name('javac')), '-cp', metadata['artifact'], '-d', str(classes), str(original_sources / 'tools/java/M2ClassLifecycleProbe.java')]
    checked_command(directory, Path('javac'), report['javac'], javac)
    checked(report['javac'])
    artifact_paths = {p.relative_to(directory).as_posix() for p in (directory / 'classes').glob('*.class')}
    binary = original / 'binaries/class_lifecycle_probe'
    if report['languages'] == ['java', 'rust']:
        checked_command(directory, Path('rustc'), report['rustc'], ['rustc', '-vV'])
        checked(report['rustc'])
        build_command = ['cargo', 'build', '-p', 'pkl-runtime', '--locked', '--release', '--example', 'class_lifecycle_probe', '--message-format=json']
        checked_command(directory, Path('cargo'), report['build'], build_command)
        checked(report['build'])
        messages = [json.loads(line) for line in channel(report['build'], directory / 'cargo', 'stdout').splitlines()]
        executables = [Path(r['executable']) for r in messages if r.get('reason') == 'compiler-artifact' and r.get('executable') and r.get('target', {}).get('name') == 'class_lifecycle_probe']
        if len(executables) != 1 or not executables[0].is_relative_to(original / 'build'):
            raise ValueError('fresh candidate binary origin missing')
        artifact_paths.add('binaries/class_lifecycle_probe')
        if report['candidate_build_sha256'] != oracle.sha256(directory / 'binaries/class_lifecycle_probe'):
            raise ValueError('candidate build/copy identity mismatch')
    if {r['path'] for r in report['artifacts']} != artifact_paths or len(report['artifacts']) != len(artifact_paths):
        raise ValueError('missing/duplicate artifact identities')
    cp = metadata['command'].index('-cp')
    prefix = [*metadata['command'][:cp], '-cp', os.pathsep.join((str(classes), metadata['artifact'])), 'M2ClassLifecycleProbe']
    expected = [(r, repetition) for r in manifest['sequences'] for repetition in range(REPETITIONS)]
    if [(r['id'], r['repetition']) for r in report['cases']] != [(r['id'], n) for r, n in expected]:
        raise ValueError('missing, duplicate or reordered sequence executions')
    for row, (spec, repetition) in zip(report['cases'], expected):
        calls = (directory / 'sources' / FIXTURES / spec['expressions']).read_text().splitlines()
        relative = Path('sequences') / spec['id'] / str(repetition)
        if row['directory'] != relative.as_posix() or row['group'] != spec['group']:
            raise ValueError('sequence directory/group changed')
        for language in report['languages']:
            dest = original / relative / language
            command = prefix if language == 'java' else [str(binary)]
            checked_command(directory, relative / language, row[language], [*command, str(original_sources / FIXTURES / spec['source']), str(original_sources / FIXTURES / spec['expressions']), str(dest)])
        observed = observe_pair(directory / row['directory'], row, calls, report['languages'])
        if observed != row['observations']:
            raise ValueError('observations differ from raw captures')
    if report['unexecuted_groups'] != manifest['unexecuted_groups']:
        raise ValueError('unexecuted group accounting changed')
    return report


def checked(result):
    if result['status'] != 'executed' or result['exit_code'] != 0:
        raise ValueError('acquisition command failed: ' + repr(result['command']))
    return result


def execute(output, java_only=False):
    output.mkdir(parents=True, exist_ok=False)
    languages = ['java'] if java_only else ['java', 'rust']
    report = {'schema': 1, 'timestamp_utc': datetime.now(timezone.utc).isoformat(),
              'scope': 'bounded ordered class-publication calls; separate from M2 single-expression inventory',
              'languages': languages, 'workspace': str(ROOT), 'output_directory': str(output), 'repetitions': REPETITIONS, 'cases': [], 'sources': [], 'artifacts': [],
              'compatibility_complete': False, 'acquisition_error': None,
              'session_contract': 'fresh evaluator per sequence/repetition; retained module and evaluator across all calls including errors; Rust collects with module root after each call, then closes',
              'comparison_channels': ['status', 'stdout', 'stderr'],
              'limitations': ['property-path scalar adapter; fixture helper members contain the expressions',
                              'Java error class is retained as provenance, not cross-language class-name equality',
                              'GC/Host/terminal architectural tests have a separate denominator',
                              'unexecuted groups remain gaps, not passing language coverage']}
    try:
        for path in source_paths(java_only=java_only):
            data = path.read_bytes(); relative = path.relative_to(ROOT); snapshot = Path('sources') / relative
            dest = output / snapshot; dest.parent.mkdir(parents=True, exist_ok=True); dest.write_bytes(data)
            report['sources'].append({'path': relative.as_posix(), 'snapshot': snapshot.as_posix(),
                                      'sha256': benchmark.digest(data), 'bytes': len(data)})
        manifest = manifest_at(output / 'sources'); report['unexecuted_groups'] = manifest['unexecuted_groups']
        metadata = oracle.ensure_oracle(ROOT, fetch=False); report['oracle'] = metadata
        benchmark.write_json(output / 'oracle.json', metadata)
        classes = output / 'classes'; classes.mkdir()
        report['javac'] = checked(benchmark.capture([str(Path(metadata['java']).with_name('javac')), '-cp', metadata['artifact'],
                                  '-d', str(classes), str(output / 'sources/tools/java/M2ClassLifecycleProbe.java')],
                                 ROOT, metadata['environment'], output / 'javac'))
        artifacts = list(sorted(classes.glob('*.class')))
        binary = None
        if not java_only:
            report['rustc'] = checked(benchmark.capture(['rustc', '-vV'], ROOT, dict(os.environ), output / 'rustc'))
            environment = dict(os.environ, CARGO_TARGET_DIR=str(output / 'build'))
            report['build'] = checked(benchmark.capture(['cargo', 'build', '-p', 'pkl-runtime', '--locked', '--release',
                                      '--example', 'class_lifecycle_probe', '--message-format=json'],
                                      ROOT, environment, output / 'cargo', timeout=240))
            messages = [json.loads(line) for line in benchmark.raw_channel(report['build'], 'stdout').splitlines()]
            executables = [Path(r['executable']) for r in messages if r.get('reason') == 'compiler-artifact'
                           and r.get('executable') and r.get('target', {}).get('name') == 'class_lifecycle_probe']
            if len(executables) != 1 or not executables[0].is_relative_to(output / 'build'):
                raise ValueError('candidate executable provenance missing')
            binary = output / 'binaries/class_lifecycle_probe'; binary.parent.mkdir(); shutil.copy2(executables[0], binary)
            report['candidate_build_sha256'] = oracle.sha256(executables[0])
            artifacts.append(binary)
        report['artifacts'] = [{'path': p.relative_to(output).as_posix(), 'sha256': oracle.sha256(p), 'bytes': p.stat().st_size} for p in artifacts]
        stable = lambda: (all(oracle.sha256(ROOT / r['path']) == r['sha256'] for r in report['sources'])
                          and all(oracle.sha256(output / r['path']) == r['sha256'] for r in report['artifacts']))
        report['source_stability_before_execution'] = stable()
        if not stable():
            raise ValueError('sources changed during build; preserve evidence and rerun')
        cp = metadata['command'].index('-cp')
        prefix = [*metadata['command'][:cp], '-cp', os.pathsep.join((str(classes), metadata['artifact'])), 'M2ClassLifecycleProbe']
        for spec in manifest['sequences']:
            source = output / 'sources' / FIXTURES / spec['source']; expressions = output / 'sources' / FIXTURES / spec['expressions']
            calls = expressions.read_text().splitlines()
            for repetition in range(REPETITIONS):
                relative = Path('sequences') / spec['id'] / str(repetition)
                row = {'id': spec['id'], 'group': spec['group'], 'repetition': repetition, 'directory': relative.as_posix()}
                for language in languages:
                    dest = output / relative / language
                    command = prefix if language == 'java' else [str(binary)]
                    row[language] = benchmark.capture([*command, str(source), str(expressions), str(dest)],
                                                     ROOT, metadata['environment'], dest, timeout=30)
                row['observations'] = observe_pair(output / relative, row, calls, languages)
                report['cases'].append(row)
                print(f"{spec['id']} repetition={repetition}: " + ', '.join(f"{lang}={row['observations']['processes'][lang]['status']}" for lang in languages), flush=True)
        report['source_stability_after_execution'] = stable()
        if not stable():
            raise ValueError('sources changed during execution; preserve evidence and rerun')
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        report['acquisition_error'] = str(error)
        raise
    finally:
        report['evidence_sha256'] = inventory(output)
        benchmark.write_json(output / 'report.json', report)
    return check_report(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--output', type=Path)
    modes.add_argument('--verify-report', type=Path)
    modes.add_argument('--verify-contract', type=Path)
    parser.add_argument('--java-only', action='store_true')
    parser.add_argument('--record-only', action='store_true')
    args = parser.parse_args()
    try:
        report = execute(args.output.resolve(), args.java_only) if args.output else check_report((args.verify_report or args.verify_contract).resolve())
        passing = contract_passes(report)
        comparisons = [call for row in report['cases'] for call in row['observations']['comparisons']]
        print(json.dumps({'sequence_executions': len(report['cases']), 'call_comparisons': len(comparisons),
                          'passing_calls': sum(c['status'] == 'pass' for c in comparisons),
                          'selected_contract_passes': passing, 'unexecuted_groups': report['unexecuted_groups'],
                          'compatibility_complete': False}))
        return 0 if args.verify_report or args.record_only or passing else 1
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        print(f'm2 class lifecycle: {error}', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
