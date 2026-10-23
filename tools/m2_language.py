#!/usr/bin/env python3
"""Raw Java/Rust M2 language evidence; selected probes never close the full gate."""
from __future__ import annotations

import argparse
from collections import Counter
from concurrent.futures import ThreadPoolExecutor
import json
import os
from pathlib import Path
import subprocess
import sys

import m2_manifest as contract
import m2_module_eval
import m2_jvm_classpath
import m2_source_inputs
import compat
import oracle

ROOT = contract.ROOT
MAX_WORKERS = 32


def validate_workers(workers):
    if type(workers) is not int or not 1 <= workers <= MAX_WORKERS:
        raise ValueError(f'workers must be an integer between 1 and {MAX_WORKERS}')
    return workers


def worker_argument(value):
    try:
        return validate_workers(int(value))
    except ValueError as error:
        raise argparse.ArgumentTypeError(f'workers must be an integer between 1 and {MAX_WORKERS}') from error


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


def expression_arguments(source, expression, power_assertions=False):
    if not isinstance(power_assertions, bool):
        raise ValueError('power_assertions must be a boolean')
    return [str(source), expression, *(['--power-assertions'] if power_assertions else [])]


def java_case_command(metadata, binary, identifier, source, expression, power_assertions=False):
    output = binary.parent.parent
    outcome = output / 'cases' / identifier / 'java.outcome.bin'
    base = list(metadata['command'][:-1])
    base[-1] += os.pathsep + str(binary.parent / 'java')
    base.insert(1, '-Dpkl.m2.outcome=' + str(outcome))
    return [*base, 'M2LanguageProbe', *expression_arguments(source, expression, power_assertions)]


def capture_java(metadata, binary, identifier, source, expression, directory, power_assertions=False):
    row = capture(java_case_command(metadata, binary, identifier, source, expression, power_assertions),
                  directory, 'java', metadata['environment'])
    outcome = directory / 'java.outcome.bin'
    if not outcome.exists():
        outcome.write_bytes(b'')
    row['outcome_sha256'] = oracle.sha256(outcome)
    (directory / 'java.process.json').write_bytes(contract.encoded(row))
    return row


def source_paths(root=ROOT):
    return m2_source_inputs.source_paths(root)


def evidence_paths(directory):
    """Enumerate actual evidence only; a child Cargo cache is never traversed."""
    for current, directories, names in os.walk(directory, followlinks=False):
        for name in directories:
            path = Path(current) / name
            if path in (directory / 'build', directory / 'jvm-classpath/build'):
                raise ValueError('Cargo build cache must stay outside the full language evidence')
            if path.is_symlink():
                raise ValueError('evidence directory symlink is not permitted')
        for name in names:
            path = Path(current) / name
            if path.is_symlink() or not path.is_file():
                raise ValueError('evidence must contain regular files')
            if path != directory / 'report.json':
                yield path


def classpath_result(case, directory, child):
    failures = m2_jvm_classpath.contract_failures(directory, child)
    outcome = next((row['status'] for row in child['results'] if row['status'] != 'pass'), 'mismatch') if failures else 'pass'
    return dict(id=case['id'], adapter=m2_jvm_classpath.ADAPTER, directory='jvm-classpath',
                execution_profile=case['execution_profile'], status=outcome,
                child_report_sha256=oracle.sha256(directory / 'report.json'))


def verify_classpath_child(directory, report):
    context = report['classpath_eval']
    recorded_parent = Path(context['parent_output'])
    if (set(context) != {'directory', 'workspace', 'parent_output', 'child_report_sha256'}
            or context['directory'] != 'jvm-classpath' or not recorded_parent.is_absolute()
            or compat.overlap(Path(context['workspace']), recorded_parent)):
        raise ValueError('JVM-classpath child/workspace boundary differs')
    child_directory = directory / context['directory']
    if oracle.sha256(child_directory / 'report.json') != context['child_report_sha256']:
        raise ValueError('JVM-classpath child report changed')
    child = m2_jvm_classpath.verify_report(child_directory)
    candidate = report['classpath_candidate']
    if (child['workspace'] != context['workspace'] or child['oracle'] != report['oracle']
            or child['output'] != str(recorded_parent / 'jvm-classpath')
            or child['candidate']['source_binding'] != 'unbound-caller-supplied'
            or child['candidate']['original_path'] != str(recorded_parent / 'artifacts/jvm_snippet_probe')
            or child['candidate']['sha256'] != candidate['sha256']
            or child['sources'] != report['source_sha256']):
        raise ValueError('JVM-classpath child lacks exact parent artifact/source/oracle binding')
    return child


