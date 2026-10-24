"""Acquire distinct pinned Java 0.32.0 union marker diagnostic module programs."""
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
SOURCES = tuple(dict.fromkeys((*AMENDMENT_SOURCES,
    "external/pkl/pkl-parser/src/main/java/org/pkl/parser/GenericParserImpl.java",
    "external/pkl/pkl-parser/src/main/java/org/pkl/parser/Lexer.java",
    "external/pkl/pkl-parser/src/main/java/org/pkl/parser/Span.java")))

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / "tests/fixtures/m2-union-marker-diagnostics"
MODULE_URI = "file:///stage66/union-marker-diagnostics.pkl"
PROGRAMS_SHA256 = "9aff6bb7a1704df308ebd0f0639683bcbdfacaab4439f6392a54ac7aa00b7b71"


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def program_input(program):
    return {"schema": 1, "baseline": "Pkl 0.32.0", "module_uri": MODULE_URI,
            "boundary": "string", "repetitions": 2,
            "rows": [{"group": "union-marker-parser", "id": program["id"],
                      "mode": "string", "expression": program["query"]}]}


def acquire(output):
    output = Path(output).resolve()
    if output.exists():
        raise ValueError("fresh declaration authority output required")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("oracle source pin differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("oracle artifact differs")
    if sha(FIXTURE / "programs.json") != PROGRAMS_SHA256:
        raise ValueError("authored program manifest differs")
    authored = json.loads((FIXTURE / "programs.json").read_bytes())
    programs = authored["programs"]
    if (authored["schema"] != 1 or authored["baseline"] != "Pkl 0.32.0"
            or authored["module_uri"] != MODULE_URI or authored["repetitions"] != 2
            or len(programs) != 40 or len({p["id"] for p in programs}) != len(programs)):
        raise ValueError("authored declaration set differs")
    expected_modules = set()
    for index, program in enumerate(programs):
        module = FIXTURE / program["module_file"]
        if (not program["id"].replace("-", "").isalnum() or program["query"] != "result"
                or program["module_file"] != f"modules/{index:02d}-{program['id']}.pkl"
                or not module.is_file() or module.is_symlink()
                or sha(module) != program["module_sha256"]
                or module.stat().st_size != program["module_bytes"]):
            raise ValueError("authored module identity differs")
        expected_modules.add(module.relative_to(FIXTURE).as_posix())
    actual_modules = {p.relative_to(FIXTURE).as_posix() for p in (FIXTURE / "modules").iterdir()}
    if actual_modules != expected_modules:
        raise ValueError("authored module file set differs")
    output.mkdir()
    shutil.copyfile(FIXTURE / "programs.json", output / "programs.json")
    shutil.copyfile(ROOT / "tools/java/CollectionConversionsOracle.java",
                    output / "CollectionConversionsOracle.java")
    shutil.copyfile(ROOT / "tools/union_marker_oracle.py", output / "run.py")
    shutil.copyfile(ROOT / "tools/amendment_construction_oracle.py", output / "imported-construction-oracle.py")
    for program in programs:
        destination = output / program["module_file"]
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(FIXTURE / program["module_file"], destination)
    for name in SOURCES:
        destination = output / "sources" / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, destination)
    java = Path(os.environ.get("PKL_ORACLE_JAVA", "/usr/bin/java")).resolve()
    env = {"PATH": os.defpath, "HOME": str(ROOT / ".cache/oracle/home"),
           "TMPDIR": str(ROOT / ".cache/oracle/tmp"), "TMP": str(ROOT / ".cache/oracle/tmp"),
           "TEMP": str(ROOT / ".cache/oracle/tmp"), "LC_ALL": "C", "LANG": "C", "TZ": "UTC",
           "TERM": "dumb", "NO_COLOR": "1", "COLUMNS": "80"}
    identity = subprocess.run([str(java), "-XshowSettings:properties", "-version"],
                              cwd=output, env=env, capture_output=True, timeout=60)
    (output / "java-identity.stdout.bin").write_bytes(identity.stdout)
    (output / "java-identity.stderr.bin").write_bytes(identity.stderr)
    if identity.returncode:
        raise ValueError("Java identity failed; raw output retained")
    properties = dict(re.findall(r"^\s+([\w.]+) = (.*)$", identity.stderr.decode(), re.M))
    for key, value in lock["jdk"]["properties"].items():
        if properties.get(key) != value:
            raise ValueError(f"pinned JDK property differs: {key}")
    home = Path(properties["java.home"])
    jdk = {"executable": str(java), "executable_sha256": sha(java),
           "modules_sha256": sha(home / "lib/modules"), "release_sha256": sha(home / "release"),
           "properties": properties}
    options = [*lock["jvm_options"], f"-Duser.home={env['HOME']}",
               f"-Djava.io.tmpdir={env['TMPDIR']}"]
    producer_inputs = ("CollectionConversionsOracle.java", "run.py",
                       "imported-construction-oracle.py", "programs.json")
    inputs = (*producer_inputs, *(p["module_file"] for p in programs))
    provenance = {"schema": 1, "scope": "Independent separate Java union marker diagnostic programs; no Rust candidate",
                  "host": platform.platform(), "module_uri": MODULE_URI,
                  "oracle_lock_sha256": sha(ROOT / "oracle.lock.json"),
                  "references_lock_sha256": sha(ROOT / "references.lock"),
                  "reference_revision": lock["source"]["revision"],
                  "artifact": str(jar), "artifact_sha256": sha(jar),
                  "jdk": jdk, "environment": env,
                  "input_before": {name: sha(output / name) for name in inputs},
                  "source_before": {name: sha(ROOT / name) for name in SOURCES},
                  "sessions": [], "complete": False}
    save(output / "provenance.json", provenance)
    observations = []
    for index, program in enumerate(programs):
        folder = output / f"programs/{index:02d}-{program['id']}"
        folder.mkdir(parents=True)
        shutil.copyfile(output / program["module_file"], folder / "module.pkl")
        save(folder / "cases.json", program_input(program))
        (folder / "queries.tsv").write_text(f"string\t{program['id']}\t{program['query']}\n")
        for repeat in range(2):
            result = folder / f"repeat-{repeat}"
            result.mkdir()
            command = [str(java), *options, "-cp", str(jar),
                       str(output / "CollectionConversionsOracle.java"), str(result), str(folder), MODULE_URI]
            started = time.time()
            process = subprocess.Popen(command, cwd=output, env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                stdout, stderr = process.communicate(timeout=120)
            except subprocess.TimeoutExpired:
                process.kill()
                stdout, stderr = process.communicate()
                (result / "process.stdout.bin").write_bytes(stdout)
                (result / "process.stderr.bin").write_bytes(stderr)
                raise ValueError("Java session timed out; raw attempt retained")
            (result / "process.stdout.bin").write_bytes(stdout)
            (result / "process.stderr.bin").write_bytes(stderr)
            entry = {"index": index, "program": program["id"], "repeat": repeat,
                     "pid": process.pid, "command": command, "cwd": str(output),
                     "started_epoch": started, "finished_epoch": time.time(),
                     "exit_code": process.returncode, "stdout_sha256": sha(result / "process.stdout.bin"),
                     "stderr_sha256": sha(result / "process.stderr.bin"),
                     "stdout_bytes": len(stdout), "stderr_bytes": len(stderr),
                     "query_directories": sum(path.is_dir() for path in result.iterdir())}
            provenance["sessions"].append(entry)
            save(output / "provenance.json", provenance)
            print(json.dumps({"program": program["id"], "repeat": repeat,
                              "pid": process.pid, "exit_code": process.returncode}), flush=True)
            if process.returncode or entry["query_directories"] != 1:
                raise ValueError("Java session failed; raw attempt retained")
        left = {p.relative_to(folder / "repeat-0").as_posix(): sha(p)
                for p in (folder / "repeat-0").rglob("*") if p.is_file()}
        right = {p.relative_to(folder / "repeat-1").as_posix(): sha(p)
                 for p in (folder / "repeat-1").rglob("*") if p.is_file()}
        if left != right:
            raise ValueError("program raw repetition differs; raw attempt retained")
        row = folder / "repeat-0" / f"000-{program['id']}"
        stderr = (row / "result.stderr.bin").read_text()
        observations.append({"id": program["id"], "module_file": program["module_file"],
                             "module_sha256": program["module_sha256"],
                             "query": program["query"],
                             "status": (row / "result.status.bin").read_text().strip(),
                             "stdout": (row / "result.stdout.bin").read_text(),
                             "stdout_sha256": sha(row / "result.stdout.bin"),
                             "stderr_sha256": sha(row / "result.stderr.bin"),
                             "error_class": (row / "result.error-class.bin").read_text(),
                             "first_error_line": stderr.splitlines()[1] if len(stderr.splitlines()) > 1 else ""})
    provenance["input_after"] = {name: sha(output / name) for name in inputs}
    provenance["source_after"] = {name: sha(ROOT / name) for name in SOURCES}
    provenance["artifact_after_sha256"] = sha(jar)
    provenance["stable"] = (provenance["input_before"] == provenance["input_after"]
                            and provenance["source_before"] == provenance["source_after"]
                            and provenance["artifact_sha256"] == provenance["artifact_after_sha256"])
    provenance["complete"] = provenance["stable"] and len(provenance["sessions"]) == len(programs) * 2
    save(output / "provenance.json", provenance)
    audit = {"schema": 1, "scope": provenance["scope"], "programs": len(programs),
             "statuses": dict(Counter(row["status"] for row in observations)),
             "observations": observations, "candidate_compatibility_passes_claimed": 0,
             "limits": ["Forty separate module sources, each evaluated only at the authored `result` expression.",
                        "Parser first-error and span observations are Java-only; no Rust or full M2 pass is claimed."]}
    save(output / "audit.json", audit)
    if not provenance["complete"]:
        raise ValueError("declaration acquisition incomplete; raw attempt retained")
    print(json.dumps(audit["statuses"], sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    acquire(args.output)
