#!/usr/bin/env python3
"""Admit and compare canonical DataSize Node/Chromium string captures.

This is an offline verifier.  The Java export/value boundary is deliberately
outside its comparison and is always reported as required and unexecuted.
"""
from __future__ import annotations

import argparse
from collections import Counter
from contextlib import contextmanager
import hashlib
import json
import math
from pathlib import Path
import tarfile
import tempfile

import data_size_fixtures
import m1_platforms
import m2_source_inputs

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = data_size_fixtures.DEFAULT
COMMON = ('expression.bin', 'expression.code-units.bin', 'result.status.bin',
          'result.stdout.bin', 'result.stdout.code-units.bin',
          'result.stderr.bin', 'result.stderr.code-units.bin')
JAVA_ONLY = {'result.error-class.bin', 'result.error-class.code-units.bin'}
EVENT_META = {'candidate.' + name + '.bin' for name in
              ('disposition', 'phase', 'reason', 'callback-origin')}
API_META = {'candidate.api-error-name.bin', 'candidate.callback-origin.bin'}
CANONICAL = {'cases.json': '0e6f86ac0c9da84e7dce1b622bd99e2577b24cae4cf921ece7b6ef02a6a18a39',
             'module.pkl': 'fba4e81343683bbe433660e168905537fbd7d6b0dd44699e2b61b321b0a23982',
             'queries.tsv': '2946ae533b25cb6ca76ab8694cd866cfb1781dfc9ec29b0c9584a28229ac4728'}
EMBEDDED = {'crates/pkl-runtime/src/stdlib/' + name for name in
            ('base.pkl', 'ref.pkl', 'json.pkl', 'test.pkl', 'modules.txt')}
TRANSPORT = 'tools/platform/capture-data-size.mjs'
CAPTURE02_TRANSPORT_SHA256 = '66c77ba1d51ed6ee18373214656d83b7ac2129ebe1cd885a215c184fd260c962'
SOURCE_EXTRAS = {'tools/data_size_portable_compare.py',
                 'tools/tests/test_data_size_portable_compare.py',
                 'tools/platform/capture-data-size.mjs',
                 'tools/platform/data-size-session.mjs',
                 'tools/platform/capture-data-size.test.mjs',
                 'tools/platform/data-size-session.test.mjs',
                 'tools/data_size_fixtures.py', 'tools/data_size_oracle.py',
                 'tools/java/DataSizeSplitOracle.java', 'tools/m1_platforms.py',
                 'tools/m2_source_inputs.py'}
CAPTURE_SOURCES = {'tools/platform/capture-data-size.mjs',
                   'tools/platform/data-size-session.mjs',
                   'tools/platform/capture-data-size.test.mjs',
                   'tools/platform/data-size-session.test.mjs',
                   'tools/platform/module-policy-session.mjs',
                   'tools/platform/resource-session.mjs',
                   'tools/platform/package.json', 'tools/platform/package-lock.json',
                   'tools/data_size_fixtures.py', 'tools/data_size_oracle.py',
                   'tools/java/DataSizeSplitOracle.java'}


def require(ok, message):
    if not ok:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(path):
    data = Path(path).read_bytes()
    return {'sha256': sha(data), 'bytes': len(data)}


def read(path):
    return json.loads(Path(path).read_bytes())


def exact_files(root):
    result = {}
    for path in root.rglob('*'):
        require(not path.is_symlink(), 'symlink in evidence: ' + str(path))
        if path.is_file():
            result[path.relative_to(root).as_posix()] = identity(path)
        else:
            require(path.is_dir(), 'nonregular evidence entry: ' + str(path))
    return result


def source_names(root=ROOT):
    names = set(m2_source_inputs.source_paths(root)) | SOURCE_EXTRAS
    names.update(p.relative_to(root).as_posix() for p in
                 (root / 'tests/fixtures/m2-data-size').rglob('*') if p.is_file())
    return sorted(names)


@contextmanager
def java_authority(fixture):
    verdict = data_size_fixtures.verify(fixture)
    with tempfile.TemporaryDirectory(prefix='pkl-data-size-portable-java-') as directory:
        root = Path(directory)
        with tarfile.open(fixture / 'oracle-evidence.tar.gz', 'r:gz') as archive:
            for member in archive:
                if not member.name.startswith('capture-03/string/'):
                    continue
                require(member.isfile() and not member.issym() and not member.islnk(),
                        'nonregular Java authority member')
                path = Path(member.name)
                require('..' not in path.parts and not path.is_absolute(), 'unsafe Java member')
                dest = root / path
                dest.parent.mkdir(parents=True, exist_ok=True)
                require(not dest.exists(), 'duplicate Java authority member')
                dest.write_bytes(archive.extractfile(member).read())
        yield root / 'capture-03/string', verdict


