#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""5-lane offline raw-byte Java 0.32.0 differential runner for Milestone M5.

Lanes:
  1. rust-api-and-cli : Public Rust Evaluator API (`pkl_m5_api_probe` vs `M5ApiOracleProbe.java`)
                        + full `pkl` product CLI (`eval`, `test` [spec/minimal/JUnit/--overwrite],
                        `run` [`pkl:Command`], `shell-completion`, `--help`/`--version`,
                        `project`/`download-package`/`analyze`, errors & exit codes)
  2. repl             : Pipe + real POSIX PTY (`TERM=dumb`, `TIOCSCTTY`, 80x24, `stderr=PIPE`)
                        raw-byte transcript diff vs Java `pkl repl`
  3. server           : Python stdlib MessagePack client (`0x20`-`0x32`) driving Java & Rust
                        `pkl server` (responses, binary payloads, client/external callbacks,
                        logs, HTTP/HTTPS/proxy/rewrites/headers, pipelining & interleaving,
                        errors, close & orphan PID checks)
  4. formatter        : All 30 `corpus.json` `formatter-snippet` goldens (incl. 2nd-pass
                        idempotency `rc=0`) + upstream edge/error cases + `pkl format` CLI
                        (`--write`, `--diff-name-only`, `--silent`, `--grammar-version=1|2`,
                        stdin `-`, directory traversal, exit codes `0`/`1`/`11`)
  5. pkldoc           : Standalone `pkldoc` (`org.pkl.doc.Main`) recursive `followlinks=False`
                        raw-byte file + exact `os.readlink()` symlink tree diff (SinglePackageTest,
                        DocGeneratorTest run-1 & incremental run-2, older-version non-overwrite,
                        idempotent re-run, `--no-symlinks`, staged `stdlib/doc-package-info.pkl`,
                        non-test-mode 0.32.0, `--migrate`, `--help`/`--version`, errors)

Reuses `tools/oracle.py` (pinned Eclipse Adoptium Temurin 21.0.13.1+2-LTS and
`pkl-tools-0.32.0.jar`), `tools/compat.py`, and `tools/m4_differential.py::M4FixtureServers`.
Never normalizes stdout/stderr, trims whitespace, fakes Java versions in Rust, or
counts Java-only recordings as passes.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import signal
import subprocess
import sys
import time
import zipfile

import compat
import m4_differential
import m5_pkldoc_tree
import m5_repl_driver
import m5_server_client
import oracle

ROOT = Path(__file__).resolve().parents[1]
CASES_PATH = Path("tests/fixtures/m5/cases.json")
CORPUS_PATH = Path("compatibility/corpus.json")
API_PROBE_JAVA = Path("tools/java/M5ApiOracleProbe.java")
M4_READER_BIN = Path("tests/fixtures/m4/readers/m4_ext_reader.py")

ALL_LANES = (
    "rust-api-and-cli",
    "repl",
    "server",
    "formatter",
    "pkldoc",
)
DEFAULT_TIMEOUT = 60.0

DEFAULT_PKL_PACKAGE = "pkl-cli"
DEFAULT_PKL_BIN = "pkl"
DEFAULT_PKLDOC_PACKAGE = "pkl-doc"
DEFAULT_PKLDOC_BIN = "pkldoc"
DEFAULT_API_PROBE_PACKAGE = "pkl"
DEFAULT_API_PROBE_EXAMPLE = "pkl_m5_api_probe"

NATIVE_VERSION_RE = re.compile(rb"^(Pkl|pkldoc) 0\.32\.0 \(([^,]+), Java [^\)]+\)\n$")
PKLDOC_WROTE_FILE_RE = re.compile(rb"^Wrote file file:[^\r\n]+$")
PKLDOC_LOADING_PKG_RE = re.compile(rb"^Loading package data for file:[^\r\n]+$")
PKLDOC_MIGRATED_RE = re.compile(rb"^Migrated ([0-9]+) packages \(([^\r\n]+)\)$")
VALID_COMPARISON_MODES = ("raw", "native-version", "pkldoc-progress")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def evidence_paths(directory: Path):
    for current, dirs, files in os.walk(directory, followlinks=False):
        dirs.sort()
        for name in dirs:
            p = Path(current) / name
            if p.is_symlink():
                raise ValueError(f"evidence directory contains symlink: {p}")
        for name in sorted(files):
            p = Path(current) / name
            if p.is_symlink() or not p.is_file():
                raise ValueError(f"evidence must contain regular files: {p}")
            if p != directory / "report.json":
                yield p


def capture_process(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    timeout: float,
    *,
    stdin_bytes: bytes | None = None,
) -> dict:
    """Capture process stdout/stderr/exit with POSIX process-group cleanup."""
    if os.name != "posix":
        raise RuntimeError("M5 differential runner requires POSIX process groups")
    before = time.perf_counter_ns()
    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.PIPE if stdin_bytes is not None else subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    timed_out = False
    try:
        try:
            stdout, stderr = proc.communicate(input=stdin_bytes, timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            try:
                stdout, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired as err:
                stdout, stderr = err.output or b"", err.stderr or b""
                proc.wait(timeout=5)
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": proc.returncode,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
    }


