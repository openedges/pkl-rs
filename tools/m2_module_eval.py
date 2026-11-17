"""Exact original module-eval process contracts; no renderer or golden replay."""
import argparse
from pathlib import Path
import json
import sys

import compat
import oracle

ADAPTER = 'language-module-eval'
CAPTURE_TIMEOUT_SECONDS = 60
ROOT = Path(__file__).resolve().parents[1]
REGISTRY = Path('compatibility/m2-module-eval.json')
# Retain the original CLI record without promoting its unresolved fixture setup.
# The separate reviewed JVM execution profile owns this case's ready execution.
PRESERVED_BLOCKED_IDS = frozenset({
    'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/basic/import1b.pkl',
})


def build_registry(corpus):
    """Admit every original-ready language snippet, never a semantic selection."""
    cases = [case for case in corpus['cases'] if case['suite'] == 'language-snippet'
             and (case['execution']['status'] == 'ready' or case['id'] in PRESERVED_BLOCKED_IDS)]
    return dict(schema_version=1, adapter=ADAPTER, baseline=corpus['baseline'],
                scope='Every original-ready upstream language snippet in corpus order, plus the '
                      'preserved blocked import1b CLI record. Entries authenticate unchanged original '
                      'commands, cwd, environment, resources, output and expected exit contracts. '
                      'Registration permits execution attempts; it establishes no semantic pass or '
                      'requirement mapping and does not fulfill blocked fixture prerequisites.',
                cases=[dict(id=case['id'], input_sha256=case['source']['sha256'],
                            upstream_case_sha256=compat.digest(compat.encoded(case))) for case in cases])


def registry_cases(root, corpus, registry):
    if (not isinstance(registry, dict) or set(registry) != {
            'schema_version', 'adapter', 'baseline', 'scope', 'cases'}
            or type(registry['schema_version']) is not int or registry['schema_version'] != 1 or registry['adapter'] != ADAPTER
            or registry['baseline'] != corpus['baseline']
            or not isinstance(registry['scope'], str) or not registry['scope']
            or not isinstance(registry['cases'], list) or not registry['cases']):
        raise ValueError('invalid original module-eval adapter registry')
    originals = {case['id']: case for case in corpus['cases']}
    if len(originals) != len(corpus['cases']):
        raise ValueError('duplicate original corpus case')
    found = {}
    checked = {}
    for entry in registry['cases']:
        if not isinstance(entry, dict) or set(entry) != {'id', 'input_sha256', 'upstream_case_sha256'}:
            raise ValueError('invalid module-eval registry entry')
        identifier = entry['id']
        if not isinstance(identifier, str):
            raise ValueError('invalid original module-eval case ID')
        if identifier in found or identifier not in originals:
            raise ValueError('duplicate or unknown original module-eval case')
        case = originals[identifier]
        if (case['suite'] != 'language-snippet' or case['execution']['status'] not in ('ready', 'blocked')
                or entry['input_sha256'] != case['source']['sha256']
                or entry['upstream_case_sha256'] != compat.digest(compat.encoded(case))):
            raise ValueError('original module-eval contract changed: ' + identifier)
        path = root / 'external/pkl' / case['source']['path']
        if oracle.sha256(path) != entry['input_sha256']:
            raise ValueError('original module-eval source changed: ' + identifier)
        for resource in case['resources']:
            compat.validate_resource(root, root, resource, checked)
        found[identifier] = entry
    if registry != build_registry(corpus):
        raise ValueError('original module-eval coverage/order is stale; regenerate with tools/m2_module_eval.py')
    return found


def execution_state(original, registered):
    """An explicit attempt never fulfills an unresolved original fixture setup."""
    ready = registered and original['execution']['status'] == 'ready'
    return dict(state='ready' if ready else 'unimplemented',
                reason=None if ready else original['execution']['reason'] or
                'Full upstream snippet/output contract adapter and Rust renderer are unimplemented.')


