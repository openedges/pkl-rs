"""Offline verifier for pinned Java math constant prerequisite evidence."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import tarfile

from int_seq_math_constants_oracle import SOURCES, INPUTS, URI

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-int-seq-math-constants"
ARCHIVE_PREFIX = "capture-02"
ARCHIVE_SHA256 = "2fee59a7a22247698c7935e95a6391d30f16cf65106e3b6b9be2891c2cb3c864"
FILESET_SHA256 = "41d256b4334758957185bd2482c322a6df965a601f719e8128b1451cc653e9fd"
ARTIFACT_SHA256 = "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059"
JDK_SHA256 = (
    "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1",
)
BITS = {
    "minInt": "8000000000000000", "minInt8": "ffffffffffffff80",
    "minInt16": "ffffffffffff8000", "minInt32": "ffffffff80000000",
    "maxInt": "7fffffffffffffff", "maxInt8": "000000000000007f",
    "maxInt16": "0000000000007fff", "maxInt32": "000000007fffffff",
    "maxUInt": "7fffffffffffffff", "maxUInt8": "00000000000000ff",
    "maxUInt16": "000000000000ffff", "maxUInt32": "00000000ffffffff",
    "minFiniteFloat": "ffefffffffffffff", "maxFiniteFloat": "7fefffffffffffff",
    "minPositiveFloat": "0000000000000001", "e": "4005bf0a8b145769",
    "pi": "400921fb54442d18",
}
FLOATS = {"minFiniteFloat", "maxFiniteFloat", "minPositiveFloat", "e", "pi"}
ROW_CHANNELS = {"string.status.bin", "typed.status.bin", "typed.raw-be8.bin"} | {
    name + suffix for name in ("expression", "string.stdout", "string.stderr",
                             "typed.java-type", "typed.value-string", "typed.stderr")
    for suffix in (".bin", ".code-units.bin")
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def load_json(raw):
    return json.loads(raw)


def checked(files, prefix, name):
    raw = files[prefix + name + ".bin"]
    units = files[prefix + name + ".code-units.bin"]
    need(len(units) % 2 == 0 and
         units.decode("utf-16-be", "surrogatepass").encode("utf-8", "replace") == raw,
         "producer UTF-8/UTF-16 channels differ")
    return raw.decode("utf-8")


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    root = Path(root)
    need(root.is_dir() and not root.is_symlink(), "fixture directory missing or symlinked")
    entries = list(root.iterdir())
    need(all(p.is_file() and not p.is_symlink() for p in entries), "undeclared or nonregular fixture path")
    root_files = {p.name: sha(p.read_bytes()) for p in entries if p.name != "manifest.json"}
    need(set(root_files) == {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
                             "module.pkl", "names.txt"} and len(entries) == 6,
         "fixture root file set differs")
    need(load_json((root / "manifest.json").read_bytes()) ==
         {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_files}, "fixture manifest differs")
    need(root_files["oracle-evidence.tar.gz"] == expected_archive, "archive SHA differs")
    identity = load_json((root / "archive-identity.json").read_bytes())
    need(identity["schema"] == 1 and identity["capture"] == "stage61-intseq-math-constants-02"
         and identity["prefix"] == ARCHIVE_PREFIX and identity["members"] == 528
         and identity["archive_sha256"] == expected_archive
         and identity["fileset_sha256"] == expected_fileset, "archive identity differs")
    files = {}
    fileset = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and name.startswith(ARCHIVE_PREFIX + "/")
                 and ".." not in Path(name).parts and not Path(name).is_absolute()
                 and name not in files, "unsafe or duplicate archive member")
            raw = archive.extractfile(member).read()
            need(len(raw) == member.size, "archive member size differs")
            files[name] = raw
            fileset.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(files) == 528 and fileset.hexdigest() == expected_fileset, "archive file set differs")
    def capture(name):
        return files[ARCHIVE_PREFIX + "/" + name]
    need(capture("module.pkl") == (root / "module.pkl").read_bytes()
         and capture("names.txt") == (root / "names.txt").read_bytes(), "authored input differs")
    need(set(identity["input_sha256"]) == set(INPUTS) | {"provenance.json", "audit.json"},
         "archive input list differs")
    for name, expected in identity["input_sha256"].items():
        need(sha(capture(name)) == expected, "archive input hash differs")
    names = capture("names.txt").decode().splitlines()
    need(len(names) == 17 and len(set(names)) == 17 and set(names) == set(BITS),
         "constant names differ")
    nodes = capture("sources/external/pkl/pkl-core/src/main/java/org/pkl/core/stdlib/math/MathNodes.java").decode()
    declared = re.findall(r"public abstract static class ([A-Za-z][A-Za-z0-9]*) extends ExternalPropertyNode", nodes)
    need(set(names) == set(declared) and len(declared) == 17, "pinned MathNodes property closure differs")
    math = capture("sources/external/pkl/stdlib/math.pkl").decode()
    need(set(names) == set(re.findall(r"^external ([A-Za-z][A-Za-z0-9]*): ", math, re.M)),
         "pinned math declaration closure differs")
    provenance = load_json(capture("provenance.json"))
    audit = load_json(capture("audit.json"))
    need(provenance["schema"] == 1 and provenance["module_uri"] == URI
         and provenance["complete"] is True and provenance["stable"] is True
         and provenance["raw_equal"] is True and provenance["raw_differences"] == []
         and provenance["raw_files"] == 257, "acquisition completion differs")
    need(provenance["reference_revision"] == "e51fb2f91774265666f94980ec630db794a4db2d"
         and provenance["artifact_sha256"] == provenance["artifact_after_sha256"] == ARTIFACT_SHA256,
         "oracle artifact/source pin differs")
    need(provenance["input_before"] == provenance["input_after"]
         and set(provenance["input_before"]) == set(INPUTS)
         and provenance["source_before"] == provenance["source_after"]
         and set(provenance["source_before"]) == set(SOURCES), "pre/post input/source closure differs")
    for name, expected in provenance["input_before"].items():
        need(sha(capture(name)) == expected, "executed input differs")
    for name, expected in provenance["source_before"].items():
        need(sha(capture("sources/" + name)) == expected, "pinned source bytes differ")
    need(sha(capture("sources/oracle.lock.json")) == provenance["oracle_lock_sha256"]
         and sha(capture("sources/references.lock")) == provenance["references_lock_sha256"],
         "lock source hash differs")
    lock = load_json(capture("sources/oracle.lock.json"))
    jdk = provenance["jdk"]
    need((jdk["executable_sha256"], jdk["modules_sha256"], jdk["release_sha256"]) == JDK_SHA256
         and all(jdk["properties"].get(k) == v for k, v in lock["jdk"]["properties"].items()),
         "pinned JDK identity differs")
    sessions = provenance["sessions"]
    need(len(sessions) == 2 and [s["repeat"] for s in sessions] == [0, 1]
         and len({s["pid"] for s in sessions}) == 2
         and sessions[0]["finished_epoch"] <= sessions[1]["started_epoch"],
         "ordered fresh JVM coverage differs")
    raw_sets = []
    observed = []
    for session in sessions:
        repeat = session["repeat"]
        base = ARCHIVE_PREFIX + f"/repeat-{repeat}/"
        cwd = Path(session["cwd"])
        command = [jdk["executable"], *lock["jvm_options"],
                   f"-Duser.home={provenance['environment']['HOME']}",
                   f"-Djava.io.tmpdir={provenance['environment']['TMPDIR']}", "-cp",
                   provenance["artifact"], str(cwd / "IntSeqMathConstantsOracle.java"),
                   str(cwd / f"repeat-{repeat}"), str(cwd), URI]
        need(cwd.is_absolute() and session["command"] == command
             and session["exit_code"] == 0 and session["query_directories"] == 17,
             "session command/result differs")
        for channel in ("stdout", "stderr"):
            raw = files[base + "process." + channel + ".bin"]
            need(sha(raw) == session[channel + "_sha256"]
                 and len(raw) == session[channel + "_bytes"], "process channel hash/size differs")
        need(files[base + "process.stderr.bin"] == b"", "process stderr differs")
        protocol = ["session-open boundary=math-constants rows=17 public-calls=34"]
        for index, name in enumerate(names):
            folder = base + f"{index:03d}-{name}/"
            actual = {path[len(folder):] for path in files if path.startswith(folder)}
            need(actual == ROW_CHANNELS, "math row channel set differs")
            expression = checked(files, folder, "expression")
            string_value = checked(files, folder, "string.stdout")
            string_error = checked(files, folder, "string.stderr")
            java_type = checked(files, folder, "typed.java-type")
            java_value = checked(files, folder, "typed.value-string")
            typed_error = checked(files, folder, "typed.stderr")
            bits = files[folder + "typed.raw-be8.bin"]
            need(expression == name and string_error == typed_error == ""
                 and files[folder + "string.status.bin"] == b"success\n"
                 and files[folder + "typed.status.bin"] == b"success\n"
                 and string_value == java_value and len(bits) == 8 and bits.hex() == BITS[name]
                 and java_type == ("java.lang.Double" if name in FLOATS else "java.lang.Long"),
                 "math value/type/bits differ")
            for kind in ("string", "typed"):
                call = f"{kind}-{index}-{name}"
                protocol.extend((f"call-enter={call}", f"call-exit={call} status=success"))
            if repeat == 0:
                observed.append({"name": name, "expression": name, "string_status": "success",
                                 "string_stdout": string_value, "typed_status": "success",
                                 "java_type": java_type, "java_value": java_value,
                                 "raw_be8_hex": bits.hex()})
        protocol.append("session-closed boundary=math-constants rows=17 public-calls=34 success=34 errors=0")
        need(files[base + "process.stdout.bin"] == ("\n".join(protocol) + "\n").encode(),
             "complete math process protocol differs")
        raw_set = {path[len(base):]: sha(raw) for path, raw in files.items() if path.startswith(base)}
        need(len(raw_set) == 257, "math raw file count differs")
        raw_sets.append(raw_set)
    need(raw_sets[0] == raw_sets[1], "math raw repetitions differ")
    need(audit == {"schema": 1, "scope": provenance["scope"], "rows": 17,
                   "public_calls_per_session": 34, "statuses": {"success": 34},
                   "observations": observed, "candidate_compatibility_passes_claimed": 0,
                   "limits": ["Math functions are outside this constant supplement.",
                              "Rust native, browser and Node comparison remain separate."]},
         "derived math audit differs")
    return {"rows": 17, "public_calls_per_session": 34, "statuses": {"success": 34},
            "archive_sha256": expected_archive, "fileset_sha256": expected_fileset,
            "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
