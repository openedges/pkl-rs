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

    def test_jdk_version_mismatch_rejected_by_default(self):
        lock = {
            "jdk": {
                "properties": {
                    "java.vendor": "Eclipse Adoptium",
                    "java.version": "21.0.12.1",
                    "java.runtime.version": "21.0.12.1+1-LTS",
                    "java.vm.name": "OpenJDK 64-Bit Server VM",
                    "java.vm.version": "21.0.12.1+1-LTS",
                }
            }
        }
        stderr_21_0_13 = (
            b"    java.vendor = Eclipse Adoptium\n"
            b"    java.version = 21.0.13\n"
            b"    java.runtime.version = 21.0.13+9-LTS\n"
            b"    java.vm.name = OpenJDK 64-Bit Server VM\n"
            b"    java.vm.version = 21.0.13+9-LTS\n"
            b"    java.home = /fake/jdk\n"
        )
        identity = subprocess.CompletedProcess([], 0, b"", stderr_21_0_13)
        with patch.dict(oracle.os.environ, {}, clear=True):
            with patch.object(oracle, "_run", return_value=identity):
                with self.assertRaisesRegex(oracle.OracleError, "Java identity mismatch: java.version"):
                    oracle.verify_java(Path("java"), lock, {})

    def test_jdk_version_mismatch_allowed_with_override_and_records_provenance(self):
        jdk_dir = self.root / "fake_jdk"
        jdk_dir.mkdir(parents=True)
        (jdk_dir / "bin").mkdir()
        java_bin = jdk_dir / "bin" / "java"
        java_bin.write_bytes(b"java_binary_content")
        (jdk_dir / "lib").mkdir()
        (jdk_dir / "lib" / "modules").write_bytes(b"modules_content")
        (jdk_dir / "release").write_bytes(b"JAVA_VERSION=\"21.0.13\"\n")

        lock = {
            "jdk": {
                "properties": {
                    "java.vendor": "Eclipse Adoptium",
                    "java.version": "21.0.12.1",
                    "java.runtime.version": "21.0.12.1+1-LTS",
                    "java.vm.name": "OpenJDK 64-Bit Server VM",
                    "java.vm.version": "21.0.12.1+1-LTS",
                }
            }
        }
        stderr_21_0_13 = (
            f"    java.vendor = Eclipse Adoptium\n"
            f"    java.version = 21.0.13\n"
            f"    java.runtime.version = 21.0.13+9-LTS\n"
            f"    java.vm.name = OpenJDK 64-Bit Server VM\n"
            f"    java.vm.version = 21.0.13+9-LTS\n"
            f"    java.home = {jdk_dir}\n"
            f"    os.name = Mac OS X\n"
            f"    os.arch = aarch64\n"
            f"    os.version = 14.5\n"
        ).encode("utf-8")
        identity = subprocess.CompletedProcess([], 0, b"", stderr_21_0_13)

        with patch.dict(oracle.os.environ, {"PKL_ALLOW_LOCAL_JDK": "1"}):
            with patch.object(oracle, "_run", return_value=identity):
                result = oracle.verify_java(java_bin, lock, {})
                self.assertFalse(result["exact_jdk_lock_match"])
                self.assertEqual(result["executable_sha256"], oracle.sha256(java_bin))
                self.assertEqual(result["runtime_modules_sha256"], oracle.sha256(jdk_dir / "lib" / "modules"))
                self.assertEqual(result["release_sha256"], oracle.sha256(jdk_dir / "release"))
                self.assertEqual(result["properties"]["java.version"], "21.0.13")

    def test_jdk_override_still_rejects_non_adoptium_or_major_version(self):
        lock = {
            "jdk": {
                "properties": {
                    "java.vendor": "Eclipse Adoptium",
                    "java.version": "21.0.12.1",
                    "java.runtime.version": "21.0.12.1+1-LTS",
                }
            }
        }
        # Wrong vendor
        stderr_wrong_vendor = (
            b"    java.vendor = Oracle Corporation\n"
            b"    java.version = 21.0.13\n"
            b"    java.runtime.version = 21.0.13+9-LTS\n"
        )
        with patch.dict(oracle.os.environ, {"PKL_ALLOW_LOCAL_JDK": "1"}):
            with patch.object(oracle, "_run", return_value=subprocess.CompletedProcess([], 0, b"", stderr_wrong_vendor)):
                with self.assertRaisesRegex(oracle.OracleError, "Java identity mismatch: java.vendor"):
                    oracle.verify_java(Path("java"), lock, {})

        # Major version difference
        stderr_wrong_major = (
            b"    java.vendor = Eclipse Adoptium\n"
            b"    java.version = 17.0.10\n"
            b"    java.runtime.version = 17.0.10+7\n"
        )
        with patch.dict(oracle.os.environ, {"PKL_ALLOW_LOCAL_JDK": "1"}):
            with patch.object(oracle, "_run", return_value=subprocess.CompletedProcess([], 0, b"", stderr_wrong_major)):
                with self.assertRaisesRegex(oracle.OracleError, "Java identity mismatch: java.version"):
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

    def test_native_image_binary_verification_checks_missing_size_hash_and_exec(self):
        binary = self.root / "pkl-macos-aarch64"
        payload = b"#!/bin/sh\necho 'Pkl 0.32.0 (macOS 26.5, native)'\n"
        binary.write_bytes(payload)
        binary.chmod(0o755)
        spec = {
            "filename": "pkl-macos-aarch64",
            "url": "https://github.com/apple/pkl/releases/download/0.32.0/pkl-macos-aarch64",
            "bytes": len(payload),
            "sha256": oracle.sha256(binary),
            "version_prefix": "Pkl 0.32.0 (",
            "version_suffix": ", native)\n",
        }
        oracle.verify_native_binary(binary, spec, "pkl")

        # Non-executable rejected
        binary.chmod(0o644)
        with self.assertRaisesRegex(oracle.OracleError, "not executable"):
            oracle.verify_native_binary(binary, spec, "pkl")
        binary.chmod(0o755)

        # Same-size checksum tamper rejected
        tampered = bytearray(payload)
        tampered[0] ^= 0xFF
        binary.write_bytes(bytes(tampered))
        with self.assertRaisesRegex(oracle.OracleError, "checksum mismatch"):
            oracle.verify_native_binary(binary, spec, "pkl")

        # Size mismatch rejected
        binary.write_bytes(payload + b"extra")
        with self.assertRaisesRegex(oracle.OracleError, "size mismatch"):
            oracle.verify_native_binary(binary, spec, "pkl")

        # Missing binary rejected
        binary.unlink()
        with self.assertRaisesRegex(oracle.OracleError, "binary missing"):
            oracle.verify_native_binary(binary, spec, "pkl")

    def test_native_image_fetch_checks_download_and_never_overwrites_corrupt(self):
        payload = b"official native-image binary fixture"
        spec = {
            "filename": "pkl-macos-aarch64",
            "url": "https://example.invalid/pkl-macos-aarch64",
            "bytes": len(payload),
            "sha256": oracle.hashlib.sha256(payload).hexdigest(),
        }
        target = self.root / "native-cache/pkl-macos-aarch64"

        # Tampered download rejected before install
        bad_resp = io.BytesIO(payload + b"!")
        bad_resp.geturl = lambda: spec["url"]
        with patch.object(oracle.urllib.request, "urlopen", return_value=bad_resp):
            with self.assertRaisesRegex(oracle.OracleError, "size mismatch"):
                oracle.fetch_native_binary(target, spec, "pkl")
        self.assertFalse(target.exists())
        self.assertEqual(list(target.parent.iterdir()), [])

        # Non-HTTPS redirect rejected
        http_resp = io.BytesIO(payload)
        http_resp.geturl = lambda: "http://insecure.invalid/pkl"
        with patch.object(oracle.urllib.request, "urlopen", return_value=http_resp):
            with self.assertRaisesRegex(oracle.OracleError, "outside HTTPS"):
                oracle.fetch_native_binary(target, spec, "pkl")
        self.assertFalse(target.exists())

        # Valid download installs executable binary
        ok_resp = io.BytesIO(payload)
        ok_resp.geturl = lambda: spec["url"]
        with patch.object(oracle.urllib.request, "urlopen", return_value=ok_resp):
            oracle.fetch_native_binary(target, spec, "pkl")
        self.assertEqual(target.read_bytes(), payload)
        self.assertTrue(oracle.os.access(target, oracle.os.X_OK))

        # Existing corrupt cache is never replaced silently
        target.write_bytes(b"corrupt")
        with patch.object(oracle.urllib.request, "urlopen") as download:
            with self.assertRaisesRegex(oracle.OracleError, "size mismatch"):
                oracle.fetch_native_binary(target, spec, "pkl")
            download.assert_not_called()

    def test_ensure_native_image_oracle_env_override_and_version_validation(self):
        pkl_bin = self.root / "override/pkl-macos-aarch64"
        pkldoc_bin = self.root / "override/pkldoc-macos-aarch64"
        pkl_bin.parent.mkdir(parents=True)
        pkl_bin.write_bytes(b"fake pkl binary")
        pkldoc_bin.write_bytes(b"fake pkldoc binary")
        pkl_bin.chmod(0o755)
        pkldoc_bin.chmod(0o755)

        lock = {
            "schema": 1,
            "version": "0.32.0",
            "source": {"path": "external/pkl", "url": "https://github.com/apple/pkl.git", "revision": "e51fb2f91774265666f94980ec630db794a4db2d", "metadata_sha256": {}},
            "native_image": {
                "distributions": {
                    "Darwin-arm64": {
                        "cache_subdir": "native-image-0.32.0",
                        "pkl": {
                            "filename": "pkl-macos-aarch64",
                            "url": "https://github.com/apple/pkl/releases/download/0.32.0/pkl-macos-aarch64",
                            "bytes": pkl_bin.stat().st_size,
                            "sha256": oracle.sha256(pkl_bin),
                            "version_prefix": "Pkl 0.32.0 (",
                            "version_suffix": ", native)\n",
                        },
                        "pkldoc": {
                            "filename": "pkldoc-macos-aarch64",
                            "url": "https://github.com/apple/pkl/releases/download/0.32.0/pkldoc-macos-aarch64",
                            "bytes": pkldoc_bin.stat().st_size,
                            "sha256": oracle.sha256(pkldoc_bin),
                            "version_prefix": "pkldoc 0.32.0 (",
                            "version_suffix": ", native)\n",
                        },
                    }
                }
            },
        }
        (self.root / "oracle.lock.json").write_text(json.dumps(lock), encoding="utf-8")

        ver_runs = [
            subprocess.CompletedProcess([], 0, b"Pkl 0.32.0 (macOS 26.5, native)\n", b""),
            subprocess.CompletedProcess([], 0, b"pkldoc 0.32.0 (macOS 26.5, native)\n", b""),
        ]
        env_overrides = {
            "PKL_ORACLE_NATIVE_PKL": str(pkl_bin),
            "PKL_ORACLE_NATIVE_PKLDOC": str(pkldoc_bin),
        }
        with (
            patch.object(oracle, "verify_sources"),
            patch.dict(oracle.os.environ, env_overrides, clear=False),
            patch.object(oracle, "_run", side_effect=ver_runs),
        ):
            meta = oracle.ensure_native_image_oracle(self.root, fetch=False, target="Darwin-arm64")
        self.assertTrue(meta["exact_native_image_lock_match"])
        self.assertEqual(meta["command"], [str(pkl_bin.resolve())])
        self.assertEqual(meta["pkldoc_command"], [str(pkldoc_bin.resolve())])
        self.assertEqual(meta["pkl"]["sha256"], oracle.sha256(pkl_bin))
        self.assertEqual(meta["pkldoc"]["sha256"], oracle.sha256(pkldoc_bin))

        # Unpinned target rejected
        with patch.object(oracle, "verify_sources"):
            with self.assertRaisesRegex(oracle.OracleError, "not pinned for Plan9-riscv"):
                oracle.ensure_native_image_oracle(self.root, fetch=False, target="Plan9-riscv")

        # Spoofed JVM version output rejected
        bad_ver = [
            subprocess.CompletedProcess([], 0, b"Pkl 0.32.0 (macOS 26.5, Java 21)\n", b""),
        ]
        with (
            patch.object(oracle, "verify_sources"),
            patch.dict(oracle.os.environ, env_overrides, clear=False),
            patch.object(oracle, "_run", side_effect=bad_ver),
        ):
            with self.assertRaisesRegex(oracle.OracleError, "runtime version check failed"):
                oracle.ensure_native_image_oracle(self.root, fetch=False, target="Darwin-arm64")


if __name__ == "__main__":
    unittest.main()
