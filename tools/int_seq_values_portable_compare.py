#!/usr/bin/env python3
"""Admit IntSeq Node/Chromium captures against sealed Java string bytes."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import tarfile

import int_seq_values_fixtures
import int_seq_values_compare as native
import data_size_portable_compare as shared
import m1_platforms

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-int-seq-values'
CAPTURE = 'tools/platform/capture-int-seq-values.mjs'
JAVA_PROVENANCE_SCRIPT = "import json,sys,tarfile; sys.path.insert(0,'tools'); import int_seq_values_fixtures as authority; archive=tarfile.open(sys.argv[1],'r:gz'); member=archive.getmember(authority.ARCHIVE_PREFIX+'/provenance.json'); assert member.isfile(); data=json.load(archive.extractfile(member)); print(json.dumps({'environment':data['environment'],'sessions':[row['command'] for row in data['sessions']],'jdk_properties':data['jdk']['properties']},sort_keys=True))"
CAPTURE_SOURCES = {
    CAPTURE, 'tools/platform/capture-int-seq-values.test.mjs',
    'tools/platform/data-size-session.mjs', 'tools/platform/data-size-session.test.mjs',
    'tools/platform/module-policy-session.mjs', 'tools/platform/resource-session.mjs',
    'tools/platform/package.json', 'tools/platform/package-lock.json',
    'tools/int_seq_values_fixtures.py', 'tools/int_seq_values_oracle.py',
    'tools/int_seq_values_portable_compare.py', 'tools/int_seq_values_compare.py',
    'tools/tests/test_int_seq_values_portable_compare.py',
    'tools/tests/test_int_seq_values_fixtures.py',
    'tools/data_size_portable_compare.py', 'tools/data_size_compare.py',
    'tools/data_size_fixtures.py', 'tools/data_size_oracle.py',
    'tools/m1_platforms.py', 'tools/m1_semantics.py', 'tools/oracle.py', 'tools/m2_source_inputs.py',
    'tools/java/IntSeqValuesOracle.java',
}
COMMON = shared.COMMON
EVENT_META = shared.EVENT_META
API_META = shared.API_META
JAVA_ONLY = shared.JAVA_ONLY
EMBEDDED = shared.EMBEDDED | {'crates/pkl-runtime/src/stdlib/math.pkl'}
classify = shared.classify
validate_trace = shared.validate_trace


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(path):
    raw = Path(path).read_bytes()
    return {'bytes': len(raw), 'sha256': sha(raw)}


def read(path):
    return json.loads(Path(path).read_bytes())


def exact_files(root):
    names = set()
    for path in root.rglob('*'):
        need(not path.is_symlink(), 'symlink in capture: ' + str(path))
        if path.is_file():
            names.add(path.relative_to(root).as_posix())
        else:
            need(path.is_dir(), 'nonregular capture entry: ' + str(path))
    return names


def canonical_input(fixture):
    verdict = int_seq_values_fixtures.verify(fixture)
    cases = read(fixture / 'cases.json')
    need(len(cases['rows']) == 237 and cases['repetitions'] == 2 and
         cases['module_uri'] == 'file:///stage61/intseq-values.pkl' and
         cases['boundary'] == 'string', 'IntSeq fixture selection differs')
    hashes = {name: sha((fixture / name).read_bytes()) for name in
              ('cases.json', 'module.pkl', 'queries.tsv')}
    return cases, hashes, verdict


def java_members(fixture):
    with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as archive:
        return {member.name[len(int_seq_values_fixtures.ARCHIVE_PREFIX) + 1:]: archive.extractfile(member).read()
                for member in archive if member.isfile() and member.name.startswith(int_seq_values_fixtures.ARCHIVE_PREFIX + '/')}


def derive_java_execution(provenance):
    environment = provenance.get('environment')
    commands = provenance.get('sessions')
    properties = provenance.get('jdk_properties')
    need(type(environment) is dict and environment and
         all(type(value) is str for value in environment.values()),
         'invalid Java session environment')
    need(type(commands) is list and len(commands) == 2 and
         all(type(command) is list and all(type(value) is str for value in command)
             for command in commands), 'invalid Java session commands')
    need(type(properties) is dict and
         all(type(properties.get(name)) is str for name in ('user.language', 'user.country')),
         'invalid Java preflight identity')

    def overrides(command):
        values = {}
        for name in ('language', 'country', 'timezone'):
            matches = [arg.removeprefix('-Duser.' + name + '=') for arg in command
                       if arg.startswith('-Duser.' + name + '=')]
            need(len(matches) == 1 and matches[0],
                 'missing or duplicate Java session override: ' + name)
            values[name] = matches[0]
        return values

    first, second = map(overrides, commands)
    need(first == second and re.fullmatch('[a-z]{2,3}', first['language']) and
         re.fullmatch('[A-Z]{2}', first['country']) and
         environment.get('TZ') == first['timezone'],
         'Java session locale or timezone differs')
    return {'environment': environment, 'sessionOverrides': first,
            'locale': first['language'] + '-' + first['country'],
            'timezoneId': first['timezone'],
            'preflightLocale': properties['user.language'] + '-' + properties['user.country']}


def verify_m1(m1, capture):
    report = read(m1 / 'report.json')
    hashes = read(m1 / 'sources.json')
    need(report['source_stability'] is True and report['source_hashes'] == hashes,
         'M1 source stability/report differs')
    names = {p.relative_to(m1 / 'sources').as_posix() for p in
             (m1 / 'sources').rglob('*') if p.is_file()}
    need(names == set(hashes) and EMBEDDED <= names and
         names == {p.relative_to(m1 / 'sources').as_posix() for p in
                                m1_platforms.source_paths(m1 / 'sources')},
         'M1 source closure differs')
    for name, digest in hashes.items():
        need(sha((m1 / 'sources' / name).read_bytes()) == digest,
             'M1 source snapshot differs: ' + name)
    command_rows = read(m1 / 'commands.json')
    commands = {row['name']: row for row in command_rows}
    need(len(commands) == len(command_rows) and
         {'napi-build', 'wasm-build', 'wasm-bindgen'} <= set(commands) and
         all(commands[name]['exit_code'] == 0 for name in
             ('napi-build', 'wasm-build', 'wasm-bindgen')) and
         commands['wasm-bindgen']['command'][1:4] ==
             ['--target', 'web', '--out-dir'] and
         commands['wasm-bindgen']['command'][4] == str(m1 / 'wasm') and
         commands['napi-build']['command'] ==
             ['cargo', 'build', '-p', 'pkl-napi', '--release', '--locked'] and
         commands['wasm-build']['command'] ==
             ['cargo', 'build', '-p', 'pkl-wasm', '--target', 'wasm32-unknown-unknown',
              '--release', '--locked'], 'M1 build commands/status differ')
    need(report['experiment_contract_status'] == 'pass' and
         report['compatibility_status'] == 'fail' and
         set(report['known_gaps']) == {'regex-unicode-15-vs-16',
                                       'regex-zero-width-supplementary'} and
         isinstance(report['total'], int) and report['total'] > 2 and
         report['shared_engine_passed'] == report['total'] and
         all(isinstance(report['oracle_passed'][target], int) and
             0 <= report['oracle_passed'][target] <= report['total'] and
             report['oracle_passed'][target] == report['total'] - 2
             for target in ('rust', 'node', 'browser')),
         'M1 bounded contract or known regex failures differ')
    for name in ('napi-build', 'wasm-build', 'wasm-bindgen'):
        need((m1 / f'{name}.stdout.bin').is_file() and
             (m1 / f'{name}.stderr.bin').is_file(),
             'M1 build process channels missing: ' + name)
    artifacts = {'napi': m1 / 'pkl_napi.node', 'wasm_js': m1 / 'wasm/pkl_wasm.js',
                 'wasm_bg': m1 / 'wasm/pkl_wasm_bg.wasm'}
    tracked = capture['inputsBefore']
    for path in artifacts.values():
        need(report['artifacts'][path.relative_to(m1).as_posix()] ==
             sha(path.read_bytes()) == tracked[str(path)],
             'M1 build/capture artifact identity differs: ' + str(path))
    wasm_files = capture['fileSetsBefore'][str(m1 / 'wasm')]
    need(capture['artifacts']['napi']['sha256'] == tracked[str(artifacts['napi'])] and
         {row['path']: row['sha256'] for row in capture['artifacts']['wasm']['files']} ==
         {path: tracked[path] for path in wasm_files} and
         {str(artifacts['wasm_js']), str(artifacts['wasm_bg'])} <= set(wasm_files),
         'capture artifact file list differs')
    return {'report': identity(m1 / 'report.json'),
            'source_count': len(names),
            'artifacts': {name: identity(path) for name, path in artifacts.items()}}


def verify_source_binding(m1, native_build, wrapper):
    """Bind both platform artifacts to the recorded native production sources."""
    native_build, wrapper = Path(native_build).resolve(), Path(wrapper).resolve()
    native_artifacts = native.verify_build(native_build)
    native_sources = read(native_build / 'source-before.json')
    m1_hashes = read(m1 / 'sources.json')
    before = read(wrapper / 'source-before.json')
    after = read(wrapper / 'source-after.json')
    report = read(wrapper / 'report.json')
    need(before == after and report['source_stable'] is True and report['complete'] is True,
         'platform wrapper source stability differs')
    copied = {name: identity(wrapper / 'sources' / name)
              for name in exact_files(wrapper / 'sources')}
    need(copied == before, 'platform wrapper source snapshot differs')
    required = set(native_sources) | set(m1_hashes) | CAPTURE_SOURCES | {'.cargo/config.toml'}
    need(required <= set(before), 'platform wrapper source closure incomplete')
    production = {name for name in m1_hashes
                  if (name.startswith('crates/') and name.endswith(('.rs', '.pkl')))
                  or name.endswith('/Cargo.toml')}
    production.update({'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml'})
    need(production <= set(native_sources), 'M1 production source absent from native build')
    need(all(before[name] == recorded for name, recorded in native_sources.items()),
         'native/platform source revision differs')
    need(all(before[name]['sha256'] == digest and
             sha((m1 / 'sources' / name).read_bytes()) == digest
             for name, digest in m1_hashes.items()),
         'M1/platform source revision differs')
    need(all(before[name] == identity(ROOT / name) for name in CAPTURE_SOURCES),
         'portable helper source revision differs from platform build')
    need('.cargo/config.toml' in native_sources and
         before['.cargo/config.toml'] == native_sources['.cargo/config.toml'],
         'Cargo configuration source revision differs')
    commands = report['commands']
    expected = ['python3', 'tools/m1_platforms.py', '--output', str(m1), '--verify-contract']
    need(report['cwd'] == str(ROOT) and isinstance(commands, list) and commands
         and commands[0]['name'] == 'm1-contract'
         and sum(row['name'] == 'm1-contract' for row in commands) == 1
         and commands[0]['argv'] == expected and commands[0]['exit_code'] == 0
         and report['environment']['CARGO_INCREMENTAL'] == '0'
         and report['environment']['CARGO_PROFILE_DEV_DEBUG'] == '0',
         'platform wrapper M1 build command differs')
    for channel in ('stdout', 'stderr'):
        need(identity(wrapper / f'm1-contract.{channel}.bin') == commands[0][channel],
             'platform wrapper M1 build channel differs')
    need(commands[0]['started_epoch'] < commands[0]['finished_epoch']
         and commands[0]['pid'] > 0, 'platform wrapper M1 process boundary differs')
    return {'wrapper': identity(wrapper / 'report.json'),
            'native_build': identity(native_build / 'build.json'),
            'native_artifacts': native_artifacts,
            'shared_source_count': len(set(native_sources) & set(m1_hashes)),
            'wrapper_source_count': len(before)}


def verify_java_environment(capture_dir, fixture, report, java):
    provenance = json.loads(java['provenance.json'])
    java_payload = {'environment': provenance['environment'],
                    'sessions': [row['command'] for row in provenance['sessions']],
                    'jdk_properties': provenance['jdk']['properties']}
    execution = derive_java_execution(java_payload)
    extraction = report.get('javaProvenanceExtraction')
    need(type(extraction) is dict and
         extraction.get('command') == [report['python']['executable'], '-c',
                                      JAVA_PROVENANCE_SCRIPT,
                                      str(fixture / 'oracle-evidence.tar.gz')] and
         extraction.get('cwd') == str(ROOT) and extraction.get('exitCode') == 0 and
         extraction.get('signal') is None and extraction.get('error') is None,
         'Java provenance extraction command differs')
    stdout = (capture_dir / 'java-provenance.stdout.bin').read_bytes()
    stderr = (capture_dir / 'java-provenance.stderr.bin').read_bytes()
    need(stdout == (json.dumps(java_payload, sort_keys=True) + '\n').encode() and stderr == b'' and
         extraction['stdoutSha256'] == sha(stdout) and
         extraction['stderrSha256'] == sha(stderr) and
         report.get('javaExecution') == execution,
         'Java provenance extraction bytes or effective execution differs')
    return execution


def verify_target_environment(execution, target, observation, child=None, platform=None):
    if target == 'napi':
        actual = child.get('environment') if type(child) is dict else None
        expected = execution['environment']
        need(type(actual) is dict and
             all(actual.get(name) == value for name, value in expected.items()),
             'N-API actual child environment differs from Java')
        extra = set(actual) - set(expected)
        need(not extra or (platform == 'darwin' and extra == {'__CF_USER_TEXT_ENCODING'}
                            and type(actual['__CF_USER_TEXT_ENCODING']) is str
                            and actual['__CF_USER_TEXT_ENCODING']),
             'N-API unexpected child environment differs from Java')
        return {name: actual[name] for name in sorted(extra)}
    else:
        need(observation.get('contextObserved') == {
            'locale': execution['locale'], 'timezoneId': execution['timezoneId']},
            'Chromium observed locale or timezone differs from Java')
        return {}


def verify_capture(capture_dir, m1, fixture, cases, hashes, java):
    report = read(capture_dir / 'report.json')
    execution = verify_java_environment(capture_dir, fixture, report, java)
    need(report['schema'] == 1 and report['complete'] is True and
         report['repetitions'] == 2 and report['orderedRows'] == 237 and
         report['fixture'] == str(fixture) and
         report['caseIds'] == [row['id'] for row in cases['rows']] and
         report['inputsBefore'] == report['inputsAfter'] and
         report['fileSetsBefore'] == report['fileSetsAfter'] and
         report['infrastructureErrors'] == [] and
         report['fixtureInputs'] == hashes and
         report['fullPublicApiComparisonPassed'] is False and
         report['performanceClaim'] is False,
         'capture identity, stability, or scope differs')
    need(set(report['targets']) == {'napi', 'chromium'} and
         all([row['repeat'] for row in report['targets'][target]] == [0, 1]
             for target in ('napi', 'chromium')),
         'target/repetition coverage differs')
    tracked = report['inputsBefore']
    required = {str(fixture / name) for name in ('cases.json', 'module.pkl', 'queries.tsv')}
    required |= {str(ROOT / name) for name in CAPTURE_SOURCES}
    required |= {str(m1 / name) for name in ('pkl_napi.node', 'wasm/pkl_wasm.js',
                                             'wasm/pkl_wasm_bg.wasm')}
    required |= {report['node']['executable'], report['python']['executable'],
                 report['playwright']['browserExecutable'], report['playwright']['package'],
                 report['playwright']['corePackage']}
    need(required <= set(tracked), 'capture source/runtime closure incomplete')
    for path, digest in tracked.items():
        need(sha(Path(path).read_bytes()) == digest, 'capture input changed: ' + path)
    need(set(report['fileSetsBefore']) == {str(fixture), str(m1 / 'wasm')},
         'captured directory selection differs')
    for folder, names in report['fileSetsBefore'].items():
        need(names == sorted(str(p) for p in Path(folder).rglob('*') if p.is_file()),
             'capture directory changed: ' + folder)
    need(report['node']['executableSha256'] == tracked[report['node']['executable']] and
         report['node']['executableRealpath'] ==
             str(Path(report['node']['executable']).resolve()) and
         report['python']['executableSha256'] == tracked[report['python']['executable']] and
         report['python']['executableRealpath'] ==
             str(Path(report['python']['executable']).resolve()) and
         report['playwright']['browserExecutableSha256'] ==
             tracked[report['playwright']['browserExecutable']] and
         bool(report['playwright']['browserVersion']),
         'portable runtime executable identity differs')
    shared.verify_http_requests(report['httpRequests'])
    need(report['playwright']['launchOptions'] == {
        'headless': True, 'executablePath': report['playwright']['browserExecutable'],
        'env': execution['environment']} and
         report['playwright']['contextOptions'] == {
             'locale': execution['locale'], 'timezoneId': execution['timezoneId']} and
         report['playwright']['browserExecutableRealpath'] ==
         str(Path(report['playwright']['browserExecutable']).resolve()),
         'Chromium launch environment or context options differ')
    package = read(ROOT / 'tools/platform/package.json')
    lock = read(ROOT / 'tools/platform/package-lock.json')
    pin = package['devDependencies']['playwright']
    need(report['playwright']['pin'] == pin ==
         lock['packages']['']['devDependencies']['playwright'] ==
         lock['packages']['node_modules/playwright']['version'] ==
         lock['packages']['node_modules/playwright-core']['version'],
         'Playwright pin differs')
    verifier = report['fixtureVerification']
    need(verifier['exitCode'] == 0 and verifier['signal'] is None and
         verifier['error'] is None and
         verifier['command'][1:] == [str(ROOT / 'tools/int_seq_values_fixtures.py'),
                                     '--fixture', str(fixture)] and
         verifier['stdoutSha256'] == sha((capture_dir / 'fixture-verification.stdout.bin').read_bytes()) and
         verifier['stderrSha256'] == sha((capture_dir / 'fixture-verification.stderr.bin').read_bytes()) and
         (capture_dir / 'fixture-verification.stderr.bin').read_bytes() == b'',
         'independent fixture verification differs')
    need(read(capture_dir / 'fixture-verification.stdout.bin')['rows'] == 237,
         'fixture authority outcome differs')
    return report


def compare_target(capture_dir, report, java, cases, fixture, hashes, target):
    rows, sessions = [], []
    for repeat, observation in enumerate(report['targets'][target]):
        path = capture_dir / target / f'repeat-{repeat}'
        need(observation['repeat'] == repeat and observation['directory'] == str(path) and
             observation['uri'] == cases['module_uri'] and observation['rowCount'] == 237,
             'session identity differs')
        process = read(path / 'process.json')
        need({k: v for k, v in observation.items() if k != 'fileSet'} == process,
             'process record differs')
        need(read(path / 'input.json') ==
             {'uri': cases['module_uri'], 'source': (fixture / 'module.pkl').read_text(),
              'rows': [{'id': row['id'], 'expression': row['expression']}
                       for row in cases['rows']], 'inputHashes': hashes},
             'session input differs')
        expected = {'input.json', 'candidate-trace.json', 'process.json',
                    'process.stdout.bin', 'process.stderr.bin'}
        if target == 'napi':
            expected.add('child.json')
        statuses = []
        for i, row in enumerate(cases['rows']):
            name = f'{i:03}-{row["id"]}'
            folder = path / name
            status = (folder / 'result.status.bin').read_bytes()
            need(status in (b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'),
                 'candidate status invalid')
            clean = status.decode('ascii').strip()
            statuses.append(clean)
            expected.update(name + '/' + channel for channel in
                            (set(COMMON) | (API_META if clean == 'api-exception' else EVENT_META)))
            reference = {channel: java[f'repeat-{repeat}/{name}/{channel}']
                         for channel in set(COMMON) | JAVA_ONLY}
            verdict = compare_row(reference, folder)
            rows.append({'repeat': repeat, 'id': row['id'], **verdict})
        need(exact_files(path) == expected and
             len(expected) == observation['fileSet']['fileCount'] and
             Counter(statuses) == observation['fileSet']['statusCounts'] and
             observation['fileSet']['rowCount'] == 237,
             'exact session file/status inventory differs')
        for channel in ('stdout', 'stderr'):
            raw = (path / f'process.{channel}.bin').read_bytes()
            need(sha(raw) == observation['fileSet'][f'process{channel.title()}Sha256'],
                 'process channel identity differs')
            if target == 'napi':
                need(sha(raw) == observation[f'process{channel.title()}Sha256'],
                     'N-API OS process channel differs')
        architecture = validate_trace(path, statuses,
                                      {'uri': cases['module_uri'],
                                       'source': (fixture / 'module.pkl').read_text(),
                                       'rows': cases['rows']}, target)
        issues = []
        if (path / 'process.stdout.bin').read_bytes() != java[f'repeat-{repeat}/process.stdout.bin']:
            issues.append('whole protocol differs')
        if (path / 'process.stderr.bin').read_bytes() != b'':
            issues.append('portable process error channel nonempty')
        if target == 'napi':
            child = read(path / 'child.json')
            environment_extra = verify_target_environment(
                report['javaExecution'], target, observation, child, report['node']['platform'])
            need(observation['exitCode'] == 0 and observation['signal'] is None and
                 observation['error'] is None and child['architecture'] == architecture and
                 child['acquisitionError'] is None and child['pid'] > 0 and
                 child['executable'] == report['node']['executable'] and
                 child['node'] == report['node']['version'] and
                 child['versions'] == report['node']['versions'] and
                 child['cwd'] == str(ROOT) and
                 child['argv'] == [report['node']['executable'], str(ROOT / CAPTURE), '--child',
                                   str(path / 'input.json'), str(path),
                                   report['artifacts']['napi']['path']] and
                 observation['command'] == child['argv'],
                 'N-API fresh process identity differs')
            sessions.append({'repeat': repeat, 'pid': child['pid'],
                             'environment_extra': environment_extra, 'issues': issues})
        else:
            verify_target_environment(report['javaExecution'], target, observation)
            need(observation['protocolOrigin'] == 'browser-page-returned-UTF8' and
                 observation['stderrOrigin'] == 'browser-pageerror-and-console-error-capture' and
                 observation['architecture'] == architecture and
                 observation['acquisitionError'] is None and
                 observation['browserErrors'] == [],
                 'Chromium context/protocol boundary differs')
            sessions.append({'repeat': repeat, 'issues': issues})
    if target == 'napi':
        need(sessions[0]['pid'] != sessions[1]['pid'], 'N-API repetitions reused child')
    counts = dict(Counter(row['status'] for row in rows))
    return {'rows': rows, 'sessions': sessions, 'counts': counts,
            'strict_string_passed': strict_target(rows, sessions)}


def strict_target(rows, sessions):
    return (len(rows) == 474 and all(row['status'] == 'pass' for row in rows) and
            len(sessions) == 2 and all(not session['issues'] for session in sessions))


def compare_row(reference, candidate):
    status = (candidate / 'result.status.bin').read_bytes()
    metadata = API_META if status == b'api-exception\n' else EVENT_META
    files = {p.name for p in candidate.iterdir()} if candidate.is_dir() else set()
    issues = []
    if files != set(COMMON) | metadata:
        issues.append('candidate row channel set differs')
    if status == b'error\n' and (candidate / 'candidate.disposition.bin').read_bytes() != b'language-error':
        issues.append('ordinary error disposition differs')
    if status == b'candidate-error\n' and (candidate / 'candidate.disposition.bin').read_bytes() == b'language-error':
        issues.append('candidate failure mislabeled ordinary')
    if status == b'success\n' and (candidate / 'candidate.disposition.bin').read_bytes():
        issues.append('success has error disposition')
    differences = [name for name in COMMON if (candidate / name).read_bytes() != reference[name]]
    return {'status': 'pass' if not issues and not differences else 'mismatch',
            'channels': differences, 'issues': issues}


def verify(capture_dir, m1, native_build, build_wrapper, fixture=FIXTURE):
    capture_dir, m1, fixture = (Path(path).resolve() for path in (capture_dir, m1, fixture))
    source_paths = sorted(CAPTURE_SOURCES | {'tools/int_seq_values_portable_compare.py',
                                            'tools/int_seq_values_compare.py',
                                            'tools/tests/test_int_seq_values_compare.py'})
    source_before = {name: identity(ROOT / name) for name in source_paths}
    cases, hashes, authority = canonical_input(fixture)
    java = java_members(fixture)
    capture = verify_capture(capture_dir, m1, fixture, cases, hashes, java)
    need(read(capture_dir / 'fixture-verification.stdout.bin') == authority,
         'capture fixture verdict differs from Java authority')
    binding = verify_m1(m1, capture)
    source_binding = verify_source_binding(m1, native_build, build_wrapper)
    result = {'schema': 1, 'integrity': 'passed', 'authority': authority,
              'm1_binding': binding,
              'source_binding': source_binding,
              'targets': {target: compare_target(capture_dir, capture, java, cases,
                                                 fixture, hashes, target)
                          for target in ('napi', 'chromium')},
              'producer_error_class': 'Java exception type, toString and cause fields are producer metadata; the candidate string boundary does not synthesize them',
              'typed_export_status': 'not-executed-required',
              'full_public_api_comparison_passed': False,
              'full_m2_complete': False, 'performance_targets_achieved': False}
    need(source_before == {name: identity(ROOT / name) for name in source_paths},
         'portable comparator sources changed during verification')
    result['verifier_sources'] = source_before
    result['strict_string_passed'] = all(
        result['targets'][target]['strict_string_passed'] for target in ('napi', 'chromium'))
    need(exact_files(capture_dir) ==
         {'report.json', 'fixture-verification.stdout.bin',
          'fixture-verification.stderr.bin', 'java-provenance.stdout.bin',
          'java-provenance.stderr.bin'} |
         {f'{target}/repeat-{repeat}/{name}' for target in ('napi', 'chromium')
          for repeat in range(2)
          for name in exact_files(capture_dir / target / f'repeat-{repeat}')},
         'capture top-level inventory differs')
    return result


def gate_exit(result, gate):
    need(gate in ('integrity', 'string', 'full-api'), 'unknown gate')
    return 0 if gate == 'integrity' or (gate == 'string' and result['strict_string_passed']) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--m1', type=Path, required=True,
                        help='Supplied M1 platform report directory with build artifacts')
    parser.add_argument('--native-build', type=Path, required=True,
                        help='Recorded native Rust build directory')
    parser.add_argument('--build-wrapper', type=Path, required=True,
                        help='Recorded source-stable M1 build wrapper directory')
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    parser.add_argument('--gate', choices=('integrity', 'string', 'full-api'), default='string')
    args = parser.parse_args()
    result = verify(args.capture, args.m1, args.native_build, args.build_wrapper, args.fixture)
    print(json.dumps(result, sort_keys=True))
    return gate_exit(result, args.gate)


if __name__ == '__main__':
    raise SystemExit(main())
