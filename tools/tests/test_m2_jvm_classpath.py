"""Offline profile/argv/outcome tampering and raw-channel checks; no JVM execution."""
import copy
import json
import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_jvm_classpath as module
import oracle


class ProfileTests(unittest.TestCase):
    def test_current_profile_binds_original_blocked_whole_input_and_services(self):
        profile = module.load_profile()
        original = module.original_case(module.ROOT)
        self.assertEqual(profile['expected_exit'], [0])
        self.assertEqual(original['execution']['status'], 'blocked')
        self.assertEqual(profile['original_case_sha256'], compat.digest(compat.encoded(original)))
        self.assertEqual(profile['input']['sha256'], oracle.sha256(module.ROOT / module.INPUT))
        self.assertIn(module.RESOURCE, profile['classpath_resources'])
        for service in ('org.pkl.core.module.ModuleKeyFactory', 'org.pkl.core.resource.ResourceReader'):
            self.assertIn(module.CLASSPATH_ROOT + '/META-INF/services/' + service, profile['classpath_resources'])
        self.assertTrue(profile['configuration']['power_assertions'])
        self.assertEqual(profile['configuration']['stack_frame_transformer'], 'StackFrameTransformers.empty')

    def test_build_source_coverage_includes_exact_root_cargo_config_and_workspace_manifests(self):
        paths = module.source_paths()
        for name in ('.cargo/config.toml', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                     'xtask/Cargo.toml', 'crates/pkl-runtime/Cargo.toml', 'crates/pkl-napi/build.rs'):
            self.assertIn(name, paths)

    def test_classpath_extra_or_changed_resources_are_not_silently_accepted(self):
        profile = module.load_profile()
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            shutil.copytree(module.ROOT / module.CLASSPATH_ROOT, root / module.CLASSPATH_ROOT)
            module.validate_classpath(root, profile)
            resource = root / module.RESOURCE
            before = resource.read_bytes()
            resource.write_bytes(before + b' ')
            with self.assertRaisesRegex(ValueError, 'classpath root'):
                module.validate_classpath(root, profile)
            resource.write_bytes(before)
            (root / module.CLASSPATH_ROOT / 'extra.pkl').write_bytes(b'value=1')
            with self.assertRaisesRegex(ValueError, 'classpath root'):
                module.validate_classpath(root, profile)