def status(java, rust, expected, actual, expected_exit=None):
    if expected.get('workspace_changes'):
        return 'oracle-error'
    if actual.get('workspace_changes'):
        return 'candidate-environment-error'
    if java['timed_out'] or java['exit_code'] not in (0, 1):
        return 'oracle-error'
    # CLI internal failures are observations, never ordinary compatible errors.
    if java['exit_code'] == 1 and not expected['stderr'].startswith('–– Pkl Error ––\n'.encode()):
        return 'oracle-error'
    if expected_exit is not None and java['exit_code'] not in expected_exit:
        # Raw agreement with a setup-dependent Java failure cannot close an
        # original contract that requires successful output evaluation.
        return 'oracle-contract-mismatch'
    if rust['timed_out']:
        return 'candidate-timeout'
    # Explicit development-probe protocol, not product CLI exit semantics.
    if rust['exit_code'] in (2, 3, 4):
        return {2: 'candidate-unsupported', 3: 'candidate-limit-exceeded',
                4: 'candidate-internal-error'}[rust['exit_code']]
    return 'mismatch' if compat.differences(expected, actual) else 'pass'


def validate_staged_case(root, workspace, case):
    original = case['upstream_case']
    source = {'path': case['input'], 'sha256': case['input_sha256']}
    checked = {}
    for resource in [source, *original['resources']]:
        compat.validate_resource(root, workspace, resource, checked)
    return checked


