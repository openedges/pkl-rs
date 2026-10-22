"""Exact original module-eval process contracts; no renderer or golden replay."""
from pathlib import Path
import json

import compat
import oracle

ADAPTER = 'language-module-eval'
CAPTURE_TIMEOUT_SECONDS = 60


def registry_cases(root, corpus, registry):
    if (not isinstance(registry, dict) or set(registry) != {
            'schema_version', 'adapter', 'baseline', 'scope', 'cases'}
            or type(registry['schema_version']) is not int or registry['schema_version'] != 1 or registry['adapter'] != ADAPTER
            or registry['baseline'] != corpus['baseline']
            or not isinstance(registry['scope'], str) or not registry['scope']
            or not isinstance(registry['cases'], list) or not registry['cases']):
        raise ValueError('invalid original module-eval adapter registry')
    originals = {case['id']: case for case in corpus['cases']}
    found = {}
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
        found[identifier] = entry
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
    original = case['upstream_case']
    validate_staged_case(root, workspace, case)
    command, process_environment, generated, recorded_environment = compat.case_execution(
        original, prefix, environment, workspace)
    cwd = compat.contained(workspace, original['cwd'])
    before = compat.snapshot(workspace)
    try:
        captured = compat.capture(command, cwd, process_environment, timeout, generated)
    except compat.CaptureTimeout as error:
        captured = error.result
    after = compat.snapshot(workspace)
    changes, secondary = compat.restore_changes(root, workspace, before, after)
    if compat.snapshot(workspace) != before:
        raise ValueError('module-eval workspace could not be restored')
    captured['files'] = {**{f'generated/{name}': data for name, data in captured['files'].items()}, **secondary}
    captured['workspace_changes'] = changes
    stored = compat.store_capture(directory, captured)
    private = compat.output_files(workspace / '.state' / compat.case_directory(case['id']))
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
    java, expected = capture_process(root, workspace, case, metadata['command'], metadata['environment'], folder / 'java')
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


def verify_case(root, workspace, case, row, output, metadata, binary, baseline):
    folder = output / 'cases' / compat.case_directory(case['id'])
    if row.get('directory') != folder.name or row.get('adapter') != ADAPTER:
        raise ValueError('module-eval adapter or capture directory changed')
    if oracle.sha256(folder / 'input.pkl') != case['input_sha256']:
        raise ValueError('module-eval input snapshot changed')
    original = case['upstream_case']
    validate_baseline(case, baseline, oracle.sha256(folder / 'input.pkl'))
    captures = {}
    for runtime, prefix in [('java', metadata['command']), ('rust', [str(binary)])]:
        process = json.loads((folder / runtime / 'process.json').read_bytes())
        if process != row[runtime]:
            raise ValueError('module-eval process metadata differs')
        if type(process.get('watchdog_seconds')) is not int or process['watchdog_seconds'] != CAPTURE_TIMEOUT_SECONDS:
            raise ValueError('module-eval external watchdog setting differs')
        # case_execution is pure with respect to command construction but resets
        # private runtime directories; verification must never invoke it.
        state = workspace / '.state' / compat.case_directory(case['id'])
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
        if process['workspace_before_sha256'] != compat.digest(compat.encoded(baseline)):
            raise ValueError('module-eval workspace baseline changed')
        after = dict(baseline)
        for name, change in process['workspace_changes'].items():
            if change['before'] != baseline.get(name):
                raise ValueError('module-eval workspace transition changed')
            if change['after'] is None:
                after.pop(name, None)
            else:
                after[name] = change['after']
        if process['workspace_after_sha256'] != compat.digest(compat.encoded(after)):
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