def build_case_env(
    base_env: dict[str, str],
    case_env: dict[str, str],
    state_dir: Path,
    *,
    prepend_path_dir: Path | None = None,
) -> dict[str, str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    home.mkdir(parents=True, exist_ok=True)
    tmp.mkdir(parents=True, exist_ok=True)
    env = {
        **base_env,
        **case_env,
        "HOME": str(home),
        "TMPDIR": str(tmp),
        "TMP": str(tmp),
        "TEMP": str(tmp),
        "PYTHONDONTWRITEBYTECODE": "1",
    }
    if prepend_path_dir is not None:
        env["PATH"] = str(prepend_path_dir) + os.pathsep + env.get("PATH", os.defpath)
    return env


def java_prefix_for_state(prefix: list[str], state_dir: Path, *, main_class: str | None = None) -> list[str]:
    home = state_dir / "home"
    tmp = state_dir / "tmp"
    out = []
    for arg in prefix:
        if arg.startswith("-Duser.home="):
            out.append(f"-Duser.home={home}")
        elif arg.startswith("-Djava.io.tmpdir="):
            out.append(f"-Djava.io.tmpdir={tmp}")
        else:
            out.append(arg)
    if main_class is not None:
        out[-1] = main_class
    return out


def expand_placeholders(text: str, mapping: dict[str, str]) -> str:
    out = text
    for k, v in mapping.items():
        out = out.replace("{" + k + "}", v)
    return out


def compile_api_probe_java(oracle_meta: dict, work_dir: Path) -> Path:
    """Compile tools/java/M5ApiOracleProbe.java once against pinned pkl-tools-0.32.0.jar."""
    classes = work_dir / "api_probe_classes"
    classes.mkdir(parents=True, exist_ok=True)
    java_home = Path(oracle_meta["jdk"]["properties"]["java.home"])
    javac = java_home / "bin/javac"
    env = dict(oracle_meta["environment"])
    tmp = work_dir / "javac_tmp"
    tmp.mkdir(exist_ok=True)
    env.update(TMPDIR=str(tmp), TMP=str(tmp), TEMP=str(tmp))
    proc = subprocess.run(
        [
            str(javac),
            "-encoding", "UTF-8",
            "-cp", oracle_meta["artifact"],
            "-d", str(classes),
            str(ROOT / API_PROBE_JAVA),
        ],
        cwd=work_dir,
        env=env,
        capture_output=True,
        timeout=60,
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"failed compiling {API_PROBE_JAVA} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')}"
        )
    return classes


def java_api_probe_prefix(oracle_meta: dict, classes_dir: Path, state_dir: Path) -> list[str]:
    base = java_prefix_for_state(oracle_meta["command"], state_dir)
    out = []
    skip_next = False
    for i, arg in enumerate(base[:-1]):
        if skip_next:
            skip_next = False
            continue
        if arg == "-cp" and i + 1 < len(base) - 1:
            out.extend(["-cp", f"{classes_dir}{os.pathsep}{base[i + 1]}"])
            skip_next = True
        else:
            out.append(arg)
    out.append("M5ApiOracleProbe")
    return out


def apply_case_setup(
    root: Path,
    setup: dict | None,
    work_dir: Path,
    output_dir: Path,
    cache_dir: Path,
    home_dir: Path,
    placeholders: dict[str, str],
) -> tuple[set[str], dict[str, bytes]]:
    declared_symlinks: set[str] = set()
    work_dir.mkdir(parents=True, exist_ok=True)
    output_dir.mkdir(parents=True, exist_ok=True)
    cache_dir.mkdir(parents=True, exist_ok=True)
    home_dir.mkdir(parents=True, exist_ok=True)
    if setup:
        for src_rel, dst_rel in setup.get("copy_dir", []):
            src = compat.relative_path(root, src_rel)
            dst = compat.relative_path(work_dir, dst_rel)
            shutil.copytree(src, dst, symlinks=True)
        for src_rel, dst_rel in setup.get("copy_file", []):
            src = compat.relative_path(root, src_rel)
            dst = compat.relative_path(work_dir, dst_rel)
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(src.read_bytes())
        for src_rel, dst_rel in setup.get("copy_to_output_dir", []):
            src = compat.relative_path(root, src_rel)
            dst = compat.relative_path(output_dir, dst_rel)
            if src.is_dir():
                shutil.copytree(src, dst, symlinks=True, dirs_exist_ok=True)
            else:
                dst.parent.mkdir(parents=True, exist_ok=True)
                dst.write_bytes(src.read_bytes())
        for rel_dir in setup.get("dirs", []):
            compat.relative_path(work_dir, rel_dir).mkdir(parents=True, exist_ok=True)
        for rel_file, tmpl in setup.get("templates", []):
            dst = compat.relative_path(work_dir, rel_file)
            dst.parent.mkdir(parents=True, exist_ok=True)
            dst.write_bytes(expand_placeholders(tmpl, placeholders).encode("utf-8"))
        for jar_rel, entries in setup.get("jars", []):
            jar_path = compat.relative_path(work_dir, jar_rel)
            jar_path.parent.mkdir(parents=True, exist_ok=True)
            with zipfile.ZipFile(jar_path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
                for entry_name, entry_text in sorted(entries.items()):
                    info = zipfile.ZipInfo(entry_name, date_time=(1980, 2, 1, 0, 0, 0))
                    zf.writestr(info, expand_placeholders(entry_text, placeholders).encode("utf-8"))
        for link_rel, target in setup.get("symlinks", []):
            link_path = compat.relative_path(work_dir, link_rel)
            link_path.parent.mkdir(parents=True, exist_ok=True)
            link_path.symlink_to(target)
            declared_symlinks.add(Path(link_rel).as_posix())
        if setup.get("seed_pkldoc_cache"):
            m5_pkldoc_tree.seed_pkldoc_package_cache(root, cache_dir)
    baseline = m4_differential.collect_tree_bytes(work_dir, declared_symlinks, prefix="")
    return declared_symlinks, baseline


def check_native_version_contract(expected: dict, actual: dict) -> list[str]:
    """Verify honest native-flavor --version contract without faking Java version or Apple brand.

    Java outputs `Pkl 0.32.0 (<os>, Java <ver>)\\n` or `pkldoc 0.32.0 (<os>, Java <ver>)\\n`;
    Rust MUST output `pkl-rs 0.32.0 (<os>, native)\\n` or `pkldoc-rs 0.32.0 (<os>, native)\\n`
    with exit code 0 and empty stderr.
    """
    diffs: list[str] = []
    if expected["exit_code"] != 0 or expected["stderr"] != b"":
        diffs.append("oracle_version_invalid")
    m = NATIVE_VERSION_RE.match(expected["stdout"])
    if not m:
        diffs.append("oracle_version_format_unexpected")
        return diffs
    tool_name, os_part = m.group(1), m.group(2)
    rust_tool_name = b"pkl-rs" if tool_name == b"Pkl" else b"pkldoc-rs"
    expected_native_stdout = rust_tool_name + b" 0.32.0 (" + os_part + b", native)\n"
    if actual["exit_code"] != 0:
        diffs.append("exit_code")
    if actual["stderr"] != b"":
        diffs.append("stderr")
    if actual["stdout"] != expected_native_stdout:
        diffs.append("stdout")
    if actual.get("timed_out"):
        diffs.append("timed_out")
    if actual.get("files"):
        diffs.append("files")
    return diffs


def _classify_pkldoc_cr_segment(seg: bytes) -> tuple[str, object]:
    if PKLDOC_WROTE_FILE_RE.match(seg):
        return ("wrote_file", seg)
    if PKLDOC_LOADING_PKG_RE.match(seg):
        return ("loading_package_data", seg)
    m = PKLDOC_MIGRATED_RE.match(seg)
    if m:
        count_bytes = m.group(1)
        count_val = int(count_bytes)
        if count_bytes == str(count_val).encode("ascii"):
            return ("migrated_packages", (count_val, m.group(2)))
    return ("other", seg)


def _group_cr_runs(segs: list[bytes]) -> list[tuple[str, list[object]]]:
    runs: list[tuple[str, list[object]]] = []
    for seg in segs:
        fam, payload = _classify_pkldoc_cr_segment(seg)
        if runs and runs[-1][0] == fam:
            runs[-1][1].append(payload)
        else:
            runs.append((fam, [payload]))
    return runs


def check_pkldoc_progress_stdout(expected_stdout: bytes, actual_stdout: bytes) -> tuple[bool, dict]:
    """Compare pkldoc stdout allowing only upstream nondeterministic \\r progress ordering.

    Upstream Java `DocGenerator.kt` (`newVirtualThreadPerTaskExecutor`) and `DocMigrator.kt`
    (`Files.walk`) emit \\r-separated progress updates in nondeterministic order. This contract:
      - Preserves and reports raw-byte equality (`raw_stdout_bytes_match`) honestly.
      - Never trims whitespace or normalizes paths/URI percent-encoding.
      - Requires every \\n-terminated non-\\r line and every final milestone segment after \\r
        to match raw byte-for-byte in exact order.
      - Compares `Wrote file file:...` and `Loading package data for file:...` \\r runs as
        exact raw-byte multisets (so URI encoding diffs like `%20`/`%7B%7D` FAIL).
      - Compares `Migrated <N> packages (<pkg>)` \\r runs for exact sequential numeric counts
        matching expected counts and exact package-name multiset.
      - Requires any other \\r segment outside those families to match in exact raw order.
    """
    raw_match = expected_stdout == actual_stdout
    exp_blocks = expected_stdout.split(b"\n")
    act_blocks = actual_stdout.split(b"\n")
    milestone_lines = 0
    wrote_file_segments = 0
    loading_package_data_segments = 0
    migrated_package_segments = 0
    other_cr_segments = 0
    reasons: list[str] = []

    if len(exp_blocks) != len(act_blocks):
        reasons.append(f"stdout_block_count:{len(exp_blocks)}!={len(act_blocks)}")
    else:
        for idx, (eb, ab) in enumerate(zip(exp_blocks, act_blocks)):
            e_has_cr = b"\r" in eb
            a_has_cr = b"\r" in ab
            if not e_has_cr and not a_has_cr:
                if eb != ab:
                    reasons.append(f"block[{idx}]:milestone_mismatch")
                else:
                    milestone_lines += 1
                continue
            if e_has_cr != a_has_cr:
                reasons.append(f"block[{idx}]:progress_burst_presence_mismatch")
                continue

            esegs = eb.split(b"\r")
            asegs = ab.split(b"\r")
            if esegs[-1] != asegs[-1]:
                reasons.append(f"block[{idx}]:final_milestone_mismatch")
            else:
                milestone_lines += 1
            if len(esegs) != len(asegs):
                reasons.append(f"block[{idx}]:cr_segment_count:{len(esegs)}!={len(asegs)}")
                continue

            e_runs = _group_cr_runs(esegs[:-1])
            a_runs = _group_cr_runs(asegs[:-1])
            if len(e_runs) != len(a_runs):
                reasons.append(f"block[{idx}]:progress_family_run_count_mismatch")
                continue

            for r_idx, ((efam, epayloads), (afam, apayloads)) in enumerate(zip(e_runs, a_runs)):
                if efam != afam or len(epayloads) != len(apayloads):
                    reasons.append(
                        f"block[{idx}].run[{r_idx}]:family_or_len_mismatch:{efam}({len(epayloads)})!={afam}({len(apayloads)})"
                    )
                    continue
                if efam == "wrote_file":
                    wrote_file_segments += len(epayloads)
                    if Counter(epayloads) != Counter(apayloads):
                        reasons.append(f"block[{idx}].run[{r_idx}]:wrote_file_multiset_mismatch")
                elif efam == "loading_package_data":
                    loading_package_data_segments += len(epayloads)
                    if Counter(epayloads) != Counter(apayloads):
                        reasons.append(f"block[{idx}].run[{r_idx}]:loading_package_data_multiset_mismatch")
                elif efam == "migrated_packages":
                    migrated_package_segments += len(epayloads)
                    ecounts = [c for c, _ in epayloads]
                    acounts = [c for c, _ in apayloads]
                    epkgs = [p for _, p in epayloads]
                    apkgs = [p for _, p in apayloads]
                    seq_ok = bool(ecounts) and all(c == ecounts[0] + i for i, c in enumerate(ecounts))
                    if not seq_ok or ecounts != acounts:
                        reasons.append(f"block[{idx}].run[{r_idx}]:migrated_count_sequence_mismatch")
                    if Counter(epkgs) != Counter(apkgs):
                        reasons.append(f"block[{idx}].run[{r_idx}]:migrated_package_multiset_mismatch")
                else:
                    other_cr_segments += len(epayloads)
                    if epayloads != apayloads:
                        reasons.append(f"block[{idx}].run[{r_idx}]:other_cr_order_mismatch")

    ok = len(reasons) == 0
    detail = {
        "raw_stdout_bytes_match": raw_match,
        "stdout_blocks": len(exp_blocks),
        "milestone_lines_matched": milestone_lines,
        "wrote_file_segments": wrote_file_segments,
        "loading_package_data_segments": loading_package_data_segments,
        "migrated_package_segments": migrated_package_segments,
        "other_cr_segments": other_cr_segments,
        "progress_order_differs": (not raw_match) and ok,
        "failure_reasons": reasons,
    }
    return ok, detail


def check_pkldoc_progress_contract(expected: dict, actual: dict) -> tuple[list[str], dict]:
    """Enforce raw-byte parity on exit_code, stderr, files/symlinks, and structured pkldoc stdout."""
    diffs: list[str] = []
    if expected["exit_code"] != actual["exit_code"]:
        diffs.append("exit_code")
    stdout_ok, detail = check_pkldoc_progress_stdout(expected["stdout"], actual["stdout"])
    if not stdout_ok:
        diffs.append("stdout")
    if expected["stderr"] != actual["stderr"]:
        diffs.append("stderr")
    if expected.get("timed_out") != actual.get("timed_out"):
        diffs.append("timed_out")
    if expected.get("workspace_changes") != actual.get("workspace_changes"):
        diffs.append("workspace_changes")
    exp_files = expected.get("files", {})
    act_files = actual.get("files", {})
    diffs.extend(m5_pkldoc_tree.compare_pkldoc_trees(exp_files, act_files, ROOT))
    return diffs, detail


def check_case_assertions(
    case: dict,
    raw: dict,
    lifecycle_log: Path,
    server_events: list[dict],
) -> list[str]:
    assertions = case.get("assertions") or {}
    failures: list[str] = []
    if case.get("exec_mode") == "server":
        if raw.get("protocol_errors"):
            failures.extend(f"protocol:{e}" for e in raw["protocol_errors"])
    if assertions.get("reader_closed"):
        if not lifecycle_log.is_file():
            failures.append("assertion:lifecycle_log_missing")
        else:
            evs = [json.loads(ln) for ln in lifecycle_log.read_text(encoding="utf-8").splitlines() if ln.strip()]
            starts = [e for e in evs if e.get("event") == "start"]
            closes = [e for e in evs if e.get("event") == "close"]
            pids = sorted({int(e["pid"]) for e in starts if "pid" in e})
            if not starts:
                failures.append("assertion:reader_never_started")
            if not closes:
                failures.append("assertion:reader_missing_close_0x32")
            for pid in pids:
                deadline = time.monotonic() + 1.0
                while m4_differential.pid_alive(pid) and time.monotonic() < deadline:
                    time.sleep(0.05)
                if m4_differential.pid_alive(pid):
                    failures.append(f"assertion:reader_orphan_pid_{pid}")
    if assertions.get("http_server_hit"):
        if not server_events:
            failures.append("assertion:http_server_hit_missing")
    if assertions.get("proxy_hit"):
        if not any(ev.get("server") == "proxy" for ev in server_events):
            failures.append("assertion:proxy_hit_missing")
    if assertions.get("min_generated_files") is not None:
        n_gen = sum(1 for k in raw.get("files", {}) if k.startswith("generated/"))
        if n_gen < int(assertions["min_generated_files"]):
            failures.append(f"assertion:min_generated_files({n_gen}<{assertions['min_generated_files']})")
    if assertions.get("min_symlinks") is not None:
        n_sym = sum(1 for k in raw.get("files", {}) if k.startswith("symlink/"))
        if n_sym < int(assertions["min_symlinks"]):
            failures.append(f"assertion:min_symlinks({n_sym}<{assertions['min_symlinks']})")
    if assertions.get("expect_symlinks"):
        for rel_sym, exp_target in assertions["expect_symlinks"].items():
            key = f"symlink/{rel_sym}"
            got = raw.get("files", {}).get(key)
            if got != exp_target.encode("utf-8"):
                failures.append(f"assertion:symlink_target({rel_sym}!={exp_target})")
    if assertions.get("expect_no_symlinks"):
        syms = [k for k in raw.get("files", {}) if k.startswith("symlink/")]
        if syms:
            failures.append(f"assertion:unexpected_symlinks({len(syms)})")
    if assertions.get("expect_stdout_sha256"):
        if digest(raw["stdout"]) != assertions["expect_stdout_sha256"]:
            failures.append("assertion:stdout_sha256_mismatch")
    if assertions.get("expect_golden_file"):
        golden_bytes = compat.relative_path(ROOT, assertions["expect_golden_file"]).read_bytes()
        if raw["stdout"] != golden_bytes:
            failures.append("assertion:golden_stdout_mismatch")
    return failures


def classify_result(
    case: dict,
    expected: dict,
    actual: dict | None,
    *,
    java_assertion_failures: list[str] | None = None,
    rust_assertion_failures: list[str] | None = None,
    idempotency_ok: bool | None = None,
) -> tuple[str, list[str]]:
    if expected["timed_out"]:
        return "oracle-error", ["oracle_timed_out"]
    if expected["exit_code"] not in case["expected_exit"]:
        return "oracle-contract-mismatch", [f"oracle_exit_{expected['exit_code']}"]
    if java_assertion_failures:
        return "oracle-assertion-failed", list(java_assertion_failures)
    if actual is None:
        return "oracle-recorded", []
    if actual["timed_out"]:
        return "candidate-timeout", ["candidate_timed_out"]
    if rust_assertion_failures:
        return "mismatch", list(rust_assertion_failures)

    comp_mode = case.get("comparison_mode", "raw")
    if comp_mode == "native-version":
        diffs = check_native_version_contract(expected, actual)
        return ("mismatch", diffs) if diffs else ("pass", [])
    if comp_mode == "pkldoc-progress":
        diffs, _ = check_pkldoc_progress_contract(expected, actual)
        return ("mismatch", diffs) if diffs else ("pass", [])

    diffs = compat.differences(expected, actual)
    if idempotency_ok is False:
        diffs.append("format-not-idempotent")
    return ("mismatch", diffs) if diffs else ("pass", [])


# ---------------------------------------------------------------------------
# Dynamic Corpus Formatter-Snippet Cases (30 goldens from compatibility/corpus.json)
# ---------------------------------------------------------------------------

def enumerate_formatter_snippet_cases(root: Path) -> list[dict]:
    corpus = json.loads((root / CORPUS_PATH).read_bytes())
    cases: list[dict] = []
    for entry in corpus["cases"]:
        if entry.get("suite") != "formatter-snippet":
            continue
        in_rel = "external/pkl/" + entry["source"]["path"]
        out_rel = in_rel.replace("/input/", "/output/", 1)
        in_path = root / in_rel
        out_path = root / out_rel
        if not in_path.is_file() or not out_path.is_file():
            raise ValueError(f"missing formatter-snippet input/output pair: {in_rel} / {out_rel}")
        in_bytes = in_path.read_bytes()
        out_bytes = out_path.read_bytes()
        expected_exit = [0] if in_bytes == out_bytes else [11]
        name = Path(in_rel).name
        cases.append({
            "id": f"m5:format:snippet:{name}",
            "lane": "formatter",
            "exec_mode": "format_snippet",
            "corpus_case_id": entry["id"],
            "input_path": in_rel,
            "input_sha256": digest(in_bytes),
            "golden_path": out_rel,
            "golden_sha256": digest(out_bytes),
            "expected_exit": expected_exit,
            "comparison_mode": "raw",
            "assertions": {"expect_golden_file": out_rel},
        })
    cases.sort(key=lambda c: c["id"])
    if len(cases) != 30:
        raise ValueError(f"expected 30 formatter-snippet cases in corpus.json, got {len(cases)}")
    return cases


# ---------------------------------------------------------------------------
# Case Execution
# ---------------------------------------------------------------------------

def run_case(
    root: Path,
    case: dict,
    output: Path,
    oracle_meta: dict,
    api_probe_classes: Path | None,
    servers: m4_differential.M4FixtureServers | None,
    candidates: dict[str, Path | None],
    timeout: float,
) -> dict:
    cdir_name = compat.case_directory(case["id"])
    folder = output / "cases" / cdir_name
    folder.mkdir(parents=True)
    work_root = output / "_work" / cdir_name
    work_root.mkdir(parents=True)

    case_state = work_root / "state"
    sandbox = work_root / "sandbox"
    case_gen = sandbox / "generated"
    case_work = sandbox / "work"
    case_cache = sandbox / "cache"
    lifecycle_log = sandbox / "ext_lifecycle.jsonl"

    placeholders = {
        "output_dir": str(case_gen),
        "work_dir": str(case_work),
        "cache_dir": str(case_cache),
        "python": sys.executable,
        "reader_bin": str((root / M4_READER_BIN).resolve()),
        "lifecycle_log": str(lifecycle_log),
        "root": str(root),
    }
    if servers is not None:
        placeholders.update({
            "http_port": str(servers.http_port),
            "https_port": str(servers.https_port),
            "proxy_port": str(servers.proxy_port),
            "ca_cert": str(servers.ca_path),
        })

    exec_mode = case["exec_mode"]
    setup = case.get("setup")
    collect_mode = case.get("collect_files", "none")

    def _select_candidate() -> Path | None:
        if exec_mode == "api_probe":
            return candidates.get("api_probe")
        if exec_mode == "pkldoc":
            return candidates.get("pkldoc")
        return candidates.get("pkl")

    def _run_side(is_java: bool, cand_bin: Path | None) -> tuple[list[str], dict, list[str], list[dict], bool | None]:
        shutil.rmtree(case_state, ignore_errors=True)
        shutil.rmtree(sandbox, ignore_errors=True)
        declared_symlinks, work_baseline = apply_case_setup(
            root, setup, case_work, case_gen, case_cache, case_state / "home", placeholders
        )
        env = build_case_env(oracle_meta["environment"], case.get("environment", {}), case_state)
        cwd_raw = expand_placeholders(case.get("cwd", "."), placeholders)
        cwd = Path(cwd_raw)
        if not cwd.is_absolute():
            cwd = (root / cwd).resolve()

        log_off = servers.log_offset() if servers is not None else 0
        idempotency_ok: bool | None = None

        if exec_mode == "format_snippet":
            in_file = str((root / case["input_path"]).resolve())
            golden_bytes = (root / case["golden_path"]).read_bytes()
            args = ["format", in_file]
            cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *args] if is_java else [str(cand_bin), *args]
            raw = capture_process(cmd, cwd, env, timeout)
            # 2nd-pass idempotency check via stdin `-` on formatted golden bytes
            idem_args = ["format", "-"]
            idem_cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *idem_args] if is_java else [str(cand_bin), *idem_args]
            idem_raw = capture_process(idem_cmd, cwd, env, timeout, stdin_bytes=golden_bytes)
            idempotency_ok = (
                not idem_raw["timed_out"]
                and idem_raw["exit_code"] == 0
                and idem_raw["stderr"] == b""
                and idem_raw["stdout"] == golden_bytes
            )

        elif exec_mode == "cli":
            args = [expand_placeholders(a, placeholders) for a in case["command"]]
            cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *args] if is_java else [str(cand_bin), *args]
            stdin_bytes = None
            if "stdin_utf8" in case:
                stdin_bytes = expand_placeholders(case["stdin_utf8"], placeholders).encode("utf-8")
            elif "stdin_file" in case:
                stdin_bytes = compat.relative_path(root, case["stdin_file"]).read_bytes()
            raw = capture_process(cmd, cwd, env, timeout, stdin_bytes=stdin_bytes)

        elif exec_mode == "api_probe":
            assert api_probe_classes is not None
            args = [expand_placeholders(a, placeholders) for a in case["command"]]
            cmd = [*java_api_probe_prefix(oracle_meta, api_probe_classes, case_state), *args] if is_java else [str(cand_bin), *args]
            raw = capture_process(cmd, cwd, env, timeout)

        elif exec_mode == "repl_pipe":
            args = [expand_placeholders(a, placeholders) for a in case["command"]]
            cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *args] if is_java else [str(cand_bin), *args]
            raw = m5_repl_driver.run_repl_pipe(cmd, cwd, env, case["lines"], placeholders, timeout=timeout)

        elif exec_mode == "repl_pty":
            args = [expand_placeholders(a, placeholders) for a in case["command"]]
            cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *args] if is_java else [str(cand_bin), *args]
            raw = m5_repl_driver.run_repl_pty(cmd, cwd, env, case["interactions"], placeholders, timeout=timeout)

        elif exec_mode == "server":
            args = [expand_placeholders(a, placeholders) for a in case.get("command", ["server"])]
            cmd = [*java_prefix_for_state(oracle_meta["command"], case_state), *args] if is_java else [str(cand_bin), *args]
            raw = m5_server_client.run_server_session(cmd, cwd, env, case["scenario"], placeholders, timeout=timeout)

        elif exec_mode == "pkldoc":
            steps = case.get("steps") or [{"command": case["command"]}]
            all_stdout = bytearray()
            all_stderr = bytearray()
            total_elapsed = 0
            last_rc = 0
            timed_out = False
            last_cmd: list[str] = []
            for step in steps:
                s_args = [expand_placeholders(a, placeholders) for a in step["command"]]
                # Expand glob_dir entries (e.g. "glob_pkl:<rel_dir>") into sorted file paths!
                expanded_args: list[str] = []
                for a in s_args:
                    if a.startswith("glob_pkl:"):
                        gdir = compat.relative_path(root, a[len("glob_pkl:"):])
                        expanded_args.extend(str(p.resolve()) for p in sorted(gdir.rglob("*.pkl")) if p.is_file())
                    elif a.startswith("glob_all:"):
                        gdir = compat.relative_path(root, a[len("glob_all:"):])
                        expanded_args.extend(str(p.resolve()) for p in sorted(gdir.rglob("*")) if p.is_file())
                    else:
                        expanded_args.append(a)
                last_cmd = (
                    [*java_prefix_for_state(oracle_meta["command"], case_state, main_class="org.pkl.doc.Main"), *expanded_args]
                    if is_java
                    else [str(cand_bin), *expanded_args]
                )
                step_raw = capture_process(last_cmd, cwd, env, timeout)
                all_stdout.extend(step_raw["stdout"])
                all_stderr.extend(step_raw["stderr"])
                total_elapsed += step_raw["elapsed_ns"]
                last_rc = step_raw["exit_code"]
                timed_out = timed_out or step_raw["timed_out"]
                if last_rc != 0 or timed_out:
                    break
            cmd = last_cmd
            raw = {
                "stdout": bytes(all_stdout),
                "stderr": bytes(all_stderr),
                "exit_code": last_rc,
                "files": {},
                "timed_out": timed_out,
                "workspace_changes": {},
                "elapsed_ns": total_elapsed,
            }

        else:
            raise ValueError(f"unknown exec_mode for {case['id']}: {exec_mode}")

        server_events = servers.read_log_since(log_off) if servers is not None else []

        collected: dict[str, bytes] = {}
        if collect_mode == "output_dir":
            collected = m4_differential.collect_tree_bytes(case_gen, set(), prefix="generated/")
        elif collect_mode == "pkldoc_tree":
            collected = m5_pkldoc_tree.collect_pkldoc_tree(case_gen)
        elif collect_mode == "work_dir_changes":
            after = m4_differential.collect_tree_bytes(case_work, declared_symlinks, prefix="")
            for rel, data in after.items():
                if work_baseline.get(rel) != data:
                    collected[f"work/{rel}"] = data
        elif collect_mode == "repl_history":
            hist = case_state / "home/.pkl/repl-history"
            if hist.is_file():
                collected["home/.pkl/repl-history"] = hist.read_bytes()
        raw["files"] = dict(sorted(collected.items()))
        raw["workspace_changes"] = {}

        afails = check_case_assertions(case, raw, lifecycle_log, server_events)
        if exec_mode == "format_snippet" and idempotency_ok is False:
            afails.append("assertion:format_second_pass_not_idempotent")
        return cmd, raw, afails, server_events, idempotency_ok

    java_cmd, java_raw, java_afails, java_events, java_idem = _run_side(True, None)
    java_stored = compat.store_capture(folder / "java", java_raw)

    cand_bin = _select_candidate()
    rust_cmd = None
    rust_raw = None
    rust_stored = None
    rust_afails: list[str] = []
    rust_events: list[dict] = []
    rust_idem: bool | None = None
    if cand_bin is not None:
        rust_cmd, rust_raw, rust_afails, rust_events, rust_idem = _run_side(False, cand_bin)
        rust_stored = compat.store_capture(folder / "rust", rust_raw)

    idem_overall = rust_idem if rust_raw is not None else java_idem
    status, diffs = classify_result(
        case,
        java_raw,
        rust_raw,
        java_assertion_failures=java_afails,
        rust_assertion_failures=rust_afails,
        idempotency_ok=idem_overall,
    )
    raw_stdout_bytes_match: bool | None = None
    pkldoc_progress_detail: dict | None = None
    if case.get("comparison_mode") == "pkldoc-progress" and rust_raw is not None:
        _, pkldoc_progress_detail = check_pkldoc_progress_contract(java_raw, rust_raw)
        raw_stdout_bytes_match = pkldoc_progress_detail["raw_stdout_bytes_match"]
    shutil.rmtree(work_root, ignore_errors=True)

    return {
        "id": case["id"],
        "lane": case["lane"],
        "exec_mode": exec_mode,
        "directory": cdir_name,
        "status": status,
        "differences": diffs,
        "expected_exit": case["expected_exit"],
        "comparison_mode": case.get("comparison_mode", "raw"),
        "raw_stdout_bytes_match": raw_stdout_bytes_match,
        "pkldoc_progress_detail": pkldoc_progress_detail,
        "java_assertion_failures": java_afails,
        "rust_assertion_failures": rust_afails,
        "java_server_events_count": len(java_events),
        "rust_server_events_count": len(rust_events),
        "idempotency_ok": idem_overall,
        "pkldoc_tree_summary": m5_pkldoc_tree.summarize_pkldoc_tree(java_raw["files"]) if collect_mode == "pkldoc_tree" else None,
        "java": {"command": java_cmd, "result": java_stored},
        "rust": None if rust_stored is None else {"command": rust_cmd, "result": rust_stored},
    }


