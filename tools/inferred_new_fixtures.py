"""Offline full-protocol verifier for Pkl 0.32.0 inferred-new Java fixtures."""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import tarfile

from inferred_new_oracle import INPUT_HASHES, ROWS, SOURCES, REVISION

ROOT = Path(__file__).resolve().parents[1]
FIXTURES = {
    "values": ROOT / "tests/fixtures/m2-inferred-new-values",
    "amends": ROOT / "tests/fixtures/m2-inferred-new-amends",
}
ARCHIVE_SHA256 = {
    "values": "231d9c1d922cae9c1b2e0c1bd5fb15a0465681043414425f36bf89480b08763e",
    "amends": "ea2c1cfe786301e1055dfd2275ecf6977d07c82f84ab5f86294322d80e2037af",
}
FILESET_SHA256 = {
    "values": "72c39593d98beb7d67506c97f56b1d2ddd31adefa902d4bac77cfdbf02c26ff7",
    "amends": "b88f7617a3cf0a61fa5e8d6d09573ae694802867e4d2d80a78c4b405aba18167",
}
STATUSES = {"values": {"success": 49, "error": 23},
            "amends": {"success": 7}}
JDK_SHA256 = (
    "51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
    "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
    "ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1",
)
ROW_CHANNELS = {"result.status.bin"} | {
    name + suffix for name in (
        "mode", "expression", "result.stdout", "result.stderr", "result.error-class",
        "result.exception-to-string", "result.cause-class", "result.cause-message",
        "result.cause-to-string", "result.java-type")
    for suffix in (".bin", ".code-units.bin")
}


