"""Failure detection for full M2 accounting and selected raw differential evidence."""
import copy
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import sys
import tempfile
import threading
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_language
import m2_manifest


class ManifestTests(unittest.TestCase):
    def test_checked_in_manifest_is_fresh_and_full_gate_stays_open(self):
        manifest = m2_manifest.current_manifest()
        self.assertTrue(m2_manifest.full_gate_failures(manifest, []))
        self.assertEqual(sum(row['kind'] == 'semantic-family' for row in manifest['requirements']), 31)
        self.assertEqual(sum(row.get('upstream_case', {}).get('suite') == 'language-snippet'
                             for row in manifest['cases']), 957)
        self.assertTrue(any(row['id'] == 'semantic:reference' for row in manifest['requirements']))
        self.assertTrue(any(row['id'] == 'semantic:tracing' for row in manifest['requirements']))

    def test_mapped_passing_subset_cannot_hide_unimplemented_or_unexecuted_rows(self):
        manifest = dict(requirements=[dict(id='feature', required=True,
                implementation=dict(state='implemented', links=['vm.rs']),
                tests=dict(state='mapped', case_ids=['one']))],
                cases=[dict(id='one', required=True, requirements=['feature'], execution=dict(state='ready'))])
        results = [dict(id='one', status='pass')]
        self.assertEqual(m2_manifest.full_gate_failures(manifest, results), [])
        for field, value in [('implementation', dict(state='partial', links=['vm.rs'])),
                             ('tests', dict(state='unmapped', case_ids=[]))]:
            broken = copy.deepcopy(manifest)
            broken['requirements'][0][field] = value
            self.assertTrue(m2_manifest.full_gate_failures(broken, results))
        for results in [[], [dict(id='one', status='mismatch')]]:
            self.assertTrue(m2_manifest.full_gate_failures(manifest, results))
        manifest['cases'].append(dict(id='pending', required=True, requirements=[], execution=dict(state='unimplemented')))
        self.assertTrue(m2_manifest.full_gate_failures(manifest, [dict(id='one', status='pass')]))

    def test_unknown_and_duplicate_case_mapping_fail(self):
        manifest = m2_manifest.current_manifest()
        for mutate in ['duplicate', 'unknown']:
            broken = copy.deepcopy(manifest)
            if mutate == 'duplicate':
                broken['cases'].append(broken['cases'][0])
            else:
                broken['cases'][0]['requirements'] = ['unknown']
            with self.assertRaises(ValueError):
                m2_manifest.validate_manifest(broken)

    def test_power_assertion_mode_is_an_explicit_boolean(self):
        manifest = m2_manifest.current_manifest()
        for value in ['false', 0, None]:
            broken = copy.deepcopy(manifest)
            broken['cases'][0]['power_assertions'] = value
            with self.assertRaises(ValueError):
                m2_manifest.validate_manifest(broken)


class AdapterSelectionTests(unittest.TestCase):
    def test_module_selection_is_explicit_and_omitted_scalar_obligations_stay_in_full_gate(self):
        cases = [dict(id='scalar', adapter='language-api-string', required=True, requirements=['feature'], execution=dict(state='ready')),
                 dict(id='original', adapter='language-module-eval', required=True, requirements=['feature'], execution=dict(state='ready'))]
        manifest = dict(requirements=[], cases=cases)
        self.assertEqual(m2_language.selected_cases(manifest), cases)
        self.assertEqual(m2_language.selected_cases(manifest, ['language-module-eval']), cases[1:])
        self.assertIn('required case incomplete: scalar', m2_manifest.full_gate_failures(manifest, [dict(id='original', status='pass')]))
        for setting in [[], ['unknown'], ['language-module-eval', 'language-module-eval']]:
            with self.assertRaises(ValueError): m2_language.selected_cases(manifest, setting)



class DevelopmentGateTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        folder = self.directory / 'cases/error-cycle'
        folder.mkdir(parents=True)
        self.folder = folder
        (folder / 'input.pkl').write_bytes(b'a = b\nb = a\nresult = a\n')
        for runtime in ('java', 'rust'):
            (folder / f'{runtime}.stdout.bin').write_bytes(b'')
        (folder / 'java.stderr.bin').write_bytes(b'Exception in thread "main" java.lang.NoClassDefFoundError: Could not initialize class org.pkl.core.runtime.VmStackOverflowException\nremaining full stack\n')
        (folder / 'rust.stderr.bin').write_bytes('–– Pkl Error ––\nA stack overflow occurred.\nfull frames\n'.encode())
        self.cycle = dict(id='error-cycle', status='oracle-error', java=dict(exit_code=1, timed_out=False),
                          rust=dict(exit_code=1, timed_out=False))
        self.report = dict(results=[dict(id='other', status='pass'), self.cycle],
                           metamorphic_results=[dict(status='pass')])

    def test_explicit_exception_is_development_only_and_keeps_failure_status(self):
        self.assertTrue(m2_language.development_passes(self.report, self.directory))
        self.assertFalse(m2_language.subset_passes(self.report))
        self.assertEqual(self.cycle['status'], 'oracle-error')
        (self.folder / 'java.stderr.bin').write_bytes('–– Pkl Error ––\nA stack overflow occurred.\nfull frames\n'.encode())
        self.cycle['status'] = 'mismatch'
        self.assertTrue(m2_language.development_passes(self.report, self.directory))
        self.assertFalse(m2_language.subset_passes(self.report))

    def test_new_mismatch_or_property_failure_is_rejected(self):
        self.report['results'][0]['status'] = 'mismatch'
        self.assertFalse(m2_language.development_passes(self.report, self.directory))
        self.report['results'][0]['status'] = 'pass'
        self.report['metamorphic_results'][0]['status'] = 'mismatch'
        self.assertFalse(m2_language.development_passes(self.report, self.directory))

    def test_cycle_timeout_exit_crash_and_changed_input_are_rejected(self):
        original = copy.deepcopy(self.cycle)
        for field, value in [('exit_code', 101), ('timed_out', True)]:
            self.cycle['rust'][field] = value
            self.assertFalse(m2_language.development_passes(self.report, self.directory))
            self.cycle['rust'] = copy.deepcopy(original['rust'])
        for channel, value in [('java.stderr.bin', b'an unrelated Java exception\n'),
                               ('rust.stderr.bin', b'an unrelated Rust failure\n'),
                               ('input.pkl', b'result = missing\n')]:
            path = self.folder / channel
            saved = path.read_bytes()
            path.write_bytes(value)
            self.assertFalse(m2_language.development_passes(self.report, self.directory))
            path.write_bytes(saved)


class RemainderDevelopmentTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        self.folder = self.directory / 'cases/error-remainder-zero'
        (self.folder / 'java-repeat').mkdir(parents=True)
        (self.folder / 'input.pkl').write_bytes(b'result = 1 % 0\n')
        self.source = m2_language.ROOT / 'tests/fixtures/m2/error-remainder-zero.pkl'
        java = (b'An unexpected error has occurred. Would you mind filing a bug report?\n'
                b'Complete full Java message retained here.\n'
                b'java.lang.ArithmeticException%3A%20%2F%20by%20zero\n'
                b'java.lang.ArithmeticException: / by zero\n'
                b'\tat org.pkl.core.runtime.VmSafeMath.remainder(VmSafeMath.java:93)\n')
        for prefix in ('', 'java-repeat/'):
            (self.folder / (prefix + 'java.stdout.bin')).write_bytes(b'')
            (self.folder / (prefix + 'java.stderr.bin')).write_bytes(java)
            (self.folder / (prefix + 'java.outcome.bin')).write_bytes(b'pkl-exception\torg.pkl.core.PklBugException\n')
        (self.folder / 'rust.stdout.bin').write_bytes(b'')
        rust = ('–– Pkl Error ––\nDivision by zero.\n\n1 | result = 1 % 0\n'
                '             ^^^^^\nat error-remainder-zero#result (' + self.source.as_uri() + ')\n'
                '\n1 | result\n    ^^^^^^\nat  (repl:text)\n').encode()
        (self.folder / 'rust.stderr.bin').write_bytes(rust)
        process = dict(exit_code=1, timed_out=False, command=['probe', str(self.source), 'result'])
        self.row = dict(id='error-remainder-zero', status='mismatch', java=copy.deepcopy(process),
                        rust=copy.deepcopy(process), java_repeat=copy.deepcopy(process))
        self.report = dict(results=[self.row], metamorphic_results=[dict(status='pass')])

    def test_known_bug_requires_duplicate_raw_reproducibility_and_stays_mismatch(self):
        self.assertTrue(m2_language.development_passes(self.report, self.directory))
        self.assertEqual(self.row['status'], 'mismatch')
        self.assertFalse(m2_language.subset_passes(self.report))

    def test_primary_duplicate_candidate_sidecar_and_source_corruption_fail(self):
        for name in ('java.stderr.bin', 'java-repeat/java.stderr.bin', 'rust.stderr.bin',
                     'java.outcome.bin', 'java-repeat/java.outcome.bin', 'input.pkl'):
            path = self.folder / name
            saved = path.read_bytes()
            path.write_bytes(saved + b'corruption')
            self.assertFalse(m2_language.development_passes(self.report, self.directory), name)
            path.write_bytes(saved)

    def test_different_bug_or_origin_is_not_allowed_even_if_reproducible(self):
        for replacement in (b'java.lang.IllegalArgumentException: wrong\n',
                            b'java.lang.ArithmeticException: another operation\n'):
            for prefix in ('', 'java-repeat/'):
                path = self.folder / (prefix + 'java.stderr.bin')
                path.write_bytes(b'An unexpected error has occurred. Would you mind filing a bug report?\n' + replacement)
            self.assertFalse(m2_language.development_passes(self.report, self.directory))

    def test_missing_repeat_timeout_exit_and_new_failure_are_not_allowed(self):
        repeated = self.row.pop('java_repeat')
        self.assertFalse(m2_language.development_passes(self.report, self.directory))
        self.row['java_repeat'] = repeated
        repeated['timed_out'] = True
        self.assertFalse(m2_language.development_passes(self.report, self.directory))
        repeated['timed_out'] = False
        self.row['rust']['exit_code'] = 101
        self.assertFalse(m2_language.development_passes(self.report, self.directory))
        self.row['rust']['exit_code'] = 1
        self.report['results'].append(dict(id='new-failure', status='mismatch'))
        self.assertFalse(m2_language.development_passes(self.report, self.directory))


class OracleProvenanceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        (self.directory / 'sources').mkdir()
        for path in ('oracle.lock.json', 'references.lock'):
            (self.directory / 'sources' / path).write_bytes((m2_language.ROOT / path).read_bytes())
        lock = json.loads((self.directory / 'sources/oracle.lock.json').read_bytes())
        environment = dict(PATH='/bin:/usr/bin', HOME='/controlled/home', TMPDIR='/controlled/tmp',
                TMP='/controlled/tmp', TEMP='/controlled/tmp', LC_ALL='C', LANG='C', TZ='UTC',
                TERM='dumb', NO_COLOR='1', COLUMNS='80')
        self.metadata = dict(profile='reference', version=lock['version'], source=lock['source'],
                lock_sha256=m2_language.oracle.sha256(self.directory / 'sources/oracle.lock.json'),
                artifact='/pinned/pkl-tools-0.32.0.jar', artifact_sha256=lock['artifact']['sha256'],
                artifact_url=lock['artifact']['url'], java='/pinned/java', environment=environment,
                jdk=dict(properties=lock['jdk']['properties'], executable_sha256='1' * 64,
                         runtime_modules_sha256='2' * 64, release_sha256='3' * 64))
        self.metadata['command'] = ['/pinned/java', *lock['jvm_options'],
                '-Duser.home=/controlled/home', '-Djava.io.tmpdir=/controlled/tmp',
                '-cp', self.metadata['artifact'], 'org.pkl.cli.Main']

    def check(self, metadata):
        (self.directory / 'oracle.json').write_bytes(m2_manifest.encoded(metadata))
        return m2_language.verify_oracle(self.directory, dict(oracle=metadata))

    def test_immutable_oracle_identity_and_profile_are_required(self):
        self.check(self.metadata)
        mutations = [('profile', 'performance'), ('artifact_sha256', '0' * 64),
                     ('command', self.metadata['command'] + ['-Xmx2g']),
                     ('environment', dict(self.metadata['environment'], JAVA_TOOL_OPTIONS='-Xmx2g')),
                     ('jdk', dict(self.metadata['jdk'], release_sha256='')),
                     ('jdk', dict(self.metadata['jdk'], properties=dict(self.metadata['jdk']['properties'],
                                                                      **{'java.version': '22'})))]
        for key, value in mutations:
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.check(dict(self.metadata, **{key: value}))