def canonical_input(fixture):
    for name, digest in CANONICAL.items():
        require(sha((fixture / name).read_bytes()) == digest, 'canonical input differs: ' + name)
    cases = read(fixture / 'cases.json')
    rows = cases['rows']
    require(cases['schema'] == 2 and cases['baseline'] == 'Pkl 0.32.0'
            and cases['repetitions'] == 2 and cases['boundaries'] == ['string', 'export']
            and cases['module_uri'] == 'file:///stage57/datasize-oracle.pkl'
            and len(rows) == 255 and len({r['id'] for r in rows}) == 255,
            'canonical selection differs')
    require((fixture / 'queries.tsv').read_bytes() ==
            ''.join(r['id'] + '\t' + r['expression'] + '\n' for r in rows).encode(),
            'ordered canonical queries differ')
    return cases


def verify_m1(m1, capture, build_wrapper, capture_wrapper):
    report, source_hashes = read(m1 / 'report.json'), read(m1 / 'sources.json')
    require(report['source_stability'] is True and
            report['source_hashes'] == source_hashes, 'M1 source report differs')
    actual_names = {p.relative_to(m1 / 'sources').as_posix() for p in
                    (m1 / 'sources').rglob('*') if p.is_file()}
    require(actual_names == set(source_hashes) and EMBEDDED <= actual_names,
            'M1 source closure differs')
    # Recompute the collector against its copied source tree, including all five
    # production literal stdlib inputs.  Do not execute the copied collector.
    require({p.relative_to(m1 / 'sources').as_posix() for p in
             m1_platforms.source_paths(m1 / 'sources')} == actual_names,
            'M1 collected source paths differ')
    for name, digest in source_hashes.items():
        require(sha((m1 / 'sources' / name).read_bytes()) == digest,
                'M1 source bytes differ: ' + name)
    commands = read(m1 / 'commands.json')
    by_name = {c['name']: c for c in commands}
    required = {'napi-build', 'wasm-build', 'wasm-bindgen'}
    require(len(by_name) == len(commands) and required <= set(by_name) and
            all(by_name[name]['exit_code'] == 0 for name in required) and
            by_name['napi-build']['command'] ==
            ['cargo', 'build', '-p', 'pkl-napi', '--release', '--locked'] and
            by_name['wasm-build']['command'] ==
            ['cargo', 'build', '-p', 'pkl-wasm', '--target', 'wasm32-unknown-unknown',
             '--release', '--locked'] and
            by_name['wasm-bindgen']['command'][1:4] == ['--target', 'web', '--out-dir'] and
            by_name['wasm-bindgen']['command'][4] == str(m1 / 'wasm'),
            'M1 portable build failed or missing')
    for name in required:
        require((m1 / f'{name}.stdout.bin').is_file() and
                (m1 / f'{name}.stderr.bin').is_file(),
                'M1 build raw process channels missing')
    require(report['experiment_contract_status'] == 'pass' and
            report['compatibility_status'] == 'fail' and
            set(report['known_gaps']) == {'regex-unicode-15-vs-16', 'regex-zero-width-supplementary'} and
            report['total'] > 0 and report['shared_engine_passed'] == report['total'] and
            all(report['oracle_passed'][name] == report['total'] - 2
                for name in ('rust', 'node', 'browser')),
            'M1 experiment contract or preserved compatibility gaps differ')
    artifact_paths = {'napi': m1 / 'pkl_napi.node', 'wasm_js': m1 / 'wasm/pkl_wasm.js',
                      'wasm_bg': m1 / 'wasm/pkl_wasm_bg.wasm', 'native': m1 / 'semantics_probe'}
    before = read(build_wrapper / 'm1-artifacts-before-capture.json')
    require(before == read(build_wrapper / 'm1-artifacts-after-capture.json') ==
            read(capture_wrapper / 'm1-artifacts-before-capture.json') ==
            read(capture_wrapper / 'm1-artifacts-after-capture.json') and
            set(before) == set(artifact_paths), 'M1 artifact capture bookends differ')
    for key, path in artifact_paths.items():
        require(identity(path) == before[key] and
                report['artifacts'][path.relative_to(m1).as_posix()] == before[key]['sha256'],
                'M1 build artifact identity differs: ' + key)
    build_bookend = read(build_wrapper / 'acquisition-source-before.json')
    bookend = read(capture_wrapper / 'acquisition-source-before.json')
    require(CAPTURE_SOURCES <= set(build_bookend) & set(bookend) and
            EMBEDDED <= set(build_bookend) & set(bookend),
            'portable capture or embedded source missing from build closure')
    for name, digest in source_hashes.items():
        snapshot = identity(m1 / 'sources' / name)
        require(build_bookend[name] == snapshot and
                (bookend[name] == (identity(ROOT / name) if name == TRANSPORT else snapshot)),
                'wrapper/M1 source bookend differs: ' + name)
    delta = read(capture_wrapper / 'm1-transport-delta.json')
    require(delta['m1_report'] == str(m1 / 'report.json') and
            delta['m1_report_identity'] == identity(m1 / 'report.json') and
            delta['unchanged_m1_source_count'] == len(source_hashes) - 1 and
            set(delta['changed_transport_only']) == {TRANSPORT} and
            delta['changed_transport_only'][TRANSPORT] ==
            {'m1_sha256': source_hashes[TRANSPORT],
             'capture02_sha256': CAPTURE02_TRANSPORT_SHA256} and
            bookend[TRANSPORT]['sha256'] == capture['inputsBefore'][str(ROOT / TRANSPORT)] ==
            CAPTURE02_TRANSPORT_SHA256,
            'capture02 transport-only source delta differs')
    for name in CAPTURE_SOURCES:
        source = ROOT / name
        require(bookend[name] == identity(source),
                'portable capture source bookend differs: ' + name)
    tracked = capture['inputsBefore']
    for path in artifact_paths.values():
        if path.name == 'semantics_probe':
            continue
        require(tracked[str(path)] == identity(path)['sha256'],
                'captured portable artifact differs from M1 build')
    return {'source_count': len(actual_names), 'artifacts': before,
            'm1_report': identity(m1 / 'report.json'), 'transport_delta': delta}


