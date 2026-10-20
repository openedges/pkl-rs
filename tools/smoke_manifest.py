#!/usr/bin/env python3
"""Generate the small required CI corpus; the full corpus remains separately accounted for."""
import argparse
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]


def build():
    cases = []
    for name, command, expected_exit in (
        ("basic", ["eval"], [0]),
        ("json", ["eval", "--format", "json"], [0]),
        ("yaml", ["eval", "--format", "yaml"], [0]),
        ("error", ["eval"], [1]),
        ("files", ["eval", "--multiple-file-output-path", "{output_dir}"], [0]),
        ("collections", ["eval"], [0]),
        ("test", ["test"], [0]),
    ):
        fixture = "basic" if name in ("json", "yaml") else name
        path = f"tests/fixtures/m0/{fixture}.pkl"
        cases.append({"id": f"project:m0-smoke:{name}", "suite": "m0-smoke",
                      "source": {"repository": "pkl-rs", "path": path,
                                 "sha256": hashlib.sha256((ROOT / path).read_bytes()).hexdigest()},
                      "command": command + ["--no-project", "--settings", "pkl:settings", "--color=never", path],
                      "cwd": ".", "expected_exit": expected_exit, "environment": {},
                      "channels": ["stdout", "stderr", "exit_code", "files"],
                      "execution": {"status": "ready"}, "target": ["native", "wasm", "napi"]})
    return {"schema_version": 1, "scope": "Required tooling smoke corpus, not full Pkl compatibility",
            "cases": cases}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    path = ROOT / "compatibility/smoke.json"
    expected = json.dumps(build(), indent=2, ensure_ascii=False) + "\n"
    if args.check:
        if not path.exists() or path.read_text(encoding="utf-8") != expected:
            print("Smoke manifest missing or stale", file=sys.stderr)
            return 1
    else:
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(expected, encoding="utf-8")
    print("Smoke manifest verified" if args.check else "Smoke manifest written")
    return 0


if __name__ == "__main__":
    sys.exit(main())
