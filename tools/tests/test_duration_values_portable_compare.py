"""Failure injection for Duration portable raw comparison admission."""
from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import duration_values_portable_compare as compare


class PortableDurationComparisonTests(unittest.TestCase):
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
            self.assertEqual(compare.verify_m1(m1, capture)['source_count'], 5)
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
        rows = [{'status': 'pass'}] * 544 + [{'status': 'mismatch'}] * 4
        self.assertFalse(compare.strict_target(rows, sessions))
        self.assertTrue(compare.strict_target([{'status': 'pass'}] * 548, sessions))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 547, sessions))
        self.assertFalse(compare.strict_target([{'status': 'pass'}] * 548,
                                               [{'issues': ['protocol']}, {'issues': []}]))

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
        self.assertEqual(len(cases['rows']), 274)
        self.assertEqual(authority['statuses'], {'success': 240, 'error': 34})
        self.assertEqual(set(hashes), {'cases.json', 'module.pkl', 'queries.tsv'})
        archive = compare.java_members(compare.FIXTURE)
        row = archive['repeat-0/000-int-ns/result.stdout.bin']
        self.assertEqual(row, b'7.ns')
        self.assertEqual(compare.gate_exit({'integrity': 'passed',
                                            'strict_string_passed': False}, 'integrity'), 0)
        self.assertEqual(compare.gate_exit({'integrity': 'passed',
                                            'strict_string_passed': False}, 'string'), 1)
        self.assertEqual(compare.gate_exit({'integrity': 'passed',
                                            'strict_string_passed': True}, 'full-api'), 1)


if __name__ == '__main__':
    unittest.main()