def need(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def checked(files: dict[str, bytes], folder: str, name: str) -> str:
    raw = files[folder + name + ".bin"]
    units = files[folder + name + ".code-units.bin"]
    need(len(units) % 2 == 0 and
         units.decode("utf-16-be", "surrogatepass").encode("utf-8", "replace") == raw,
         f"UTF-8/UTF-16 channels differ: {folder}{name}")
    return raw.decode("utf-8")


def validate_capture(files: dict[str, bytes], profile: str) -> dict:
    prefix = "capture-01/"
    def capture(name: str) -> bytes:
        return files[prefix + name]
    cases = json.loads(capture("cases.json"))
    provenance = json.loads(capture("provenance.json"))
    audit = json.loads(capture("audit.json"))
    rows = cases["rows"]
    need(cases["schema"] == 1 and cases["baseline"] == "Pkl 0.32.0"
         and cases["boundary"] == "string" and cases["repetitions"] == 2
         and len(rows) == ROWS[profile] and len({row["id"] for row in rows}) == len(rows)
         and all(set(row) == {"id", "group", "expression", "mode"}
                 and row["mode"] == "string" and re.fullmatch(r"[a-z0-9-]+", row["id"])
                 and "\t" not in row["expression"] and "\n" not in row["expression"]
                 for row in rows), "authored rows differ")
    need(capture("queries.tsv") == "".join(
        f"string\t{row['id']}\t{row['expression']}\n" for row in rows).encode(),
        "ordered queries differ")
    need(provenance["schema"] == 1 and provenance["profile"] == profile
         and provenance["complete"] is True and provenance["stable"] is True
         and provenance["raw_equal"] is True and provenance["raw_differences"] == []
         and provenance["candidate_compatibility_passes_claimed"] == 0,
         "acquisition completion/claim differs")
    need(provenance["reference_revision"] == REVISION
         and provenance["artifact_sha256"] == provenance["artifact_after_sha256"]
         == "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
         "pinned source/artifact differs")
    input_names = {"CollectionConversionsOracle.java", "run.py", *INPUT_HASHES[profile]}
    need(set(provenance["input_before"]) == input_names
         and provenance["input_before"] == provenance["input_after"]
         and set(provenance["source_before"]) == set(SOURCES)
         and provenance["source_before"] == provenance["source_after"]
         == provenance["source_copies_after"], "input/source closure differs")
    for name, expected in provenance["input_before"].items():
        need(sha(capture(name)) == expected, f"captured input differs: {name}")
    for name, expected in provenance["source_before"].items():
        need(sha(capture("sources/" + name)) == expected, f"source copy differs: {name}")
    need(capture("run.py") == (ROOT / "tools/inferred_new_oracle.py").read_bytes()
         and capture("CollectionConversionsOracle.java") ==
         (ROOT / "tools/java/CollectionConversionsOracle.java").read_bytes(),
         "tracked producer/helper closure differs")
    for name, expected in INPUT_HASHES[profile].items():
        need(sha(capture(name)) == expected, f"fixed authored input differs: {name}")
    need(sha(capture("sources/oracle.lock.json")) == provenance["oracle_lock_sha256"]
         and sha(capture("sources/references.lock")) == provenance["references_lock_sha256"],
         "pinned lock copies differ")
    lock = json.loads(capture("sources/oracle.lock.json"))
    need(lock["version"] == "0.32.0" and lock["source"]["revision"] == REVISION
         and lock["artifact"]["sha256"] == provenance["artifact_sha256"]
         and ("pkl.revision=" + REVISION).encode() in capture("sources/references.lock"),
         "pinned lock contents differ")
    identity_stdout = capture("java-identity.stdout.bin")
    identity_stderr = capture("java-identity.stderr.bin")
    properties = dict(re.findall(r"^\s+([\w.]+) = (.*)$", identity_stderr.decode(), re.M))
    jdk = provenance["jdk"]
    need(identity_stdout == b"" and properties == jdk["properties"]
         and b'openjdk version "21.0.12.1"' in identity_stderr
         and (jdk["executable_sha256"], jdk["modules_sha256"],
              jdk["release_sha256"]) == JDK_SHA256
         and all(properties.get(name) == value for name, value in lock["jdk"]["properties"].items()),
         "raw JDK identity differs")
    sessions = provenance["sessions"]
    need(len(sessions) == 2 and [row["repeat"] for row in sessions] == [0, 1]
         and len({row["pid"] for row in sessions}) == 2
         and all(row["pid"] > 0 and row["started_epoch"] < row["finished_epoch"]
                 for row in sessions)
         and sessions[0]["finished_epoch"] <= sessions[1]["started_epoch"],
         "two fresh ordered JVMs absent")
    if profile == "values":
        uri = "file:///stage65/inferred-new-values.pkl"
        need(cases["module_uri"] == uri, "stable normal URI differs")
    else:
        need(cases["module_uri_policy"] == "captured-absolute-file-uri-with-sibling-base",
             "amends URI policy differs")
        uri = Path(sessions[0]["cwd"], "module.pkl").as_uri()
        need(capture("base.pkl") == (ROOT / "tests/fixtures/m2-inferred-new-amends/base.pkl").read_bytes()
             and b'amends "base.pkl"' in capture("module.pkl"),
             "real sibling amends source differs")
    need(provenance["module_uri"] == uri, "recorded module URI differs")
    observed = []
    repetitions = []
    for session in sessions:
        repeat = session["repeat"]
        base = prefix + f"repeat-{repeat}/"
        cwd = Path(session["cwd"])
        command = [jdk["executable"], *lock["jvm_options"],
                   f"-Duser.home={provenance['environment']['HOME']}",
                   f"-Djava.io.tmpdir={provenance['environment']['TMPDIR']}", "-cp",
                   provenance["artifact"], str(cwd / "CollectionConversionsOracle.java"),
                   str(cwd / f"repeat-{repeat}"), str(cwd), uri]
        need(cwd.is_absolute() and session["cwd"] == str(cwd)
             and session["command"] == command and session["exit_code"] == 0
             and session["timed_out"] is False
             and session["query_directories"] == ROWS[profile],
             "session argv/exit/row count differs")
        for channel in ("stdout", "stderr"):
            raw = files[base + "process." + channel + ".bin"]
            need(sha(raw) == session[channel + "_sha256"]
                 and len(raw) == session[channel + "_bytes"],
                 "raw process stream differs")
        need(files[base + "process.stderr.bin"] == b"", "unexpected Java process stderr")
        protocol = [f"session-open mode=string rows={ROWS[profile]}"]
        statuses = Counter()
        for index, row in enumerate(rows):
            folder = base + f"{index:03d}-{row['id']}/"
            actual = {name[len(folder):] for name in files if name.startswith(folder)}
            need(actual == ROW_CHANNELS, f"21-channel row set differs: {folder}")
            need(checked(files, folder, "mode") == "string"
                 and checked(files, folder, "expression") == row["expression"],
                 "raw row input echo differs")
            values = {name: checked(files, folder, "result." + name) for name in
                      ("stdout", "stderr", "error-class", "exception-to-string", "cause-class",
                       "cause-message", "cause-to-string", "java-type")}
            status = files[folder + "result.status.bin"].decode()
            need(status in ("success\n", "error\n"), "unexpected API/oracle status")
            if status == "success\n":
                need(all(not values[name] for name in values if name != "stdout"),
                     "success metadata differs")
            else:
                need(values["stdout"] == values["java-type"] == ""
                     and values["stderr"] and values["error-class"] == "org.pkl.core.PklException"
                     and values["exception-to-string"].startswith(
                         "org.pkl.core.PklException: ")
                     and bool(values["cause-class"]) == bool(values["cause-message"])
                     == bool(values["cause-to-string"]), "ordinary error metadata differs")
            statuses[status.strip()] += 1
            call = f"string-{index}-{row['id']}"
            protocol.extend((f"call-enter={call}", f"call-exit={call} status={status.strip()}"))
            if repeat == 0:
                lines = values["stderr"].splitlines()
                observed.append({"id": row["id"], "group": row["group"], "mode": "string",
                                 "expression": row["expression"], "status": status.strip(),
                                 "stdout_sha256": sha(files[folder + "result.stdout.bin"]),
                                 "stderr_sha256": sha(files[folder + "result.stderr.bin"]),
                                 "stdout": values["stdout"],
                                 "first_error_line": lines[1] if len(lines) > 1 else "",
                                 "error_class": values["error-class"]})
        protocol.append(f"session-closed mode=string rows={ROWS[profile]} "
                        f"public-calls={ROWS[profile]} success={statuses['success']} "
                        f"errors={statuses['error']}")
        need(files[base + "process.stdout.bin"] == ("\n".join(protocol) + "\n").encode(),
             "complete Java process protocol differs")
        raw_set = {name[len(base):]: sha(raw) for name, raw in files.items()
                   if name.startswith(base)}
        need(len(raw_set) == 2 + 21 * ROWS[profile], "raw file count differs")
        repetitions.append(raw_set)
    need(repetitions[0] == repetitions[1]
         and provenance["raw_files"] == len(repetitions[0]),
         "two raw JVM repetitions differ")
    need(audit == {"schema": 1, "scope": provenance["scope"], "profile": profile,
                   "rows": ROWS[profile], "statuses": dict(Counter(
                       row["status"] for row in observed)), "observations": observed,
                   "candidate_compatibility_passes_claimed": 0,
                   "limits": ["Authored public string observations only; no Rust candidate compared.",
                              "Amends uses a real sibling file and captured absolute file URI; portable Host behavior is unverified."]},
         "derived Java audit differs")
    need(audit["statuses"] == STATUSES[profile], "Java status vector differs")
    expected_files = {prefix + name for name in (
        {"CollectionConversionsOracle.java", "run.py", *INPUT_HASHES[profile],
         "provenance.json", "audit.json", "java-identity.stdout.bin",
         "java-identity.stderr.bin"})}
    expected_files |= {prefix + "sources/" + name for name in SOURCES}
    expected_files |= {prefix + f"repeat-{repeat}/" + name for repeat, raw in enumerate(repetitions)
                       for name in raw}
    need(set(files) == expected_files, "complete capture file set differs")
    return {"rows": ROWS[profile], "statuses": audit["statuses"],
            "raw_files_per_session": provenance["raw_files"],
            "source_copies": len(SOURCES), "module_uri": uri}


def verify(profile: str, fixture: Path | None = None) -> dict:
    root = FIXTURES[profile] if fixture is None else Path(fixture)
    need(root.is_dir() and not root.is_symlink(), "fixture root missing or symlinked")
    entries = list(root.iterdir())
    need(all(path.is_file() and not path.is_symlink() for path in entries),
         "undeclared or nonregular fixture root path")
    names = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
             "module.pkl", "queries.tsv", "cases.json"}
    if profile == "amends":
        names.add("base.pkl")
    need(set(path.name for path in entries) == names | {"manifest.json"},
         "fixture root file set differs")
    root_files = {path.name: sha(path.read_bytes()) for path in entries
                  if path.name != "manifest.json"}
    manifest = json.loads((root / "manifest.json").read_bytes())
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_files},
         "fixture manifest differs")
    identity = json.loads((root / "archive-identity.json").read_bytes())
    need(identity["schema"] == 1 and identity["profile"] == profile
         and identity["archive_sha256"] == ARCHIVE_SHA256[profile]
         and identity["fileset_sha256"] == FILESET_SHA256[profile]
         and root_files["oracle-evidence.tar.gz"] == ARCHIVE_SHA256[profile]
         and identity["captures"] == {"capture-01": f"stage65-inferred-new-{profile}-oracle-01"},
         "fixed archive/fixture identity differs")
    files = {}
    file_set = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as stream:
        for member in stream:
            path = PurePosixPath(member.name)
            need(member.isfile() and not member.issym() and not member.islnk()
                 and not path.is_absolute() and ".." not in path.parts
                 and member.name.startswith("capture-01/") and member.name not in files,
                 "unsafe or duplicate archive member")
            handle = stream.extractfile(member)
            need(handle is not None, "unreadable archive member")
            raw = handle.read()
            need(len(raw) == member.size, "archive member size differs")
            files[member.name] = raw
            file_set.update(member.name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(files) == identity["members"] and file_set.hexdigest() == FILESET_SHA256[profile],
         "complete archive file set differs")
    for name in INPUT_HASHES[profile]:
        need(files["capture-01/" + name] == (root / name).read_bytes(),
             f"fixture/authored input differs: {name}")
    result = validate_capture(files, profile)
    return {"profile": profile, "baseline": "Pkl 0.32.0", **result,
            "archive_sha256": ARCHIVE_SHA256[profile],
            "fileset_sha256": FILESET_SHA256[profile],
            "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--profile", choices=sorted(FIXTURES), required=True)
    parser.add_argument("--fixture", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.profile, args.fixture), indent=2, sort_keys=True))
