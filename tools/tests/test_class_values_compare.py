"""Synthetic transport/tamper checks; these never execute a Pkl candidate."""
from contextlib import contextmanager, redirect_stdout
import copy
import json
import io
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import class_values_compare as compare


PROTOCOL = b'call-enter=outer-0\ncall-exit=outer-0 status=success\nclosed logs=0 warnings=0\n'
SOURCE = b'answer = 42\n'


def text(directory, name, data):
    (directory / (name + '.bin')).write_bytes(data.encode('utf-8', 'replace'))
    (directory / (name + '.code-units.bin')).write_bytes(data.encode('utf-16-be', 'surrogatepass'))


def populate(directory, java=False):
    directory.mkdir(parents=True)
    text(directory, 'outer-0.stdout', '42')
    text(directory, 'outer-0.stderr', '')
    (directory / 'outer-0.status.bin').write_bytes(b'success\n')
    if java:
        (directory / 'outer-0.error-class.bin').write_bytes(b'')
    else:
        for field in ['disposition', 'phase', 'reason', 'callback-origin']:
            (directory / ('outer-0.' + field + '.bin')).write_bytes(b'')
        (directory / 'candidate-trace.tsv').write_bytes(
            b'constructor\t("module-evaluator", "compact", true, false, false)\n'
            b'terminal-heap-slots\t0\nterminal-context-depth\t0\npolls\t1\ncollections\t2\n')
        (directory / 'protocol.bin').write_bytes(PROTOCOL)
    (directory / 'process').mkdir()
    (directory / 'process/stdout.bin').write_bytes(PROTOCOL)
    (directory / 'process/stderr.bin').write_bytes(b'')
    if not java:
        compare.put(directory / 'process/result.json', {})


class RawComparisonTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.java, self.native = self.root / 'java', self.root / 'native'
        populate(self.java, java=True)
        populate(self.native)
        self.case = dict(id='sample', expressions=['answer'], support_modules=[],
                         logger=dict(mode='capture', action='none', callback='', callback_after=0))
        self.process = dict(status='executed', exit_code=0)

    def comparison(self, process=None):
        return compare.compare_session(self.java, self.native, self.case, 'native', process or self.process)

    def test_raw_boundary_keeps_java_provenance_separate(self):
        (self.java / 'outer-0.error-class.bin').write_bytes(b'org.pkl.core.PklException')
        result = self.comparison()
        self.assertEqual(result['status'], 'pass')
        self.assertTrue(result['raw_equal'])
        self.assertIn('outer-0.error-class.bin', result['java_provenance'])
        self.assertEqual(len(result['candidate_provenance']), 4)

    def test_candidate_must_not_synthesize_even_empty_java_class_channels(self):
        (self.native / 'outer-0.error-class.bin').write_bytes(b'')
        result = self.comparison()
        self.assertFalse(result['exact_coverage'])
        self.assertNotEqual(result['status'], 'pass')
        self.assertIn('candidate synthesized Java exception provenance', result['issues'])

    def test_unknown_metadata_looking_file_is_a_real_extra_channel(self):
        (self.native / 'log-0.reason.bin').write_bytes(b'ignored?')
        result = self.comparison()
        self.assertFalse(result['exact_coverage'])
        self.assertIn('log-0.reason.bin', [row['name'] for row in result['channels']])

    def test_ghost_call_metadata_cannot_be_silently_excluded(self):
        (self.native / 'outer-999.phase.bin').write_bytes(b'evaluation')
        result = self.comparison()
        self.assertNotEqual(result['status'], 'pass')
        self.assertTrue(any('without matching public call' in issue for issue in result['issues']))

    def test_unexpected_native_process_channel_cannot_be_hidden(self):
        (self.native / 'process/unexpected.bin').write_bytes(b'extra channel')
        result = self.comparison()
        self.assertNotEqual(result['status'], 'pass')
        self.assertIn('native process file coverage differs', result['issues'])

    def test_missing_unknown_or_successful_callback_failure_origin_cannot_pass(self):
        path = self.native / 'outer-0.callback-origin.bin'
        path.unlink()
        self.assertTrue(any('callback origin' in issue for issue in self.comparison()['issues']))
        path.write_bytes(b'invented-host-stage')
        self.assertTrue(any('callback origin' in issue for issue in self.comparison()['issues']))
        path.write_bytes(b'diagnostic-transformer')
        result = self.comparison()
        self.assertNotEqual(result['status'], 'pass')
        self.assertIn('successful call carries callback failure origin: outer-0', result['issues'])

    def test_callback_origin_metadata_cannot_invent_a_nested_call(self):
        (self.native / 'log-callback-99.callback-origin.bin').write_bytes(b'log-delegate')
        result = self.comparison()
        self.assertNotEqual(result['status'], 'pass')
        self.assertTrue(any('without matching public call' in issue for issue in result['issues']))

    def test_whitespace_and_surrogate_units_are_not_normalized(self):
        result = compare.compare_bytes({'error': b'one\r\ntwo', 'units': b'\xd8\x00'},
                                       {'error': b'one\ntwo', 'units': b'\xff\xfd'})
        self.assertFalse(result['raw_equal'])
        self.assertTrue(all(not row['raw_equal'] for row in result['channels']))

    def test_missing_outer_channels_never_pass_even_when_both_outputs_are_empty(self):
        for path in self.native.glob('outer-0.*.bin'):
            path.unlink()
        result = self.comparison()
        self.assertNotEqual(result['status'], 'pass')
        self.assertIn('missing outer call channels', result['issues'])

    def test_matching_api_exception_stays_a_nonpass(self):
        for directory in [self.java, self.native]:
            (directory / 'outer-0.status.bin').write_bytes(b'api-exception\n')
            text(directory, 'outer-0.stderr', 'closed')
        for field in ['disposition', 'phase', 'reason']:
            (self.native / ('outer-0.' + field + '.bin')).unlink()
        (self.native / 'outer-0.api-error-kind.bin').write_bytes(b'Closed')
        result = self.comparison()
        self.assertTrue(result['raw_equal'])
        self.assertEqual(result['status'], 'api-exception')
        self.assertEqual(result['reference_nonpasses'], {'outer-0': 'api-exception'})

    def test_matching_oracle_error_is_never_promoted(self):
        for directory in [self.java, self.native]:
            (directory / 'outer-0.status.bin').write_bytes(b'oracle-error\n')
        result = self.comparison()
        self.assertTrue(result['raw_equal'])
        self.assertNotEqual(result['status'], 'pass')
        self.assertIn('oracle-error', result['nonpass_reasons'])

    def test_unsupported_and_timeout_keep_their_failure_classes(self):
        (self.native / 'outer-0.disposition.bin').write_bytes(b'unsupported')
        (self.native / 'outer-0.status.bin').write_bytes(b'candidate-error\n')
        self.assertEqual(self.comparison()['status'], 'candidate-unsupported')
        self.assertEqual(self.comparison(dict(status='timeout', exit_code=-9))['status'], 'candidate-timeout')

    def test_nonzero_or_duplicate_terminal_heap_records_cannot_pass(self):
        path = self.native / 'candidate-trace.tsv'
        original = path.read_bytes()
        path.write_bytes(original.replace(b'heap-slots\t0', b'heap-slots\t1'))
        self.assertEqual(self.comparison()['status'], 'architecture-failed')
        path.write_bytes(original + b'terminal-context-depth\t0\n')
        self.assertFalse(self.comparison()['architecture']['valid'])

    def test_portable_owned_buffer_failure_is_not_a_raw_match_pass(self):
        path = self.root / 'portable'
        path.mkdir()
        architecture = dict(sessionCreated=True, terminalSlots=0, terminalDepth=0,
                            retainedPayloadsStable=False, polls=1, collections=2)
        compare.put(path / 'candidate-trace.json', dict(architecture=architecture,
            acquisitionError=None, protocol=['closed logs=0 warnings=0']))
        (path / 'protocol.bin').write_bytes(b'closed logs=0 warnings=0\n')
        compare.put(path / 'process.json', dict(exitCode=0, signal=None, error=None))
        compare.put(path / 'child.json', dict(architecture=architecture, acquisitionError=None))
        for name in ['process.stdout.bin', 'process.stderr.bin']:
            (path / name).write_bytes(b'')
        result = compare.portable_architecture(path, 'napi')
        self.assertFalse(result['valid'])
        self.assertFalse(result['checks']['owned_buffers_stable'])

    def test_boolean_false_is_not_a_zero_heap_depth_or_process_exit(self):
        path = self.root / 'portable-boolean'
        path.mkdir()
        architecture = dict(sessionCreated=True, terminalSlots=False, terminalDepth=False,
                            retainedPayloadsStable=True, polls=1, collections=2)
        compare.put(path / 'candidate-trace.json', dict(architecture=architecture,
            acquisitionError=None, protocol=PROTOCOL.decode().splitlines()))
        (path / 'protocol.bin').write_bytes(PROTOCOL)
        compare.put(path / 'process.json', dict(exitCode=False, signal=None, error=None))
        compare.put(path / 'child.json', dict(architecture=architecture, acquisitionError=None))
        for name in ['process.stdout.bin', 'process.stderr.bin']:
            (path / name).write_bytes(b'')
        result = compare.portable_architecture(path, 'napi')
        self.assertFalse(result['valid'])
        for name in ['terminal_heap_slots', 'terminal_context_depth', 'process_exit']:
            self.assertFalse(result['checks'][name])

    def test_metadata_json_equality_preserves_boolean_number_distinctions(self):
        self.assertFalse(compare.same_json({'callback_after': False}, {'callback_after': 0}))
        self.assertFalse(compare.same_json({'repeat': True}, {'repeat': 1}))

    def test_napi_process_diagnostics_are_separate_but_cannot_hide_a_failure(self):
        path = self.root / 'portable-comparison'
        path.mkdir()
        for channel in self.native.glob('*.bin'):
            (path / channel.name).write_bytes(channel.read_bytes())
        architecture = dict(sessionCreated=True, terminalSlots=0, terminalDepth=0,
                            retainedPayloadsStable=True, polls=1, collections=2)
        compare.put(path / 'candidate-trace.json', dict(architecture=architecture,
            acquisitionError=None, protocol=PROTOCOL.decode().splitlines()))
        compare.put(path / 'process.json', dict(exitCode=0, signal=None, error=None))
        compare.put(path / 'child.json', dict(architecture=architecture, acquisitionError=None))
        for name in ['process.stdout.bin', 'process.stderr.bin']:
            (path / name).write_bytes(b'')
        result = compare.compare_session(self.java, path, self.case, 'napi', self.process)
        self.assertEqual(result['status'], 'pass')
        self.assertIn('process.stderr.bin', result['candidate_provenance'])
        (path / 'process.stderr.bin').write_bytes(b'actual child diagnostic')
        result = compare.compare_session(self.java, path, self.case, 'napi', self.process)
        self.assertTrue(result['raw_equal'])
        self.assertEqual(result['status'], 'architecture-failed')

    def test_launch_error_preserves_an_explicit_unexecuted_process(self):
        path = self.root / 'launch'
        result = compare.capture([str(self.root / 'missing-binary')], self.root, {}, path, 1)
        self.assertEqual(result['status'], 'launch-error')
        self.assertIsNone(result['exit_code'])
        self.assertIsNotNone(result['error'])
        compare.verify_process(path, result)

    def test_timeout_retains_partial_stdout_and_is_independently_auditable(self):
        path = self.root / 'timeout'
        result = compare.capture([sys.executable, '-c',
            'import sys,time;sys.stdout.write("partial");sys.stdout.flush();time.sleep(30)'],
            self.root, {}, path, 1)
        self.assertEqual(result['status'], 'timeout')
        self.assertEqual((path / 'stdout.bin').read_bytes(), b'partial')
        compare.verify_process(path, result)


class EvidenceTamperTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.output = self.root / 'evidence'
        self.oracle = self.root / 'synthetic-java'
        self.output.mkdir()
        self.case = dict(id='sample', source='inputs/sample.pkl', source_sha256=compare.digest(SOURCE),
                         uri='file:///synthetic.pkl', support_modules=[],
                         logger=dict(mode='capture', action='none', callback='', callback_after=0),
                         expressions=['answer'])
        self.manifest = dict(cases=[self.case], repetitions=2)
        self.verification = dict(provider_supplement={'callback_counts': {'create': 2, 'resolve': 0}},
                                 public_export_supplement={'sessions': 1, 'identity_comparisons': 2})
        fixture = self.output / 'fixture/inputs'
        fixture.mkdir(parents=True)
        (fixture / 'sample.pkl').write_bytes(SOURCE)
        source = self.output / 'sources/source.rs'
        source.parent.mkdir()
        source.write_bytes(b'synthetic source snapshot\n')
        catalog_source = self.output / 'sources/tools/m2_source_inputs.py'
        catalog_source.parent.mkdir()
        catalog_source.write_bytes(b'synthetic catalog implementation\n')
        sources = {'source.rs': compare.sha(source), 'tools/m2_source_inputs.py': compare.sha(catalog_source)}
        compare.put(self.output / 'source-catalog.json', dict(schema=1, paths=sorted(sources),
                    catalog_source_sha256=sources['tools/m2_source_inputs.py']))
        artifact = self.output / 'artifacts/class_values_fixture_probe'
        artifact.parent.mkdir()
        artifact.write_bytes(b'synthetic candidate bytes; never executed')
        binding = dict(origin='/recorded/original/native', frozen=str(artifact),
                       origin_before=compare.sha(artifact), frozen_before=compare.sha(artifact),
                       origin_after=compare.sha(artifact), frozen_after=compare.sha(artifact), admission_error=None)
        self.report = dict(schema=1, boundary=compare.BOUNDARY, output_directory=str(self.output),
            workspace='/recorded/workspace', environment={'LANG': 'C'},
            source_binding='supplied-artifacts-with-source-snapshot; no recorded-build claim',
            requested_targets=['native'], sources_before=sources, sources_after=sources.copy(),
            source_catalog_sha256=compare.sha(self.output / 'source-catalog.json'),
            source_changes=[], artifacts={'native': binding}, wasm_file_sets=None,
            artifact_changes=[], infrastructure_errors=[], portable_issues=[], layout_issues=[], fixture_verification=self.verification,
            case_ids=['sample'], repetitions=2,
            **compare.supplements(self.verification), status_scope=compare.MAIN_SCOPE,
            full_public_api_comparison_passed=False, full_public_api_status='blocked-unexecuted-required')
        for repeat in [0, 1]:
            populate(self.oracle / 'cases/sample' / str(repeat), java=True)
            directory = self.output / 'native/sample' / str(repeat)
            populate(directory)
            (directory / 'input.pkl').write_bytes(SOURCE)
            (directory / 'expressions.txt').write_bytes(b'answer\n')
            (directory / 'modules.tsv').write_bytes(b'')
            process = dict(command=compare.native_arguments(artifact, directory, self.case),
                cwd=self.report['workspace'], environment=self.report['environment'], timeout_seconds=1,
                elapsed_ns=1, status='executed', exit_code=0, error=None,
                stdout=dict(sha256=compare.digest(PROTOCOL), bytes=len(PROTOCOL)),
                stderr=dict(sha256=compare.digest(b''), bytes=0))
            compare.put(directory / 'process/result.json', process)
        self.report['targets'] = compare.derive(self.output, self.report, self.manifest, self.oracle)
        self.report['strict_main_comparison_passed'] = compare.strict_passes(self.report)
        self.report['status'] = 'passed'
        compare.put(self.output / 'report.json', self.report)
        self.refresh()

        @contextmanager
        def fake_authority(_):
            yield self.manifest, self.verification, self.oracle

        self.patcher = patch.object(compare, 'authority', fake_authority)
        self.patcher.start()
        self.addCleanup(self.patcher.stop)
        for name, value in [('MANDATORY_SOURCES', frozenset(sources)), ('source_names', lambda root=None: sorted(sources))]:
            patcher = patch.object(compare, name, value)
            patcher.start()
            self.addCleanup(patcher.stop)

    def refresh(self):
        inventory = compare.files(self.output)
        inventory.pop('inventory.json', None)
        compare.put(self.output / 'inventory.json', inventory)

    def edit_report(self, change):
        report = compare.read(self.output / 'report.json')
        change(report)
        compare.put(self.output / 'report.json', report)
        self.refresh()

    def test_valid_synthetic_packet_verifies_without_executing_a_candidate(self):
        result = compare.verify_report(self.output)
        self.assertTrue(result['strict_main_comparison_passed'])

    def test_raw_replacement_is_rejected_after_inventory_refresh(self):
        (self.output / 'native/sample/0/outer-0.stdout.bin').write_bytes(b'forged')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'raw comparison/result accounting'):
            compare.verify_report(self.output)

    def test_dropped_session_is_rejected_after_report_counts_are_refreshed(self):
        self.edit_report(lambda report: report['targets']['native']['rows'].pop())
        with self.assertRaisesRegex(ValueError, 'result accounting'):
            compare.verify_report(self.output)

    def test_source_snapshot_replacement_cannot_be_hidden_by_inventory_refresh(self):
        (self.output / 'sources/source.rs').write_bytes(b'changed source')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'source snapshot identity'):
            compare.verify_report(self.output)

    def test_frozen_artifact_replacement_cannot_be_hidden_by_inventory_refresh(self):
        (self.output / 'artifacts/class_values_fixture_probe').write_bytes(b'changed artifact')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'frozen artifact identity'):
            compare.verify_report(self.output)

    def test_protocol_file_must_be_actual_native_process_stdout(self):
        (self.output / 'native/sample/0/protocol.bin').write_bytes(b'forged protocol\n')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'actual process stdout'):
            compare.verify_report(self.output)

    def test_provider_and_export_supplements_cannot_be_silently_promoted_or_stripped(self):
        original = compare.read(self.output / 'report.json')
        for key in ['provider_supplement', 'public_export_supplement']:
            for edit in ['promotion', 'authority-removal', 'deletion']:
                with self.subTest(key=key, edit=edit):
                    report = copy.deepcopy(original)
                    if edit == 'promotion':
                        report[key]['status'] = 'passed'
                    elif edit == 'authority-removal':
                        report[key]['authority'] = {}
                    else:
                        report.pop(key)
                    compare.put(self.output / 'report.json', report)
                    self.refresh()
                    with self.assertRaises((ValueError, KeyError)):
                        compare.verify_report(self.output)
        compare.put(self.output / 'report.json', original)
        self.refresh()

    def test_full_api_boolean_or_status_cannot_promote_unexecuted_obligations(self):
        original = compare.read(self.output / 'report.json')
        for value in [True, 0, 1, None]:
            self.edit_report(lambda report: report.update(full_public_api_comparison_passed=value))
            with self.assertRaisesRegex(ValueError, 'public API obligations were promoted'):
                compare.verify_report(self.output)
        compare.put(self.output / 'report.json', original)
        self.edit_report(lambda report: report.update(full_public_api_status='passed'))
        with self.assertRaisesRegex(ValueError, 'public API obligations were promoted'):
            compare.verify_report(self.output)

    def test_main_scope_cannot_be_promoted_to_entire_java_session(self):
        self.edit_report(lambda report: report.update(status_scope='whole-public-api'))
        with self.assertRaisesRegex(ValueError, 'main comparison scope'):
            compare.verify_report(self.output)


    def test_cli_main_gate_passes_common_packet_while_full_api_gate_stays_failed(self):
        for mode, expected in [('--verify-report', 0), ('--verify-main', 0), ('--verify-full', 1)]:
            with self.subTest(mode=mode), patch.object(sys, 'argv', ['class_values_compare.py', mode, str(self.output)]):
                stream = io.StringIO()
                with redirect_stdout(stream):
                    actual = compare.main()
                self.assertEqual(actual, expected)
                result = json.loads(stream.getvalue())
                self.assertTrue(result['strict_main_comparison_passed'])
                self.assertIs(result['full_public_api_comparison_passed'], False)
                for key in ['provider_supplement', 'public_export_supplement']:
                    self.assertEqual(result[key]['status'], 'not-executed-required')

    def test_native_support_bytes_uri_mapping_and_extra_files_cannot_be_self_resealed(self):
        support = b'module child\r\nclass C {}\r\n'
        source = self.output / 'fixture/inputs/child.pkl'
        source.write_bytes(support)
        self.case['support_modules'] = [dict(uri='file:///classes/child.pkl', source='inputs/child.pkl',
                                              source_sha256=compare.digest(support))]
        for repeat in [0, 1]:
            directory = self.output / 'native/sample' / str(repeat)
            for name, data in compare.native_inputs(self.case, self.output / 'fixture').items():
                (directory / name).write_bytes(data)
        self.report['targets'] = compare.derive(self.output, self.report, self.manifest, self.oracle)
        compare.put(self.output / 'report.json', self.report)
        self.refresh()
        self.assertTrue(compare.verify_report(self.output)['strict_main_comparison_passed'])
        directory = self.output / 'native/sample/0'
        originals = {name: (directory / name).read_bytes() for name in ['modules.tsv', 'support-0.pkl']}
        changes = [('support-0.pkl', b'forged support payload'),
                   ('modules.tsv', b'file:///classes/other.pkl\tsupport-0.pkl\n'),
                   ('modules.tsv', b'file:///classes/child.pkl\tsupport-1.pkl\n')]
        for name, data in changes:
            with self.subTest(name=name, data=data):
                (directory / name).write_bytes(data)
                self.refresh()
                derived = compare.derive(self.output, self.report, self.manifest, self.oracle)
                self.assertEqual(derived['native']['rows'][0]['status'], 'acquisition-failed')
                with self.assertRaisesRegex(ValueError, 'result accounting'):
                    compare.verify_report(self.output)
                (directory / name).write_bytes(originals[name])
        (directory / 'support-999.pkl').write_bytes(b'unconfigured source')
        self.refresh()
        derived = compare.derive(self.output, self.report, self.manifest, self.oracle)
        self.assertIn('unexpected candidate artifact: support-999.pkl', derived['native']['rows'][0]['issues'])
        self.assertNotEqual(derived['native']['rows'][0]['status'], 'pass')

    def test_source_catalog_binding_cannot_be_replaced_after_its_hash_is_refreshed(self):
        catalog = compare.read(self.output / 'source-catalog.json')
        catalog['catalog_source_sha256'] = 'f' * 64
        compare.put(self.output / 'source-catalog.json', catalog)
        self.edit_report(lambda report: report.update(source_catalog_sha256=compare.sha(self.output / 'source-catalog.json')))
        with self.assertRaisesRegex(ValueError, 'source catalog binding'):
            compare.verify_report(self.output)

    def test_empty_source_snapshot_cannot_be_self_resealed(self):
        shutil.rmtree(self.output / 'sources')
        self.edit_report(lambda report: report.update(sources_before={}, sources_after={}))
        with self.assertRaisesRegex(ValueError, 'mandatory source catalog'):
            compare.verify_report(self.output)

    def test_extra_case_directory_cannot_be_self_resealed(self):
        directory = self.output / 'native/unexpected/0'
        directory.mkdir(parents=True)
        (directory / 'anything.bin').write_bytes(b'extra')
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'directory coverage differs'):
            compare.verify_report(self.output)

    def test_missing_required_native_artifact_declaration_is_rejected(self):
        (self.output / 'artifacts/class_values_fixture_probe').unlink()
        self.edit_report(lambda report: report.update(artifacts={}))
        with self.assertRaisesRegex(ValueError, 'artifact closure differs'):
            compare.verify_report(self.output)

    def test_portable_artifact_closure_cannot_omit_delivered_wasm(self):
        report = copy.deepcopy(self.report)
        report['requested_targets'] = ['napi', 'chromium']
        report['artifacts'] = {}
        report['wasm_file_sets'] = dict(before=[], after=[])
        with self.assertRaisesRegex(ValueError, 'artifact closure differs'):
            compare.verify_artifact_closure(self.output, report)

    def test_changed_sources_are_recordable_failures_never_strict_passes(self):
        report = copy.deepcopy(self.report)
        report['source_changes'] = ['source.rs']
        self.assertFalse(compare.strict_passes(report))

    def test_nonpass_rows_cannot_be_removed_from_the_full_gate(self):
        report = copy.deepcopy(self.report)
        report['targets']['native']['statuses'] = {'pass': 1, 'oracle-error': 1}
        self.assertFalse(compare.strict_passes(report))

    def test_partial_portable_capture_still_accounts_for_every_required_pair(self):
        report = copy.deepcopy(self.report)
        report['requested_targets'] = ['napi', 'chromium']
        targets = compare.derive(self.output, report, self.manifest, self.oracle)
        for name in ['napi', 'chromium']:
            self.assertEqual(len(targets[name]['rows']), 2)
            self.assertEqual(targets[name]['statuses'], {'acquisition-failed': 2})

    def test_malformed_portable_report_is_a_recorded_admission_failure(self):
        directory = self.output / 'portable'
        directory.mkdir()
        (directory / 'report.json').write_bytes(b'{partial')
        portable, issues = compare.portable_admission(self.output, self.report, self.manifest)
        self.assertIsNone(portable)
        self.assertTrue(issues)

    def prepare_portable_admission_packet(self):
        report = copy.deepcopy(self.report)
        base = self.output / 'portable'
        base.mkdir()
        compare.put(self.output / 'fixture/manifest.json', {})
        compare.put(base / 'fixture-verification.stdout.bin', self.verification)
        report['node_runtime'] = dict(executable='/runtime/node', before_sha256='0' * 64, after_sha256='0' * 64)
        for name, value in [('package.json', {'devDependencies': {'playwright': '1.2.3'}}),
                            ('package-lock.json', {'packages': {
                                '': {'devDependencies': {'playwright': '1.2.3'}},
                                'node_modules/playwright': {'version': '1.2.3'},
                                'node_modules/playwright-core': {'version': '1.2.3'}}})]:
            path = self.output / 'sources/tools/platform' / name
            compare.put(path, value)
            report['sources_before']['tools/platform/' + name] = compare.sha(path)
        for name, relative in [('napi', 'artifacts/class-values.node'),
                               ('wasm/pkl_wasm.js', 'artifacts/wasm/pkl_wasm.js'),
                               ('wasm/pkl_wasm_bg.wasm', 'artifacts/wasm/pkl_wasm_bg.wasm')]:
            path = self.output / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'synthetic bytes; never executed')
            report['artifacts'][name] = dict(frozen=str(path), frozen_before=compare.sha(path))
        tracked, sets = {}, {}
        for name in ['fixture', 'artifacts/wasm']:
            path = self.output / name
            names = compare.files(path)
            sets[str(path)] = [str(path / name) for name in names]
            tracked.update({str(path / name): record['sha256'] for name, record in names.items()})
        tracked.update({'/runtime/node': '0' * 64, '/runtime/chromium': '1' * 64,
                        report['artifacts']['napi']['frozen']: report['artifacts']['napi']['frozen_before']})
        for name in compare.portable_tracked_sources(self.manifest):
            path = self.output / 'sources' / name
            if not path.exists():
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'synthetic tracked driver input')
            report['sources_before'][name] = compare.sha(path)
            tracked[str(Path(report['workspace']) / name)] = compare.sha(path)
        script = str(Path(report['workspace']) / 'tools/platform/capture-class-values.mjs')
        portable = dict(schema=1, caseIds=['sample'], repetitions=2, infrastructureErrors=[],
            fixtureManifestSha256=compare.sha(self.output / 'fixture/manifest.json'),
            inputsBefore=tracked, inputsAfter=tracked.copy(), fileSetsBefore=sets, fileSetsAfter=sets.copy(),
            node=dict(executable='/runtime/node', version='v1.2.3', versions={'node': '1.2.3'}),
            environment={'LANG': 'C'}, playwright='1.2.3',
            browser=dict(executable='/runtime/chromium', version='123', sha256='1' * 64),
            argv=['/runtime/node', script, str(base), str(self.output / 'artifacts/wasm'),
                  report['artifacts']['napi']['frozen'], str(self.output / 'fixture')],
            targets={'napi': [], 'chromium': []})
        for target in ['napi', 'chromium']:
            for repeat in [0, 1]:
                directory = base / target / 'sample' / str(repeat)
                directory.mkdir(parents=True)
                row = dict(id='sample', repeat=repeat, directory=str(directory))
                if target == 'napi':
                    command = ['/runtime/node', script, '--child', str(directory / 'input.json'),
                               str(directory), report['artifacts']['napi']['frozen']]
                    row.update(command=command, exitCode=0, signal=None, error=None)
                    compare.put(directory / 'child.json', dict(id='sample', node='v1.2.3', versions={'node': '1.2.3'},
                        executable='/runtime/node', argv=command, cwd=report['workspace']))
                compare.put(directory / 'process.json', row)
                portable['targets'][target].append(row)
        compare.put(base / 'report.json', portable)
        return report, portable, base

    def test_portable_frozen_input_runtime_and_command_admission(self):
        report, portable, base = self.prepare_portable_admission_packet()
        _, issues = compare.portable_admission(self.output, report, self.manifest)
        self.assertEqual(issues, [])
        mandatory = {str(Path(report['workspace']) / name) for name in compare.portable_tracked_sources(self.manifest)}
        mandatory.update({report['artifacts']['napi']['frozen'], '/runtime/node', '/runtime/chromium'})
        for name in sorted(mandatory):
            with self.subTest(missing_tracked_input=name):
                changed = copy.deepcopy(portable)
                del changed['inputsBefore'][name]
                del changed['inputsAfter'][name]
                compare.put(base / 'report.json', changed)
                _, issues = compare.portable_admission(self.output, report, self.manifest)
                self.assertTrue(issues)
                if name not in {'/runtime/node', '/runtime/chromium'}:
                    self.assertTrue(any('exact tracked-input coverage differs' in issue for issue in issues))
        portable['inputsBefore'][report['artifacts']['napi']['frozen']] = 'f' * 64
        portable['inputsAfter'] = portable['inputsBefore'].copy()
        compare.put(base / 'report.json', portable)
        _, issues = compare.portable_admission(self.output, report, self.manifest)
        self.assertTrue(any('captured input identity differs' in issue for issue in issues))


    def test_supplemental_portable_closure_requires_both_new_verifier_helpers(self):
        self.manifest['kind'] = compare.METADATA_KIND
        report, portable, base = self.prepare_portable_admission_packet()
        self.assertEqual(compare.portable_admission(self.output, report, self.manifest)[1], [])
        for name in compare.METADATA_SOURCES:
            with self.subTest(helper=name):
                changed = copy.deepcopy(portable)
                absolute = str(Path(report['workspace']) / name)
                del changed['inputsBefore'][absolute]
                del changed['inputsAfter'][absolute]
                compare.put(base / 'report.json', changed)
                issues = compare.portable_admission(self.output, report, self.manifest)[1]
                self.assertTrue(any('exact tracked-input coverage' in issue for issue in issues))

    def test_stripping_helpers_and_collector_literals_cannot_admit_supplement(self):
        self.manifest['kind'] = compare.METADATA_KIND
        for name in compare.METADATA_SOURCES:
            path = self.output / 'sources' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(b'# synthetic helper never executed\n')
            self.report['sources_before'][name] = compare.sha(path)
        collector = self.output / 'sources/tools/m2_source_inputs.py'
        collector.write_text('FILES = ' + repr(sorted(compare.METADATA_SOURCES)) + '\n')
        def update_catalog():
            self.report['sources_before']['tools/m2_source_inputs.py'] = compare.sha(collector)
            self.report['sources_after'] = self.report['sources_before'].copy()
            compare.put(self.output / 'source-catalog.json', dict(schema=1,
                paths=sorted(self.report['sources_before']),
                catalog_source_sha256=self.report['sources_before']['tools/m2_source_inputs.py']))
            self.report['source_catalog_sha256'] = compare.sha(self.output / 'source-catalog.json')
            compare.put(self.output / 'report.json', self.report)
            self.refresh()
        update_catalog()
        self.assertTrue(compare.verify_report(self.output)['strict_main_comparison_passed'])
        # The attack reseals every ordinary source/catalog/inventory field and
        # strips collector literals too. Authority kind still requires helpers.
        for name in compare.METADATA_SOURCES:
            (self.output / 'sources' / name).unlink()
            del self.report['sources_before'][name]
        collector.write_text('# historical catalog without metadata helper literals\n')
        update_catalog()
        with self.assertRaisesRegex(ValueError, 'mandatory supplemental source catalog paths missing'):
            compare.verify_report(self.output)

class ClassInputTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        (self.root / 'inputs').mkdir()
        self.primary = b'module main\r\nclass C {}\r\n'
        self.child = 'module child\nclass \u00e9 {}\n'.encode()
        (self.root / 'inputs/main.pkl').write_bytes(self.primary)
        (self.root / 'inputs/child.pkl').write_bytes(self.child)
        self.case = dict(id='input', uri='file:///classes/main.pkl', source='inputs/main.pkl',
            source_sha256=compare.digest(self.primary), expressions=['child.C == child.C', 'C.simpleName'],
            logger=dict(mode='capture', action='close', callback='nested()', callback_after=2),
            support_modules=[dict(uri='file:///classes/child.pkl', source='inputs/child.pkl', source_sha256=compare.digest(self.child))])

    def test_native_cli_and_input_map_preserve_all_exact_authored_support_bytes(self):
        self.assertEqual(compare.native_arguments('/candidate', '/output', self.case),
                         ['/candidate', '/output', 'file:///classes/main.pkl', 'capture', 'close', 'nested()', '2'])
        self.assertEqual(compare.native_inputs(self.case, self.root), {
            'input.pkl': self.primary, 'expressions.txt': b'child.C == child.C\nC.simpleName\n',
            'modules.tsv': b'file:///classes/child.pkl\tsupport-0.pkl\n', 'support-0.pkl': self.child})

    def test_primary_and_support_hashes_are_required_for_both_candidate_input_shapes(self):
        for target in ['primary', 'support']:
            for function in [compare.native_inputs, compare.portable_input]:
                with self.subTest(target=target, function=function.__name__):
                    case = copy.deepcopy(self.case)
                    record = case if target == 'primary' else case['support_modules'][0]
                    record['source_sha256'] = '0' * 64
                    with self.assertRaisesRegex(ValueError, 'fixture source digest'):
                        function(case, self.root)

    def test_portable_input_retains_primary_and_support_paths_text_uri_hashes_and_fallbacks(self):
        result = compare.portable_input(self.case, self.root)
        self.assertEqual(result['source'], self.primary.decode())
        self.assertEqual(result['sourcePath'], 'inputs/main.pkl')
        self.assertEqual(result['moduleName'], 'main')
        self.assertEqual(result['support_modules'], [{**self.case['support_modules'][0],
            'sourcePath': 'inputs/child.pkl', 'source': self.child.decode(), 'moduleName': 'child'}])
        self.assertEqual(self.case['source'], 'inputs/main.pkl')

    def test_boolean_native_process_exit_cannot_satisfy_success(self):
        java, actual = self.root / 'java', self.root / 'native'
        populate(java, java=True)
        populate(actual)
        for exit_code in [False, True, None, '0']:
            with self.subTest(exit_code=exit_code):
                row = compare.compare_session(java, actual, self.case, 'native', dict(status='executed', exit_code=exit_code))
                self.assertEqual(row['status'], 'acquisition-failed')

    def test_portable_support_source_hash_and_uri_tampering_prevents_main_pass(self):
        manifest = dict(cases=[self.case], repetitions=2)
        oracle = self.root / 'java'
        output = self.root / 'evidence'
        fixture = output / 'fixture'
        shutil.copytree(self.root / 'inputs', fixture / 'inputs')
        # This helper case has two expressions; use one exact authored call for
        # the synthetic transport packet, without running a Pkl evaluator.
        self.case['expressions'] = ['C.simpleName']
        architecture = dict(sessionCreated=True, terminalSlots=0, terminalDepth=0,
                            retainedPayloadsStable=True, polls=1, collections=2)
        for repeat in [0, 1]:
            populate(oracle / 'cases/input' / str(repeat), java=True)
            for target in ['napi', 'chromium']:
                directory = output / 'portable' / target / 'input' / str(repeat)
                populate(directory)
                (directory / 'candidate-trace.tsv').unlink()
                shutil.rmtree(directory / 'process')
                compare.put(directory / 'candidate-trace.json', dict(architecture=architecture,
                    acquisitionError=None, protocol=PROTOCOL.decode().splitlines()))
                compare.put(directory / 'input.json', compare.portable_input(self.case, fixture))
                if target == 'napi':
                    compare.put(directory / 'process.json', dict(exitCode=0, signal=None, error=None))
                    compare.put(directory / 'child.json', dict(architecture=architecture, acquisitionError=None))
                    for name in ['process.stdout.bin', 'process.stderr.bin']:
                        (directory / name).write_bytes(b'')
                else:
                    compare.put(directory / 'process.json', dict(isolation='new-browser-context-and-wasm-instance',
                        browserErrors=[], acquisitionError=None, architecture=architecture))
        report = dict(requested_targets=['napi', 'chromium'])
        baseline = compare.derive(output, report, manifest, oracle)
        for target in ['napi', 'chromium']:
            self.assertEqual(baseline[target]['statuses'], {'pass': 2})
            path = output / 'portable' / target / 'input/0/input.json'
            original = compare.read(path)
            for field, value in [('source', 'forged'), ('source_sha256', 'f' * 64), ('uri', 'file:///classes/other.pkl'),
                                 ('sourcePath', 'inputs/other.pkl'), ('moduleName', 'other')]:
                with self.subTest(target=target, field=field):
                    changed = copy.deepcopy(original)
                    changed['support_modules'][0][field] = value
                    compare.put(path, changed)
                    row = compare.derive(output, report, manifest, oracle)[target]['rows'][0]
                    self.assertEqual(row['status'], 'acquisition-failed')
                    self.assertIn('portable exact primary/support source/query input differs', row['issues'])
            compare.put(path, original)