def verify_wrapper(build_wrapper, capture_wrapper, m1, capture_dir, fixture, capture):
    old = read(build_wrapper / 'acquisition-commands.json')
    new = read(capture_wrapper / 'acquisition-commands.json')
    require([c['name'] for c in old] == ['m1-contract', 'datasize-portable'] and
            [c['name'] for c in new] == ['datasize-portable'],
            'wrapper command sequence differs')
    for wrapper in (build_wrapper, capture_wrapper):
        require(read(wrapper / 'acquisition-source-before.json') ==
                read(wrapper / 'acquisition-source-after.json'),
                'sources changed during portable acquisition')
    historical, summary = (read(path / 'acquisition-summary.json') for path in
                           (build_wrapper, capture_wrapper))
    old_capture = capture_dir.with_name('stage57-data-size-portable-capture-01')
    require(historical['m1_exit'] == 0 and historical['datasize_capture_exit'] == 1 and
            historical['capture_complete'] is False and
            historical['source_stable'] is True and
            historical['capture_report'] == identity(old_capture / 'report.json') and
            read(old_capture / 'report.json')['complete'] is False,
            'failed first capture history was promoted or changed')
    require(summary['m1_exit'] is None and summary['datasize_capture_exit'] == 0 and
            summary['source_stable'] is True and summary['capture_complete'] is True and
            summary['capture_report'] == identity(capture_dir / 'report.json'),
            'fresh capture wrapper reports incomplete acquisition')
    require(old[0]['argv'] == ['python3', 'tools/m1_platforms.py', '--output',
                              str(m1), '--verify-contract'] and
            old[0]['exit_code'] == 0 and old[1]['exit_code'] == 1 and
            old[0]['finished_epoch'] <= old[1]['started_epoch'] <=
            old[1]['finished_epoch'] <= new[0]['started_epoch'],
            'historical M1/build/capture process order differs')
    expected = [new[0]['argv'][0], str(ROOT / TRANSPORT), str(capture_dir),
                str(m1 / 'wasm'), str(m1 / 'pkl_napi.node'), str(fixture)]
    require(new[0]['argv'] == expected and
            capture['argv'] == [capture['node']['executable'], *expected[1:]] and
            capture['cwd'] == str(ROOT) and
            all(row['cwd'] == str(ROOT) for row in old + new),
            'portable actual command/cwd differs')
    for wrapper, command in ([(build_wrapper, row) for row in old] +
                             [(capture_wrapper, row) for row in new]):
        require(type(command['exit_code']) is int and command['signal'] is None and
                type(command['started_epoch']) in (int, float) and
                type(command['finished_epoch']) in (int, float) and
                math.isfinite(command['started_epoch']) and
                math.isfinite(command['finished_epoch']) and
                command['started_epoch'] <= command['finished_epoch'],
                'wrapper process status/timing differs')
        for channel in ('stdout', 'stderr'):
            path = wrapper / f'{command["name"]}.{channel}.bin'
            require(command[channel + '_log'] == str(path) and
                    command[channel] == identity(path), 'wrapper OS channel differs')
    require(new[0]['exit_code'] == 0 and
            (capture_wrapper / 'datasize-portable.stdout.bin').read_bytes() == b'' and
            (capture_wrapper / 'datasize-portable.stderr.bin').read_bytes() == b'',
            'fresh capture OS process failed or emitted unexpected output')
    require({k: v for k, v in new[0]['environment'].items() if v is not None and
             k in capture['environment']} == capture['environment'],
            'portable capture effective environment differs')
    return {'m1_command': {'argv': old[0]['argv'], 'exit_code': old[0]['exit_code']},
            'preserved_failed_capture': {'report': historical['capture_report'],
                                         'exit_code': old[1]['exit_code']},
            'fresh_capture_command': {'argv': new[0]['argv'], 'exit_code': new[0]['exit_code'],
                                      'stdout': new[0]['stdout'], 'stderr': new[0]['stderr']},
            'source_count': len(read(capture_wrapper / 'acquisition-source-before.json'))}