def acquire_case(case, output, metadata, frozen):
    directory = output / 'cases' / case['id']
    directory.mkdir(parents=True)
    source = ROOT / case['input']
    (directory / 'input.pkl').write_bytes(source.read_bytes())
    arguments = expression_arguments(source, case['expression'], case.get('power_assertions', False))
    java = capture_java(metadata, frozen, case['id'], source, case['expression'], directory,
                        case.get('power_assertions', False))
    rust = capture([str(frozen), *arguments], directory, 'rust', metadata['environment'])
    outcome = status(case, java, rust, *((directory / f'{name}.{channel}.bin').read_bytes()
                                         for name in ('java', 'rust') for channel in ('stdout', 'stderr')),
                     (directory / 'java.outcome.bin').read_bytes())
    result = dict(id=case['id'], status=outcome, java=java, rust=rust)
    if case['id'] == 'error-remainder-zero':
        repeated = directory / 'java-repeat'
        repeated.mkdir()
        result['java_repeat'] = capture_java(metadata, frozen, case['id'] + '/java-repeat',
                                             source, case['expression'], repeated,
                                             case.get('power_assertions', False))
    return result


def acquire_cases(selected, output, metadata, frozen, workers=1):
    validate_workers(workers)
    identifiers = [case['id'] for case in selected]
    if len(set(identifiers)) != len(identifiers):
        raise ValueError('duplicate case capture directory')

    def acquire(case):
        return acquire_case(case, output, metadata, frozen)

    results = []
    # map yields in manifest order even when processes finish out of order.
    with ThreadPoolExecutor(max_workers=workers) as executor:
        for result in executor.map(acquire, selected):
            results.append(result)
            print(f"{result['id']}: {result['status']}", flush=True)
    return results


def selected_cases(manifest, adapters=None):
    known = ('language-api-string', m2_module_eval.ADAPTER, m2_jvm_classpath.ADAPTER)
    if adapters is not None and (not adapters or len(set(adapters)) != len(adapters)
                                 or not set(adapters) <= set(known)):
        raise ValueError('invalid or duplicate selected adapters')
    return [row for row in manifest['cases'] if row.get('adapter') in (adapters or known)]


