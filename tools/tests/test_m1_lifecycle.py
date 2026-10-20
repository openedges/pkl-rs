"""Failure detection for the bounded lifecycle differential and its raw evidence."""
import hashlib
import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_lifecycle


class LifecycleEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def case(self, mode="retry", stdout=None):
        directory = self.root / mode
        results = {}
        for language in ("java", "rust"):
            destination = directory / language
            destination.mkdir(parents=True)
            result = {"status": "executed", "exit_code": 0}
            for channel, data in (("stdout", m1_lifecycle.PROTOCOLS[mode] if stdout is None else stdout),
                                  ("stderr", b"")):
                (destination / f"{channel}.bin").write_bytes(data)
                result[channel] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
            for name in m1_lifecycle.DIAGNOSTICS[mode]:
                (destination / f"{name}.error.bin").write_bytes(language.encode() + b" full error\n")
                (destination / f"{name}.error-class.bin").write_bytes(b"actual-error-class")
            results[language] = result
        return directory, results

    def observe(self, mode="retry", stdout=None):
        directory, results = self.case(mode, stdout)
        return m1_lifecycle.observe(mode, directory, results["java"], results["rust"])

    def test_protocol_matches_keep_all_diagnostic_mismatches(self):
        observations = self.observe()
        self.assertTrue(observations["protocol_raw_equal"])
        self.assertEqual(len(observations["diagnostics"]), 3)
        self.assertTrue(all(not row["raw_equal"] for row in observations["diagnostics"]))

    def test_same_empty_or_incomplete_outputs_cannot_pass(self):
        for mode, data in (("retry", b""), ("reentry", b"inner value=S reader_calls=2\n")):
            with self.assertRaisesRegex(ValueError, "protocol"):
                self.observe(mode, data)

    def test_whitespace_and_wrong_first_error_or_cache_counts_fail(self):
        protocol = m1_lifecycle.PROTOCOLS["retry"]
        variants = [protocol + b"\n", protocol.replace(b"second_calls=0", b"second_calls=1"),
                    protocol.replace(b"first_calls=2 second_calls=2", b"first_calls=3 second_calls=3")]
        for index, data in enumerate(variants):
            with self.subTest(index=index):
                temporary = tempfile.TemporaryDirectory()
                self.addCleanup(temporary.cleanup)
                self.root = Path(temporary.name)
                with self.assertRaisesRegex(ValueError, "protocol"):
                    self.observe(stdout=data)

    def test_nested_result_without_outer_resumption_cannot_pass(self):
        protocol = m1_lifecycle.PROTOCOLS["reentry"].replace(b"outer value=S!S! reader_calls=2\n", b"")
        with self.assertRaisesRegex(ValueError, "protocol"):
            self.observe("reentry", protocol)

    def test_timeout_exit_failure_missing_and_empty_diagnostics_fail(self):
        directory, results = self.case()
        for key, value in (("status", "timeout"), ("exit_code", 1)):
            original = results["rust"][key]
            results["rust"][key] = value
            with self.assertRaisesRegex(ValueError, "failed"):
                m1_lifecycle.observe("retry", directory, results["java"], results["rust"])
            results["rust"][key] = original
        diagnostic = directory / "rust/attempt-1.error.bin"
        diagnostic.write_bytes(b"")
        with self.assertRaisesRegex(ValueError, "empty full diagnostic"):
            m1_lifecycle.observe("retry", directory, results["java"], results["rust"])
        diagnostic.unlink()
        with self.assertRaises(OSError):
            m1_lifecycle.observe("retry", directory, results["java"], results["rust"])

    def test_capture_tampering_fails(self):
        directory, results = self.case()
        (directory / "rust/stdout.bin").write_bytes(b"changed")
        with self.assertRaisesRegex(ValueError, "identity"):
            m1_lifecycle.observe("retry", directory, results["java"], results["rust"])

    def report(self):
        report = {"schema": 1, "acquisition_error": None, "modes": list(m1_lifecycle.MODES),
                  "source_stability_before_execution": True, "source_stability_after_execution": True,
                  "sources": [], "artifacts": [], "cases": []}
        for mode in m1_lifecycle.MODES:
            directory, results = self.case(mode)
            report["cases"].append({"id": mode, **results,
                                    "observations": m1_lifecycle.observe(mode, directory, **results)})
            inputs = self.root / "inputs"
            inputs.mkdir(exist_ok=True)
            (inputs / f"{mode}.pkl").write_bytes(m1_lifecycle.INPUTS[mode])
        (inputs / f"{m1_lifecycle.NEGATIVE_MODE}.pkl").write_bytes(m1_lifecycle.NEGATIVE_INPUT)
        negative, result = self.negative()
        report["java_negative_observation"] = {"java": result,
            "observations": m1_lifecycle.observe_negative(negative, result)}
        source = self.root / "sources/source.rs"
        source.parent.mkdir()
        source.write_bytes(b"candidate source")
        report["sources"].append({"snapshot": "sources/source.rs", "sha256": m1_lifecycle.oracle.sha256(source)})
        artifact = self.root / "candidate"
        artifact.write_bytes(b"candidate executable")
        report["artifacts"].append({"path": "candidate", "sha256": m1_lifecycle.oracle.sha256(artifact)})
        self.save_report(report)
        return report

    def negative(self):
        directory = self.root / m1_lifecycle.NEGATIVE_MODE / "java"
        directory.mkdir(parents=True)
        result = {"status": "executed", "exit_code": 0}
        for channel, data in (("stdout", m1_lifecycle.NEGATIVE_PROTOCOL), ("stderr", b"")):
            (directory / f"{channel}.bin").write_bytes(data)
            result[channel] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
        for name, data in (("outer.error-class.bin", b"org.pkl.core.PklBugException"),
                           ("outer.error.bin", b"full java.util.ConcurrentModificationException diagnostic"),
                           ("outer.trace.bin", b"full Java trace"), ("after-close.error.bin", b"closed"),
                           ("after-close.error-class.bin", b"actual-close-class")):
            (directory / name).write_bytes(data)
        return directory, result

    def test_java_negative_observation_keeps_failure_distinct_and_requires_diagnostics(self):
        directory, result = self.negative()
        observation = m1_lifecycle.observe_negative(directory, result)
        self.assertFalse(observation["compatibility_pass"])
        self.assertFalse(observation["rust_comparison"])
        (directory / "outer.trace.bin").unlink()
        with self.assertRaises(OSError):
            m1_lifecycle.observe_negative(directory, result)

    def save_report(self, report):
        report["evidence_sha256"] = m1_lifecycle.inventory(self.root)
        (self.root / "report.json").write_text(json.dumps(report))

    def test_report_rechecks_case_coverage_and_source_stability(self):
        report = self.report()
        self.assertEqual(len(m1_lifecycle.check_report(self.root)["cases"]), 2)
        report["cases"].pop()
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "cases"):
            m1_lifecycle.check_report(self.root)
        report["cases"].append(report["cases"][0])
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "cases"):
            m1_lifecycle.check_report(self.root)

    def test_report_rejects_mutated_artifact_and_incomplete_inventory(self):
        self.report()
        (self.root / "candidate").write_bytes(b"wrong executable")
        with self.assertRaisesRegex(ValueError, "artifact"):
            m1_lifecycle.check_report(self.root)
        (self.root / "candidate").write_bytes(b"candidate executable")
        (self.root / "retry/java/attempt-1.error.bin").unlink()
        with self.assertRaisesRegex(ValueError, "inventory"):
            m1_lifecycle.check_report(self.root)


if __name__ == "__main__":
    unittest.main()