def verify_capture_report(capture_dir, m1, fixture, cases):
    report = read(capture_dir / 'report.json')
    require(type(report['schema']) is int and report['schema'] == 1 and
            report['complete'] is True and
            type(report['repetitions']) is int and report['repetitions'] == 2 and
            type(report['orderedRows']) is int and report['orderedRows'] == 255 and
            report['fixture'] == str(fixture) and
            report['caseIds'] == [r['id'] for r in cases['rows']],
            'portable capture selection/integrity differs')
    require(report['inputsBefore'] == report['inputsAfter'] and
            report['fileSetsBefore'] == report['fileSetsAfter'] and
            report['infrastructureErrors'] == [] and
            report['fullPublicApiComparisonPassed'] is False and
            report['performanceClaim'] is False and
            report['artifactSourceBinding'].startswith('Not inferred'),
            'portable capture input, error or scope accounting differs')
    verify_http_requests(report['httpRequests'])
    verify_target_selection(report['targets'])
    tracked = report['inputsBefore']
    required_paths = {str(p) for p in (fixture / name for name in CANONICAL)}
    required_paths |= {str(ROOT / name) for name in CAPTURE_SOURCES}
    required_paths |= {str(m1 / 'pkl_napi.node'), str(m1 / 'wasm/pkl_wasm.js'),
                       str(m1 / 'wasm/pkl_wasm_bg.wasm')}
    required_paths |= {report['node']['executable'], report['python']['executable'],
                       report['playwright']['browserExecutable'], report['playwright']['package'],
                       report['playwright']['corePackage']}
    require(required_paths <= set(tracked), 'portable tracked input closure incomplete')
    for name, digest in tracked.items():
        require(sha(Path(name).read_bytes()) == digest, 'portable tracked input differs: ' + name)
    require(set(report['fileSetsBefore']) == {str(fixture), str(m1 / 'wasm')},
            'portable tracked directory selection differs')
    for directory, names in report['fileSetsBefore'].items():
        require(names == sorted(str(p) for p in Path(directory).rglob('*') if p.is_file()),
                'portable tracked file set differs')
    require(report['fixtureInputs'] == CANONICAL and
            report['artifacts']['napi']['sha256'] == tracked[str(m1 / 'pkl_napi.node')] and
            {r['path']: r['sha256'] for r in report['artifacts']['wasm']['files']} ==
            {name: tracked[name] for name in report['fileSetsBefore'][str(m1 / 'wasm')]},
            'portable fixture/artifact identity differs')
    require(report['node']['executableSha256'] == tracked[report['node']['executable']] and
            report['python']['executableSha256'] == tracked[report['python']['executable']] and
            report['playwright']['browserExecutableSha256'] ==
            tracked[report['playwright']['browserExecutable']] and
            bool(report['playwright']['browserVersion']),
            'portable runtime identity differs')
    verify_browser_launch(report['playwright'])
    package = read(ROOT / 'tools/platform/package.json')
    lock = read(ROOT / 'tools/platform/package-lock.json')
    pin = package['devDependencies']['playwright']
    require(report['playwright']['pin'] == pin ==
            lock['packages']['']['devDependencies']['playwright'] ==
            lock['packages']['node_modules/playwright']['version'] ==
            lock['packages']['node_modules/playwright-core']['version'],
            'portable Playwright pin differs')
    command = report['fixtureVerification']
    require(command['exitCode'] == 0 and command['signal'] is None and
            command['error'] is None and
            command['command'][1:] == [str(ROOT / 'tools/data_size_fixtures.py'), '--fixture', str(fixture)] and
            command['stdoutSha256'] == sha((capture_dir / 'fixture-verification.stdout.bin').read_bytes()) and
            command['stderrSha256'] == sha((capture_dir / 'fixture-verification.stderr.bin').read_bytes()) and
            (capture_dir / 'fixture-verification.stderr.bin').read_bytes() == b'',
            'portable independent fixture verification differs')
    require(read(capture_dir / 'fixture-verification.stdout.bin')['kind'] ==
            'pkl-java-data-size-split-v1', 'portable fixture authority kind differs')
    return report


