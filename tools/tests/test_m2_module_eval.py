"""Four-channel original CLI execution/verifier failures, using synthetic child tools."""
import copy
import gc
import json
import os
from pathlib import Path
import sys
import shutil
from unittest.mock import patch
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_manifest
import m2_language
import m2_module_eval as module
import m2_jvm_classpath as jvm


class RegistryTests(unittest.TestCase):
    def test_original_registry_binds_complete_case_not_only_source(self):
        root = m2_manifest.ROOT
        corpus = m2_manifest.load(Path('compatibility/corpus.json'))
        registry = m2_manifest.load(m2_manifest.MODULE_ADAPTERS)
        valid = module.registry_cases(root, corpus, registry)
        expected_ids = [case['id'] for case in corpus['cases']
                        if case['suite'] == 'language-snippet' and (
                            case['execution']['status'] == 'ready'
                            or case['id'] in module.PRESERVED_BLOCKED_IDS)]
        self.assertEqual(list(valid), expected_ids)
        module_cases = {case['id'] for case in corpus['cases']
                        if case['suite'] == 'language-snippet' and case['source']['path'].startswith(
                            'pkl-core/src/test/files/LanguageSnippetTests/input/modules/')}
        self.assertEqual(len(module_cases), 41)
        self.assertTrue(module_cases <= valid.keys())
        selected = [case for case in corpus['cases'] if case['id'] in valid]
        self.assertEqual({case['expected_outcome'] for case in selected}, {'success', 'observe'})
        for change in ('argv', 'environment', 'duplicate', 'source', 'outcome', 'resource'):
            changed = copy.deepcopy(corpus)
            selected = next(c for c in changed['cases'] if c['id'] in valid)
            entries = copy.deepcopy(registry)
            if change == 'argv':
                selected['command'].insert(1, '--test-mode')
            elif change == 'environment':
                selected['environment']['LANG'] = 'C'
            elif change == 'duplicate':
                entries['cases'].append(entries['cases'][0])
            elif change == 'outcome':
                selected['expected_exit'] = [1] if selected['expected_exit'] == [0] else [0]
            elif change == 'resource':
                selected['resources'] = []
            else:
                entries['cases'][0]['input_sha256'] = '0' * 64
            with self.subTest(change=change), self.assertRaises(ValueError):
                module.registry_cases(root, changed, entries)

    def test_historical_blocked_cli_is_preserved_under_explicit_jvm_execution_profile(self):
        manifest = m2_manifest.build_manifest()
        identifier = 'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/basic/import1b.pkl'
        case = next(row for row in manifest['cases'] if row['id'] == identifier)
        self.assertEqual(case['upstream_case']['execution']['status'], 'blocked')
        self.assertEqual(case['execution'], dict(state='ready', reason=None))
        self.assertNotIn(case, m2_language.selected_cases(manifest, [module.ADAPTER]))
        self.assertIn(case, m2_language.selected_cases(manifest, [jvm.ADAPTER]))
        selected = m2_language.selected_cases(manifest)
        self.assertEqual(len(selected), manifest['counts']['cases_by_execution']['ready'])
        self.assertEqual(manifest['counts']['cases_by_execution'].get('unimplemented', 0), 0)
        self.assertEqual(manifest['counts']['cases_by_execution']['unmapped'], 11)
        # The explicit profile still requires actual successful execution.
        self.assertIn('required case incomplete: ' + identifier,
                      m2_manifest.full_gate_failures(manifest, []))
        self.assertNotIn('required case incomplete: ' + identifier,
                         m2_manifest.full_gate_failures(manifest, [dict(id=identifier, status='pass')]))
        forged = copy.deepcopy(manifest)
        next(row for row in forged['cases'] if row['id'] == identifier)['adapter'] = module.ADAPTER
        with self.assertRaisesRegex(ValueError, 'original JVM execution profile contract differs'):
            m2_manifest.validate_manifest(forged)


class RegistryGenerationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        source = self.root / 'external/pkl/input.pkl'
        source.parent.mkdir(parents=True)
        source.write_bytes(b'value = 42\n')
        resource = self.root / 'external/pkl/fixture.bin'
        resource.write_bytes(b'raw fixture\x00\xff')
        case = dict(id='original:ready', suite='language-snippet',
                    execution=dict(status='ready', reason=None),
                    source=dict(path='input.pkl', sha256=compat.digest(source.read_bytes())),
                    command=['eval', '--settings', 'pkl:settings', 'external/pkl/input.pkl'],
                    cwd='.', environment={'LANG': 'C.UTF-8'}, expected_exit=[0],
                    output=None, resources=[dict(path='external/pkl/fixture.bin',
                                                sha256=compat.digest(resource.read_bytes()))])
        blocked = copy.deepcopy(case)
        blocked.update(id='original:unresolved-fixture',
                       execution=dict(status='blocked', reason='Missing fixture adapter'))
        historical = copy.deepcopy(blocked)
        historical['id'] = next(iter(module.PRESERVED_BLOCKED_IDS))
        self.corpus = dict(baseline='0.32.0', cases=[blocked, case, historical])
        self.registry = module.build_registry(self.corpus)

    def test_exhaustive_admission_retains_corpus_order_and_blocked_prerequisites(self):
        self.assertEqual(list(module.registry_cases(self.root, self.corpus, self.registry)),
                         ['original:ready', next(iter(module.PRESERVED_BLOCKED_IDS))])
        blocked, ready, historical = self.corpus['cases']
        self.assertEqual(module.execution_state(ready, True), dict(state='ready', reason=None))
        for original in (blocked, historical):
            self.assertEqual(module.execution_state(original, True),
                             dict(state='unimplemented', reason='Missing fixture adapter'))
        self.assertEqual(self.registry, module.build_registry(self.corpus))

    def test_dropped_reordered_or_extra_blocked_entries_cannot_hide_coverage(self):
        for change in ('drop', 'reorder', 'blocked'):
            registry = copy.deepcopy(self.registry)
            if change == 'drop': registry['cases'].pop(0)
            elif change == 'reorder': registry['cases'].reverse()
            else:
                blocked = self.corpus['cases'][0]
                registry['cases'].append(dict(id=blocked['id'], input_sha256=blocked['source']['sha256'],
                                             upstream_case_sha256=compat.digest(compat.encoded(blocked))))
            with self.subTest(change=change), self.assertRaisesRegex(ValueError, 'coverage/order'):
                module.registry_cases(self.root, self.corpus, registry)

    def test_missing_or_changed_original_input_and_declared_fixture_are_rejected(self):
        for relative in ('external/pkl/input.pkl', 'external/pkl/fixture.bin'):
            path = self.root / relative
            original = path.read_bytes()
            for mutation in ('missing', 'changed'):
                if mutation == 'missing': path.unlink()
                else: path.write_bytes(original + b'changed')
                with self.subTest(relative=relative, mutation=mutation), self.assertRaises((OSError, ValueError)):
                    module.registry_cases(self.root, self.corpus, self.registry)
                path.write_bytes(original)

    def test_complete_original_contract_is_authenticated_including_output(self):
        for key, value in [('command', ['eval', '--test-mode', 'external/pkl/input.pkl']),
                           ('cwd', 'external/pkl'), ('environment', {'LANG': 'C'}),
                           ('expected_exit', [1]), ('resources', []),
                           ('output', dict(kind='multiple', path='generated'))]:
            corpus = copy.deepcopy(self.corpus)
            corpus['cases'][1][key] = value
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'contract changed'):
                module.registry_cases(self.root, corpus, self.registry)


class ModuleExecutionTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.workspace = self.root / 'target/workspace'
        self.workspace.mkdir(parents=True)
        for base in (self.root, self.workspace):
            (base / 'input.pkl').write_bytes(b'foo {\n')
            (base / 'original.err').write_bytes(b'provenance only; not expected output')
        self.case = dict(id='original:first', adapter=module.ADAPTER, input='input.pkl',
                         input_sha256=compat.digest(b'foo {\n'), upstream_case=dict(
                             id='original:first', command=['eval', '--settings', 'pkl:settings', 'input.pkl'],
                             cwd='.', environment={'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'NO_COLOR': '1'},
                             resources=[dict(path='original.err', sha256=compat.digest(b'provenance only; not expected output'))], expected_exit=[0, 1]))
        self.binary = self.root / 'fake-cli'
        self.binary.write_text(f'#!{sys.executable}\nimport sys\nsys.stderr.buffer.write("–– Pkl Error ––\\nexact raw error\\r\\n".encode())\nsys.exit(1)\n')
        self.binary.chmod(0o755)
        self.metadata = dict(command=[str(self.binary)], environment={'PATH': os.defpath, 'LANG': 'C', 'LC_ALL': 'C', 'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1'})
        self.output = self.root / 'evidence'
        self.output.mkdir()
        self.baseline = compat.snapshot(self.workspace)
        self.row = module.acquire_case(self.root, self.workspace, self.case, self.metadata, self.binary, self.output)

    def verify(self, row=None):
        module.verify_case(self.root, self.workspace, self.case, row or self.row, self.output, self.metadata, self.binary, self.baseline)

    def test_exact_process_bytes_and_original_profile_are_verified_without_execution(self):
        self.assertEqual(self.row['status'], 'pass')
        self.binary.unlink()
        self.verify()
        self.assertEqual(compat.snapshot(self.workspace), self.baseline)
        expected_uri = (self.workspace / 'input.pkl').as_uri()
        self.assertEqual(self.row['java']['source_uri'], expected_uri)
        self.assertEqual(self.row['java']['command'][-len(self.case['upstream_case']['command']):], self.case['upstream_case']['command'])

    def test_changed_source_raw_exit_files_and_process_profile_are_not_accepted(self):
        folder = self.output / 'cases' / self.row['directory']
        for relative in ['input.pkl', 'java/stderr.bin', 'rust/stdout.bin']:
            path = folder / relative
            before = path.read_bytes()
            path.write_bytes(before + b'X')
            with self.assertRaises(ValueError): self.verify()
            path.write_bytes(before)
        for mutate in ('exit', 'argv', 'env', 'workspace', 'watchdog', 'status'):
            row = copy.deepcopy(self.row)
            if mutate == 'status':
                row['status'] = 'mismatch'
                with self.assertRaises(ValueError): self.verify(row)
                continue
            path = folder / 'rust/process.json'
            before = path.read_bytes()
            if mutate == 'exit': row['rust']['exit_code'] = 0
            elif mutate == 'argv': row['rust']['command'].insert(1, '--test-mode')
            elif mutate == 'env': row['rust']['environment'].pop('LANG')
            elif mutate == 'watchdog': row['rust']['watchdog_seconds'] = 120
            else: row['rust']['workspace_after_sha256'] = '0' * 64
            path.write_bytes(compat.encoded(row['rust']))
            with self.assertRaises(ValueError): self.verify(row)
            path.write_bytes(before)
        private = folder / 'rust/private-state'
        private.mkdir();(private / 'unexpected').write_bytes(b'private bytes')
        with self.assertRaisesRegex(ValueError, 'private-state evidence differs'): self.verify()
        (private / 'unexpected').unlink()
        files = folder / 'rust/files'
        files.mkdir();(files / 'unexpected').write_bytes(b'not empty')
        with self.assertRaises(ValueError): self.verify()

    def test_full_module_report_rejects_dropped_execution_false_completion_and_fake_recorded_build(self):
        output = self.root / 'report-evidence'
        artifacts = output / 'artifacts'
        artifacts.mkdir(parents=True)
        frozen = artifacts / 'module_eval_probe'
        shutil.copy2(self.binary, frozen)
        case = dict(self.case, required=True, requirements=['feature'], execution=dict(state='ready'))
        row = module.acquire_case(self.root, self.workspace, case, self.metadata, frozen, output)
        manifest = dict(cases=[case], requirements=[dict(id='feature', required=True,
            implementation=dict(state='partial', links=[]), tests=dict(state='partial', case_ids=[case['id']]))])
        resources = [dict(path='input.pkl', kind='file', sha256=case['input_sha256']),
                     dict(path='original.err', kind='file', sha256=case['upstream_case']['resources'][0]['sha256'])]
        corpus = dict(sources=[dict(resources=resources, resource_set_sha256=compat.digest(compat.encoded(resources)))])
        sources = {'compatibility/m2-language.json': manifest, 'compatibility/corpus.json': corpus}
        for relative, value in sources.items():
            path = output / 'sources' / relative;path.parent.mkdir(parents=True, exist_ok=True);path.write_bytes(compat.encoded(value))
        (output / 'module-workspace.json').write_bytes(compat.encoded(self.baseline))
        declared = {resource['path']: resource['sha256'] for resource in resources}
        (output / 'module-inputs.json').write_bytes(compat.encoded(dict(files=len(declared), sha256=compat.digest(compat.encoded(declared)))))
        selection = dict(adapters=[module.ADAPTER], case_ids=[case['id']])
        (output / 'selection.json').write_bytes(compat.encoded(selection))
        report = dict(schema_version=3, selection=selection, oracle=self.metadata, source_stability=True, artifact_stability=True,
            source_sha256={name: compat.digest(compat.encoded(value)) for name, value in sources.items()},
            candidate=None, module_candidate=dict(sha256=m2_language.oracle.sha256(frozen), source_binding='unbound-caller-supplied', build=None),
            module_eval=dict(workspace=str(self.workspace), workers=1, watchdog_seconds=module.CAPTURE_TIMEOUT_SECONDS,
                baseline_sha256=m2_language.oracle.sha256(output / 'module-workspace.json'),
                input_validation_sha256=m2_language.oracle.sha256(output / 'module-inputs.json')),
            selected_count=1, execution=dict(workers=1), results=[row], metamorphic_results=[], status_counts={'pass': 1},
            full_gate_failures=m2_manifest.full_gate_failures(manifest, [row]))
        def save(value):
            value['evidence_sha256'] = {str(p.relative_to(output)): m2_language.oracle.sha256(p)
                for p in output.rglob('*') if p.is_file() and p != output / 'report.json'}
            (output / 'report.json').write_bytes(compat.encoded(value))
        with patch.object(m2_manifest, 'current_manifest', return_value=manifest), patch.object(m2_language, 'source_paths', return_value=list(sources)), patch.object(m2_language, 'verify_oracle'):
            save(report);m2_language.check_report(output)
            for kind in ['drop-execution', 'false-completion', 'downgrade-schema', 'fake-build', 'missing-process']:
                changed = copy.deepcopy(report)
                if kind == 'drop-execution':
                    changed.update(results=[], selected_count=0, status_counts={})
                elif kind == 'false-completion': changed['full_gate_failures'] = []
                elif kind == 'downgrade-schema': changed['schema_version'] = 2
                elif kind == 'fake-build':
                    build = dict(command=['true'], exit_code=0, timed_out=False)
                    (output / 'build.process.json').write_bytes(compat.encoded(build))
                    changed['module_candidate'].update(source_binding='recorded-build', build=build)
                else:
                    path = output / 'cases' / row['directory'] / 'rust/process.json'
                    process_bytes = path.read_bytes();path.unlink()
                save(changed)
                with self.assertRaises((ValueError, OSError)): m2_language.check_report(output)
                if kind == 'missing-process':path.write_bytes(process_bytes)
            save(report);m2_language.check_report(output)

    def test_private_runtime_files_are_provenance_and_never_public_expected_output(self):
        self.binary.write_text(f'#!{sys.executable}\nfrom pathlib import Path\nimport os,sys\nPath(os.environ["HOME"],"private.cache").write_bytes(b"private bytes")\nsys.stderr.buffer.write("–– Pkl Error ––\\nexact raw error\\r\\n".encode())\nsys.exit(1)\n')
        directory = self.root / 'private-writer'
        process, captured = module.capture_process(self.root, self.workspace, self.case, [str(self.binary)], self.metadata['environment'], directory)
        self.assertEqual((directory / 'private-state/home/private.cache').read_bytes(), b'private bytes')
        self.assertEqual(process['private_files_sha256'], {'home/private.cache': compat.digest(b'private bytes')})
        self.assertEqual(captured['files'], {})
        self.assertEqual(captured['workspace_changes'], {})
        expected = compat.load_capture(self.output / 'cases' / self.row['directory'] / 'java')
        self.assertEqual(module.status(self.row['java'], process, expected, captured), 'pass')

    def test_forged_baseline_with_refreshed_process_metadata_cannot_omit_or_change_pinned_resources(self):
        folder = self.output / 'cases' / self.row['directory']
        for resource in ['input.pkl', 'original.err']:
            for change in ['omit', 'kind', 'hash']:
                baseline = copy.deepcopy(self.baseline)
                if change == 'omit': baseline.pop(resource)
                elif change == 'kind': baseline[resource]['kind'] = 'directory'
                else: baseline[resource]['sha256'] = '0' * 64
                row = copy.deepcopy(self.row)
                for runtime in ['java', 'rust']:
                    row[runtime]['workspace_before_sha256'] = compat.digest(compat.encoded(baseline))
                    row[runtime]['workspace_after_sha256'] = compat.digest(compat.encoded(baseline))
                    (folder / runtime / 'process.json').write_bytes(compat.encoded(row[runtime]))
                with self.assertRaisesRegex(ValueError, 'original resource baseline differs'):
                    module.verify_case(self.root, self.workspace, self.case, row, self.output, self.metadata, self.binary, baseline)
        for runtime in ['java', 'rust']:
            (folder / runtime / 'process.json').write_bytes(compat.encoded(self.row[runtime]))
        self.verify()

    def test_unknown_workspace_write_is_captured_restored_and_never_passes(self):
        self.binary.write_text(f'#!{sys.executable}\nfrom pathlib import Path\nimport sys\nPath("unexpected.bin").write_bytes(b"\\x00raw\\xff")\nsys.stderr.buffer.write("–– Pkl Error ––\\nexact raw error\\r\\n".encode())\nsys.exit(1)\n')
        directory = self.root / 'writer'
        process, captured = module.capture_process(self.root, self.workspace, self.case, [str(self.binary)], self.metadata['environment'], directory)
        self.assertEqual(captured['files']['workspace/unexpected.bin'], b'\x00raw\xff')
        self.assertTrue(process['workspace_changes'])
        self.assertFalse((self.workspace / 'unexpected.bin').exists())
        self.assertEqual(compat.snapshot(self.workspace), self.baseline)
        expected = compat.load_capture(self.output / 'cases' / self.row['directory'] / 'java')
        self.assertEqual(module.status(self.row['java'], process, expected, captured), 'candidate-environment-error')
        self.assertEqual(module.status(process, process, captured, captured), 'oracle-error')

    def test_output_unsupported_and_internal_java_failure_cannot_be_success(self):
        expected = dict(stdout=b'', stderr='–– Pkl Error ––\nexact\n'.encode(), exit_code=1, files={}, timed_out=False)
        for code, status in [(2, 'candidate-unsupported'), (3, 'candidate-limit-exceeded'), (4, 'candidate-internal-error')]:
            actual = dict(expected, exit_code=code)
            self.assertEqual(module.status(dict(exit_code=1, timed_out=False), dict(exit_code=code, timed_out=False), expected, actual), status)
        bug = dict(expected, stderr=b'An unexpected error has occurred.\nfull raw bug\n')
        self.assertEqual(module.status(dict(exit_code=1, timed_out=False), dict(exit_code=1, timed_out=False), bug, bug), 'oracle-error')

    def test_success_output_is_lossless_bytes_and_failed_partial_output_never_matches(self):
        self.binary.write_text(f'#!{sys.executable}\nimport sys\nsys.stdout.buffer.write(bytes([0,255,65,13,10]))\n')
        row = module.acquire_case(self.root, self.workspace, self.case, self.metadata,
                                  self.binary, self.root / 'binary-success')
        folder = self.root / 'binary-success/cases' / row['directory']
        self.assertEqual(row['status'], 'pass')
        self.assertEqual((folder / 'rust/stdout.bin').read_bytes(), b'\x00\xffA\r\n')
        self.assertEqual((folder / 'rust/stderr.bin').read_bytes(), b'')
        error = dict(stdout=b'', stderr='–– Pkl Error ––\nactual failure\n'.encode(),
                     exit_code=1, files={}, timed_out=False)
        partial = dict(error, stdout=b'unpublished renderer prefix')
        self.assertEqual(module.status(dict(exit_code=1, timed_out=False),
                                       dict(exit_code=1, timed_out=False), error, partial), 'mismatch')

    def test_raw_equal_ordinary_errors_do_not_satisfy_original_success_contract(self):
        case = copy.deepcopy(self.case)
        case['id'] = 'original:expected-success'
        case['upstream_case']['id'] = case['id']
        case['upstream_case']['expected_exit'] = [0]
        row = module.acquire_case(self.root, self.workspace, case, self.metadata,
                                  self.binary, self.root / 'expected-success')
        self.assertTrue(row['raw_channels_match'])
        self.assertFalse(row['upstream_expected_exit_match'])
        self.assertEqual(row['status'], 'oracle-contract-mismatch')
        directory = self.root / 'expected-success'
        module.verify_case(self.root, self.workspace, case, row, directory,
                           self.metadata, self.binary, self.baseline)
        forged = dict(row, status='pass', upstream_expected_exit_match=True)
        with self.assertRaisesRegex(ValueError, 'raw comparison status changed'):
            module.verify_case(self.root, self.workspace, case, forged, directory,
                               self.metadata, self.binary, self.baseline)

    def test_verify_case_with_distinct_reallocated_baselines_never_aliases_digest(self):
        # Verify distinct baselines allocated after freeing prior baselines never alias
        # even if CPython reuses the same object id().
        for index in range(6):
            extra = self.workspace / f'extra_{index}.txt'
            extra.write_bytes(f'payload-{index}'.encode())
            baseline_b = compat.snapshot(self.workspace)
            self.assertNotEqual(
                compat.digest(compat.encoded(self.baseline)),
                compat.digest(compat.encoded(baseline_b)),
            )
            out_b = self.root / f'reallocated-{index}'
            out_b.mkdir()
            case_b = copy.deepcopy(self.case)
            case_b['id'] = f'original:reallocated-{index}'
            case_b['upstream_case']['id'] = case_b['id']
            row_b = module.acquire_case(self.root, self.workspace, case_b, self.metadata, self.binary, out_b)
            # First verify with self.baseline on self.output, then drop references / collect.
            self.verify()
            holder = dict(baseline_b)
            del holder
            gc.collect()
            # Verifying row_b with baseline_b must succeed even if its id() matches a freed dict.
            module.verify_case(self.root, self.workspace, case_b, row_b, out_b, self.metadata, self.binary, baseline_b)
            # Mismatched baseline_a or mismatched explicit baseline_digest must still fail.
            with self.assertRaisesRegex(ValueError, 'module-eval workspace baseline changed'):
                module.verify_case(self.root, self.workspace, case_b, row_b, out_b, self.metadata, self.binary, self.baseline)
            with self.assertRaisesRegex(ValueError, 'module-eval workspace baseline changed'):
                module.verify_case(self.root, self.workspace, case_b, row_b, out_b, self.metadata, self.binary, baseline_b, baseline_digest='0' * 64)
            extra.unlink()


if __name__ == '__main__':
    unittest.main()
