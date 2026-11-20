"""Unit, negative, package-archive, and tamper-detection tests for tools/m4_differential.py."""
from __future__ import annotations

from collections import Counter
import hashlib
import io
import json
from pathlib import Path
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m4_differential
import oracle

ROOT = m4_differential.ROOT


def make_capture(
    stdout: bytes = b"ok\n",
    stderr: bytes = b"",
    exit_code: int = 0,
    files: dict[str, bytes] | None = None,
    timed_out: bool = False,
) -> dict:
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": exit_code,
        "files": {} if files is None else dict(files),
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": 1000,
    }


def make_pkg_bundle(
    entries: dict[str, bytes],
    *,
    pkg_name: str = "demo",
    version: str = "1.0.0",
    dependencies: dict | None = None,
    compresslevel: int = 6,
    date_time: tuple[int, int, int, int, int, int] = (1980, 2, 1, 0, 0, 0),
    include_dir_entry: bool = False,
    trailing_newline_meta: bool = False,
    bad_sidecar: bool = False,
) -> dict[str, bytes]:
    buf = io.BytesIO()
    with zipfile.ZipFile(buf, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=compresslevel) as zf:
        if include_dir_entry:
            zi = zipfile.ZipInfo(filename="sub/", date_time=date_time)
            zf.writestr(zi, b"")
        for name in sorted(entries):
            zi = zipfile.ZipInfo(filename=name, date_time=date_time)
            zi.compress_type = zipfile.ZIP_DEFLATED
            zf.writestr(zi, entries[name], compresslevel=compresslevel)
    zip_bytes = buf.getvalue()
    zip_sha = hashlib.sha256(zip_bytes).hexdigest()
    slug = f"{pkg_name}@{version}"
    meta = {
        "name": pkg_name,
        "packageUri": f"package://localhost:0/{slug}",
        "version": version,
        "packageZipUrl": f"https://localhost:0/{slug}/{slug}.zip",
        "packageZipChecksums": {"sha256": zip_sha},
        "dependencies": {} if dependencies is None else dependencies,
        "authors": [],
        "annotations": [],
    }
    meta_bytes = json.dumps(meta, indent=2).encode("utf-8") + (b"\n" if trailing_newline_meta else b"")
    meta_sha = hashlib.sha256(meta_bytes).hexdigest()
    return {
        f"generated/{slug}/{slug}": meta_bytes,
        f"generated/{slug}/{slug}.sha256": (b"deadbeef" if bad_sidecar else meta_sha.encode("utf-8")),
        f"generated/{slug}/{slug}.zip": zip_bytes,
        f"generated/{slug}/{slug}.zip.sha256": zip_sha.encode("utf-8"),
    }


