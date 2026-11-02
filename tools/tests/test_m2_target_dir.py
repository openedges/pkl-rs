"""Recorded-build target path and artifact integrity regression controls."""
import copy
from contextlib import ExitStack
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools'))
import m2_language as subject

class TargetDirTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.base = Path(self.temp.name).resolve()
        self.evidence = self.base / 'evidence'
        (self.evidence / 'artifacts').mkdir(parents=True)
        (self.evidence / 'artifacts/language_probe').write_bytes(b'actual binary')
        self.target = self.base / 'fresh-cargo'
        self.build = dict(command=['cargo','build','-p','pkl-runtime','--locked','--release',
                                   '--example','language_probe','--target-dir',str(self.target)],
                          exit_code=0,timed_out=False)
        self.candidate = dict(path=str(self.target/'release/examples/language_probe'),
                              sha256=hashlib.sha256(b'actual binary').hexdigest(),
                              source_binding='recorded-build',build=self.build)
        self.report = dict(schema_version=6,cargo_target_dir=str(self.target),candidate=self.candidate,
                           module_candidate=None,classpath_candidate=None)
        self.write_build()

    def write_build(self):
        (self.evidence/'build.process.json').write_text(json.dumps(self.build))

    def verify(self):
        subject.verify_candidate_binding(self.evidence,self.report,'language_probe',self.candidate)

    def test_exact_fresh_target_command_and_artifact_pass(self):
        self.verify()

    def test_wrong_target_command_or_artifact_path_fails(self):
        for field in ('command','artifact','report'):
            original=copy.deepcopy((self.build,self.candidate,self.report))
            if field=='command': self.build['command'][-1]=str(self.base/'other')
            elif field=='artifact': self.candidate['path']=str(self.base/'other/release/examples/language_probe')
            else: self.report['cargo_target_dir']=str(self.base/'other')
            self.write_build()
            with self.assertRaises(ValueError): self.verify()
            self.build,self.candidate,self.report=original
            self.candidate['build']=self.build;self.report['candidate']=self.candidate

    def test_frozen_artifact_hash_and_build_failure_rejected(self):
        (self.evidence/'artifacts/language_probe').write_bytes(b'corrupted')
        with self.assertRaises(ValueError): self.verify()
        (self.evidence/'artifacts/language_probe').write_bytes(b'actual binary')
        for field,value in [('exit_code',1),('timed_out',True)]:
            original=self.build[field];self.build[field]=value;self.write_build()
            with self.assertRaises(ValueError): self.verify()
            self.build[field]=original

    def test_target_selection_rejects_existing_and_evidence_overlap(self):
        self.assertEqual(subject.select_cargo_target(None,self.evidence,[],[None]),subject.ROOT/'target')
        self.assertEqual(subject.select_cargo_target(self.target,self.evidence,[],[None]),self.target)
        for path, requested in [(self.evidence,[None]), (self.evidence/'build',[None]),
                                (self.base,[None]), (self.target,[Path('/tmp/supplied')])]:
            with self.assertRaises(ValueError): subject.select_cargo_target(path,self.evidence,[],requested)
        self.target.mkdir()
        with self.assertRaises(ValueError): subject.select_cargo_target(self.target,self.evidence,[],[None])

    def test_execute_uses_explicit_target_in_cargo_command(self):
        output=self.base/'run'
        case={'id':'sample','adapter':'language-api-string'}
        commands=[]
        def fake_capture(command, directory, name, environment=None, timeout=120):
            commands.append(command)
            return {'exit_code':0,'timed_out':False}
        with patch.object(subject.contract,'current_manifest',return_value={}), \
             patch.object(subject,'selected_cases',return_value=[case]), \
             patch.object(subject.oracle,'ensure_oracle',return_value={}), \
             patch.object(subject,'source_paths',return_value=[]), \
             patch.object(subject,'capture',side_effect=fake_capture):
            with self.assertRaisesRegex(ValueError,'candidate artifact missing'):
                subject.execute(output,None,cargo_target_dir=self.target)
        self.assertEqual(commands[1],['cargo','build','-p','pkl-runtime','--locked','--release',
                                      '--example','language_probe','--target-dir',str(self.target)])

    def acquire_all_adapters_with_mocked_processes(self, supplied_module=False):
        """Exercise execute's selection, staging and binding path, without binaries."""
        output = self.base / 'all-adapters'
        java = self.base / 'jdk/bin/java'
        java.parent.mkdir(parents=True)
        java.write_bytes(b'mocked java identity only')
        java.with_name('javac').write_bytes(b'mocked javac identity only')
        metadata = {'java': str(java), 'artifact': str(self.base / 'oracle.jar'),
                    'environment': {}}
        cases = [
            {'id': 'scalar', 'adapter': 'language-api-string'},
            {'id': 'module', 'adapter': subject.m2_module_eval.ADAPTER, 'upstream_case': {}},
            {'id': subject.m2_jvm_classpath.CASE_ID, 'adapter': subject.m2_jvm_classpath.ADAPTER},
            {'id': 'historical', 'adapter': subject.m2_historical.ADAPTER},
        ]
        manifest = {'cases': cases, 'comparison_boundary': 'mocked acquisition', 'counts': {}}
        supplied = self.base / 'supplied-module-probe' if supplied_module else None
        if supplied is not None:
            supplied.write_bytes(b'supplied module artifact, never a recorded build')
        commands = []
        children = []

        def fake_capture(command, directory, name, environment=None, timeout=120):
            commands.append((name, command, timeout))
            if name == 'build':
                destination = Path(command[command.index('--target-dir') + 1]) / 'release/examples'
                destination.mkdir(parents=True)
                for index, argument in enumerate(command):
                    if argument == '--example':
                        (destination / command[index + 1]).write_bytes(command[index + 1].encode())
            row = {'command': [str(x) for x in command], 'exit_code': 0, 'timed_out': False}
            for channel in ('stdout', 'stderr'):
                (directory / f'{name}.{channel}.bin').write_bytes(b'')
                row[channel + '_sha256'] = hashlib.sha256(b'').hexdigest()
            (directory / f'{name}.process.json').write_text(json.dumps(row))
            return row

        def fake_classpath(directory, candidate, workspace):
            directory.mkdir()
            child = {'candidate': {'source_binding': 'unbound-caller-supplied',
                                   'sha256': subject.oracle.sha256(candidate)},
                     'oracle': metadata}
            (directory / 'report.json').write_text(json.dumps(child))
            children.append(child)
            return child

        def result(case):
            return {'id': case['id'], 'status': 'pass'}

        with ExitStack() as stack:
            mocks = [
                (subject.contract, 'current_manifest', {'return_value': manifest}),
                (subject.contract, 'load', {'return_value': {'sources': {}}}),
                (subject.contract, 'full_gate_failures', {'return_value': []}),
                (subject.oracle, 'ensure_oracle', {'return_value': metadata}),
                (subject, 'source_paths', {'return_value': []}),
                (subject, 'capture', {'side_effect': fake_capture}),
                (subject, 'acquire_cases', {'side_effect': lambda selected, *_: [result(c) for c in selected]}),
                (subject.m2_workspace, 'profile_record', {'return_value': {'profile': 'mock'}}),
                (subject.m2_workspace, 'prepare', {'side_effect': lambda root, workspace, profile: workspace.mkdir()}),
                (subject.compat, 'validate_paths', {'return_value': None}),
                (subject.compat, 'validate_inputs', {'return_value': {'mocked': True}}),
                (subject.compat, 'snapshot', {'return_value': {}}),
                (subject.m2_module_eval, 'validate_resource_set', {'return_value': None}),
                (subject.m2_workspace, 'validate_baseline', {'return_value': None}),
                (subject.m2_module_eval, 'acquire_case', {'side_effect': lambda root, workspace, case, *_: result(case)}),
                (subject.m2_historical, 'acquire', {'side_effect': lambda root, output, selected, *_: ([result(c) for c in selected], {'mocked': True})}),
                (subject.m2_jvm_classpath, 'execute', {'side_effect': fake_classpath}),
                (subject, 'classpath_result', {'side_effect': lambda case, *_: result(case)}),
                (subject, 'check_report', {'return_value': None}),
            ]
            for owner, name, options in mocks:
                stack.enter_context(patch.object(owner, name, **options))
            report = subject.execute(output, None, module_candidate=supplied,
                                     cargo_target_dir=self.target)
        self.assertEqual([row['id'] for row in report['results']], [row['id'] for row in cases])
        self.assertTrue(report['artifact_stability'])
        for name, field in [('language_probe', 'candidate'), ('module_eval_probe', 'module_candidate'),
                            ('jvm_snippet_probe', 'classpath_candidate')]:
            subject.verify_candidate_binding(output, report, name, report[field])
        self.assertEqual(children[0]['candidate']['source_binding'], 'unbound-caller-supplied')
        return report, commands, supplied

    def test_all_three_internal_builds_use_exact_ordered_command_and_binding(self):
        report, commands, _ = self.acquire_all_adapters_with_mocked_processes()
        builds = [(command, timeout) for name, command, timeout in commands if name == 'build']
        self.assertEqual(builds, [(['cargo', 'build', '-p', 'pkl-runtime', '--locked', '--release',
                                  '--example', 'language_probe', '--example', 'module_eval_probe',
                                  '--example', 'jvm_snippet_probe', '--target-dir', str(self.target)], 300)])
        for field in ('candidate', 'module_candidate', 'classpath_candidate'):
            self.assertEqual(report[field]['source_binding'], 'recorded-build')
            self.assertEqual(report[field]['build']['command'], builds[0][0])
        self.assertTrue(subject.full_gate_passes(report))

    def test_mixed_acquisition_builds_only_missing_candidates_without_upgrading_supplied(self):
        report, commands, supplied = self.acquire_all_adapters_with_mocked_processes(True)
        builds = [command for name, command, _ in commands if name == 'build']
        self.assertEqual(builds, [['cargo', 'build', '-p', 'pkl-runtime', '--locked', '--release',
                                  '--example', 'language_probe', '--example', 'jvm_snippet_probe',
                                  '--target-dir', str(self.target)]])
        self.assertEqual(report['module_candidate']['path'], str(supplied))
        self.assertEqual(report['module_candidate']['sha256'], subject.oracle.sha256(supplied))
        self.assertEqual(report['module_candidate']['source_binding'], 'unbound-caller-supplied')
        self.assertIsNone(report['module_candidate']['build'])
        self.assertFalse((self.target / 'release/examples/module_eval_probe').exists())
        for field in ('candidate', 'classpath_candidate'):
            self.assertEqual(report[field]['source_binding'], 'recorded-build')
            self.assertEqual(report[field]['build']['command'], builds[0])
        self.assertFalse(subject.full_gate_passes(report))

    def test_selected_workspace_overlap_rejected(self):
        workspace=self.base/'workspace'
        with self.assertRaises(ValueError): subject.select_cargo_target(workspace/'build',self.evidence,[workspace],[None])

    def test_default_workspaces_rejected_before_any_mutation(self):
        for adapter, suffix in [
            (subject.m2_module_eval.ADAPTER, '-workspace'),
            (subject.m2_historical.ADAPTER, '-historical-workspace'),
            (subject.m2_jvm_classpath.ADAPTER, '-jvm-classpath-workspace'),
        ]:
            for relation in ('same', 'descendant', 'ancestor'):
                with self.subTest(adapter=adapter, relation=relation):
                    base = self.base / (adapter.replace(':', '_') + '-' + relation)
                    output = base / 'run'
                    workspace = base / ('run' + suffix)
                    if relation == 'same':
                        target = workspace
                    elif relation == 'descendant':
                        target = workspace / 'cargo'
                    else:
                        # Put both output and workspace below a fresh target.
                        target = base
                    case = {'id': 'case', 'adapter': adapter}
                    with patch.object(subject.contract, 'current_manifest', return_value={}), \
                         patch.object(subject, 'selected_cases', return_value=[case]), \
                         patch.object(subject.oracle, 'ensure_oracle', side_effect=AssertionError('must not execute')):
                        with self.assertRaises(ValueError):
                            subject.execute(output, None, cargo_target_dir=target)
                    self.assertFalse(output.exists())
                    self.assertFalse(target.exists())

    def test_symlink_alias_workspace_and_output_overlap_rejected(self):
        parent = self.base / 'real'
        parent.mkdir()
        alias = self.base / 'alias'
        alias.symlink_to(parent, target_is_directory=True)
        target = parent / 'future'
        for output, workspaces in [
            (self.evidence, [alias / 'future' / 'workspace']),
            (self.evidence, [alias / 'future']),
            (alias / 'future' / 'evidence', []),
        ]:
            with self.subTest(output=output, workspaces=workspaces):
                with self.assertRaises(ValueError):
                    subject.select_cargo_target(target, output, workspaces, [None])
        self.assertFalse(target.exists())

    def test_strict_gate_rejects_mixed_built_and_supplied_candidates(self):
        built = {'source_binding': 'recorded-build'}
        supplied = {'source_binding': 'unbound-caller-supplied'}
        report = {'full_gate_failures': [], 'candidate': built,
                  'module_candidate': supplied, 'classpath_candidate': built}
        self.assertFalse(subject.full_gate_passes(report))
        report['module_candidate'] = built
        self.assertTrue(subject.full_gate_passes(report))
        report['full_gate_failures'] = ['required case incomplete']
        self.assertFalse(subject.full_gate_passes(report))

    def test_verify_full_cli_rejects_mixed_provenance(self):
        report = {'status_counts': {'pass': 1}, 'full_gate_failures': [],
                  'candidate': {'source_binding': 'recorded-build'},
                  'module_candidate': {'source_binding': 'unbound-caller-supplied'},
                  'classpath_candidate': None}
        with patch.object(sys, 'argv', ['m2_language.py', '--verify-full', str(self.evidence)]), \
             patch.object(subject, 'check_report', return_value=report), \
             patch.object(subject, 'subset_passes', return_value=True):
            self.assertEqual(subject.main(), 1)
            report['module_candidate']['source_binding'] = 'recorded-build'
            self.assertEqual(subject.main(), 0)
            report['full_gate_failures'] = ['required case incomplete']
            self.assertEqual(subject.main(), 1)

    def test_default_legacy_target_still_accepted(self):
        del self.report['cargo_target_dir']
        self.build['command'][-1]=str(subject.ROOT/'target')
        self.candidate['path']=str(subject.ROOT/'target/release/examples/language_probe')
        self.write_build();self.verify()

    def test_supplied_candidate_never_upgraded(self):
        self.candidate['source_binding']='unbound-caller-supplied';self.candidate['build']=None
        self.verify()
        self.candidate['build']=self.build
        with self.assertRaises(ValueError): self.verify()

if __name__ == '__main__': unittest.main()