def capture_process(root, workspace, case, prefix, environment, directory, timeout=CAPTURE_TIMEOUT_SECONDS):
    """A fresh process, original argv/cwd/env, and complete filesystem changes."""
    if (directory / 'acquisition-failure.json').exists():
        raise ValueError('failed module-eval acquisition requires a new output directory')
    original = case['upstream_case']
    validate_staged_case(root, workspace, case)
    command, process_environment, generated, recorded_environment = compat.case_execution(
        original, prefix, environment, workspace)
    cwd = compat.contained(workspace, original['cwd'])
    before = compat.snapshot(workspace)
    captured = capture_error = restoration_error = None
    try:
        captured = compat.capture(command, cwd, process_environment, timeout, generated)
    except compat.CaptureTimeout as error:
        captured = error.result
    except BaseException as error:
        # Restoration also applies to interrupted or failed acquisition. A
        # failed process launch is never a comparable language observation.
        capture_error = error
    after = None
    changes, secondary, retained = {}, {}, {}
    retention_errors = []
    try:
        after = compat.snapshot(workspace)
        changes = {name: dict(before=before.get(name), after=after.get(name))
                   for name in sorted(before.keys() | after.keys()) if before.get(name) != after.get(name)}
        # restore_changes may fail after deleting some changed files. Retain
        # their original bytes before cleanup so that failure is reviewable.
        for name, change in changes.items():
            if change['after'] is not None and change['after']['kind'] != 'directory':
                try:
                    retained[f'workspace/{name}'] = compat.resource_bytes(compat.relative_path(workspace, name))
                except Exception as error:
                    retention_errors.append(f'{name}: {type(error).__name__}: {error}')
        changes, secondary = compat.restore_changes(root, workspace, before, after)
        if compat.snapshot(workspace) != before:
            raise ValueError('module-eval workspace could not be restored')
    except BaseException as error:
        restoration_error = error
    if capture_error is not None or restoration_error is not None:
        import traceback

        def failure(error):
            if error is None:
                return None
            return dict(type=f'{type(error).__module__}.{type(error).__qualname__}',
                        message=str(error), representation=repr(error),
                        traceback=''.join(traceback.format_exception(error)))

        primary = capture_error if capture_error is not None else restoration_error
        record = dict(schema_version=1, status='acquisition-failure', case_id=case['id'],
                      command=command, cwd=str(cwd), environment=recorded_environment,
                      source_uri=(workspace / case['input']).as_uri(),
                      upstream_case_sha256=compat.digest(compat.encoded(original)),
                      watchdog_seconds=timeout, capture_error=failure(capture_error),
                      restoration_error=failure(restoration_error),
                      workspace_restored=restoration_error is None,
                      workspace_before_sha256=compat.digest(compat.encoded(before)),
                      workspace_after_sha256=None if after is None else compat.digest(compat.encoded(after)),
                      workspace_changes=changes, capture_result_available=captured is not None,
                      raw_channels={}, files_sha256={}, retention_errors=retention_errors)
        try:
            directory.mkdir(parents=True, exist_ok=True)
            # Write the failure identity before attempting any optional payload
            # retention; an unreadable payload must not erase the failed attempt.
            compat.write_json(directory / 'acquisition-failure.json', record)
            for channel in ('stdout', 'stderr'):
                data = captured.get(channel) if captured is not None else getattr(capture_error, channel, None)
                if channel == 'stdout' and data is None:
                    data = getattr(capture_error, 'output', None)
                record['raw_channels'][channel] = None
                if isinstance(data, bytes):
                    path = directory / f'failure-{channel}.bin'
                    path.write_bytes(data)
                    record['raw_channels'][channel] = dict(path=path.name, sha256=compat.digest(data))
            if captured is not None:
                record['process'] = {key: captured[key] for key in ('exit_code', 'timed_out', 'elapsed_ns')}
                retained.update({f'generated/{name}': data for name, data in captured['files'].items()})
            else:
                try:
                    retained.update({f'generated/{name}': data for name, data in compat.output_files(generated).items()})
                except Exception as error:
                    retention_errors.append(f'generated files: {type(error).__name__}: {error}')
            try:
                private = compat.output_files(workspace / '.state' / compat.case_directory(original['id']))
                retained.update({f'private-state/{name}': data for name, data in private.items()})
            except Exception as error:
                retention_errors.append(f'private state: {type(error).__name__}: {error}')
            for name, data in retained.items():
                path = compat.contained(directory / 'failure-files', name)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
                record['files_sha256'][name] = compat.digest(data)
            compat.write_json(directory / 'acquisition-failure.json', record)
        except BaseException as error:
            primary.add_note(f'Could not retain module-eval acquisition failure evidence: {error!r}')
        if capture_error is not None and restoration_error is not None:
            capture_error.add_note(f'Module-eval workspace restoration also failed: {restoration_error!r}')
        raise primary
    captured['files'] = {**{f'generated/{name}': data for name, data in captured['files'].items()}, **secondary}
    captured['workspace_changes'] = changes
    stored = compat.store_capture(directory, captured)
    private = compat.output_files(workspace / '.state' / compat.case_directory(original['id']))
    for name, data in private.items():
        path = compat.contained(directory / 'private-state', name)
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    metadata = dict(command=command, cwd=str(cwd), environment=recorded_environment,
                    source_uri=(workspace / case['input']).as_uri(),
                    upstream_case_sha256=compat.digest(compat.encoded(original)),
                    exit_code=stored['exit_code'], timed_out=stored['timed_out'], watchdog_seconds=timeout,
                    stdout_sha256=stored['sha256']['stdout'], stderr_sha256=stored['sha256']['stderr'],
                    files_sha256=stored['files'], workspace_changes=changes,
                    private_files_sha256={name: compat.digest(data) for name, data in private.items()},
                    private_files_comparison='Host-private state retained as provenance, excluded from original public files comparison.',
                    workspace_before_sha256=compat.digest(compat.encoded(before)),
                    workspace_after_sha256=compat.digest(compat.encoded(after)))
    (directory / 'process.json').write_bytes(compat.encoded(metadata))
    return metadata, captured