def write_sealed_m4_report(
    directory: Path,
    cases: list[dict],
    captures: dict[str, tuple[dict, dict | None]],
    *,
    exact_jdk_lock_match: bool = True,
    source_binding: str | None = "recorded-build",
    source_stability: bool = True,
    include_sources_snapshot: bool = True,
    row_overrides: dict[str, dict] | None = None,
) -> dict:
    directory.mkdir(parents=True, exist_ok=True)
    oracle_meta = {
        "schema": 1,
        "version": "0.32.0",
        "exact_jdk_lock_match": exact_jdk_lock_match,
        "jdk": {"exact_jdk_lock_match": exact_jdk_lock_match, "properties": {"java.version": "21.0.13.1"}},
    }
    static_doc = {"schema_version": 1, "baseline": "0.32.0", "cases": cases}
    dynamic_doc = {"schema_version": 1, "cases": []}
    compat.write_json(directory / "oracle.json", oracle_meta)
    static_bytes = compat.encoded(static_doc)
    dynamic_bytes = compat.encoded(dynamic_doc)
    (directory / "cases.snapshot.json").write_bytes(static_bytes)
    (directory / "dynamic-cases.snapshot.json").write_bytes(dynamic_bytes)

    sources_bytes: bytes | None = None
    if include_sources_snapshot:
        sources_snap = {
            "before": {"Cargo.toml": "a" * 64},
            "after": {"Cargo.toml": ("a" if source_stability else "b") * 64},
        }
        sources_bytes = compat.encoded(sources_snap)
        (directory / "sources.snapshot.json").write_bytes(sources_bytes)

    cand_obj: dict | None = None
    if source_binding is not None:
        artifacts_dir = directory / "artifacts"
        artifacts_dir.mkdir(parents=True, exist_ok=True)
        probe_path = artifacts_dir / "probe"
        probe_path.write_bytes(b"fake-candidate-binary\n")
        cand_obj = {
            "name": "probe",
            "sha256": oracle.sha256(probe_path),
            "source_binding": source_binding,
            "build": None,
        }

    lanes = list(dict.fromkeys(c["lane"] for c in cases))
    results = []
    for case in cases:
        cdir = compat.case_directory(case["id"])
        folder = directory / "cases" / cdir
        java_raw, rust_raw = captures[case["id"]]
        java_stored = compat.store_capture(folder / "java", java_raw)
        rust_stored = compat.store_capture(folder / "rust", rust_raw) if rust_raw is not None else None
        extra = (row_overrides or {}).get(case["id"], {})
        status, diffs = m4_differential.classify_result(
            case,
            java_raw,
            rust_raw,
            java_assertion_failures=extra.get("java_assertion_failures"),
            rust_assertion_failures=extra.get("rust_assertion_failures"),
            cross_interop_ok=extra.get("cross_interop_ok"),
            idempotency_ok=extra.get("idempotency_ok"),
        )
        row = {
            "id": case["id"],
            "lane": case["lane"],
            "directory": cdir,
            "status": status,
            "differences": diffs,
            "expected_exit": case["expected_exit"],
            "comparison_mode": case.get("comparison_mode", "raw"),
            "java_assertion_failures": [],
            "rust_assertion_failures": [],
            "java": {"command": ["java"], "result": java_stored},
            "rust": None if rust_stored is None else {"command": ["rust"], "result": rust_stored},
        }
        row.update(extra)
        results.append(row)

    by_status = dict(sorted(Counter(r["status"] for r in results).items()))
    by_lane = {
        lane: dict(sorted(Counter(r["status"] for r in results if r["lane"] == lane).items()))
        for lane in lanes
    }
    report = {
        "schema_version": 1,
        "baseline": "0.32.0",
        "mode": "java-only-oracle-recording" if source_binding is None else "differential",
        "cases_manifest_sha256": compat.digest(static_bytes),
        "dynamic_cases_sha256": compat.digest(dynamic_bytes),
        "lanes": lanes,
        "workers": 1,
        "timeout_seconds": 30.0,
        "oracle": oracle_meta,
        "candidate": cand_obj,
        "server_self_test": {"passed": True, "checks": []},
        "selected_count": len(cases),
        "status_counts": by_status,
        "lane_status_counts": by_lane,
        "results": results,
    }
    if sources_bytes is not None:
        report["sources_snapshot_sha256"] = compat.digest(sources_bytes)
        report["sources_file_count"] = 1
        report["source_stability"] = source_stability
    report["evidence_sha256"] = {
        str(p.relative_to(directory).as_posix()): oracle.sha256(p)
        for p in sorted(m4_differential.evidence_paths(directory))
    }
    compat.write_json(directory / "report.json", report)
    return report


