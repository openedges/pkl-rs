"""Acquire independent pinned Java 0.32.0 public IntSeq value observations."""
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

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/fixtures/m2-int-seq-values"
CASES_SHA256 = "ee7a51d9e6908ee3699a0d96b5b58df4c3aafdd0f76042b51afec1078b270bf4"
MODULE_SHA256 = "3a8947ed64bfbbdfb08014751757a76d40ceed52870318328da43abcac6e458e"
QUERIES_SHA256 = "080d8515431599bfb68dce0967c808f7db426a69d37fd55dc613495b7e8d4e81"
EXPORT_CASES_SHA256 = "441c5ae52d97039b461710423ac45693f1489cdf1b5f7fb52a8468d313099036"
EXPORT_QUERIES_SHA256 = "a3413d27cef901171b4bceb40a0c3ecf29960480dc298957f877022b3f19688b"
INPUTS = ("IntSeqValuesOracle.java", "run.py", "module.pkl", "queries.tsv", "cases.json")
SOURCES = (
    "external/pkl/stdlib/base.pkl",
    "external/pkl/stdlib/math.pkl",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmIntSeq.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/IntSeqNodes.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/BaseNodes.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/generator/GeneratorForNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/generator/GeneratorSpreadNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmValueRenderer.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/PcfRenderer.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/JsonRendererNodes.java",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/intseq.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/intseq.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/bytes.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/bytes.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/any.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/generators/spreadSyntaxListing.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/generators/spreadSyntaxDynamic.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/generators/spreadSyntaxMapping.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/generators/elementGeneratorsTyped.pkl",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/generators/entryGeneratorsTyped.pkl",
    "oracle.lock.json", "references.lock",
)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def files(root):
    return {path.relative_to(root).as_posix(): sha(path) for path in root.rglob("*") if path.is_file()}