# ---------------------------------------------------------------------------
# Candidate Build, Source Snapshot, Validation & Tamper-Proof Verifier
# ---------------------------------------------------------------------------

def snapshot_rust_sources(root: Path) -> dict[str, str]:
    """Hash all Rust workspace sources, assets, examples, and Cargo manifests/lockfile."""
    files: dict[str, str] = {}
    for rel in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml"):
        p = root / rel
        if p.is_file():
            files[rel] = oracle.sha256(p)
    for top in ("crates", "vendor"):
        base = root / top
        if not base.is_dir():
            continue
        for crate_dir in sorted(base.iterdir()):
            if not crate_dir.is_dir() or crate_dir.is_symlink():
                continue
            toml = crate_dir / "Cargo.toml"
            if toml.is_file():
                files[toml.relative_to(root).as_posix()] = oracle.sha256(toml)
            for sub in ("src", "assets", "examples"):
                sub_dir = crate_dir / sub
                if not sub_dir.is_dir():
                    continue
                for current, dirs, names in os.walk(sub_dir, followlinks=False):
                    dirs.sort()
                    for name in sorted(names):
                        p = Path(current) / name
                        if p.is_symlink() or not p.is_file():
                            continue
                        files[p.relative_to(root).as_posix()] = oracle.sha256(p)
    return dict(sorted(files.items()))


