"""Integrity checks for the bounded runtime evidence collector."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_runtime


class RuntimeEvidenceTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.serial = 0

    def capture(self, stdout=b"42", stderr=b"", code=0):
        result = {"status": "executed", "exit_code": code, "peak_rss_bytes": 4096}
        for name, data in (("stdout", stdout), ("stderr", stderr)):
            path = self.root / f"{self.serial}-{name}"
            path.write_bytes(data)
            result[name] = {"path": str(path), "bytes": len(data),
                            "sha256": hashlib.sha256(data).hexdigest()}
        self.serial += 1
        return result

    def test_comparison_preserves_newlines_and_error_bytes(self):
        self.assertFalse(m1_runtime.equivalent(self.capture(b"42"), self.capture(b"42\n")))
        self.assertFalse(m1_runtime.equivalent(self.capture(b"", b"first error", 1),
                                               self.capture(b"", b"second error", 1)))
        self.assertFalse(m1_runtime.equivalent(self.capture(code=1), self.capture(code=0)))
        self.assertTrue(m1_runtime.equivalent(self.capture(), self.capture()))

    def test_capture_tampering_is_rejected(self):
        result = self.capture()
        Path(result["stdout"]["path"]).write_bytes(b"99")
        with self.assertRaisesRegex(ValueError, "identity mismatch"):
            m1_runtime.raw(result, "stdout")

    def test_timed_out_process_does_not_compare_equal(self):
        left, right = self.capture(), self.capture()
        left["status"] = right["status"] = "timeout"
        self.assertFalse(m1_runtime.equivalent(left, right))

    def benchmark(self, java=True):
        rows = [{"phase": "preflight", "seeds": 256}]
        for index in range(100):
            rows.append({"phase": "warmup", "index": index, "iterations": 1000,
                         "elapsed_ns": 100_000_000, "ns_per_op": 100_000})
        if java:
            rows.append({"phase": "warmup-result", "converged": True})
        for index in range(5):
            rows.append({"phase": "measurement", "index": index, "iterations": 2500,
                         "elapsed_ns": 250_000_000, "ns_per_op": 100_000})
        return rows

    def series(self, rows, java=True):
        data = b"\n".join(json.dumps(row).encode() for row in rows)
        return m1_runtime.series(self.capture(data), 5, java)

    def test_valid_series_and_missing_sample_detection(self):
        self.assertTrue(self.series(self.benchmark())["warmup_converged"])
        rows = self.benchmark()
        rows.pop()
        with self.assertRaisesRegex(ValueError, "phases"):
            self.series(rows)

    def test_insufficient_warmup_and_false_convergence_are_rejected(self):
        rows = self.benchmark()
        del rows[1:11]
        for index, row in enumerate(rows[1:91]):
            row["index"] = index
        with self.assertRaisesRegex(ValueError, "warmup time"):
            self.series(rows)
        rows = self.benchmark()
        rows[99]["ns_per_op"] = 200_000
        rows[99]["iterations"] = 500
        with self.assertRaisesRegex(ValueError, "convergence"):
            self.series(rows)

    def test_architecture_decision_keeps_nonconvergence_and_regressions_visible(self):
        fork = {"rust": {"warmup_converged": True}, "java": {"warmup_converged": True},
                "rust_java_median_time_ratio": 1.27}
        self.assertIn("reject", m1_runtime.candidate_decision([fork], False))
        fork["rust_java_median_time_ratio"] = 0.46
        self.assertIn("bounded M2 baseline", m1_runtime.candidate_decision([fork], True))
        fork["java"]["warmup_converged"] = False
        self.assertIn("inconclusive", m1_runtime.candidate_decision([fork], True))


if __name__ == "__main__":
    unittest.main()
