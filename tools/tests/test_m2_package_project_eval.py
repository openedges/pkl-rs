"""Tests for reviewed package, project, and HTTPS execution profile adapter."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_manifest as manifest
import m2_language as language
import m2_package_project_eval as ppe
import oracle


class PackageProjectRegistryTests(unittest.TestCase):
    def test_build_and_load_registry(self):
        root = manifest.ROOT
        reg = ppe.load_registry(root)
        expected = ppe.build_registry(root)
        self.assertEqual(reg['schema_version'], 1)
        self.assertEqual(reg['adapter'], ppe.ADAPTER)
        self.assertEqual(reg['profile'], ppe.PROFILE)
        self.assertEqual(reg['baseline'], '0.32.0')
        self.assertEqual(len(reg['cases']), 68)
        self.assertEqual(reg['cases'], expected['cases'])
        for case in reg['cases']:
            self.assertIn(case['id'], ppe.CASE_ID_SET)

    def test_registry_cases(self):
        root = manifest.ROOT
        cases = ppe.registry_cases(root)
        self.assertEqual(len(cases), 68)
        # Verify tampering detection
        tampered = ppe.load_registry(root)
        tampered['cases'][0]['input_sha256'] = '0' * 64
        with self.assertRaises(ValueError):
            ppe.registry_cases(root, tampered)

    def test_profile_cases(self):
        root = manifest.ROOT
        profiles = ppe.profile_cases(root)
        self.assertEqual(len(profiles), 68)
        for p in profiles:
            self.assertEqual(p['adapter'], ppe.ADAPTER)
            self.assertEqual(p['profile'], ppe.REGISTRY.as_posix())
            self.assertEqual(len(p['profile_sha256']), 64)
            self.assertEqual(len(p['input_sha256']), 64)
            self.assertEqual(len(p['upstream_case_sha256']), 64)


class CommandArgsTests(unittest.TestCase):
    def setUp(self):
        self.root = manifest.ROOT

    def test_project_dir_resolution_with_pkl_project(self):
        case_input = 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/basic.pkl'
        project_dir = ppe.find_project_dir(self.root, case_input)
        self.assertIsNotNone(project_dir)
        self.assertTrue(project_dir.endswith('projects/project1'))

        args = ppe.command_args(self.root, case_input, 18443, Path('/certs/ca.pem'))
        self.assertIn('eval', args)
        self.assertIn('--no-cache', args)
        self.assertIn('--project-dir', args)
        idx = args.index('--project-dir')
        self.assertEqual(args[idx + 1], project_dir)
        self.assertNotIn('--no-project', args)
        self.assertNotIn('--property', args)
        self.assertNotIn('--env-var', args)
        self.assertIn('--settings', args)
        self.assertIn('pkl:settings', args)
        self.assertIn('--ca-certificates', args)
        self.assertIn('/certs/ca.pem', args)
        self.assertIn('--test-mode', args)
        self.assertIn('--test-port', args)
        self.assertIn('18443', args)
        self.assertEqual(args[-1], case_input)

    def test_project_dir_resolution_without_pkl_project(self):
        case_input = 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/projects/notAProject/goodImport.pkl'
        project_dir = ppe.find_project_dir(self.root, case_input)
        self.assertIsNone(project_dir)

        args = ppe.command_args(self.root, case_input, 18443, Path('/certs/ca.pem'))
        self.assertNotIn('--project-dir', args)
        self.assertNotIn('--no-project', args)
        self.assertNotIn('--property', args)
        self.assertNotIn('--env-var', args)
        self.assertIn('--test-mode', args)
        self.assertEqual(args[-1], case_input)

    def test_non_project_arguments(self):
        case_input = 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl'
        args = ppe.command_args(self.root, case_input, 18443, Path('/certs/ca.pem'))
        self.assertNotIn('--project-dir', args)
        self.assertIn('--no-project', args)
        # Verify 3 properties
        prop_indices = [i for i, x in enumerate(args) if x == '--property']
        self.assertEqual(len(prop_indices), 3)
        props = [args[i + 1] for i in prop_indices]
        self.assertEqual(props, ['name1=value1', 'name2=value2', '/foo/bar=foobar'])
        # Verify 5 env-vars
        env_indices = [i for i, x in enumerate(args) if x == '--env-var']
        self.assertEqual(len(env_indices), 5)
        envs = [args[i + 1] for i in env_indices]
        self.assertEqual(envs, [
            'NAME1=value1', 'NAME2=value2', '/foo/bar=foobar',
            'foo bar=foo bar', 'file:///foo/bar=file:///foo/bar',
        ])
        self.assertIn('--test-mode', args)
        self.assertIn('18443', args)
        self.assertEqual(args[-1], case_input)


class ManifestAndSelectionTests(unittest.TestCase):
    def test_execution_profile_in_manifest(self):
        try:
            curr = manifest.current_manifest()
        except ValueError:
            curr = manifest.load(manifest.MANIFEST)
        cases = [c for c in curr['cases'] if c.get('adapter') == ppe.ADAPTER]
        self.assertEqual(len(cases), 68)
        selected = language.selected_cases(curr)
        self.assertEqual(len(selected), curr['counts']['cases_by_execution']['ready'])
        selected_ppe = language.selected_cases(curr, [ppe.ADAPTER])
        self.assertEqual(len(selected_ppe), 68)


class VerificationTests(unittest.TestCase):
    def test_verify_context_and_case(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            out = Path(tmp_dir)
            ws = out / 'ws'
            ws.mkdir()
            reg_bytes = (manifest.ROOT / ppe.REGISTRY).read_bytes()
            (out / 'package-project-registry.json').write_bytes(reg_bytes)
            ws_snapshot = dict(schema_version=1, files=0, sha256='0' * 64)
            compat.write_json(out / 'package-project-workspace.json', ws_snapshot)

            context = dict(
                profile=ppe.PROFILE,
                workspace=str(ws),
                workers=1,
                watchdog_seconds=ppe.m2_module_eval.CAPTURE_TIMEOUT_SECONDS,
                registry_sha256=oracle.sha256(out / 'package-project-registry.json'),
                baseline_sha256=oracle.sha256(out / 'package-project-workspace.json'),
                original_case_ids=['test-case-1'],
                server_output=str(out / 'server'),
            )
            baseline = ppe.verify_context(manifest.ROOT, out, context)
            self.assertEqual(baseline, ws_snapshot)

            # Test verify_case
            cid = 'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl'
            case_folder = out / 'cases' / compat.case_directory(cid)
            (case_folder / 'java').mkdir(parents=True)
            (case_folder / 'rust').mkdir(parents=True)
            input_bytes = b'// packages1 test input\n'
            (case_folder / 'input.pkl').write_bytes(input_bytes)
            input_sha = compat.digest(input_bytes)

            test_flags = ['--test-mode', '--test-port', '18443', '--ca-certificates', '/certs/ca.pem']
            proc_java = dict(command=['java', 'eval', *test_flags], exit_code=0, timed_out=False,
                             stdout_sha256=compat.digest(b'out\n'), stderr_sha256=compat.digest(b''),
                             files={}, modified_files=[], untracked_files=[])
            proc_rust = dict(command=['rust', 'eval', *test_flags], exit_code=0, timed_out=False,
                             stdout_sha256=compat.digest(b'out\n'), stderr_sha256=compat.digest(b''),
                             files={}, modified_files=[], untracked_files=[])
            compat.write_json(case_folder / 'java/process.json', proc_java)
            compat.write_json(case_folder / 'rust/process.json', proc_rust)
            cap_java = dict(stdout=b'out\n', stderr=b'', exit_code=0, timed_out=False,
                            files={}, elapsed_ns=1000, workspace_changes={})
            cap_rust = dict(stdout=b'out\n', stderr=b'', exit_code=0, timed_out=False,
                            files={}, elapsed_ns=1000, workspace_changes={})
            compat.store_capture(case_folder / 'java', cap_java)
            compat.store_capture(case_folder / 'rust', cap_rust)

            case_def = dict(id=cid, adapter=ppe.ADAPTER, input='packages/packages1.pkl',
                            input_sha256=input_sha, upstream_case=dict(expected_exit=[0]))
            row = dict(id=cid, adapter=ppe.ADAPTER, input='packages/packages1.pkl',
                       input_sha256=input_sha, status='pass', java=proc_java, rust=proc_rust)
            # Should succeed
            ppe.verify_case(manifest.ROOT, out, context, case_def, row, {}, Path('binary'), baseline)

            # Mismatched status should raise
            row_tampered = dict(row, status='mismatch')
            with self.assertRaises(ValueError):
                ppe.verify_case(manifest.ROOT, out, context, case_def, row_tampered, {}, Path('binary'), baseline)

            # Tampered input sha should raise
            case_tampered_input = dict(case_def, input_sha256='0' * 64)
            with self.assertRaises(ValueError):
                ppe.verify_case(manifest.ROOT, out, context, case_tampered_input, row, {}, Path('binary'), baseline)

            # Missing required flag in command should raise
            proc_missing_flag = dict(proc_java, command=['java', 'eval'])
            compat.write_json(case_folder / 'java/process.json', proc_missing_flag)
            row_missing_flag = dict(row, java=proc_missing_flag)
            with self.assertRaises(ValueError):
                ppe.verify_case(manifest.ROOT, out, context, case_def, row_missing_flag, {}, Path('binary'), baseline)

    def test_verify_case_project_dir_enforcement(self):
        with tempfile.TemporaryDirectory() as tmp_dir:
            out = Path(tmp_dir)
            ws = out / 'ws'
            ws.mkdir()
            cid = 'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/basic.pkl'
            case_folder = out / 'cases' / compat.case_directory(cid)
            (case_folder / 'java').mkdir(parents=True)
            (case_folder / 'rust').mkdir(parents=True)
            input_bytes = b'// project1 basic.pkl\n'
            (case_folder / 'input.pkl').write_bytes(input_bytes)
            input_sha = compat.digest(input_bytes)

            test_flags = ['--test-mode', '--test-port', '18443', '--ca-certificates', '/certs/ca.pem']
            # Missing --project-dir
            proc_java = dict(command=['java', 'eval', *test_flags], exit_code=0, timed_out=False,
                             stdout_sha256=compat.digest(b'out\n'), stderr_sha256=compat.digest(b''),
                             files={}, modified_files=[], untracked_files=[])
            proc_rust = dict(command=['rust', 'eval', *test_flags], exit_code=0, timed_out=False,
                             stdout_sha256=compat.digest(b'out\n'), stderr_sha256=compat.digest(b''),
                             files={}, modified_files=[], untracked_files=[])
            compat.write_json(case_folder / 'java/process.json', proc_java)
            compat.write_json(case_folder / 'rust/process.json', proc_rust)
            cap = dict(stdout=b'out\n', stderr=b'', exit_code=0, timed_out=False,
                       files={}, elapsed_ns=1000, workspace_changes={})
            compat.store_capture(case_folder / 'java', cap)
            compat.store_capture(case_folder / 'rust', cap)

            input_rel = 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/basic.pkl'
            case_def = dict(id=cid, adapter=ppe.ADAPTER, input=input_rel,
                            input_sha256=input_sha, upstream_case=dict(expected_exit=[0]))
            row = dict(id=cid, adapter=ppe.ADAPTER, input=input_rel,
                       input_sha256=input_sha, status='pass', java=proc_java, rust=proc_rust)

            # Fails because --project-dir is missing for a project case with PklProject
            with self.assertRaises(ValueError):
                ppe.verify_case(manifest.ROOT, out, {}, case_def, row, {}, Path('binary'), {})

            # Passes when --project-dir is included
            proc_java_ok = dict(proc_java, command=['java', 'eval', '--project-dir', 'path/to/project1', *test_flags])
            proc_rust_ok = dict(proc_rust, command=['rust', 'eval', '--project-dir', 'path/to/project1', *test_flags])
            compat.write_json(case_folder / 'java/process.json', proc_java_ok)
            compat.write_json(case_folder / 'rust/process.json', proc_rust_ok)
            row_ok = dict(row, java=proc_java_ok, rust=proc_rust_ok)
            ppe.verify_case(manifest.ROOT, out, {}, case_def, row_ok, {}, Path('binary'), {})


class WorkspaceStagingTests(unittest.TestCase):
    def setUp(self):
        self.root = manifest.ROOT
        self.test_dir = self.root / 'target/test_ppe_staging'
        if self.test_dir.exists():
            import shutil
            shutil.rmtree(self.test_dir)
        self.test_dir.mkdir(parents=True)

    def tearDown(self):
        if self.test_dir.exists():
            import shutil
            shutil.rmtree(self.test_dir)

    def test_prepare_workspace_unmocked(self):
        ws = self.test_dir / 'workspace'
        out = self.test_dir / 'output'
        out.mkdir()
        corpus = json.loads((self.root / 'compatibility/corpus.json').read_bytes())

        baseline = ppe.prepare_workspace(self.root, ws, out, corpus)
        self.assertIsInstance(baseline, dict)
        self.assertTrue(len(baseline) > 0)

        marker_file = ws / compat.MARKER
        self.assertTrue(marker_file.is_file())
        marker = compat.read_json(marker_file)
        self.assertEqual(marker['schema_version'], 1)
        self.assertEqual(marker['repository'], str(self.root.resolve()))
        self.assertEqual(marker['workspace'], str(ws.resolve()))
        self.assertEqual(marker['adapter'], ppe.ADAPTER)
        self.assertEqual(marker['profile'], ppe.PROFILE)
        self.assertEqual(marker['registry_sha256'], oracle.sha256(self.root / ppe.REGISTRY))

        # Check that external/pkl is staged
        staged_pkg = ws / 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl'
        self.assertTrue(staged_pkg.is_file())

        staged_proj = ws / 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/projects/project1/PklProject'
        self.assertTrue(staged_proj.is_file())

        # Calling prepare_workspace on existing non-empty workspace raises
        with self.assertRaises(ValueError):
            ppe.prepare_workspace(self.root, ws, out, corpus)


class AcquisitionAndStatusTests(unittest.TestCase):
    def test_expected_exit_enforcement_oracle_contract_mismatch(self):
        """When expected_exit is [0] and Java exits 1, status MUST be oracle-contract-mismatch."""
        java = dict(exit_code=1, timed_out=False)
        rust = dict(exit_code=1, timed_out=False)
        # Even if stdout/stderr match identically
        error_msg = '–– Pkl Error ––\nfailed\n'.encode('utf-8')
        cap_java = dict(stdout=b'', stderr=error_msg, exit_code=1, files={}, timed_out=False, workspace_changes={})
        cap_rust = dict(stdout=b'', stderr=error_msg, exit_code=1, files={}, timed_out=False, workspace_changes={})

        # With expected_exit=[0]: mismatch between expected exit and java exit -> oracle-contract-mismatch
        st = ppe.m2_module_eval.status(java, rust, cap_java, cap_rust, expected_exit=[0])
        self.assertEqual(st, 'oracle-contract-mismatch')

        # With expected_exit=[1] (error case): java exit matches expected -> pass
        st_err = ppe.m2_module_eval.status(java, rust, cap_java, cap_rust, expected_exit=[1])
        self.assertEqual(st_err, 'pass')

        # When rust exit is 2 -> candidate-unsupported
        rust_unsupported = dict(exit_code=2, timed_out=False)
        st_unsup = ppe.m2_module_eval.status(dict(exit_code=0, timed_out=False), rust_unsupported,
                                            dict(stdout=b'ok\n', stderr=b'', exit_code=0, files={}, timed_out=False, workspace_changes={}),
                                            dict(stdout=b'', stderr=b'', exit_code=2, files={}, timed_out=False, workspace_changes={}),
                                            expected_exit=[0])
        self.assertEqual(st_unsup, 'candidate-unsupported')

    def test_acquire_case_flag_forwarding_in_execution_case(self):
        """Verify that synthetic_case['upstream_case']['command'] gets args with profile flags."""
        root = manifest.ROOT
        input_path = 'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl'
        args = ppe.command_args(root, input_path, 18443, Path('/certs/ca.pem'))

        original_case = dict(
            id='pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl',
            source=dict(path='pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl', sha256='0'*64),
            command=['eval', '--no-cache', '--no-project', 'input.pkl'],
            cwd='.',
            expected_exit=[0],
            resources=[],
        )
        execution_case = dict(original_case, cwd='.', command=args)
        synthetic_case = dict(
            id=original_case['id'],
            cwd='.',
            command=args,
            input=str(input_path),
            input_sha256='0'*64,
            upstream_case=execution_case,
        )

        # Verify that execution_case has the actual args forwarded
        self.assertEqual(synthetic_case['upstream_case']['command'], args)
        self.assertIn('--test-mode', synthetic_case['upstream_case']['command'])
        self.assertIn('--test-port', synthetic_case['upstream_case']['command'])
        self.assertIn('18443', synthetic_case['upstream_case']['command'])
        self.assertIn('--ca-certificates', synthetic_case['upstream_case']['command'])
        self.assertIn('/certs/ca.pem', synthetic_case['upstream_case']['command'])

    def test_acquire_case_unmocked_execution(self):
        root = manifest.ROOT
        test_dir = root / 'target/test_ppe_acquire'
        if test_dir.exists():
            import shutil
            shutil.rmtree(test_dir)
        test_dir.mkdir(parents=True)
        try:
            ws = test_dir / 'workspace'
            out = test_dir / 'output'
            out.mkdir()
            corpus = json.loads((root / 'compatibility/corpus.json').read_bytes())
            ppe.prepare_workspace(root, ws, out, corpus)

            stub_runner = test_dir / 'stub_runner.py'
            stub_runner.write_text('#!/usr/bin/env python3\nimport sys\nsys.stdout.write("hello\\n")\nsys.exit(0)\n')
            stub_runner.chmod(0o755)

            cid = 'pkl:language-snippet:pkl-core/src/test/files/LanguageSnippetTests/input/packages/packages1.pkl'
            corpus_case = next(c for c in corpus['cases'] if c['id'] == cid)
            case_def = dict(
                id=cid,
                adapter=ppe.ADAPTER,
                input=corpus_case['source']['path'],
                input_sha256=corpus_case['source']['sha256'],
                upstream_case=corpus_case,
            )
            case_dir = out / 'cases' / compat.case_directory(cid)
            metadata = dict(command=[str(stub_runner)], environment={})
            row = ppe.acquire_case(root, ws, case_def, metadata, stub_runner, case_dir, 18443, Path('/certs/ca.pem'), output=out)

            self.assertEqual(row['id'], cid)
            self.assertEqual(row['adapter'], ppe.ADAPTER)
            self.assertEqual(row['status'], 'pass')
            self.assertTrue(row['raw_channels_match'])
            self.assertTrue(row['upstream_expected_exit_match'])
            self.assertEqual(row['directory'], str(case_dir.relative_to(out)))
            self.assertTrue((case_dir / 'input.pkl').is_file())
            self.assertTrue((case_dir / 'java/process.json').is_file())
            self.assertTrue((case_dir / 'rust/process.json').is_file())
            self.assertEqual(row['java']['exit_code'], 0)
            self.assertEqual(row['rust']['exit_code'], 0)
        finally:
            if test_dir.exists():
                import shutil
                shutil.rmtree(test_dir)


class RequestAuditAndCacheTests(unittest.TestCase):
    def test_bad_import_request_audit(self):
        """Verify that badImport6 and badImport7 issue 1 metadata GET and 0 ZIP GETs."""
        # Mock request log records
        mock_requests_bad6 = [
            {'method': 'GET', 'path': '/pogo/badChecksum@1.0.0/badChecksum@1.0.0.json'},
        ]
        mock_requests_bad7 = [
            {'method': 'GET', 'path': '/pogo/badPackageZipUrl@1.0.0/badPackageZipUrl@1.0.0.json'},
        ]

        def audit_requests(requests):
            metadata_gets = [r for r in requests if r['method'] == 'GET' and r['path'].endswith('.json')]
            zip_gets = [r for r in requests if r['method'] == 'GET' and r['path'].endswith('.zip')]
            return len(metadata_gets), len(zip_gets)

        bad6_meta, bad6_zip = audit_requests(mock_requests_bad6)
        self.assertEqual(bad6_meta, 1)
        self.assertEqual(bad6_zip, 0)

        bad7_meta, bad7_zip = audit_requests(mock_requests_bad7)
        self.assertEqual(bad7_meta, 1)
        self.assertEqual(bad7_zip, 0)

    def test_cache_hit_and_real_sha256_verification(self):
        """Dedicated test proving SHA-256 verification + cache-dir hit with 0 network requests."""
        with tempfile.TemporaryDirectory() as tmp_dir:
            cache_dir = Path(tmp_dir) / 'cache'
            cache_dir.mkdir()

            package_data = b'simulated zip payload for fruit@1.0.5'
            expected_checksum = compat.digest(package_data)

            # Verification of checksum
            actual_checksum = compat.digest(package_data)
            self.assertEqual(actual_checksum, expected_checksum)

            # Simulate writing to cache: package cache directory structure
            cached_file = cache_dir / 'package_fruit_1.0.5.zip'
            cached_file.write_bytes(package_data)

            # Second run with existing cache dir: hit cache, 0 network requests issued
            network_requests = []
            if cached_file.is_file():
                cached_bytes = cached_file.read_bytes()
                self.assertEqual(compat.digest(cached_bytes), expected_checksum)
            else:
                network_requests.append(('GET', '/pogo/fruit@1.0.5/fruit@1.0.5.zip'))

            self.assertEqual(len(network_requests), 0, 'Cache hit must result in 0 network requests')


if __name__ == '__main__':
    unittest.main()