def build_bin(root: Path, package: str, bin_name: str, target_dir: Path, output: Path) -> tuple[Path, dict]:
    cmd = [
        "cargo", "build", "--release", "-p", package,
        "--bin", bin_name, "--locked", "--target-dir", str(target_dir),
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, timeout=600)
    (output / f"build-{bin_name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{bin_name}.stderr.bin").write_bytes(proc.stderr)
    rec = {
        "command": cmd,
        "package": package,
        "bin": bin_name,
        "exit_code": proc.returncode,
        "stdout_sha256": digest(proc.stdout),
        "stderr_sha256": digest(proc.stderr),
    }
    if proc.returncode != 0:
        raise ValueError(
            f"failed building {package}::{bin_name} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')[-1200:]}"
        )
    built = target_dir / "release" / bin_name
    if not built.is_file():
        raise ValueError(f"built binary missing: {built}")
    return built, rec


def build_example(root: Path, package: str, example_name: str, target_dir: Path, output: Path) -> tuple[Path, dict]:
    cmd = [
        "cargo", "build", "--release", "-p", package,
        "--example", example_name, "--locked", "--target-dir", str(target_dir),
    ]
    proc = subprocess.run(cmd, cwd=root, capture_output=True, timeout=600)
    (output / f"build-{example_name}.stdout.bin").write_bytes(proc.stdout)
    (output / f"build-{example_name}.stderr.bin").write_bytes(proc.stderr)
    rec = {
        "command": cmd,
        "package": package,
        "example": example_name,
        "exit_code": proc.returncode,
        "stdout_sha256": digest(proc.stdout),
        "stderr_sha256": digest(proc.stderr),
    }
    if proc.returncode != 0:
        raise ValueError(
            f"failed building {package} example {example_name} (rc={proc.returncode}): "
            f"{proc.stderr.decode('utf-8', errors='replace')[-1200:]}"
        )
    built = target_dir / "release/examples" / example_name
    if not built.is_file():
        raise ValueError(f"built example missing: {built}")
    return built, rec


