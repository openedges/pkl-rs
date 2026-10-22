#!/usr/bin/env python3
"""Capture complete source-driven trace protocols from Java and native Rust."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import sys

import benchmark
import oracle

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def load_manifest(path):
    manifest = json.loads(path.read_bytes())
    if manifest.get('schema') != 1 or manifest.get('repetitions') != 2:
        raise ValueError('expected schema 1 and two fresh repetitions')
    if manifest.get('uri') != 'file:///stage37-trace.pkl':
        raise ValueError('Java probe requires the fixed trace source URI')
    seen = set()
    for case in manifest['cases']:
        if not re.fullmatch(r'[a-z0-9-]+', case['id']) or case['id'] in seen:
            raise ValueError('invalid or duplicate case identifier')
        seen.add(case['id'])
        if case['mode'] not in {'COMPACT', 'PRETTY'} or case['action'] not in {'capture', 'noop', 'throw', 'close', 'close-reenter'}:
            raise ValueError('unsupported trace driver configuration')
        if not case['expressions'] or any(not isinstance(expr, str) or not expr or '\n' in expr or '\r' in expr for expr in case['expressions']):
            raise ValueError('expected nonempty one-line expressions')
        for expression in [*case['expressions'], case['logger_callback'], case['reader_callback']]:
            if expression and not re.fullmatch(r'[^\W\d]\w*(?:\.[^\W\d]\w*)*', expression):
                raise ValueError('development source session accepts property paths only')
        source = (ROOT / case['source']).resolve()
        if not source.is_relative_to(ROOT / 'tests/fixtures/m2-trace') or not source.is_file():
            raise ValueError('source must be an existing trace fixture')
        if sha(source) != case['origin']['input_sha256']:
            raise ValueError('original trace input changed: ' + case['id'])
    if not seen:
        raise ValueError('empty trace inventory')
    return manifest


def payloads(directory):
    # JVM class names are retained as provenance, not Rust class-name expectations.
    return {path.name: path for path in sorted(directory.glob('*.bin'))
            if not path.name.endswith('.error-class.bin')}


def compare_payloads(expected, actual):
    expected_files, actual_files = payloads(expected), payloads(actual)
    rows = []
    for name in sorted(expected_files.keys() | actual_files.keys()):
        left, right = expected_files.get(name), actual_files.get(name)
        rows.append(dict(name=name, expected_sha256=sha(left) if left else None,
                         actual_sha256=sha(right) if right else None,
                         raw_equal=left is not None and right is not None and left.read_bytes() == right.read_bytes()))
    exact_coverage = expected_files.keys() == actual_files.keys()
    oracle_errors = [name for name, path in expected_files.items()
                     if name.endswith('.status.bin') and path.read_bytes() == b'oracle-error\n']
    return dict(exact_coverage=exact_coverage, oracle_errors=oracle_errors, comparisons=rows,
                raw_equal=bool(rows) and exact_coverage and all(row['raw_equal'] for row in rows))


def missing_calls(directory, expressions):
    missing = []
    for index in range(len(expressions)):
        for channel in ['status', 'stdout', 'stderr']:
            name = f'outer-{index}.{channel}.bin'
            if not (directory / name).is_file():
                missing.append(name)
    for path in directory.glob('*.status.bin'):
        if path.read_bytes() not in {b'success\n', b'error\n', b'oracle-error\n'}:
            missing.append(path.name + ':invalid-status')
        for channel in ['stdout', 'stderr']:
            name = path.name.removesuffix('.status.bin') + '.' + channel + '.bin'
            if not (directory / name).is_file():
                missing.append(name)
    return sorted(set(missing))


def snapshot(output, paths):
    records = {}
    for source in sorted(paths):
        relative = source.relative_to(ROOT)
        destination = output / 'sources' / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        records[str(relative)] = sha(destination)
    return records


def inventory(output):
    files = {}
    for folder, directories, names in os.walk(output):
        # Cargo cache is excluded before traversal, including offline audits.
        directories[:] = sorted(name for name in directories if Path(folder) / name != output / 'build')
        for name in sorted(names):
            path = Path(folder) / name
            if path == output / 'report.json':
                continue
            if path.is_symlink() or not path.is_file():
                raise ValueError('unexpected evidence entry: ' + str(path))
            files[str(path.relative_to(output))] = sha(path)
    return files


def verify_report(output):
    report = json.loads((output / 'report.json').read_bytes())
    if report.get('schema') != 1 or report['files'] != inventory(output):
        raise ValueError('trace evidence inventory changed')
    captured_manifest = output / 'sources' / report['manifest_path']
    if sha(captured_manifest) != report['manifest_sha256']:
        raise ValueError('trace manifest identity changed')
    manifest = json.loads(captured_manifest.read_bytes())
    selected_ids = [case['id'] for case in report['cases']]
    expected_cases = [case for case in manifest['cases'] if case['id'] in selected_ids]
    if report['cases'] != expected_cases or report['repetitions'] != manifest['repetitions']:
        raise ValueError('trace selection differs from captured manifest')
    if report['oracle'] != json.loads((output / 'oracle.json').read_bytes()):
        raise ValueError('trace oracle provenance changed')
    if report['commands'] != json.loads((output / 'commands.json').read_bytes()):
        raise ValueError('trace command coverage changed')
    binding = report['candidate']['source_binding']
    if binding not in {'recorded-build', 'supplied-artifact-only'}:
        raise ValueError('unknown trace artifact provenance')
    expected_commands = ['build-native', 'javac'] if binding == 'recorded-build' else ['javac']
    if [command['name'] for command in report['commands']] != expected_commands:
        raise ValueError('trace build command coverage changed')
    expected_pairs = [(case['id'], repeat) for case in report['cases'] for repeat in range(report['repetitions'])]
    if [(row['id'], row['repeat']) for row in report['results']] != expected_pairs:
        raise ValueError('trace execution coverage changed')
    if report['results'] != json.loads((output / 'partial-results.json').read_bytes()):
        raise ValueError('trace execution records differ from captured results')
    captured_sources = {name.removeprefix('sources/') for name in report['files']
                        if name.startswith('sources/')}
    if set(report['sources']) != captured_sources or set(report['sources_after']) != captured_sources:
        raise ValueError('trace source snapshot coverage changed')
    for name, digest in report['sources'].items():
        if sha(output / 'sources' / name) != digest:
            raise ValueError('trace source snapshot changed')
    if sha(output / 'artifacts/trace_host_probe') != report['candidate']['sha256']:
        raise ValueError('native artifact changed')
    for row in report['results']:
        if set(row['processes']) != {'java', 'rust'}:
            raise ValueError('trace execution requires both Java and Rust process provenance')
        case = next(case for case in report['cases'] if case['id'] == row['id'])
        base = output / 'cases' / row['id'] / str(row['repeat'])
        comparison = compare_payloads(base / 'java', base / 'rust')
        if any(row[key] != value for key, value in comparison.items()):
            raise ValueError('raw trace comparison changed')
        missing = {target: missing_calls(base / target, case['expressions']) for target in ['java', 'rust']}
        if missing != row['missing_calls']:
            raise ValueError('trace call coverage changed')
        for target, process in row['processes'].items():
            directory = base / target
            if json.loads((directory / 'process/result.json').read_bytes()) != process:
                raise ValueError('trace process provenance changed')
            if sha(directory / 'input.pkl') != case['origin']['input_sha256']:
                raise ValueError('trace source input changed')
            if (directory / 'expressions.txt').read_bytes() != ('\n'.join(case['expressions']) + '\n').encode():
                raise ValueError('trace expression input changed')
            for callback in ['logger', 'reader']:
                if (directory / (callback + '-callback.txt')).read_bytes() != case[callback + '_callback'].encode():
                    raise ValueError('trace callback input changed')
            for channel in ['stdout', 'stderr']:
                path = directory / 'process' / (channel + '.bin')
                if sha(path) != process[channel]['sha256'] or path.stat().st_size != process[channel]['bytes']:
                    raise ValueError('trace process capture changed')
            if (directory / 'protocol.bin').read_bytes() != (directory / 'process/stdout.bin').read_bytes():
                raise ValueError('trace protocol differs from actual process stdout')
            if (directory / 'process-stderr.bin').read_bytes() != (directory / 'process/stderr.bin').read_bytes():
                raise ValueError('trace stderr differs from actual process stderr')
        executed = not any(missing.values()) and all(p['status'] == 'executed' and p['exit_code'] == 0 for p in row['processes'].values())
        status = ('acquisition-failed' if not executed else 'oracle-error' if comparison['oracle_errors']
                  else 'pass' if comparison['raw_equal'] else 'mismatch')
        if row['status'] != status:
            raise ValueError('trace status does not follow complete observations')
    if report['status_counts'] != dict(Counter(row['status'] for row in report['results'])):
        raise ValueError('trace result counts changed')
    for command in report['commands']:
        process = json.loads((output / 'processes' / command['name'] / 'result.json').read_bytes())
        if any(command[key] != value for key, value in process.items()):
            raise ValueError('trace build provenance changed')
    stability = report['sources'] == report['sources_after']
    complete_selection = report['cases'] == manifest['cases']
    all_selected_match = bool(report['results']) and all(row['status'] == 'pass' for row in report['results'])
    artifact_stability = report['candidate']['sha256'] == report['artifact_after_sha256']
    bound_build = report['candidate']['source_binding'] == 'recorded-build'
    if bound_build and not any(command['name'] == 'build-native' and command['status'] == 'executed'
                              and command['exit_code'] == 0 for command in report['commands']):
        raise ValueError('trace candidate lacks its claimed build')
    derived = dict(source_stability=stability, complete_selection=complete_selection,
        artifact_stability=artifact_stability, all_selected_match=all_selected_match,
        compatibility_complete=complete_selection and stability and artifact_stability and bound_build and all_selected_match)
    if any(report[key] != value for key, value in derived.items()):
        raise ValueError('trace completion claim exceeds its recorded evidence')
    return report


def run(output, manifest_path, candidate, selected):
    manifest = load_manifest(manifest_path)
    selected_ids = set(selected)
    if selected_ids - {case['id'] for case in manifest['cases']}:
        raise ValueError('unknown selected trace case')
    cases = [case for case in manifest['cases'] if not selected_ids or case['id'] in selected_ids]
    output.mkdir(parents=True, exist_ok=False)
    paths = {ROOT / name for name in ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'references.lock',
        'tools/m2_trace.py', 'tools/benchmark.py', 'tools/oracle.py', 'tools/java/M2TraceProbe.java',
        'crates/pkl-runtime/Cargo.toml', 'crates/pkl-runtime/examples/trace_host_probe.rs']}
    paths.add(manifest_path)
    paths.update((ROOT / 'crates/pkl-runtime/src').rglob('*.rs'))
    paths.update(ROOT / case['source'] for case in manifest['cases'])
    sources = snapshot(output, paths)
    meta = oracle.ensure_oracle(ROOT, fetch=False)
    benchmark.write_json(output / 'oracle.json', meta)
    commands = []

    def execute(name, command, timeout=180, build_environment=False):
        environment = dict(os.environ) if build_environment else meta['environment']
        process = benchmark.capture(command, ROOT, environment, output / 'processes' / name, timeout=timeout)
        commands.append(dict(name=name, environment_profile='inherited-build' if build_environment else 'pinned-oracle', **process))
        benchmark.write_json(output / 'commands.json', commands)
        return process

    binding = 'supplied-artifact-only'
    if candidate is None:
        build = execute('build-native', ['cargo', 'build', '-p', 'pkl-runtime', '--example', 'trace_host_probe',
            '--release', '--locked', '--target-dir', str(output / 'build')], timeout=600, build_environment=True)
        if build['exit_code'] != 0 or build['status'] != 'executed':
            raise RuntimeError('native build failed; process captures retained')
        candidate = output / 'build/release/examples/trace_host_probe'
        binding = 'recorded-build'
    artifact = output / 'artifacts/trace_host_probe'
    artifact.parent.mkdir()
    shutil.copy2(candidate, artifact)
    candidate_hash = sha(artifact)
    classes = output / 'classes'
    classes.mkdir()
    compile_java = execute('javac', [str(Path(meta['java']).with_name('javac')), '-cp', meta['artifact'],
        '-d', str(classes), str(ROOT / 'tools/java/M2TraceProbe.java')])
    if compile_java['exit_code'] != 0 or compile_java['status'] != 'executed':
        raise RuntimeError('Java probe build failed; process captures retained')
    index = meta['command'].index('-cp')
    java = [*meta['command'][:index], '-cp', str(classes) + os.pathsep + meta['artifact'], 'M2TraceProbe']
    rows = []
    for case in cases:
        for repeat in range(manifest['repetitions']):
            directories = {}
            processes = {}
            for target in ['java', 'rust']:
                directory = output / 'cases' / case['id'] / str(repeat) / target
                directories[target] = directory
                directory.mkdir(parents=True)
                shutil.copyfile(ROOT / case['source'], directory / 'input.pkl')
                (directory / 'expressions.txt').write_bytes(('\n'.join(case['expressions']) + '\n').encode())
                for name in ['logger', 'reader']:
                    (directory / (name + '-callback.txt')).write_bytes(case[name + '_callback'].encode())
                command = [*(java if target == 'java' else [str(artifact)]), str(directory), case['mode'], case['action']]
                process = benchmark.capture(command, ROOT, meta['environment'], directory / 'process', timeout=30)
                processes[target] = process
                (directory / 'protocol.bin').write_bytes(Path(process['stdout']['path']).read_bytes())
                (directory / 'process-stderr.bin').write_bytes(Path(process['stderr']['path']).read_bytes())
            comparison = compare_payloads(directories['java'], directories['rust'])
            missing = {target: missing_calls(directory, case['expressions']) for target, directory in directories.items()}
            executed = not any(missing.values()) and all(process['status'] == 'executed' and process['exit_code'] == 0 for process in processes.values())
            status = ('acquisition-failed' if not executed else 'oracle-error' if comparison['oracle_errors']
                      else 'pass' if comparison['raw_equal'] else 'mismatch')
            rows.append(dict(id=case['id'], repeat=repeat, status=status, processes=processes, missing_calls=missing, **comparison))
            benchmark.write_json(output / 'partial-results.json', rows)
            print(case['id'], repeat, status, flush=True)
    sources_after = {name: sha(ROOT / name) for name in sources}
    source_stability = sources == sources_after
    artifact_after = sha(artifact)
    report = dict(schema=1, scope=manifest['scope'], host=platform.platform(), cases=cases,
        manifest_path=str(manifest_path.relative_to(ROOT)), manifest_sha256=sha(manifest_path), complete_selection=len(cases) == len(manifest['cases']),
        repetitions=manifest['repetitions'], sources=sources, sources_after=sources_after, source_stability=source_stability,
        candidate=dict(path=str(candidate), sha256=candidate_hash, source_binding=binding),
        artifact_stability=artifact_after == candidate_hash, artifact_after_sha256=artifact_after, oracle=meta, commands=commands,
        results=rows, status_counts=dict(Counter(row['status'] for row in rows)),
        all_selected_match=all(row['status'] == 'pass' for row in rows),
        compatibility_complete=len(cases) == len(manifest['cases']) and source_stability and artifact_after == candidate_hash
            and binding == 'recorded-build' and all(row['status'] == 'pass' for row in rows),
        excluded_before_traversal=['build'])
    report['files'] = inventory(output)
    benchmark.write_json(output / 'report.json', report)
    verify_report(output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--output', type=Path)
    modes.add_argument('--verify-report', type=Path)
    modes.add_argument('--verify-contract', type=Path)
    parser.add_argument('--manifest', type=Path, default=ROOT / 'compatibility/m2-trace.json')
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--case', action='append', default=[])
    parser.add_argument('--record-only', action='store_true')
    args = parser.parse_args()
    if args.verify_report or args.verify_contract:
        report = verify_report((args.verify_report or args.verify_contract).resolve())
        print(json.dumps({key:report[key] for key in ['status_counts', 'source_stability', 'compatibility_complete']}))
        return int(bool(args.verify_contract) and not report['compatibility_complete'])
    report = run(args.output.resolve(), args.manifest.resolve(), args.candidate.resolve() if args.candidate else None, args.case)
    print(json.dumps({name: report[name] for name in ['status_counts', 'source_stability', 'artifact_stability', 'compatibility_complete']}))
    return 0 if args.record_only or (report['compatibility_complete'] and report['source_stability'] and report['artifact_stability']) else 1


if __name__ == '__main__':
    sys.exit(main())
