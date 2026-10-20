#!/usr/bin/env python3
"""Raw Java/Rust M2 language evidence; selected probes never close the full gate."""
from __future__ import annotations

import argparse
from collections import Counter
import json
import os
from pathlib import Path
import subprocess
import sys

import m2_manifest as contract
import oracle

ROOT = contract.ROOT


def capture(command, directory, name, environment=None, timeout=120):
    try:
        process = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, timeout=timeout)
        row = dict(command=[str(x) for x in command], exit_code=process.returncode, timed_out=False)
        stdout, stderr = process.stdout, process.stderr
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b'', error.stderr or b''
        row = dict(command=[str(x) for x in command], exit_code=None, timed_out=True)
    for channel, data in [('stdout', stdout), ('stderr', stderr)]:
        (directory / f'{name}.{channel}.bin').write_bytes(data)
        row[channel + '_sha256'] = contract.digest(data)
    (directory / f'{name}.process.json').write_bytes(contract.encoded(row))
    return row


def status(case, java, rust, java_stdout, java_stderr, rust_stdout, rust_stderr, java_outcome):
    expected = 0 if case['expected_outcome'] == 'success' else 1
    if java['timed_out'] or java['exit_code'] != expected:
        return 'oracle-error'
    if expected == 0 and (java_stderr or java_outcome != b'success\n'):
        return 'oracle-error'
    if expected == 1 and (java_stdout or not java_stderr or not valid_exception_outcome(java_outcome)):
        return 'oracle-error'
    if rust['timed_out']:
        return 'candidate-timeout'
    if (java['exit_code'], java_stdout, java_stderr) == (rust['exit_code'], rust_stdout, rust_stderr):
        return 'pass'
    return 'mismatch'


