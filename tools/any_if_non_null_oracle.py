"""Acquire independent Pkl 0.32.0 Any.ifNonNull public string evidence."""
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
FIXTURE = ROOT / "tests/fixtures/m2-any-if-non-null"
SOURCE_NAMES = [
    "external/pkl/stdlib/base.pkl",
    "external/pkl/stdlib/ref.pkl",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/base/AnyNodes.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/ast/lambda/ApplyVmFunction1Node.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmNull.java",
    "external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/ExternalMethod1Node.java",
    "oracle.lock.json", "references.lock",
]
INPUT_NAMES = ["AnyIfNonNullOracle.java", "run.py", "module.pkl", "queries.tsv", "cases.json"]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def case_rows():
    groups = {
        "scalar": [
            ("int-identity", "10.ifNonNull((x) -> x)"),
            ("int-transform", "10.ifNonNull((x) -> x + 2)"),
            ("int-null-result", "10.ifNonNull((x) -> null)"),
            ("float", "1.5.ifNonNull((x) -> x + 0.5)"),
            ("boolean", "true.ifNonNull((x) -> !x)"),
            ("string", '"abc".ifNonNull((x) -> x.length)'),
            ("duration", "2.s.ifNonNull((x) -> x.value)"),
            ("datasize", "3.mb.ifNonNull((x) -> x.unit)"),
            ("bytes", "Bytes(1, 2, 3).ifNonNull((x) -> x.length)"),
            ("pair", 'Pair("a", 4).ifNonNull((x) -> x.second)'),
            ("regex", 'Regex("a+").ifNonNull((x) -> x.toString())'),
            ("intseq", 'IntSeq(1, 3).ifNonNull((x) -> x.length)'),
        ],
        "null": [
            ("plain", "null.ifNonNull((x) -> x)"),
            ("plain-constant", 'null.ifNonNull((x) -> "unreachable")'),
            ("plain-throw", 'null.ifNonNull((x) -> throw("unreachable"))'),
            ("default-identity", 'Null(new Dynamic { marker = "default" }).ifNonNull((x) -> x)'),
            ("default-constant", 'Null(new Dynamic { marker = "default" }).ifNonNull((x) -> "unreachable")'),
            ("default-throw", 'Null(new Dynamic { marker = "default" }).ifNonNull((x) -> throw("unreachable"))'),
            ("default-coalesce", 'Null(new Dynamic { marker = "default" }).ifNonNull((x) -> x) ?? "fallback"'),
            ("default-amend", '((Null(new Dynamic { marker = "base" }).ifNonNull((x) -> x)) { extra = 7 }).extra'),
            ("default-equality", 'Null(new Dynamic { marker = "default" }).ifNonNull((x) -> x) == null'),
            ("default-lambda", 'Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> "unreachable")'),
            ("default-lambda-amend", '((Null(() -> new Dynamic { marker = "deferred" }).ifNonNull((x) -> x)) { extra = 8 }).extra'),
            ("nested", 'null.ifNonNull((x) -> x).ifNonNull((x) -> "bad")'),
        ],
        "collections": [
            ("list", "List(1, 2).ifNonNull((x) -> x.length)"),
            ("set", "Set(1, 2).ifNonNull((x) -> x.length)"),
            ("map", 'Map("a", 1).ifNonNull((x) -> x["a"])'),
            ("listing", "(new Listing { 1; 2 }).ifNonNull((x) -> x.length)"),
            ("mapping", '(new Mapping { ["a"] = 1 }).ifNonNull((x) -> x["a"])'),
            ("dynamic", '(new Dynamic { n = 7 }).ifNonNull((x) -> x.n)'),
            ("typed-object", "(new Box {}).ifNonNull((x) -> x.value)"),
            ("module", "module.ifNonNull((x) -> x.marker)"),
        ],
        "metavalues": [
            ("class-value", "Box.ifNonNull((x) -> x.simpleName)"),
            ("typealias-value", "BoxAlias.ifNonNull((x) -> x.toString())"),
            ("function-value", "((x) -> x + 1).ifNonNull((f) -> f.apply(4))"),
            ("typed-function-value", "typedTransform.ifNonNull((f) -> f.apply(4))"),
            ("reference-type", 'import("pkl:ref").Reference.ifNonNull((x) -> x.simpleName)'),
            ("reference-instance", 'referenceValue.ifNonNull((x) -> x.getPath().length)'),
            ("reference-path", 'referenceValue.ifNonNull((x) -> x.value.getPath().length)'),
        ],
        "types-and-arity": [
            ("int-wrong-param", '10.ifNonNull((x: String) -> x)'),
            ("null-wrong-param", 'null.ifNonNull((x: String) -> x)'),
            ("default-wrong-param", 'Null(new Dynamic { n = 3 }).ifNonNull((x: String) -> x)'),
            ("nonfunction", "10.ifNonNull(4)"),
            ("null-nonfunction", "null.ifNonNull(4)"),
            ("default-nonfunction", "Null(new Dynamic { n = 3 }).ifNonNull(4)"),
            ("no-args", "10.ifNonNull()"),
            ("null-no-args", "null.ifNonNull()"),
            ("too-many", "10.ifNonNull((x) -> x, 4)"),
            ("null-too-many", "null.ifNonNull((x) -> x, 4)"),
            ("null-too-many-throw", 'null.ifNonNull((x) -> x, throw("second argument"))'),
            ("value-too-many-throw", '10.ifNonNull((x) -> x, throw("second argument"))'),
            ("zero-param", "10.ifNonNull(() -> 9)"),
            ("null-zero-param", "null.ifNonNull(() -> 9)"),
            ("two-param", "10.ifNonNull((x, y) -> x)"),
            ("null-two-param", "null.ifNonNull((x, y) -> x)"),
            ("typed-return-value", "typedTransform.ifNonNull((f) -> f.apply(4))"),
            ("typed-return-null", "null.ifNonNull((x: Int) -> x + 1)"),
        ],
        "evaluation-order": [
            ("null-argument-throw", 'null.ifNonNull(throw("argument first"))'),
            ("value-argument-throw", '10.ifNonNull(throw("argument first"))'),
            ("null-receiver-throw", 'throw("receiver first").ifNonNull(throw("argument second"))'),
            ("nested-first-error", '10.ifNonNull((x) -> throw("transform first")).ifNonNull(throw("argument second"))'),
            ("null-deferred-property", 'null.ifNonNull((x) -> lazyBomb)'),
            ("value-forced-property", '10.ifNonNull((x) -> lazyBomb)'),
            ("null-typed-function", 'null.ifNonNull(typedTransform)'),
            ("value-typed-function", '10.ifNonNull(typedTransform)'),
            ("null-closure-throw", 'null.ifNonNull((x) -> throw("closure boom"))'),
            ("value-closure-throw", '10.ifNonNull((x) -> throw("closure boom"))'),
            ("retry-after-error", "10.ifNonNull((x) -> x + 1)"),
            ("repeat-same-receiver", "holder.value.ifNonNull((x) -> x + 1)"),
            ("repeat-other-result", "holder.value.ifNonNull((x) -> x + 2)"),
        ],
        "dispatch": [
            ("dynamic-custom-method", '(new Dynamic { local function ifNonNull(f) = "custom" }).ifNonNull((x) -> "base")'),
            ("dynamic-shadow-property", '(new Dynamic { ifNonNull = "shadow" }).ifNonNull((x) -> "base")'),
            ("object-custom-method", '(new BoxWithMethod {}).ifNonNull((x) -> "base")'),
            ("object-custom-wrong-arity", '(new BoxWithMethod {}).ifNonNull()'),
            ("object-super-method", '(new ChildMethod {}).ifNonNull((x) -> "base")'),
            ("object-super-any", '(new DelegatingMethod {}).ifNonNull((x) -> x.value)'),
            ("implicit-module-method", 'ifNonNull((x) -> x.marker)'),
            ("object-method-throw-arg", '(new BoxWithMethod {}).ifNonNull(throw("custom argument"))'),
            ("generic-on-any", "(10 as Any).ifNonNull((x) -> x.toString())"),
        ],
        "const": [
            ("literal-property", "constLiteral"),
            ("module-implicit", "constImplicit()"),
            ("module-explicit", "constExplicit()"),
            ("class-inherited", "(new ConstMethod {}).run()"),
            ("class-shadow", "(new ConstShadow {}).run()"),
            ("class-shadow-direct", "(new ConstShadow {}).ifNonNull((x) -> x)"),
            ("class-ordinary-direct", "(new ConstMethod {}).ifNonNull((x) -> x.value)"),
        ],
    }
    return [{"id": f"{group}-{name}", "group": group, "expression": expr}
            for group, entries in groups.items() for name, expr in entries]