def acquire_case(root, workspace, case, metadata, binary, output):
    folder = output / 'cases' / compat.case_directory(case['id'])
    folder.mkdir(parents=True)
    (folder / 'input.pkl').write_bytes((workspace / case['input']).read_bytes())
    java_cmd = list(metadata['command'])
    if 'execution_profile' in case:
        java_cmd = [arg if arg != '-Xmx512m' else '-Xmx2g' for arg in java_cmd]
    java, expected = capture_process(root, workspace, case, java_cmd, metadata['environment'], folder / 'java')
    rust, actual = capture_process(root, workspace, case, [str(binary)], metadata['environment'], folder / 'rust')
    expected_exit = case['upstream_case']['expected_exit']
    return dict(id=case['id'], adapter=ADAPTER, directory=folder.name,
                status=status(java, rust, expected, actual, expected_exit),
                raw_channels_match=not compat.differences(expected, actual),
                upstream_expected_exit_match=not java['timed_out'] and java['exit_code'] in expected_exit,
                java=java, rust=rust)


def validate_resource_set(baseline, sources):
    for source in sources:
        resources = source['resources']
        if compat.digest(compat.encoded(resources)) != source['resource_set_sha256']:
            raise ValueError('module-eval resource-set identity differs')
        for resource in resources:
            expected = dict(kind=resource['kind'], sha256=resource['sha256'])
            if baseline.get(resource['path']) != expected:
                raise ValueError('module-eval declared resource-set baseline differs: ' + resource['path'])


def validate_baseline(case, baseline, captured_input_sha256):
    """Bind offline workspace declarations to pinned original inputs/resources."""
    if captured_input_sha256 != case['input_sha256']:
        raise ValueError('module-eval source snapshot differs from original input')
    resources = [dict(path=case['input'], sha256=case['input_sha256'], kind='file'),
                 *case['upstream_case']['resources']]
    for resource in resources:
        expected = dict(kind=resource.get('kind', 'file'), sha256=resource['sha256'])
        if baseline.get(resource['path']) != expected:
            raise ValueError('module-eval original resource baseline differs: ' + resource['path'])


