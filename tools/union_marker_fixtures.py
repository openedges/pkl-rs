"""Offline raw verifier for forty separate Pkl 0.32.0 union marker diagnostic modules."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import tarfile

from union_marker_oracle import MODULE_URI, PROGRAMS_SHA256, SOURCES, program_input
from collection_conversions_fixtures import (
    ARTIFACT_SHA256, JDK_SHA256, REVISION, checked, need,
    validate_process_protocol, validate_row_channels,
)

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-union-marker-diagnostics"
ARCHIVE_PREFIX = "capture-01"
ARCHIVE_SHA256 = "1c5b48de0cb8f7ec937d5314edb3232c1c1756d9b0665df1681b9da65597b419"
FILESET_SHA256 = "265cefec5c62b9fbdf73ae4c95d86d960a7cbe7d5e029d5031223ff7de76fe55"
MEMBERS = 2085
PROGRAMS = 40
STATUSES = {"success": 9, "error": 31}
RAW_FILES_PER_REPEAT = 23
INPUT_SHA256 = {
    "CollectionConversionsOracle.java": "6488595d614869fb59f50fe8d175f18e1d695031f768a38bfdc85f7fe28c9cc0",
    "imported-construction-oracle.py": "a73e1fa1fd047ff4477197c3715571ce21499ac5905148ecc7b045a1c63438cb",
    "programs.json": PROGRAMS_SHA256,
    "run.py": "09655dcdfc8ae618f180ccf822c48dce6e93c235b5ca8f17747e8e5f24b8da58",
}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    root = Path(root)
    need(root.is_dir() and not root.is_symlink(), "union marker diagnostic fixture root missing")
    authored = json.loads((root / "programs.json").read_bytes())
    programs = authored["programs"]
    need(authored["schema"] == 1 and authored["baseline"] == "Pkl 0.32.0"
         and authored["module_uri"] == MODULE_URI and authored["repetitions"] == 2
         and len(programs) == PROGRAMS and len({p["id"] for p in programs}) == PROGRAMS
         and sha((root / "programs.json").read_bytes()) == PROGRAMS_SHA256,
         "authored union marker diagnostic program set differs")
    expected_modules = {program["module_file"] for program in programs}
    need(all(program["module_file"] == f"modules/{index:02d}-{program['id']}.pkl"
             and re.fullmatch(r"[a-z0-9-]+", program["id"])
             and program["query"] == "result"
             and sha((root / program["module_file"]).read_bytes()) == program["module_sha256"]
             and (root / program["module_file"]).stat().st_size == program["module_bytes"]
             for index, program in enumerate(programs)), "authored module bytes/order differ")
    expected_root_files = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
                           "programs.json", "manifest.json"} | expected_modules
    actual_root_files = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file()}
    actual_root_dirs = {p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_dir()}
    need(actual_root_files == expected_root_files and actual_root_dirs == {"modules"}
         and all(not p.is_symlink() for p in root.rglob("*")),
         "undeclared or nonregular union marker diagnostic fixture path")
    root_hashes = {name: sha((root / name).read_bytes()) for name in expected_root_files
                   if name != "manifest.json"}
    manifest = json.loads((root / "manifest.json").read_bytes())
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_hashes},
         "union marker diagnostic fixture root manifest differs")
    need(root_hashes["oracle-evidence.tar.gz"] == expected_archive,
         "union marker diagnostic archive SHA differs")
    archive_identity = json.loads((root / "archive-identity.json").read_bytes())
    need(archive_identity == {"schema": 1, "archive_sha256": expected_archive,
         "fileset_sha256": expected_fileset, "members": MEMBERS,
         "captures": {ARCHIVE_PREFIX: "stage66-union-marker-oracle-01"},
         "programs": PROGRAMS, "statuses": STATUSES, "oracle_input_sha256": INPUT_SHA256},
         "union marker diagnostic archive identity differs")
    files = {}
    digest = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and not Path(name).is_absolute()
                 and ".." not in Path(name).parts and name.startswith(ARCHIVE_PREFIX + "/")
                 and name not in files, "unsafe or duplicate union marker diagnostic archive member")
            raw = archive.extractfile(member).read()
            need(len(raw) == member.size, "union marker diagnostic archive member size differs")
            files[name] = raw
            digest.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(files) == MEMBERS and digest.hexdigest() == expected_fileset,
         "union marker diagnostic archive fileset differs")

    def capture(name):
        return files[ARCHIVE_PREFIX + "/" + name]

    for name, expected in INPUT_SHA256.items():
        need(sha(capture(name)) == expected, "union marker diagnostic producer/helper input differs")
    need(capture("programs.json") == (root / "programs.json").read_bytes(),
         "authored union marker diagnostic program manifest differs")
    for program in programs:
        need(capture(program["module_file"]) == (root / program["module_file"]).read_bytes(),
             "archived union marker diagnostic module differs")
    provenance = json.loads(capture("provenance.json"))
    audit = json.loads(capture("audit.json"))
    need(provenance["schema"] == 1 and provenance["complete"] is True
         and provenance["stable"] is True and provenance["module_uri"] == MODULE_URI
         and provenance["reference_revision"] == REVISION
         and provenance["artifact_sha256"] == provenance["artifact_after_sha256"] == ARTIFACT_SHA256,
         "union marker diagnostic provenance completion/pin differs")
    input_names = set(INPUT_SHA256) | expected_modules
    need(provenance["input_before"] == provenance["input_after"]
         and set(provenance["input_before"]) == input_names
         and provenance["source_before"] == provenance["source_after"]
         and set(provenance["source_before"]) == set(SOURCES),
         "union marker diagnostic source/input closure differs")
    for name, expected in provenance["input_before"].items():
        need(sha(capture(name)) == expected, "archived union marker diagnostic input differs")
    for name, expected in provenance["source_before"].items():
        need(sha(capture("sources/" + name)) == expected,
             "archived pinned union marker diagnostic source differs")
    need(sha(capture("sources/oracle.lock.json")) == provenance["oracle_lock_sha256"]
         and sha(capture("sources/references.lock")) == provenance["references_lock_sha256"],
         "union marker diagnostic lock source differs")
    lock = json.loads(capture("sources/oracle.lock.json"))
    need(lock["version"] == "0.32.0" and lock["source"]["revision"] == REVISION
         and lock["artifact"]["sha256"] == ARTIFACT_SHA256
         and ("pkl.revision=" + REVISION).encode() in capture("sources/references.lock"),
         "union marker diagnostic pinned locks differ")
    jdk = provenance["jdk"]
    raw_identity_stdout = capture("java-identity.stdout.bin")
    raw_identity_stderr = capture("java-identity.stderr.bin")
    properties = dict(re.findall(r"^\s+([\w.]+) = (.*)$", raw_identity_stderr.decode(), re.M))
    need(raw_identity_stdout == b"" and properties == jdk["properties"]
         and b'openjdk version "21.0.12.1"' in raw_identity_stderr
         and (jdk["executable_sha256"], jdk["modules_sha256"], jdk["release_sha256"]) == JDK_SHA256
         and all(properties.get(key) == value for key, value in lock["jdk"]["properties"].items()),
         "union marker diagnostic raw JDK identity differs")
    sessions = provenance["sessions"]
    need(len(sessions) == PROGRAMS * 2 and len({s["pid"] for s in sessions}) == PROGRAMS * 2
         and all(s["pid"] > 0 and s["started_epoch"] < s["finished_epoch"]
                 and s["exit_code"] == 0 and s["query_directories"] == 1 for s in sessions)
         and all(left["finished_epoch"] <= right["started_epoch"]
                 for left, right in zip(sessions, sessions[1:])),
         "union marker diagnostic JVM sessions not fresh/ordered/complete")
    observed = []
    counts = Counter()
    for index, program in enumerate(programs):
        base = f"{ARCHIVE_PREFIX}/programs/{index:02d}-{program['id']}/"
        need(capture(f"programs/{index:02d}-{program['id']}/module.pkl") ==
             (root / program["module_file"]).read_bytes(), "executed module differs")
        need(json.loads(files[base + "cases.json"]) == program_input(program)
             and files[base + "queries.tsv"] ==
             f"string\t{program['id']}\t{program['query']}\n".encode(),
             "union marker diagnostic generated call input differs")
        raw_sets = []
        for repeat in range(2):
            session = sessions[index * 2 + repeat]
            process_base = base + f"repeat-{repeat}/"
            result_dir = process_base + f"000-{program['id']}/"
            cwd = Path(session["cwd"])
            actual_command = [jdk["executable"], *lock["jvm_options"],
                              f"-Duser.home={provenance['environment']['HOME']}",
                              f"-Djava.io.tmpdir={provenance['environment']['TMPDIR']}",
                              "-cp", provenance["artifact"],
                              str(cwd / "CollectionConversionsOracle.java"),
                              str(cwd / f"programs/{index:02d}-{program['id']}/repeat-{repeat}"),
                              str(cwd / f"programs/{index:02d}-{program['id']}"), MODULE_URI]
            need(cwd.is_absolute() and session["index"] == index
                 and session["program"] == program["id"] and session["repeat"] == repeat
                 and session["command"] == actual_command,
                 "union marker diagnostic Java argv/cwd/order differs")
            for channel in ("stdout", "stderr"):
                raw = files[process_base + f"process.{channel}.bin"]
                need(sha(raw) == session[channel + "_sha256"]
                     and len(raw) == session[channel + "_bytes"],
                     "union marker diagnostic whole-process channel differs")
            need(files[process_base + "process.stderr.bin"] == b"",
                 "union marker diagnostic process stderr differs")
            validate_row_channels(files, result_dir)
            need(checked(files, result_dir, "mode") == "string"
                 and checked(files, result_dir, "expression") == program["query"],
                 "union marker diagnostic row identity differs")
            stdout = checked(files, result_dir, "result.stdout")
            stderr = checked(files, result_dir, "result.stderr")
            error_class = checked(files, result_dir, "result.error-class")
            exception = checked(files, result_dir, "result.exception-to-string")
            cause_class = checked(files, result_dir, "result.cause-class")
            cause_message = checked(files, result_dir, "result.cause-message")
            cause_string = checked(files, result_dir, "result.cause-to-string")
            java_type = checked(files, result_dir, "result.java-type")
            status = files[result_dir + "result.status.bin"].decode()
            need(status in ("success\n", "error\n"), "union marker diagnostic API/internal error status differs")
            if status == "success\n":
                need(stderr == java_type == error_class == exception == cause_class ==
                     cause_message == cause_string == "", "union marker diagnostic success channels differ")
            else:
                need(stdout == java_type == "" and stderr and error_class == "org.pkl.core.PklException"
                     and exception.startswith(error_class + ": ")
                     and bool(cause_class) == bool(cause_message) == bool(cause_string)
                     and (not cause_class or cause_string.startswith(cause_class + ": ")),
                     "union marker diagnostic error channels differ")
            call = f"string-0-{program['id']}"
            validate_process_protocol(files[process_base + "process.stdout.bin"], [
                "session-open mode=string rows=1", f"call-enter={call}",
                f"call-exit={call} status={status.strip()}",
                f"session-closed mode=string rows=1 public-calls=1 success={int(status == 'success' + chr(10))} errors={int(status != 'success' + chr(10))}",
            ])
            raw_set = {name[len(process_base):]: sha(raw) for name, raw in files.items()
                       if name.startswith(process_base)}
            need(len(raw_set) == RAW_FILES_PER_REPEAT,
                 "union marker diagnostic per-session raw file count differs")
            raw_sets.append(raw_set)
            if repeat == 0:
                lines = stderr.splitlines()
                counts[status.strip()] += 1
                observed.append({"id": program["id"], "module_file": program["module_file"],
                                 "module_sha256": program["module_sha256"],
                                 "query": program["query"], "status": status.strip(),
                                 "stdout": stdout,
                                 "stdout_sha256": sha(files[result_dir + "result.stdout.bin"]),
                                 "stderr_sha256": sha(files[result_dir + "result.stderr.bin"]),
                                 "error_class": error_class,
                                 "first_error_line": lines[1] if len(lines) > 1 else ""})
        need(raw_sets[0] == raw_sets[1], "union marker diagnostic raw repetition differs")
    need(dict(counts) == STATUSES and audit == {"schema": 1, "scope": provenance["scope"],
         "programs": PROGRAMS, "statuses": dict(counts), "observations": observed,
         "candidate_compatibility_passes_claimed": 0,
         "limits": ["Forty separate module sources, each evaluated only at the authored `result` expression.",
                    "Parser first-error and span observations are Java-only; no Rust or full M2 pass is claimed."]},
         "derived union marker diagnostic audit differs")
    return {"baseline": "Pkl 0.32.0", "programs": PROGRAMS, "statuses": dict(counts),
            "archive_sha256": expected_archive, "fileset_sha256": expected_fileset,
            "raw_repetitions_equal": True, "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
