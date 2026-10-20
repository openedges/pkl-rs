"""Reject absent guest compilation and altered diagnostic evidence (no JVM required)."""
import json
from pathlib import Path
import sys
import tempfile
import shutil
from unittest import mock
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_graal


class GraalEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)

    def event(self, outcome="done", source="file:///fixture.pkl:1", tier=2):
        return (f"[engine] opt {outcome} engine=1 id=340 FunctionNode@123 |Tier {tier}|"
                f"Time 7ms|{'Reason: FrameWithoutBoxing|' if outcome == 'failed' else ''}Src {source} 0x1234\n").encode()

    def test_only_exact_guest_target_compilation_counts(self):
        raw = (self.event(source="pkl:base:1032") + self.event(source="file:///fixture.pkl:10")
               + self.event(source="file:///fixture.pkl.extra:1")
               + b"123 4 M1GraalProbe::main (50 bytes)\n"
               + self.event("failed", tier=1) + self.event(tier=2))
        events = m1_graal.compilation_events(raw, "file:///fixture.pkl", 1)
        self.assertEqual([(row["event"], row["tier"]) for row in events], [("failed", 1), ("done", 2)])
        self.assertEqual(bytes.fromhex(events[1]["raw_line_hex"]) + b"\n", self.event())

    def test_failed_queued_and_unrelated_compilations_do_not_become_successes(self):
        raw = self.event("failed") + self.event("queued") + self.event(source="pkl:base:1032")
        events = m1_graal.compilation_events(raw, "file:///fixture.pkl", 1)
        self.assertEqual([row["event"] for row in events], ["failed"])
        self.assertEqual(m1_graal.compilation_events(b"", "file:///fixture.pkl", 1), [])

    def test_missing_positive_target_and_tier_cannot_close_investigation(self):
        for raw in (b"", self.event("failed"), self.event(tier=1),
                    self.event(source="pkl:base:1032")):
            with self.subTest(raw=raw), self.assertRaisesRegex(ValueError, "guest target/tier"):
                m1_graal.verify_target_events(raw, "file:///fixture.pkl", 1, (1, 2))
        self.assertEqual(len(m1_graal.verify_target_events(
            self.event(tier=1) + self.event(tier=2), "file:///fixture.pkl", 1, (1, 2))), 2)

    def test_failure_label_without_actual_target_and_full_diagnostic_fails(self):
        stack = b"FrameWithoutBoxing EnsureVirtualizedNode.ensureVirtualFailure\n"
        for raw in (self.event("failed"), self.event(source="pkl:base:1032") + stack,
                    stack):
            with self.subTest(raw=raw), self.assertRaises(ValueError):
                m1_graal.verify_target_events(raw, "file:///fixture.pkl", 1, require_bailout=True)
        m1_graal.verify_target_events(self.event("failed") + stack,
                                     "file:///fixture.pkl", 1, require_bailout=True)

    def test_missing_or_partial_correctness_probe_fails(self):
        valid = {"phase": "compilation-probe", "verified_calls": 1000, "seeds": 256}
        m1_graal.verify_probe_output(json.dumps(valid).encode(), 256)
        for value in ({}, {**valid, "verified_calls": 0}, {**valid, "seeds": 1},
                      {**valid, "verified_calls": True}, {**valid, "phase": "preflight"}):
            with self.subTest(value=value), self.assertRaises(ValueError):
                m1_graal.verify_probe_output(json.dumps(value).encode(), 256)
        with self.assertRaises(ValueError):
            m1_graal.verify_probe_output(b"", 256)

    def test_success_tiers_cannot_be_combined_across_guest_targets_or_property_nodes(self):
        alternatives = [self.event(tier=1) + self.event(tier=2).replace(b"id=340", b"id=341"),
                        (self.event(tier=1) + self.event(tier=2)).replace(b"FunctionNode@123", b"PropertyTypeNode@123")]
        for raw in alternatives:
            with self.subTest(raw=raw), self.assertRaisesRegex(ValueError, "guest target/tier"):
                m1_graal.verify_target_events(raw, "file:///fixture.pkl", 1, (1, 2))

    def test_provenance_revalidates_original_baseline_jdk_options_and_environment(self):
        for relative in ("oracle.lock.json", "references.lock", "tools/OracleRuntimeProbe.java"):
            target = self.root / "sources" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((m1_graal.ROOT / relative).read_bytes())
        lock = json.loads((self.root / "sources/oracle.lock.json").read_bytes())
        distribution = next(iter(lock["performance_jdk"]["distributions"].values()))
        environment = {"PATH": "/bin:/usr/bin", "HOME": "/controlled/home", "TMPDIR": "/controlled/tmp",
                       "TMP": "/controlled/tmp", "TEMP": "/controlled/tmp", "LC_ALL": "C", "LANG": "C",
                       "TZ": "UTC", "TERM": "dumb", "NO_COLOR": "1", "COLUMNS": "80"}
        metadata = {"profile": "performance", "version": "0.32.0", "source": lock["source"],
                    "artifact": "/pinned/pkl-tools-0.32.0.jar", "artifact_sha256": lock["artifact"]["sha256"],
                    "artifact_url": lock["artifact"]["url"], "lock_sha256": m1_graal.oracle.sha256(self.root / "sources/oracle.lock.json"),
                    "java": "/pinned/java", "environment": environment,
                    "jdk": {"distribution": distribution, "properties": lock["performance_jdk"]["properties"],
                            "executable_sha256": distribution["runtime_sha256"]["bin/java"],
                            "release_sha256": distribution["runtime_sha256"]["release"],
                            "runtime_modules_sha256": distribution["runtime_sha256"]["lib/modules"],
                            "optimizing_runtime_probe": {"stdout_hex": b"GraalVM CE\n".hex(), "stderr_hex": "",
                                "source_sha256": m1_graal.oracle.sha256(self.root / "sources/tools/OracleRuntimeProbe.java")}}}
        metadata["command"] = [metadata["java"], *lock["jvm_options"], *lock["performance_jdk"]["jvm_options"],
                               "-Duser.home=" + environment["HOME"], "-Djava.io.tmpdir=" + environment["TMPDIR"],
                               "-cp", metadata["artifact"], "org.pkl.cli.Main"]
        def check(value):
            (self.root / "oracle.json").write_text(json.dumps(value))
            with mock.patch.object(m1_graal.oracle, "verify_artifact"):
                m1_graal.verify_oracle(self.root, {"oracle": value})
        check(metadata)
        for key, value in (("profile", "reference"), ("artifact_sha256", "0" * 64),
                           ("command", [*metadata["command"], "-Xmx2g"]),
                           ("environment", {**environment, "JAVA_TOOL_OPTIONS": "-Xmx2g"}),
                           ("jdk", {**metadata["jdk"], "release_sha256": "0" * 64})):
            with self.subTest(key=key), self.assertRaises(ValueError):
                check({**metadata, key: value})

    def save_capture(self, relative, command, stdout=b"", stderr=b""):
        folder = self.root / relative
        folder.mkdir(parents=True, exist_ok=True)
        result = {"status": "executed", "exit_code": 0, "command": command, "cwd": "/original/workspace"}
        for channel, data in (("stdout", stdout), ("stderr", stderr)):
            (folder / f"{channel}.bin").write_bytes(data)
            result[channel] = {"sha256": m1_graal.benchmark.digest(data), "bytes": len(data),
                               "path": str(folder / f"{channel}.bin")}
        (folder / "result.json").write_text(json.dumps(result))

    def report(self):
        report = {"schema": 1, "acquisition_error": None, "performance_evidence": False,
                  "compatibility_complete": False, "seconds": 15,
                  "acquisition_directory": str(self.root), "acquisition_workspace": "/original/workspace",
                  "source_stability_before_execution": True, "source_stability_after_execution": True,
                  "contract": m1_graal.cases_contract(), "sources": [], "artifacts": [], "expected": [], "cases": [],
                  "oracle": {"java": "/pinned/bin/java", "artifact": "/pinned/pkl-tools-0.32.0.jar",
                             "command": ["/pinned/bin/java", "-cp", "/pinned/pkl-tools-0.32.0.jar", "org.pkl.cli.Main"]}}
        for relative in m1_graal.source_paths():
            target = self.root / "sources" / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes((m1_graal.ROOT / relative).read_bytes())
            report["sources"].append(m1_graal.file_identity(self.root, "sources/" + relative))
        jar = self.root / "artifacts/pkl-tools-0.32.0.jar"
        for relative in m1_graal.required_artifact_paths(jar.name):
            target = self.root / relative
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_bytes(b"synthetic tooling unit test artifact")
        report["artifacts"] = [m1_graal.file_identity(self.root, relative)
                               for relative in sorted(m1_graal.required_artifact_paths(jar.name))]
        javac = "/pinned/bin/javac"
        self.save_capture("compile/helper", [javac, "-proc:none", "-cp", str(jar), "-d",
                          str(self.root / "artifacts/helper"), str(self.root / "sources/tools/java/M1GraalProbe.java")])
        for name, (filename, text) in m1_graal.override_sources(self.root).items():
            target = self.root / "generated" / name / filename
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(text)
            self.save_capture("compile/" + name, [javac, "-proc:none", "-cp", str(jar), "-d",
                              str(self.root / "artifacts" / name), str(target)])
        for workload, source, seeds in (("arithmetic", m1_graal.ARITHMETIC, 256), ("fib", m1_graal.FIB, 2)):
            expected = self.root / "expected" / workload
            expected.mkdir(parents=True)
            for seed in range(seeds):
                target = expected / f"seed-{seed:03d}.bin"
                target.write_bytes(b"synthetic Java oracle result\n")
                report["expected"].append(m1_graal.file_identity(self.root, target.relative_to(self.root).as_posix()))
            command = [*m1_graal.java_command(self.root, report), "preflight", str(self.root / "sources" / source),
                       workload, str(expected)]
            stdout = (json.dumps({"phase": "preflight", "seeds": seeds}, separators=(",", ":")) + "\n").encode()
            self.save_capture("preflight/" + workload, command, stdout)
        for spec in report["contract"]:
            source = (self.root / "sources" / spec["source"]).as_uri() + ":" + str(spec["line"])
            if spec["require_bailout"]:
                stderr = (self.event("failed", source) + b"FrameWithoutBoxing EnsureVirtualizedNode.ensureVirtualFailure\n"
                          b"VmUtils.skipInvisibleScopes VmUtils.getFrame GetEnclosingFrameNode.executeGeneric\n"
                          b"AbstractInvokeMethodLexicalNode.executeGeneric\n")
            else:
                stderr = self.event(source=source, tier=1) + self.event(source=source, tier=2)
                stderr += self.event("failed", source.rsplit(":", 1)[0] + ":5")
            seeds = 256 if spec["workload"] == "arithmetic" else 2
            stdout = json.dumps({"phase": "compilation-probe", "verified_calls": 1000, "seeds": seeds}).encode() + b"\n"
            self.save_capture("cases/" + spec["id"], m1_graal.case_command(self.root, report, spec), stdout, stderr)
            log = []
            for name, overlays in (("org.pkl.core.ast.frame.GetEnclosingFrameNode", {"original", "bypass"}),
                                   ("org.pkl.core.runtime.VmUtils", {"scope-original", "scope-bypass"})):
                artifact = self.root / "artifacts" / (spec["overlay"] if spec["overlay"] in overlays else jar.name)
                log.append("[class,load] " + name + " source: " + artifact.as_uri() + "\n")
            (self.root / "cases" / spec["id"] / "class-loading.log").write_text("".join(log))
            report["cases"].append({"id": spec["id"], "observations": m1_graal.observe_case(self.root, report, spec)})
        self.save_report(report)
        return report

    def save_report(self, report):
        report["evidence_sha256"] = m1_graal.inventory(self.root)
        (self.root / "report.json").write_text(json.dumps(report))

    def check_report(self):
        # The byte/coverage tests isolate the report verifier; oracle pin validation has its own tests.
        locked = {"artifact": {"filename": "pkl-tools-0.32.0.jar"}, "source": {"metadata_sha256": {}}}
        with mock.patch.object(m1_graal, "verify_oracle", return_value=locked):
            return m1_graal.check_report(self.root)

    def test_full_report_is_portable_and_preserves_other_guest_failures(self):
        self.report()
        report = self.check_report()
        positives = [row for row in report["cases"] if row["observations"]["completed_tiers"]]
        self.assertTrue(all(row["observations"]["all_guest_failure_events"] for row in positives))
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        relocated = Path(temporary.name) / "extracted"
        shutil.copytree(self.root, relocated)
        self.root = relocated
        (relocated / "EVIDENCE.json").write_text("independently verified archive metadata")
        self.assertEqual(len(self.check_report()["cases"]), 9)

    def test_report_missing_duplicate_case_and_forged_performance_claim_fail(self):
        report = self.report()
        original = list(report["cases"])
        for rows in ([], original[:-1], original[:-1] + original[:1]):
            report["cases"] = rows
            self.save_report(report)
            with self.assertRaisesRegex(ValueError, "required Graal case"):
                self.check_report()
        report["cases"] = original
        report["performance_evidence"] = True
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "performance claim"):
            self.check_report()

    def test_report_rejects_missing_compiled_target_even_when_logs_rehashed(self):
        report = self.report()
        spec = next(row for row in report["contract"] if row["id"] == "arithmetic-receiver-baseline")
        folder = self.root / "cases" / spec["id"]
        result = json.loads((folder / "result.json").read_bytes())
        stdout = (folder / "stdout.bin").read_bytes()
        self.save_capture("cases/" + spec["id"], result["command"], stdout, self.event(source="pkl:base:1032"))
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "guest target/tier"):
            self.check_report()

    def test_report_rejects_changed_class_origin_and_missing_raw_expected_output(self):
        report = self.report()
        path = self.root / "cases/arithmetic-frame-bypass/class-loading.log"
        original = path.read_bytes()
        path.write_bytes(original.replace(b"/artifacts/bypass", b"/artifacts/original"))
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "actual loaded class"):
            self.check_report()
        path.write_bytes(original)
        missing = self.root / "expected/fib/seed-001.bin"
        missing.unlink()
        report["expected"] = [row for row in report["expected"] if row["path"] != "expected/fib/seed-001.bin"]
        self.save_report(report)
        with self.assertRaises(OSError):
            self.check_report()

    def test_report_rejects_source_stability_and_override_source_mutations(self):
        report = self.report()
        report["source_stability_after_execution"] = False
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "sources changed"):
            self.check_report()
        report["source_stability_after_execution"] = True
        path = self.root / "generated/bypass/GetEnclosingFrameNode.java"
        path.write_text("unreported patch")
        self.save_report(report)
        with self.assertRaisesRegex(ValueError, "override source"):
            self.check_report()

    def capture(self):
        folder = self.root / "case"
        folder.mkdir()
        result = {"status": "executed", "exit_code": 0, "command": ["java", "Probe"]}
        for channel, data in (("stdout", b"all results checked\n"), ("stderr", self.event())):
            (folder / f"{channel}.bin").write_bytes(data)
            result[channel] = {"sha256": m1_graal.benchmark.digest(data), "bytes": len(data)}
        (folder / "result.json").write_text(json.dumps(result))
        return folder, result

    def test_raw_capture_changed_even_one_whitespace_byte_fails(self):
        folder, _ = self.capture()
        m1_graal.captured(self.root, "case", ["java", "Probe"])
        with (folder / "stdout.bin").open("ab") as stream:
            stream.write(b"\n")
        with self.assertRaisesRegex(ValueError, "capture identity"):
            m1_graal.captured(self.root, "case")

    def test_timeout_failed_command_and_command_substitution_fail(self):
        folder, result = self.capture()
        for key, value in (("status", "timeout"), ("exit_code", 1), ("command", ["echo", "fake"])):
            original = result[key]
            result[key] = value
            (folder / "result.json").write_text(json.dumps(result))
            with self.subTest(key=key), self.assertRaises(ValueError):
                m1_graal.captured(self.root, "case", ["java", "Probe"])
            result[key] = original

    def test_missing_and_unlisted_source_snapshots_fail(self):
        folder = self.root / "sources"
        folder.mkdir()
        source = folder / "helper.java"
        source.write_bytes(b"class Helper {}\n")
        rows = [m1_graal.file_identity(self.root, "sources/helper.java")]
        m1_graal.verify_files(self.root, rows, "sources")
        for variants in ([], rows + rows):
            with self.assertRaisesRegex(ValueError, "inventory"):
                m1_graal.verify_files(self.root, variants, "sources")
        (folder / "unlisted.java").write_bytes(b"altered input")
        with self.assertRaisesRegex(ValueError, "unlisted"):
            m1_graal.verify_files(self.root, rows, "sources")

    def test_unsafe_and_symlink_snapshot_paths_fail(self):
        for relative in ("../escape", "/absolute"):
            with self.assertRaisesRegex(ValueError, "unsafe"):
                m1_graal.contained(self.root, relative)
        (self.root / "escape").symlink_to(self.root.parent)
        with self.assertRaisesRegex(ValueError, "escaping"):
            m1_graal.contained(self.root, "escape/file")

    def test_artifact_and_input_byte_mutations_fail(self):
        folder = self.root / "artifacts"
        folder.mkdir()
        path = folder / "pkl.jar"
        path.write_bytes(b"original bytes")
        rows = [m1_graal.file_identity(self.root, "artifacts/pkl.jar")]
        path.write_bytes(b"replaced bytes")
        with self.assertRaisesRegex(ValueError, "identity changed"):
            m1_graal.verify_files(self.root, rows, "artifacts")


if __name__ == "__main__":
    unittest.main()
