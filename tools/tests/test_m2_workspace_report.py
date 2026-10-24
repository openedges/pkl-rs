"""Module workspace report provenance, using original contracts and fake processes."""
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_language
import m2_manifest
import m2_module_eval
import m2_workspace
import test_m2_language as scalar_tests
import test_m2_workspace as workspace_tests


class WorkspaceReportTests(unittest.TestCase):
    def setUp(self):
        self.fixture = workspace_tests.WorkspaceProfileTests()
        self.fixture.setUp()
        self.addCleanup(self.fixture.doCleanups)
        self.root = self.fixture.root
        self.workspace = self.fixture.destination
        self.output = self.root / 'target/report'
        self.original = self.fixture.corpus['cases'][0]
        self.original.update(command=['eval', '--no-project', '--settings', 'pkl:settings',
                                      'external/pkl/input.pkl'],
                             environment={'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8'},
                             expected_exit=[0])
        self.case = self.fixture.cases[0]
        self.case.update(required=True, requirements=['feature'])
        self.manifest = dict(cases=[self.case], requirements=[dict(id='feature', required=True,
            implementation=dict(state='partial', links=[]),
            tests=dict(state='partial', case_ids=[self.case['id']]))])

    def acquire_report(self, *, schema=5, capture_profile_tool=True):
        profile = m2_workspace.ORIGINAL_PROFILE if schema == 5 else m2_workspace.DEFAULT_PROFILE
        self.profile = self.fixture.record(profile)
        self.fixture.prepare(self.profile)
        artifacts = self.output / 'artifacts'
        artifacts.mkdir(parents=True)
        executable = self.root / 'fake-cli'
        executable.write_text(f'#!{sys.executable}\n'
            'from pathlib import Path\nimport sys\n'
            'sys.stdout.buffer.write(Path(sys.argv[-1]).read_bytes() + bytes([0,255,13,10]))\n')
        executable.chmod(0o755)
        frozen = artifacts / 'module_eval_probe'
        shutil.copy2(executable, frozen)
        self.metadata = dict(command=[str(executable)], environment=dict(PATH=os.defpath,
            LANG='C', LC_ALL='C', TZ='UTC', TERM='dumb', NO_COLOR='1'))
        self.baseline = compat.snapshot(self.workspace)
        row = m2_module_eval.acquire_case(self.root, self.workspace, self.case,
                                         self.metadata, frozen, self.output)
        self.assertEqual(row['status'], 'pass')
        self.manifest['sources'] = {'compatibility/corpus.json':
            compat.digest(compat.encoded(self.fixture.corpus))}
        self.sources = {'compatibility/m2-language.json': compat.encoded(self.manifest),
                        'compatibility/corpus.json': compat.encoded(self.fixture.corpus)}
        if capture_profile_tool:
            tool = m2_manifest.ROOT / 'tools/m2_workspace.py'
            self.sources['tools/m2_workspace.py'] = tool.read_bytes()
        for relative, data in self.sources.items():
            path = self.output / 'sources' / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        (self.output / 'module-workspace.json').write_bytes(compat.encoded(self.baseline))
        declared = {resource['path']: resource['sha256']
                    for source in self.fixture.corpus['sources'] for resource in source['resources']}
        (self.output / 'module-inputs.json').write_bytes(compat.encoded(dict(
            files=len(declared), sha256=compat.digest(compat.encoded(declared)))))
        selection = dict(adapters=[m2_module_eval.ADAPTER], case_ids=[self.case['id']])
        (self.output / 'selection.json').write_bytes(compat.encoded(selection))
        context = dict(workspace=str(self.workspace), workers=1,
            watchdog_seconds=m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
            baseline_sha256=m2_language.oracle.sha256(self.output / 'module-workspace.json'),
            input_validation_sha256=m2_language.oracle.sha256(self.output / 'module-inputs.json'))
        if schema == 5:
            (self.output / 'module-workspace-profile.json').write_bytes(compat.encoded(self.profile))
            context.update(workspace_profile=profile, workspace_profile_sha256=
                m2_language.oracle.sha256(self.output / 'module-workspace-profile.json'))
        self.report = dict(schema_version=schema, selection=selection, oracle=self.metadata,
            source_stability=True, artifact_stability=True,
            source_sha256={name: compat.digest(data) for name, data in self.sources.items()},
            candidate=None, module_candidate=dict(sha256=m2_language.oracle.sha256(frozen),
                source_binding='unbound-caller-supplied', build=None), module_eval=context,
            selected_count=1, execution=dict(workers=1), results=[row], metamorphic_results=[],
            status_counts={'pass': 1}, full_gate_failures=m2_manifest.full_gate_failures(self.manifest, [row]))
        self.save()

    def save(self):
        # Refresh the outer evidence hashes so tampering must fail semantic
        # provenance checks, rather than merely the general file integrity gate.
        self.report['evidence_sha256'] = {str(path.relative_to(self.output)):
            m2_language.oracle.sha256(path) for path in self.output.rglob('*')
            if path.is_file() and path != self.output / 'report.json'}
        (self.output / 'report.json').write_bytes(compat.encoded(self.report))

    def verify(self):
        with patch.object(m2_manifest, 'current_manifest', return_value=self.manifest), \
                patch.object(m2_language, 'source_paths', return_value=list(self.sources)), \
                patch.object(m2_language, 'verify_oracle'):
            return m2_language.check_report(self.output)

    def test_explicit_profile_report_checks_raw_process_and_complete_pinned_baseline_offline(self):
        self.acquire_report()
        # Offline verification must not rerun either executable or need the
        # mutable live workspace after acquisition.
        (self.root / 'fake-cli').unlink()
        shutil.rmtree(self.workspace)
        report = self.verify()
        self.assertEqual(report['schema_version'], 5)
        self.assertTrue(m2_language.subset_passes(report))
        self.assertTrue(report['full_gate_failures'])
        self.assertEqual((self.output / 'cases' / report['results'][0]['directory'] /
                          'rust/stdout.bin').read_bytes(), b'value = 42\n\x00\xff\r\n')

    def test_profile_name_hash_and_record_tampering_are_rejected_after_rehash(self):
        self.acquire_report()
        original_report = copy.deepcopy(self.report)
        profile_path = self.output / 'module-workspace-profile.json'
        original_profile = profile_path.read_bytes()
        for mutation in ('context-profile', 'context-hash', 'record-profile', 'source-revision'):
            self.report = copy.deepcopy(original_report)
            profile_path.write_bytes(original_profile)
            if mutation == 'context-profile':
                self.report['module_eval']['workspace_profile'] = m2_workspace.DEFAULT_PROFILE
            elif mutation == 'context-hash':
                self.report['module_eval']['workspace_profile_sha256'] = '0' * 64
            else:
                profile = json.loads(original_profile)
                if mutation == 'record-profile':
                    profile.update(profile=m2_workspace.DEFAULT_PROFILE, project_fixtures='include')
                else:
                    profile['retained_sources'][0]['revision'] = 'different-source'
                profile_path.write_bytes(compat.encoded(profile))
                self.report['module_eval']['workspace_profile_sha256'] = m2_language.oracle.sha256(profile_path)
            self.save()
            with self.subTest(mutation=mutation), self.assertRaisesRegex(ValueError, 'profile provenance'):
                self.verify()

    def test_captured_profile_tool_prevents_schema_downgrade_even_after_dropping_profile_fields(self):
        self.acquire_report()
        self.report['schema_version'] = 3
        self.report['module_eval'].pop('workspace_profile')
        self.report['module_eval'].pop('workspace_profile_sha256')
        (self.output / 'module-workspace-profile.json').unlink()
        self.save()
        with self.assertRaisesRegex(ValueError, 'captured workspace-profile tooling requires report schema 5'):
            self.verify()

    def test_profile_fields_cannot_be_hidden_in_legacy_report_without_profile_tool_snapshot(self):
        self.acquire_report(capture_profile_tool=False)
        self.report['schema_version'] = 3
        self.save()
        with self.assertRaisesRegex(ValueError, 'workspace profile requires report schema 5'):
            self.verify()

    def test_legacy_schema3_full_fixture_report_remains_valid(self):
        self.acquire_report(schema=3, capture_profile_tool=False)
        self.assertIn('tests/fixtures/unrelated.pkl', self.baseline)
        report = self.verify()
        self.assertEqual(report['schema_version'], 3)
        self.assertNotIn('workspace_profile', report['module_eval'])

    def test_rehashed_baseline_cannot_add_or_remove_unrelated_pinned_resources(self):
        self.acquire_report()
        path = self.output / 'module-workspace.json'
        report = copy.deepcopy(self.report)
        for mutation in ('omit-pantry-resource', 'extra-project-fixture', 'missing-directory'):
            self.report = copy.deepcopy(report)
            baseline = copy.deepcopy(self.baseline)
            if mutation == 'omit-pantry-resource':
                baseline.pop('external/pkl-pantry/other.pkl')
            elif mutation == 'extra-project-fixture':
                baseline['tests/fixtures/extra.pkl'] = dict(kind='file', sha256='0' * 64)
            else:
                baseline.pop('external/pkl')
            path.write_bytes(compat.encoded(baseline))
            self.report['module_eval']['baseline_sha256'] = m2_language.oracle.sha256(path)
            self.save()
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.verify()

    def test_self_consistent_forged_resource_inventory_cannot_replace_manifest_corpus_pin(self):
        self.acquire_report()
        corpus = copy.deepcopy(self.fixture.corpus)
        pantry = corpus['sources'][1]
        pantry['resources'][0]['sha256'] = '0' * 64
        pantry['resource_set_sha256'] = compat.digest(compat.encoded(pantry['resources']))
        corpus_path = self.output / 'sources/compatibility/corpus.json'
        corpus_path.write_bytes(compat.encoded(corpus))
        self.report['source_sha256']['compatibility/corpus.json'] = m2_language.oracle.sha256(corpus_path)
        profile = m2_workspace.profile_record(m2_workspace.ORIGINAL_PROFILE, corpus, [self.case])
        profile_path = self.output / 'module-workspace-profile.json'
        profile_path.write_bytes(compat.encoded(profile))
        context = self.report['module_eval']
        context['workspace_profile_sha256'] = m2_language.oracle.sha256(profile_path)
        baseline = copy.deepcopy(self.baseline)
        baseline['external/pkl-pantry/other.pkl']['sha256'] = '0' * 64
        baseline_path = self.output / 'module-workspace.json'
        baseline_path.write_bytes(compat.encoded(baseline))
        context['baseline_sha256'] = m2_language.oracle.sha256(baseline_path)
        declared = {resource['path']: resource['sha256']
                    for source in corpus['sources'] for resource in source['resources']}
        inputs_path = self.output / 'module-inputs.json'
        inputs_path.write_bytes(compat.encoded(dict(files=len(declared),
            sha256=compat.digest(compat.encoded(declared)))))
        context['input_validation_sha256'] = m2_language.oracle.sha256(inputs_path)
        row = self.report['results'][0]
        for runtime in ('java', 'rust'):
            row[runtime]['workspace_before_sha256'] = compat.digest(compat.encoded(baseline))
            row[runtime]['workspace_after_sha256'] = compat.digest(compat.encoded(baseline))
            path = self.output / 'cases' / row['directory'] / runtime / 'process.json'
            path.write_bytes(compat.encoded(row[runtime]))
        self.save()
        with self.assertRaisesRegex(ValueError, 'corpus.*(pin|identity|manifest)|manifest.*corpus'):
            self.verify()

    def test_unselected_or_unknown_profile_is_rejected_before_oracle_or_output_creation(self):
        scalar = dict(self.case, adapter='language-api-string')
        manifest = dict(self.manifest, cases=[scalar])
        for profile in (m2_workspace.ORIGINAL_PROFILE, 'unknown-profile'):
            with self.subTest(profile=profile), \
                    patch.object(m2_manifest, 'current_manifest', return_value=manifest), \
                    patch.object(m2_language.oracle, 'ensure_oracle') as ensure:
                with self.assertRaisesRegex(ValueError, 'workspace profile'):
                    m2_language.execute(self.output, None, adapters=['language-api-string'],
                                        module_workspace_profile=profile)
                ensure.assert_not_called()
                self.assertFalse(self.output.exists())


class UnselectedWorkspaceReportTests(unittest.TestCase):
    def test_scalar_report_cannot_claim_an_unselected_module_workspace_profile(self):
        fixture = scalar_tests.EvidenceTests()
        fixture.setUp()
        self.addCleanup(fixture.doCleanups)
        original_report = copy.deepcopy(fixture.report)
        profile_path = fixture.directory / 'module-workspace-profile.json'
        for mutation in ('context', 'profile-file', 'schema5'):
            fixture.report = copy.deepcopy(original_report)
            profile_path.unlink(missing_ok=True)
            if mutation == 'context':
                fixture.report['module_eval'] = dict(workspace_profile=m2_workspace.ORIGINAL_PROFILE,
                                                     workspace_profile_sha256='0' * 64)
            elif mutation == 'profile-file':
                profile_path.write_bytes(compat.encoded(dict(profile=m2_workspace.ORIGINAL_PROFILE)))
            else:
                fixture.report['schema_version'] = 5
            fixture.refresh()
            with self.subTest(mutation=mutation), \
                    self.assertRaisesRegex(ValueError, 'unselected.*module|module.*unselected'):
                m2_language.check_report(fixture.directory)


if __name__ == '__main__':
    unittest.main()
