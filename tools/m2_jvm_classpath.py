#!/usr/bin/env python3
"""Additive original JVM-classpath snippet evidence; no CLI-contract promotion."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys

import compat
import m2_source_inputs
import oracle

ROOT = Path(__file__).resolve().parents[1]
PROFILE = 'compatibility/m2-jvm-classpath.json'
ADAPTER = 'language-jvm-classpath-output'
CASE_ID = 'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/basic/import1b.pkl'
CLASSPATH_ROOT = 'external/pkl/pkl-core/src/test/resources'
INPUT = 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/import1b.pkl'
RESOURCE = CLASSPATH_ROOT + '/org/pkl/core/snippets/imported.pkl'
PROBE = 'tools/java/M2JvmSnippetOutputProbe.java'
JAVA_SOURCES = [PROBE,
    'tools/java/classpath/org/pkl/core/module/TestModuleKeyFactory.java',
    'tools/java/classpath/org/pkl/core/resource/TestResourceReader.java']
UPSTREAM_SOURCES = [
    'pkl-core/src/test/kotlin/org/pkl/core/LanguageSnippetTestsEngine.kt',
    'pkl-core/src/test/kotlin/org/pkl/core/module/TestModuleKeyFactory.kt',
    'pkl-core/src/test/kotlin/org/pkl/core/resource/TestResourceReader.kt',
    'pkl-core/src/main/java/org/pkl/core/EvaluatorBuilder.java',
    'pkl-core/src/main/java/org/pkl/core/module/ModuleKeys.java',
    'pkl-core/src/main/java/org/pkl/core/module/ResolvedModuleKeys.java',
    'pkl-core/src/main/java/org/pkl/core/SecurityManagers.java',
    'pkl-core/src/main/java/org/pkl/core/runtime/ModuleCache.java',
    'pkl-core/src/main/java/org/pkl/core/runtime/VmUtils.java',
    'pkl-commons-cli/src/main/kotlin/org/pkl/commons/cli/CliCommand.kt',
]
TIMEOUT = 60


def build_profile(root=ROOT):
    corpus = compat.read_json(root / 'compatibility/corpus.json')
    original = next(case for case in corpus['cases'] if case['id'] == CASE_ID)
    if (original['source']['path'] != INPUT.removeprefix('external/pkl/')
            or original['execution']['status'] != 'blocked'
            or original['expected_exit'] != [0] or original['expected_outcome'] != 'success'):
        raise ValueError('historical blocked success contract changed')
    source = next(source for source in corpus['sources'] if source['repository'] == 'pkl')
    resources = {item['path']: dict(kind=item['kind'], sha256=item['sha256'])
                 for item in source['resources'] if item['path'].startswith(CLASSPATH_ROOT + '/')}
    if resources.get(RESOURCE) != dict(kind='file', sha256=oracle.sha256(root / RESOURCE)):
        raise ValueError('pinned classpath resource differs')
    if oracle.sha256(root / INPUT) != original['source']['sha256']:
        raise ValueError('original whole JVM snippet changed')
    return dict(schema=1, adapter=ADAPTER, baseline=corpus['baseline'], repetitions=2,
        scope='Original import1b whole-file JVM API output with its directory classpath. '
              'The historical CLI command and blocked corpus record remain unchanged; '
              'reviewed M2 execution mappings are separate from this profile.',
        original_case_id=CASE_ID, original_case_sha256=compat.digest(compat.encoded(original)),
        input=dict(path=INPUT, sha256=original['source']['sha256']),
        expected_exit=[0], expected_outcome='success', classpath_root=CLASSPATH_ROOT,
        classpath_resources=resources,
        reference=dict(revision=source['revision'], sources={
            'external/pkl/' + path: oracle.sha256(root / 'external/pkl' / path)
            for path in UPSTREAM_SOURCES}),
        configuration=dict(builder='EvaluatorBuilder.preconfigured', operation='evaluateOutputBytes',
            module_source='ModuleSource.path', power_assertions=True, color=False,
            stack_frame_transformer='StackFrameTransformers.empty', logger='Loggers.stdErr',
            module_cache_dir=None,
            environment={'NAME1': 'value1', 'NAME2': 'value2', '/foo/bar': 'foobar',
                         'foo bar': 'foo bar', 'file:///foo/bar': 'file:///foo/bar'},
            properties={'name1': 'value1', 'name2': 'value2', '/foo/bar': 'foobar'},
            classpath_services='Unchanged resource descriptors with source-bound Java companions '
                               'of the two upstream Kotlin test providers.',
            unused_fixture_setup='PackageServer TLS/test-port configuration is not installed; '
                                 'the selected input and imported resource perform no HTTP/package operations.'),
        boundary=dict(channels=['stdout', 'stderr', 'exit_code', 'files'],
            success='Raw evaluateOutputBytes; no upstream golden normalization.',
            error='Complete uncolored PklException.getMessage UTF-8; JVM bugs are strict nonpasses.',
            historical_cli_command='Preserved as provenance; not executed or claimed equivalent by this profile.',
            loader='Original modulepath URI, ClassLoader.getResource URL resolved identity, '
                   'original diagnostic source URI; no file canonicalization.',
            limitations=['Directory classpath and the selected original input only.',
                         'No general JAR classpath, provider API, URI/security or product compatibility claim.',
                         'No automatic original blocked-case promotion or full-M2 completion.']))


def load_profile(root=ROOT):
    profile = compat.read_json(root / PROFILE)
    if compat.encoded(profile) != compat.encoded(build_profile(root)):
        raise ValueError('JVM classpath profile/source binding is stale')
    return profile


def source_paths(root=ROOT):
    load_profile(root)
    return m2_source_inputs.source_paths(root)


def capture(command, directory, name, environment=None, timeout=120):
    """A build/compiler process, independent of the full language runner."""
    try:
        process = subprocess.run(command, cwd=ROOT, env=environment, capture_output=True, timeout=timeout)
        row = dict(command=[str(item) for item in command], exit_code=process.returncode, timed_out=False)
        stdout, stderr = process.stdout, process.stderr
    except subprocess.TimeoutExpired as error:
        stdout, stderr = error.stdout or b'', error.stderr or b''
        row = dict(command=[str(item) for item in command], exit_code=None, timed_out=True)
    for channel, data in [('stdout', stdout), ('stderr', stderr)]:
        (directory / f'{name}.{channel}.bin').write_bytes(data)
        row[channel + '_sha256'] = compat.digest(data)
    compat.write_json(directory / f'{name}.process.json', row)
    return row


def valid_exception_outcome(outcome):
    prefix = b'pkl-exception\torg.pkl.core.'
    if not outcome.startswith(prefix) or not outcome.endswith(b'\n'):
        return False
    name = outcome[len(prefix):-1]
    return bool(name) and all(unit in b'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789_$' for unit in name)


def classpath_files(root):
    return {CLASSPATH_ROOT + '/' + name: item for name, item in compat.snapshot(root / CLASSPATH_ROOT).items()
            if item['kind'] != 'directory'}


def validate_classpath(workspace, profile):
    if classpath_files(workspace) != profile['classpath_resources']:
        raise ValueError('JVM classpath root has missing, extra, changed or symlink resources')


def inventory(output):
    output = Path(output)
    if output.is_symlink() or not output.is_dir():
        raise ValueError('unexpected evidence directory: ' + str(output))
    files = {}
    for current, directories, names in os.walk(output, followlinks=False):
        # The exact fresh Cargo cache is excluded before inspecting/traversing
        # it. Every other directory must be an actual evidence directory.
        directories[:] = sorted(name for name in directories if Path(current) / name != output / 'build')
        for name in directories:
            path = Path(current) / name
            if path.is_symlink() or not path.is_dir():
                raise ValueError('unexpected evidence directory: ' + str(path))
        for name in sorted(names):
            path = Path(current) / name
            if path.is_symlink() or not path.is_file():
                raise ValueError('unexpected evidence entry: ' + str(path))
            if path == output / 'report.json':
                continue
            files[path.relative_to(output).as_posix()] = oracle.sha256(path)
    return files


def original_case(root):
    return next(case for case in compat.read_json(root / 'compatibility/corpus.json')['cases']
                if case['id'] == CASE_ID)


def build_command(output):
    return ['cargo', 'build', '-p', 'pkl-runtime', '--example', 'jvm_snippet_probe',
            '--release', '--locked', '--target-dir', str(output / 'build')]


def compiler_command(metadata, output):
    return [str(Path(metadata['java']).with_name('javac')), '-encoding', 'UTF-8',
            '-cp', str(output / 'artifacts/pkl-tools-0.32.0.jar'), '-d', str(output / 'artifacts/java'),
            *(str(output / 'sources' / path) for path in JAVA_SOURCES)]


def process_command(runtime, metadata, output, workspace, repeat):
    source, classpath = workspace / INPUT, workspace / CLASSPATH_ROOT
    if runtime == 'rust':
        return [str(output / 'artifacts/jvm_snippet_probe'), str(source), str(classpath)]
    if runtime != 'java':
        raise ValueError('unknown JVM classpath runtime')
    # The official prefix includes pinned JVM options and -cp; only its
    # development main class and explicit classpath change in this profile.
    prefix = list(metadata['command'][:-1])
    if len(prefix) < 3 or prefix[-2] != '-cp':
        raise ValueError('unknown pinned Java command layout')
    state = workspace / '.state' / compat.case_directory(CASE_ID)
    prefix = [f'-Duser.home={state / "home"}' if arg.startswith('-Duser.home=')
              else f'-Djava.io.tmpdir={state / "tmp"}' if arg.startswith('-Djava.io.tmpdir=')
              else arg for arg in prefix]
    prefix[-1] = os.pathsep.join(map(str, [output / 'artifacts/pkl-tools-0.32.0.jar',
                                           output / 'artifacts/java', classpath]))
    prefix.insert(1, '-Dpkl.m2.outcome=' + str(output / 'cases' / str(repeat) / 'java/outcome.bin'))
    return [*prefix, 'M2JvmSnippetOutputProbe', str(source)]


def capture_process(root, output, workspace, metadata, repeat, runtime):
    original = original_case(root)
    _, environment, generated, recorded_environment = compat.case_execution(
        original, [], metadata['environment'], workspace)
    command = process_command(runtime, metadata, output, workspace, repeat)
    folder = output / 'cases' / str(repeat) / runtime
    folder.mkdir(parents=True)
    before = compat.snapshot(workspace)
    try:
        captured = compat.capture(command, workspace / original['cwd'], environment, TIMEOUT, generated)
    except compat.CaptureTimeout as error:
        captured = error.result
    after = compat.snapshot(workspace)
    changes, secondary = compat.restore_changes(root, workspace, before, after)
    if compat.snapshot(workspace) != before:
        raise ValueError('JVM snippet workspace could not be restored')
    captured['workspace_changes'] = changes
    captured['files'] = {**{'generated/' + name: data for name, data in captured['files'].items()}, **secondary}
    stored = compat.store_capture(folder, captured)
    outcome = folder / 'outcome.bin'
    if runtime == 'java' and not outcome.exists():
        outcome.write_bytes(b'')
    private = compat.output_files(workspace / '.state' / compat.case_directory(CASE_ID))
    for name, data in private.items():
        path = compat.contained(folder / 'private-state', name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    process = dict(command=command, cwd=str(workspace / original['cwd']), environment=recorded_environment,
        original_case_sha256=compat.digest(compat.encoded(original)),
        source_uri=(workspace / INPUT).as_uri(), classpath_root=str(workspace / CLASSPATH_ROOT),
        timeout_seconds=TIMEOUT, capture=stored,
        outcome_sha256=oracle.sha256(outcome) if runtime == 'java' else None,
        workspace_before_sha256=compat.digest(compat.encoded(before)),
        workspace_after_sha256=compat.digest(compat.encoded(after)),
        private_files_sha256={name: compat.digest(data) for name, data in private.items()})
    compat.write_json(folder / 'process.json', process)
    return process


def comparison(output, repeat):
    java = compat.load_capture(output / 'cases' / str(repeat) / 'java')
    rust = compat.load_capture(output / 'cases' / str(repeat) / 'rust')
    outcome = (output / 'cases' / str(repeat) / 'java/outcome.bin').read_bytes()
    oracle_success = (not java['timed_out'] and java['exit_code'] == 0 and outcome == b'success\n'
                      and not java['stderr'] and not java['workspace_changes'])
    differences = compat.differences(java, rust)
    oracle_error = (java['timed_out'] or java['exit_code'] not in (0, 1)
                    or java['workspace_changes'] or outcome.startswith(b'oracle-error\t')
                    or (outcome != b'success\n' and not valid_exception_outcome(outcome)))
    if oracle_error:
        status = 'oracle-error'
    elif not oracle_success:
        status = 'oracle-contract-mismatch'
    elif rust['timed_out']:
        status = 'candidate-timeout'
    elif rust['exit_code'] in (2, 3, 4):
        status = {2: 'candidate-unsupported', 3: 'candidate-limit-exceeded', 4: 'candidate-internal-error'}[rust['exit_code']]
    else:
        status = 'mismatch' if differences else 'pass'
    return dict(status=status, raw_channels_match=not differences, differences=differences,
                upstream_expected_exit_match=java['exit_code'] == 0 and not java['timed_out'])


def execute(output, candidate=None, workspace=None):
    output = output.absolute()
    workspace = (workspace or output.with_name(output.name + '-workspace')).absolute()
    compat.validate_paths(ROOT, workspace, output)
    if output.exists() or workspace.exists():
        raise ValueError('choose new output and workspace directories; original evidence is immutable')
    profile = load_profile()
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    output.mkdir(parents=True)
    sources = {name: oracle.sha256(ROOT / name) for name in source_paths()}
    for name in sources:
        destination = output / 'sources' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, destination)
    artifacts = output / 'artifacts'
    artifacts.mkdir()
    shutil.copyfile(metadata['artifact'], artifacts / 'pkl-tools-0.32.0.jar')
    commands = []
    if candidate is None:
        command = build_command(output)
        build = capture(command, output, 'build-native', timeout=600)
        commands.append(dict(name='build-native', **build))
        if build['exit_code'] != 0 or build['timed_out']:
            raise ValueError('native build failed; raw evidence retained')
        candidate = output / 'build/release/examples/jvm_snippet_probe'
        binding = 'recorded-build'
    else:
        candidate = candidate.absolute()
        binding = 'unbound-caller-supplied'
    shutil.copyfile(candidate, artifacts / 'jvm_snippet_probe')
    (artifacts / 'jvm_snippet_probe').chmod(0o755)
    (artifacts / 'java').mkdir()
    shutil.copyfile(Path(metadata['java']).with_name('javac'), artifacts / 'javac-launcher')
    compiled = capture(compiler_command(metadata, output), output, 'javac', metadata['environment'])
    commands.append(dict(name='javac', **compiled))
    if compiled['exit_code'] != 0 or compiled['timed_out']:
        raise ValueError('JVM snippet compilation failed; raw evidence retained')
    artifact_before = {path.relative_to(artifacts).as_posix(): oracle.sha256(path)
                       for path in artifacts.rglob('*') if path.is_file()}
    if artifact_before['pkl-tools-0.32.0.jar'] != metadata['artifact_sha256']:
        raise ValueError('frozen oracle JAR changed before execution')
    compat.prepare_workspace(ROOT, workspace)
    validate_classpath(workspace, profile)
    corpus = compat.read_json(ROOT / 'compatibility/corpus.json')
    inputs = compat.validate_inputs(ROOT, dict(sources=corpus['sources'], cases=[original_case(ROOT)]), workspace)
    compat.write_json(output / 'workspace-inputs.json', inputs)
    baseline = compat.snapshot(workspace)
    compat.write_json(output / 'workspace-baseline.json', baseline)
    results = []
    for repeat in range(profile['repetitions']):
        processes = {runtime: capture_process(ROOT, output, workspace, metadata, repeat, runtime)
                     for runtime in ('java', 'rust')}
        row = dict(repeat=repeat, processes=processes, **comparison(output, repeat))
        results.append(row)
        compat.write_json(output / 'partial-results.json', results)
    after = {name: oracle.sha256(ROOT / name) for name in sources}
    compat.write_json(output / 'oracle.json', metadata)
    report = dict(schema=1, adapter=ADAPTER, profile=profile, original_case=original_case(ROOT),
        original_cli_contract_preserved=True, full_m2_complete=False,
        original_root=str(ROOT), output=str(output), workspace=str(workspace),
        oracle=metadata, sources=sources, sources_after=after, commands=commands,
        artifacts_before=artifact_before,
        artifacts_after={path.relative_to(artifacts).as_posix(): oracle.sha256(path)
                         for path in artifacts.rglob('*') if path.is_file()},
        candidate=dict(source_binding=binding, original_path=str(candidate),
                       sha256=artifact_before['jvm_snippet_probe']),
        javac=dict(path=str(Path(metadata['java']).with_name('javac')),
                   sha256=oracle.sha256(Path(metadata['java']).with_name('javac'))),
        results=results, files=inventory(output))
    compat.write_json(output / 'report.json', report)
    return report


def verify_capture(output, report, repeat, runtime, process, baseline):
    original_output, workspace = Path(report['output']), Path(report['workspace'])
    original = report['original_case']
    folder = output / 'cases' / str(repeat) / runtime
    if process != compat.read_json(folder / 'process.json'):
        raise ValueError('JVM snippet process metadata changed')
    if (process['command'] != process_command(runtime, report['oracle'], original_output, workspace, repeat)
            or process['cwd'] != str(workspace / original['cwd'])
            or process['source_uri'] != (workspace / INPUT).as_uri()
            or process['classpath_root'] != str(workspace / CLASSPATH_ROOT)
            or process['timeout_seconds'] != TIMEOUT
            or process['original_case_sha256'] != compat.digest(compat.encoded(original))):
        raise ValueError('JVM snippet profile argv/source/classpath contract changed')
    state = workspace / '.state' / compat.case_directory(CASE_ID)
    environment = {**report['oracle']['environment'], **original['environment'],
        'HOME': str(state / 'home'), 'TMPDIR': str(state / 'tmp'),
        'TMP': str(state / 'tmp'), 'TEMP': str(state / 'tmp')}
    keys = set(original['environment']) | {'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'NO_COLOR'}
    if process['environment'] != {key: environment[key] for key in keys if key in environment}:
        raise ValueError('JVM snippet environment changed')
    if process['capture'] != compat.read_json(folder / 'result.json'):
        raise ValueError('JVM snippet raw capture metadata changed')
    captured = compat.load_capture(folder)
    after = dict(baseline)
    for name, transition in captured['workspace_changes'].items():
        if transition['before'] != baseline.get(name):
            raise ValueError('JVM snippet workspace transition changed')
        if transition['after'] is None:
            after.pop(name, None)
        else:
            after[name] = transition['after']
    if (process['workspace_before_sha256'] != compat.digest(compat.encoded(baseline))
            or process['workspace_after_sha256'] != compat.digest(compat.encoded(after))):
        raise ValueError('JVM snippet workspace identity changed')
    expected_files = {'workspace/' + name: transition['after']['sha256']
                      for name, transition in captured['workspace_changes'].items()
                      if transition['after'] is not None and transition['after']['kind'] != 'directory'}
    if {name: compat.digest(data) for name, data in captured['files'].items()
            if name.startswith('workspace/')} != expected_files:
        raise ValueError('JVM snippet workspace file bytes changed')
    private = {name: compat.digest(data) for name, data in compat.output_files(folder / 'private-state').items()}
    if private != process['private_files_sha256']:
        raise ValueError('JVM snippet private runtime provenance changed')
    expected_outcome = oracle.sha256(folder / 'outcome.bin') if runtime == 'java' else None
    if process['outcome_sha256'] != expected_outcome:
        raise ValueError('JVM snippet outcome changed')


def verify_report(output):
    output = Path(output)
    report = compat.read_json(output / 'report.json')
    if report.get('schema') != 1 or report.get('adapter') != ADAPTER or report['files'] != inventory(output):
        raise ValueError('JVM classpath evidence inventory changed')
    snapshot = output / 'sources'
    profile = load_profile(snapshot)
    if (report['profile'] != profile or report['original_case'] != original_case(snapshot)
            or report['original_cli_contract_preserved'] is not True or report['full_m2_complete'] is not False):
        raise ValueError('JVM classpath profile/original contract changed')
    names = source_paths(snapshot)
    if set(report['sources']) != set(names) or report['sources_after'] != report['sources']:
        raise ValueError('JVM classpath source coverage/stability changed')
    if report['sources'] != {name: oracle.sha256(snapshot / name) for name in names}:
        raise ValueError('JVM classpath captured implementation source changed')
    captured_artifacts = {name.removeprefix('artifacts/'): digest for name, digest in report['files'].items()
                          if name.startswith('artifacts/')}
    if report['artifacts_before'] != captured_artifacts or report['artifacts_after'] != captured_artifacts:
        raise ValueError('JVM classpath execution artifact stability changed')
    validate_classpath(snapshot, profile)
    if report['oracle'] != compat.read_json(output / 'oracle.json'):
        raise ValueError('JVM classpath oracle provenance changed')
    lock = compat.read_json(snapshot / 'oracle.lock.json')
    metadata = report['oracle']
    expected_environment = {
        'PATH': os.defpath, 'HOME': str(Path(report['original_root']) / '.cache/oracle/home'),
        'TMPDIR': str(Path(report['original_root']) / '.cache/oracle/tmp'),
        'TMP': str(Path(report['original_root']) / '.cache/oracle/tmp'),
        'TEMP': str(Path(report['original_root']) / '.cache/oracle/tmp'),
        'LC_ALL': 'C', 'LANG': 'C', 'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1', 'COLUMNS': '80'}
    expected_oracle_command = [metadata['java'], *lock['jvm_options'],
        '-Duser.home=' + expected_environment['HOME'], '-Djava.io.tmpdir=' + expected_environment['TMPDIR'],
        '-cp', metadata['artifact'], 'org.pkl.cli.Main']
    if (metadata['version'] != lock['version'] or metadata['source'] != lock['source']
            or metadata['profile'] != 'reference' or metadata['command'] != expected_oracle_command
            or metadata['environment'] != expected_environment
            or metadata['java'] != metadata['jdk']['executable']
            or any(metadata['jdk']['properties'].get(key) != value for key, value in lock['jdk']['properties'].items())
            or profile['reference']['revision'] != lock['source']['revision']
            or metadata['lock_sha256'] != oracle.sha256(snapshot / 'oracle.lock.json')
            or metadata['artifact_sha256'] != lock['artifact']['sha256']
            or oracle.sha256(output / 'artifacts/pkl-tools-0.32.0.jar') != lock['artifact']['sha256']):
        raise ValueError('JVM classpath pinned oracle changed')
    original_output = Path(report['output'])
    original_root = Path(report['original_root'])
    workspace = Path(report['workspace'])
    if (not original_root.is_absolute() or not original_output.is_relative_to(original_root / 'target')
            or not workspace.is_relative_to(original_root / 'target') or compat.overlap(original_output, workspace)):
        raise ValueError('JVM classpath recorded workspace/output boundaries changed')
    expected_compiler = compiler_command(metadata, original_output)
    if (report['javac']['path'] != expected_compiler[0]
            or report['javac']['sha256'] != oracle.sha256(output / 'artifacts/javac-launcher')):
        raise ValueError('JVM classpath compiler identity changed')
    binding = report['candidate']['source_binding']
    expected_commands = [('build-native', build_command(original_output))] if binding == 'recorded-build' else []
    if binding not in ('recorded-build', 'unbound-caller-supplied'):
        raise ValueError('unknown JVM classpath native artifact binding')
    if (binding == 'recorded-build' and report['candidate']['original_path']
            != str(original_output / 'build/release/examples/jvm_snippet_probe')):
        raise ValueError('JVM classpath recorded native build artifact path changed')
    expected_commands.append(('javac', expected_compiler))
    if [(row['name'], row['command']) for row in report['commands']] != expected_commands:
        raise ValueError('JVM classpath compiler/build command changed')
    for row in report['commands']:
        process = compat.read_json(output / (row['name'] + '.process.json'))
        if process != {key: value for key, value in row.items() if key != 'name'}:
            raise ValueError('JVM classpath compiler/build metadata changed')
        if process['exit_code'] != 0 or process['timed_out']:
            raise ValueError('JVM classpath compiler/build did not complete')
        for channel in ('stdout', 'stderr'):
            if process[channel + '_sha256'] != oracle.sha256(output / (row['name'] + '.' + channel + '.bin')):
                raise ValueError('JVM classpath compiler/build bytes changed')
    classes = {name for name in report['files'] if name.startswith('artifacts/java/')}
    expected_classes = {'artifacts/java/M2JvmSnippetOutputProbe.class',
        'artifacts/java/org/pkl/core/module/TestModuleKeyFactory.class',
        'artifacts/java/org/pkl/core/resource/TestResourceReader.class'}
    if classes != expected_classes:
        raise ValueError('JVM classpath compiled class coverage changed')
    if oracle.sha256(output / 'artifacts/jvm_snippet_probe') != report['candidate']['sha256']:
        raise ValueError('JVM classpath frozen native artifact changed')
    baseline = compat.read_json(output / 'workspace-baseline.json')
    corpus = compat.read_json(snapshot / 'compatibility/corpus.json')
    declared_inputs = {}
    for source in corpus['sources']:
        if compat.digest(compat.encoded(source['resources'])) != source['resource_set_sha256']:
            raise ValueError('JVM classpath original resource-set declaration changed')
        for resource in source['resources']:
            if baseline.get(resource['path']) != dict(kind=resource['kind'], sha256=resource['sha256']):
                raise ValueError('JVM classpath original workspace resource changed')
            declared_inputs[resource['path']] = resource['sha256']
    if compat.read_json(output / 'workspace-inputs.json') != dict(
            files=len(declared_inputs), sha256=compat.digest(compat.encoded(declared_inputs))):
        raise ValueError('JVM classpath workspace input validation changed')
    for name in [INPUT, *profile['classpath_resources']]:
        if baseline.get(name) != dict(kind='file', sha256=oracle.sha256(snapshot / name)):
            raise ValueError('JVM classpath staged original input/resource changed')
    if report['results'] != compat.read_json(output / 'partial-results.json'):
        raise ValueError('JVM classpath partial execution records changed')
    if [row['repeat'] for row in report['results']] != list(range(profile['repetitions'])):
        raise ValueError('JVM classpath fresh repetition coverage changed')
    for row in report['results']:
        if set(row['processes']) != {'java', 'rust'}:
            raise ValueError('JVM classpath requires actual Java and Rust processes')
        for runtime, process in row['processes'].items():
            verify_capture(output, report, row['repeat'], runtime, process, baseline)
        if any(row[key] != value for key, value in comparison(output, row['repeat']).items()):
            raise ValueError('JVM classpath raw comparison/outcome changed')
    return report


def contract_failures(output, report):
    failures = [f'repetition {row["repeat"]}: {row["status"]}' for row in report['results'] if row['status'] != 'pass']
    for runtime in ('java', 'rust'):
        first = compat.load_capture(output / 'cases/0' / runtime)
        second = compat.load_capture(output / 'cases/1' / runtime)
        if compat.differences(first, second):
            failures.append(runtime + ' fresh repetition raw channels differ')
    return failures


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--check', action='store_true')
    mode.add_argument('--write-profile', action='store_true')
    mode.add_argument('--output', type=Path)
    mode.add_argument('--verify-report', type=Path)
    mode.add_argument('--verify-contract', type=Path)
    parser.add_argument('--workspace', type=Path)
    parser.add_argument('--candidate', type=Path)
    parser.add_argument('--record-only', action='store_true')
    args = parser.parse_args()
    if (args.workspace or args.candidate or args.record_only) and not args.output:
        parser.error('workspace/candidate/record-only require --output')
    try:
        if args.check:
            load_profile()
            print('JVM classpath profile/source binding is current; historical CLI case remains blocked.')
            return 0
        if args.write_profile:
            compat.write_json(ROOT / PROFILE, build_profile())
            return 0
        output = args.output or args.verify_report or args.verify_contract
        if args.output:
            execute(output, args.candidate, args.workspace)
        report = verify_report(output)
        print(json.dumps([dict(repeat=row['repeat'], status=row['status']) for row in report['results']]))
        if args.verify_report or args.record_only:
            return 0
        failures = contract_failures(output, report)
        for failure in failures:
            print(failure, file=sys.stderr)
        return int(bool(failures))
    except (OSError, ValueError, RuntimeError, KeyError, TypeError, StopIteration) as error:
        print('JVM classpath evidence failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
