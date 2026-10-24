"""Record all main frame-transformer sessions and audit exact Java/candidate channels.

This runner does not build candidates. Supplied artifacts and concurrent source
snapshots are separate evidence; an enclosing build record must bind them.
"""
import argparse
from collections import Counter
from contextlib import contextmanager
import hashlib
import json
import math
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import time

import frame_transformer_fixtures as fixtures
import frame_transformer_oracle as frame_oracle
import m2_source_inputs

ROOT = Path(__file__).resolve().parents[1]
TARGETS = ('native', 'napi', 'chromium')
MANDATORY_SOURCES = frozenset({
    'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'references.lock',
    'crates/pkl-runtime/Cargo.toml', 'crates/pkl-runtime/src/vm.rs',
    'crates/pkl-runtime/src/heap.rs', 'crates/pkl-runtime/src/probe.rs',
    'crates/pkl-runtime/src/logging.rs', 'crates/pkl-runtime/src/vm/deprecation.rs',
    'crates/pkl-wasm/src/lib.rs', 'crates/pkl-napi/src/lib.rs',
    'tools/frame_transformer_compare.py', 'tools/tests/test_frame_transformer_compare.py',
    'tools/m2_source_inputs.py', 'tools/frame_transformer_fixtures.py', 'tools/frame_transformer_oracle.py',
    'tools/m2_deprecation.py', 'tools/benchmark.py', 'tools/java/M2FrameTransformerProbe.java',
    'crates/pkl-runtime/examples/frame_transformer_fixture_probe.rs', 'tools/platform/capture-frame-transformer.mjs',
    'tools/platform/frame-transformer-session.mjs', 'tools/platform/module-policy-session.mjs',
    'tools/platform/resource-session.mjs', 'tools/platform/package.json', 'tools/platform/package-lock.json',
})
WASM_REQUIRED = frozenset({'pkl_wasm.js', 'pkl_wasm_bg.wasm'})
PORTABLE_TRACKED_SOURCES = frozenset({
    'tools/platform/capture-frame-transformer.mjs', 'tools/frame_transformer_fixtures.py', 'tools/frame_transformer_oracle.py',
    'tools/platform/frame-transformer-session.mjs', 'tools/platform/module-policy-session.mjs',
    'tools/platform/resource-session.mjs', 'tools/platform/package.json', 'tools/platform/package-lock.json',
})
CALL = r'(?:outer-\d+|transform-callback-\d+|log-callback-\d+)'
META = re.compile(CALL + r'\.(?:disposition|phase|reason|api-error-kind|callback-origin)\.bin\Z')
BOUNDARY = {
    'compared': 'Every public payload .bin file and complete ordered protocol bytes, including Java UTF-8 and UTF-16, with exact filename coverage.',
    'java_provenance': 'Only Java *.error-class.bin files are producer provenance; no Java exception classes are synthesized by candidates.',
    'candidate_provenance': 'Only per-call disposition/phase/reason/api-error-kind/callback-origin .bin files are candidate metadata; N-API process.stdout.bin/process.stderr.bin are separate process provenance. Trace and process captures remain separate evidence.',
    'nonpasses': 'Java oracle-error and API-exception observations remain nonpasses even when their compared bytes agree. Candidate unsupported, limits, internal failures and missing captures never pass.',
    'buffer_supplement': 'Supplemental Java evaluateTest buffer observations are not executed by this main expression comparison. Test API/buffer parity remains a separate M2 obligation.',
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def sha(path):
    return digest(Path(path).read_bytes())


def put(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes((json.dumps(value, indent=2, ensure_ascii=True) + '\n').encode())


def read(path):
    return json.loads(path.read_bytes())


def same_json(left, right):
    """Type-aware metadata equality; public payloads never pass through JSON."""
    return json.dumps(left, sort_keys=True, ensure_ascii=True) == json.dumps(right, sort_keys=True, ensure_ascii=True)


def safe(root, name):
    path = PurePosixPath(name)
    require(isinstance(name, str) and str(path) == name and not path.is_absolute()
            and '..' not in path.parts, 'unsafe evidence path')
    result = root.joinpath(*path.parts)
    require(not result.is_symlink() and result.resolve().is_relative_to(root.resolve()), 'unsafe evidence link')
    return result


def files(root):
    require(not root.is_symlink(), 'evidence root is a symlink')
    found = {}
    if not root.exists():
        return found
    for directory, subdirs, names in os.walk(root, followlinks=False):
        for name in subdirs + names:
            require(not (Path(directory) / name).is_symlink(), 'evidence contains a symlink')
        for name in names:
            path = Path(directory) / name
            require(path.is_file(), 'nonregular evidence entry')
            found[path.relative_to(root).as_posix()] = dict(sha256=sha(path), bytes=path.stat().st_size)
    return dict(sorted(found.items()))


@contextmanager
def authority(fixture):
    verification = fixtures.verify(fixture)
    with tempfile.TemporaryDirectory(prefix='frame-transformer-java-authority-') as temporary:
        root = Path(temporary)
        # verify() has already checked every member and the independent archive digest.
        with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as stream:
            stream.extractall(root, filter='data')
        yield read(fixture / 'cases.json'), verification, root / 'main'


def public_channels(directory, java=False, target=None):
    compared, provenance = {}, {}
    for path in sorted(directory.glob('*.bin')) if directory.exists() else []:
        require(not path.is_symlink() and path.is_file(), 'nonregular public channel')
        if java and path.name.endswith('.error-class.bin'):
            provenance[path.name] = path.read_bytes()
        elif not java and (META.fullmatch(path.name) or target == 'napi' and path.name in {'process.stdout.bin', 'process.stderr.bin'}):
            provenance[path.name] = path.read_bytes()
        else:
            compared[path.name] = path.read_bytes()
    if java:
        compared['protocol.bin'] = (directory / 'process/stdout.bin').read_bytes()
    return compared, provenance


def compare_bytes(expected, actual):
    rows = []
    for name in sorted(expected.keys() | actual.keys()):
        left, right = expected.get(name), actual.get(name)
        rows.append(dict(name=name, expected_sha256=None if left is None else digest(left),
                         actual_sha256=None if right is None else digest(right),
                         expected_bytes=None if left is None else len(left),
                         actual_bytes=None if right is None else len(right),
                         raw_equal=left is not None and right is not None and left == right))
    return dict(exact_coverage=expected.keys() == actual.keys(),
                raw_equal=bool(rows) and expected.keys() == actual.keys() and all(row['raw_equal'] for row in rows),
                channels=rows)


def records(raw):
    return {name: dict(sha256=digest(value), bytes=len(value)) for name, value in sorted(raw.items())}


def native_arguments(binary, directory, case):
    require(case['operation'] == 'expression', 'Java-only test supplement cannot become a candidate expression')
    result = [str(binary), str(directory), case['uri'], case['operation']]
    for key in ['transformer', 'logger']:
        profile = case[key]
        result.extend([profile['mode'], profile['action'], profile['callback'], str(profile['callback_after'])])
    return result


def replacement_inputs(case):
    frame = case['replacement']
    if frame is None:
        return {}
    def units(value):
        require(isinstance(value, list) and all(type(unit) is int and 0 <= unit <= 65535 for unit in value),
                'invalid replacement UTF-16 units')
        return b''.join(unit.to_bytes(2, 'big') for unit in value)
    result = {'uri.u16be': units(frame['moduleUri']),
              'member-null.txt': b'true' if frame['memberName'] is None else b'false',
              'position.txt': ':'.join(str(frame[name]) for name in ['startLine', 'startColumn', 'endLine', 'endColumn']).encode(),
              'source-count.txt': str(len(frame['sourceLines'])).encode()}
    if frame['memberName'] is not None:
        result['member.u16be'] = units(frame['memberName'])
    for index, line in enumerate(frame['sourceLines']):
        result[f'source-{index}.u16be'] = units(line)
    return result


def native_inputs(case, fixture):
    require(case['operation'] == 'expression', 'Java-only supplement requires its own API boundary')
    source = (fixture / case['source']).read_bytes()
    require(digest(source) == case['source_sha256'], 'fixture source digest differs')
    result = {'input.pkl': source, 'expressions.txt': ('\n'.join(case['expressions']) + '\n').encode()}
    result.update({'replacement/' + name: value for name, value in replacement_inputs(case).items()})
    return result


def native_architecture(directory, case):
    issues, checks = [], {}
    try:
        data = (directory / 'candidate-trace.tsv').read_bytes()
        require(data and data.endswith(b'\n'), 'native trace missing or truncated')
        fields = {}
        for line in data.decode('utf-8').splitlines():
            require('\t' in line, 'invalid native trace line')
            key, value = line.split('\t', 1)
            fields.setdefault(key, []).append(value)

        def one(name):
            require(len(fields.get(name, [])) == 1, 'missing/duplicate native trace field: ' + name)
            return fields[name][0]

        constructor = ('module-evaluator', 'compact', case['logger']['mode'], case['transformer']['mode'])
        checks['constructor'] = one('constructor') == '(' + ', '.join(json.dumps(part) for part in constructor) + ')'
        checks['terminal_heap_slots'] = one('terminal-heap-slots') == '0'
        checks['terminal_context_depth'] = one('terminal-context-depth') == '0'
        polls, collections = one('polls'), one('collections')
        require(polls.isdecimal() and collections.isdecimal(), 'invalid native architecture counts')
        checks['polls_and_collections'] = int(polls) > 0 and int(collections) > 0
        checks['process_stderr_empty'] = (directory / 'process/stderr.bin').read_bytes() == b''
    except (OSError, ValueError, UnicodeError) as error:
        issues.append(str(error))
    return dict(checks=checks, issues=issues, valid=bool(checks) and not issues and all(checks.values()))


def portable_architecture(directory, target):
    checks, issues = {}, []
    try:
        trace = read(directory / 'candidate-trace.json')
        architecture = trace['architecture']
        checks = dict(session_created=architecture['sessionCreated'] is True,
                      terminal_heap_slots=type(architecture['terminalSlots']) is int and architecture['terminalSlots'] == 0,
                      terminal_context_depth=type(architecture['terminalDepth']) is int and architecture['terminalDepth'] == 0,
                      owned_buffers_stable=architecture['retainedPayloadsStable'] is True,
                      acquisition_completed=trace['acquisitionError'] is None,
                      polls_and_collections=type(architecture['polls']) is int and architecture['polls'] > 0
                          and type(architecture['collections']) is int
                          and architecture['collections'] > 0,
                      protocol_identity=(directory / 'protocol.bin').read_bytes()
                          == ('\n'.join(trace['protocol']) + '\n').encode())
        process = read(directory / 'process.json')
        if target == 'napi':
            child = read(directory / 'child.json')
            checks.update(process_exit=type(process['exitCode']) is int and process['exitCode'] == 0 and process['signal'] is None and process['error'] is None,
                          child_architecture=same_json(child['architecture'], architecture) and child['acquisitionError'] is None,
                          process_stdout_empty=(directory / 'process.stdout.bin').read_bytes() == b'',
                          process_stderr_empty=(directory / 'process.stderr.bin').read_bytes() == b'')
        else:
            checks.update(isolation=process['isolation'] == 'new-browser-context-and-wasm-instance',
                          browser_errors=process['browserErrors'] == [],
                          process_acquisition=process['acquisitionError'] is None,
                          process_architecture=same_json(process['architecture'], architecture))
    except (OSError, ValueError, KeyError, TypeError, UnicodeError) as error:
        issues.append(str(error))
    return dict(checks=checks, issues=issues, valid=bool(checks) and not issues and all(checks.values()))


def compare_session(expected_directory, actual_directory, case, target, process, extra_issues=()):
    expected, java_provenance = public_channels(expected_directory, java=True)
    actual, candidate_provenance = public_channels(actual_directory, target=target)
    comparison = compare_bytes(expected, actual)
    issues = list(extra_issues)
    allowed_files = {'input.pkl', 'expressions.txt', 'candidate-trace.tsv'} if target == 'native' else {
        'input.json', 'candidate-trace.json', 'process.json', *(['child.json'] if target == 'napi' else [])}
    for path in actual_directory.iterdir() if actual_directory.exists() else []:
        if path.name == 'replacement' and path.is_dir() and target == 'native':
            expected_replacement = replacement_inputs(case)
            if not expected_replacement or files(path) != records(expected_replacement):
                issues.append('native replacement input coverage or bytes differ')
            continue
        if path.name == 'process' and path.is_dir() and target == 'native':
            if set(entry.name for entry in path.iterdir()) != {'stdout.bin', 'stderr.bin', 'result.json'}:
                issues.append('native process file coverage differs')
            continue
        if not path.is_file() or not (path.name.endswith('.bin') or path.name in allowed_files):
            issues.append('unexpected candidate artifact: ' + path.name)
    statuses = {name.removesuffix('.status.bin'): data for name, data in actual.items() if name.endswith('.status.bin')}
    expected_statuses = {name.removesuffix('.status.bin'): data for name, data in expected.items() if name.endswith('.status.bin')}
    required = {f'outer-{i}' for i in range(len(case['expressions']))}
    if not required.issubset(statuses):
        issues.append('missing outer call channels')
    for prefix, status in statuses.items():
        origin = candidate_provenance.get(prefix + '.callback-origin.bin')
        if origin not in {b'', b'log-transformer', b'log-delegate', b'diagnostic-transformer', b'diagnostic-renderer'}:
            issues.append('missing or invalid callback origin: ' + prefix)
        elif status == b'success\n' and origin:
            issues.append('successful call carries callback failure origin: ' + prefix)
        if status not in {b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'}:
            issues.append('invalid candidate status: ' + prefix)
        for field in ['stdout', 'stderr']:
            try:
                frame_oracle.check_string(actual, prefix + '.' + field)
            except ValueError as error:
                issues.append(prefix + ': ' + str(error))
        if status != b'api-exception\n':
            for field in ['disposition', 'phase', 'reason']:
                if prefix + '.' + field + '.bin' not in candidate_provenance:
                    issues.append('missing candidate metadata: ' + prefix + '.' + field)
        elif target == 'native' and prefix + '.api-error-kind.bin' not in candidate_provenance:
            issues.append('missing native API error kind: ' + prefix)
    for name in candidate_provenance:
        if not META.fullmatch(name):
            continue
        prefix, field, _ = name.rsplit('.', 2)
        if prefix not in statuses or prefix not in expected_statuses:
            issues.append('candidate metadata without matching public call: ' + name)
        elif (field == 'api-error-kind' and statuses[prefix] != b'api-exception\n'
              or field in {'disposition', 'phase', 'reason'} and statuses[prefix] == b'api-exception\n'):
            issues.append('candidate metadata/status conflict: ' + name)
    for prefix, status in statuses.items():
        disposition = candidate_provenance.get(prefix + '.disposition.bin')
        if status == b'success\n' and disposition != b'' or status == b'error\n' and disposition != b'language-error':
            issues.append('candidate disposition/status conflict: ' + prefix)
        if status == b'candidate-error\n' and disposition in {None, b'', b'language-error'}:
            issues.append('candidate disposition/status conflict: ' + prefix)
    if any(name.endswith('.error-class.bin') for name in actual):
        issues.append('candidate synthesized Java exception provenance')
    dispositions = {name: data.decode('utf-8', 'backslashreplace') for name, data in candidate_provenance.items()
                    if name.endswith('.disposition.bin')}
    architecture = native_architecture(actual_directory, case) if target == 'native' else portable_architecture(actual_directory, target)
    reference_nonpasses = {prefix: status.decode().removesuffix('\n') for prefix, status in expected_statuses.items()
                           if status in {b'oracle-error\n', b'api-exception\n'}}
    reasons = []
    if process.get('status') == 'timeout':
        reasons.append('candidate-timeout')
    elif process.get('status') != 'executed' or type(process.get('exit_code')) is not int or process.get('exit_code') != 0:
        reasons.append('acquisition-failed')
    if issues:
        reasons.append('acquisition-failed')
    if not architecture['valid']:
        reasons.append('architecture-failed')
    if 'oracle-error' in reference_nonpasses.values():
        reasons.append('oracle-error')
    if 'api-exception' in reference_nonpasses.values():
        reasons.append('api-exception')
    for label, predicate in [
            ('candidate-unsupported', lambda x: x == 'unsupported'),
            ('candidate-limit-exceeded', lambda x: x == 'limit-exceeded'),
            ('candidate-internal-error', lambda x: x not in {'', 'language-error', 'unsupported', 'limit-exceeded'})]:
        if any(predicate(value) for value in dispositions.values()):
            reasons.append(label)
    if b'candidate-error\n' in statuses.values() and not any(reason.startswith('candidate-') for reason in reasons):
        reasons.append('candidate-non-language-failure')
    if not comparison['raw_equal']:
        reasons.append('mismatch')
    return dict(status=reasons[0] if reasons else 'pass', nonpass_reasons=list(dict.fromkeys(reasons)),
                issues=issues, architecture=architecture, reference_nonpasses=reference_nonpasses,
                java_provenance=records(java_provenance), candidate_provenance=records(candidate_provenance),
                candidate_dispositions=dispositions, **comparison)


def capture(command, cwd, environment, directory, timeout):
    directory.mkdir(parents=True, exist_ok=False)
    started = time.perf_counter_ns()
    stdout = stderr = b''
    error, exit_code, status = None, None, 'launch-error'
    try:
        child = subprocess.Popen(command, cwd=cwd, env=environment, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=subprocess.PIPE, start_new_session=True)
        try:
            stdout, stderr = child.communicate(timeout=timeout)
            status = 'executed'
        except subprocess.TimeoutExpired:
            try:
                os.killpg(child.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            stdout, stderr = child.communicate()
            status = 'timeout'
        exit_code = child.returncode
    except OSError as exception:
        error = dict(type=type(exception).__name__, errno=exception.errno, message=str(exception))
    for channel, value in [('stdout', stdout), ('stderr', stderr)]:
        (directory / (channel + '.bin')).write_bytes(value)
    result = dict(command=[str(arg) for arg in command], cwd=str(cwd), environment=environment,
                  timeout_seconds=timeout, status=status, exit_code=exit_code, error=error,
                  elapsed_ns=time.perf_counter_ns() - started,
                  stdout=dict(sha256=digest(stdout), bytes=len(stdout)),
                  stderr=dict(sha256=digest(stderr), bytes=len(stderr)))
    put(directory / 'result.json', result)
    return result


def verify_process(directory, recorded):
    require(same_json(read(directory / 'result.json'), recorded), 'process record differs')
    require(recorded['status'] in {'executed', 'timeout', 'launch-error'}, 'unknown process status')
    require(type(recorded['elapsed_ns']) is int and recorded['elapsed_ns'] > 0
            and type(recorded['timeout_seconds']) in {int, float} and math.isfinite(recorded['timeout_seconds'])
            and recorded['timeout_seconds'] > 0, 'invalid process timing contract')
    require((recorded['error'] is not None) == (recorded['status'] == 'launch-error'), 'launch failure classification differs')
    require((recorded['exit_code'] is None) == (recorded['status'] == 'launch-error'), 'process exit coverage differs')
    require(recorded['exit_code'] is None or type(recorded['exit_code']) is int, 'invalid process exit code type')
    for channel in ['stdout', 'stderr']:
        value = (directory / (channel + '.bin')).read_bytes()
        require(same_json(recorded[channel], dict(sha256=digest(value), bytes=len(value))), 'raw process channel differs')


def source_names(root=ROOT):
    names = set(m2_source_inputs.source_paths(root)) | MANDATORY_SOURCES
    return sorted(names)


def snapshot_sources(output):
    names = source_names()
    before = {}
    for name in names:
        source = ROOT / name
        target = safe(output / 'sources', name)
        target.parent.mkdir(parents=True, exist_ok=True)
        data = source.read_bytes()
        before[name] = digest(data)
        target.write_bytes(data)
    put(output / 'source-catalog.json', dict(schema=1, paths=names,
        catalog_source_sha256=before['tools/m2_source_inputs.py']))
    return before


def after_sources(before):
    names = sorted(set(before) | set(source_names()))
    return {name: sha(ROOT / name) if (ROOT / name).is_file() else None for name in names}


def freeze_file(source, destination):
    record = dict(origin=str(source), frozen=destination.as_posix(), origin_before=None,
                  frozen_before=None, origin_after=None, frozen_after=None, admission_error=None)
    try:
        record['origin_before'] = sha(source)
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(source, destination)
        record['frozen_before'] = sha(destination)
    except OSError as error:
        record['admission_error'] = dict(type=type(error).__name__, message=str(error))
    return record


def artifact_changes(artifacts, wasm_file_sets=None):
    changes = [name for name, row in artifacts.items() if row['admission_error'] is not None
                  or len({row['origin_before'], row['frozen_before'], row['origin_after'], row['frozen_after']}) != 1
                  or row['origin_before'] is None]
    if wasm_file_sets is not None and wasm_file_sets['before'] != wasm_file_sets['after']:
        changes.append('wasm-file-set')
    return sorted(changes)


def verify_artifact_closure(output, report):
    expected = {}
    if 'native' in report['requested_targets']:
        expected['native'] = 'artifacts/frame_transformer_fixture_probe'
    if 'napi' in report['requested_targets']:
        expected['napi'] = 'artifacts/frame-transformer.node'
        sets = report['wasm_file_sets']
        require(sets['before'] == sorted(set(sets['before'])) and sets['after'] == sorted(set(sets['after'])),
                'invalid Wasm artifact file sets')
        for name in set(sets['before']) | WASM_REQUIRED:
            safe(output / 'artifacts/wasm', name)
            expected['wasm/' + name] = 'artifacts/wasm/' + name
    else:
        require(report['wasm_file_sets'] is None, 'unrequested Wasm file-set evidence')
    require(set(report['artifacts']) == set(expected), 'requested frozen artifact closure differs')
    actual_files = set()
    for name, relative in expected.items():
        row = report['artifacts'][name]
        require(row['frozen'] == str(Path(report['output_directory']) / relative), 'frozen artifact path differs')
        if row['frozen_after'] is not None:
            actual_files.add(relative.removeprefix('artifacts/'))
    require(set(files(output / 'artifacts')) == actual_files, 'frozen artifact file coverage differs')


def layout_issues(output, requested, manifest):
    issues = []
    ids = {case['id'] for case in manifest['cases']}
    repeats = {str(value) for value in range(manifest['repetitions'])}
    for target in TARGETS:
        base = output / ('native' if target == 'native' else 'portable/' + target)
        if not base.exists():
            continue
        if target not in requested:
            issues.append('unrequested target capture: ' + target)
        for case in sorted(base.iterdir()):
            if not case.is_dir() or case.name not in ids:
                issues.append('unexpected target case: ' + target + '/' + case.name)
                continue
            for repeat in sorted(case.iterdir()):
                if not repeat.is_dir() or repeat.name not in repeats:
                    issues.append('unexpected target repeat: ' + target + '/' + case.name + '/' + repeat.name)
    return issues


def pairs(manifest):
    return [(case, repeat) for case in manifest['cases'] for repeat in range(manifest['repetitions'])]


def portable_admission(output, report, manifest):
    issues = []
    portable = None
    path = output / 'portable/report.json'
    if not path.is_file():
        return None, ['portable runner report missing']
    try:
        portable = read(path)
        require(type(portable['schema']) is int and portable['schema'] == 1
                and same_json(portable['caseIds'], [case['id'] for case in manifest['cases']])
                and type(portable['repetitions']) is int and portable['repetitions'] == manifest['repetitions'], 'portable main selection differs')
        require(set(portable['targets']) == {'napi', 'chromium'}, 'portable target coverage differs')
        require(portable['inputsBefore'] == portable['inputsAfter'], 'portable inputs changed during acquisition')
        require(portable['fileSetsBefore'] == portable['fileSetsAfter'], 'portable input file coverage changed')
        require(portable['infrastructureErrors'] == [], 'portable infrastructure failures recorded')
        require(portable['fixtureManifestSha256'] == sha(output / 'fixture/manifest.json'), 'portable fixture identity differs')
        require(same_json(json.loads((output / 'portable/fixture-verification.stdout.bin').read_bytes()),
                report['fixture_verification']), 'portable independent fixture verification differs')
        require(portable['node']['executable'] == report['node_runtime']['executable']
                and portable['inputsBefore'][portable['node']['executable']] == report['node_runtime']['before_sha256'],
                'portable Node runtime identity differs')
        require(isinstance(portable['node']['version'], str) and portable['node']['version'].startswith('v')
                and isinstance(portable['node']['versions'], dict), 'portable Node version identity missing')
        environment_keys = ['PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'TZ', 'TERM',
                            'PLAYWRIGHT_BROWSERS_PATH', 'PKL_FIXTURE_PYTHON']
        require(portable['environment'] == {name: report['environment'][name] for name in environment_keys
                                           if name in report['environment']}, 'portable child environment differs')
        require(portable['argv'] == [report['node_runtime']['executable'],
            str(Path(report['workspace']) / 'tools/platform/capture-frame-transformer.mjs'),
            str(Path(report['output_directory']) / 'portable'), str(Path(report['output_directory']) / 'artifacts/wasm'),
            report['artifacts']['napi']['frozen'], str(Path(report['output_directory']) / 'fixture')],
            'portable runner argv identity differs')
        package = read(output / 'sources/tools/platform/package.json')
        lock = read(output / 'sources/tools/platform/package-lock.json')
        pin = package['devDependencies']['playwright']
        require(isinstance(pin, str) and re.fullmatch(r'\d+\.\d+\.\d+', pin)
                and portable['playwright'] == pin
                and lock['packages']['']['devDependencies']['playwright'] == pin
                and lock['packages']['node_modules/playwright']['version'] == pin
                and lock['packages']['node_modules/playwright-core']['version'] == pin,
                'portable browser tooling pin differs')
        require(portable['browser']['sha256'] == portable['inputsBefore'][portable['browser']['executable']]
                and bool(portable['browser']['version']), 'portable browser runtime identity missing')
        known = {str(Path(report['workspace']) / name): value for name, value in report['sources_before'].items()}
        known.update({str(Path(report['output_directory']) / 'fixture' / name): value['sha256']
                      for name, value in files(output / 'fixture').items()})
        for name, row in report['artifacts'].items():
            if name == 'napi' or name.startswith('wasm/'):
                known[row['frozen']] = row['frozen_before']
        # Driver input paths are retained as original provenance; never read them during offline audit.
        for name, value in portable['inputsBefore'].items():
            if name in known:
                require(value == known[name], 'portable captured input identity differs: ' + name)
            else:
                require(name in {portable['node']['executable'], portable['browser']['executable']},
                        'portable tracked input lacks a source/artifact snapshot: ' + name)
        expected_sets = {}
        for name in ['fixture', 'artifacts/wasm']:
            base = str(Path(report['output_directory']) / name)
            expected_sets[base] = sorted(str(Path(base) / entry) for entry in files(output / name))
        require(portable['fileSetsBefore'] == expected_sets, 'portable frozen input file sets differ')
        required_tracks = {str(Path(report['workspace']) / name) for name in PORTABLE_TRACKED_SOURCES}
        required_tracks.update({report['artifacts']['napi']['frozen'], portable['node']['executable'],
                                portable['browser']['executable']})
        for names in expected_sets.values():
            required_tracks.update(names)
        require(set(portable['inputsBefore']) == required_tracks, 'portable exact tracked-input coverage differs')
        for target in ['napi', 'chromium']:
            require(all(type(row['repeat']) is int for row in portable['targets'][target]), 'portable repeat identity type differs')
            require([(row['id'], row['repeat']) for row in portable['targets'][target]]
                    == [(case['id'], repeat) for case, repeat in pairs(manifest)], 'portable missing/duplicate session')
            for row in portable['targets'][target]:
                expected = str(Path(report['output_directory']) / 'portable' / target / row['id'] / str(row['repeat']))
                require(row['directory'] == expected, 'portable session directory differs')
                require(same_json(read(output / 'portable' / target / row['id'] / str(row['repeat']) / 'process.json'), row),
                        'portable process identity differs')
                if target == 'napi':
                    command = [report['node_runtime']['executable'],
                        str(Path(report['workspace']) / 'tools/platform/capture-frame-transformer.mjs'), '--child',
                        str(Path(expected) / 'input.json'), expected, report['artifacts']['napi']['frozen']]
                    require(row['command'] == command, 'portable child execution command differs')
                    child_path = output / 'portable' / target / row['id'] / str(row['repeat']) / 'child.json'
                    if child_path.is_file():
                        child = read(child_path)
                        require(child['id'] == row['id'] and child['node'] == portable['node']['version']
                                and child['versions'] == portable['node']['versions']
                                and child['executable'] == portable['node']['executable']
                                and child['argv'] == command and child['cwd'] == report['workspace'],
                                'portable child runtime/execution identity differs')
    except (OSError, ValueError, KeyError, TypeError) as error:
        issues.append(str(error))
    return portable, issues


def derive(output, report, manifest, oracle_root):
    targets = {}
    for target in TARGETS:
        if target not in report['requested_targets']:
            targets[target] = dict(status='not-requested', expected_executions=len(pairs(manifest)), rows=[])
            continue
        results = []
        for case, repeat in pairs(manifest):
            relative = (Path('native') if target == 'native' else Path('portable') / target) / case['id'] / str(repeat)
            directory = output / relative
            issues = []
            if target == 'native':
                process_path = directory / 'process/result.json'
                process = read(process_path) if process_path.is_file() else dict(status='not-executed', exit_code=None)
            else:
                process_path = directory / 'process.json'
                try:
                    raw = read(process_path) if process_path.is_file() else None
                    require(raw is None or isinstance(raw, dict), 'invalid portable process record')
                except (OSError, ValueError) as error:
                    issues.append(str(error))
                    raw = None
                if raw is None:
                    issues.append('portable session process record missing')
                executed = raw is not None and (type(raw.get('exitCode')) is int and raw.get('exitCode') == 0 and raw.get('signal') is None
                            and raw.get('error') is None if target == 'napi' else raw.get('acquisitionError') is None)
                process = dict(status='executed' if executed else 'acquisition-failed', exit_code=0 if executed else None)
            if target == 'native':
                for name, expected_bytes in native_inputs(case, output / 'fixture').items():
                    path = directory / name
                    if not path.is_file() or path.read_bytes() != expected_bytes:
                        issues.append('native exact source/query/replacement input differs or is missing: ' + name)
            else:
                try:
                    value = read(directory / 'input.json')
                    expected = {**case, 'sourcePath': case['source'], 'source': (output / 'fixture' / case['source']).read_bytes().decode('utf-8')}
                    require(same_json(value, expected), 'portable exact source/query input differs')
                except (OSError, ValueError) as error:
                    issues.append(str(error))
            comparison = compare_session(oracle_root / 'cases' / case['id'] / str(repeat), directory,
                                         case, target, process, issues)
            results.append(dict(id=case['id'], repeat=repeat, directory=relative.as_posix(), **comparison))
        targets[target] = dict(status='recorded', expected_executions=len(pairs(manifest)),
                               selection_accounted=len(results) == len(pairs(manifest)),
                               statuses=dict(Counter(row['status'] for row in results)), rows=results)
    return targets


def infrastructure_issues(report):
    issues = list(report['layout_issues'])
    if 'napi' in report['requested_targets']:
        process = report['portable_process']
        if process['status'] != 'executed' or type(process['exit_code']) is not int or process['exit_code'] != 0:
            issues.append('portable process failed, timed out or could not launch; raw channels retained')
        if process['stderr']['bytes']:
            issues.append('portable runner emitted stderr; raw bytes retained')
        issues.extend(report['portable_issues'])
        if report['node_runtime']['before_sha256'] != report['node_runtime']['after_sha256']:
            issues.append('Node runtime changed during acquisition')
    return issues


def strict_passes(report):
    return (bool(report['requested_targets']) and not report['source_changes'] and not report['artifact_changes']
            and not report['infrastructure_errors']
            and all(report['targets'][name]['statuses'] == {'pass': len(report['case_ids']) * report['repetitions']}
                    for name in report['requested_targets']))


def verify_report(output):
    report = read(output / 'report.json')
    actual = files(output)
    actual.pop('inventory.json', None)
    require(same_json(read(output / 'inventory.json'), actual), 'comparison evidence inventory differs')
    require(type(report['schema']) is int and report['schema'] == 1 and same_json(report['boundary'], BOUNDARY), 'comparison boundary differs')
    require(report['requested_targets'] in [['native'], ['napi', 'chromium'], ['native', 'napi', 'chromium']], 'invalid requested target profile')
    require(report['source_binding'] == 'supplied-artifacts-with-source-snapshot; no recorded-build claim', 'candidate build provenance differs')
    require(MANDATORY_SOURCES.issubset(report['sources_before']), 'mandatory source catalog paths missing')
    catalog = read(output / 'source-catalog.json')
    require(type(catalog['schema']) is int and catalog['schema'] == 1
            and catalog['paths'] == sorted(report['sources_before'])
            and catalog['catalog_source_sha256'] == report['sources_before']['tools/m2_source_inputs.py']
            and sha(output / 'source-catalog.json') == report['source_catalog_sha256'], 'captured source catalog binding differs')
    require(set(files(output / 'sources')) == set(report['sources_before']), 'source snapshot file coverage differs')
    require(source_names(output / 'sources') == catalog['paths'], 'captured source catalog closure differs')
    for name, value in report['sources_before'].items():
        require(sha(safe(output / 'sources', name)) == value, 'source snapshot identity differs')
    require(report['source_changes'] == sorted(name for name in report['sources_before'].keys() | report['sources_after'].keys()
            if report['sources_before'].get(name) != report['sources_after'].get(name)), 'source change classification differs')
    for row in report['artifacts'].values():
        relative = Path(row['frozen']).relative_to(report['output_directory']).as_posix()
        path = safe(output, relative)
        require((sha(path) if path.is_file() else None) == row['frozen_after'], 'frozen artifact identity differs')
    verify_artifact_closure(output, report)
    require(report['artifact_changes'] == artifact_changes(report['artifacts'], report['wasm_file_sets']), 'artifact change classification differs')
    with authority(output / 'fixture') as (manifest, verification, oracle_root):
        require(same_json(report['fixture_verification'], verification)
                and same_json(report['case_ids'], [case['id'] for case in manifest['cases']])
                and type(report['repetitions']) is int and report['repetitions'] == manifest['repetitions'], 'independent main Java selection differs')
        require(same_json(report['buffer_supplement'], dict(status='not-executed', authority=verification['buffer_supplement'],
                obligation=BOUNDARY['buffer_supplement'])), 'supplemental Java test observations were omitted or promoted')
        require(report['layout_issues'] == layout_issues(output, report['requested_targets'], manifest), 'target capture directory coverage differs')
        for case, repeat in pairs(manifest):
            if 'native' not in report['requested_targets']:
                break
            directory = output / 'native' / case['id'] / str(repeat)
            process = read(directory / 'process/result.json')
            verify_process(directory / 'process', process)
            expected = native_arguments(report['artifacts']['native']['frozen'],
                Path(report['output_directory']) / 'native' / case['id'] / str(repeat), case)
            require(process['command'] == expected and process['cwd'] == report['workspace']
                    and process['environment'] == report['environment'], 'native execution contract differs')
            require((directory / 'protocol.bin').read_bytes() == (directory / 'process/stdout.bin').read_bytes(),
                    'native protocol is not actual process stdout')
        if 'napi' in report['requested_targets']:
            process = read(output / 'portable-process/result.json')
            verify_process(output / 'portable-process', process)
            require(process == report['portable_process'], 'portable process provenance differs')
            command = [report['node_runtime']['executable'],
                str(Path(report['workspace']) / 'tools/platform/capture-frame-transformer.mjs'),
                str(Path(report['output_directory']) / 'portable'), str(Path(report['output_directory']) / 'artifacts/wasm'),
                report['artifacts']['napi']['frozen'], str(Path(report['output_directory']) / 'fixture')]
            require(process['command'] == command and process['cwd'] == report['workspace']
                    and process['environment'] == report['environment'], 'portable runner execution contract differs')
            _, issues = portable_admission(output, report, manifest)
            require(report['portable_issues'] == issues, 'portable acquisition admission differs')
        require(same_json(report['targets'], derive(output, report, manifest, oracle_root)), 'raw comparison/result accounting differs')
    require(report['infrastructure_errors'] == infrastructure_issues(report), 'infrastructure failure accounting differs')
    require(type(report['strict_main_comparison_passed']) is bool and report['strict_main_comparison_passed'] == strict_passes(report), 'strict full main comparison verdict differs')
    require(report['status'] == ('passed' if strict_passes(report) else 'failed'), 'comparison status differs')
    return report


def execute(args):
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    fixture = args.fixture.resolve()
    fixtures.verify(fixture)
    shutil.copytree(fixture, output / 'fixture')
    environment = {name: os.environ[name] for name in ['PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL',
        'TZ', 'TERM', 'PLAYWRIGHT_BROWSERS_PATH'] if name in os.environ}
    environment.update(PKL_FIXTURE_PYTHON=sys.executable, PYTHONDONTWRITEBYTECODE='1')
    report = dict(schema=1, boundary=BOUNDARY, output_directory=str(output), workspace=str(ROOT), argv=sys.argv,
        host=dict(platform=platform.platform(), uname=dict(platform.uname()._asdict())), environment=environment,
        python=dict(executable=sys.executable, version=sys.version, sha256=sha(Path(sys.executable).resolve())),
        source_binding='supplied-artifacts-with-source-snapshot; no recorded-build claim',
        requested_targets=(['native'] if args.native else []) + (['napi', 'chromium'] if args.wasm else []),
        sources_before=snapshot_sources(output), artifacts={}, wasm_file_sets=None,
        infrastructure_errors=[], portable_issues=[], layout_issues=[])
    report['source_catalog_sha256'] = sha(output / 'source-catalog.json')
    with authority(output / 'fixture') as (manifest, verification, oracle_root):
        report.update(fixture_verification=verification, case_ids=[case['id'] for case in manifest['cases']],
            repetitions=manifest['repetitions'], buffer_supplement=dict(status='not-executed',
                authority=verification['buffer_supplement'], obligation=BOUNDARY['buffer_supplement']))
        if args.native:
            native = output / 'artifacts/frame_transformer_fixture_probe'
            report['artifacts']['native'] = freeze_file(args.native.resolve(), native)
            for case, repeat in pairs(manifest):
                directory = output / 'native' / case['id'] / str(repeat)
                directory.mkdir(parents=True)
                for name, value in native_inputs(case, output / 'fixture').items():
                    path = directory / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(value)
                capture(native_arguments(native, directory, case),
                        ROOT, environment, directory / 'process', args.timeout)
                (directory / 'protocol.bin').write_bytes((directory / 'process/stdout.bin').read_bytes())
        if args.wasm:
            wasm = args.wasm.resolve()
            wasm_before = sorted(files(wasm))
            for name in sorted(set(wasm_before) | WASM_REQUIRED):
                report['artifacts']['wasm/' + name] = freeze_file(wasm / name, output / 'artifacts/wasm' / name)
            report['artifacts']['napi'] = freeze_file(args.napi.resolve(), output / 'artifacts/frame-transformer.node')
            node = shutil.which(args.node)
            node = str(Path(node).resolve()) if node else args.node
            report['node_runtime'] = dict(executable=node, before_sha256=sha(Path(node)) if Path(node).is_file() else None)
            command = [node, str(ROOT / 'tools/platform/capture-frame-transformer.mjs'), str(output / 'portable'),
                       str(output / 'artifacts/wasm'), str(output / 'artifacts/frame-transformer.node'), str(output / 'fixture')]
            report['portable_process'] = capture(command, ROOT, environment, output / 'portable-process', args.portable_timeout)
            report['node_runtime']['after_sha256'] = sha(Path(node)) if Path(node).is_file() else None
            report['wasm_file_sets'] = dict(before=wasm_before, after=sorted(files(wasm)))
            _, report['portable_issues'] = portable_admission(output, report, manifest)
        report['targets'] = derive(output, report, manifest, oracle_root)
        report['layout_issues'] = layout_issues(output, report['requested_targets'], manifest)
        report['infrastructure_errors'] = infrastructure_issues(report)
    report['sources_after'] = after_sources(report['sources_before'])
    report['source_changes'] = sorted(name for name in report['sources_before'].keys() | report['sources_after'].keys()
        if report['sources_before'].get(name) != report['sources_after'].get(name))
    for row in report['artifacts'].values():
        row['origin_after'] = sha(Path(row['origin'])) if Path(row['origin']).is_file() else None
        row['frozen_after'] = sha(Path(row['frozen'])) if Path(row['frozen']).is_file() else None
    report['artifact_changes'] = artifact_changes(report['artifacts'], report['wasm_file_sets'])
    report['strict_main_comparison_passed'] = strict_passes(report)
    report['status'] = 'passed' if report['strict_main_comparison_passed'] else 'failed'
    put(output / 'report.json', report)
    put(output / 'inventory.json', files(output))
    return verify_report(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument('--output', type=Path)
    mode.add_argument('--verify-report', type=Path, help='offline evidence integrity; retains every failed comparison')
    mode.add_argument('--verify-full', type=Path, help='strict complete main matrix on every requested target')
    parser.add_argument('--native', type=Path)
    parser.add_argument('--wasm', type=Path)
    parser.add_argument('--napi', type=Path)
    parser.add_argument('--fixture', type=Path, default=fixtures.DEFAULT)
    parser.add_argument('--node', default='node')
    parser.add_argument('--timeout', type=float, default=30)
    parser.add_argument('--portable-timeout', type=float, default=1200)
    parser.add_argument('--record-only', action='store_true')
    args = parser.parse_args()
    if args.output and (not (args.native or args.wasm) or bool(args.wasm) != bool(args.napi)):
        parser.error('supply --native and/or paired --wasm/--napi')
    if not all(math.isfinite(value) and value > 0 for value in [args.timeout, args.portable_timeout]):
        parser.error('timeouts must be finite positive values')
    if args.record_only and not args.output:
        parser.error('--record-only requires --output')
    try:
        report = execute(args) if args.output else verify_report((args.verify_report or args.verify_full).resolve())
        print(json.dumps(dict(integrity='verified', status=report['status'],
            strict_main_comparison_passed=report['strict_main_comparison_passed'],
            targets={target: {key: value for key, value in row.items() if key != 'rows'}
                     for target, row in report['targets'].items()}, buffer_supplement=report['buffer_supplement'])))
        return 0 if args.verify_report or args.record_only or report['strict_main_comparison_passed'] else 1
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('Frame transformer comparison failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