MODULE = '''// Stage58 Any.ifNonNull public expression source; expressions are separate calls.
import "pkl:ref"
marker = "stage58"
class Box { value: Int = 7 }
typealias BoxAlias = Box
class RDomain extends ref.Domain { function renderReference(reference: ref.Reference<RDomain, Any>): String = "ref" }
referenceValue = ref.Reference(new RDomain {}, Box, "data")
open class BoxWithMethod { function ifNonNull(f) = "override" }
class ChildMethod extends BoxWithMethod { function ifNonNull(f) = super.ifNonNull(f) + ":child" }
class DelegatingMethod { value: Int = 7; function ifNonNull(f) = super.ifNonNull(f) }
function makeValue(x: Int): Int = x + 1
typedTransform = (x: Int) -> x + 1
lazyBomb = throw("lazy property boom")
holder = new { value = 10 }
const constLiteral = 1.ifNonNull((x) -> x)
const function constImplicit() = ifNonNull((x) -> x.marker)
const function constExplicit() = module.ifNonNull((x) -> x.marker)
class ConstMethod { value: Int = 9; const function run() = this.ifNonNull((x) -> x.value) }
class ConstShadow { const function ifNonNull(f) = "const shadow"; const function run() = this.ifNonNull((x) -> x) }
'''