def verify_browser_launch(playwright):
    require(playwright['launchOptions'] ==
            {'headless': True,
             'executablePath': playwright['browserExecutable']} and
            playwright['browserExecutableRealpath'] ==
            str(Path(playwright['browserExecutable']).resolve()),
            'Chromium actual launch request differs from pinned executable')


def verify_http_requests(requests):
    one = [{'path': path, 'status': 204 if path == '/favicon.ico' else 200}
           for path in ('/', '/favicon.ico', '/pkl_wasm.js', '/pkl_wasm_bg.wasm',
                        '/session.mjs', '/module-policy-session.mjs',
                        '/resource-session.mjs')]
    require(requests == one + one,
            'browser HTTP resource path/status/order differs; preserve all 404s')


def verify_target_selection(targets):
    require(type(targets) is dict and set(targets) == {'napi', 'chromium'},
            'portable target coverage differs')
    for target in ('napi', 'chromium'):
        rows = targets[target]
        require(type(rows) is list and len(rows) == 2 and
                all(type(row) is dict and type(row.get('repeat')) is int
                    for row in rows) and
                [row['repeat'] for row in rows] == [0, 1],
                'portable target/repetition coverage differs')


def validate_trace(path, row_records, fixture, target):
    trace = read(path / 'candidate-trace.json')
    architecture = trace['architecture']
    terminal_count = sum(status != 'api-exception' for status in row_records)
    require(trace['acquisitionError'] is None and
            trace['boundary'].startswith('String query status and complete UTF16/Java UTF8') and
            architecture['sessionCreated'] is True and architecture['closed'] is True and
            type(architecture['polls']) is int and architecture['polls'] >= terminal_count and
            type(architecture['collections']) is int and architecture['collections'] >= terminal_count and
            architecture['terminalSlots'] == architecture['terminalDepth'] == 0 and
            architecture['retainedPayloadsStable'] is True,
            'portable session cleanup/ownership differs')
    events = trace['trace']
    queries = [e for e in events if e.get('kind') == 'evaluate-module-source']
    terminals = [e for e in events if e.get('kind') == 'terminal']
    requires = [(i, r['id'], r['expression']) for i, r in enumerate(fixture['rows'])]
    require([(e['index'], e['id'], e['expression']) for e in queries] == requires and
            all(e['uri'] == fixture['uri'] for e in queries),
            'portable ordered source/query trace differs')
    failures = [e for e in events if e.get('kind') == 'api-error']
    expected_terminal = [(i, r['id']) for i, r in enumerate(fixture['rows'])
                         if row_records[i] != 'api-exception']
    expected_failure = [(i, r['id']) for i, r in enumerate(fixture['rows'])
                        if row_records[i] == 'api-exception']
    require([(e['index'], e['id']) for e in terminals] == expected_terminal and
            [(e['index'], e['id']) for e in failures] == expected_failure,
            'portable terminal/API failure trace coverage differs')
    for event in failures:
        index = event['index']
        row = fixture['rows'][index]
        message = event['error']['message']
        require(type(message) is str and
                message.encode('utf-16-be', errors='surrogatepass') ==
                (path / f'{index:03}-{row["id"]}' / 'result.stderr.code-units.bin').read_bytes(),
                'portable API failure/row UTF16 payload differs')
    for event, (_, status) in zip(terminals, ((i, row_records[i]) for i, _ in expected_terminal)):
        derived = 'success' if event['event']['kind'] == 'value' else (
            'error' if event['event']['kind'] == 'error' and
            event['event'].get('disposition') == 'language-error' else 'candidate-error')
        require(derived == status, 'portable terminal/row status differs')
        index = event['index']
        raw = event['event']['value' if status == 'success' else 'error']
        require(isinstance(raw, dict) and
                list(raw) == [str(i) for i in range(len(raw))] and
                all(type(unit) is int and 0 <= unit <= 65535 for unit in raw.values()),
                'portable terminal UTF16 payload shape differs')
        data = bytes(byte for unit in raw.values() for byte in (unit >> 8, unit & 255))
        name = 'result.stdout.code-units.bin' if status == 'success' else 'result.stderr.code-units.bin'
        row = fixture['rows'][index]
        require(data == (path / f'{index:03}-{row["id"]}' / name).read_bytes(),
                'portable retained terminal UTF16 payload differs')
    api_calls = [e for e in events if e.get('kind') == 'api-call']
    evaluator_calls = [e for e in api_calls if e['method'] == 'evaluateModuleSource']
    require([e['args'] for e in evaluator_calls] ==
            [[fixture['uri'], fixture['source'], row['expression'], 0, None]
             for row in fixture['rows']] and
            [e['method'] for e in api_calls if e['method'] in {'evaluateModuleSource', 'close'}] ==
            ['evaluateModuleSource'] * len(row_records) + ['close'] and
            len([e for e in events if e.get('kind') == 'constructor']) == 1 and
            all(e['method'] in {'evaluateModuleSource', 'close', 'completeDefaultModule',
                                'completeModuleKey', 'completeModuleCachePolicy',
                                'completeModuleFragmentPaths', 'completeModuleCapabilities'}
                for e in api_calls),
            'portable public evaluator call coverage differs')
    positions = [(i, e) for i, e in enumerate(events)]
    starts = [i for i, e in positions if e.get('kind') == 'evaluate-module-source']
    ends = [i for i, e in positions if e.get('kind') in {'terminal', 'api-error'}]
    calls = [i for i, e in positions if e.get('kind') == 'api-call' and
             e.get('method') == 'evaluateModuleSource']
    closes = [i for i, e in positions if e.get('kind') == 'api-call' and e.get('method') == 'close']
    require(len(starts) == len(ends) == len(calls) == len(row_records) and
            len(closes) == 1 and all(starts[i] < calls[i] < ends[i] and
                                     (i + 1 == len(starts) or ends[i] < starts[i + 1])
                                     for i in range(len(starts))) and
            ends[-1] < closes[0], 'portable evaluator call order differs')
    requests = [(i, e['event']) for i, e in positions if e.get('kind') == 'module-request']
    completions = [(i, e) for i, e in positions if e.get('kind') == 'api-call' and
                   e.get('method', '').startswith('complete')]
    require(len(requests) == len(completions) and
            len({event['token'] for _, event in requests}) == len(requests) and
            len({event['module']['id'] for _, event in requests}) == len(requests),
            'portable Host request ownership count/identity differs')
    for (requested_at, request), (completed_at, completion) in zip(requests, completions):
        require(requested_at < completed_at and
                completion['args'] and completion['args'][0] == request['token'] and
                completion['method'] in {'completeDefaultModule', 'completeModuleKey',
                                         'completeModuleCachePolicy',
                                         'completeModuleFragmentPaths',
                                         'completeModuleCapabilities'},
                'portable Host request completion ownership differs')
        module = request['module']
        if (module['kind'] == 'select-key' and module.get('selectionPhase') == 'primary' and
                module.get('originalUri') == fixture['uri']):
            require(completion['method'] == 'completeModuleKey' and
                    completion['args'][1:] ==
                    [module['keyId'], fixture['uri'], 'host',
                     'fixture-key:' + module['keyId'], 0],
                    'portable primary Host ModuleKey binding differs')
    if target == 'chromium':
        require((path / 'process.stdout.bin').read_bytes().startswith(b'session-open mode=string'),
                'browser page-returned protocol missing')
    return architecture