class MetamorphicTests(unittest.TestCase):
    def test_each_runtime_must_execute_and_preserve_complete_equivalent_payloads(self):
        with tempfile.TemporaryDirectory() as temporary:
            directory = Path(temporary)
            relation = dict(kind='equal-stdout', case_ids=['pipe', 'apply'],
                            domain='Bounded one-argument Int function application')
            manifest = dict(cases=[dict(id=name, metamorphic_relation=relation) for name in relation['case_ids']])
            for name in relation['case_ids']:
                folder = directory / 'cases' / name
                folder.mkdir(parents=True)
                for runtime in ('java', 'rust'):
                    (folder / f'{runtime}.process.json').write_bytes(m2_manifest.encoded(dict(exit_code=0, timed_out=False)))
                    (folder / f'{runtime}.stdout.bin').write_bytes(b'42')
                    (folder / f'{runtime}.stderr.bin').write_bytes(b'')
            self.assertEqual(m2_language.check_relations(manifest, directory)[0]['status'], 'pass')
            java = directory / 'cases/apply/java.stdout.bin'
            java.write_bytes(b'42\n')
            result = m2_language.check_relations(manifest, directory)[0]
            self.assertEqual(result['status'], 'mismatch')
            self.assertEqual(result['runtimes'], dict(java=False, rust=True))
            java.write_bytes(b'42')
            process = directory / 'cases/pipe/rust.process.json'
            process.write_bytes(m2_manifest.encoded(dict(exit_code=1, timed_out=False)))
            self.assertEqual(m2_language.check_relations(manifest, directory)[0]['status'], 'mismatch')
            process.unlink()
            with self.assertRaises(OSError):
                m2_language.check_relations(manifest, directory)


