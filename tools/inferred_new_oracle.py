"""Acquire independent pinned Java 0.32.0 inferred-new string authority."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import time

from amendment_construction_oracle import SOURCES as AMENDMENT_SOURCES

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = {
    "values": ROOT / "tests/fixtures/m2-inferred-new-values",
    "amends": ROOT / "tests/fixtures/m2-inferred-new-amends",
}
ROWS = {"values": 72, "amends": 7}
INPUT_HASHES = {
    "values": {
        "cases.json": "10bc691c2aab559e5ade734087e26619f8903945875924a92da9784416182057",
        "module.pkl": "d0a0fc1c19a588d9188ecf73243d819d32fca34da249764573f3739b3a1dd03e",
        "queries.tsv": "6efd438f068e88f23f7864745860c4b37e01e5b531602bacddb3aae2f9d03c59",
    },
    "amends": {
        "base.pkl": "5e24f1414ea17b41a1cf910d9a6c4a67777c3208f8adb804951ba892a5c52fbd",
        "cases.json": "548d716b7d6d62fa78f13c2c937ef0e466739ad5de5de348a61a9b4cf68521ce",
        "module.pkl": "a62249869005ab96156ca902f397b9e1a5802feb8bf7a963a3f3fd5a946747ae",
        "queries.tsv": "15b0a87a2aaf2c842d3a561d75e1f74727d52c221d499b9c1180741326e56602",
    },
}
EXTRA_SOURCES = (
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/InferParentWithinMethodNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/InferParentWithinObjectMethodNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/InferParentWithinPropertyNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/ReadSuperEntryNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/ReadSuperPropertyNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/GetParentForTypeNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/TypeNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/ResolveDeclaredTypeNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/ResolveSimpleDeclaredTypeNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/ResolveQualifiedDeclaredTypeNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/literal/AmendModuleNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/member/ModuleNode.java",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/newInAmendingModuleMethod.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/newInsideIf.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/newInsideLet.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/newType.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/baseModule.pkl",
    "tools/amendment_construction_oracle.py",
    "tools/java/CollectionConversionsOracle.java",
)
SOURCES = tuple(sorted(set(AMENDMENT_SOURCES) | set(EXTRA_SOURCES)))
REVISION = "e51fb2f91774265666f94980ec630db794a4db2d"


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def files(folder: Path) -> dict[str, str]:
    return {path.relative_to(folder).as_posix(): sha(path)
            for path in sorted(folder.rglob("*")) if path.is_file()}


def save(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def acquire(profile: str, output: Path) -> None:
    output = output.resolve()
    fixture = FIXTURES[profile]
    if output.exists():
        raise ValueError("fresh inferred-new output directory required")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != REVISION:
        raise ValueError("pinned Java source differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("pinned Java artifact differs")
    for name, expected in INPUT_HASHES[profile].items():
        if sha(fixture / name) != expected:
            raise ValueError(f"authored {profile} input differs: {name}")
    cases = json.loads((fixture / "cases.json").read_bytes())
    rows = cases["rows"]
    if (cases["schema"] != 1 or cases["baseline"] != "Pkl 0.32.0"
            or cases["boundary"] != "string" or cases["repetitions"] != 2
            or len(rows) != ROWS[profile] or len({row["id"] for row in rows}) != len(rows)
            or any(row["mode"] != "string" for row in rows)
            or (fixture / "queries.tsv").read_text() != "".join(
                f"string\t{row['id']}\t{row['expression']}\n" for row in rows)):
        raise ValueError("authored ordered rows differ")
    if profile == "values":
        if cases["module_uri"] != "file:///stage65/inferred-new-values.pkl":
            raise ValueError("normal module URI differs")
    elif cases["module_uri_policy"] != "captured-absolute-file-uri-with-sibling-base":
        raise ValueError("amends relative import policy differs")
    output.mkdir(parents=True)
    for name, origin in (("CollectionConversionsOracle.java", ROOT / "tools/java/CollectionConversionsOracle.java"),
                         ("run.py", Path(__file__)),
                         *((name, fixture / name) for name in INPUT_HASHES[profile])):
        shutil.copyfile(origin, output / name)
    for name in SOURCES:
        target = output / "sources" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    if {name: sha(output / "sources" / name) for name in SOURCES} != {
            name: sha(ROOT / name) for name in SOURCES}:
        raise ValueError("source copy differs")
    module_uri = (cases["module_uri"] if profile == "values"
                  else (output / "module.pkl").as_uri())
    java = Path(os.environ.get("PKL_ORACLE_JAVA", "/usr/bin/java")).resolve()
    env = {"PATH": os.defpath, "HOME": str(ROOT / ".cache/oracle/home"),
           "TMPDIR": str(ROOT / ".cache/oracle/tmp"), "TMP": str(ROOT / ".cache/oracle/tmp"),
           "TEMP": str(ROOT / ".cache/oracle/tmp"), "LC_ALL": "C", "LANG": "C",
           "TZ": "UTC", "TERM": "dumb", "NO_COLOR": "1", "COLUMNS": "80"}
    identity = subprocess.run([str(java), "-XshowSettings:properties", "-version"],
                              cwd=output, env=env, capture_output=True, timeout=60)
    (output / "java-identity.stdout.bin").write_bytes(identity.stdout)
    (output / "java-identity.stderr.bin").write_bytes(identity.stderr)
    if identity.returncode:
        raise ValueError("Java identity failed; raw output retained")
    properties = dict(re.findall(r"^\s+([\w.]+) = (.*)$", identity.stderr.decode(), re.M))
    for name, value in lock["jdk"]["properties"].items():
        if properties.get(name) != value:
            raise ValueError(f"pinned Java property {name} differs; raw output retained")
    home = Path(properties["java.home"])
    jdk = {"executable": str(java), "executable_sha256": sha(java),
           "modules_sha256": sha(home / "lib/modules"),
           "release_sha256": sha(home / "release"), "properties": properties}
    options = [*lock["jvm_options"], f"-Duser.home={env['HOME']}",
               f"-Djava.io.tmpdir={env['TMPDIR']}"]
    inputs = {"CollectionConversionsOracle.java", "run.py", *INPUT_HASHES[profile]}
    provenance = {"schema": 1, "profile": profile,
                  "scope": "Independent public inferred-new Java string authority; no Rust candidate",
                  "host": platform.platform(), "module_uri": module_uri,
                  "oracle_lock_sha256": sha(ROOT / "oracle.lock.json"),
                  "references_lock_sha256": sha(ROOT / "references.lock"),
                  "reference_revision": REVISION, "artifact": str(jar),
                  "artifact_sha256": sha(jar), "jdk": jdk, "environment": env,
                  "input_before": {name: sha(output / name) for name in sorted(inputs)},
                  "source_before": {name: sha(ROOT / name) for name in SOURCES},
                  "sessions": [], "candidate_compatibility_passes_claimed": 0}
    save(output / "provenance.json", provenance)
    for repeat in range(2):
        folder = output / f"repeat-{repeat}"
        folder.mkdir()
        command = [str(java), *options, "-cp", str(jar),
                   str(output / "CollectionConversionsOracle.java"),
                   str(folder), str(output), module_uri]
        started = time.time()
        process = subprocess.Popen(command, cwd=output, env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        timed_out = False
        try:
            stdout, stderr = process.communicate(timeout=240)
        except subprocess.TimeoutExpired:
            timed_out = True
            process.kill()
            stdout, stderr = process.communicate()
        (folder / "process.stdout.bin").write_bytes(stdout)
        (folder / "process.stderr.bin").write_bytes(stderr)
        entry = {"repeat": repeat, "pid": process.pid, "command": command,
                 "cwd": str(output), "exit_code": process.returncode,
                 "timed_out": timed_out, "started_epoch": started,
                 "finished_epoch": time.time(),
                 "stdout_sha256": sha(folder / "process.stdout.bin"),
                 "stderr_sha256": sha(folder / "process.stderr.bin"),
                 "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                 "query_directories": sum(path.is_dir() for path in folder.iterdir())}
        provenance["sessions"].append(entry)
        save(output / "provenance.json", provenance)
        print(json.dumps({"profile": profile, "repeat": repeat, "pid": process.pid,
                          "exit_code": process.returncode, "rows": entry["query_directories"]}), flush=True)
        if timed_out or process.returncode or entry["query_directories"] != len(rows):
            raise ValueError("Java session failed; raw attempt retained")
    provenance["input_after"] = {name: sha(output / name) for name in sorted(inputs)}
    provenance["source_after"] = {name: sha(ROOT / name) for name in SOURCES}
    provenance["source_copies_after"] = {name: sha(output / "sources" / name) for name in SOURCES}
    provenance["artifact_after_sha256"] = sha(jar)
    provenance["stable"] = (provenance["input_before"] == provenance["input_after"]
                            and provenance["source_before"] == provenance["source_after"]
                            == provenance["source_copies_after"]
                            and provenance["artifact_sha256"] == provenance["artifact_after_sha256"])
    first, second = files(output / "repeat-0"), files(output / "repeat-1")
    provenance["raw_equal"] = first == second
    provenance["raw_files"] = len(first)
    provenance["raw_differences"] = [name for name in sorted(first.keys() | second.keys())
                                     if first.get(name) != second.get(name)]
    observations = []
    for index, row in enumerate(rows):
        folder = output / "repeat-0" / f"{index:03d}-{row['id']}"
        status = (folder / "result.status.bin").read_text().strip()
        stderr = (folder / "result.stderr.bin").read_text()
        observations.append({"id": row["id"], "group": row["group"], "mode": row["mode"],
                             "expression": row["expression"], "status": status,
                             "stdout_sha256": sha(folder / "result.stdout.bin"),
                             "stderr_sha256": sha(folder / "result.stderr.bin"),
                             "stdout": (folder / "result.stdout.bin").read_text(),
                             "first_error_line": stderr.splitlines()[1] if len(stderr.splitlines()) > 1 else "",
                             "error_class": (folder / "result.error-class.bin").read_text()})
    statuses = dict(Counter(row["status"] for row in observations))
    provenance["complete"] = (provenance["stable"] and provenance["raw_equal"]
                              and len(provenance["sessions"]) == 2
                              and not statuses.get("oracle-error") and not statuses.get("api-exception"))
    save(output / "provenance.json", provenance)
    audit = {"schema": 1, "scope": provenance["scope"], "profile": profile,
             "rows": len(rows), "statuses": statuses, "observations": observations,
             "candidate_compatibility_passes_claimed": 0,
             "limits": ["Authored public string observations only; no Rust candidate compared.",
                        "Amends uses a real sibling file and captured absolute file URI; portable Host behavior is unverified."]}
    save(output / "audit.json", audit)
    if not provenance["complete"]:
        raise ValueError("acquisition incomplete; raw attempt retained")
    print(json.dumps(statuses, sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=sorted(FIXTURES), required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    acquire(args.profile, args.output)
