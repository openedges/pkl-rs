#!/usr/bin/env python3
"""Admit 40 separate-module Node/Chromium captures against sealed Java bytes."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import tarfile

import collection_conversions_compare as native
import collection_conversions_portable_compare as shared_collection
import data_size_portable_compare as shared
import m1_platforms
import union_marker_fixtures as authority
from union_marker_oracle import MODULE_URI, PROGRAMS_SHA256

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-union-marker-diagnostics'
CAPTURE = 'tools/platform/capture-union-marker-modules.mjs'
EXTRACTION_SCRIPT = ("import json,sys,tarfile; from pathlib import Path; p=Path(sys.argv[1]); "
                     "a=tarfile.open(p/'oracle-evidence.tar.gz','r:gz'); "
                     "d=json.load(a.extractfile(a.getmember('capture-01/provenance.json'))); "
                     "print(json.dumps({'environment':d['environment'],'sessions':d['sessions'],"
                     "'jdk':d['jdk']},sort_keys=True))")
CAPTURE_SOURCES = {
    CAPTURE, 'tools/platform/capture-union-marker-modules.test.mjs',
    'tools/platform/data-size-session.mjs', 'tools/platform/data-size-session.test.mjs',
    'tools/platform/module-policy-session.mjs', 'tools/platform/resource-session.mjs',
    'tools/platform/package.json', 'tools/platform/package-lock.json',
    'tools/union_marker_oracle.py', 'tools/union_marker_fixtures.py',
    'tools/tests/test_union_marker_fixtures.py', 'tools/union_marker_portable_compare.py',
    'tools/tests/test_union_marker_portable_compare.py',
    'tools/amendment_construction_oracle.py', 'tools/collection_conversions_fixtures.py',
    'tools/collection_conversions_oracle.py', 'tools/collection_conversions_portable_compare.py',
    'tools/collection_conversions_compare.py', 'tools/data_size_portable_compare.py',
    'tools/data_size_compare.py', 'tools/data_size_fixtures.py', 'tools/data_size_oracle.py',
    'tools/java/CollectionConversionsOracle.java', 'tools/m1_platforms.py',
    'tools/m1_semantics.py', 'tools/m2_source_inputs.py',
}
COMMON = shared.COMMON
JAVA_ONLY = shared.JAVA_ONLY
EVENT_META = shared.EVENT_META
API_META = shared.API_META


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def identity(path):
    raw = Path(path).read_bytes()
    return {'bytes': len(raw), 'sha256': sha(raw)}


def read(path):
    return json.loads(Path(path).read_bytes())


def exact_files(root):
    names = set()
    for path in root.rglob('*'):
        need(not path.is_symlink(), 'symlink in portable evidence')
        if path.is_file():
            names.add(path.relative_to(root).as_posix())
        else:
            need(path.is_dir(), 'nonregular portable evidence')
    return names


def canonical(fixture):
    verdict = authority.verify(fixture)
    programs = read(fixture / 'programs.json')['programs']
    need(len(programs) == 40 and sha((fixture / 'programs.json').read_bytes()) == PROGRAMS_SHA256,
         'authored module selection differs')
    with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as archive:
        java = {member.name.removeprefix('capture-01/'): archive.extractfile(member).read()
                for member in archive}
    provenance = json.loads(java['provenance.json'])
    need(provenance['complete'] is True and provenance['stable'] is True and
         len(provenance['sessions']) == 80, 'Java separate-module sessions differ')
    return programs, java, provenance, verdict


def java_execution(provenance):
    commands = [row['command'] for row in provenance['sessions']]
    require = shared_collection.derive_java_execution
    # That helper's two-session contract is applied to every exact module pair,
    # then all forty pairs must agree on the effective Java evaluation identity.
    executions = [require({'environment': provenance['environment'],
                           'sessions': commands[index:index + 2],
                           'jdk_properties': provenance['jdk']['properties']})
                  for index in range(0, 80, 2)]
    need(all(row == executions[0] for row in executions), 'Java module evaluation environment differs')
    return executions[0]


def verify_http_requests(records, programs):
    need(type(records) is list and len(records) == len(programs) * 2 * 7,
         'browser HTTP request/session count differs')
    # Preserve raw order and each session boundary. The shared source-backed
    # two-session rule admits only dependency-preserving helper/favicons order.
    for index, program in enumerate(programs):
        start = index * 14
        pair = records[start:start + 14]
        for repeat in (0, 1):
            session = f"{index}:{program['id']}:{repeat}"
            need(all(type(row) is dict and set(row) == {'session', 'path', 'status'} and
                     row['session'] == session for row in pair[repeat * 7:(repeat + 1) * 7]),
                 'browser HTTP session ownership differs')
        shared.verify_http_requests([{'path': row['path'], 'status': row['status']} for row in pair])


def verify_m1_and_sources(m1, capture, native_build, wrapper, capture_dir):
    m1_binding = shared_collection.verify_m1(m1, capture)
    native.verify_build(native_build)
    native_sources = read(native_build / 'source-before.json')
    m1_hashes = read(m1 / 'sources.json')
    before, after, report = (read(wrapper / name) for name in
                             ('source-before.json', 'source-after.json', 'report.json'))
    need(before == after and report['complete'] is True and report['source_stable'] is True,
         'platform source bookends differ')
    copied = {name: identity(wrapper / 'sources' / name)
              for name in exact_files(wrapper / 'sources')}
    need(copied == before, 'platform copied source bytes differ')
    required = set(native_sources) | set(m1_hashes) | CAPTURE_SOURCES | {'.cargo/config.toml'}
    need(required <= set(before), 'multi-module portable source closure incomplete')
    need(all(before[name] == value for name, value in native_sources.items()),
         'native/platform source revision differs')
    need(all(before[name]['sha256'] == digest and
             sha((m1 / 'sources' / name).read_bytes()) == digest for name, digest in m1_hashes.items()),
         'M1/platform production source revision differs')
    need(all(before[name] == identity(ROOT / name) for name in CAPTURE_SOURCES),
         'multi-module portable helper source differs')
    need(before['.cargo/config.toml'] == native_sources['.cargo/config.toml'],
         'Cargo source configuration differs')
    commands = report['commands']
    m1_rows = [row for row in commands if row['name'] == 'm1-contract']
    portable = [row for row in commands if row['name'] == 'union-marker-portable']
    need(len(m1_rows) == len(portable) == 1 and
         m1_rows[0]['argv'] == ['python3', 'tools/m1_platforms.py', '--output', str(m1), '--verify-contract'] and
         m1_rows[0]['exit_code'] == portable[0]['exit_code'] == 0 and
         portable[0]['argv'] == [capture['node']['executable'], str(ROOT / CAPTURE),
                                str(capture_dir), str(m1 / 'wasm'),
                                str(m1 / 'pkl_napi.node'), str(capture['fixture'])],
         'platform captured M1 or union module command differs')
    for row in (m1_rows[0], portable[0]):
        for channel in ('stdout', 'stderr'):
            need(row[channel] == identity(wrapper / f"{row['name']}.{channel}.bin"),
                 'platform process raw channel differs')
    return {'m1': m1_binding, 'native_build': identity(native_build / 'build.json'),
            'wrapper': identity(wrapper / 'report.json'), 'source_count': len(before)}


def verify_capture(capture_dir, m1, fixture, capture, programs, provenance, verdict):
    need(capture['schema'] == 1 and capture['complete'] is True and capture['fixture'] == str(fixture)
         and capture['moduleUri'] == MODULE_URI and capture['repetitions'] == 2
         and capture['argv'] == [capture['node']['executable'], str(ROOT / CAPTURE),
                                 str(capture_dir), str(m1 / 'wasm'),
                                 str(m1 / 'pkl_napi.node'), str(fixture)]
         and capture['cwd'] == str(ROOT)
         and capture['fullPublicApiComparisonPassed'] is False and
         capture['performanceClaim'] is False and capture['infrastructureErrors'] == []
         and capture['artifactSourceBinding'].startswith('Supplied addon and Wasm source identity requires')
         and capture['inputsBefore'] == capture['inputsAfter'] and
         capture['fileSetsBefore'] == capture['fileSetsAfter'],
         'portable separate-module capture admission differs')
    expected_modules = [{'index': i, 'id': row['id'], 'moduleFile': row['module_file'],
                         'moduleSha256': row['module_sha256']} for i, row in enumerate(programs)]
    need(capture['modules'] == expected_modules, 'portable ordered modules differ')
    tracked = capture['inputsBefore']
    required = ({str(ROOT / name) for name in CAPTURE_SOURCES} |
                {str(m1 / 'pkl_napi.node'), str(m1 / 'wasm/pkl_wasm.js'),
                 str(m1 / 'wasm/pkl_wasm_bg.wasm'), capture['node']['executable'],
                 capture['python']['executable'], capture['playwright']['browserExecutable'],
                 capture['playwright']['package'], capture['playwright']['corePackage']})
    need(required <= set(tracked), 'portable input/source/runtime closure incomplete')
    need(all(sha(Path(path).read_bytes()) == digest for path, digest in tracked.items()),
         'portable source or artifact changed since capture')
    need(set(capture['fileSetsBefore']) == {str(fixture), str(m1 / 'wasm')},
         'portable source directory selection differs')
    for path, names in capture['fileSetsBefore'].items():
        need(names == sorted(str(p) for p in Path(path).rglob('*') if p.is_file()),
             'portable input file set changed')
    need(capture['artifacts']['napi']['sha256'] == tracked[str(m1 / 'pkl_napi.node')] and
         {row['path']: row['sha256'] for row in capture['artifacts']['wasm']['files']} ==
         {path: tracked[path] for path in capture['fileSetsBefore'][str(m1 / 'wasm')]},
         'portable build artifact identity differs')
    need(capture['node']['executableSha256'] == tracked[capture['node']['executable']] and
         capture['python']['executableSha256'] == tracked[capture['python']['executable']] and
         capture['playwright']['browserExecutableSha256'] ==
         tracked[capture['playwright']['browserExecutable']] and
         bool(capture['playwright']['browserVersion']),
         'Node/Python/Chromium executable identity differs')
    need(capture['playwright']['launchOptions'] == {'headless': True,
         'executablePath': capture['playwright']['browserExecutable'],
         'env': capture['javaExecution']['environment']} and
         capture['playwright']['contextOptions'] == {'locale': capture['javaExecution']['locale'],
         'timezoneId': capture['javaExecution']['timezoneId']},
         'browser requested environment differs')
    report = read(ROOT / 'tools/platform/package.json')
    lock = read(ROOT / 'tools/platform/package-lock.json')
    pin = report['devDependencies']['playwright']
    need(capture['playwright']['pin'] == pin ==
         lock['packages']['']['devDependencies']['playwright'] ==
         lock['packages']['node_modules/playwright']['version'] ==
         lock['packages']['node_modules/playwright-core']['version'],
         'browser package pin differs')
    fixture_command = capture['fixtureVerification']
    need(fixture_command['command'][0] in ('python3', capture['python']['executable']) and
         fixture_command['command'][1:] == [str(ROOT / 'tools/union_marker_fixtures.py'),
                                           '--fixture', str(fixture)] and
         fixture_command['exitCode'] == 0 and fixture_command['signal'] is None and
         fixture_command['error'] is None and
         fixture_command['stdoutSha256'] == sha((capture_dir / 'fixture-verification.stdout.bin').read_bytes()) and
         fixture_command['stderrSha256'] == sha((capture_dir / 'fixture-verification.stderr.bin').read_bytes()) and
         (capture_dir / 'fixture-verification.stderr.bin').read_bytes() == b'' and
         read(capture_dir / 'fixture-verification.stdout.bin') == verdict,
         'portable offline fixture proof differs')
    extracted = capture['javaProvenanceExtraction']
    need(extracted['command'] == [capture['python']['executable'], '-c', EXTRACTION_SCRIPT,
                                   str(fixture)] and extracted['cwd'] == str(ROOT) and
         extracted['exitCode'] == 0 and extracted['signal'] is None and extracted['error'] is None and
         extracted['stdoutSha256'] == sha((capture_dir / 'java-provenance.stdout.bin').read_bytes()) and
         extracted['stderrSha256'] == sha((capture_dir / 'java-provenance.stderr.bin').read_bytes()) and
         (capture_dir / 'java-provenance.stderr.bin').read_bytes() == b'',
         'Java provenance extraction process differs')
    raw_extract = read(capture_dir / 'java-provenance.stdout.bin')
    need(raw_extract == {'environment': provenance['environment'],
                         'sessions': provenance['sessions'], 'jdk': provenance['jdk']} and
         (capture_dir / 'java-provenance.stdout.bin').read_bytes() ==
         (json.dumps(raw_extract, sort_keys=True) + '\n').encode() and
         capture['javaExecution'] == java_execution(provenance),
         'Java actual evaluation environment differs')
    verify_http_requests(capture['httpRequests'], programs)
    need(set(capture['targets']) == {'napi', 'chromium'} and
         all(len(capture['targets'][target]) == 80 for target in ('napi', 'chromium')),
         'portable target or module coverage differs')


def compare_target(capture_dir, capture, programs, java, fixture, target):
    rows, sessions, pids = [], [], set()
    observed = capture['targets'][target]
    for index, program in enumerate(programs):
        for repeat in (0, 1):
            position = index * 2 + repeat
            row = observed[position]
            id_ = program['id']
            path = capture_dir / target / 'programs' / f'{index:02d}-{id_}' / f'repeat-{repeat}'
            java_prefix = f'programs/{index:02d}-{id_}/repeat-{repeat}/'
            result_prefix = java_prefix + f'000-{id_}/'
            need(row['index'] == index and row['id'] == id_ and row['repeat'] == repeat and
                 row['uri'] == MODULE_URI and row['directory'] == str(path),
                 'candidate module/session identity differs')
            module = read(path / 'input.json')
            need(module == {'index': index, 'id': id_, 'moduleFile': program['module_file'],
                            'moduleSha256': program['module_sha256'], 'uri': MODULE_URI,
                            'source': (fixture / program['module_file']).read_bytes().decode('utf-8'),
                            'rows': [{'id': id_, 'expression': 'result'}]},
                 'candidate separate module/query source differs')
            process_record = read(path / 'process.json')
            need(process_record == {key: value for key, value in row.items() if key != 'fileSet'},
                 'candidate process receipt differs')
            status = (path / f'000-{id_}' / 'result.status.bin').read_bytes()
            need(status in (b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'),
                 'candidate row status differs')
            clean = status.decode().strip()
            expected = {'input.json', 'process.stdout.bin', 'process.stderr.bin', 'process.json',
                        'candidate-trace.json'} | ({'child.json'} if target == 'napi' else set())
            expected.update(f'000-{id_}/{name}' for name in
                            set(COMMON) | (API_META if clean == 'api-exception' else EVENT_META))
            need(exact_files(path) == expected and row['fileSet']['fileCount'] == len(expected)
                 and row['fileSet']['status'] == clean,
                 'candidate raw channel/inventory differs')
            candidate = path / f'000-{id_}'
            reference = {name: java[result_prefix + name] for name in COMMON + tuple(JAVA_ONLY)}
            verdict = shared_collection.compare_row(reference, candidate)
            rows.append({'index': index, 'id': id_, 'repeat': repeat, **verdict})
            architecture = shared.validate_trace(path, [clean],
                {'uri': MODULE_URI, 'source': module['source'], 'rows': module['rows']}, target)
            issues = []
            for channel in ('stdout', 'stderr'):
                raw = (path / f'process.{channel}.bin').read_bytes()
                need(sha(raw) == row['fileSet'][channel + 'Sha256'],
                     'candidate whole process channel identity differs')
                if raw != java[java_prefix + f'process.{channel}.bin']:
                    issues.append('process.' + channel + ' differs')
            if target == 'napi':
                child = read(path / 'child.json')
                shared_collection.verify_target_environment(capture['javaExecution'], target,
                                                             row, child, capture['node']['platform'])
                need(row['exitCode'] == 0 and row['signal'] is None and row['error'] is None
                     and child['acquisitionError'] is None and child['architecture'] == architecture
                     and child['pid'] > 0 and child['pid'] not in pids
                     and child['argv'] == row['command'] ==
                       [capture['node']['executable'], str(ROOT / CAPTURE), '--child',
                        str(path / 'input.json'), str(path), capture['artifacts']['napi']['path']]
                     and child['cwd'] == str(ROOT) and child['executable'] == capture['node']['executable']
                     and child['node'] == capture['node']['version'] and
                     child['versions'] == capture['node']['versions'] and
                     row['processStdoutSha256'] == sha((path / 'process.stdout.bin').read_bytes()) and
                     row['processStderrSha256'] == sha((path / 'process.stderr.bin').read_bytes()),
                     'fresh N-API child/source/ownership differs')
                pids.add(child['pid'])
            else:
                shared_collection.verify_target_environment(capture['javaExecution'], target, row)
                need(row['protocolOrigin'] == 'browser-page-returned-UTF8' and
                     row['stderrOrigin'] == 'browser-pageerror-and-console-error-capture' and
                     row['architecture'] == architecture and row['acquisitionError'] is None and
                     row['browserErrors'] == [], 'fresh Chromium context/source/ownership differs')
            sessions.append({'index': index, 'id': id_, 'repeat': repeat, 'issues': issues})
    need(target != 'napi' or len(pids) == 80, 'N-API child process reused')
    return {'rows': rows, 'sessions': sessions,
            'counts': dict(Counter(row['status'] for row in rows)),
            'strict_string_passed': len(rows) == 80 and all(row['status'] == 'pass' for row in rows)
                                    and all(not row['issues'] for row in sessions)}


def verify(capture_dir, m1, native_build, build_wrapper, fixture=FIXTURE):
    capture_dir, m1, native_build, build_wrapper, fixture = (
        Path(path).resolve() for path in (capture_dir, m1, native_build, build_wrapper, fixture))
    source_before = {name: identity(ROOT / name) for name in sorted(CAPTURE_SOURCES)}
    programs, java, provenance, verdict = canonical(fixture)
    capture = read(capture_dir / 'report.json')
    verify_capture(capture_dir, m1, fixture, capture, programs, provenance, verdict)
    binding = verify_m1_and_sources(m1, capture, native_build, build_wrapper, capture_dir)
    targets = {target: compare_target(capture_dir, capture, programs, java, fixture, target)
               for target in ('napi', 'chromium')}
    need(source_before == {name: identity(ROOT / name) for name in sorted(CAPTURE_SOURCES)},
         'portable verifier source changed during readback')
    result = {'schema': 1, 'integrity': 'passed', 'authority': verdict,
              'source_binding': binding, 'targets': targets, 'verifier_sources': source_before,
              'producer_error_class': 'Java exception class, toString and cause are producer-only metadata; the portable string boundary does not synthesize them',
              'full_public_api_comparison_passed': False, 'full_m2_complete': False,
              'performance_targets_achieved': False,
              'strict_string_passed': all(row['strict_string_passed'] for row in targets.values())}
    expected = {'report.json', 'fixture-verification.stdout.bin', 'fixture-verification.stderr.bin',
                'java-provenance.stdout.bin', 'java-provenance.stderr.bin'}
    for target in ('napi', 'chromium'):
        for index, program in enumerate(programs):
            for repeat in (0, 1):
                relative = f"{target}/programs/{index:02d}-{program['id']}/repeat-{repeat}"
                expected |= {relative + '/' + name for name in exact_files(capture_dir / relative)}
    need(exact_files(capture_dir) == expected, 'portable top-level file set differs')
    return result


def gate_exit(result, gate):
    need(gate in ('integrity', 'string', 'full-api'), 'unknown gate')
    return 0 if gate == 'integrity' or (gate == 'string' and result['strict_string_passed']) else 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('capture', 'm1', 'native-build', 'build-wrapper', 'fixture'):
        parser.add_argument('--' + name, type=Path, required=name != 'fixture', default=FIXTURE if name == 'fixture' else None)
    parser.add_argument('--gate', choices=('integrity', 'string', 'full-api'), default='string')
    args = parser.parse_args()
    result = verify(args.capture, args.m1, args.native_build, args.build_wrapper, args.fixture)
    print(json.dumps(result, indent=2, sort_keys=True))
    raise SystemExit(gate_exit(result, args.gate))


if __name__ == '__main__':
    main()