def classify(reference, candidate):
    require({p.name for p in reference.iterdir()} == set(COMMON) | JAVA_ONLY,
            'Java common/producer channel layout differs')
    status_file = candidate / 'result.status.bin'
    status = status_file.read_bytes() if status_file.is_file() else None
    metadata = API_META if status == b'api-exception\n' else EVENT_META
    names = {p.name for p in candidate.iterdir()} if candidate.is_dir() else set()
    issues = []
    if names != set(COMMON) | metadata:
        issues.append('candidate row channel set differs')
    if status not in {b'success\n', b'error\n', b'candidate-error\n', b'api-exception\n'}:
        issues.append('candidate row status invalid')
    if (status == b'error\n' and (candidate / 'candidate.disposition.bin').is_file() and
            (candidate / 'candidate.disposition.bin').read_bytes() != b'language-error'):
        issues.append('ordinary error disposition differs')
    if (status == b'candidate-error\n' and (candidate / 'candidate.disposition.bin').is_file() and
            (candidate / 'candidate.disposition.bin').read_bytes() == b'language-error'):
        issues.append('candidate failure mislabeled ordinary')
    if (status == b'success\n' and (candidate / 'candidate.disposition.bin').is_file() and
            (candidate / 'candidate.disposition.bin').read_bytes()):
        issues.append('success has error disposition')
    differences = [name for name in COMMON if not (candidate / name).is_file() or
                   (candidate / name).read_bytes() != (reference / name).read_bytes()]
    return {'status': 'pass' if not issues and not differences else 'mismatch',
            'channels': differences, 'issues': issues}


