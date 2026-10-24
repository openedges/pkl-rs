"""Acquire independent pinned Java 0.32.0 Duration string observations."""
import argparse
import hashlib
import json
import os
import platform
import re
import shutil
import subprocess
import time
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/fixtures/m2-duration-values"
CASES_SHA256 = "5350939174115c15e671547ffa6e9d3eafb57d464b4a35fcbce4c6344deda7bb"
MODULE_SHA256 = "0225905b404e3290b4ea72c104128d91f4f89451ce721b18b227fe062c0764fc"
QUERIES_SHA256 = "b81f94def71ad51bedce0a3368a8c17465df0fc69c210233b63d358af7e88c85"
SOURCE_NAMES = [
 "external/pkl/stdlib/base.pkl",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmDuration.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/DurationNodes.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/util/DurationUtils.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/Duration.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/DurationUnit.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/AdditionNode.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/SubtractionNode.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/DivisionNode.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/TruncatingDivisionNode.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/RemainderNode.java",
 "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/binary/ExponentiationNode.java",
 "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/reference.pkl",
 "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/reference.pkl",
 "oracle.lock.json", "references.lock",
]
INPUT_NAMES = ["DurationValuesOracle.java", "run.py", "module.pkl", "queries.tsv", "cases.json"]

def sha(path):
 return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def acquire(output, *, fixture=FIXTURE):
    output = Path(output).resolve()
    fixture = Path(fixture)
    if output.exists():
        raise ValueError(f"fresh output required: {output}")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("oracle source pin differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("oracle artifact differs")
    m = json.loads((fixture / "cases.json").read_bytes())
    if m["boundary"] != "string" or m["repetitions"] != 2 or m["module_uri"] != "file:///stage59/duration-values.pkl" or len(m["rows"]) != 274 or sha(fixture / "cases.json") != CASES_SHA256 or sha(fixture / "module.pkl") != MODULE_SHA256 or sha(fixture / "queries.tsv") != QUERIES_SHA256:
        raise ValueError("authored case manifest differs")
    if (fixture / "queries.tsv").read_text() != "".join(r["id"] + "\t" + r["expression"] + "\n" for r in m["rows"]):
        raise ValueError("authored queries differ")
    output.mkdir(parents=True)
    for name, origin in [("DurationValuesOracle.java", ROOT / "tools/java/DurationValuesOracle.java"),
                         ("run.py", ROOT / "tools/duration_values_oracle.py"),
                         ("module.pkl", fixture / "module.pkl"), ("queries.tsv", fixture / "queries.tsv"),
                         ("cases.json", fixture / "cases.json")]:
        shutil.copyfile(origin, output / name)
    for name in SOURCE_NAMES:
        target = output / "sources" / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    java = Path(os.environ.get("PKL_ORACLE_JAVA", "/usr/bin/java")).resolve()
    env = {"PATH": os.defpath, "HOME": str(ROOT / ".cache/oracle/home"), "TMPDIR": str(ROOT / ".cache/oracle/tmp"),
           "TMP": str(ROOT / ".cache/oracle/tmp"), "TEMP": str(ROOT / ".cache/oracle/tmp"), "LC_ALL": "C",
           "LANG": "C", "TZ": "UTC", "TERM": "dumb", "NO_COLOR": "1", "COLUMNS": "80"}
    identity = subprocess.run([str(java), "-XshowSettings:properties", "-version"], cwd=output, env=env, capture_output=True, timeout=60)
    (output / "java-identity.stdout.bin").write_bytes(identity.stdout)
    (output / "java-identity.stderr.bin").write_bytes(identity.stderr)
    if identity.returncode:
        raise ValueError("Java identity failed; raw attempt retained")
    props = dict(re.findall(r"^\s+([\w.]+) = (.*)$", identity.stderr.decode(), re.M))
    for name, expected in lock["jdk"]["properties"].items():
        if props.get(name) != expected:
            raise ValueError(f"Java property {name} differs; raw attempt retained")
    home = Path(props["java.home"])
    jdk = {"executable": str(java), "executable_sha256": sha(java), "modules_sha256": sha(home / "lib/modules"),
           "release_sha256": sha(home / "release"), "properties": props}
    options = [*lock["jvm_options"], f"-Duser.home={env['HOME']}", f"-Djava.io.tmpdir={env['TMPDIR']}"]
    p = {"schema": 1, "scope": "Independent Java public expression-string Duration observations; no Rust candidate",
         "host": platform.platform(), "oracle_lock_sha256": sha(ROOT / "oracle.lock.json"),
         "references_lock_sha256": sha(ROOT / "references.lock"), "reference_revision": lock["source"]["revision"],
         "artifact": str(jar), "artifact_sha256": sha(jar), "jdk": jdk, "environment": env,
         "input_before": {name: sha(output / name) for name in INPUT_NAMES},
         "source_before": {name: sha(ROOT / name) for name in SOURCE_NAMES}, "sessions": []}
    def save():
        (output / "provenance.json").write_text(json.dumps(p, indent=2, sort_keys=True) + "\n")
    save()
    for repeat in range(2):
        folder = output / f"repeat-{repeat}"
        folder.mkdir()
        command = [str(java), *options, "-cp", str(jar), str(output / "DurationValuesOracle.java"),
                   str(folder), str(output), m["module_uri"]]
        started = time.time()
        proc = subprocess.Popen(command, cwd=output, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            stdout, stderr = proc.communicate(timeout=1200)
        except subprocess.TimeoutExpired:
            proc.kill()
            stdout, stderr = proc.communicate()
            (folder / "process.stdout.bin").write_bytes(stdout)
            (folder / "process.stderr.bin").write_bytes(stderr)
            raise ValueError("Java session timed out; raw attempt retained")
        (folder / "process.stdout.bin").write_bytes(stdout)
        (folder / "process.stderr.bin").write_bytes(stderr)
        entry = {"repeat": repeat, "pid": proc.pid, "command": command, "cwd": str(output), "exit_code": proc.returncode,
                 "started_epoch": started, "finished_epoch": time.time(), "stdout_sha256": sha(folder / "process.stdout.bin"),
                 "stderr_sha256": sha(folder / "process.stderr.bin"), "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                 "query_directories": sum(x.is_dir() for x in folder.iterdir())}
        p["sessions"].append(entry)
        save()
        print(json.dumps({"repeat": repeat, "pid": proc.pid, "exit_code": proc.returncode,
                          "rows": entry["query_directories"]}), flush=True)
        if proc.returncode or entry["query_directories"] != len(m["rows"]):
            raise ValueError("Java session failed; raw attempt retained")
    p["input_after"] = {name: sha(output / name) for name in INPUT_NAMES}
    p["source_after"] = {name: sha(ROOT / name) for name in SOURCE_NAMES}
    p["artifact_after_sha256"] = sha(jar)
    p["stable"] = p["input_before"] == p["input_after"] and p["source_before"] == p["source_after"] and p["artifact_sha256"] == p["artifact_after_sha256"]
    def files(folder):
        return {x.relative_to(folder).as_posix(): sha(x) for x in folder.rglob("*") if x.is_file()}
    first, second = files(output / "repeat-0"), files(output / "repeat-1")
    p["raw_equal"] = first == second
    p["raw_files"] = len(first)
    p["raw_differences"] = [x for x in sorted(first.keys() | second.keys()) if first.get(x) != second.get(x)]
    p["complete"] = p["stable"] and p["raw_equal"] and len(p["sessions"]) == 2
    save()
    observations = []
    for index, row in enumerate(m["rows"]):
        d = output / "repeat-0" / f"{index:03d}-{row['id']}"
        observations.append({"id": row["id"], "group": row["group"], "expression": row["expression"],
                             "status": (d / "result.status.bin").read_text().strip(),
                             "stdout_sha256": sha(d / "result.stdout.bin"), "stderr_sha256": sha(d / "result.stderr.bin"),
                             "error_class": (d / "result.error-class.bin").read_text()})
    audit = {"schema": 1, "scope": p["scope"], "rows": len(observations), "statuses": dict(Counter(x["status"] for x in observations)),
             "observations": observations, "compatibility_passes_claimed": 0,
             "remaining_required": ["Rust native, browser and Node raw comparisons", "typed export API", "full M2 language and performance gates"]}
    (output / "audit.json").write_text(json.dumps(audit, indent=2, sort_keys=True) + "\n")
    if not p["complete"]:
        raise ValueError("acquisition incomplete; raw attempt retained")
    return audit


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument("--output", type=Path)
    args = parser.parse_args()
    print(json.dumps(acquire(args.output)["statuses"], sort_keys=True))