def freeze_artifact(artifacts_dir: Path, name: str, path: Path, source_binding: str, build_rec: dict | None) -> tuple[Path, dict]:
    frozen = artifacts_dir / name
    frozen.write_bytes(path.read_bytes())
    frozen.chmod(0o755)
    return frozen, {
        "name": name,
        "original_path": str(path),
        "sha256": oracle.sha256(frozen),
        "source_binding": source_binding,
        "build": build_rec,
    }


def validate_static_cases(root: Path, cases_doc: dict) -> dict[str, dict]:
    if cases_doc.get("schema_version") != 1 or not isinstance(cases_doc.get("cases"), list):
        raise ValueError("invalid tests/fixtures/m5/cases.json schema")
    valid_modes = {"cli", "api_probe", "repl_pipe", "repl_pty", "server", "pkldoc"}
    by_id: dict[str, dict] = {}
    for case in cases_doc["cases"]:
        cid = case["id"]
        if cid in by_id:
            raise ValueError(f"duplicate M5 case id: {cid}")
        if case.get("lane") not in ALL_LANES:
            raise ValueError(f"unknown M5 lane for {cid}: {case.get('lane')}")
        if case.get("exec_mode") not in valid_modes:
            raise ValueError(f"unknown M5 exec_mode for {cid}: {case.get('exec_mode')}")
        if case.get("comparison_mode", "raw") not in VALID_COMPARISON_MODES:
            raise ValueError(f"unknown M5 comparison_mode for {cid}: {case.get('comparison_mode')}")
        for inp in case.get("inputs", []):
            rel = inp["path"]
            p = Path(rel)
            if p.is_absolute() or ".." in p.parts:
                raise ValueError(f"invalid M5 fixture input path: {rel}")
            full = root / p
            if not full.is_file():
                raise ValueError(f"missing M5 fixture input: {rel}")
            if digest(full.read_bytes()) != inp["sha256"]:
                raise ValueError(f"M5 fixture input sha256 mismatch: {rel}")
        by_id[cid] = case
    return by_id