class M4DifferentialTests(unittest.TestCase):
    def test_classify_result_raw_bytes_and_no_normalization(self) -> None:
        case = {"id": "m4:test:raw", "lane": "host-io-http-security", "expected_exit": [0]}
        exp = make_capture(stdout=b"a = 1\n", files={"work/deps.json": b"{}\n"})
        self.assertEqual(
            m4_differential.classify_result(case, exp, make_capture(stdout=b"a = 1\n", files={"work/deps.json": b"{}\n"})),
            ("pass", []),
        )
        # No trimming or CRLF normalization
        self.assertEqual(
            m4_differential.classify_result(case, exp, make_capture(stdout=b"a = 1\r\n", files={"work/deps.json": b"{}\n"})),
            ("mismatch", ["stdout"]),
        )
        # Java-only recording NEVER passes
        self.assertEqual(m4_differential.classify_result(case, exp, None), ("oracle-recorded", []))
        # Candidate exit codes 2/3/4
        self.assertEqual(
            m4_differential.classify_result(case, exp, make_capture(exit_code=2)),
            ("candidate-unsupported", ["candidate_exit_2"]),
        )
        self.assertEqual(
            m4_differential.classify_result(case, exp, make_capture(exit_code=4)),
            ("candidate-internal-error", ["candidate_exit_4"]),
        )
        # Oracle and Rust assertion failures preserved honestly
        self.assertEqual(
            m4_differential.classify_result(case, exp, exp, java_assertion_failures=["assertion:proxy_hit_missing"]),
            ("oracle-assertion-failed", ["assertion:proxy_hit_missing"]),
        )
        self.assertEqual(
            m4_differential.classify_result(case, exp, exp, rust_assertion_failures=["assertion:reader_orphan_pid_99"]),
            ("mismatch", ["assertion:reader_orphan_pid_99"]),
        )

    def test_package_archive_honest_comparison(self) -> None:
        case = {
            "id": "m4:project:package-demo",
            "lane": "projects-packages-cache",
            "expected_exit": [0],
            "comparison_mode": "package-archive",
        }
        payload = {
            "Main.pkl": "".join(
                f'k{i} = "val-{i * 17 % 97}-{(i * 31) % 89}-item-{i}"\n' for i in range(200)
            ).encode("utf-8")
        }
        java_files = make_pkg_bundle(payload, compresslevel=9)
        rust_same = make_pkg_bundle(payload, compresslevel=9)
        rust_diff_deflate = make_pkg_bundle(payload, compresslevel=1)

        # Identical DEFLATE bytes -> raw_zip_bytes_match = True
        meta_same: dict = {}
        st, diffs = m4_differential.classify_result(
            case, make_capture(files=java_files), make_capture(files=rust_same),
            package_compare_meta=meta_same, idempotency_ok=True, cross_interop_ok=True,
        )
        self.assertEqual((st, diffs), ("pass", []))
        self.assertTrue(meta_same["raw_zip_bytes_match"])

        # Different DEFLATE stream (same entries/uncompressed/DOS mtime/self-consistent sha256)
        meta_diff: dict = {}
        st2, diffs2 = m4_differential.classify_result(
            case, make_capture(files=java_files), make_capture(files=rust_diff_deflate),
            package_compare_meta=meta_diff, idempotency_ok=True, cross_interop_ok=True,
        )
        self.assertEqual((st2, diffs2), ("pass", []))
        self.assertFalse(meta_diff["raw_zip_bytes_match"])

        # Non-idempotent or failed Java cross-interop MUST fail
        st_ni, diffs_ni = m4_differential.classify_result(
            case, make_capture(files=java_files), make_capture(files=rust_diff_deflate),
            idempotency_ok=False, cross_interop_ok=True,
        )
        self.assertEqual((st_ni, diffs_ni), ("mismatch", ["package-not-idempotent"]))
        st_xi, diffs_xi = m4_differential.classify_result(
            case, make_capture(files=java_files), make_capture(files=rust_diff_deflate),
            idempotency_ok=True, cross_interop_ok=False,
        )
        self.assertEqual((st_xi, diffs_xi), ("mismatch", ["java-cross-interop-failed"]))

        # Bad DOS mtime, extra dir entry, trailing newline on metadata JSON, bad sidecar all fail closed
        for bad_kwargs in (
            {"date_time": (2026, 1, 1, 0, 0, 0)},
            {"include_dir_entry": True},
            {"trailing_newline_meta": True},
            {"bad_sidecar": True},
        ):
            bad_files = make_pkg_bundle(payload, **bad_kwargs)
            st_b, _ = m4_differential.classify_result(
                case, make_capture(files=java_files), make_capture(files=bad_files),
                idempotency_ok=True, cross_interop_ok=True,
            )
            self.assertEqual(st_b, "mismatch", f"expected failure for {bad_kwargs}")

        # Empty ZIP exact SHA-256 verification
        empty_bundle = make_pkg_bundle({})
        bundles = m4_differential.verify_package_bundles(empty_bundle, expect_empty_zip_sha256=m4_differential.EMPTY_ZIP_SHA256)
        self.assertEqual(next(iter(bundles.values()))["zip_sha256"], m4_differential.EMPTY_ZIP_SHA256)

    def test_verify_report_strict_vs_integrity_and_tamper_detection(self) -> None:
        case = {"id": "m4:test:one", "lane": "host-io-http-security", "expected_exit": [0]}
        with tempfile.TemporaryDirectory() as td:
            td_path = Path(td)

            # 1. Java-only passes verify_report(strict=False), FAILS verify_report(strict=True)
            j_dir = td_path / "java_only"
            write_sealed_m4_report(j_dir, [case], {"m4:test:one": (make_capture(), None)}, source_binding=None)
            m4_differential.verify_report(j_dir, strict=False)
            with self.assertRaisesRegex(ValueError, "recorded-build Rust candidate"):
                m4_differential.verify_report(j_dir, strict=True)

            # 2. Full differential recorded-build + exact JDK lock passes BOTH
            ok_dir = td_path / "pass_ok"
            write_sealed_m4_report(ok_dir, [case], {"m4:test:one": (make_capture(), make_capture())})
            m4_differential.verify_report(ok_dir, strict=False)
            m4_differential.verify_report(ok_dir, strict=True)

            # 3. Tampering with stdout.bin fails closed
            cdir = compat.case_directory("m4:test:one")
            (ok_dir / "cases" / cdir / "rust/stdout.bin").write_bytes(b"tampered\n")
            with self.assertRaisesRegex(ValueError, "evidence file modified"):
                m4_differential.verify_report(ok_dir, strict=False)

            # 4. Non-exact JDK lock fails strict gate
            bad_jdk = td_path / "bad_jdk"
            write_sealed_m4_report(
                bad_jdk, [case], {"m4:test:one": (make_capture(), make_capture())}, exact_jdk_lock_match=False
            )
            m4_differential.verify_report(bad_jdk, strict=False)
            with self.assertRaisesRegex(ValueError, "exact_jdk_lock_match"):
                m4_differential.verify_report(bad_jdk, strict=True)

            # 5. Unbound caller-supplied candidate fails strict gate
            unbound = td_path / "unbound"
            write_sealed_m4_report(
                unbound, [case], {"m4:test:one": (make_capture(), make_capture())}, source_binding="unbound-caller-supplied"
            )
            m4_differential.verify_report(unbound, strict=False)
            with self.assertRaisesRegex(ValueError, "recorded-build"):
                m4_differential.verify_report(unbound, strict=True)

            # 6. Source drift during run (source_stability=False) fails strict gate
            drifted = td_path / "drifted"
            write_sealed_m4_report(
                drifted, [case], {"m4:test:one": (make_capture(), make_capture())}, source_stability=False
            )
            m4_differential.verify_report(drifted, strict=False)
            with self.assertRaisesRegex(ValueError, "source_stability"):
                m4_differential.verify_report(drifted, strict=True)

            # 7. Omitted sources_snapshot_sha256 / source_stability fails strict gate
            no_snap = td_path / "no_snap"
            write_sealed_m4_report(
                no_snap,
                [case],
                {"m4:test:one": (make_capture(), make_capture())},
                include_sources_snapshot=False,
            )
            m4_differential.verify_report(no_snap, strict=False)
            with self.assertRaisesRegex(ValueError, "sources_snapshot_sha256"):
                m4_differential.verify_report(no_snap, strict=True)

            # 8. Tampered candidate.sha256 (disagreeing with evidence_sha256["artifacts/probe"]) fails closed
            bad_cand = td_path / "bad_cand"
            rep = write_sealed_m4_report(bad_cand, [case], {"m4:test:one": (make_capture(), make_capture())})
            rep["candidate"]["sha256"] = "0" * 64
            compat.write_json(bad_cand / "report.json", rep)
            with self.assertRaisesRegex(ValueError, "candidate sha256 disagrees"):
                m4_differential.verify_report(bad_cand, strict=False)

            # 9. Forged cross_interop_ok disagrees with cross_interop_detail fails closed
            pkg_case = {
                "id": "m4:project:package-demo",
                "lane": "projects-packages-cache",
                "expected_exit": [0],
                "comparison_mode": "package-archive",
                "assertions": {"cross_interop_modules": ["package://localhost:0/demo@1.0.0#/Main.pkl"]},
            }
            pkg_files = make_pkg_bundle({"Main.pkl": b"x = 1\n"})
            forged_xi = td_path / "forged_xi"
            write_sealed_m4_report(
                forged_xi,
                [pkg_case],
                {"m4:project:package-demo": (make_capture(files=pkg_files), make_capture(files=pkg_files))},
                row_overrides={
                    "m4:project:package-demo": {
                        "idempotency_ok": True,
                        "cross_interop_ok": True,
                        "cross_interop_detail": {
                            "modules": {
                                "package://localhost:0/demo@1.0.0#/Main.pkl": {
                                    "java_pkg": {"exit_code": 0, "stdout_sha256": "a" * 64, "stderr_sha256": "b" * 64},
                                    "rust_pkg": {"exit_code": 1, "stdout_sha256": "c" * 64, "stderr_sha256": "d" * 64},
                                    "match": False,
                                }
                            }
                        },
                    }
                },
            )
            with self.assertRaisesRegex(ValueError, "cross_interop_ok disagrees"):
                m4_differential.verify_report(forged_xi, strict=False)

            # 10. Cold-https-mirror zero server hit fails closed
            mirror_case = {
                "id": "m4:pantry-packaged:cold-https-mirror:pkl.toml:toml.pkl",
                "lane": "pantry-packaged",
                "expected_exit": [0],
                "comparison_mode": "raw",
            }
            zero_hit = td_path / "zero_hit"
            write_sealed_m4_report(
                zero_hit,
                [mirror_case],
                {mirror_case["id"]: (make_capture(), make_capture())},
                row_overrides={
                    mirror_case["id"]: {
                        "java_server_events_count": 2,
                        "rust_server_events_count": 0,
                        "java_assertion_failures": [],
                        "rust_assertion_failures": [],
                    }
                },
            )
            with self.assertRaisesRegex(ValueError, "cold-https-mirror rust server hit assertion missing"):
                m4_differential.verify_report(zero_hit, strict=False)

    def test_package_archive_co_packaged_local_dep_alignment_and_remote_strictness(self) -> None:
        dep_payload = {
            "Dep.pkl": "".join(f'd{i} = "dep-val-{(i * 23) % 97}-{i}"\n' for i in range(180)).encode("utf-8")
        }
        consumer_payload = {
            "Consumer.pkl": "".join(f'c{i} = "cons-val-{(i * 29) % 89}-{i}"\n' for i in range(180)).encode("utf-8")
        }
        remote_sha = "1" * 64

        def build_closure(compresslevel: int, *, override_local_sha: str | None = None, override_remote_sha: str | None = None) -> dict[str, bytes]:
            dep_files = make_pkg_bundle(dep_payload, pkg_name="dep", compresslevel=compresslevel)
            dep_meta_sha = hashlib.sha256(dep_files["generated/dep@1.0.0/dep@1.0.0"]).hexdigest()
            cons_files = make_pkg_bundle(
                consumer_payload,
                pkg_name="consumer",
                compresslevel=compresslevel,
                dependencies={
                    "dep": {
                        "uri": "package://localhost:0/dep@1.0.0",
                        "checksums": {"sha256": override_local_sha or dep_meta_sha},
                    },
                    "remote": {
                        "uri": "package://localhost:0/remote@2.0.0",
                        "checksums": {"sha256": override_remote_sha or remote_sha},
                    },
                },
            )
            return {**dep_files, **cons_files}

        java_closure = build_closure(9)
        rust_closure = build_closure(1)
        self.assertNotEqual(
            java_closure["generated/dep@1.0.0/dep@1.0.0.zip"],
            rust_closure["generated/dep@1.0.0/dep@1.0.0.zip"],
        )

        # Valid co-packaged local dep with different DEFLATE stream aligns cleanly
        diffs, meta = m4_differential.compare_package_archives(java_closure, rust_closure)
        self.assertEqual(diffs, [])
        self.assertFalse(meta["raw_zip_bytes_match"])
        self.assertIn("dep", meta["bundles"]["generated/consumer@1.0.0/consumer@1.0.0"]["aligned_local_dep_shas"])

        # Corrupted co-packaged local dep checksum in consumer MUST fail closed
        bad_local = build_closure(1, override_local_sha="f" * 64)
        diffs_bad_local, _ = m4_differential.compare_package_archives(java_closure, bad_local)
        self.assertTrue(any("candidate-bundle-invalid" in d for d in diffs_bad_local), diffs_bad_local)

        # Tampered remote dep checksum MUST fail even if it aliases the local dep's SHA-256!
        rust_dep_meta_sha = hashlib.sha256(rust_closure["generated/dep@1.0.0/dep@1.0.0"]).hexdigest()
        bad_remote = build_closure(1, override_remote_sha=rust_dep_meta_sha)
        diffs_bad_remote, _ = m4_differential.compare_package_archives(java_closure, bad_remote)
        self.assertIn("files/generated/consumer@1.0.0/consumer@1.0.0:metadata-json", diffs_bad_remote)

    def test_snapshot_rust_sources_covers_runtime_and_host_native(self) -> None:
        snap = m4_differential.snapshot_rust_sources(ROOT)
        self.assertIn("Cargo.toml", snap)
        self.assertIn("Cargo.lock", snap)
        self.assertIn("rust-toolchain.toml", snap)
        self.assertIn("crates/pkl-runtime/src/lib.rs", snap)
        self.assertIn("crates/pkl-host-native/src/lib.rs", snap)
        self.assertIn("crates/pkl-host-native/assets/PklCARoots.pem", snap)


if __name__ == "__main__":
    unittest.main()