def execute(output: Path, candidate: Path | None, workers=1, *, adapters=None,
            module_candidate=None, module_workspace=None, classpath_candidate=None, classpath_workspace=None):
    validate_workers(workers)
    manifest = contract.current_manifest()
    selected = selected_cases(manifest, adapters)
    if not selected:
        raise ValueError('empty executable adapter selection')
    scalars = [row for row in selected if row['adapter'] == 'language-api-string']
    modules = [row for row in selected if row['adapter'] == m2_module_eval.ADAPTER]
    classpath = [row for row in selected if row['adapter'] == m2_jvm_classpath.ADAPTER]
    if (candidate is not None and not scalars) or ((module_candidate is not None or module_workspace is not None) and not modules):
        raise ValueError('candidate/workspace supplied for an unselected adapter')
    if (classpath_candidate is not None or classpath_workspace is not None) and not classpath:
        raise ValueError('classpath candidate/workspace supplied for an unselected adapter')
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    output.mkdir(parents=True, exist_ok=False)
    selection = dict(adapters=list(adapters) if adapters else ['language-api-string', m2_module_eval.ADAPTER, m2_jvm_classpath.ADAPTER],
                     case_ids=[row['id'] for row in selected])
    (output / 'selection.json').write_bytes(contract.encoded(selection))
    initial = {path: oracle.sha256(ROOT / path) for path in source_paths()}
    for path in initial:
        destination = output / 'sources' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((ROOT / path).read_bytes())
    (output / 'oracle.json').write_bytes(contract.encoded(metadata))
    compiler = capture(['rustc', '--version', '--verbose'], output, 'rustc')
    if compiler['exit_code'] != 0:
        raise ValueError('cannot identify Rust compiler; raw evidence preserved')
    artifacts = output / 'artifacts'
    artifacts.mkdir()
    requested = [('language_probe', candidate)] if scalars else []
    if modules:
        requested.append(('module_eval_probe', module_candidate))
    if classpath:
        requested.append(('jvm_snippet_probe', classpath_candidate))
    need_build = [name for name, supplied in requested if supplied is None]
    build = None
    if need_build:
        command = ['cargo', 'build', '-p', 'pkl-runtime', '--locked', '--release']
        for name in need_build:
            command += ['--example', name]
        command += ['--target-dir', str(ROOT / 'target')]
        build = capture(command, output, 'build', timeout=300)
        if build['exit_code'] != 0:
            raise ValueError('candidate build failed; raw build evidence preserved')
    bindings = {}
    for name, supplied in requested:
        path = supplied or ROOT / 'target/release/examples' / name
        if not path.is_file():
            raise ValueError('candidate artifact missing: ' + name)
        frozen = artifacts / name
        frozen.write_bytes(path.read_bytes())
        frozen.chmod(0o755)
        bindings[name] = dict(path=str(path), sha256=oracle.sha256(frozen),
                             source_binding='unbound-caller-supplied' if supplied else 'recorded-build',
                             build=None if supplied else build)
    results = []
    if scalars:
        classes = artifacts / 'java'
        classes.mkdir()
        javac = Path(metadata['java']).with_name('javac')
        compiled = capture([str(javac), '-encoding', 'UTF-8', '-cp', metadata['artifact'],
                            '-d', str(classes), str(ROOT / 'tools/java/M2LanguageProbe.java')],
                           output, 'javac', metadata['environment'])
        if compiled['exit_code'] != 0:
            raise ValueError('Java oracle compile failed; raw evidence preserved')
        results += acquire_cases(scalars, output, metadata, artifacts / 'language_probe', workers)
    module_context = None
    if modules:
        workspace = module_workspace or output.with_name(output.name + '-workspace')
        compat.validate_paths(ROOT, workspace, output)
        if not workspace.exists():
            compat.prepare_workspace(ROOT, workspace)
        else:
            expected = dict(schema_version=1, repository=str(ROOT.resolve()), workspace=str(workspace.resolve()))
            if not (workspace / compat.MARKER).is_file() or compat.read_json(workspace / compat.MARKER) != expected:
                raise ValueError('module-eval workspace is not owned by this checkout')
        corpus = contract.load(Path('compatibility/corpus.json'))
        original_manifest = dict(sources=corpus['sources'], cases=[case['upstream_case'] for case in modules])
        verified_inputs = compat.validate_inputs(ROOT, original_manifest, workspace)
        (output / 'module-inputs.json').write_bytes(contract.encoded(verified_inputs))
        baseline = compat.snapshot(workspace)
        m2_module_eval.validate_resource_set(baseline, corpus['sources'])
        (output / 'module-workspace.json').write_bytes(contract.encoded(baseline))
        module_context = dict(workspace=str(workspace), workers=1, watchdog_seconds=m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
                              baseline_sha256=oracle.sha256(output / 'module-workspace.json'),
                              input_validation_sha256=oracle.sha256(output / 'module-inputs.json'))
        for case in modules:
            if compat.snapshot(workspace) != baseline:
                raise ValueError('module-eval shared workspace changed before acquisition')
            row = m2_module_eval.acquire_case(ROOT, workspace, case, metadata, artifacts / 'module_eval_probe', output)
            results.append(row)
            print(f"{row['id']}: {row['status']}", flush=True)
    classpath_context = None
    if classpath:
        if len(classpath) != 1 or classpath[0]['id'] != m2_jvm_classpath.CASE_ID:
            raise ValueError('unexpected JVM-classpath case selection')
        workspace = classpath_workspace or output.with_name(output.name + '-jvm-classpath-workspace')
        compat.validate_paths(ROOT, workspace, output)
        child_directory = output / 'jvm-classpath'
        child = m2_jvm_classpath.execute(child_directory, artifacts / 'jvm_snippet_probe', workspace)
        # The child records a supplied artifact; only this parent can link its
        # exact frozen bytes and source snapshot to the recorded native build.
        if (child['candidate']['source_binding'] != 'unbound-caller-supplied'
                or child['candidate']['sha256'] != bindings['jvm_snippet_probe']['sha256']
                or child['oracle'] != metadata):
            raise ValueError('JVM-classpath child artifact/oracle differs from its parent')
        classpath_context = dict(directory='jvm-classpath', workspace=str(workspace), parent_output=str(output),
                                 child_report_sha256=oracle.sha256(child_directory / 'report.json'))
        results.append(classpath_result(classpath[0], child_directory, child))
    by_id = {row['id']: row for row in results}
    results = [by_id[row['id']] for row in selected]
    relation_manifest = dict(manifest, cases=scalars)
    relations = check_relations(relation_manifest, output)
    report = dict(schema_version=4 if classpath else 3, scope=manifest['comparison_boundary'],
                  module_scope=manifest.get('module_comparison_boundary'),
                  inventory_counts=manifest['counts'], selected_count=len(selected),
                  selection=selection,
                  execution=dict(workers=workers), oracle=metadata,
                  candidate=bindings.get('language_probe'), module_candidate=bindings.get('module_eval_probe'),
                  classpath_candidate=bindings.get('jvm_snippet_probe'), classpath_eval=classpath_context,
                  module_eval=module_context,
                  javac=dict(path=str(Path(metadata['java']).with_name('javac')),
                             sha256=oracle.sha256(Path(metadata['java']).with_name('javac'))) if scalars else None,
                  source_sha256=initial,
                  source_stability=all((ROOT / path).is_file() and oracle.sha256(ROOT / path) == value
                                       for path, value in initial.items()),
                  artifact_stability=all(Path(row['path']).is_file() and oracle.sha256(row['path']) == row['sha256']
                                         for row in bindings.values()),
                  results=results, metamorphic_results=relations,
                  status_counts=dict(Counter(row['status'] for row in results)),
                  full_gate_failures=contract.full_gate_failures(manifest, results))
    report['evidence_sha256'] = {str(path.relative_to(output)): oracle.sha256(path)
                                 for path in sorted(evidence_paths(output))}
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
    if 'execution' in report:
        if not isinstance(report['execution'], dict) or set(report['execution']) != {'workers'}:
            raise ValueError('invalid acquisition execution setting')
        validate_workers(report['execution']['workers'])
    manifest = contract.current_manifest()
    verify_oracle(directory, report)
    if not report['source_stability'] or not report['artifact_stability']:
        raise ValueError('sources or artifact changed during evidence acquisition')
    inventory = report['evidence_sha256']
    observed = {str(path.relative_to(directory)) for path in evidence_paths(directory)}
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
    adapters = report.get('selection', {}).get('adapters', ['language-api-string'])
    selected = {row['id']: row for row in selected_cases(manifest, adapters)}
    if not selected:
        raise ValueError('empty executable adapter selection')
    has_modules = any(row['adapter'] == m2_module_eval.ADAPTER for row in selected.values())
    has_classpath = any(row['adapter'] == m2_jvm_classpath.ADAPTER for row in selected.values())
    if has_modules and (type(report.get('schema_version')) is not int or report['schema_version'] not in (3, 4)):
        raise ValueError('module-eval requires the current report schema')
    if has_classpath and report.get('schema_version') != 4:
        raise ValueError('JVM-classpath requires the bound child report schema')
    if report.get('schema_version', 2) >= 3:
        selection = report.get('selection')
        if (not isinstance(selection, dict) or set(selection) != {'adapters', 'case_ids'}
                or json.loads((directory / 'selection.json').read_bytes()) != selection
                or selection['case_ids'] != list(selected)):
            raise ValueError('captured adapter selection differs')
    for name, key, adapter in [('language_probe', 'candidate', 'language-api-string'),
                                ('module_eval_probe', 'module_candidate', m2_module_eval.ADAPTER),
                                ('jvm_snippet_probe', 'classpath_candidate', m2_jvm_classpath.ADAPTER)]:
        needed = any(row['adapter'] == adapter for row in selected.values())
        candidate = report.get(key)
        if not needed:
            if candidate is not None:
                raise ValueError('unselected candidate binding')
            continue
        if not candidate or oracle.sha256(directory / 'artifacts' / name) != candidate['sha256']:
            raise ValueError('candidate artifact identity differs')
        binding = candidate.get('source_binding')
        if binding == 'recorded-build':
            build = json.loads((directory / 'build.process.json').read_bytes())
            if build != candidate['build'] or build['exit_code'] != 0 or build['timed_out']:
                raise ValueError('candidate recorded build differs or failed')
            if report.get('schema_version', 2) >= 3:
                built = [('language_probe', report.get('candidate')), ('module_eval_probe', report.get('module_candidate')),
                         ('jvm_snippet_probe', report.get('classpath_candidate'))]
                command = ['cargo', 'build', '-p', 'pkl-runtime', '--locked', '--release']
                for example, claim in built:
                    if claim is not None and claim['source_binding'] == 'recorded-build':
                        command += ['--example', example]
                command += ['--target-dir', str(ROOT / 'target')]
                if build['command'] != command:
                    raise ValueError('candidate build command differs from selected artifacts')
                if candidate['path'] != str(ROOT / 'target/release/examples' / name):
                    raise ValueError('candidate recorded build artifact path differs')
        elif binding != 'unbound-caller-supplied' or candidate.get('build') is not None:
            raise ValueError('invalid source/artifact binding claim')
    if any(case['adapter'] == m2_module_eval.ADAPTER for case in selected.values()):
        context = report['module_eval']
        if (context['workers'] != 1 or type(context.get('watchdog_seconds')) is not int
                or context['watchdog_seconds'] != m2_module_eval.CAPTURE_TIMEOUT_SECONDS
                or oracle.sha256(directory / 'module-workspace.json') != context['baseline_sha256']
                or oracle.sha256(directory / 'module-inputs.json') != context['input_validation_sha256']):
            raise ValueError('module-eval staged input provenance differs')
        corpus = json.loads((directory / 'sources/compatibility/corpus.json').read_bytes())
        m2_module_eval.validate_resource_set(json.loads((directory / 'module-workspace.json').read_bytes()), corpus['sources'])
        declared = {resource['path']: resource['sha256'] for source in corpus['sources'] for resource in source['resources']}
        for case in selected.values():
            if case['adapter'] == m2_module_eval.ADAPTER:
                for resource in [dict(path=case['input'], sha256=case['input_sha256']), *case['upstream_case']['resources']]:
                    if resource['path'] in declared and declared[resource['path']] != resource['sha256']:
                        raise ValueError('module-eval resource provenance conflicts')
                    declared[resource['path']] = resource['sha256']
        if json.loads((directory / 'module-inputs.json').read_bytes()) != dict(files=len(declared), sha256=compat.digest(compat.encoded(declared))):
            raise ValueError('module-eval input validation record differs')
    child = None
    if has_classpath:
        child = verify_classpath_child(directory, report)
    elif report.get('classpath_eval') is not None:
        raise ValueError('unselected JVM-classpath child context')
    ids = [row['id'] for row in report['results']]
    if set(ids) != set(selected) or len(ids) != len(selected) or report['selected_count'] != len(selected):
        raise ValueError('selected execution coverage incomplete or duplicate')
    if 'execution' in report and ids != list(selected):
        raise ValueError('selected execution order differs from manifest')
    for row in report['results']:
        case = selected[row['id']]
        if case['adapter'] == m2_jvm_classpath.ADAPTER:
            if row != classpath_result(case, directory / 'jvm-classpath', child):
                raise ValueError('JVM-classpath original result/profile binding changed')
            continue
        if case['adapter'] == m2_module_eval.ADAPTER:
            context = report['module_eval']
            if context['workers'] != 1 or oracle.sha256(directory / 'module-workspace.json') != context['baseline_sha256']:
                raise ValueError('module-eval workspace evidence changed')
            m2_module_eval.verify_case(ROOT, Path(context['workspace']), case, row, directory, report['oracle'],
                                      directory / 'artifacts/module_eval_probe',
                                      json.loads((directory / 'module-workspace.json').read_bytes()))
            continue
        folder = directory / 'cases' / row['id']
        if oracle.sha256(folder / 'input.pkl') != case['input_sha256']:
            raise ValueError('case input differs: ' + row['id'])
        for runtime in ('java', 'rust'):
            process = json.loads((folder / f'{runtime}.process.json').read_bytes())
            if process != row[runtime]:
                raise ValueError('process metadata differs: ' + row['id'])
            arguments = expression_arguments(ROOT / case['input'], case['expression'],
                                             case.get('power_assertions', False))
            if process['command'][-len(arguments):] != arguments:
                raise ValueError('oracle/candidate input URI, expression or power assertion mode differs: ' + row['id'])
            if runtime == 'rust' and process['command'][1:] != arguments:
                raise ValueError('candidate command adds unrecorded evaluation options: ' + row['id'])
            for channel in ('stdout', 'stderr'):
                if oracle.sha256(folder / f'{runtime}.{channel}.bin') != process[channel + '_sha256']:
                    raise ValueError('process raw hash differs: ' + row['id'])
        if row['java']['command'] != java_case_command(report['oracle'], Path(row['rust']['command'][0]),
                case['id'], ROOT / case['input'], case['expression'], case.get('power_assertions', False)):
            raise ValueError('case Java command changes pinned oracle options/classpath')
        if oracle.sha256(folder / 'java.outcome.bin') != row['java']['outcome_sha256']:
            raise ValueError('oracle outcome sidecar differs')
        if row['id'] == 'error-remainder-zero':
            repeated = folder / 'java-repeat'
            process = json.loads((repeated / 'java.process.json').read_bytes())
            if process != row.get('java_repeat'):
                raise ValueError('independent remainder Java capture metadata differs')
            if process['command'] != java_case_command(report['oracle'], Path(row['rust']['command'][0]),
                    case['id'] + '/java-repeat', ROOT / case['input'], case['expression'],
                    case.get('power_assertions', False)):
                raise ValueError('independent remainder Java command differs')
            for channel in ('stdout', 'stderr', 'outcome'):
                if oracle.sha256(repeated / f'java.{channel}.bin') != process[channel + '_sha256']:
                    raise ValueError('independent remainder Java raw capture differs')
        outcome = status(case, row['java'], row['rust'], *((folder / f'{name}.{channel}.bin').read_bytes()
                                    for name in ('java', 'rust') for channel in ('stdout', 'stderr')),
                         (folder / 'java.outcome.bin').read_bytes())
        if row['status'] != outcome:
            raise ValueError('raw comparison status differs: ' + row['id'])
    relation_manifest = dict(manifest, cases=[row for row in selected.values() if row['adapter'] == 'language-api-string'])
    if report['metamorphic_results'] != check_relations(relation_manifest, directory):
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
    parser.add_argument('--adapter', action='append', choices=['language-api-string', m2_module_eval.ADAPTER, m2_jvm_classpath.ADAPTER],
                        help='explicit focused acquisition; omitted adapters remain required in the full gate')
    parser.add_argument('--module-candidate', type=Path, help='override module-eval build; source binding remains unverified')
    parser.add_argument('--module-workspace', type=Path, help='exclusive managed physical workspace shared by Java and module candidate')
    parser.add_argument('--classpath-candidate', type=Path, help='override the JVM-profile native artifact; parent binding remains unverified')
    parser.add_argument('--classpath-workspace', type=Path, help='fresh JVM-profile workspace outside the parent evidence directory')
    parser.add_argument('--workers', type=worker_argument, default=1,
                        help=f'scalar acquisition concurrency (1-{MAX_WORKERS}, default 1); module eval remains sequential; throughput only')
    args = parser.parse_args()
    if args.record_only and not args.output:
        parser.error('--record-only requires --output')
    if args.workers != 1 and not args.output:
        parser.error('--workers requires --output')
    if (args.adapter or args.module_candidate or args.module_workspace or args.classpath_candidate or args.classpath_workspace) and not args.output:
        parser.error('adapter/module acquisition settings require --output')
    extra = {}
    if args.adapter is not None:
        extra['adapters'] = args.adapter
    if args.module_candidate is not None:
        extra['module_candidate'] = args.module_candidate.resolve()
    if args.module_workspace is not None:
        extra['module_workspace'] = args.module_workspace.absolute()
    if args.classpath_candidate is not None:
        extra['classpath_candidate'] = args.classpath_candidate.resolve()
    if args.classpath_workspace is not None:
        extra['classpath_workspace'] = args.classpath_workspace.absolute()
    report = execute(args.output.resolve(), args.candidate.resolve() if args.candidate else None, args.workers,
                     **extra) if args.output else check_report(
        (args.verify_report or args.verify_subset or args.verify_full or args.verify_development).resolve())
    print(json.dumps(dict(selected=report['status_counts'], full_gate_failures=len(report['full_gate_failures'])), sort_keys=True))
    if args.verify_report or args.record_only:
        return 0
    if args.verify_development:
        return int(not development_passes(report, args.verify_development.resolve()))
    return int(not subset_passes(report) or bool(args.verify_full and (report['full_gate_failures']
                     or any(row['source_binding'] != 'recorded-build' for row in [report.get('candidate'), report.get('module_candidate'), report.get('classpath_candidate')] if row))))


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f'M2 language error: {error}', file=sys.stderr)
        sys.exit(1)
