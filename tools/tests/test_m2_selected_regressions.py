"""Original t40 contracts, bounded provenance, and canonical runner regressions."""
import copy
from contextlib import redirect_stdout
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_language
import m2_manifest
import m2_module_eval
import m2_selected_regressions as selected

PROJECT = Path(m2_module_eval.__file__).resolve().parents[1]


class SelectedOriginalTests(unittest.TestCase):
    def setUp(self):
        # Keep owned test workspaces under a canonical project directory. On
        # macOS the default /var temporary spelling violates compat.no_symlinks.
        (PROJECT/'target').mkdir(exist_ok=True)
        self.temporary = tempfile.TemporaryDirectory(prefix='stage153-t40-unit-', dir=PROJECT/'target')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)/'root'
        self.root.mkdir()
        (self.root/'target').mkdir()
        originals = json.loads((PROJECT/'compatibility/regressions.json').read_bytes())
        (self.root/'compatibility').mkdir()
        (self.root/'compatibility/regressions.json').write_bytes(compat.encoded(originals))
        wanted = {r['path'] for c in originals['cases'] if c['id'] in selected.SELECTED for r in c['resources']}
        wanted.update(r['path'] for r in originals['files'] if r['kind'] == 'license')
        for name in wanted:
            destination = self.root/name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((PROJECT/name).read_bytes())
        self.originals = originals
        self.registry = selected.build_registry(self.root)
        (self.root/selected.REGISTRY).write_bytes(compat.encoded(self.registry))
        self.entries = selected.registry_cases(self.root)
        self.cases = [selected.manifest_case(c, self.entries.get('m2:'+c['id'])) for c in originals['cases']]

    def test_exact_two_original_commands_and_t36_unresolved(self):
        self.assertEqual(list(self.entries), ['m2:'+name for name in selected.SELECTED])
        self.assertEqual(len(self.registry['resources']), 7)
        self.assertIsNone(self.cases[0]['adapter'])
        self.assertEqual(self.cases[0]['execution']['state'], 'unimplemented')
        for case in self.cases[1:]:
            self.assertEqual(case['upstream_case'], case['regression_case'])
            command = case['upstream_case']['command']
            self.assertIn('--color=never', command)
            self.assertNotIn('--no-cache', command)
            self.assertNotIn('--expression', command)
        selected.validate_manifest(self.root, dict(cases=self.cases))

    def test_original_argv_exit_or_resource_mutations_are_rejected(self):
        path = self.root/'compatibility/regressions.json'
        for change in ['cache', 'color', 'exit', 'resource']:
            broken = copy.deepcopy(self.originals)
            case = broken['cases'][1]
            if change == 'cache':
                case['command'].insert(1, '--no-cache')
            elif change == 'color':
                case['command'].remove('--color=never')
            elif change == 'exit':
                case['expected_exit'] = [1]
            else:
                case['resources'].pop()
            path.write_bytes(compat.encoded(broken))
            with self.assertRaises(ValueError):
                selected.build_registry(self.root)
        path.write_bytes(compat.encoded(self.originals))

    def test_full_helper_bytes_and_license_are_required(self):
        for row in self.registry['resources']:
            path = self.root/row['path']
            original = path.read_bytes()
            path.write_bytes(original+b'\n')
            with self.assertRaises(ValueError):
                selected.registry_cases(self.root)
            path.write_bytes(original)

    def test_no_scope_expansion_or_duplicate_registry_admission(self):
        for change in ['extra', 'duplicate', 'reorder']:
            broken = copy.deepcopy(self.registry)
            if change == 'extra':
                broken['cases'].append(dict(id='m2:rev1:t36-foreign-nominal-identity'))
            elif change == 'duplicate':
                broken['cases'].append(copy.deepcopy(broken['cases'][0]))
            else:
                broken['cases'].reverse()
            with self.assertRaises(ValueError):
                selected.registry_cases(self.root, broken)

    def test_manifest_cannot_promote_t36_or_rewrite_original(self):
        for change in ['t36', 'contract', 'unknown']:
            cases = copy.deepcopy(self.cases)
            if change == 't36':
                cases[0]['adapter'] = selected.ADAPTER
            elif change == 'contract':
                cases[1]['upstream_case']['command'].append('--no-cache')
            else:
                cases.append(dict(id='other', adapter=selected.ADAPTER))
            with self.assertRaises(ValueError):
                selected.validate_manifest(self.root, dict(cases=cases))

    def test_workspace_is_exact_and_fresh_without_project_files(self):
        output = self.root/'target/evidence'
        output.mkdir()
        workspace = self.root/'target/workspace'
        baseline = selected.prepare_workspace(self.root, workspace, output, self.registry)
        self.assertEqual(baseline, selected.resource_baseline(self.registry))
        self.assertFalse(any('t36' in name or 'PklProject' in name for name in baseline))
        for row in self.registry['resources']:
            self.assertEqual((workspace/row['path']).read_bytes(), (self.root/row['path']).read_bytes())
        with self.assertRaises(ValueError):
            selected.prepare_workspace(self.root, workspace, output, self.registry)

    def test_capture_requires_both_ids_and_reuses_original_four_channel_path(self):
        output = self.root/'target/evidence'
        output.mkdir()
        calls = []
        def capture(root, workspace, case, metadata, binary, evidence):
            self.assertEqual(case['upstream_case'], self.entries[case['id']]['contract'])
            calls.append(case['id'])
            return dict(id=case['id'], adapter=m2_module_eval.ADAPTER, status='mismatch')
        with patch.object(m2_module_eval, 'acquire_case', side_effect=capture):
            rows, context = selected.acquire(self.root, output, self.cases[1:], {}, Path('/unexecuted/probe'))
        self.assertEqual(calls, list(self.entries))
        self.assertEqual([r['adapter'] for r in rows], [selected.ADAPTER]*2)
        self.assertEqual([r['status'] for r in rows], ['mismatch']*2)
        self.assertEqual(selected.verify_context(self.root, output, context), selected.resource_baseline(self.registry))
        for cases in [self.cases[1:2], self.cases[:1], list(reversed(self.cases[1:]))]:
            with self.assertRaises(ValueError):
                selected.acquire(self.root, output, cases, {}, Path('/unexecuted/probe'))
        context['historical_results_reused'] = True
        with self.assertRaises(ValueError):
            selected.verify_context(self.root, output, context)

    def captured_distinct_id_case(self):
        """No Java/native process: exercise capture/store/verify with raw fake I/O."""
        case = self.cases[1]
        self.assertNotEqual(case['id'], case['upstream_case']['id'])
        output = self.root/'target/distinct-id-evidence'
        output.mkdir()
        workspace = self.root/'target/distinct-id-workspace'
        baseline = selected.prepare_workspace(self.root, workspace, output, self.registry)
        metadata = dict(command=['/unexecuted/java', '-Duser.home=/placeholder',
                                 '-Djava.io.tmpdir=/placeholder'],
                        environment=dict(LANG='C', LC_ALL='C', TZ='UTC', TERM='dumb', NO_COLOR='1'))
        binary = Path('/unexecuted/module_eval_probe')
        state = workspace/'.state'/compat.case_directory(case['upstream_case']['id'])
        payload = b'private\x00\xff\n'
        def capture(argv, cwd, environment, timeout, generated):
            self.assertEqual(cwd, workspace)
            self.assertEqual(environment['HOME'], str(state/'home'))
            self.assertEqual(environment['TMPDIR'], str(state/'tmp'))
            self.assertEqual(argv[-len(case['upstream_case']['command']):], case['upstream_case']['command'])
            (Path(environment['HOME'])/'private.cache').write_bytes(payload)
            return dict(stdout=b'raw output\x00\xff\n', stderr=b'', files={},
                        exit_code=0, timed_out=False, elapsed_ns=1)
        with patch.object(compat, 'capture', side_effect=capture):
            row = m2_module_eval.acquire_case(self.root, workspace, case, metadata, binary, output)
        row['adapter'] = selected.ADAPTER
        return case, row, output, workspace, baseline, metadata, binary, payload

    def test_distinct_adapter_id_preserves_original_state_and_private_bytes(self):
        case, row, output, workspace, baseline, metadata, binary, payload = self.captured_distinct_id_case()
        folder = output/'cases'/compat.case_directory(case['id'])
        self.assertEqual(row['directory'], folder.name)
        for runtime in ('java', 'rust'):
            self.assertEqual(row[runtime]['private_files_sha256'], {'home/private.cache': compat.digest(payload)})
            self.assertEqual((folder/runtime/'private-state/home/private.cache').read_bytes(), payload)
        with patch.object(compat, 'case_execution', side_effect=AssertionError('verification must not execute/reset state')):
            selected.verify_case(self.root, output, dict(workspace=str(workspace)), case, row,
                                 metadata, binary, baseline)

    def test_distinct_id_verifier_rejects_rewritten_state_argv_cwd_source_and_environment(self):
        case, row, output, workspace, baseline, metadata, binary, payload = self.captured_distinct_id_case()
        folder = output/'cases'/compat.case_directory(case['id'])
        process_path = folder/'java/process.json'
        original = copy.deepcopy(row['java'])
        wrong_state = workspace/'.state'/compat.case_directory(case['id'])
        mutations = [
            {**original, 'command': [f'-Duser.home={wrong_state/"home"}' if arg.startswith('-Duser.home=') else arg for arg in original['command']]},
            {**original, 'cwd': str(workspace/'other')},
            {**original, 'source_uri': (workspace/'other.pkl').as_uri()},
            {**original, 'upstream_case_sha256': '0'*64},
            {**original, 'environment': {**original['environment'], 'HOME': str(wrong_state/'home')}}]
        for mutated in mutations:
            changed = {**row, 'java': mutated}
            process_path.write_bytes(compat.encoded(mutated))
            with self.subTest(mutated=mutated), self.assertRaises(ValueError):
                selected.verify_case(self.root, output, dict(workspace=str(workspace)), case, changed,
                                     metadata, binary, baseline)
        process_path.write_bytes(compat.encoded(original))

    def test_selected_only_canonical_driver_requests_module_artifact_and_schema7(self):
        output = self.root/'target/canonical'
        binary = self.root/'target/candidate'
        binary.write_bytes(b'unit-test placeholder; never executed')
        (self.root/'source.txt').write_bytes(b'fixed source')
        manifest = dict(cases=self.cases, requirements=[], counts={}, comparison_boundary='test', module_comparison_boundary='test')
        def acquire(root, evidence, cases, metadata, frozen, workspace):
            self.assertEqual([c['id'] for c in cases], list(self.entries))
            self.assertEqual(frozen.name, 'module_eval_probe')
            self.assertEqual(frozen.read_bytes(), binary.read_bytes())
            return [dict(id=c['id'], adapter=selected.ADAPTER, status='mismatch') for c in cases], dict(profile=selected.PROFILE)
        with patch.object(m2_language, 'ROOT', self.root), \
             patch.object(m2_manifest, 'current_manifest', return_value=manifest), \
             patch.object(m2_language.oracle, 'ensure_oracle', return_value=dict(environment={}, java='/unexecuted/java')), \
             patch.object(m2_language, 'capture', return_value=dict(exit_code=0)), \
             patch.object(m2_language, 'source_paths', return_value=['source.txt']) as provenance, \
             patch.object(selected, 'acquire', side_effect=acquire), \
             patch.object(m2_language, 'check_report'):
            report = m2_language.execute(output, None, adapters=[selected.ADAPTER], module_candidate=binary)
        self.assertEqual(provenance.call_args.args, (self.root, selected.SOURCE_PROFILE))
        self.assertEqual(report['schema_version'], 7)
        self.assertEqual(report['source_profile'], selected.SOURCE_PROFILE)
        self.assertEqual(report['selection']['case_ids'], list(self.entries))
        self.assertEqual(report['module_candidate']['source_binding'], 'unbound-caller-supplied')
        self.assertFalse(m2_language.full_gate_passes(report))
        self.assertTrue(report['full_gate_failures'])

    def test_source_profile_does_not_enumerate_unrelated_oracle_fixture_trees(self):
        path = self.root/'crates/pkg/src/lib.rs'
        path.parent.mkdir(parents=True)
        path.write_text('pub fn example() {}\n')
        unrelated = self.root/'tests/fixtures/m2-map-apis-oracle/unused.bin'
        unrelated.parent.mkdir(parents=True)
        unrelated.write_bytes(b'not a selected dependency')
        paths = selected.source_paths(self.root)
        self.assertIn('crates/pkg/src/lib.rs', paths)
        self.assertNotIn(unrelated.relative_to(self.root).as_posix(), paths)
        self.assertTrue({r['path'] for r in self.registry['resources']} <= set(paths))

    def test_canonical_cli_admits_only_explicit_adapter_and_workspace(self):
        output = self.root/'target/cli-output'
        workspace = self.root/'target/cli-workspace'
        candidate = self.root/'target/cli-candidate'
        candidate.write_bytes(b'never executed')
        report = dict(status_counts={'mismatch': 2}, full_gate_failures=['unexecuted required scope'])
        argv = ['m2_language.py', '--output', str(output), '--adapter', selected.ADAPTER,
                '--module-candidate', str(candidate), '--selected-regression-workspace',
                str(workspace), '--record-only']
        with patch.object(sys, 'argv', argv), patch.object(m2_language, 'execute', return_value=report) as execute, redirect_stdout(io.StringIO()):
            self.assertEqual(m2_language.main(), 0)
        self.assertEqual(execute.call_args.kwargs['adapters'], [selected.ADAPTER])
        self.assertEqual(execute.call_args.kwargs['selected_regression_workspace'], workspace)
        self.assertEqual(execute.call_args.kwargs['module_candidate'], candidate)
        with self.assertRaises(ValueError):
            m2_language.selected_cases(dict(cases=self.cases), [selected.ADAPTER, selected.ADAPTER])
        with self.assertRaises(ValueError):
            m2_language.source_paths(self.root, 'unreviewed-source-profile')

    def test_raw_status_does_not_normalize_or_accept_wrong_oracle_outcome(self):
        proc = dict(exit_code=1, timed_out=False)
        error = '–– Pkl Error ––\nwhole error\n'.encode()
        expected = dict(stdout=b'', stderr=error, exit_code=1, files={}, workspace_changes={})
        actual = copy.deepcopy(expected)
        self.assertEqual(m2_module_eval.status(proc, proc, expected, actual, [1]), 'pass')
        for key, value in [('stdout', b'\n'), ('stderr', error[:-1]), ('files', {'generated/x': b'x'})]:
            actual = {**expected, key: value}
            self.assertEqual(m2_module_eval.status(proc, proc, expected, actual, [1]), 'mismatch')
        self.assertEqual(m2_module_eval.status(proc, proc, expected, expected, [0]), 'oracle-contract-mismatch')
        internal = {**expected, 'stderr': b'java.lang.InternalError\n'}
        self.assertEqual(m2_module_eval.status(proc, proc, internal, internal, [1]), 'oracle-error')
        self.assertEqual(m2_module_eval.status(proc, proc, {**expected, 'workspace_changes': {'unexpected': {}}}, expected, [1]), 'oracle-error')


if __name__ == '__main__':
    unittest.main()
