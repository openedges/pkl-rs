"""Acquire independent pinned Java 0.32.0 converted-object control observations."""
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
FIXTURE = ROOT / "tests/fixtures/m2-collection-conversion-controls"
MODULE_URI = "file:///stage62/collection-conversion-controls.pkl"
CASES_SHA256 = "2dbd453c55268cf80fdb8765d6bcea44535eed917f418eff27d3566f78f0d453"
MODULE_SHA256 = "90c2a9ef0fabcc8f8b22e4abf036caedbe7773663e605363ebecf9a16a7d9102"
QUERIES_SHA256 = "f7affd443c1fa5d0287b820e3ede1eb0d67782edf80000c4518b72f35aad1c09"
INPUTS = ("CollectionConversionsOracle.java", "run.py", "module.pkl", "queries.tsv", "cases.json")
SOURCES = (
    "external/pkl/stdlib/base.pkl",
    *(
        f"external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/{name}.java"
        for name in ("BaseNodes", "TypedNodes", "DynamicNodes", "MappingNodes",
                     "ListingNodes", "MapNodes", "ListNodes", "SetNodes")
    ),
    *(
        f"external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/{name}.java"
        for name in ("VmObject", "VmObjectLike", "VmTyped", "VmDynamic", "VmMapping", "VmListing",
                     "VmMap", "VmList", "VmSet", "VmBytes", "VmClass", "VmCollection",
                     "BaseModule", "VmUtils", "Identifier", "VmNull", "VmListingOrMapping",
                     "VmExceptionBuilder", "MemberLookupSuggestions",
                     "VmObjectBuilder", "VmValueRenderer")
    ),
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/TypeNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/ReadPropertyNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/lambda/ApplyVmFunction1Node.java",
    *(
        f"external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/{name}.java"
        for name in ("ExternalMethod0Node", "ExternalMethod1Node", "ExternalMethod2Node")
    ),
    *(
        f"external/pkl/pkl-core/src/main/java/org/pkl/core/ast/member/{name}.java"
        for name in ("ObjectMember", "TypeCheckedPropertyNode", "ListingOrMappingTypeCastNode",
                     "DelegateToExtraStorageObjNode",
                     "DelegateToExtraStorageObjOrParentNode", "DelegateToExtraStorageMapOrParentNode")
    ),
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/generator/GeneratorForNode.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/generator/GeneratorSpreadNode.java",
    *(
        f"external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/api/{name}.pkl"
        for name in ("typed", "dynamic", "mapping", "listing", "map", "list", "set", "bytes")
    ),
    *(
        f"external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/generators/{name}.pkl"
        for name in ("spreadSyntaxListing", "spreadSyntaxDynamic", "spreadSyntaxMapping",
                     "elementGenerators", "elementGeneratorsTyped")
    ),
    *(
        f"external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/{name}"
        for name in ("pklbinaryTest.pkl", "listings/hashCode.pkl", "mappings/hashCode.pkl",
                     "objects/hashCode.pkl")
    ),
    "oracle.lock.json", "references.lock",
)


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def files(root):
    return {path.relative_to(root).as_posix(): sha(path) for path in root.rglob("*") if path.is_file()}


def acquire(output, *, fixture=FIXTURE):
    output = Path(output).resolve()
    fixture = Path(fixture)
    if output.exists():
        raise ValueError("fresh collection conversion control output required")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("oracle source pin differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("oracle artifact differs")
    cases = json.loads((fixture / "cases.json").read_bytes())
    rows = cases["rows"]
    if (cases["schema"] != 1 or cases["baseline"] != "Pkl 0.32.0"
            or cases["module_uri"] != MODULE_URI
            or cases["boundary"] != "string"
            or cases["repetitions"] != 2 or len(rows) != 58
            or any(row["mode"] != "string" for row in rows)
            or sha(fixture / "cases.json") != CASES_SHA256
            or sha(fixture / "module.pkl") != MODULE_SHA256
            or sha(fixture / "queries.tsv") != QUERIES_SHA256):
        raise ValueError("authored row set differs")
    if (fixture / "queries.tsv").read_text() != "".join(
            f"{row['mode']}\t{row['id']}\t{row['expression']}\n" for row in rows):
        raise ValueError("authored ordered queries differ")
    output.mkdir()
    for name, origin in (("CollectionConversionsOracle.java", ROOT / "tools/java/CollectionConversionsOracle.java"),
                         ("run.py", ROOT / "tools/collection_conversion_controls_oracle.py"),
                         ("module.pkl", fixture / "module.pkl"),
                         ("queries.tsv", fixture / "queries.tsv"),
                         ("cases.json", fixture / "cases.json")):
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
    provenance = {"schema": 1, "scope": "Independent public converted-object Java string controls; no Rust candidate",
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
        command = [str(java), *options, "-cp", str(jar), str(output / "CollectionConversionsOracle.java"),
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
    args = parser.parse_args()
    acquire(args.output)