def acquire(output, *, fixture=FIXTURE, supplement=False):
    output = Path(output).resolve()
    fixture = Path(fixture)
    if output.exists():
        raise ValueError("fresh IntSeq authority output required")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("oracle source pin differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("oracle artifact differs")
    case_name = "export-cases.json" if supplement else "cases.json"
    query_name = "export-queries.tsv" if supplement else "queries.tsv"
    cases = json.loads((fixture / case_name).read_bytes())
    rows = cases["rows"]
    if (cases["schema"] != 1 or cases["baseline"] != "Pkl 0.32.0"
            or cases["module_uri"] != "file:///stage61/intseq-values.pkl"
            or cases["boundary"] != ("typed-export-supplement" if supplement else "string")
            or cases["repetitions"] != 2 or len(rows) != (3 if supplement else 237)
            or any(row["mode"] != ("export" if supplement else "string") for row in rows)
            or sha(fixture / case_name) != (EXPORT_CASES_SHA256 if supplement else CASES_SHA256)
            or sha(fixture / "module.pkl") != MODULE_SHA256
            or sha(fixture / query_name) != (EXPORT_QUERIES_SHA256 if supplement else QUERIES_SHA256)):
        raise ValueError("authored row set differs")
    if (fixture / query_name).read_text() != "".join(
            f"{row['mode']}\t{row['id']}\t{row['expression']}\n" for row in rows):
        raise ValueError("authored ordered queries differ")
    output.mkdir()
    for name, origin in (("IntSeqValuesOracle.java", ROOT / "tools/java/IntSeqValuesOracle.java"),
                         ("run.py", ROOT / "tools/int_seq_values_oracle.py"),
                         ("module.pkl", fixture / "module.pkl"),
                         ("queries.tsv", fixture / query_name),
                         ("cases.json", fixture / case_name)):
        shutil.copyfile(origin, output / name)
    for name in SOURCES:
        target = output / "sources" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    java = Path(os.environ.get("PKL_ORACLE_JAVA", "/usr/bin/java")).resolve()
    env = {"PATH": os.defpath, "HOME": str(ROOT / ".cache/oracle/home"),
           "TMPDIR": str(ROOT / ".cache/oracle/tmp"), "TMP": str(ROOT / ".cache/oracle/tmp"),
           "TEMP": str(ROOT / ".cache/oracle/tmp"), "LC_ALL": "C", "LANG": "C", "TZ": "UTC",
           "TERM": "dumb", "NO_COLOR": "1", "COLUMNS": "80"}
    identity = subprocess.run([str(java), "-XshowSettings:properties", "-version"], cwd=output,
                              env=env, capture_output=True, timeout=60)
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
           "modules_sha256": sha(home / "lib/modules"), "release_sha256": sha(home / "release"),
           "properties": properties}
    options = [*lock["jvm_options"], f"-Duser.home={env['HOME']}", f"-Djava.io.tmpdir={env['TMPDIR']}"]
    provenance = {"schema": 1, "scope": ("Independent public IntSeq Java typed-export supplement; no Rust candidate" if supplement else "Independent public IntSeq Java string observations; no Rust candidate"),
                  "host": platform.platform(), "oracle_lock_sha256": sha(ROOT / "oracle.lock.json"),
                  "references_lock_sha256": sha(ROOT / "references.lock"),
                  "reference_revision": lock["source"]["revision"],
                  "artifact": str(jar), "artifact_sha256": sha(jar), "jdk": jdk, "environment": env,
                  "input_before": {name: sha(output / name) for name in INPUTS},
                  "source_before": {name: sha(ROOT / name) for name in SOURCES}, "sessions": []}
    save(output / "provenance.json", provenance)
    for repeat in range(2):
        folder = output / f"repeat-{repeat}"
        folder.mkdir()
        command = [str(java), *options, "-cp", str(jar), str(output / "IntSeqValuesOracle.java"),
                   str(folder), str(output), cases["module_uri"]]
        started = time.time()
        process = subprocess.Popen(command, cwd=output, env=env,
                                   stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            stdout, stderr = process.communicate(timeout=240)
        except subprocess.TimeoutExpired:
            process.kill()
            stdout, stderr = process.communicate()
            (folder / "process.stdout.bin").write_bytes(stdout)
            (folder / "process.stderr.bin").write_bytes(stderr)
            raise ValueError("Java session timed out; raw attempt retained")
        (folder / "process.stdout.bin").write_bytes(stdout)
        (folder / "process.stderr.bin").write_bytes(stderr)
        entry = {"repeat": repeat, "pid": process.pid, "command": command, "cwd": str(output),
                 "exit_code": process.returncode, "started_epoch": started, "finished_epoch": time.time(),
                 "stdout_sha256": sha(folder / "process.stdout.bin"),
                 "stderr_sha256": sha(folder / "process.stderr.bin"),
                 "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                 "query_directories": sum(path.is_dir() for path in folder.iterdir())}
        provenance["sessions"].append(entry)
        save(output / "provenance.json", provenance)
        print(json.dumps({"repeat": repeat, "pid": process.pid, "exit_code": process.returncode,
                          "rows": entry["query_directories"]}), flush=True)
        if process.returncode or entry["query_directories"] != len(rows):
            raise ValueError("Java session failed; raw attempt retained")
    provenance["input_after"] = {name: sha(output / name) for name in INPUTS}
    provenance["source_after"] = {name: sha(ROOT / name) for name in SOURCES}
    provenance["artifact_after_sha256"] = sha(jar)
    provenance["stable"] = (provenance["input_before"] == provenance["input_after"] and
                            provenance["source_before"] == provenance["source_after"] and
                            provenance["artifact_sha256"] == provenance["artifact_after_sha256"])
    first, second = files(output / "repeat-0"), files(output / "repeat-1")
    provenance["raw_equal"] = first == second
    provenance["raw_files"] = len(first)
    provenance["raw_differences"] = [name for name in sorted(first.keys() | second.keys())
                                     if first.get(name) != second.get(name)]
    provenance["complete"] = provenance["stable"] and provenance["raw_equal"] and len(provenance["sessions"]) == 2
    save(output / "provenance.json", provenance)
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
    audit = {"schema": 1, "scope": provenance["scope"], "rows": len(observations),
             "statuses": dict(Counter(row["status"] for row in observations)),
             "observations": observations, "candidate_compatibility_passes_claimed": 0,
             "limits": ["Authored public calls only; original upstream snippets are source evidence, not replayed modules.",
                        "Rust native, browser and Node raw comparison, typed candidate export, full M2 and performance gates remain unverified."]}
    save(output / "audit.json", audit)
    if not provenance["complete"]:
        raise ValueError("acquisition incomplete; raw attempt retained")
    print(json.dumps(audit["statuses"], sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--supplement", action="store_true", help="capture separate typed export supplement")
    args = parser.parse_args()
    acquire(args.output, supplement=args.supplement)