def verify_case(root, workspace, case, row, output, metadata, binary, baseline, *, baseline_digest=None):
    folder = output / 'cases' / compat.case_directory(case['id'])
    if row.get('directory') != folder.name or row.get('adapter') != ADAPTER:
        raise ValueError('module-eval adapter or capture directory changed')
    if oracle.sha256(folder / 'input.pkl') != case['input_sha256']:
        raise ValueError('module-eval input snapshot changed')
    original = case['upstream_case']
    validate_baseline(case, baseline, oracle.sha256(folder / 'input.pkl'))
    before_digest = baseline_digest if baseline_digest is not None else compat.digest(compat.encoded(baseline))
    captures = {}
    java_cmd = list(metadata['command'])
    if 'execution_profile' in case:
        java_cmd = [arg if arg != '-Xmx512m' else '-Xmx2g' for arg in java_cmd]
    for runtime, prefix in [('java', java_cmd), ('rust', [str(binary)])]:
        process = json.loads((folder / runtime / 'process.json').read_bytes())
        if process != row[runtime]:
            raise ValueError('module-eval process metadata differs')
        if type(process.get('watchdog_seconds')) is not int or process['watchdog_seconds'] != CAPTURE_TIMEOUT_SECONDS:
            raise ValueError('module-eval external watchdog setting differs')
        # case_execution is pure with respect to command construction but resets
        # private runtime directories; verification must never invoke it.
        # case_execution receives the original contract, whose ID can differ
        # from the manifest adapter ID. Preserve its exact private-state key.
        state = workspace / '.state' / compat.case_directory(original['id'])
        expected_prefix = [f'-Duser.home={state / "home"}' if arg.startswith('-Duser.home=')
                           else f'-Djava.io.tmpdir={state / "tmp"}' if arg.startswith('-Djava.io.tmpdir=')
                           else arg for arg in prefix]
        command = expected_prefix + original['command']
        if (process['command'] != command or process['cwd'] != str(compat.contained(workspace, original['cwd']))
                or process['source_uri'] != (workspace / case['input']).as_uri()
                or process['upstream_case_sha256'] != compat.digest(compat.encoded(original))):
            raise ValueError('module-eval original argv/cwd/source contract changed')
        expected_env = {**metadata['environment'], **original['environment'],
                        'HOME': str(state / 'home'), 'TMPDIR': str(state / 'tmp'),
                        'TMP': str(state / 'tmp'), 'TEMP': str(state / 'tmp')}
        keys = set(original['environment']) | {'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'NO_COLOR'}
        if process['environment'] != {k: expected_env[k] for k in keys if k in expected_env}:
            raise ValueError('module-eval original process environment changed')
        if process['workspace_before_sha256'] != before_digest:
            raise ValueError('module-eval workspace baseline changed')
        after = dict(baseline)
        if not process['workspace_changes']:
            after_digest = before_digest
        else:
            for name, change in process['workspace_changes'].items():
                if change['before'] != baseline.get(name):
                    raise ValueError('module-eval workspace transition changed')
                if change['after'] is None:
                    after.pop(name, None)
                else:
                    after[name] = change['after']
            after_digest = compat.digest(compat.encoded(after))
        if process['workspace_after_sha256'] != after_digest:
            raise ValueError('module-eval workspace after snapshot changed')
        private = compat.output_files(folder / runtime / 'private-state')
        if {name: compat.digest(data) for name, data in private.items()} != process['private_files_sha256']:
            raise ValueError('module-eval private-state evidence differs')
        if process['private_files_comparison'] != 'Host-private state retained as provenance, excluded from original public files comparison.':
            raise ValueError('module-eval private-state boundary differs')
        captured = compat.load_capture(folder / runtime)
        workspace_files = {f'workspace/{name}': transition['after']['sha256']
                           for name, transition in process['workspace_changes'].items()
                           if transition['after'] is not None and transition['after']['kind'] != 'directory'}
        if {name: compat.digest(data) for name, data in captured['files'].items()
                if name.startswith('workspace/')} != workspace_files:
            raise ValueError('module-eval captured workspace bytes differ from transition')
        for channel in ('stdout', 'stderr'):
            if compat.digest(captured[channel]) != process[channel + '_sha256']:
                raise ValueError('module-eval raw process channel changed')
        if (captured['exit_code'] != process['exit_code'] or captured['timed_out'] != process['timed_out']
                or captured['workspace_changes'] != process['workspace_changes']
                or {n: compat.digest(data) for n, data in captured['files'].items()} != process['files_sha256']):
            raise ValueError('module-eval exit/files/workspace channels changed')
        captures[runtime] = captured
    if row['status'] != status(row['java'], row['rust'], captures['java'], captures['rust'], original['expected_exit']):
        raise ValueError('module-eval raw comparison status changed')
    # These fields are additive to reports from the original error-only adapter.
    # Classification above always enforces the expectation even for older rows.
    for field, expected in [
            ('raw_channels_match', not compat.differences(captures['java'], captures['rust'])),
            ('upstream_expected_exit_match', not row['java']['timed_out'] and row['java']['exit_code'] in original['expected_exit'])]:
        if field in row and row[field] is not expected:
            raise ValueError('module-eval raw/contract match metadata changed')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--check', action='store_true',
                        help='check complete original-ready admission and fixture integrity, not compatibility')
    args = parser.parse_args()
    corpus = json.loads((ROOT / 'compatibility/corpus.json').read_bytes())
    registry = json.loads((ROOT / REGISTRY).read_bytes()) if args.check else build_registry(corpus)
    entries = registry_cases(ROOT, corpus, registry)
    if not args.check:
        (ROOT / REGISTRY).write_bytes(compat.encoded(registry))
    print(json.dumps(dict(registered=len(entries),
                          original_ready=sum(case['id'] in entries and case['execution']['status'] == 'ready'
                                             for case in corpus['cases'])), sort_keys=True))
    return 0


if __name__ == '__main__':
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError) as error:
        print(f'Module-eval registry error: {error}', file=sys.stderr)
        sys.exit(1)