def compare_target(capture_dir, report, java, cases, fixture, target):
    rows, sessions = [], []
    for repeat, observation in enumerate(report['targets'][target]):
        path = capture_dir / target / f'repeat-{repeat}'
        require(type(observation['repeat']) is int and observation['repeat'] == repeat and
                observation['directory'] == str(path) and
                observation['uri'] == cases['module_uri'] and observation['rowCount'] == 255,
                'portable session identity differs')
        process = read(path / 'process.json')
        require({k: v for k, v in observation.items() if k != 'fileSet'} == process,
                'portable process record differs')
        inp = read(path / 'input.json')
        require(inp == {'uri': cases['module_uri'], 'source':
                (fixture / 'module.pkl').read_text(),
                'rows': [{'id': r['id'], 'expression': r['expression']} for r in cases['rows']],
                'inputHashes': CANONICAL}, 'portable session input differs')
        actual = exact_files(path)
        expected = {'input.json', 'candidate-trace.json', 'process.json',
                    'process.stdout.bin', 'process.stderr.bin'}
        if target == 'napi':
            expected.add('child.json')
        statuses = []
        for i, row in enumerate(cases['rows']):
            name = f'{i:03}-{row["id"]}'
            candidate = path / name
            status = (candidate / 'result.status.bin').read_bytes()
            statuses.append(status.decode('ascii').removesuffix('\n'))
            expected.update(name + '/' + n for n in (set(COMMON) | (API_META if status == b'api-exception\n' else EVENT_META)))
            verdict = classify(java / f'repeat-{repeat}' / name, candidate)
            rows.append({'repeat': repeat, 'id': row['id'], **verdict})
        require(set(actual) == expected and len(actual) == observation['fileSet']['fileCount'],
                'portable exact session file inventory differs')
        require(Counter(statuses) == observation['fileSet']['statusCounts'] and
                observation['fileSet']['rowCount'] == 255,
                'portable status/file-set accounting differs')
        for channel in ('stdout', 'stderr'):
            data = (path / f'process.{channel}.bin').read_bytes()
            require(sha(data) == observation['fileSet'][f'process{channel.title()}Sha256'],
                    'portable process channel identity differs')
            if target == 'napi':
                require(sha(data) == observation[f'process{channel.title()}Sha256'],
                        'N-API OS process channel differs')
        architecture = validate_trace(path, statuses, inp, target)
        issues = []
        if ((path / 'process.stdout.bin').read_bytes() !=
                (java / f'repeat-{repeat}' / 'process.stdout.bin').read_bytes()):
            issues.append('whole protocol differs')
        if (path / 'process.stderr.bin').read_bytes() != b'':
            issues.append('portable error channel nonempty')
        if target == 'napi':
            child = read(path / 'child.json')
            require(type(observation['exitCode']) is int and observation['exitCode'] == 0 and
                    observation['signal'] is None and
                    observation['error'] is None and
                    child['architecture'] == architecture and child['acquisitionError'] is None and
                    child['pid'] > 0 and type(child['pid']) is int and
                    child['executable'] == report['node']['executable'] and
                    child['node'] == report['node']['version'] and
                    child['versions'] == report['node']['versions'] and
                    child['cwd'] == str(ROOT) and child['argv'] ==
                    [report['node']['executable'], str(ROOT / 'tools/platform/capture-data-size.mjs'), '--child',
                     str(path / 'input.json'), str(path), str(Path(report['artifacts']['napi']['path']))] and
                    observation['command'] == child['argv'],
                    'N-API child process boundary differs')
            sessions.append({'repeat': repeat, 'pid': child['pid'], 'issues': issues})
        else:
            require(observation['protocolOrigin'] == 'browser-page-returned-UTF8' and
                    observation['stderrOrigin'] == 'browser-pageerror-and-console-error-capture' and
                    observation['isolation'] == 'new-browser-context-and-Wasm-instance-for-entire-ordered-session' and
                    observation['architecture'] == architecture and
                    observation['acquisitionError'] is None and observation['browserErrors'] == [],
                    'Chromium page/context/error boundary differs')
            sessions.append({'repeat': repeat, 'protocol_origin': observation['protocolOrigin'],
                             'issues': issues})
    if target == 'napi':
        require(sessions[0]['pid'] != sessions[1]['pid'], 'N-API fresh process identities reused')
    counts = dict(Counter(row['status'] for row in rows))
    return {'rows': rows, 'sessions': sessions, 'counts': counts,
            'strict_string_passed': counts == {'pass': 510} and
            all(not row['issues'] for row in sessions)}


