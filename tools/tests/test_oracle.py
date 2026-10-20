"""Regression tests for rejecting untrusted or unreproducible oracle inputs."""

import importlib.util
import io
import json
from pathlib import Path
import subprocess
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

MODULE_PATH = Path(__file__).resolve().parents[1] / "oracle.py"
SPEC = importlib.util.spec_from_file_location("oracle", MODULE_PATH)
oracle = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(oracle)


class OracleTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.artifact = self.root / "oracle.jar"
        self.lock = {"version": "0.32.0", "artifact": {"commit_id": "e51fb2f"}}
        self.write_archive("0.32.0")

    def write_archive(self, version):
        with zipfile.ZipFile(self.artifact, "w") as archive:
            archive.writestr("org/pkl/core/Release.properties", f"version={version}\ncommitId=e51fb2f\n")
            for entry in ("org/pkl/cli/Main.class", "org/pkl/core/Evaluator.class", "org/pkl/server/Server.class"):
                archive.writestr(entry, b"fixture; not executable")
        self.lock["artifact"]["sha256"] = oracle.sha256(self.artifact)

    def test_missing_artifact_is_not_a_pass(self):
        self.artifact.unlink()
        with self.assertRaisesRegex(oracle.OracleError, "artifact missing"):
            oracle.verify_artifact(self.artifact, self.lock)

    def test_artifact_checksum_tamper_is_rejected(self):
        with self.artifact.open("ab") as stream:
            stream.write(b"tamper")
        with self.assertRaisesRegex(oracle.OracleError, "checksum mismatch"):
            oracle.verify_artifact(self.artifact, self.lock)

    def test_valid_checksum_does_not_hide_wrong_embedded_version(self):
        self.write_archive("0.32.1")
        with self.assertRaisesRegex(oracle.OracleError, "release metadata"):
            oracle.verify_artifact(self.artifact, self.lock)

    def test_valid_checksum_does_not_hide_wrong_source_commit(self):
        self.lock["artifact"]["commit_id"] = "1111111"
        with self.assertRaisesRegex(oracle.OracleError, "release metadata"):
            oracle.verify_artifact(self.artifact, self.lock)

    def test_fetch_checks_download_before_installing(self):
        self.lock["artifact"]["url"] = "https://example.invalid/pinned.jar"
        target = self.root / "cache/oracle.jar"
        response = io.BytesIO(self.artifact.read_bytes() + b"tamper")
        response.geturl = lambda: self.lock["artifact"]["url"]
        with patch.object(oracle.urllib.request, "urlopen", return_value=response):
            with self.assertRaisesRegex(oracle.OracleError, "checksum mismatch"):
                oracle.fetch_artifact(target, self.lock)
        self.assertFalse(target.exists())
        self.assertEqual(list(target.parent.iterdir()), [])

    def test_fetch_never_replaces_existing_corrupt_cache(self):
        self.artifact.write_bytes(b"corrupt")
        with patch.object(oracle.urllib.request, "urlopen") as download:
            with self.assertRaisesRegex(oracle.OracleError, "checksum mismatch"):
                oracle.fetch_artifact(self.artifact, self.lock)
            download.assert_not_called()
        self.assertEqual(self.artifact.read_bytes(), b"corrupt")

    def test_environment_excludes_secrets_and_java_option_injection(self):
        with patch.dict(oracle.os.environ, {"AWS_SECRET_ACCESS_KEY": "secret", "JAVA_TOOL_OPTIONS": "-Dtest=injected", "JDK_JAVA_OPTIONS": "-Xmx9g", "CLASSPATH": "untrusted"}):
            environment = oracle.deterministic_environment(self.root)
        self.assertFalse(any(key in environment for key in ("AWS_SECRET_ACCESS_KEY", "JAVA_TOOL_OPTIONS", "JDK_JAVA_OPTIONS", "CLASSPATH")))
        self.assertEqual(environment["TZ"], "UTC")
        self.assertEqual(environment["LC_ALL"], "C")
        self.assertTrue(Path(environment["HOME"]).is_dir())

    def test_different_java_build_is_rejected(self):
        lock = {"jdk": {"properties": {"java.runtime.version": "21.0.12.1+1-LTS"}}}
        identity = subprocess.CompletedProcess([], 0, b"", b"    java.runtime.version = 21.0.11+1-LTS\n")
        with patch.object(oracle, "_run", return_value=identity):
            with self.assertRaisesRegex(oracle.OracleError, "Java identity mismatch"):
                oracle.verify_java(Path("java"), lock, {})

    def test_performance_runtime_tamper_is_rejected(self):
        cache = self.root / ".cache/oracle"
        cache.mkdir(parents=True)
        archive = cache / "jdk.tar.gz"
        archive.write_bytes(b"pinned archive fixture")
        java = cache / "jdk/bin/java"
        java.parent.mkdir(parents=True)
        java.write_bytes(b"changed launcher")
        distribution = {"filename": archive.name, "sha256": oracle.sha256(archive),
                        "java_path": "jdk/bin/java", "runtime_sha256": {"bin/java": "0" * 64}}
        target = f"{oracle.platform.system()}-{oracle.platform.machine()}"
        lock = {"performance_jdk": {"distributions": {target: distribution}}}
        with self.assertRaisesRegex(oracle.OracleError, "runtime checksum mismatch"):
            oracle.ensure_performance_jdk(self.root, lock, False)

    def test_performance_archive_path_traversal_is_rejected(self):
        cache = self.root / ".cache/oracle"
        cache.mkdir(parents=True)
        archive = cache / "jdk.tar.gz"
        with tarfile.open(archive, "w:gz") as bundle:
            entry = tarfile.TarInfo("../escape")
            entry.size = 6
            bundle.addfile(entry, io.BytesIO(b"unsafe"))
        distribution = {"filename": archive.name, "sha256": oracle.sha256(archive),
                        "java_path": "jdk/bin/java", "runtime_sha256": {}}
        target = f"{oracle.platform.system()}-{oracle.platform.machine()}"
        lock = {"performance_jdk": {"distributions": {target: distribution}}}
        with self.assertRaisesRegex(oracle.OracleError, "unsafe performance JDK archive path"):
            oracle.ensure_performance_jdk(self.root, lock, True)
        self.assertFalse((cache.parent / "escape").exists())

    def test_reference_lock_mismatch_is_rejected_before_execution(self):
        lock = {"version": "0.32.0", "source": {"path": "external/pkl", "revision": "1234", "url": "https://github.com/apple/pkl.git"}}
        (self.root / "references.lock").write_text("baseline.pkl=0.32.1\n")
        with patch.object(oracle, "_run") as execute:
            with self.assertRaisesRegex(oracle.OracleError, "references.lock disagrees"):
                oracle.verify_sources(self.root, lock)
            execute.assert_not_called()

    def test_duplicate_reference_metadata_is_rejected(self):
        with self.assertRaisesRegex(oracle.OracleError, "duplicate metadata key"):
            oracle.read_properties("version=0.32.0\nversion=0.32.1\n")

    def test_smoke_compares_raw_replay_bytes(self):
        metadata = {"command": ["java"], "environment": {}}
        executions = [
            subprocess.CompletedProcess([], 0, b'answer = 42\nmessage = "oracle"\n', b""),
            subprocess.CompletedProcess([], 0, b'answer = 42\nmessage = "oracle"\n ', b""),
            subprocess.CompletedProcess([], 1, b"", b"error\n"),
            subprocess.CompletedProcess([], 1, b"", b"error\n"),
        ]
        with patch.object(oracle, "_run", side_effect=executions):
            with self.assertRaisesRegex(oracle.OracleError, "smoke failed"):
                oracle.smoke(self.root, metadata)
        report = json.loads((self.root / "target/m0/oracle/smoke.json").read_text())
        self.assertFalse(report["results"][0]["replay_equal"])
        self.assertFalse(report["results"][0]["passed"])
        self.assertEqual((self.root / "target/m0/oracle/error.stderr.bin").read_bytes(), b"error\n")


if __name__ == "__main__":
    unittest.main()
