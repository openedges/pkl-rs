"""Unit & live IPC tests for M4 external reader fixtures, MessagePack codec, offline Pantry staging, and server compilation."""
from __future__ import annotations

import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "tools"))
sys.path.insert(0, str(ROOT / "tests/fixtures/m4/readers"))

import m4_differential
import m4_pantry_offline as pantry_offline
import msgpack_ipc as mp
import oracle


class M4ServersAndReadersTests(unittest.TestCase):
    def test_msgpack_roundtrip_all_13_codes_and_key_omission(self) -> None:
        samples = [
            (mp.READ_RESOURCE_REQUEST, {"requestId": -5127070350816258203, "evaluatorId": 0, "uri": "extres:///a.txt"}),
            (mp.READ_RESOURCE_RESPONSE, {"requestId": 1, "evaluatorId": 0, "contents": b"\x00\xff", "error": None}),
            (mp.READ_MODULE_REQUEST, {"requestId": 2**62, "evaluatorId": 7, "uri": "extmod:///root.pkl"}),
            (mp.READ_MODULE_RESPONSE, {"requestId": 2**62, "evaluatorId": 7, "contents": "x = 1", "error": None}),
            (mp.LIST_RESOURCES_REQUEST, {"requestId": -1, "evaluatorId": 0, "uri": "extres:/items/"}),
            (mp.LIST_RESOURCES_RESPONSE, {"requestId": -1, "evaluatorId": 0, "pathElements": [{"name": "a.txt", "isDirectory": False}]}),
            (mp.LIST_MODULES_REQUEST, {"requestId": 42, "evaluatorId": 0, "uri": "extmod:/glob/"}),
            (mp.LIST_MODULES_RESPONSE, {"requestId": 42, "evaluatorId": 0, "pathElements": []}),
            (mp.INITIALIZE_MODULE_READER_REQUEST, {"requestId": 100, "scheme": "extmod"}),
            (mp.INITIALIZE_MODULE_READER_RESPONSE, {"requestId": 100, "spec": None}),
            (mp.INITIALIZE_RESOURCE_READER_REQUEST, {"requestId": 101, "scheme": "extres"}),
            (mp.INITIALIZE_RESOURCE_READER_RESPONSE, {"requestId": 101, "spec": {"scheme": "extres", "hasHierarchicalUris": True, "isGlobbable": True}}),
            (mp.CLOSE_EXTERNAL_PROCESS, {}),
        ]
        buf = io.BytesIO()
        for code, body in samples:
            mp.write_message(buf, code, body)
        buf.seek(0)
        for expected_code, original_body in samples:
            decoded = mp.read_message(buf)
            self.assertIsNotNone(decoded)
            got_code, got_body = decoded
            self.assertEqual(got_code, expected_code)
            # Optional keys with None MUST be omitted (never encoded as nil)
            expected_body = {k: v for k, v in original_body.items() if v is not None}
            self.assertEqual(got_body, expected_body)
            self.assertNotIn(None, got_body.values())
        self.assertIsNone(mp.read_message(buf))

    def test_external_reader_live_ipc_and_lifecycle(self) -> None:
        reader = ROOT / m4_differential.READER_BIN
        with tempfile.TemporaryDirectory() as td:
            log_path = Path(td) / "life.jsonl"
            proc = subprocess.Popen(
                [sys.executable, str(reader), "--lifecycle-log", str(log_path), "--tag", "unit-test", "extra1"],
                stdin=subprocess.PIPE,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            assert proc.stdin is not None and proc.stdout is not None
            try:
                # 1. Initialize known module scheme
                mp.write_message(proc.stdin, mp.INITIALIZE_MODULE_READER_REQUEST, {"requestId": -99, "scheme": "extmod"})
                code, body = mp.read_message(proc.stdout)
                self.assertEqual(code, mp.INITIALIZE_MODULE_READER_RESPONSE)
                self.assertEqual(body["requestId"], -99)
                self.assertEqual(body["spec"]["scheme"], "extmod")

                # 2. Initialize unknown scheme -> spec omitted
                mp.write_message(proc.stdin, mp.INITIALIZE_MODULE_READER_REQUEST, {"requestId": 2, "scheme": "unknown"})
                code, body = mp.read_message(proc.stdout)
                self.assertEqual(code, mp.INITIALIZE_MODULE_READER_RESPONSE)
                self.assertNotIn("spec", body)

                # 3. Missing resource omits BOTH contents and error (Pkl 0.32.0 Missing quirk)
                mp.write_message(proc.stdin, mp.READ_RESOURCE_REQUEST, {"requestId": 3, "evaluatorId": 0, "uri": "extres:///missing.txt"})
                code, body = mp.read_message(proc.stdout)
                self.assertEqual(code, mp.READ_RESOURCE_RESPONSE)
                self.assertNotIn("contents", body)
                self.assertNotIn("error", body)

                # 4. Graceful close via 0x32
                mp.write_message(proc.stdin, mp.CLOSE_EXTERNAL_PROCESS, {})
                proc.wait(timeout=5)
                self.assertEqual(proc.returncode, 0)
            finally:
                if proc.poll() is None:
                    proc.kill()
                    proc.wait(timeout=5)
                proc.stdin.close()
                proc.stdout.close()
                if proc.stderr:
                    proc.stderr.close()

            events = [json.loads(line) for line in log_path.read_text(encoding="utf-8").splitlines() if line.strip()]
            self.assertEqual([e["event"] for e in events], ["start", "close"])
            self.assertFalse(m4_differential.pid_alive(events[0]["pid"]))

    def test_pantry_offline_remote_cache_and_mirror_staging(self) -> None:
        remotes = pantry_offline.load_archive_remote_packages(ROOT)
        self.assertEqual(len(remotes), 7)
        self.assertIn("package://pkg.pkl-lang.org/pkl-k8s/k8s@1.0.1", remotes)
        self.assertIn("package://pkg.pkl-lang.org/pkl-k8s/k8s@1.2.0", remotes)
        self.assertIn("package://pkg.pkl-lang.org/pkl-k8s/k8s@1.3.0", remotes)

        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)
            cache = td_path / "cache"
            seeded = pantry_offline.seed_remote_cache(cache, ROOT)
            self.assertEqual(len(seeded), 7)
            for row in seeded.values():
                self.assertTrue(Path(row["metadata_path"]).is_file())
                self.assertTrue(Path(row["zip_path"]).is_file())

            mirror = td_path / "mirror"
            mseeded = pantry_offline.seed_mirror_root(mirror, None, ROOT)
            self.assertEqual(len(mseeded), 7)
            self.assertTrue((mirror / "pkg.pkl-lang.org/pkl-k8s/k8s@1.0.1/k8s@1.0.1.json").is_file())
            self.assertTrue((mirror / "releases/pkl-k8s/k8s@1.0.1/k8s@1.0.1.zip").is_file())

            raw_gh = td_path / "raw_gh"
            rseeded = pantry_offline.seed_structured_read_fixtures(raw_gh, ROOT)
            self.assertEqual(rseeded, pantry_offline.STRUCTURED_READ_PINNED_SHA256)
            for rel in rseeded:
                self.assertTrue((raw_gh / "raw-gh" / rel).is_file())

        dev = pantry_offline.enumerate_pantry_dev_cases(ROOT)
        self.assertEqual(len(dev["all_cases"]), 125)
        self.assertEqual(len(dev["shell_suites"]), 1)
        pkg = pantry_offline.enumerate_m0_packaged_module_census(ROOT)
        self.assertEqual(pkg["package_count"], 29)
        self.assertEqual(pkg["module_count"], 206)

    def test_m4_http_fixture_server_compiles_with_pinned_jdk(self) -> None:
        pinned_java = ROOT / ".cache/oracle/temurin-21.0.13.1+2-LTS/jdk-21.0.13.1+2/Contents/Home/bin/java"
        if not pinned_java.is_file():
            self.fail(f"pinned oracle JDK missing: {pinned_java}")
        prev = os.environ.get("PKL_ORACLE_JAVA")
        os.environ["PKL_ORACLE_JAVA"] = str(pinned_java)
        try:
            meta = oracle.ensure_oracle(ROOT, fetch=False)
        finally:
            if prev is None:
                os.environ.pop("PKL_ORACLE_JAVA", None)
            else:
                os.environ["PKL_ORACLE_JAVA"] = prev
        self.assertTrue(meta["exact_jdk_lock_match"])
        javac = Path(meta["java"]).with_name("javac")
        with tempfile.TemporaryDirectory() as td:
            proc = subprocess.run(
                [str(javac), "-encoding", "UTF-8", "-d", td, str(ROOT / m4_differential.SERVER_SOURCE)],
                capture_output=True,
                text=True,
                timeout=60,
            )
            self.assertEqual(proc.returncode, 0, proc.stderr)
            self.assertTrue((Path(td) / "M4HttpFixtureServer.class").is_file())


if __name__ == "__main__":
    unittest.main()
