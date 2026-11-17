"""Negative tests for byte comparison, corpus accounting, and runner failures."""

import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat


class ComparisonTests(unittest.TestCase):
    def fixture(self):
        return {"stdout": b"a\r\n", "stderr": b"error: \xff\n", "exit_code": 1,
                "files": {"dir/output": b"\x00\xfe"}, "elapsed_ns": 1}

    def test_detects_whitespace_non_utf8_exit_and_file_changes(self):
        expected = self.fixture()
        for field, replacement in (("stdout", b"a\n"), ("stderr", b"error: \xff"),
                                   ("exit_code", 0), ("files", {}),
                                   ("files", {"dir/output": b"\x00\xff"}),
                                   ("files", {"dir/output": b"\x00\xfe", "extra": b""})):
            with self.subTest(field=field, replacement=replacement):
                actual = {**expected, field: replacement}
                self.assertTrue(compat.differences(expected, actual))
        self.assertEqual([], compat.differences(expected, self.fixture()))

    def test_missing_and_corrupt_expectations_fail(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            with self.assertRaises(FileNotFoundError):
                compat.load_capture(path)
            compat.store_capture(path, self.fixture())
            self.assertEqual([], compat.differences(self.fixture(), compat.load_capture(path)))
            (path / "stdout.bin").write_bytes(b"wrong")
            with self.assertRaisesRegex(ValueError, "Corrupt"):
                compat.load_capture(path)
            compat.store_capture(path, self.fixture())
            (path / "files/dir/output").unlink()
            with self.assertRaisesRegex(ValueError, "Missing"):
                compat.load_capture(path)

    def test_empty_partial_duplicate_and_skipped_results_fail(self):
        self.assertTrue(compat.validate_coverage({"cases": []}, {"results": []}, {"passed"}))
        manifest = {"cases": [{"id": "one"}, {"id": "two"}]}
        for results in ([], [{"id": "one", "status": "passed"}],
                        [{"id": "one", "status": "passed"}] * 2,
                        [{"id": "one", "status": "passed"}, {"id": "two", "status": "skipped"}],
                        [{"id": "one", "status": "passed"}, {"id": "two", "status": "blocked"}],
                        [{"id": "one", "status": "passed"}, {"id": "two", "status": "environment_error"}]):
            self.assertTrue(compat.validate_coverage(manifest, {"results": results}, {"passed"}))
        report = {"results": [{"id": "one", "status": "passed"}, {"id": "two", "status": "passed"}]}
        self.assertEqual([], compat.validate_coverage(manifest, report, {"passed"}))

    def test_unavailable_runner_and_timeout_fail(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            with self.assertRaisesRegex(RuntimeError, "unavailable"):
                compat.capture([str(path / "missing")], path, dict(os.environ), 1, path / "files")
            with self.assertRaisesRegex(RuntimeError, "timed out"):
                compat.capture([sys.executable, "-c", "import time; time.sleep(2)"], path,
                               dict(os.environ), 0.02, path / "files")

    def test_real_subprocess_preserves_bytes_and_nonzero_status(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            result = compat.capture([sys.executable, "-c",
                                     "import os,sys;os.write(1,b'x\\r\\n');os.write(2,b'\\xff');sys.exit(7)"],
                                    path, dict(os.environ), 5, path / "files")
            self.assertEqual(b"x\r\n", result["stdout"])
            self.assertEqual(b"\xff", result["stderr"])
            self.assertEqual(7, result["exit_code"])

    def test_rejects_path_escape_duplicate_cases_and_empty_manifest(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp)
            with self.assertRaises(ValueError):
                compat.contained(path, "../escape")
            manifest = path / "manifest.json"
            for cases in ([], [{"id": "same"}] * 2):
                manifest.write_text(json.dumps({"schema_version": 1, "cases": cases}))
                with self.assertRaises(ValueError):
                    compat.load_manifest(manifest)

    def test_candidate_comparison_cannot_pass_missing_case_outputs(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            manifest = {"cases": [{"id": "a", "command": [], "cwd": ".",
                                    "execution": {"status": "ready"}, "expected_exit": [0]}]}
            prefix = [sys.executable, "-c", "print('output')"]
            result = compat.run_cases(manifest, prefix, dict(os.environ), root,
                                      root / "actual", 5, 1, root / "missing")
            self.assertEqual("environment_error", result[0]["status"])


class IntegrityTests(unittest.TestCase):
    def test_workspace_validation_rejects_repo_reference_overlap_and_symlink(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / "external/pkl").mkdir(parents=True)
            sentinel = root / "external/pkl/KEEP"
            sentinel.write_bytes(b"reference")
            for workspace in (root, root / "external/pkl", root / "target"):
                with self.assertRaises(ValueError):
                    compat.prepare_workspace(root, workspace)
            self.assertEqual(sentinel.read_bytes(), b"reference")
            (root / "target").mkdir()
            (root / "target/link").symlink_to(root / "external", target_is_directory=True)
            with self.assertRaisesRegex(ValueError, "symlink"):
                compat.validate_paths(root, root / "target/link/pkl")
            with self.assertRaisesRegex(ValueError, "overlap"):
                compat.validate_paths(root, root / "target/work", root / "target/work/evidence")
            with self.assertRaisesRegex(ValueError, "protected"):
                compat.validate_paths(root, root / "target/work", protected=(root / "target/work/input.json",))
            unowned = root / "target/unowned"
            unowned.mkdir()
            (unowned / "KEEP").write_bytes(b"user data")
            with self.assertRaisesRegex(ValueError, "unowned"):
                compat.prepare_workspace(root, unowned)
            self.assertTrue((unowned / "KEEP").exists())
            owned = root / "target/owned"
            with patch.object(compat, "tracked_paths", return_value=[]):
                compat.prepare_workspace(root, owned)
                (owned / "stale.pkl").write_bytes(b"stale")
                compat.prepare_workspace(root, owned)
            self.assertFalse((owned / "stale.pkl").exists())

    def test_modified_or_missing_source_resource_and_staged_input_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "root"
            staged = Path(temp) / "staged"
            root.mkdir()
            staged.mkdir()
            for path in (root, staged):
                (path / "case.pkl").write_bytes(b"answer = 42\n")
                (path / "resource.bin").write_bytes(b"\x00\xff")
            case = {"id": "input", "execution": {"status": "ready"},
                    "source": {"repository": "pkl-rs", "path": "case.pkl", "sha256": compat.digest(b"answer = 42\n")},
                    "resources": [{"path": "resource.bin", "sha256": compat.digest(b"\x00\xff")} ]}
            manifest = {"cases": [case]}
            self.assertEqual(compat.validate_inputs(root, manifest, staged)["files"], 2)
            (staged / "case.pkl").write_bytes(b"answer = 43\n")
            with self.assertRaisesRegex(ValueError, "checksum"):
                compat.validate_inputs(root, manifest, staged)
            (staged / "case.pkl").write_bytes(b"answer = 42\n")
            (root / "resource.bin").unlink()
            with self.assertRaises(FileNotFoundError):
                compat.validate_inputs(root, manifest, staged)

    def test_check_validates_raw_data_metadata_and_input_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            case = {"id": "case"}
            directory = compat.case_directory("case")
            metadata = compat.store_capture(root / directory, ComparisonTests().fixture())
            compat.write_json(root / "inputs.json", {})
            report = {"schema_version": 1, "mode": "oracle-recording", "manifest_sha256": "manifest",
                      "inputs": {"snapshot_sha256": compat.digest(compat.encoded({}))},
                      "results": [{"id": "case", "directory": directory, "status": "recorded", "result": metadata}]}
            self.assertEqual(compat.check_report({"cases": [case]}, report, root, "manifest"), [])
            (root / directory / "stderr.bin").unlink()
            self.assertTrue(compat.check_report({"cases": [case]}, report, root, "manifest"))
            compat.write_json(root / "inputs.json", {"unexpected": "change"})
            with self.assertRaisesRegex(ValueError, "input evidence"):
                compat.check_report({"cases": [case]}, report, root, "manifest")

    @unittest.skipUnless(os.name == "posix", "POSIX process-group runner")
    def test_timeout_keeps_raw_bytes_and_kills_descendant_with_open_pipes(self):
        import subprocess
        import time
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            generated = root / "generated"
            generated.mkdir()
            code = ("import os,pathlib,subprocess,sys,time;"
                    "p=subprocess.Popen([sys.executable,'-c','import time;time.sleep(60)']);"
                    "pathlib.Path('pid').write_text(str(p.pid));"
                    "pathlib.Path('generated/raw').write_bytes(b'\\x00\\xff');"
                    "os.write(1,b'partial\\r\\n');os.write(2,b'\\xfe');time.sleep(60)")
            before = time.monotonic()
            with self.assertRaises(compat.CaptureTimeout) as caught:
                compat.capture([sys.executable, "-c", code], root, dict(os.environ), 0.2, generated)
            self.assertLess(time.monotonic() - before, 3)
            result = caught.exception.result
            self.assertEqual(result["stdout"], b"partial\r\n")
            self.assertEqual(result["stderr"], b"\xfe")
            self.assertEqual(result["files"], {"raw": b"\x00\xff"})
            self.assertTrue(result["timed_out"])
            try:
                status = subprocess.run(["ps", "-p", (root / "pid").read_text(), "-o", "stat="], capture_output=True).stdout.strip()
                self.assertTrue(not status or status.startswith(b"Z"), status)
            except PermissionError:
                pass

    def test_case_home_tmp_and_java_properties_are_isolated_and_fresh(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            case = {"id": "a", "command": ["eval", "a.pkl"]}
            prefix = ["java", "-Duser.home=/untrusted", "-Djava.io.tmpdir=/untrusted", "Main"]
            command, environment, _, _ = compat.case_execution(case, prefix, {}, root)
            home = Path(environment["HOME"])
            self.assertIn(f"-Duser.home={home}", command)
            self.assertIn(f"-Djava.io.tmpdir={environment['TMPDIR']}", command)
            (home / "stale").write_bytes(b"leak")
            compat.case_execution(case, prefix, {}, root)
            self.assertFalse((home / "stale").exists())
            _, other_environment, _, _ = compat.case_execution({**case, "id": "b"}, prefix, {}, root)
            self.assertNotEqual(environment["HOME"], other_environment["HOME"])

    def test_secondary_writes_are_captured_and_original_inputs_restored(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "root"
            workspace = Path(temp) / "workspace"
            root.mkdir()
            workspace.mkdir()
            for location in (root, workspace):
                (location / "input.pkl").write_bytes(b"original")
            before = compat.snapshot(workspace)
            (workspace / "input.pkl").write_bytes(b"changed")
            (workspace / "new").mkdir()
            (workspace / "new/raw.bin").write_bytes(b"\xff\x00")
            changes, files = compat.restore_changes(root, workspace, before, compat.snapshot(workspace))
            self.assertIn("input.pkl", changes)
            self.assertEqual(files["workspace/new/raw.bin"], b"\xff\x00")
            self.assertEqual((workspace / "input.pkl").read_bytes(), b"original")
            self.assertFalse((workspace / "new").exists())
            self.assertEqual(compat.snapshot(workspace), before)

    def test_comparison_rejects_changed_oracle_before_workspace_mutation(self):
        from unittest.mock import patch
        import oracle
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / "target/expected").mkdir(parents=True)
            (root / "case.pkl").write_bytes(b"answer=1")
            manifest = root / "manifest.json"
            compat.write_json(manifest, {"schema_version": 1, "cases": [{"id": "a", "command": ["eval", "case.pkl"],
                "execution": {"status": "ready"}, "source": {"repository": "pkl-rs", "path": "case.pkl", "sha256": compat.digest(b"answer=1")}}]})
            metadata = {"profile": "reference", "lock_sha256": "current", "artifact_sha256": "artifact", "source": {}, "jdk": {}, "command": [sys.executable], "environment": {}}
            compat.write_json(root / "target/expected/report.json", {"mode": "oracle-recording", "oracle": {**metadata, "lock_sha256": "stale"}})
            arguments = ["compat.py", "compare", "--manifest", str(manifest), "--expected", str(root / "target/expected"),
                         "--runner", str(root / "runner.json"), "--workspace", str(root / "target/work"), "--output", str(root / "target/actual")]
            with patch.object(compat, "ROOT", root), patch.object(sys, "argv", arguments), patch.object(oracle, "ensure_oracle", return_value=metadata), patch.object(compat, "prepare_workspace") as prepare:
                self.assertEqual(compat.main(), 1)
                prepare.assert_not_called()

    def test_restoring_changed_directory_symlink_never_deletes_external_files(self):
        import shutil
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "root"
            workspace = Path(temp) / "workspace"
            outside = Path(temp) / "outside"
            for location in (root / "directory", workspace / "directory", outside):
                location.mkdir(parents=True)
                (location / "KEEP").write_bytes(b"original")
            before = compat.snapshot(workspace)
            shutil.rmtree(workspace / "directory")
            (workspace / "directory").symlink_to(outside, target_is_directory=True)
            compat.restore_changes(root, workspace, before, compat.snapshot(workspace))
            self.assertEqual((outside / "KEEP").read_bytes(), b"original")
            self.assertFalse((workspace / "directory").is_symlink())
            self.assertEqual(compat.snapshot(workspace), before)

    def test_writable_cases_capture_secondary_bytes_and_restore_before_next_case(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "root"
            workspace = Path(temp) / "workspace"
            for path in (root, workspace):
                path.mkdir()
                (path / "input").write_bytes(b"original")
            script = ("import pathlib,sys;"
                      "p=pathlib.Path('input');"
                      "p.write_bytes(b'changed') if sys.argv[1]=='write' else None;"
                      "pathlib.Path('secondary').write_bytes(b'\\x00\\xff') if sys.argv[1]=='write' else None;"
                      "sys.stdout.buffer.write(p.read_bytes())")
            manifest = {"cases": [{"id": mode, "command": [mode], "execution": {"status": "ready"}}
                                  for mode in ("write", "read")]}
            with patch.object(compat, "ROOT", root):
                results = compat.run_cases(manifest, [sys.executable, "-c", script], dict(os.environ),
                                           workspace, Path(temp) / "captures", 5, 4)
            self.assertEqual([r["status"] for r in results], ["recorded", "recorded"])
            first = compat.load_capture(Path(temp) / "captures" / results[0]["directory"])
            second = compat.load_capture(Path(temp) / "captures" / results[1]["directory"])
            self.assertEqual(first["files"]["workspace/secondary"], b"\x00\xff")
            self.assertEqual(second["stdout"], b"original")
            self.assertFalse((workspace / "secondary").exists())

    def test_parallel_eval_mutation_invalidates_batch_and_restores_sources(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "root"
            workspace = Path(temp) / "workspace"
            for path in (root, workspace):
                path.mkdir()
                (path / "input").write_bytes(b"original")
            script = "import pathlib;pathlib.Path('input').write_bytes(b'changed')"
            manifest = {"cases": [{"id": name, "command": ["eval"], "execution": {"status": "ready"}}
                                  for name in ("a", "b")]}
            with patch.object(compat, "ROOT", root):
                results = compat.run_cases(manifest, [sys.executable, "-c", script], dict(os.environ),
                                           workspace, Path(temp) / "captures", 5, 2)
            self.assertEqual([r["status"] for r in results], ["environment_error", "environment_error"])
            self.assertEqual((workspace / "input").read_bytes(), b"original")

    def test_check_revalidates_corrupted_failed_case_capture(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            directory = compat.case_directory("failed")
            metadata = compat.store_capture(root / directory, ComparisonTests().fixture())
            compat.write_json(root / "inputs.json", {})
            manifest = {"cases": [{"id": "failed"}]}
            report = {"schema_version": 1, "mode": "oracle-recording", "manifest_sha256": "manifest",
                      "inputs": {"snapshot_sha256": compat.digest(compat.encoded({}))},
                      "results": [{"id": "failed", "directory": directory,
                                   "status": "unexpected_oracle_exit", "result": metadata}]}
            self.assertEqual(compat.check_report(manifest, report, root, "manifest"),
                             ["failed: unexpected_oracle_exit"])
            (root / directory / "stderr.bin").write_bytes(b"changed failing diagnostic")
            failures = compat.check_report(manifest, report, root, "manifest")
            self.assertEqual(len(failures), 2)
            self.assertIn("Corrupt expected stderr", failures[1])

    def test_source_revision_cannot_bypass_reference_lock_with_changed_manifest(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            (root / "references.lock").write_text("baseline.pkl=0.32.0\npkl.path=external/pkl\npkl.revision=pinned\npkl.url=https://github.com/apple/pkl.git\n")
            manifest = {"baseline": "0.32.0", "cases": [], "sources": [
                {"repository": "pkl", "path": "external/pkl", "revision": "changed",
                 "url": "https://github.com/apple/pkl.git", "resources": []}]}
            with patch.object(compat.subprocess, "check_output") as execute:
                with self.assertRaisesRegex(ValueError, "references.lock"):
                    compat.validate_inputs(root, manifest)
                execute.assert_not_called()


if __name__ == "__main__":
    unittest.main()
