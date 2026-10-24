"""Offline integrity and complete raw protocol verifier for Duration Java authority."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import tarfile

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-duration-values"
ARCHIVE_SHA256 = "7250661447adfeabdc5134ec883b786c90c8698e1260b8150b5492940cff59f7"
FILESET_SHA256 = "90bcb267170872f4bc53e8c9d9263c8f910557228ac12c3f027c9653f41dbd7e"
ARTIFACT_SHA256 = "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059"
JDK_SHA256 = (
    "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1",
)
REVISION = "e51fb2f91774265666f94980ec630db794a4db2d"
ROWS = 274
from duration_values_oracle import SOURCE_NAMES
ARCHIVE_PREFIX = "capture-02"
INPUT_NAMES = ["DurationValuesOracle.java", "run.py", "module.pkl", "queries.tsv", "cases.json"]
ROW_CHANNELS = {"result.status.bin"} | {
    name + suffix for name in ("expression", "result.stdout", "result.stderr", "result.error-class")
    for suffix in (".bin", ".code-units.bin")
}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def load_json(raw):
    return json.loads(raw)


def validate_rows(rows, queries):
    need(len(rows) == ROWS and len({row["id"] for row in rows}) == ROWS
         and all(set(row) == {"id", "group", "expression"}
                 and re.fullmatch(r"[a-z0-9-]+", row["id"])
                 and "\t" not in row["expression"] and "\n" not in row["expression"]
                 for row in rows), "case rows differ")
    need(queries == "".join(row["id"] + "\t" + row["expression"] + "\n"
                            for row in rows).encode(), "query order differs")


def validate_row_channels(files, folder):
    actual = {name[len(folder):] for name in files if name.startswith(folder)}
    need(actual == ROW_CHANNELS, f"row channel set differs: {folder}")


def validate_process_protocol(actual, protocol):
    need(actual == ("\n".join(protocol) + "\n").encode(), "complete process protocol differs")


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256,
           expected_capture="stage59-duration-oracle-02", archive_prefix=ARCHIVE_PREFIX,
           expected_rows=ROWS, expected_members=4961, expected_raw_files=2468,
           expected_module_uri="file:///stage59/duration-values.pkl",
           expected_statuses=None):
    root = Path(root)
    expected_statuses = {"success": 240, "error": 34} if expected_statuses is None else expected_statuses
    need(root.is_dir(), "fixture directory missing")
    need(not any(path.is_symlink() for path in root.rglob("*")), "fixture symlink detected")
    root_files = {path.relative_to(root).as_posix(): sha(path.read_bytes())
                  for path in root.rglob("*") if path.is_file() and path.name != "manifest.json"}
    need(set(root_files) == {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
                             "module.pkl", "queries.tsv", "cases.json"}, "fixture root file set differs")
    manifest = load_json((root / "manifest.json").read_bytes())
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_files},
         "fixture manifest differs")
    archive_path = root / "oracle-evidence.tar.gz"
    need(root_files["oracle-evidence.tar.gz"] == expected_archive, "archive SHA differs")
    identity = load_json((root / "archive-identity.json").read_bytes())
    need(identity["schema"] == 1 and identity["archive_sha256"] == expected_archive
         and identity["fileset_sha256"] == expected_fileset and identity["members"] == expected_members
         and identity["capture"] == expected_capture, "archive identity differs")

    files = {}
    file_set = hashlib.sha256()
    with tarfile.open(archive_path, "r:gz") as stream:
        for member in stream:
            name = member.name
            relative = name[len(archive_prefix) + 1:]
            need(member.isfile() and name.startswith(archive_prefix + "/") and not Path(name).is_absolute()
                 and ".." not in Path(name).parts and relative not in files,
                 "unsafe or duplicate archive member")
            raw = stream.extractfile(member).read()
            need(len(raw) == member.size, "archive member size differs")
            files[relative] = raw
            file_set.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(files) == identity["members"] and file_set.hexdigest() == expected_fileset,
         "archive file set differs")
    need(set(identity["oracle_input_sha256"]) == {"cases.json", "module.pkl", "queries.tsv",
          "DurationValuesOracle.java", "run.py", "provenance.json", "audit.json"}, "identity input list differs")
    for name, expected in identity["oracle_input_sha256"].items():
        need(sha(files[name]) == expected, f"pinned capture input differs: {name}")
    for name in ("cases.json", "module.pkl", "queries.tsv"):
        need((root / name).read_bytes() == files[name], f"authored input differs: {name}")

    p = load_json(files["provenance.json"])
    cases = load_json(files["cases.json"])
    audit = load_json(files["audit.json"])
    need(cases["schema"] == 1 and cases["baseline"] == "Pkl 0.32.0"
         and cases["module_uri"] == expected_module_uri
         and cases["boundary"] == "string" and cases["repetitions"] == 2,
         "case boundary differs")
    rows = cases["rows"]
    need(expected_rows == ROWS, "pinned row count differs")
    validate_rows(rows, files["queries.tsv"])
    need(p["schema"] == 1 and p["complete"] is True and p["stable"] is True
         and p["raw_equal"] is True and p["raw_differences"] == [] and p["raw_files"] == expected_raw_files,
         "acquisition completion/repetition differs")
    need(p["reference_revision"] == REVISION and p["artifact_sha256"] == p["artifact_after_sha256"] == ARTIFACT_SHA256,
         "oracle source/artifact identity differs")
    need(p["input_before"] == p["input_after"] and set(p["input_before"]) == set(INPUT_NAMES)
         and p["source_before"] == p["source_after"] and set(p["source_before"]) == set(SOURCE_NAMES),
         "pre/post input/source closure differs")
    for name, expected in p["input_before"].items():
        need(sha(files[name]) == expected, f"executed input differs: {name}")
    for name, expected in p["source_before"].items():
        need(sha(files["sources/" + name]) == expected, f"pinned source bytes differ: {name}")
    need(sha(files["sources/oracle.lock.json"]) == p["oracle_lock_sha256"]
         and sha(files["sources/references.lock"]) == p["references_lock_sha256"], "lock hash differs")
    lock = load_json(files["sources/oracle.lock.json"])
    need(lock["version"] == "0.32.0" and lock["source"]["revision"] == REVISION
         and lock["artifact"]["sha256"] == ARTIFACT_SHA256
         and ("pkl.revision=" + REVISION).encode() in files["sources/references.lock"], "pinned locks differ")
    jdk = p["jdk"]
    need((jdk["executable_sha256"], jdk["modules_sha256"], jdk["release_sha256"]) == JDK_SHA256
         and all(jdk["properties"].get(k) == v for k, v in lock["jdk"]["properties"].items()),
         "JDK identity differs")
    need(len(p["sessions"]) == 2 and [session["repeat"] for session in p["sessions"]] == [0, 1]
         and len({session["pid"] for session in p["sessions"]}) == 2
         and all(session["pid"] > 0 and session["started_epoch"] < session["finished_epoch"]
                 for session in p["sessions"])
         and p["sessions"][0]["finished_epoch"] <= p["sessions"][1]["started_epoch"],
         "fresh ordered JVM coverage differs")

    def checked(prefix, name):
        raw = files[prefix + name + ".bin"]
        units = files[prefix + name + ".code-units.bin"]
        need(len(units) % 2 == 0 and units.decode("utf-16-be", "surrogatepass").encode("utf-8", "replace") == raw,
             f"UTF-8/UTF-16 producer channels differ: {prefix}{name}")
        return raw.decode("utf-8")

    session_files = []
    observed = []
    for session in p["sessions"]:
        repeat = session["repeat"]
        prefix = f"repeat-{repeat}/"
        cwd = Path(session["cwd"])
        expected_command = [jdk["executable"], *lock["jvm_options"],
                            f"-Duser.home={p['environment']['HOME']}",
                            f"-Djava.io.tmpdir={p['environment']['TMPDIR']}", "-cp", p["artifact"],
                            str(cwd / "DurationValuesOracle.java"), str(cwd / f"repeat-{repeat}"),
                            str(cwd), cases["module_uri"]]
        need(cwd.is_absolute() and session["command"] == expected_command and session["exit_code"] == 0
             and session["query_directories"] == expected_rows, "session command/result differs")
        for channel in ("stdout", "stderr"):
            raw = files[prefix + "process." + channel + ".bin"]
            need(sha(raw) == session[channel + "_sha256"] and len(raw) == session[channel + "_bytes"],
                 "process channel hash/size differs")
        need(files[prefix + "process.stderr.bin"] == b"", "process stderr differs")
        protocol = [f"session-open mode=string rows={expected_rows}"]
        counts = Counter()
        for i, row in enumerate(rows):
            folder = prefix + f"{i:03d}-{row['id']}/"
            validate_row_channels(files, folder)
            need(checked(folder, "expression") == row["expression"], f"expression differs: {row['id']}")
            stdout = checked(folder, "result.stdout")
            stderr = checked(folder, "result.stderr")
            error_class = checked(folder, "result.error-class")
            status = files[folder + "result.status.bin"].decode()
            need(status in ("success\n", "error\n", "oracle-error\n", "api-exception\n"), "row status differs")
            if status == "success\n":
                need(stderr == error_class == "", "success error channel differs")
            else:
                need(stdout == "" and stderr and error_class, "failure channel differs")
            counts[status.strip()] += 1
            call = f"string-{i}-{row['id']}"
            protocol.extend((f"call-enter={call}", f"call-exit={call} status={status.strip()}"))
            if repeat == 0:
                observed.append({"id": row["id"], "group": row["group"], "expression": row["expression"],
                                 "status": status.strip(), "stdout_sha256": sha(files[folder + "result.stdout.bin"]),
                                 "stderr_sha256": sha(files[folder + "result.stderr.bin"]), "error_class": error_class})
        protocol.append(f"session-closed mode=string rows={expected_rows} public-calls={expected_rows} "
                        f"success={counts['success']} errors={expected_rows - counts['success']}")
        validate_process_protocol(files[prefix + "process.stdout.bin"], protocol)
        subset = {name[len(prefix):]: sha(raw) for name, raw in files.items() if name.startswith(prefix)}
        need(len(subset) == expected_raw_files == p["raw_files"], "session raw file count differs")
        session_files.append(subset)
    need(session_files[0] == session_files[1], "ordered raw repetition differs")
    need(audit == {"schema": 1, "scope": p["scope"], "rows": expected_rows,
                   "statuses": dict(Counter(row["status"] for row in observed)),
                   "observations": observed, "compatibility_passes_claimed": 0,
                   "remaining_required": ["Rust native, browser and Node raw comparisons", "typed export API",
                                          "full M2 language and performance gates"]}, "derived audit differs")
    need(audit["statuses"] == expected_statuses, "independent status vector differs")
    return {"baseline": "Pkl 0.32.0", "rows": expected_rows, "statuses": audit["statuses"],
            "archive_sha256": expected_archive, "fileset_sha256": expected_fileset,
            "raw_repetitions_equal": True, "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
