"""Offline integrity verifier for independent Pkl 0.32.0 IntSeq Java evidence."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import tarfile

from int_seq_values_oracle import SOURCES

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-int-seq-values"
ARCHIVE_PREFIX = "capture-01"
EXPORT_PREFIX = "export-supplement-01"
ARCHIVE_SHA256 = "7e8ce6e9b4bd05174570115f8d3cc02d7e286d54f8a24830d1fe95a9b7dbd89b"
FILESET_SHA256 = "355bc2feddfb0274821f9956a320172d36934652e9ff3b4e2180c2e0106cf158"
ARTIFACT_SHA256 = "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059"
JDK_SHA256 = (
    "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1",
)
REVISION = "e51fb2f91774265666f94980ec630db794a4db2d"
MODULE_URI = "file:///stage61/intseq-values.pkl"
ROW_CHANNELS = {"result.status.bin"} | {
    name + suffix for name in (
        "mode", "expression", "result.stdout", "result.stderr", "result.error-class",
        "result.exception-to-string", "result.cause-class", "result.cause-message",
        "result.cause-to-string", "result.java-type")
    for suffix in (".bin", ".code-units.bin")
}
INPUT_NAMES = {"IntSeqValuesOracle.java", "run.py", "module.pkl", "queries.tsv", "cases.json"}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def load_json(raw):
    return json.loads(raw)


def validate_rows(rows, queries, *, mode="string", count=237):
    need(len(rows) == count and len({row["id"] for row in rows}) == count
         and all(set(row) == {"id", "group", "expression", "mode"}
                 and row["mode"] == mode and re.fullmatch(r"[a-z0-9-]+", row["id"])
                 and "\t" not in row["expression"] and "\n" not in row["expression"]
                 for row in rows), "case rows differ")
    need(queries == "".join(
        f"{mode}\t{row['id']}\t{row['expression']}\n" for row in rows).encode(),
        "query order differs")


def validate_row_channels(files, folder):
    actual = {name[len(folder):] for name in files if name.startswith(folder)}
    need(actual == ROW_CHANNELS, f"row channel set differs: {folder}")


def validate_process_protocol(actual, protocol):
    need(actual == ("\n".join(protocol) + "\n").encode(), "complete process protocol differs")


def checked(files, prefix, name):
    raw = files[prefix + name + ".bin"]
    units = files[prefix + name + ".code-units.bin"]
    need(len(units) % 2 == 0 and
         units.decode("utf-16-be", "surrogatepass").encode("utf-8", "replace") == raw,
         f"UTF-8/UTF-16 producer channels differ: {prefix}{name}")
    return raw.decode("utf-8")


def validate_capture(files, prefix, *, mode, rows_count, expected_statuses, expected_raw_files,
                     module_uri=MODULE_URI, source_names=SOURCES):
    def capture(name):
        return files[prefix + "/" + name]

    cases = load_json(capture("cases.json"))
    provenance = load_json(capture("provenance.json"))
    audit = load_json(capture("audit.json"))
    boundary = "string" if mode == "string" else "typed-export-supplement"
    need(cases["schema"] == 1 and cases["baseline"] == "Pkl 0.32.0"
         and cases["module_uri"] == module_uri and cases["boundary"] == boundary
         and cases["repetitions"] == 2, "case boundary differs")
    rows = cases["rows"]
    validate_rows(rows, capture("queries.tsv"), mode=mode, count=rows_count)
    need(provenance["schema"] == 1 and provenance["complete"] is True
         and provenance["stable"] is True and provenance["raw_equal"] is True
         and provenance["raw_differences"] == [] and provenance["raw_files"] == expected_raw_files,
         "acquisition completion/repetition differs")
    need(provenance["reference_revision"] == REVISION
         and provenance["artifact_sha256"] == provenance["artifact_after_sha256"] == ARTIFACT_SHA256,
         "oracle source/artifact identity differs")
    need(provenance["input_before"] == provenance["input_after"]
         and set(provenance["input_before"]) == INPUT_NAMES
         and provenance["source_before"] == provenance["source_after"]
         and set(provenance["source_before"]) == set(source_names), "pre/post input/source closure differs")
    for name, expected in provenance["input_before"].items():
        need(sha(capture(name)) == expected, f"executed input differs: {prefix}/{name}")
    for name, expected in provenance["source_before"].items():
        need(sha(capture("sources/" + name)) == expected, f"pinned source bytes differ: {prefix}/{name}")
    need(sha(capture("sources/oracle.lock.json")) == provenance["oracle_lock_sha256"]
         and sha(capture("sources/references.lock")) == provenance["references_lock_sha256"],
         "lock hash differs")
    lock = load_json(capture("sources/oracle.lock.json"))
    need(lock["version"] == "0.32.0" and lock["source"]["revision"] == REVISION
         and lock["artifact"]["sha256"] == ARTIFACT_SHA256
         and ("pkl.revision=" + REVISION).encode() in capture("sources/references.lock"),
         "pinned locks differ")
    jdk = provenance["jdk"]
    need((jdk["executable_sha256"], jdk["modules_sha256"], jdk["release_sha256"]) == JDK_SHA256
         and all(jdk["properties"].get(k) == v for k, v in lock["jdk"]["properties"].items()),
         "JDK identity differs")
    sessions = provenance["sessions"]
    need(len(sessions) == 2 and [session["repeat"] for session in sessions] == [0, 1]
         and len({session["pid"] for session in sessions}) == 2
         and all(session["pid"] > 0 and session["started_epoch"] < session["finished_epoch"]
                 for session in sessions)
         and sessions[0]["finished_epoch"] <= sessions[1]["started_epoch"],
         "fresh ordered JVM coverage differs")
    observed = []
    raw_sets = []
    for session in sessions:
        repeat = session["repeat"]
        base = prefix + f"/repeat-{repeat}/"
        cwd = Path(session["cwd"])
        expected_command = [jdk["executable"], *lock["jvm_options"],
                            f"-Duser.home={provenance['environment']['HOME']}",
                            f"-Djava.io.tmpdir={provenance['environment']['TMPDIR']}", "-cp",
                            provenance["artifact"], str(cwd / "IntSeqValuesOracle.java"),
                            str(cwd / f"repeat-{repeat}"), str(cwd), module_uri]
        need(cwd.is_absolute() and session["command"] == expected_command
             and session["exit_code"] == 0 and session["query_directories"] == rows_count,
             "session command/result differs")
        for channel in ("stdout", "stderr"):
            raw = files[base + "process." + channel + ".bin"]
            need(sha(raw) == session[channel + "_sha256"] and
                 len(raw) == session[channel + "_bytes"], "process channel hash/size differs")
        need(files[base + "process.stderr.bin"] == b"", "process stderr differs")
        protocol = [f"session-open mode={mode} rows={rows_count}"]
        counts = Counter()
        for index, row in enumerate(rows):
            folder = base + f"{index:03d}-{row['id']}/"
            validate_row_channels(files, folder)
            need(checked(files, folder, "mode") == mode and
                 checked(files, folder, "expression") == row["expression"],
                 f"mode/expression differs: {row['id']}")
            stdout = checked(files, folder, "result.stdout")
            stderr = checked(files, folder, "result.stderr")
            error_class = checked(files, folder, "result.error-class")
            exception_to_string = checked(files, folder, "result.exception-to-string")
            cause_class = checked(files, folder, "result.cause-class")
            cause_message = checked(files, folder, "result.cause-message")
            cause_to_string = checked(files, folder, "result.cause-to-string")
            java_type = checked(files, folder, "result.java-type")
            status = files[folder + "result.status.bin"].decode()
            need(status in ("success\n", "error\n", "oracle-error\n", "api-exception\n"),
                 "row status differs")
            if status == "success\n":
                need(stderr == java_type == error_class == exception_to_string == cause_class ==
                     cause_message == cause_to_string == "", "success error channel differs")
            else:
                need(stdout == java_type == "" and stderr and error_class and
                     exception_to_string.startswith(error_class + ": ") and
                     bool(cause_class) == bool(cause_message) == bool(cause_to_string) and
                     (not cause_class or cause_to_string.startswith(cause_class + ": ")),
                     "failure channel differs")
                if status == "error\n":
                    need(error_class == "org.pkl.core.PklException", "ordinary Pkl error class differs")
            counts[status.strip()] += 1
            call = f"{mode}-{index}-{row['id']}"
            protocol.extend((f"call-enter={call}", f"call-exit={call} status={status.strip()}"))
            if repeat == 0:
                lines = stderr.splitlines()
                observed.append({"id": row["id"], "group": row["group"], "mode": mode,
                                 "expression": row["expression"], "status": status.strip(),
                                 "stdout_sha256": sha(files[folder + "result.stdout.bin"]),
                                 "stderr_sha256": sha(files[folder + "result.stderr.bin"]),
                                 "stdout": stdout,
                                 "first_error_line": lines[1] if len(lines) > 1 else "",
                                 "error_class": error_class})
        protocol.append(f"session-closed mode={mode} rows={rows_count} public-calls={rows_count} "
                        f"success={counts['success']} errors={rows_count - counts['success']}")
        validate_process_protocol(files[base + "process.stdout.bin"], protocol)
        raw_set = {name[len(base):]: sha(raw) for name, raw in files.items() if name.startswith(base)}
        need(len(raw_set) == expected_raw_files, "session raw file count differs")
        raw_sets.append(raw_set)
    need(raw_sets[0] == raw_sets[1], "ordered raw repetition differs")
    need(audit == {"schema": 1, "scope": provenance["scope"], "rows": rows_count,
                   "statuses": dict(Counter(row["status"] for row in observed)),
                   "observations": observed, "candidate_compatibility_passes_claimed": 0,
                   "limits": ["Authored public calls only; original upstream snippets are source evidence, not replayed modules.",
                              "Rust native, browser and Node raw comparison, typed candidate export, full M2 and performance gates remain unverified."]},
         "derived audit differs")
    need(audit["statuses"] == expected_statuses, "independent status vector differs")
    return audit["statuses"]


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    root = Path(root)
    need(root.is_dir() and not root.is_symlink(), "fixture directory missing or symlinked")
    entries = list(root.iterdir())
    need(all(path.is_file() and not path.is_symlink() for path in entries),
         "undeclared or nonregular fixture path")
    names = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
             "module.pkl", "queries.tsv", "cases.json", "export-queries.tsv", "export-cases.json"}
    root_files = {path.name: sha(path.read_bytes()) for path in entries if path.name != "manifest.json"}
    need(set(root_files) == names and len(entries) == len(names) + 1, "fixture root file set differs")
    manifest = load_json((root / "manifest.json").read_bytes())
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_files},
         "fixture manifest differs")
    need(root_files["oracle-evidence.tar.gz"] == expected_archive, "archive SHA differs")
    identity = load_json((root / "archive-identity.json").read_bytes())
    need(identity["schema"] == 1 and identity["archive_sha256"] == expected_archive
         and identity["fileset_sha256"] == expected_fileset
         and identity["captures"] == {ARCHIVE_PREFIX: "stage61-intseq-oracle-string-01",
                                      EXPORT_PREFIX: "stage61-intseq-oracle-export-01"},
         "archive identity differs")
    files = {}
    file_set = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as stream:
        for member in stream:
            name = member.name
            need(member.isfile() and not Path(name).is_absolute() and ".." not in Path(name).parts
                 and name.startswith((ARCHIVE_PREFIX + "/", EXPORT_PREFIX + "/"))
                 and name not in files, "unsafe or duplicate archive member")
            raw = stream.extractfile(member).read()
            need(len(raw) == member.size, "archive member size differs")
            files[name] = raw
            file_set.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(files) == identity["members"] and file_set.hexdigest() == expected_fileset,
         "archive file set differs")
    for prefix, authored in ((ARCHIVE_PREFIX, {"cases.json": "cases.json", "queries.tsv": "queries.tsv"}),
                             (EXPORT_PREFIX, {"cases.json": "export-cases.json", "queries.tsv": "export-queries.tsv"})):
        for archive_name, root_name in {**authored, "module.pkl": "module.pkl"}.items():
            need(files[prefix + "/" + archive_name] == (root / root_name).read_bytes(),
                 "authored input differs")
        for name, expected in identity["oracle_input_sha256"][prefix].items():
            need(sha(files[prefix + "/" + name]) == expected, "archive input identity differs")
    string_status = validate_capture(files, ARCHIVE_PREFIX, mode="string", rows_count=237,
                                     expected_statuses={"success": 198, "error": 39}, expected_raw_files=4979)
    export_status = validate_capture(files, EXPORT_PREFIX, mode="export", rows_count=3,
                                     expected_statuses={"error": 3}, expected_raw_files=65)
    return {"baseline": "Pkl 0.32.0", "rows": 237, "statuses": string_status,
            "export_supplement_rows": 3, "export_supplement_statuses": export_status,
            "archive_sha256": expected_archive, "fileset_sha256": expected_fileset,
            "raw_repetitions_equal": True, "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