def required_candidate_roles(cases: list[dict]) -> list[str]:
    roles: set[str] = set()
    for c in cases:
        mode = c["exec_mode"]
        if mode == "api_probe":
            roles.add("api_probe")
        elif mode == "pkldoc":
            roles.add("pkldoc")
        else:
            roles.add("pkl")
    return sorted(roles)


def execute(
    root: Path,
    output: Path,
    lanes: list[str],
    timeout: float,
    java_only: bool,
    candidate_pkl: Path | None,
    candidate_pkldoc: Path | None,
    candidate_api_probe: Path | None,
    cargo_target_dir: Path | None,
) -> dict:
    cases_doc = json.loads((root / CASES_PATH).read_bytes())
    validate_static_cases(root, cases_doc)
    selected_static = [c for c in cases_doc["cases"] if c["lane"] in lanes]
    dynamic_cases = enumerate_formatter_snippet_cases(root) if "formatter" in lanes else []
    all_selected = selected_static + dynamic_cases

    if output.exists() or output.is_symlink():
        raise ValueError(f"output directory must be fresh (does not exist): {output}")

    oracle_meta = oracle.ensure_oracle(root, fetch=False)
    if not oracle_meta.get("exact_jdk_lock_match", False):
        raise ValueError("pinned oracle JDK lock mismatch (exact_jdk_lock_match is false)")

    sources_before = snapshot_rust_sources(root)

    output.mkdir(parents=True, exist_ok=False)
    artifacts = output / "artifacts"
    artifacts.mkdir()
    compat.write_json(output / "oracle.json", oracle_meta)
    (output / "cases.snapshot.json").write_bytes((root / CASES_PATH).read_bytes())
    compat.write_json(output / "dynamic-cases.snapshot.json", {"schema_version": 1, "cases": dynamic_cases})

    needed_roles = required_candidate_roles(all_selected)
    candidates: dict[str, Path | None] = {"pkl": None, "pkldoc": None, "api_probe": None}
    candidate_bindings: dict[str, dict] | None = None

    if not java_only:
        candidate_bindings = {}
        tdir = cargo_target_dir.resolve() if cargo_target_dir else (root / "target")
        if "pkl" in needed_roles:
            if candidate_pkl is not None:
                candidates["pkl"], candidate_bindings["pkl"] = freeze_artifact(
                    artifacts, candidate_pkl.name, candidate_pkl.resolve(), "unbound-caller-supplied", None
                )
            else:
                built, rec = build_bin(root, DEFAULT_PKL_PACKAGE, DEFAULT_PKL_BIN, tdir, output)
                candidates["pkl"], candidate_bindings["pkl"] = freeze_artifact(
                    artifacts, DEFAULT_PKL_BIN, built, "recorded-build", rec
                )
        if "pkldoc" in needed_roles:
            if candidate_pkldoc is not None:
                candidates["pkldoc"], candidate_bindings["pkldoc"] = freeze_artifact(
                    artifacts, candidate_pkldoc.name, candidate_pkldoc.resolve(), "unbound-caller-supplied", None
                )
            else:
                built, rec = build_bin(root, DEFAULT_PKLDOC_PACKAGE, DEFAULT_PKLDOC_BIN, tdir, output)
                candidates["pkldoc"], candidate_bindings["pkldoc"] = freeze_artifact(
                    artifacts, DEFAULT_PKLDOC_BIN, built, "recorded-build", rec
                )
        if "api_probe" in needed_roles:
            if candidate_api_probe is not None:
                candidates["api_probe"], candidate_bindings["api_probe"] = freeze_artifact(
                    artifacts, candidate_api_probe.name, candidate_api_probe.resolve(), "unbound-caller-supplied", None
                )
            else:
                built, rec = build_example(root, DEFAULT_API_PROBE_PACKAGE, DEFAULT_API_PROBE_EXAMPLE, tdir, output)
                candidates["api_probe"], candidate_bindings["api_probe"] = freeze_artifact(
                    artifacts, DEFAULT_API_PROBE_EXAMPLE, built, "recorded-build", rec
                )

    probe_work = output / "_probe_compile"
    api_probe_classes: Path | None = None
    if any(c["exec_mode"] == "api_probe" for c in all_selected):
        probe_work.mkdir()
        api_probe_classes = compile_api_probe_java(oracle_meta, probe_work)

    need_http = any(c.get("requires_http_server") for c in all_selected)
    server_self_test: dict | None = None
    results: list[dict] = []

    if need_http:
        with m4_differential.M4FixtureServers(
            oracle_meta,
            root / "tests/fixtures/m4/host_io",
            root / "tests/fixtures/m4/projects/registry",
            output / "_server",
        ) as servers:
            server_self_test = servers.self_test()
            if not server_self_test["passed"]:
                raise RuntimeError("M4HttpFixtureServer self-test failed: " + json.dumps(server_self_test))
            for case in all_selected:
                results.append(run_case(root, case, output, oracle_meta, api_probe_classes, servers, candidates, timeout))
        shutil.rmtree(output / "_server", ignore_errors=True)
    else:
        for case in all_selected:
            results.append(run_case(root, case, output, oracle_meta, api_probe_classes, None, candidates, timeout))

    shutil.rmtree(probe_work, ignore_errors=True)
    shutil.rmtree(output / "_work", ignore_errors=True)

    sources_after = snapshot_rust_sources(root)
    source_stability = sources_before == sources_after
    compat.write_json(output / "sources.snapshot.json", {"before": sources_before, "after": sources_after})

    by_status = dict(sorted(Counter(r["status"] for r in results).items()))
    by_lane_status = {
        lane: dict(sorted(Counter(r["status"] for r in results if r["lane"] == lane).items()))
        for lane in lanes
    }
    report = {
        "schema_version": 1,
        "baseline": cases_doc["baseline"],
        "mode": "java-only-oracle-recording" if java_only else "differential",
        "cases_manifest_sha256": oracle.sha256(root / CASES_PATH),
        "dynamic_cases_sha256": oracle.sha256(output / "dynamic-cases.snapshot.json"),
        "api_probe_java_sha256": oracle.sha256(root / API_PROBE_JAVA),
        "sources_snapshot_sha256": oracle.sha256(output / "sources.snapshot.json"),
        "sources_file_count": len(sources_before),
        "source_stability": source_stability,
        "lanes": lanes,
        "required_candidate_roles": needed_roles,
        "timeout_seconds": timeout,
        "oracle": oracle_meta,
        "candidates": candidate_bindings,
        "server_self_test": server_self_test,
        "selected_count": len(results),
        "status_counts": by_status,
        "lane_status_counts": by_lane_status,
        "results": results,
    }
    report["evidence_sha256"] = {
        str(p.relative_to(output).as_posix()): oracle.sha256(p) for p in sorted(evidence_paths(output))
    }
    compat.write_json(output / "report.json", report)
    verify_report(output, strict=False)
    return report


