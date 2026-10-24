"""Failure injection for collection conversion portable raw comparison admission."""
from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import collection_conversions_portable_compare as compare


class PortableCollectionConversionComparisonTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.path = Path(temp.name)
        self.reference = {name: b'' for name in set(compare.COMMON) | compare.JAVA_ONLY}
        self.reference.update({
            'expression.bin': b'10.ifNonNull((x) -> x)',
            'expression.code-units.bin': '10.ifNonNull((x) -> x)'.encode('utf-16-be'),
            'result.status.bin': b'success\n',
            'result.stdout.bin': b'10',
            'result.stdout.code-units.bin': '10'.encode('utf-16-be'),
        })
        self.reference['result.error-class.bin'] = b''
        self.reference['result.error-class.code-units.bin'] = b''
        self.write_candidate()

    def write_candidate(self, status=b'success\n', disposition=b''):
        for path in self.path.iterdir():
            path.unlink()
        for name in compare.COMMON:
            (self.path / name).write_bytes(status if name == 'result.status.bin'
                                           else self.reference[name])
        for name in compare.EVENT_META:
            (self.path / name).write_bytes(disposition if name == 'candidate.disposition.bin'
                                           else b'')

    def test_exact_common_bytes_and_java_only_class(self):
        self.assertEqual(compare.compare_row(self.reference, self.path)['status'], 'pass')
        for name, content in [('result.stdout.bin', b'10 '),
                              ('result.stdout.code-units.bin', b'\x00?'),
                              ('expression.bin', b'10.ifNonNull((x)->x)'),
                              ('result.status.bin', b'error\n')]:
            with self.subTest(name=name):
                original = (self.path / name).read_bytes()
                (self.path / name).write_bytes(content)
                self.assertEqual(compare.compare_row(self.reference, self.path)['status'],
                                 'mismatch')
                (self.path / name).write_bytes(original)
        (self.path / 'result.error-class.bin').write_bytes(b'fabricated producer metadata')
        self.assertEqual(compare.compare_row(self.reference, self.path)['status'], 'mismatch')

    def test_candidate_failure_cannot_be_relabelled_ordinary(self):
        self.reference.update({'result.status.bin': b'error\n',
                               'result.stdout.bin': b'',
                               'result.stdout.code-units.bin': b'',
                               'result.stderr.bin': b'failed',
                               'result.stderr.code-units.bin': 'failed'.encode('utf-16-be')})
        self.write_candidate(b'error\n', b'unsupported')
        result = compare.compare_row(self.reference, self.path)
        self.assertEqual(result['status'], 'mismatch')
        self.assertEqual(result['channels'], [])
        self.write_candidate(b'candidate-error\n', b'unsupported')
        self.assertIn('result.status.bin', compare.compare_row(self.reference, self.path)['channels'])
        self.write_candidate(b'error\n', b'language-error')
        self.assertEqual(compare.compare_row(self.reference, self.path)['status'], 'pass')

    def test_supplied_m1_build_artifact_identity_is_required(self):
        m1 = self.path / 'm1'
        (m1 / 'sources').mkdir(parents=True)
        (m1 / 'wasm').mkdir()
        source_paths = []
        for name in sorted(compare.EMBEDDED):
            path = m1 / 'sources' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(name.encode())
            source_paths.append(path)
        source_hashes = {name: compare.sha(name.encode()) for name in compare.EMBEDDED}
        files = {'pkl_napi.node': b'addon', 'wasm/pkl_wasm.js': b'js',
                 'wasm/pkl_wasm_bg.wasm': b'wasm'}
        for name, raw in files.items():
            (m1 / name).write_bytes(raw)
        report = {'source_stability': True, 'source_hashes': source_hashes,
                  'experiment_contract_status': 'pass', 'compatibility_status': 'fail',
                  'known_gaps': ['regex-unicode-15-vs-16', 'regex-zero-width-supplementary'],
                  'total': 6078, 'shared_engine_passed': 6078, 'oracle_passed': {'rust': 6076, 'node': 6076, 'browser': 6076},
                  'artifacts': {name: compare.sha(raw) for name, raw in files.items()}}
        (m1 / 'report.json').write_text(json.dumps(report))
        (m1 / 'sources.json').write_text(json.dumps(source_hashes))
        commands = [
            {'name': 'napi-build', 'exit_code': 0,
             'command': ['cargo', 'build', '-p', 'pkl-napi', '--release', '--locked']},
            {'name': 'wasm-build', 'exit_code': 0,
             'command': ['cargo', 'build', '-p', 'pkl-wasm',
                         '--target', 'wasm32-unknown-unknown', '--release', '--locked']},
            {'name': 'wasm-bindgen', 'exit_code': 0,
             'command': ['wasm-bindgen', '--target', 'web', '--out-dir', str(m1 / 'wasm')]},
        ]
        (m1 / 'commands.json').write_text(json.dumps(commands))
        for name in ('napi-build', 'wasm-build', 'wasm-bindgen'):
            for channel in ('stdout', 'stderr'):
                (m1 / f'{name}.{channel}.bin').write_bytes(b'')
        tracked = {str(m1 / name): compare.sha(raw) for name, raw in files.items()}
        capture = {'inputsBefore': tracked,
                   'fileSetsBefore': {str(m1 / 'wasm'):
                                      sorted(str(m1 / name) for name in files if name.startswith('wasm/'))},
                   'artifacts': {'napi': {'sha256': tracked[str(m1 / 'pkl_napi.node')]},
                                 'wasm': {'files': [{'path': str(m1 / name),
                                                     'sha256': compare.sha(raw)}
                                                    for name, raw in files.items()
                                                    if name.startswith('wasm/')]}}}
        with patch.object(compare.m1_platforms, 'source_paths', return_value=source_paths):
            self.assertEqual(compare.verify_m1(m1, capture)['source_count'], 6)
            commands.insert(0, {'name': 'napi-build', 'exit_code': 1, 'command': []})
            (m1 / 'commands.json').write_text(json.dumps(commands))
            with self.assertRaisesRegex(ValueError, 'build commands'):
                compare.verify_m1(m1, capture)
            commands.pop(0)
            (m1 / 'commands.json').write_text(json.dumps(commands))
            report.update(total=0, shared_engine_passed=0,
                          oracle_passed={'rust': -2, 'node': -2, 'browser': -2})
            (m1 / 'report.json').write_text(json.dumps(report))
            with self.assertRaisesRegex(ValueError, 'bounded contract'):
                compare.verify_m1(m1, capture)
            report.update(total=6078, shared_engine_passed=6078,
                          oracle_passed={'rust': 6076, 'node': 6076, 'browser': 6076})
            (m1 / 'report.json').write_text(json.dumps(report))
            missing = source_paths[0]
            missing.unlink()
            source_hashes.pop(missing.relative_to(m1 / 'sources').as_posix())
            report['source_hashes'] = source_hashes
            (m1 / 'report.json').write_text(json.dumps(report))
            (m1 / 'sources.json').write_text(json.dumps(source_hashes))
            with self.assertRaisesRegex(ValueError, 'source closure'):
                compare.verify_m1(m1, capture)
            name = missing.relative_to(m1 / 'sources').as_posix()
            missing.write_bytes(name.encode())
            source_hashes[name] = compare.sha(name.encode())
            report['source_hashes'] = source_hashes
            (m1 / 'report.json').write_text(json.dumps(report))
            (m1 / 'sources.json').write_text(json.dumps(source_hashes))
            (m1 / 'pkl_napi.node').write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'artifact identity'):
                compare.verify_m1(m1, capture)

    def test_missing_or_mismatching_rows_fail_strict_gate(self):
        sessions = [{'issues': []}, {'issues': []}]
        rows = [{'status': 'pass'}] * 470 + [{'status': 'mismatch'}] * 4
        self.assertFalse(compare.strict_target(rows, sessions, 236))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 472, sessions, 236))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 116, sessions, 58))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 266, sessions, 133))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 44, sessions, 22))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 20, sessions, 10))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 52, sessions, 26))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 20, sessions, 22))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 52, sessions, 10))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 547, sessions, 236))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 116, sessions, 236))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 472,
                                               [{'issues': ['protocol']}, {'issues': []}], 236))

    def test_native_m1_and_wrapper_sources_must_be_the_same_revision(self):
        m1 = self.path / 'm1'
        build = self.path / 'build'
        wrapper = self.path / 'wrapper'
        names = ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                 '.cargo/config.toml', 'crates/pkl-runtime/src/duration.rs',
                 'crates/pkl-napi/src/lib.rs']
        contents = {name: name.encode() for name in names}
        for name, data in contents.items():
            for root in (m1 / 'sources', build / 'sources', wrapper / 'sources'):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(data)
        for name in compare.CAPTURE_SOURCES:
            path = wrapper / 'sources' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((compare.ROOT / name).read_bytes())
        native = {name: compare.identity(build / 'sources' / name) for name in names}
        wrapper_sources = {name: compare.identity(path) for name, path in
                           ((path.relative_to(wrapper / 'sources').as_posix(), path)
                            for path in (wrapper / 'sources').rglob('*') if path.is_file())}
        m1_hashes = {name: compare.sha((m1 / 'sources' / name).read_bytes())
                     for name in names if name != '.cargo/config.toml'}
        (build / 'source-before.json').write_text(json.dumps(native))
        (m1 / 'sources.json').write_text(json.dumps(m1_hashes))
        (build / 'build.json').write_text('{}')
        (wrapper / 'source-before.json').write_text(json.dumps(wrapper_sources))
        (wrapper / 'source-after.json').write_text(json.dumps(wrapper_sources))
        (wrapper / 'm1-contract.stdout.bin').write_bytes(b'contract passed\n')
        (wrapper / 'm1-contract.stderr.bin').write_bytes(b'')
        command = {'name': 'm1-contract', 'argv': ['python3', 'tools/m1_platforms.py',
                   '--output', str(m1), '--verify-contract'], 'exit_code': 0,
                   'started_epoch': 1.0, 'finished_epoch': 2.0, 'pid': 123,
                   'stdout': compare.identity(wrapper / 'm1-contract.stdout.bin'),
                   'stderr': compare.identity(wrapper / 'm1-contract.stderr.bin')}
        report = {'complete': True, 'source_stable': True, 'cwd': str(compare.ROOT),
                  'environment': {'CARGO_INCREMENTAL': '0',
                                  'CARGO_PROFILE_DEV_DEBUG': '0'},
                  'commands': [command]}
        (wrapper / 'report.json').write_text(json.dumps(report))
        with patch.object(compare.native, 'verify_build', return_value={}):
            self.assertEqual(compare.verify_source_binding(m1, build, wrapper)
                             ['shared_source_count'], len(m1_hashes))
            changed = dict(wrapper_sources)
            changed['crates/pkl-runtime/src/duration.rs'] = {
                'bytes': 4, 'sha256': compare.sha(b'old!')}
            (wrapper / 'source-before.json').write_text(json.dumps(changed))
            (wrapper / 'source-after.json').write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError, 'snapshot differs'):
                compare.verify_source_binding(m1, build, wrapper)
            (wrapper / 'source-before.json').write_text(json.dumps(wrapper_sources))
            (wrapper / 'source-after.json').write_text(json.dumps(wrapper_sources))
            (wrapper / 'sources/crates/pkl-runtime/src/duration.rs').write_bytes(b'old!')
            changed = dict(wrapper_sources)
            changed['crates/pkl-runtime/src/duration.rs'] = compare.identity(
                wrapper / 'sources/crates/pkl-runtime/src/duration.rs')
            (wrapper / 'source-before.json').write_text(json.dumps(changed))
            (wrapper / 'source-after.json').write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError, 'native/platform source revision'):
                compare.verify_source_binding(m1, build, wrapper)
            (wrapper / 'sources/crates/pkl-runtime/src/duration.rs').write_bytes(
                contents['crates/pkl-runtime/src/duration.rs'])
            (wrapper / 'source-before.json').write_text(json.dumps(wrapper_sources))
            (wrapper / 'source-after.json').write_text(json.dumps(wrapper_sources))
            (wrapper / 'sources/.cargo/config.toml').unlink()
            with self.assertRaisesRegex(ValueError, 'snapshot differs'):
                compare.verify_source_binding(m1, build, wrapper)
            (wrapper / 'sources/.cargo/config.toml').write_bytes(contents['.cargo/config.toml'])
            missing_helper = sorted(compare.CAPTURE_SOURCES)[0]
            (wrapper / 'sources' / missing_helper).unlink()
            with self.assertRaisesRegex(ValueError, 'snapshot differs'):
                compare.verify_source_binding(m1, build, wrapper)
            (wrapper / 'sources' / missing_helper).write_bytes(b'old helper revision')
            changed = dict(wrapper_sources)
            changed[missing_helper] = compare.identity(wrapper / 'sources' / missing_helper)
            (wrapper / 'source-before.json').write_text(json.dumps(changed))
            (wrapper / 'source-after.json').write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError, 'portable helper source revision'):
                compare.verify_source_binding(m1, build, wrapper)

    def test_canonical_archive_and_gate_scope(self):
        cases, hashes, authority = compare.canonical_input(compare.FIXTURE)
        self.assertEqual(len(cases['rows']), 236)
        self.assertEqual(authority['statuses'], {'success': 158, 'error': 78})
        self.assertEqual(set(hashes), {'cases.json', 'module.pkl', 'queries.tsv'})
        archive = compare.java_members(compare.FIXTURE)
        row = archive['repeat-0/000-typed-empty-map/result.stdout.bin']
        self.assertEqual(row, b'Map()')
        provenance = json.loads(archive['provenance.json'])
        execution = compare.derive_java_execution({
            'environment': provenance['environment'],
            'sessions': [session['command'] for session in provenance['sessions']],
            'jdk_properties': provenance['jdk']['properties'],
        })
        self.assertEqual((execution['locale'], execution['timezoneId'],
                          execution['preflightLocale']), ('en-US', 'UTC', 'ko-KR'))
        self.assertEqual(compare.gate_exit({'integrity': 'passed',
                                            'strict_string_passed': False}, 'integrity'), 0)
        self.assertEqual(compare.gate_exit({'integrity': 'passed',
                                            'strict_string_passed': False}, 'string'), 1)
        self.assertEqual(compare.gate_exit({'integrity': 'passed',
                                            'strict_string_passed': True}, 'full-api'), 1)

    def test_control_profile_selects_58_rows_and_separate_authority(self):
        fixture = compare.ROOT / 'tests/fixtures/m2-collection-conversion-controls'
        cases, hashes, authority = compare.canonical_input(fixture)
        self.assertEqual(len(cases['rows']), 58)
        self.assertEqual(cases['module_uri'], 'file:///stage62/collection-conversion-controls.pkl')
        self.assertEqual(authority['statuses'], {'success': 44, 'error': 14})
        self.assertEqual(hashes, compare.profiles.PROFILES[cases['module_uri']]['input_hashes'])
        archive = compare.java_members(fixture)
        self.assertEqual(archive['repeat-0/028-direct-dependent-base/result.stdout.bin'],
                         b'hello-base')
        self.assertEqual(archive['repeat-0/031-dynamic-dependent-source-name/result.stdout.bin'],
                         b'hello-base')
        self.assertEqual(archive['repeat-0/032-dynamic-dependent-source-name-read/result.stdout.bin'],
                         b'source')

    def test_lookup_profile_selects_133_rows_and_distinct_cache_observation(self):
        fixture = compare.ROOT / 'tests/fixtures/m2-collection-lookups'
        cases, hashes, authority = compare.canonical_input(fixture)
        self.assertEqual(len(cases['rows']), 133)
        self.assertEqual(cases['module_uri'], 'file:///stage62/collection-lookups.pkl')
        self.assertEqual(authority['statuses'], {'success': 82, 'error': 51})
        self.assertEqual(hashes, compare.profiles.PROFILES[cases['module_uri']]['input_hashes'])
        archive = compare.java_members(fixture)
        self.assertEqual(archive['repeat-0/022-dyn-cache-module-before/result.stdout.bin'],
                         b'false')
        self.assertEqual(archive['repeat-0/024-dyn-cache-module-after/result.stdout.bin'],
                         b'true')
        self.assertEqual(archive['repeat-0/026-dyn-cache-module-get-after/result.status.bin'],
                         b'error\n')

    def test_additional_profiles_keep_distinct_authority(self):
        for uri, rows, statuses, channel, expected in (
            ('file:///stage62/collection-default-contracts.pkl', 22,
             {'success': 12, 'error': 10},
             'repeat-0/000-list-nonfunction-missing/result.status.bin', b'error\n'),
            ('file:///stage62/dynamic-render.pkl', 10,
             {'success': 10},
             'repeat-0/000-high-to-string/result.stdout.code-units.bin',
             'new Dynamic { `\ud800` = 7 }'.encode('utf-16-be', 'surrogatepass')),
            ('file:///stage62/collection-property-names.pkl', 26,
             {'success': 26},
             'repeat-0/008-amended-to-map/result.stdout.code-units.bin',
             'Map("\ud800", 1, "\x00property:[55296]", 2)'.encode('utf-16-be', 'surrogatepass')),
            ('file:///stage63/collection-addition.pkl', 125,
             {'success': 86, 'error': 39},
             'repeat-0/001-ls-ordered/result.stdout.code-units.bin',
             'List(1, 2, 2, 3)'.encode('utf-16-be')),
            ('file:///stage63/set-endpoints.pkl', 70,
             {'success': 47, 'error': 23},
             'repeat-0/001-set-empty-first-or-null/result.stdout.code-units.bin',
             'null'.encode('utf-16-be')),
            ('file:///stage64/amendment-construction.pkl', 188,
             {'success': 102, 'error': 86},
             'repeat-0/000-module-first/result.stdout.code-units.bin',
             '1'.encode('utf-16-be')),
        ):
            with self.subTest(uri=uri):
                fixture = compare.ROOT / compare.profiles.PROFILES[uri]['fixture']
                cases, hashes, authority = compare.canonical_input(fixture)
                self.assertEqual((cases['module_uri'], len(cases['rows']), authority['statuses']),
                                 (uri, rows, statuses))
                self.assertEqual(hashes, compare.profiles.PROFILES[uri]['input_hashes'])
                self.assertEqual(compare.java_members(fixture)[channel], expected)
                self.assertTrue(set(compare.profiles.PROFILES[uri]['tool_sources'])
                                <= compare.CAPTURE_SOURCES)

    def test_actual_java_session_overrides_preflight_locale_and_rejects_drift(self):
        payload = {
            'environment': {'TZ': 'UTC', 'PATH': '/bin:/usr/bin', 'NO_COLOR': '1'},
            'sessions': [["java", '-Duser.language=en', '-Duser.country=US',
                          '-Duser.timezone=UTC'] for _ in range(2)],
            'jdk_properties': {'user.language': 'ko', 'user.country': 'KR'},
        }
        execution = compare.derive_java_execution(payload)
        self.assertEqual((execution['locale'], execution['timezoneId'],
                          execution['preflightLocale']), ('en-US', 'UTC', 'ko-KR'))
        for change in (
            lambda p: p['environment'].pop('TZ'),
            lambda p: p['environment'].update(TZ='Asia/Seoul'),
            lambda p: p['sessions'][1].__setitem__(1, '-Duser.language=ko'),
            lambda p: p['sessions'][0].append('-Duser.language=en'),
            lambda p: p['sessions'][0].remove('-Duser.timezone=UTC'),
        ):
            changed = json.loads(json.dumps(payload))
            change(changed)
            with self.assertRaises(ValueError):
                compare.derive_java_execution(changed)

    def test_provenance_extraction_and_actual_target_environment_are_admitted(self):
        provenance = {
            'environment': {'TZ': 'UTC', 'PATH': '/bin:/usr/bin'},
            'sessions': [{'command': ['java', '-Duser.language=en', '-Duser.country=US',
                                      '-Duser.timezone=UTC']} for _ in range(2)],
            'jdk': {'properties': {'user.language': 'ko', 'user.country': 'KR'}},
        }
        payload = {'environment': provenance['environment'],
                   'sessions': [row['command'] for row in provenance['sessions']],
                   'jdk_properties': provenance['jdk']['properties']}
        execution = compare.derive_java_execution(payload)
        stdout = (json.dumps(payload, sort_keys=True) + '\n').encode()
        (self.path / 'java-provenance.stdout.bin').write_bytes(stdout)
        (self.path / 'java-provenance.stderr.bin').write_bytes(b'')
        fixture = self.path / 'fixture'
        report = {
            'python': {'executable': '/pinned/python3'},
            'javaExecution': execution,
            'javaProvenanceExtraction': {
                'command': ['/pinned/python3', '-c', compare.JAVA_PROVENANCE_SCRIPT,
                            str(fixture)],
                'cwd': str(compare.ROOT), 'exitCode': 0, 'signal': None, 'error': None,
                'stdoutSha256': compare.sha(stdout), 'stderrSha256': compare.sha(b''),
            },
        }
        java = {'provenance.json': json.dumps(provenance).encode()}
        self.assertEqual(compare.verify_java_environment(self.path, fixture, report, java), execution)
        self.assertEqual(compare.verify_target_environment(
            execution, 'napi', {}, {'environment': execution['environment']}, 'darwin'), {})
        self.assertEqual(compare.verify_target_environment(execution, 'napi', {},
                         {'environment': {**execution['environment'],
                                          '__CF_USER_TEXT_ENCODING': '0x1F5:0x0:0x0'}}, 'darwin'),
                         {'__CF_USER_TEXT_ENCODING': '0x1F5:0x0:0x0'})
        compare.verify_target_environment(execution, 'chromium', {
            'contextObserved': {'locale': 'en-US', 'timezoneId': 'UTC'}})
        with self.assertRaisesRegex(ValueError, 'child environment'):
            compare.verify_target_environment(execution, 'napi', {}, {'environment': {'TZ': 'UTC'}}, 'darwin')
        with self.assertRaisesRegex(ValueError, 'child environment'):
            compare.verify_target_environment(execution, 'napi', {},
                {'environment': {**execution['environment'], 'TZ': 'Asia/Seoul'}}, 'darwin')
        with self.assertRaisesRegex(ValueError, 'unexpected child environment'):
            compare.verify_target_environment(execution, 'napi', {},
                {'environment': {**execution['environment'], 'EXTRA': 'bad'}}, 'darwin')
        with self.assertRaisesRegex(ValueError, 'unexpected child environment'):
            compare.verify_target_environment(execution, 'napi', {},
                {'environment': {**execution['environment'], '__CF_USER_TEXT_ENCODING': 'x'}}, 'linux')
        with self.assertRaisesRegex(ValueError, 'observed locale'):
            compare.verify_target_environment(execution, 'chromium', {
                'contextObserved': {'locale': 'ko-KR', 'timezoneId': 'UTC'}})
        for field, value in [('javaExecution', {'locale': 'ko-KR'}),
                             ('javaProvenanceExtraction', {'command': ['unverified']})]:
            changed = dict(report)
            changed[field] = value
            with self.assertRaises(ValueError):
                compare.verify_java_environment(self.path, fixture, changed, java)
        (self.path / 'java-provenance.stdout.bin').write_bytes(stdout + b' ')
        with self.assertRaisesRegex(ValueError, 'extraction bytes'):
            compare.verify_java_environment(self.path, fixture, report, java)


if __name__ == '__main__':
    unittest.main()