def synthetic_report(output, original_root):
    """Synthetic process/artifact evidence, explicitly unrelated to Java results."""
    snapshot = output / 'sources'
    profile = module.load_profile()
    # source_paths reads both retained registries before enumerating their raw
    # resources. Bootstrap these inputs alongside the JVM profile, not a mocked
    # or empty catalog that would silently omit historical source evidence.
    initial = {module.PROFILE, module.m2_source_inputs.HISTORICAL,
               module.m2_source_inputs.SELECTED_REGRESSIONS,
               'compatibility/corpus.json', module.INPUT,
               *profile['reference']['sources'], *profile['classpath_resources']}
    for name in initial:
        path = snapshot / name
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(module.ROOT / name, path)
    # Only these synthetic compiler/runner records are under test. No cargo,
    # javac, Java executable, or captured artifact is invoked.
    for name in module.source_paths(snapshot):
        path = snapshot / name
        if not path.exists():
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(module.ROOT / name, path)
    artifacts = output / 'artifacts'
    classes = artifacts / 'java'
    for name in ('M2JvmSnippetOutputProbe.class', 'org/pkl/core/module/TestModuleKeyFactory.class',
                 'org/pkl/core/resource/TestResourceReader.class'):
        path = classes / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'synthetic class, never executed\n')
    (artifacts / 'jvm_snippet_probe').write_bytes(b'synthetic native artifact, never executed\n')
    (artifacts / 'javac-launcher').write_bytes(b'synthetic compiler, never executed\n')
    jar = artifacts / 'pkl-tools-0.32.0.jar'
    jar.write_bytes(b'synthetic jar, never executed\n')
    lock = compat.read_json(snapshot / 'oracle.lock.json')
    lock['artifact']['sha256'] = oracle.sha256(jar)
    compat.write_json(snapshot / 'oracle.lock.json', lock)
    cache = original_root / '.cache/oracle'
    environment = dict(PATH=os.defpath, HOME=str(cache / 'home'), TMPDIR=str(cache / 'tmp'),
        TMP=str(cache / 'tmp'), TEMP=str(cache / 'tmp'), LC_ALL='C', LANG='C', TZ='UTC',
        TERM='dumb', NO_COLOR='1', COLUMNS='80')
    java = '/synthetic-jdk/bin/java'
    oracle_jar = str(cache / 'pkl-tools-0.32.0.jar')
    metadata = dict(profile='reference', version=lock['version'], source=lock['source'],
        java=java, artifact=oracle_jar, artifact_sha256=lock['artifact']['sha256'],
        lock_sha256=oracle.sha256(snapshot / 'oracle.lock.json'), environment=environment,
        jdk=dict(executable=java, properties=lock['jdk']['properties']),
        command=[java, *lock['jvm_options'], '-Duser.home=' + environment['HOME'],
                 '-Djava.io.tmpdir=' + environment['TMPDIR'], '-cp', oracle_jar, 'org.pkl.cli.Main'])
    compat.write_json(output / 'oracle.json', metadata)
    original = module.original_case(snapshot)
    corpus = compat.read_json(snapshot / 'compatibility/corpus.json')
    declared = {item['path']: item for source in corpus['sources'] for item in source['resources']}
    baseline = {name: dict(kind=item['kind'], sha256=item['sha256']) for name, item in declared.items()}
    compat.write_json(output / 'workspace-baseline.json', baseline)
    input_hashes = {name: item['sha256'] for name, item in declared.items()}
    compat.write_json(output / 'workspace-inputs.json',
        dict(files=len(input_hashes), sha256=compat.digest(compat.encoded(input_hashes))))
    compiler = module.compiler_command(metadata, output)
    compile_row = dict(command=compiler, exit_code=0, timed_out=False,
        stdout_sha256=compat.digest(b''), stderr_sha256=compat.digest(b''))
    for channel in ('stdout', 'stderr'):
        (output / ('javac.' + channel + '.bin')).write_bytes(b'')
    compat.write_json(output / 'javac.process.json', compile_row)
    workspace = original_root / 'target/classpath-synthetic-workspace'
    results = []
    for repeat in range(2):
        processes = {}
        for runtime in ('java', 'rust'):
            folder = output / 'cases' / str(repeat) / runtime
            captured = dict(stdout=b'whole raw output  \r\n', stderr=b'', exit_code=0,
                            timed_out=False, elapsed_ns=1, workspace_changes={}, files={})
            stored = compat.store_capture(folder, captured)
            outcome = folder / 'outcome.bin'
            if runtime == 'java':
                outcome.write_bytes(b'success\n')
            state = workspace / '.state' / compat.case_directory(module.CASE_ID)
            runtime_env = {**environment, **original['environment'], 'HOME': str(state / 'home'),
                'TMPDIR': str(state / 'tmp'), 'TMP': str(state / 'tmp'), 'TEMP': str(state / 'tmp')}
            keys = set(original['environment']) | {'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'NO_COLOR'}
            process = dict(command=module.process_command(runtime, metadata, output, workspace, repeat),
                cwd=str(workspace / original['cwd']), environment={key: runtime_env[key] for key in keys},
                original_case_sha256=compat.digest(compat.encoded(original)),
                source_uri=(workspace / module.INPUT).as_uri(), classpath_root=str(workspace / module.CLASSPATH_ROOT),
                timeout_seconds=module.TIMEOUT, capture=stored,
                outcome_sha256=oracle.sha256(outcome) if runtime == 'java' else None,
                workspace_before_sha256=compat.digest(compat.encoded(baseline)),
                workspace_after_sha256=compat.digest(compat.encoded(baseline)), private_files_sha256={})
            compat.write_json(folder / 'process.json', process)
            processes[runtime] = process
        results.append(dict(repeat=repeat, processes=processes, **module.comparison(output, repeat)))
    compat.write_json(output / 'partial-results.json', results)
    sources = {name: oracle.sha256(snapshot / name) for name in module.source_paths(snapshot)}
    report = dict(schema=1, adapter=module.ADAPTER, profile=profile, original_case=original,
        original_cli_contract_preserved=True, full_m2_complete=False,
        original_root=str(original_root), output=str(output), workspace=str(workspace), oracle=metadata,
        sources=sources, sources_after=sources,
        artifacts_before={path.relative_to(artifacts).as_posix(): oracle.sha256(path)
                          for path in artifacts.rglob('*') if path.is_file()},
        artifacts_after={path.relative_to(artifacts).as_posix(): oracle.sha256(path)
                         for path in artifacts.rglob('*') if path.is_file()},
        commands=[dict(name='javac', **compile_row)],
        candidate=dict(source_binding='unbound-caller-supplied', original_path='/synthetic/candidate',
                       sha256=oracle.sha256(artifacts / 'jvm_snippet_probe')),
        javac=dict(path=compiler[0], sha256=oracle.sha256(artifacts / 'javac-launcher')),
        results=results, files=module.inventory(output))
    compat.write_json(output / 'report.json', report)
    return report


class OfflineReportTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temporary.name) / 'project'
        cls.output = cls.root / 'target/classpath-synthetic'
        cls.output.mkdir(parents=True)
        cls.original = synthetic_report(cls.output, cls.root)

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def setUp(self):
        self.before = {path: path.read_bytes() for path in self.output.rglob('*') if path.is_file()}
        self.report = copy.deepcopy(self.original)

    def tearDown(self):
        for path in self.output.rglob('*'):
            if path.is_file() and path not in self.before:
                path.unlink()
        for path, data in self.before.items():
            path.write_bytes(data)

    def save(self):
        # Recompute the ordinary inventory to ensure semantic checks, rather
        # than only a stale outer checksum, catch these mutations.
        self.report['files'] = module.inventory(self.output)
        compat.write_json(self.output / 'report.json', self.report)

    def test_offline_verifier_never_executes_or_reads_live_workspace(self):
        with (patch('subprocess.run', side_effect=AssertionError('must not execute')),
              patch('subprocess.Popen', side_effect=AssertionError('must not execute'))):
            report = module.verify_report(self.output)
        self.assertEqual(module.contract_failures(self.output, report), [])
        self.assertFalse(Path(report['workspace']).exists())
        registry_path = module.m2_source_inputs.HISTORICAL
        self.assertIn(registry_path, report['sources'])
        registry = compat.read_json(self.output / 'sources' / registry_path)
        self.assertEqual(len(registry['cases']), 8)
        for resource in registry['resources']:
            self.assertEqual(report['sources'][resource['path']], resource['sha256'])

    def test_unlisted_directory_symlink_is_rejected_without_following_it(self):
        with tempfile.TemporaryDirectory() as temporary:
            external = Path(temporary)
            (external / 'external.bin').write_bytes(b'must not be followed')
            link = self.output / 'unlisted-directory'
            link.symlink_to(external, target_is_directory=True)
            try:
                with self.assertRaisesRegex(ValueError, 'unexpected evidence directory'):
                    module.verify_report(self.output)
            finally:
                link.unlink()

    def test_only_exact_root_build_directory_is_pruned_before_traversal(self):
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            output, external = base / 'evidence', base / 'external'
            output.mkdir()
            external.mkdir()
            (external / 'loop').symlink_to(external, target_is_directory=True)
            (external / 'not-evidence.bin').write_bytes(b'Cargo cache')
            (output / 'build').symlink_to(external, target_is_directory=True)
            with patch.object(module.oracle, 'sha256', side_effect=AssertionError('cache must not be inspected')):
                self.assertEqual(module.inventory(output), {})
            alias = base / 'evidence-alias'
            alias.symlink_to(output, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, 'unexpected evidence directory'):
                module.inventory(alias)
            nested = output / 'nested'
            nested.mkdir()
            (nested / 'build').symlink_to(external, target_is_directory=True)
            with self.assertRaisesRegex(ValueError, 'unexpected evidence directory'):
                module.inventory(output)

    def test_recorded_build_candidate_path_is_bound_to_actual_build_output(self):
        command = module.build_command(Path(self.report['output']))
        row = dict(command=command, exit_code=0, timed_out=False,
                   stdout_sha256=compat.digest(b''), stderr_sha256=compat.digest(b''))
        for channel in ('stdout', 'stderr'):
            (self.output / ('build-native.' + channel + '.bin')).write_bytes(b'')
        compat.write_json(self.output / 'build-native.process.json', row)
        self.report['commands'].insert(0, dict(name='build-native', **row))
        self.report['candidate']['source_binding'] = 'recorded-build'
        self.report['candidate']['original_path'] = str(self.output / 'build/release/examples/jvm_snippet_probe')
        self.save()
        module.verify_report(self.output)
        self.report['candidate']['original_path'] = '/unrelated/path'
        self.save()
        with self.assertRaisesRegex(ValueError, 'recorded native build artifact path'):
            module.verify_report(self.output)

    def test_cargo_config_change_cannot_be_hidden_by_refreshing_outer_inventory(self):
        config = self.output / 'sources/.cargo/config.toml'
        config.write_bytes(config.read_bytes() + b'\n[build]\nrustflags = ["--cfg=unexpected"]\n')
        self.save()
        with self.assertRaisesRegex(ValueError, 'captured implementation source'):
            module.verify_report(self.output)

    def test_profile_original_exit_and_unblocked_inventory_forgery_are_rejected(self):
        for change in ('configuration', 'exit', 'unblocked'):
            self.report = copy.deepcopy(self.original)
            if change == 'configuration':
                self.report['profile']['configuration']['power_assertions'] = False
            elif change == 'exit':
                self.report['original_case']['expected_exit'] = [1]
            else:
                self.report['original_case']['execution'] = dict(status='ready', reason=None)
            self.save()
            with self.subTest(change=change), self.assertRaises(ValueError):
                module.verify_report(self.output)

    def test_rehashed_oracle_runtime_options_and_compiler_identity_are_rejected(self):
        self.report['oracle']['command'].insert(1, '-Xmx2g')
        compat.write_json(self.output / 'oracle.json', self.report['oracle'])
        self.save()
        with self.assertRaisesRegex(ValueError, 'pinned oracle'):
            module.verify_report(self.output)
        for path, data in self.before.items():
            path.write_bytes(data)
        self.report = copy.deepcopy(self.original)
        self.report['javac']['sha256'] = '0' * 64
        self.save()
        with self.assertRaisesRegex(ValueError, 'compiler identity'):
            module.verify_report(self.output)

    def test_rehashed_java_rust_argv_root_or_compiler_tampering_is_rejected(self):
        for runtime, change in [('java', 'argv'), ('rust', 'root'), ('java', 'source')]:
            self.report = copy.deepcopy(self.original)
            row = self.report['results'][0]['processes'][runtime]
            if change == 'argv':
                row['command'].insert(1, '-Dsurprise=true')
            elif change == 'root':
                row['command'][-1] += '-other'
            else:
                row['source_uri'] += '-other'
            compat.write_json(self.output / 'cases/0' / runtime / 'process.json', row)
            compat.write_json(self.output / 'partial-results.json', self.report['results'])
            self.save()
            with self.subTest(runtime=runtime, change=change), self.assertRaisesRegex(ValueError, 'argv/source/classpath'):
                module.verify_report(self.output)
            for path, data in self.before.items():
                path.write_bytes(data)
        self.report = copy.deepcopy(self.original)
        self.report['commands'][0]['command'].append('Unexpected.java')
        self.save()
        with self.assertRaisesRegex(ValueError, 'compiler/build command'):
            module.verify_report(self.output)

    def test_classpath_resource_and_compiled_class_rehashes_fail_binding(self):
        resource = self.output / 'sources' / module.RESOURCE
        resource.write_bytes(resource.read_bytes() + b'\n')
        self.save()
        with self.assertRaises(ValueError):
            module.verify_report(self.output)
        resource.write_bytes(self.before[resource])
        extra = self.output / 'artifacts/java/Unexpected.class'
        extra.write_bytes(b'extra class')
        self.report['artifacts_before']['java/Unexpected.class'] = oracle.sha256(extra)
        self.report['artifacts_after']['java/Unexpected.class'] = oracle.sha256(extra)
        self.save()
        with self.assertRaisesRegex(ValueError, 'compiled class coverage'):
            module.verify_report(self.output)

    def test_artifact_changed_during_execution_cannot_be_forged_into_stable_provenance(self):
        self.report['artifacts_after']['jvm_snippet_probe'] = '0' * 64
        self.save()
        with self.assertRaisesRegex(ValueError, 'artifact stability'):
            module.verify_report(self.output)

    def test_success_outcome_cannot_be_changed_to_internal_failure_and_count_as_pass(self):
        outcome = self.output / 'cases/0/java/outcome.bin'
        outcome.write_bytes(b'oracle-error\torg.pkl.core.PklBugException\n')
        row = self.report['results'][0]['processes']['java']
        row['outcome_sha256'] = oracle.sha256(outcome)
        compat.write_json(self.output / 'cases/0/java/process.json', row)
        compat.write_json(self.output / 'partial-results.json', self.report['results'])
        self.save()
        with self.assertRaisesRegex(ValueError, 'raw comparison/outcome'):
            module.verify_report(self.output)

    def test_raw_whitespace_difference_and_matching_fixture_failure_are_nonpasses(self):
        folder = self.output / 'cases/0/rust'
        captured = compat.load_capture(folder)
        captured['stdout'] = captured['stdout'].strip()
        captured['elapsed_ns'] = 1
        compat.store_capture(folder, captured)
        self.assertEqual(module.comparison(self.output, 0)['status'], 'mismatch')
        for runtime in ('java', 'rust'):
            folder = self.output / 'cases/0' / runtime
            compat.store_capture(folder, dict(stdout=b'', stderr=b'Cannot find module\n', exit_code=1,
                elapsed_ns=1, timed_out=False, files={}, workspace_changes={}))
        (self.output / 'cases/0/java/outcome.bin').write_bytes(b'pkl-exception\torg.pkl.core.PklException\n')
        comparison = module.comparison(self.output, 0)
        self.assertTrue(comparison['raw_channels_match'])
        self.assertFalse(comparison['upstream_expected_exit_match'])
        self.assertEqual(comparison['status'], 'oracle-contract-mismatch')


if __name__ == '__main__':
    unittest.main()