class SourceCatalogClosureTests(unittest.TestCase):
    def test_metadata_kind_is_explicit_and_unknown_profiles_cannot_relax_source_requirements(self):
        self.assertEqual(compare.portable_tracked_sources({}), compare.PORTABLE_TRACKED_SOURCES)
        self.assertEqual(compare.portable_tracked_sources({'kind': compare.METADATA_KIND}),
                         compare.PORTABLE_TRACKED_SOURCES | compare.METADATA_SOURCES)
        for kind in ['unknown', '', None, False, 0, [], {}]:
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, 'unknown Class fixture'):
                compare.mandatory_sources({'kind': kind})

    def test_older_catalog_missing_new_literals_remains_closed_without_executing_captured_code(self):
        names = compare.source_names()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in set(names) - compare.CATALOG_ADDITIONS:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(b'')
            profile = compare.m2_source_inputs.PROFILE
            (root / profile).write_bytes((compare.ROOT / profile).read_bytes())
            historical = compare.m2_source_inputs.HISTORICAL
            (root / historical).write_bytes((compare.ROOT / historical).read_bytes())
            collector = root / 'tools/m2_source_inputs.py'
            collector.write_text('raise RuntimeError("captured code must never execute")\n')
            self.assertEqual(compare.source_names(root), sorted(set(names) - compare.CATALOG_ADDITIONS))
            # A missing helper declared by a newer collector is mandatory in
            # that closure, even after its physical source file is deleted.
            name = next(iter(compare.METADATA_SOURCES))
            collector.write_text('FILES = [' + repr(name) + ']\n')
            self.assertIn(name, compare.source_names(root))

    def test_real_catalog_is_recomputable_from_its_frozen_file_closure(self):
        names = compare.source_names()
        self.assertTrue(compare.MANDATORY_SOURCES.issubset(names))
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in names:
                target = root / name
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(b'')
            profile = compare.m2_source_inputs.PROFILE
            (root / profile).write_bytes((compare.ROOT / profile).read_bytes())
            historical = compare.m2_source_inputs.HISTORICAL
            (root / historical).write_bytes((compare.ROOT / historical).read_bytes())
            self.assertEqual(compare.source_names(root), names)


if __name__ == '__main__':
    unittest.main()