def prepare():
    FIXTURE.mkdir(parents=True, exist_ok=True)
    rows = case_rows()
    manifest = {"schema": 1, "baseline": "Pkl 0.32.0", "module_uri": "file:///stage58/any-if-non-null.pkl",
                "boundary": "string", "repetitions": 2, "rows": rows}
    (FIXTURE / "module.pkl").write_text(MODULE)
    (FIXTURE / "cases.json").write_text(json.dumps(manifest, indent=2) + "\n")
    (FIXTURE / "queries.tsv").write_text("".join(r["id"] + "\t" + r["expression"] + "\n" for r in rows))
    return len(rows)


def acquire(output, *, fixture=FIXTURE, authored_rows=None, authored_module=None, run_source=None):
    output = Path(output).resolve()
    fixture = Path(fixture)
    authored_rows = case_rows() if authored_rows is None else authored_rows
    authored_module = MODULE if authored_module is None else authored_module
    run_source = ROOT / "tools/any_if_non_null_oracle.py" if run_source is None else Path(run_source)
    if output.exists():
        raise ValueError(f"fresh output required: {output}")
    lock = json.loads((ROOT / "oracle.lock.json").read_bytes())
    if lock["version"] != "0.32.0" or lock["source"]["revision"] != "e51fb2f91774265666f94980ec630db794a4db2d":
        raise ValueError("oracle source pin differs")
    jar = ROOT / ".cache/oracle" / lock["artifact"]["filename"]
    if sha(jar) != lock["artifact"]["sha256"]:
        raise ValueError("oracle artifact differs")
    m = json.loads((fixture / "cases.json").read_bytes())
    if m["rows"] != authored_rows or m["boundary"] != "string" or m["repetitions"] != 2:
        raise ValueError("authored case manifest differs")
    if (fixture / "module.pkl").read_text() != authored_module:
        raise ValueError("authored module differs")
    if (fixture / "queries.tsv").read_text() != "".join(r["id"] + "\t" + r["expression"] + "\n" for r in m["rows"]):
        raise ValueError("authored queries differ")
    output.mkdir(parents=True)
    for name, origin in [("AnyIfNonNullOracle.java", ROOT / "tools/java/AnyIfNonNullOracle.java"),
                         ("run.py", run_source),
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
    p = {"schema": 1, "scope": "Independent Java public expression-string Any.ifNonNull observations; no Rust candidate",
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
        command = [str(java), *options, "-cp", str(jar), str(output / "AnyIfNonNullOracle.java"),
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
    group.add_argument("--prepare", action="store_true")
    group.add_argument("--output", type=Path)
    args = parser.parse_args()
    print(prepare() if args.prepare else json.dumps(acquire(args.output)["statuses"], sort_keys=True))