def verify(capture_dir, m1, build_wrapper, capture_wrapper, fixture=FIXTURE):
    capture_dir, m1, build_wrapper, capture_wrapper, fixture = (Path(p).resolve() for p in
        (capture_dir, m1, build_wrapper, capture_wrapper, fixture))
    names = source_names()
    source_before = {name: identity(ROOT / name) for name in names}
    cases = canonical_input(fixture)
    with java_authority(fixture) as (java, authority):
        report = verify_capture_report(capture_dir, m1, fixture, cases)
        require(read(capture_dir / 'fixture-verification.stdout.bin') == authority,
                'portable fixture verification verdict differs')
        binding = verify_m1(m1, report, build_wrapper, capture_wrapper)
        process = verify_wrapper(build_wrapper, capture_wrapper, m1, capture_dir, fixture, report)
        results = {target: compare_target(capture_dir, report, java, cases, fixture, target)
                   for target in ('napi', 'chromium')}
    require(set(exact_files(capture_dir)) ==
            {'report.json', 'fixture-verification.stdout.bin', 'fixture-verification.stderr.bin'} |
            {f'{target}/repeat-{repeat}/{name}' for target in ('napi', 'chromium')
             for repeat in range(2) for name in exact_files(capture_dir / target / f'repeat-{repeat}')},
            'portable capture top-level inventory differs')
    require(source_before == {name: identity(ROOT / name) for name in source_names()},
            'portable comparator source closure changed during verification')
    source_manifest = sha(json.dumps(source_before, sort_keys=True,
                                    separators=(',', ':')).encode())
    return {'schema': 1, 'integrity': 'passed', 'authority': authority,
            'offline_verifier_source_count': len(source_before),
            'offline_verifier_source_manifest_sha256': source_manifest,
            'offline_verifier_only_sources':
            {name: source_before[name] for name in
             ('tools/data_size_portable_compare.py',
              'tools/tests/test_data_size_portable_compare.py')},
            'm1_binding': binding, 'wrapper': process,
            'targets': results,
            'strict_string_passed': all(results[t]['strict_string_passed'] for t in results),
            'public_export_status': 'not-executed-required',
            'full_public_api_comparison_passed': False,
            'full_m2_complete': False, 'performance_targets_achieved': False}


def gate_exit(result, gate):
    require(gate in {'integrity', 'string', 'full-api'}, 'unknown portable comparison gate')
    if gate == 'integrity':
        return int(result['integrity'] != 'passed')
    if gate == 'string':
        return int(not result['strict_string_passed'])
    return int(not result['full_public_api_comparison_passed'])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', type=Path, required=True)
    parser.add_argument('--m1', type=Path, required=True)
    parser.add_argument('--build-wrapper', type=Path, required=True,
                        help='M1 build and preserved failed first capture packet')
    parser.add_argument('--capture-wrapper', type=Path, required=True,
                        help='Fresh capture-only packet and transport source delta')
    parser.add_argument('--fixture', type=Path, default=FIXTURE)
    parser.add_argument('--gate', choices=('integrity', 'string', 'full-api'),
                        default='string', help='Integrity admission, strict common strings, or full public API')
    args = parser.parse_args()
    result = verify(args.capture, args.m1, args.build_wrapper, args.capture_wrapper, args.fixture)
    print(json.dumps(result, sort_keys=True))
    return gate_exit(result, args.gate)


if __name__ == '__main__':
    raise SystemExit(main())