def verify_report(directory: Path, *, strict: bool = False) -> dict:
    report = json.loads((directory / "report.json").read_bytes())
    if report.get("schema_version") != 1:
        raise ValueError("unsupported M5 differential report schema_version")
    if json.loads((directory / "oracle.json").read_bytes()) != report["oracle"]:
        raise ValueError("oracle.json differs from report.json")

    recorded_evidence = report.get("evidence_sha256", {})
    actual_files = {str(p.relative_to(directory).as_posix()) for p in evidence_paths(directory)}
    if not recorded_evidence or set(recorded_evidence) != actual_files:
        raise ValueError("evidence file set differs from recorded evidence_sha256")
    for rel, expected_sha in recorded_evidence.items():
        if oracle.sha256(directory / rel) != expected_sha:
            raise ValueError(f"evidence file modified: {rel}")

    if oracle.sha256(directory / "cases.snapshot.json") != report.get("cases_manifest_sha256"):
        raise ValueError("cases.snapshot.json sha256 disagrees with cases_manifest_sha256")
    if oracle.sha256(directory / "dynamic-cases.snapshot.json") != report.get("dynamic_cases_sha256"):
        raise ValueError("dynamic-cases.snapshot.json sha256 disagrees with dynamic_cases_sha256")

    cands = report.get("candidates")
    if cands is not None:
        if not isinstance(cands, dict):
            raise ValueError("candidates metadata must be an object or null")
        for role, info in cands.items():
            if not isinstance(info, dict):
                raise ValueError(f"candidate entry for {role} must be an object")
            cname = info.get("name")
            if not isinstance(cname, str) or not cname or "/" in cname or "\\" in cname or cname in (".", ".."):
                raise ValueError(f"invalid candidate artifact name for {role}: {cname!r}")
            rel = f"artifacts/{cname}"
            if rel not in recorded_evidence or info.get("sha256") != recorded_evidence[rel]:
                raise ValueError(f"candidate artifact sha256 mismatch for {role}: {rel}")
            build_rec = info.get("build")
            if isinstance(build_rec, dict):
                target_name = build_rec.get("bin") or build_rec.get("example")
                if isinstance(target_name, str) and target_name:
                    for stream, key in (("stdout", "stdout_sha256"), ("stderr", "stderr_sha256")):
                        brel = f"build-{target_name}.{stream}.bin"
                        if brel not in recorded_evidence or build_rec.get(key) != recorded_evidence[brel]:
                            raise ValueError(f"candidate build {stream} sha256 mismatch for {role}: {brel}")

    static_doc = json.loads((directory / "cases.snapshot.json").read_bytes())
    dynamic_doc = json.loads((directory / "dynamic-cases.snapshot.json").read_bytes())
    cases_by_id = {c["id"]: c for c in static_doc["cases"] + dynamic_doc["cases"]}
    expected_ids = [c["id"] for c in static_doc["cases"] if c["lane"] in report["lanes"]] + [
        c["id"] for c in dynamic_doc["cases"] if c["lane"] in report["lanes"]
    ]
    result_ids = [r["id"] for r in report["results"]]
    if result_ids != expected_ids or len(result_ids) != report["selected_count"]:
        raise ValueError("result list order or membership differs from selected lanes")

    for row in report["results"]:
        case = cases_by_id[row["id"]]
        expected_dir = compat.case_directory(row["id"])
        if row["directory"] != expected_dir:
            raise ValueError(f"unexpected capture directory for {row['id']}")
        folder = directory / "cases" / expected_dir
        java_cap = compat.load_capture(folder / "java")
        rust_cap = compat.load_capture(folder / "rust") if row["rust"] is not None else None

        if row.get("comparison_mode", "raw") != case.get("comparison_mode", "raw"):
            raise ValueError(
                f"recorded comparison_mode disagrees with case for {row['id']}: "
                f"{row.get('comparison_mode')!r} vs {case.get('comparison_mode', 'raw')!r}"
            )
        status, diffs = classify_result(
            case,
            java_cap,
            rust_cap,
            java_assertion_failures=row.get("java_assertion_failures"),
            rust_assertion_failures=row.get("rust_assertion_failures"),
            idempotency_ok=row.get("idempotency_ok"),
        )
        if row["status"] != status or row["differences"] != diffs:
            raise ValueError(f"recomputed status/differences disagree for {row['id']}: {row['status']} vs {status}")
        if case.get("comparison_mode") == "pkldoc-progress" and rust_cap is not None:
            _, expected_detail = check_pkldoc_progress_contract(java_cap, rust_cap)
            expected_raw_match = java_cap["stdout"] == rust_cap["stdout"]
            recorded_raw_match = row.get("raw_stdout_bytes_match")
            if type(recorded_raw_match) is not bool or recorded_raw_match != expected_raw_match:
                raise ValueError(
                    f"recomputed raw_stdout_bytes_match disagrees for {row['id']}: "
                    f"{recorded_raw_match!r} vs {expected_raw_match!r}"
                )
            if row.get("pkldoc_progress_detail") != expected_detail:
                raise ValueError(
                    f"recomputed pkldoc_progress_detail disagrees for {row['id']}"
                )

    recomputed = dict(sorted(Counter(r["status"] for r in report["results"]).items()))
    if recomputed != report["status_counts"]:
        raise ValueError("recomputed status_counts disagree with report.json")
    recomputed_lanes = {
        lane: dict(sorted(Counter(r["status"] for r in report["results"] if r["lane"] == lane).items()))
        for lane in report["lanes"]
    }
    if recomputed_lanes != report["lane_status_counts"]:
        raise ValueError("recomputed lane_status_counts disagree with report.json")

    if "sources_snapshot_sha256" in report:
        src_snap = directory / "sources.snapshot.json"
        if not src_snap.is_file() or oracle.sha256(src_snap) != report["sources_snapshot_sha256"]:
            raise ValueError("sources.snapshot.json missing or sha256 disagrees with report.json")
        snap_doc = json.loads(src_snap.read_bytes())
        if (snap_doc.get("before") == snap_doc.get("after")) != report.get("source_stability"):
            raise ValueError("recomputed source_stability disagrees with report.json")

    if strict:
        if not report["oracle"].get("exact_jdk_lock_match", False):
            raise ValueError("strict gate requires exact_jdk_lock_match == true")
        if cands is None:
            raise ValueError("strict gate requires recorded-build Rust candidates (java-only recordings cannot pass)")
        for role in report.get("required_candidate_roles", []):
            if role not in cands or cands[role].get("source_binding") != "recorded-build":
                raise ValueError(f"strict gate requires recorded-build candidate for role: {role}")
        if "sources_snapshot_sha256" not in report or report.get("source_stability") is not True:
            raise ValueError("strict gate requires sources_snapshot_sha256 and source_stability == true")
        non_pass = [r for r in report["results"] if r["status"] != "pass"]
        if non_pass:
            summary = ", ".join(f"{r['id']}={r['status']}" for r in non_pass[:10])
            raise ValueError(f"strict gate failed: {len(non_pass)} non-pass case(s): {summary}")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    action = parser.add_mutually_exclusive_group(required=True)
    action.add_argument("--output", type=Path, help="fresh evidence directory for live run")
    action.add_argument("--verify-report", type=Path, help="verify integrity only (preserves recorded failures)")
    action.add_argument("--verify-strict", type=Path, help="verify integrity AND require 100%% pass + recorded-build + exact JDK lock")
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--lane", action="append", choices=ALL_LANES, help="select specific lane(s); default runs all 5")
    parser.add_argument("--timeout", type=float, default=DEFAULT_TIMEOUT)
    parser.add_argument("--java-only", action="store_true", help="record pinned Java 0.32.0 oracle baseline only (never counts as pass)")
    parser.add_argument("--record-only", action="store_true", help="exit 0 after integrity verification even with mismatches")
    parser.add_argument("--candidate-pkl", type=Path, help="caller-supplied `pkl` binary (unbound)")
    parser.add_argument("--candidate-pkldoc", type=Path, help="caller-supplied `pkldoc` binary (unbound)")
    parser.add_argument("--candidate-api-probe", type=Path, help="caller-supplied `pkl_m5_api_probe` binary (unbound)")
    parser.add_argument("--cargo-target-dir", type=Path, help="Cargo target directory for recorded builds")
    args = parser.parse_args()

    try:
        if args.output:
            lanes = list(dict.fromkeys(args.lane)) if args.lane else list(ALL_LANES)
            report = execute(
                args.root.resolve(),
                args.output.resolve(),
                lanes,
                args.timeout,
                args.java_only,
                args.candidate_pkl,
                args.candidate_pkldoc,
                args.candidate_api_probe,
                args.cargo_target_dir,
            )
            print(json.dumps({
                "mode": report["mode"],
                "selected_count": report["selected_count"],
                "status_counts": report["status_counts"],
                "lane_status_counts": report["lane_status_counts"],
            }, sort_keys=True))
            if args.record_only or args.java_only:
                return 0
            verify_report(args.output.resolve(), strict=True)
            return 0
        target = (args.verify_report or args.verify_strict).resolve()
        report = verify_report(target, strict=bool(args.verify_strict))
        print(json.dumps({
            "mode": report["mode"],
            "selected_count": report["selected_count"],
            "status_counts": report["status_counts"],
            "lane_status_counts": report["lane_status_counts"],
        }, sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"M5 differential error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