def valid_exception_outcome(outcome):
    prefix = b'pkl-exception\torg.pkl.core.'
    if not outcome.startswith(prefix) or not outcome.endswith(b'\n'):
        return False
    name = outcome[len(prefix):-1]
    return bool(name) and all(ch in b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_$' for ch in name)


def java_case_command(metadata, binary, identifier, source, expression):
    output = binary.parent.parent
    outcome = output / 'cases' / identifier / 'java.outcome.bin'
    base = list(metadata['command'][:-1])
    base[-1] += os.pathsep + str(binary.parent / 'java')
    base.insert(1, '-Dpkl.m2.outcome=' + str(outcome))
    return [*base, 'M2LanguageProbe', str(source), expression]


def capture_java(metadata, binary, identifier, source, expression, directory):
    row = capture(java_case_command(metadata, binary, identifier, source, expression),
                  directory, 'java', metadata['environment'])
    outcome = directory / 'java.outcome.bin'
    if not outcome.exists():
        outcome.write_bytes(b'')
    row['outcome_sha256'] = oracle.sha256(outcome)
    (directory / 'java.process.json').write_bytes(contract.encoded(row))
    return row


def source_paths(root=ROOT):
    paths = set(contract.INPUTS) | {'tools/m2_manifest.py', 'tools/m2_language.py',
            'tools/m2_generate.py', 'tools/m2_identifier_tables.py', 'tools/java/M2IdentifierProbe.java', 'tools/java/M2LanguageProbe.java', str(contract.MANIFEST), 'tools/oracle.py', 'oracle.lock.json',
            'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'}
    for directory, pattern in [('crates', '*.rs'), ('crates', 'Cargo.toml'), ('tests/fixtures/m2', '*')]:
        paths.update(str(p.relative_to(root)) for p in (root / directory).rglob(pattern) if p.is_file())
    return sorted(paths)


def execute(output: Path, candidate: Path | None):
    manifest = contract.current_manifest()
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    output.mkdir(parents=True, exist_ok=False)
    initial = {path: oracle.sha256(ROOT / path) for path in source_paths()}
    for path in initial:
        destination = output / 'sources' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((ROOT / path).read_bytes())
    (output / 'oracle.json').write_bytes(contract.encoded(metadata))
    compiler = capture(['rustc', '--version', '--verbose'], output, 'rustc')
    if compiler['exit_code'] != 0:
        raise ValueError('cannot identify Rust compiler; raw evidence preserved')
    build = None
    if candidate is None:
        build = capture(['cargo', 'build', '-p', 'pkl-runtime', '--example', 'language_probe',
                         '--locked', '--release', '--target-dir', str(ROOT / 'target')],
                        output, 'build', timeout=300)
        if build['exit_code'] != 0:
            raise ValueError('candidate build failed; raw build evidence preserved')
        candidate = ROOT / 'target/release/examples/language_probe'
    if not candidate.is_file():
        raise ValueError('candidate artifact missing')
    artifacts = output / 'artifacts'
    artifacts.mkdir()
    frozen = artifacts / 'language_probe'
    frozen.write_bytes(candidate.read_bytes())
    frozen.chmod(0o755)
    candidate_hash = oracle.sha256(candidate)
    classes = artifacts / 'java'
    classes.mkdir()
    javac = Path(metadata['java']).with_name('javac')
    compile_row = capture([str(javac), '-encoding', 'UTF-8', '-cp', metadata['artifact'],
                           '-d', str(classes), str(ROOT / 'tools/java/M2LanguageProbe.java')],
                          output, 'javac', metadata['environment'])
    if compile_row['exit_code'] != 0:
        raise ValueError('Java oracle compile failed; raw evidence preserved')
    results = []
    selected = [row for row in manifest['cases'] if row.get('adapter') == 'language-api-string']
    for case in selected:
        directory = output / 'cases' / case['id']
        directory.mkdir(parents=True)
        source = ROOT / case['input']
        (directory / 'input.pkl').write_bytes(source.read_bytes())
        arguments = [str(source), case['expression']]
        java = capture_java(metadata, frozen, case['id'], source, case['expression'], directory)
        rust = capture([str(frozen), *arguments], directory, 'rust', metadata['environment'])
        outcome = status(case, java, rust, *( (directory / f'{name}.{channel}.bin').read_bytes()
                                             for name in ('java', 'rust') for channel in ('stdout', 'stderr')),
                         (directory / 'java.outcome.bin').read_bytes())
        result = dict(id=case['id'], status=outcome, java=java, rust=rust)
        if case['id'] == 'error-remainder-zero':
            repeated = directory / 'java-repeat'
            repeated.mkdir()
            result['java_repeat'] = capture_java(metadata, frozen, case['id'] + '/java-repeat',
                                                 source, case['expression'], repeated)
        results.append(result)
        print(f"{case['id']}: {outcome}", flush=True)
    relations = check_relations(manifest, output)
    report = dict(schema_version=2, scope=manifest['comparison_boundary'],
                  inventory_counts=manifest['counts'], selected_count=len(selected),
                  oracle=metadata, candidate=dict(path=str(candidate), sha256=candidate_hash,
                                 source_binding='recorded-build' if build else 'unbound-caller-supplied', build=build),
                  javac=dict(path=str(javac), sha256=oracle.sha256(javac)),
                  source_sha256=initial,
                  source_stability=all((ROOT / path).is_file() and oracle.sha256(ROOT / path) == value
                                       for path, value in initial.items()),
                  artifact_stability=candidate.is_file() and oracle.sha256(candidate) == candidate_hash,
                  results=results, metamorphic_results=relations,
                  status_counts=dict(Counter(row['status'] for row in results)),
                  full_gate_failures=contract.full_gate_failures(manifest, results))
    report['evidence_sha256'] = {str(path.relative_to(output)): oracle.sha256(path)
                                 for path in sorted(output.rglob('*')) if path.is_file()}
    (output / 'report.json').write_bytes(contract.encoded(report))
    check_report(output)
    return report


def check_relations(manifest, directory):
    relations = {}
    for row in manifest['cases']:
        if 'metamorphic_relation' in row:
            relation = row['metamorphic_relation']
            relations[tuple(relation['case_ids'])] = relation
    results = []
    for ids, relation in sorted(relations.items()):
        statuses = {}
        for runtime in ('java', 'rust'):
            captures = []
            for identifier in ids:
                case = directory / 'cases' / identifier
                process = json.loads((case / f'{runtime}.process.json').read_bytes())
                captures.append((process['exit_code'], process['timed_out'],
                                 (case / f'{runtime}.stdout.bin').read_bytes(),
                                 (case / f'{runtime}.stderr.bin').read_bytes()))
            statuses[runtime] = all(row[0:2] == (0, False) and row[3] == b'' for row in captures) and len(set(captures)) == 1
        results.append(dict(**relation, runtimes=statuses, status='pass' if all(statuses.values()) else 'mismatch'))
    return results


def verify_oracle(directory: Path, report):
    """Recheck immutable baseline, JVM options and environment from snapshots."""
    metadata = report['oracle']
    lock_path = directory / 'sources/oracle.lock.json'
    locked = json.loads(lock_path.read_bytes())
    refs = oracle.read_properties((directory / 'sources/references.lock').read_text())
    if json.loads((directory / 'oracle.json').read_bytes()) != metadata:
        raise ValueError('oracle metadata differs from raw record')
    if (metadata['profile'] != 'reference' or metadata['version'] != locked['version']
            or metadata['lock_sha256'] != oracle.sha256(lock_path)
            or metadata['artifact_sha256'] != locked['artifact']['sha256']
            or metadata['artifact_url'] != locked['artifact']['url']
            or metadata['source'] != locked['source']
            or refs['baseline.pkl'] != locked['version']
            or refs['pkl.revision'] != locked['source']['revision']):
        raise ValueError('oracle baseline provenance differs from pins')
    environment = metadata['environment']
    if (set(environment) != {'PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LC_ALL', 'LANG', 'TZ', 'TERM', 'NO_COLOR', 'COLUMNS'}
            or environment['LC_ALL'] != 'C' or environment['LANG'] != 'C'
            or environment['TZ'] != 'UTC' or environment['TERM'] != 'dumb'
            or environment['NO_COLOR'] != '1' or environment['COLUMNS'] != '80'
            or not environment['TMPDIR'] == environment['TMP'] == environment['TEMP']):
        raise ValueError('oracle environment differs from controlled contract')
    options = [*locked['jvm_options'], '-Duser.home=' + environment['HOME'],
               '-Djava.io.tmpdir=' + environment['TMPDIR']]
    if metadata['command'] != [metadata['java'], *options, '-cp', metadata['artifact'], 'org.pkl.cli.Main']:
        raise ValueError('oracle JVM options or classpath differ from pins')
    for key, value in locked['jdk']['properties'].items():
        if metadata['jdk']['properties'].get(key) != value:
            raise ValueError('oracle JDK runtime differs from pins')
    for key in ('executable_sha256', 'runtime_modules_sha256', 'release_sha256'):
        value = metadata['jdk'].get(key, '')
        if len(value) != 64 or any(ch not in '0123456789abcdef' for ch in value):
            raise ValueError('missing exact JDK identity hash')


def check_report(directory: Path):
    report = json.loads((directory / 'report.json').read_bytes())
    manifest = contract.current_manifest()
    verify_oracle(directory, report)
    if not report['source_stability'] or not report['artifact_stability']:
        raise ValueError('sources or artifact changed during evidence acquisition')
    inventory = report['evidence_sha256']
    observed = {str(p.relative_to(directory)) for p in directory.rglob('*')
                if p.is_file() and p != directory / 'report.json'}
    if not inventory or set(inventory) != observed:
        raise ValueError('evidence file coverage incomplete')
    for relative, expected in inventory.items():
        if oracle.sha256(directory / relative) != expected:
            raise ValueError('evidence bytes changed: ' + relative)
    if set(report['source_sha256']) != set(source_paths(directory / 'sources')):
        raise ValueError('source provenance coverage incomplete')
    for path, expected in report['source_sha256'].items():
        if oracle.sha256(directory / 'sources' / path) != expected:
            raise ValueError('source snapshot changed: ' + path)
    if (directory / 'sources' / contract.MANIFEST).read_bytes() != contract.encoded(manifest):
        raise ValueError('recorded M2 manifest differs from current required contract')
    if oracle.sha256(directory / 'artifacts/language_probe') != report['candidate']['sha256']:
        raise ValueError('candidate artifact identity differs')
    binding = report['candidate'].get('source_binding')
    if binding == 'recorded-build':
        build = json.loads((directory / 'build.process.json').read_bytes())
        if build != report['candidate']['build'] or build['exit_code'] != 0 or build['timed_out']:
            raise ValueError('candidate recorded build differs or failed')
    elif binding != 'unbound-caller-supplied' or report['candidate'].get('build') is not None:
        raise ValueError('invalid source/artifact binding claim')
    selected = {row['id']: row for row in manifest['cases'] if row.get('adapter') == 'language-api-string'}
    ids = [row['id'] for row in report['results']]
    if set(ids) != set(selected) or len(ids) != len(selected) or report['selected_count'] != len(selected):
        raise ValueError('selected execution coverage incomplete or duplicate')
    for row in report['results']:
        case = selected[row['id']]
        folder = directory / 'cases' / row['id']
        if oracle.sha256(folder / 'input.pkl') != case['input_sha256']:
            raise ValueError('case input differs: ' + row['id'])
        for runtime in ('java', 'rust'):
            process = json.loads((folder / f'{runtime}.process.json').read_bytes())
            if process != row[runtime]:
                raise ValueError('process metadata differs: ' + row['id'])
            if process['command'][-2:] != [str(ROOT / case['input']), case['expression']]:
                raise ValueError('oracle/candidate input URI or expression differs: ' + row['id'])
            for channel in ('stdout', 'stderr'):
                if oracle.sha256(folder / f'{runtime}.{channel}.bin') != process[channel + '_sha256']:
                    raise ValueError('process raw hash differs: ' + row['id'])
        if row['java']['command'] != java_case_command(report['oracle'], Path(row['rust']['command'][0]),
                case['id'], ROOT / case['input'], case['expression']):
            raise ValueError('case Java command changes pinned oracle options/classpath')
        if oracle.sha256(folder / 'java.outcome.bin') != row['java']['outcome_sha256']:
            raise ValueError('oracle outcome sidecar differs')
        if row['id'] == 'error-remainder-zero':
            repeated = folder / 'java-repeat'
            process = json.loads((repeated / 'java.process.json').read_bytes())
            if process != row.get('java_repeat'):
                raise ValueError('independent remainder Java capture metadata differs')
            if process['command'] != java_case_command(report['oracle'], Path(row['rust']['command'][0]),
                    case['id'] + '/java-repeat', ROOT / case['input'], case['expression']):
                raise ValueError('independent remainder Java command differs')
            for channel in ('stdout', 'stderr', 'outcome'):
                if oracle.sha256(repeated / f'java.{channel}.bin') != process[channel + '_sha256']:
                    raise ValueError('independent remainder Java raw capture differs')
        outcome = status(case, row['java'], row['rust'], *((folder / f'{name}.{channel}.bin').read_bytes()
                                    for name in ('java', 'rust') for channel in ('stdout', 'stderr')),
                         (folder / 'java.outcome.bin').read_bytes())
        if row['status'] != outcome:
            raise ValueError('raw comparison status differs: ' + row['id'])
    if report['metamorphic_results'] != check_relations(manifest, directory):
        raise ValueError('metamorphic results differ')
    if report['status_counts'] != dict(Counter(row['status'] for row in report['results'])):
        raise ValueError('comparison counts differ')
    if report['full_gate_failures'] != contract.full_gate_failures(manifest, report['results']):
        raise ValueError('full gate accounting differs')
    return report


def subset_passes(report):
    return all(row['status'] == 'pass' for row in report['results']) and all(
        row['status'] == 'pass' for row in report['metamorphic_results'])


def remainder_development_exception(row, directory: Path):
    """Known Pkl 0.32 remainder bug, retained as a failed compatibility case."""
    if row['status'] != 'mismatch' or 'java_repeat' not in row:
        return False
    folder = directory / 'cases/error-remainder-zero'
    if (folder / 'input.pkl').read_bytes() != b'result = 1 % 0\n':
        return False
    for runtime, process in [('java', row['java']), ('rust', row['rust']),
                              ('java-repeat/java', row['java_repeat'])]:
        if process['exit_code'] != 1 or process['timed_out'] or (folder / f'{runtime}.stdout.bin').read_bytes():
            return False
    outcome = b'pkl-exception\torg.pkl.core.PklBugException\n'
    if any((folder / prefix / 'java.outcome.bin').read_bytes() != outcome
           for prefix in ('', 'java-repeat')):
        return False
    java = (folder / 'java.stderr.bin').read_bytes()
    repeated = (folder / 'java-repeat/java.stderr.bin').read_bytes()
    if java != repeated:
        return False
    banner = (b'An unexpected error has occurred. Would you mind filing a bug report?\n')
    # This class/cause/origin check identifies the explicitly investigated bug;
    # compatibility still compares the entire Java/Rust payloads and remains failed.
    if (not java.startswith(banner)
            or b'java.lang.ArithmeticException%3A%20%2F%20by%20zero\n' not in java
            or b'java.lang.ArithmeticException: / by zero\n' not in java
            or b'\tat org.pkl.core.runtime.VmSafeMath.remainder(VmSafeMath.java:93)\n' not in java):
        return False
    source = Path(row['java']['command'][-2])
    if source != ROOT / 'tests/fixtures/m2/error-remainder-zero.pkl':
        return False
    expected = ('–– Pkl Error ––\nDivision by zero.\n\n1 | result = 1 % 0\n'
                '             ^^^^^\nat error-remainder-zero#result (' + source.as_uri() + ')\n'
                '\n1 | result\n    ^^^^^^\nat  (repl:text)\n').encode()
    return (folder / 'rust.stderr.bin').read_bytes() == expected


def development_passes(report, directory: Path):
    """Temporary development gate; retains explicit cycle/remainder failures as failed.

    This is not a compatibility gate. Strict subset/full verification continues
    to reject the exact same report while this known exception is present.
    """
    if not report['results'] or any(row['status'] != 'pass' for row in report['metamorphic_results']):
        return False
    for row in report['results']:
        if row['status'] == 'pass':
            continue
        if row['id'] == 'error-remainder-zero':
            if not remainder_development_exception(row, directory):
                return False
            continue
        if row['id'] != 'error-cycle' or row['status'] not in ('oracle-error', 'mismatch'):
            return False
        folder = directory / 'cases/error-cycle'
        if (folder / 'input.pkl').read_bytes() != b'a = b\nb = a\nresult = a\n':
            return False
        for runtime in ('java', 'rust'):
            process = row[runtime]
            if process['exit_code'] != 1 or process['timed_out'] or (folder / f'{runtime}.stdout.bin').read_bytes():
                return False
        java = (folder / 'java.stderr.bin').read_bytes()
        rust = (folder / 'rust.stderr.bin').read_bytes()
        headers = [b'Exception in thread "main" java.lang.NoClassDefFoundError: Could not initialize class org.pkl.core.runtime.VmStackOverflowException\n',
                   '–– Pkl Error ––\nA stack overflow occurred.\n'.encode()]
        if not any(java.startswith(header) for header in headers):
            return False
        if not rust.startswith('–– Pkl Error ––\ncyclic forcing of a\n'.encode()):
            return False
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument('--output', type=Path)
    action.add_argument('--verify-report', type=Path, help='integrity only; preserves recorded compatibility failures')
    action.add_argument('--verify-subset', type=Path, help='integrity and every selected probe must pass')
    action.add_argument('--verify-full', type=Path, help='strict full M2 language completion')
    action.add_argument('--verify-development', type=Path, help='development-only gate allowing explicitly recorded cycle/remainder failures; does not establish compatibility')
    parser.add_argument('--record-only', action='store_true', help='with --output, acquire and validate evidence without requiring compatibility success')
    parser.add_argument('--candidate', type=Path, help='override the default recorded release build; caller-supplied artifact has no verified source binding')
    args = parser.parse_args()
    if args.record_only and not args.output:
        parser.error('--record-only requires --output')
    report = execute(args.output.resolve(), args.candidate.resolve() if args.candidate else None) if args.output else check_report(
        (args.verify_report or args.verify_subset or args.verify_full or args.verify_development).resolve())
    print(json.dumps(dict(selected=report['status_counts'], full_gate_failures=len(report['full_gate_failures'])), sort_keys=True))
    if args.verify_report or args.record_only:
        return 0
    if args.verify_development:
        return int(not development_passes(report, args.verify_development.resolve()))
    return int(not subset_passes(report) or bool(args.verify_full and (report['full_gate_failures']
                     or report['candidate']['source_binding'] != 'recorded-build')))


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f'M2 language error: {error}', file=sys.stderr)
        sys.exit(1)