class ConcurrentAcquisitionTests(unittest.TestCase):
    def test_workers_overlap_with_bounded_concurrency_but_report_in_manifest_order(self):
        selected = [dict(id=name) for name in ['first', 'second', 'third']]
        rendezvous = threading.Barrier(2)
        second_finished = threading.Event()
        lock = threading.Lock()
        active = 0
        peak = 0
        completed = []

        def acquire(case, *unused):
            nonlocal active, peak
            with lock:
                active += 1
                peak = max(peak, active)
            if case['id'] in ['first', 'second']:
                rendezvous.wait(timeout=5)
            if case['id'] == 'first':
                self.assertTrue(second_finished.wait(timeout=5))
            with lock:
                completed.append(case['id'])
                active -= 1
            if case['id'] == 'second':
                second_finished.set()
            return dict(id=case['id'], status='pass')

        stdout = io.StringIO()
        with patch.object(m2_language, 'acquire_case', side_effect=acquire), redirect_stdout(stdout):
            results = m2_language.acquire_cases(selected, Path('unused'), {}, Path('unused'), workers=2)
        self.assertEqual(peak, 2)
        self.assertLess(completed.index('second'), completed.index('first'))
        self.assertEqual([row['id'] for row in results], ['first', 'second', 'third'])
        self.assertEqual(stdout.getvalue(), 'first: pass\nsecond: pass\nthird: pass\n')

    def test_default_is_serial_and_duplicate_directories_are_rejected_before_acquisition(self):
        selected = [dict(id=name) for name in ['first', 'second']]
        with patch.object(m2_language, 'acquire_case', side_effect=lambda case, *args: dict(id=case['id'], status='pass')) as acquire:
            with redirect_stdout(io.StringIO()):
                m2_language.acquire_cases(selected, Path('unused'), {}, Path('unused'))
            self.assertEqual([call.args[0]['id'] for call in acquire.call_args_list], ['first', 'second'])
            acquire.reset_mock()
            with self.assertRaises(ValueError):
                m2_language.acquire_cases(selected * 2, Path('unused'), {}, Path('unused'), workers=2)
            acquire.assert_not_called()

    def test_invalid_counts_fail_before_creating_evidence_and_cli_rejects_them(self):
        for workers in [0, -1, m2_language.MAX_WORKERS + 1, True, 1.5, '2', None]:
            with self.subTest(workers=workers), patch.object(m2_manifest, 'current_manifest') as manifest:
                with self.assertRaises(ValueError):
                    m2_language.execute(Path('unused'), None, workers)
                manifest.assert_not_called()
        for argument in ['0', '-1', str(m2_language.MAX_WORKERS + 1), '1.5', 'many']:
            with self.subTest(argument=argument), patch.object(sys, 'argv', ['m2_language', '--output', 'unused', '--workers', argument]):
                with redirect_stdout(io.StringIO()), patch('sys.stderr', new_callable=io.StringIO), self.assertRaises(SystemExit) as raised:
                    m2_language.main()
                self.assertEqual(raised.exception.code, 2)

    def test_cli_forwards_default_and_explicit_workers_only_to_acquisition(self):
        report = dict(status_counts={'pass': 1}, full_gate_failures=[])
        for arguments, workers in [([], 1), (['--workers', '2'], 2)]:
            with patch.object(sys, 'argv', ['m2_language', '--output', 'unused', '--record-only', *arguments]), patch.object(m2_language, 'execute', return_value=report) as execute, redirect_stdout(io.StringIO()):
                self.assertEqual(m2_language.main(), 0)
                execute.assert_called_once_with(Path('unused').resolve(), None, workers)
        with patch.object(sys, 'argv', ['m2_language', '--verify-report', 'unused', '--workers', '2']), patch('sys.stderr', new_callable=io.StringIO), self.assertRaises(SystemExit) as raised:
            m2_language.main()
        self.assertEqual(raised.exception.code, 2)

    def test_mismatch_timeout_oracle_failure_and_remainder_repeat_keep_raw_channels(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / 'input.pkl').write_bytes(b'result = 42\n')
            identifiers = ['mismatch', 'timeout', 'oracle-failure', 'error-remainder-zero']
            selected = [dict(id=identifier, input='input.pkl', expression='result',
                             expected_outcome='success') for identifier in identifiers]
            selected[-1]['expected_outcome'] = 'failure'
            environment = {'PINNED': 'unchanged'}
            repeated = []

            def java(metadata, frozen, identifier, source, expression, directory, power=False):
                self.assertEqual(metadata['environment'], environment)
                self.assertEqual(source, root / 'input.pkl')
                repeated.append(identifier)
                remainder = identifier.startswith('error-remainder-zero')
                (directory / 'java.stdout.bin').write_bytes(b'' if remainder else b'42\n ')
                (directory / 'java.stderr.bin').write_bytes(b'full error\n ' if remainder else b'')
                (directory / 'java.outcome.bin').write_bytes(
                    b'pkl-exception\torg.pkl.core.PklBugException\n' if remainder else
                    b'' if identifier == 'oracle-failure' else b'success\n')
                return dict(exit_code=1 if remainder else 0, timed_out=False)

            def rust(command, directory, name, observed_environment):
                self.assertEqual(observed_environment, environment)
                remainder = directory.name == 'error-remainder-zero'
                (directory / 'rust.stdout.bin').write_bytes(b'' if remainder else b'42\n')
                (directory / 'rust.stderr.bin').write_bytes(b'other full error\n' if remainder else b'')
                return dict(exit_code=1 if remainder else 0, timed_out=directory.name == 'timeout')

            with patch.object(m2_language, 'ROOT', root), patch.object(m2_language, 'capture_java', side_effect=java), patch.object(m2_language, 'capture', side_effect=rust), redirect_stdout(io.StringIO()):
                results = m2_language.acquire_cases(selected, root / 'output', {'environment': environment}, Path('binary'), workers=2)
            self.assertEqual([r['status'] for r in results], ['mismatch', 'candidate-timeout', 'oracle-error', 'mismatch'])
            self.assertCountEqual(repeated, identifiers + ['error-remainder-zero/java-repeat'])
            self.assertIn('java_repeat', results[-1])
            for identifier in identifiers:
                self.assertEqual((root / 'output/cases' / identifier / 'input.pkl').read_bytes(), b'result = 42\n')
            self.assertEqual((root / 'output/cases/mismatch/java.stdout.bin').read_bytes(), b'42\n ')
            self.assertEqual((root / 'output/cases/error-remainder-zero/java-repeat/java.stderr.bin').read_bytes(), b'full error\n ')

    def test_acquisition_exception_is_propagated(self):
        with patch.object(m2_language, 'acquire_case', side_effect=OSError('capture failed')), redirect_stdout(io.StringIO()):
            with self.assertRaisesRegex(OSError, 'capture failed'):
                m2_language.acquire_cases([dict(id='one')], Path('unused'), {}, Path('unused'), workers=2)


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.case = self.directory / 'cases/one'
        self.case.mkdir(parents=True)
        input_data = b'result = 42\n'
        (self.case / 'input.pkl').write_bytes(input_data)
        selected = dict(id='one', input='tests/fixtures/m2/one.pkl', expression='result', adapter='language-api-string', input_sha256=m2_manifest.digest(input_data),
                        expected_outcome='success', required=True, requirements=['feature'], execution=dict(state='ready'))
        self.manifest = dict(requirements=[dict(id='feature', required=True,
                implementation=dict(state='partial', links=[]), tests=dict(state='partial', case_ids=['one']))],
                cases=[selected])
        (self.directory / 'sources/compatibility').mkdir(parents=True)
        (self.directory / 'sources' / m2_manifest.MANIFEST).write_bytes(m2_manifest.encoded(self.manifest))
        (self.directory / 'artifacts').mkdir()
        (self.directory / 'artifacts/language_probe').write_bytes(b'binary')
        processes = {}
        for runtime in ('java', 'rust'):
            command = [str(self.directory / 'artifacts/language_probe'), str(m2_language.ROOT / selected['input']), 'result'] if runtime == 'rust' else m2_language.java_case_command(dict(command=['java', '-cp', 'jar', 'org.pkl.cli.Main']), self.directory / 'artifacts/language_probe', 'one', m2_language.ROOT / selected['input'], 'result')
            row = dict(command=command, exit_code=0, timed_out=False)
            for channel, data in [('stdout', b'42'), ('stderr', b'')]:
                (self.case / f'{runtime}.{channel}.bin').write_bytes(data)
                row[channel + '_sha256'] = m2_manifest.digest(data)
            if runtime == 'java':
                (self.case / 'java.outcome.bin').write_bytes(b'success\n')
                row['outcome_sha256'] = m2_manifest.digest(b'success\n')
            (self.case / f'{runtime}.process.json').write_bytes(m2_manifest.encoded(row))
            processes[runtime] = row
        results = [dict(id='one', status='pass', **processes)]
        self.report = dict(oracle=dict(command=['java', '-cp', 'jar', 'org.pkl.cli.Main']), source_stability=True, artifact_stability=True, selected_count=1,
                           source_sha256={str(m2_manifest.MANIFEST): m2_manifest.digest(m2_manifest.encoded(self.manifest))},
                           candidate=dict(sha256=m2_manifest.digest(b'binary'), source_binding='unbound-caller-supplied', build=None), results=results,
                           metamorphic_results=[], status_counts={'pass': 1},
                           full_gate_failures=m2_manifest.full_gate_failures(self.manifest, results))
        self.refresh()
        self.addCleanup(patch.stopall)
        patch.object(m2_manifest, 'current_manifest', return_value=self.manifest).start()
        patch.object(m2_language, 'source_paths', return_value=[str(m2_manifest.MANIFEST)]).start()
        patch.object(m2_language, 'verify_oracle').start()

    def save(self):
        (self.directory / 'report.json').write_bytes(m2_manifest.encoded(self.report))

    def refresh(self):
        self.report['evidence_sha256'] = {str(p.relative_to(self.directory)): m2_language.oracle.sha256(p)
                for p in self.directory.rglob('*') if p.is_file() and p != self.directory / 'report.json'}
        self.save()

    def test_integrity_success_does_not_close_full_gate(self):
        result = m2_language.check_report(self.directory)
        self.assertTrue(m2_language.subset_passes(result))
        self.assertTrue(result['full_gate_failures'])

    def test_execution_setting_is_optional_for_older_reports_and_validated_when_present(self):
        m2_language.check_report(self.directory)
        self.report['execution'] = dict(workers=2)
        self.save()
        m2_language.check_report(self.directory)
        for setting in [dict(workers=0), dict(workers=True), dict(workers=33),
                        dict(workers=2, omitted_failures=True), None]:
            self.report['execution'] = setting
            self.save()
            with self.assertRaises(ValueError):
                m2_language.check_report(self.directory)

    def test_new_report_must_retain_manifest_result_order(self):
        self.manifest['cases'].append(dict(self.manifest['cases'][0], id='two'))
        data = m2_manifest.encoded(self.manifest)
        (self.directory / 'sources' / m2_manifest.MANIFEST).write_bytes(data)
        self.report['source_sha256'][str(m2_manifest.MANIFEST)] = m2_manifest.digest(data)
        self.report['execution'] = dict(workers=2)
        self.report['selected_count'] = 2
        self.report['results'] = [dict(self.report['results'][0], id='two'), self.report['results'][0]]
        self.refresh()
        with self.assertRaisesRegex(ValueError, 'execution order differs'):
            m2_language.check_report(self.directory)

    def test_missing_duplicate_and_unknown_results_fail(self):
        for rows in [[], self.report['results'] * 2, [dict(id='unknown')]]:
            original = copy.deepcopy(self.report)
            self.report['results'] = rows
            self.save()
            with self.assertRaises(ValueError):
                m2_language.check_report(self.directory)
            self.report = original

    def test_raw_difference_fails_even_if_capture_hashes_are_updated(self):
        (self.case / 'rust.stdout.bin').write_bytes(b'42\n')
        row = self.report['results'][0]['rust']
        row['stdout_sha256'] = m2_manifest.digest(b'42\n')
        (self.case / 'rust.process.json').write_bytes(m2_manifest.encoded(row))
        self.refresh()
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)
        self.report['results'][0]['status'] = 'mismatch'
        self.report['status_counts'] = {'mismatch': 1}
        self.report['full_gate_failures'] = m2_manifest.full_gate_failures(self.manifest, self.report['results'])
        self.save()
        result = m2_language.check_report(self.directory)
        self.assertFalse(m2_language.subset_passes(result))

    def test_different_input_uri_fails(self):
        row = self.report['results'][0]['rust']
        row['command'][-2] = '/another/module.pkl'
        (self.case / 'rust.process.json').write_bytes(m2_manifest.encoded(row))
        self.refresh()
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)

    def test_explicit_power_assertion_mode_binds_both_recorded_commands(self):
        self.manifest['cases'][0]['power_assertions'] = True
        (self.directory / 'sources' / m2_manifest.MANIFEST).write_bytes(m2_manifest.encoded(self.manifest))
        self.report['source_sha256'][str(m2_manifest.MANIFEST)] = m2_manifest.digest(m2_manifest.encoded(self.manifest))
        for runtime in ('java', 'rust'):
            row = self.report['results'][0][runtime]
            row['command'].append('--power-assertions')
            (self.case / f'{runtime}.process.json').write_bytes(m2_manifest.encoded(row))
        self.refresh()
        m2_language.check_report(self.directory)
        for runtime in ('java', 'rust'):
            row = self.report['results'][0][runtime]
            row['command'].pop()
            (self.case / f'{runtime}.process.json').write_bytes(m2_manifest.encoded(row))
            self.refresh()
            with self.assertRaises(ValueError):
                m2_language.check_report(self.directory)
            row['command'].append('--power-assertions')
            (self.case / f'{runtime}.process.json').write_bytes(m2_manifest.encoded(row))

    def test_default_candidate_cannot_inject_power_assertions_before_source(self):
        row = self.report['results'][0]['rust']
        row['command'].insert(1, '--power-assertions')
        (self.case / 'rust.process.json').write_bytes(m2_manifest.encoded(row))
        self.refresh()
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)

    def test_missing_source_identity_fails(self):
        self.report['source_sha256'] = {}
        self.save()
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)

    def test_truncated_or_injected_evidence_fails(self):
        (self.case / 'java.stderr.bin').unlink()
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)
        (self.case / 'java.stderr.bin').write_bytes(b'')
        (self.directory / 'extra.bin').write_bytes(b'extra')
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)

    def test_artifact_or_source_drift_fails(self):
        for field in ['source_stability', 'artifact_stability']:
            self.report[field] = False
            self.save()
            with self.assertRaises(ValueError):
                m2_language.check_report(self.directory)
            self.report[field] = True
        (self.directory / 'artifacts/language_probe').write_bytes(b'new binary')
        self.refresh()
        with self.assertRaises(ValueError):
            m2_language.check_report(self.directory)

    def test_empty_success_string_is_valid_but_empty_error_is_not(self):
        case = dict(expected_outcome='success')
        process = dict(exit_code=0, timed_out=False)
        self.assertEqual(m2_language.status(case, process, process, b'', b'', b'', b'', b'success\n'), 'pass')
        failed = dict(exit_code=1, timed_out=False)
        self.assertEqual(m2_language.status(dict(expected_outcome='failure'), failed, failed,
                                          b'', b'', b'', b'', b'pkl-exception\torg.pkl.core.PklException\n'), 'oracle-error')

    def test_caught_bug_and_uncaught_jvm_failure_are_distinguished_by_sidecar(self):
        case = dict(expected_outcome='failure')
        process = dict(exit_code=1, timed_out=False)
        payload = b'An unexpected error has occurred. Full caught PklBugException payload.\n'
        self.assertEqual(m2_language.status(case, process, process, b'', payload, b'', payload,
                b'pkl-exception\torg.pkl.core.PklBugException\n'), 'pass')
        self.assertEqual(m2_language.status(case, process, process, b'', payload, b'', b'other',
                b'pkl-exception\torg.pkl.core.PklBugException\n'), 'mismatch')
        self.assertEqual(m2_language.status(case, process, process, b'', payload, b'', payload, b''), 'oracle-error')

    def test_whole_error_payload_including_last_newline_is_compared(self):
        error = '–– Pkl Error ––\nproblem\n\n1 | x\n    ^\nat m (file:///x.pkl)\n'.encode()
        case = dict(expected_outcome='failure')
        process = dict(exit_code=1, timed_out=False)
        self.assertEqual(m2_language.status(case, process, process, b'', error, b'', error, b'pkl-exception\torg.pkl.core.PklException\n'), 'pass')
        self.assertEqual(m2_language.status(case, process, process, b'', error, b'', error[:-1], b'pkl-exception\torg.pkl.core.PklException\n'), 'mismatch')
        process = dict(exit_code=1, timed_out=True)
        self.assertEqual(m2_language.status(case, process, process, b'', error, b'', error, b'pkl-exception\torg.pkl.core.PklException\n'), 'oracle-error')


if __name__ == '__main__':
    unittest.main()
