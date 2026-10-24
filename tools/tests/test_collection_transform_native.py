"""Negative tests for native evidence parsing, not evaluator compatibility."""
import importlib.util
import hashlib
import json
import sys
from unittest.mock import patch
from pathlib import Path
import tempfile
import unittest

SPEC = importlib.util.spec_from_file_location("collection_transform_native", Path(__file__).parents[1] / "collection_transform_native.py")
MODULE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(MODULE)


class NativeCaptureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.fixture = {"id": "synthetic-protocol-only", "expected": [{"index": 0}]}
        for name, value in {
            "call-0.expression": "value", "call-0.status": "success\n",
            "call-0.disposition": "", "call-0.phase": "", "call-0.stdout": "answer",
            "call-0.stderr": "", "call-0.reads-before": "0", "call-0.reads-after": "0",
            "call-0.logs-before": "0", "call-0.logs-after": "0", "retained-0": "answer",
            "calls": "1", "reads": "0", "logs": "0", "effects": "0", "module-requests": "0",
            "collections": "3", "terminal-slots": "0", "retained-output-buffers": "1",
            "provider-entries": "0", "resource-policy-requests": "0",
        }.items():
            self.put(name, value)

    def put(self, name, value):
        (self.root / (name + ".bin")).write_bytes(value.encode("utf-8", "replace"))
        (self.root / (name + ".utf16be.bin")).write_bytes(value.encode("utf-16-be", "surrogatepass"))

    def test_lossless_supplementary_and_unpaired_units(self):
        self.put("unicode", "\U0001f60a\ud800x\udc00")
        self.assertEqual(MODULE.field(self.root, "unicode")[1], [0xD83D, 0xDE0A, 0xD800, 120, 0xDC00])
        (self.root / "unicode.bin").write_bytes(b"normalized")
        with self.assertRaisesRegex(ValueError, "UTF8/UTF16"):
            MODULE.field(self.root, "unicode")

    def test_unormalized_status_and_counter_rejected(self):
        self.put("call-0.status", "success")
        with self.assertRaisesRegex(ValueError, "status"):
            MODULE.actual_session(self.fixture, self.root)
        self.put("call-0.status", "success\n")
        self.put("calls", "01")
        with self.assertRaisesRegex(ValueError, "counter"):
            MODULE.actual_session(self.fixture, self.root)

    def test_owned_buffer_changes_rejected(self):
        self.put("retained-0", "changed")
        with self.assertRaisesRegex(ValueError, "owned payload"):
            MODULE.actual_session(self.fixture, self.root)

    def test_post_close_api_classification_is_consumed_and_preserved(self):
        self.put("call-0.status", "api-error\n")
        self.put("call-0.stdout", "")
        self.put("call-0.stderr", "evaluator closed")
        self.put("retained-output-buffers", "0")
        for suffix in [".bin", ".utf16be.bin"]:
            (self.root / ("retained-0" + suffix)).unlink()
        with self.assertRaises(FileNotFoundError):
            MODULE.actual_session(self.fixture, self.root)
        (self.root / "call-0.api-error-kind.bin").write_bytes(b"Closed")
        actual = MODULE.actual_session(self.fixture, self.root)
        self.assertEqual(actual["calls"][0]["apiErrorKindRawHex"], b"Closed".hex())
        self.assertEqual(actual["calls"][0]["stderrUnits"], list(map(ord, "evaluator closed")))

    def test_effect_counts_must_join_calls_and_terminal(self):
        self.put("reads", "1")
        with self.assertRaisesRegex(ValueError, "terminal effect counters"):
            MODULE.actual_session(self.fixture, self.root)
        self.put("reads", "0")
        self.put("call-0.logs-before", "1")
        with self.assertRaisesRegex(ValueError, "counter continuity"):
            MODULE.actual_session(self.fixture, self.root)

    def test_unconsumed_call_effect_and_retained_records_rejected(self):
        for name in ['call-1.stdout', 'effect-0.kind', 'retained-1']:
            with self.subTest(name=name):
                self.put(name, 'orphan')
                try:
                    with self.assertRaisesRegex(ValueError, 'unconsumed raw'):
                        MODULE.actual_session(self.fixture, self.root)
                finally:
                    (self.root / (name + '.bin')).unlink()
                    (self.root / (name + '.utf16be.bin')).unlink()

    def test_nonregular_raw_directory_rejected_without_descent(self):
        extra = self.root / 'unexpected-directory'
        extra.mkdir()
        with self.assertRaisesRegex(ValueError, 'nonregular raw'):
            MODULE.actual_session(self.fixture, self.root)

    def test_fixture_preflight_precedes_manifest_payload_reads(self):
        fixture = self.root / 'untrusted-fixture'
        fixture.mkdir()
        (fixture / 'manifest.json').write_text(json.dumps({'files': {
            'foreign.bin': {'sha256': hashlib.sha256(b'foreign').hexdigest(), 'bytes': 7}}}))
        (fixture / 'foreign.bin').write_bytes(b'foreign')
        snapshot = self.root / 'snapshot'
        snapshot.mkdir()
        (snapshot / 'source.txt').write_bytes(b'source')
        artifact = self.root / 'not-executed-artifact'
        artifact.write_bytes(b'not executable; protocol mock only')
        proof = self.root / 'proof.json'
        proof.write_text(json.dumps({'schema': 1, 'status': 'CLOSED', 'sourceStable': True,
            'snapshotRoot': str(snapshot), 'sources': {'source.txt': hashlib.sha256(b'source').hexdigest()},
            'artifacts': {'native': {'path': str(artifact), 'sha256': hashlib.sha256(artifact.read_bytes()).hexdigest()}}}))
        original_sha = MODULE.sha
        reads, commands = [], []
        def guarded_sha(path):
            if path.is_relative_to(fixture):
                reads.append(str(path))
            return original_sha(path)
        class Process:
            pid = 1  # Explicit mocked process; no native, Node or Java is run.
            def __init__(self, argv, **kwargs):
                commands.append(argv)
            def wait(self):
                return 0 if len(commands) == 1 else 1
        argv = ['collection_transform_native.py', '--output', str(self.root / 'output'),
                '--fixtures', str(fixture), '--build-proof', str(proof),
                '--build-proof-sha256', original_sha(proof), '--node', sys.executable]
        with patch.object(sys, 'argv', argv), patch.object(MODULE, 'sha', guarded_sha), \
                patch.object(MODULE.subprocess, 'Popen', Process), patch.object(MODULE.platform, 'platform', return_value='mocked-protocol-host'):
            self.assertEqual(MODULE.main(), 1)
        self.assertEqual(len(commands), 2)  # version, then rejecting verifier
        self.assertIn('--fixtures', commands[1])
        self.assertEqual(reads, [])

    def test_unaccounted_module_request_rejected(self):
        (self.root / "module-0.request.bin").write_bytes(b"Debug request")
        (self.root / "module-0.token.bin").write_bytes(b"token")
        with self.assertRaisesRegex(ValueError, "module request ledger"):
            MODULE.actual_session(self.fixture, self.root)
        self.put("module-requests", "1")
        actual = MODULE.actual_session(self.fixture, self.root)
        self.assertEqual(actual["moduleRequests"][0]["requestRawHex"], b"Debug request".hex())
        self.assertEqual(actual["moduleRequests"][0]["tokenRawHex"], b"token".hex())

    def policy(self, uri, allowed, call=0):
        self.put("resource-policy-requests", "1")
        for name, value in {"call-index": str(call), "uri": uri,
                            "allowed": "true" if allowed else "false",
                            "message": "" if allowed else "Host policy denial",
                            "token": "resource-token"}.items():
            self.put("resource-policy-0." + name, value)

    def reader(self, uri):
        for name, value in {"effects": "1", "reads": "1", "call-0.reads-after": "1",
                            "effect-0.call-index": "0", "effect-0.kind": "read",
                            "effect-0.uri": uri, "effect-0.attempt": "1",
                            "effect-0.result": "text", "effect-0.value": "reader value"}.items():
            self.put(name, value)

    def test_policy_denial_preserved_without_reader_effect(self):
        self.policy("prop:pkl.outputFormat", False)
        actual = MODULE.actual_session(self.fixture, self.root)
        self.assertEqual(actual["effects"], [])
        self.assertEqual(actual["resourcePolicyRequests"], [{"callIndex": 0,
            "uri": "prop:pkl.outputFormat", "allowed": False,
            "message": "Host policy denial", "token": "resource-token"}])
        self.reader("prop:pkl.outputFormat")
        with self.assertRaisesRegex(ValueError, "policy/read effect join"):
            MODULE.actual_session(self.fixture, self.root)

    def test_allowed_policy_requires_corresponding_reader(self):
        self.policy("ct:allowed", True)
        with self.assertRaisesRegex(ValueError, "policy/read effect join"):
            MODULE.actual_session(self.fixture, self.root)
        self.reader("ct:allowed")
        self.assertTrue(MODULE.actual_session(self.fixture, self.root)["resourcePolicyRequests"][0]["allowed"])
        self.put("effect-0.uri", "ct:different")
        with self.assertRaisesRegex(ValueError, "policy/read effect join"):
            MODULE.actual_session(self.fixture, self.root)

    def test_policy_wrong_call_decision_and_orphan_rejected(self):
        self.policy("ct:allowed", True, call=1)
        with self.assertRaisesRegex(ValueError, "absent API call"):
            MODULE.actual_session(self.fixture, self.root)
        self.policy("prop:denied", True)
        with self.assertRaisesRegex(ValueError, "configured allowlist"):
            MODULE.actual_session(self.fixture, self.root)
        self.policy("prop:denied", False)
        self.put("resource-policy-requests", "0")
        with self.assertRaisesRegex(ValueError, "unconsumed raw"):
            MODULE.actual_session(self.fixture, self.root)

    def test_policy_prefix_preserves_java_looking_at_semantics(self):
        # Protocol-only URI input: this checks policy parsing, not URI acceptance.
        self.policy("ct:prefix\ntrailing", True)
        self.reader("ct:prefix\ntrailing")
        actual = MODULE.actual_session(self.fixture, self.root)
        self.assertTrue(actual["resourcePolicyRequests"][0]["allowed"])
        self.put("resource-policy-0.allowed", "True")
        with self.assertRaisesRegex(ValueError, "invalid resource policy decision"):
            MODULE.actual_session(self.fixture, self.root)

    def second_call(self):
        self.fixture["expected"].append({"index": 1})
        for source in self.root.glob("call-0.*"):
            (self.root / source.name.replace("call-0.", "call-1.")).write_bytes(source.read_bytes())
        self.put("retained-1", "answer")
        self.put("calls", "2")
        self.put("retained-output-buffers", "2")

    def duplicate_policy(self):
        for source in self.root.glob("resource-policy-0.*"):
            (self.root / source.name.replace("resource-policy-0.", "resource-policy-1.")).write_bytes(source.read_bytes())
        self.put("resource-policy-requests", "2")

    def test_duplicate_denied_policy_token_rejected(self):
        self.policy("prop:denied", False)
        self.duplicate_policy()
        with self.assertRaisesRegex(ValueError, "duplicate resource policy token"):
            MODULE.actual_session(self.fixture, self.root)

    def test_denied_policy_call_order_rejected(self):
        self.second_call()
        self.policy("prop:denied", False, call=1)
        self.duplicate_policy()
        self.put("resource-policy-1.call-index", "0")
        self.put("resource-policy-1.token", "different-resource-token")
        with self.assertRaisesRegex(ValueError, "policy API call order"):
            MODULE.actual_session(self.fixture, self.root)

    def test_effect_call_order_rejected_even_when_counts_join(self):
        self.second_call()
        self.reader("ct:allowed")
        for source in self.root.glob("effect-0.*"):
            (self.root / source.name.replace("effect-0.", "effect-1.")).write_bytes(source.read_bytes())
        self.put("effect-0.call-index", "1")
        self.put("effects", "2")
        self.put("reads", "2")
        self.put("call-1.reads-before", "1")
        self.put("call-1.reads-after", "2")
        with self.assertRaisesRegex(ValueError, "effect API call order"):
            MODULE.actual_session(self.fixture, self.root)


if __name__ == "__main__":
    unittest.main()
