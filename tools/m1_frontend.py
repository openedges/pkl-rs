#!/usr/bin/env python3
"""Byte-exact bounded frontend/VM differential experiment, not product compatibility."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys

import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = Path("tests/fixtures/m1/frontend")
CASES = (
    ("unicode-positions", "positions", "unicode-crlf.pkl"),
    ("unicode-parser", "parse", "unicode-crlf.pkl"),
    ("first-error", "parse", "first-error.pkl"),
    ("unicode-error", "parse", "unicode-error.pkl"),
    ("unexpected-eof", "parse", "unexpected-eof.pkl"),
    ("eof-no-trailing", "parse", "eof-no-trailing.pkl"),
    ("missing-close", "parse", "missing-close.pkl"),
    ("missing-object-close", "parse", "missing-object-close.pkl"),
    ("nested-let-object", "parse", "nesting.pkl"),
    ("owner-receiver-parser", "parse", "owner-receiver.pkl"),
    ("lexical-vs-inherited", "value", "lexical-vs-inherited.pkl"),
    ("virtual-this-super", "value", "virtual-this-super.pkl"),
    ("local-owner", "value", "local-owner.pkl"),
    ("let-capture", "value", "let-capture.pkl"),
    ("implicit-this", "value", "implicit-this.pkl"),
)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def contract():
    return [{"id": name, "mode": mode, "input": str(FIXTURES / filename),
             "input_sha256": oracle.sha256(ROOT / FIXTURES / filename)}
            for name, mode, filename in CASES]


def run(command, *, cwd, environment, timeout=120):
    return subprocess.run(command, cwd=cwd, env=environment, capture_output=True, timeout=timeout)


def check_report(directory):
    report = json.loads((directory / "report.json").read_text())
    if not report["source_stability"]:
        raise ValueError("sources changed during frontend evidence acquisition")
    observed = {path.relative_to(directory).as_posix() for path in directory.rglob("*")
                if path.is_file() and path != directory / "report.json"}
    if observed != set(report["evidence_sha256"]):
        raise ValueError("frontend evidence inventory is incomplete")
    if not report["source_sha256"]:
        raise ValueError("frontend source identities are missing")
    for source, expected in report["source_sha256"].items():
        if oracle.sha256(directory / "sources" / source) != expected:
            raise ValueError(f"frontend source snapshot changed: {source}")
    if report["contract"] != contract():
        raise ValueError("frontend contract changed or required cases are missing")
    if {row["id"] for row in report["results"]} != {row["id"] for row in contract()} or len(report["results"]) != len(CASES):
        raise ValueError("frontend case coverage is incomplete")
    for relative, expected in report["evidence_sha256"].items():
        if oracle.sha256(directory / relative) != expected:
            raise ValueError(f"frontend evidence changed: {relative}")
    for row in report["results"]:
        case = directory / row["id"]
        java = (case / "java.stdout.bin").read_bytes()
        rust = (case / "rust.result.bin").read_bytes()
        java_stderr = (case / "java.stderr.bin").read_bytes()
        rust_stderr = (case / "rust.stderr.bin").read_bytes()
        equal = (row["java_exit_code"] == row["rust_exit_code"] == 0
                 and bool(java) and java == rust and java_stderr == rust_stderr == b"")
        if row["passed"] != equal or not equal:
            raise ValueError(f"frontend case did not pass exact comparison: {row['id']}")
    return report


def execute(output):
    if output.exists():
        raise ValueError("evidence directory already exists; use a new --output directory")
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    output.mkdir(parents=True)
    tracked = sorted({str(path.relative_to(ROOT)) for path in (ROOT / "crates/pkl-runtime/src").rglob("*.rs")}
                     | {str(path.relative_to(ROOT)) for path in (ROOT / "crates/pkl-runtime/src").rglob("*.pkl")}
                     | {"crates/pkl-runtime/tests/frontend.rs", "crates/pkl-runtime/Cargo.toml", "Cargo.toml", "Cargo.lock", "rust-toolchain.toml",
                        "tools/java/M1FrontendProbe.java", "tools/m1_frontend.py"})
    initial_hashes = {path: oracle.sha256(ROOT / path) for path in tracked}
    for relative in tracked:
        destination = output / "sources" / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((ROOT / relative).read_bytes())
    rust_identity = subprocess.run(["rustc", "--version", "--verbose"], cwd=ROOT, capture_output=True, timeout=30)
    (output / "rustc.stdout.bin").write_bytes(rust_identity.stdout)
    (output / "rustc.stderr.bin").write_bytes(rust_identity.stderr)
    if rust_identity.returncode:
        raise ValueError("cannot identify the Rust compiler")
    # Build once, then execute the exact recorded Rust artifact for every case.
    build_command = ["cargo", "test", "-p", "pkl-runtime", "--locked", "--test", "frontend", "--no-run", "--message-format=json"]
    build = subprocess.run(build_command, cwd=ROOT, capture_output=True, timeout=180)
    (output / "build.stdout.bin").write_bytes(build.stdout)
    (output / "build.stderr.bin").write_bytes(build.stderr)
    if build.returncode:
        raise ValueError("Rust frontend probe build failed; raw build evidence preserved")
    executables = [row["executable"] for line in build.stdout.splitlines()
                   if (row := json.loads(line)).get("reason") == "compiler-artifact"
                   and row.get("target", {}).get("name") == "frontend" and row.get("executable")]
    if len(executables) != 1:
        raise ValueError("expected exactly one frontend test executable")
    rust_binary = executables[0]
    results = []
    cases = contract()
    for item in cases:
        case = output / item["id"]
        case.mkdir()
        source = ROOT / item["input"]
        (case / "input.pkl").write_bytes(source.read_bytes())
        java_command = [*metadata["command"][:-1], str(ROOT / "tools/java/M1FrontendProbe.java"), item["mode"], str(source)]
        java = run(java_command, cwd=ROOT, environment=metadata["environment"])
        for channel in ("stdout", "stderr"):
            (case / f"java.{channel}.bin").write_bytes(getattr(java, channel))
        rust_command = [rust_binary, "--ignored", "--exact", "frontend_probe", "--nocapture"]
        rust_environment = dict(metadata["environment"],
                                PKL_M1_FRONTEND_INPUT=str(source),
                                PKL_M1_FRONTEND_MODE=item["mode"],
                                PKL_M1_FRONTEND_OUTPUT=str(case / "rust.result.bin"))
        rust = run(rust_command, cwd=ROOT, environment=rust_environment)
        for channel in ("stdout", "stderr"):
            (case / f"rust.{channel}.bin").write_bytes(getattr(rust, channel))
        candidate = case / "rust.result.bin"
        if not candidate.exists():
            candidate.write_bytes(b"")
        equal = (java.returncode == rust.returncode == 0 and bool(java.stdout) and java.stdout == candidate.read_bytes()
                 and java.stderr == rust.stderr == b"")
        results.append({**item, "java_command": java_command, "rust_command": rust_command,
                        "cwd": str(ROOT), "java_exit_code": java.returncode, "rust_exit_code": rust.returncode,
                        "passed": equal})
        print(f"{item['id']}: {'passed' if equal else 'FAILED'}", flush=True)
    report = {"schema_version": 1, "scope": "M1 bounded parser property/expression UTF16 spans, ParserError.message bytes/first error, scalar positions, and five VM result values; not CLI error formatting or full language compatibility",
              "channel_contract": "Java stdout/stderr/exit versus Rust result file/stderr/test-process exit; libtest stdout captured separately and is harness metadata",
              "contract": cases, "oracle": metadata,
              "rust_artifact": {"path": rust_binary, "sha256": oracle.sha256(rust_binary)},
              "source_sha256": initial_hashes,
              "source_stability": all(oracle.sha256(ROOT / path) == expected for path, expected in initial_hashes.items()),
              "build_command": build_command,
              "results": results}
    report["evidence_sha256"] = {path.relative_to(output).as_posix(): oracle.sha256(path)
                                 for path in sorted(output.rglob("*")) if path.is_file()}
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    if not all(row["passed"] for row in results):
        raise ValueError(f"frontend differential mismatch; see {output}")
    check_report(output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT / "target/m1/frontend")
    parser.add_argument("--check-contract", action="store_true")
    parser.add_argument("--check-report", type=Path)
    args = parser.parse_args()
    try:
        if args.check_contract:
            print(f"Frontend contract: {len(contract())} required cases")
        elif args.check_report:
            check_report(args.check_report.resolve())
            print("Frontend exact-byte report verified")
        else:
            execute(args.output.resolve())
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError, oracle.OracleError) as error:
        print(f"m1 frontend: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
