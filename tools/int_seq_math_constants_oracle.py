"""Acquire independent pinned Pkl 0.32.0 math constant prerequisite evidence."""
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
FIXTURE = ROOT / "tests/fixtures/m2-int-seq-math-constants"
URI = "file:///stage61/math-constants.pkl"
NAMES_SHA256 = "0a9e6ff25844edb2188e85158d73f7e192170ffe75dc15e10d378137b11235fc"
MODULE_SHA256 = "67f3fd49aa0f2b9a6d825a4139eb991451e98e9d29d8f65f029a57314721ade5"
SOURCES = (
    "external/pkl/stdlib/math.pkl",
    "external/pkl/stdlib/base.pkl",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/math/MathNodes.java",
    "external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/mathModule.pkl",
    "oracle.lock.json", "references.lock",
)
INPUTS = ("IntSeqMathConstantsOracle.java", "run.py", "module.pkl", "names.txt")


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, data):
    path.write_text(json.dumps(data, indent=2, sort_keys=True) + "\n")


def raw_files(root):
    return {p.relative_to(root).as_posix(): sha(p) for p in root.rglob("*") if p.is_file()}


def acquire(output):
    output = Path(output).resolve()
    if output.exists():
        raise ValueError("fresh math constant output required")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("oracle source pin differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("oracle artifact differs")
    if sha(FIXTURE / "names.txt") != NAMES_SHA256 or sha(FIXTURE / "module.pkl") != MODULE_SHA256:
        raise ValueError("authored math inputs differ")
    names = (FIXTURE / "names.txt").read_text().splitlines()
    source = (ROOT / SOURCES[2]).read_text()
    classes = re.findall(r"public abstract static class ([A-Za-z][A-Za-z0-9]*) extends ExternalPropertyNode", source)
    if set(names) != set(classes) or len(names) != len(set(names)) or len(names) != 17:
        raise ValueError("authored names do not match pinned MathNodes properties")
    output.mkdir()
    for name, origin in (("IntSeqMathConstantsOracle.java", ROOT / "tools/java/IntSeqMathConstantsOracle.java"),
                         ("run.py", ROOT / "tools/int_seq_math_constants_oracle.py"),
                         ("module.pkl", FIXTURE / "module.pkl"), ("names.txt", FIXTURE / "names.txt")):
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
    props = dict(re.findall(r"^\s+([\w.]+) = (.*)$", identity.stderr.decode(), re.M))
    if any(props.get(k) != v for k, v in lock["jdk"]["properties"].items()):
        raise ValueError("pinned JDK properties differ; raw output retained")
    home = Path(props["java.home"])
    jdk = {"executable": str(java), "executable_sha256": sha(java),
           "modules_sha256": sha(home / "lib/modules"), "release_sha256": sha(home / "release"),
           "properties": props}
    options = [*lock["jvm_options"], f"-Duser.home={env['HOME']}", f"-Djava.io.tmpdir={env['TMPDIR']}"]
    provenance = {"schema": 1, "scope": "Independent Java math constant prerequisite; no Rust candidate",
                  "module_uri": URI, "host": platform.platform(),
                  "oracle_lock_sha256": sha(ROOT / "oracle.lock.json"),
                  "references_lock_sha256": sha(ROOT / "references.lock"),
                  "reference_revision": lock["source"]["revision"],
                  "artifact": str(jar), "artifact_sha256": sha(jar), "jdk": jdk,
                  "environment": env,
                  "input_before": {name: sha(output / name) for name in INPUTS},
                  "source_before": {name: sha(ROOT / name) for name in SOURCES},
                  "sessions": []}
    save(output / "provenance.json", provenance)
    for repeat in range(2):
        folder = output / f"repeat-{repeat}"
        folder.mkdir()
        command = [str(java), *options, "-cp", str(jar),
                   str(output / "IntSeqMathConstantsOracle.java"), str(folder), str(output), URI]
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
        if process.returncode or entry["query_directories"] != len(names):
            raise ValueError("Java session failed; raw attempt retained")
    provenance["input_after"] = {name: sha(output / name) for name in INPUTS}
    provenance["source_after"] = {name: sha(ROOT / name) for name in SOURCES}
    provenance["artifact_after_sha256"] = sha(jar)
    provenance["stable"] = (provenance["input_before"] == provenance["input_after"] and
                            provenance["source_before"] == provenance["source_after"] and
                            provenance["artifact_sha256"] == provenance["artifact_after_sha256"])
    first, second = raw_files(output / "repeat-0"), raw_files(output / "repeat-1")
    provenance["raw_equal"] = first == second
    provenance["raw_files"] = len(first)
    provenance["raw_differences"] = [name for name in sorted(first.keys() | second.keys())
                                     if first.get(name) != second.get(name)]
    provenance["complete"] = provenance["stable"] and provenance["raw_equal"] and len(provenance["sessions"]) == 2
    save(output / "provenance.json", provenance)
    observations = []
    for index, name in enumerate(names):
        folder = output / "repeat-0" / f"{index:03d}-{name}"
        observations.append({"name": name, "expression": name,
                             "string_status": (folder / "string.status.bin").read_text().strip(),
                             "string_stdout": (folder / "string.stdout.bin").read_text(),
                             "typed_status": (folder / "typed.status.bin").read_text().strip(),
                             "java_type": (folder / "typed.java-type.bin").read_text(),
                             "java_value": (folder / "typed.value-string.bin").read_text(),
                             "raw_be8_hex": (folder / "typed.raw-be8.bin").read_bytes().hex()})
    audit = {"schema": 1, "scope": provenance["scope"], "rows": len(names),
             "public_calls_per_session": 2 * len(names),
             "statuses": dict(Counter(o["string_status"] for o in observations) +
                              Counter(o["typed_status"] for o in observations)),
             "observations": observations, "candidate_compatibility_passes_claimed": 0,
             "limits": ["Math functions are outside this constant supplement.",
                        "Rust native, browser and Node comparison remain separate."]}
    save(output / "audit.json", audit)
    if not provenance["complete"] or audit["statuses"] != {"success": 34}:
        raise ValueError("constant acquisition incomplete; raw attempt retained")
    print(json.dumps(audit["statuses"], sort_keys=True))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    acquire(args.output)
