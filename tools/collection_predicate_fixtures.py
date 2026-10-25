"""Offline byte verifiers for two Pkl 0.32.0 collection predicates."""
from __future__ import annotations

from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import stat
import sys
import tarfile


def project_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracle.lock.json").is_file():
            return parent
    raise ValueError("project root missing")


ROOT = project_root()
sys.path.insert(0, str(ROOT / "tools"))
from collection_conversion_controls_fixtures import ROW_CHANNELS, validate_capture  # noqa: E402

DRAFT = ROOT / "target/m2/stage68-collection-predicate-fixture-draft-01"
INSTALLED_IN_TOOLS = Path(__file__).resolve().parent == ROOT / "tools"


def default_fixture(kind: str) -> Path:
    need(kind in EXPECTED, "unknown predicate profile")
    return (ROOT / "tests/fixtures" / f"m2-{kind}-oracle") if INSTALLED_IN_TOOLS else DRAFT / kind
EXPECTED = {
    "collection-emptiness": {
        "archive": "431e41e78ff9f2ad799549becb30463cd67c7d4883a147add78daaf96fecbd62",
        "fileset": "ed94162e8ff012786ed6281e7bb136a7168a4ffb8f74c9027d89e17d2a116b5c",
        "members": 2057, "rows": 47, "statuses": {"success": 34, "error": 13},
        "raw_files": 989, "uri": "file:///stage68/collection-emptiness.pkl",
        "capture_prefix": "capture-02",
        "prefixes": {"capture-02": "stage68-collection-emptiness-oracle-02",
                     "preparation-02": "stage68-collection-emptiness-prep-02",
                     "launch-02": "stage68-collection-emptiness-launch-02",
                     "review-01": "stage68-collection-emptiness-review-01"},
    },
    "collection-every": {
        "archive": "42d5d89cd10c35de457f273da34b22d02fd48791210de7a3e15816b0955a979e",
        "fileset": "6872aa67dc17118cd6df73d15a971f5aa0f97f79b49c1c073c6765430724089d",
        "members": 3701, "rows": 64, "statuses": {"success": 36, "error": 28},
        "raw_files": 1798, "uri": "file:///stage68/collection-every.pkl",
        "capture_prefix": "capture-03",
        "prefixes": {"capture-03": "stage68-collection-every-oracle-03",
                     "preparation-03": "stage68-collection-every-prep-03",
                     "launch-03": "stage68-collection-every-launch-03",
                     "audit-02": "stage68-collection-every-audit-02"},
    },
}
LOG_FIELDS = ("level", "message", "module-uri", "member", "position")
ROOT_NAMES = {"README.md", "archive-identity.json", "member-manifest.json",
              "oracle-evidence.tar.gz", "cases.json", "module.pkl", "queries.tsv", "manifest.json"}
JDK_SHA = ("51ec0764f7225b40d5b286b01401aa6b3990d0d171b446d7dc81ca7542b45552",
           "40fcf7a32bada7e299e3513939a4472d27f8f61eb81cdd1675ea00648de7e7cf",
           "ab66b8afeea98b25f1869fbe7c0dde368d448e7cc0f7bab0359b322ade134fe1")
JAR_SHA = "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059"


def need(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def ident(raw: bytes, mode: int) -> dict[str, object]:
    return {"bytes": len(raw), "sha256": sha(raw), "mode": mode}


def utf16_equivalent(raw: bytes, units: bytes) -> bool:
    return len(units) % 2 == 0 and units.decode("utf-16-be", "surrogatepass").encode("utf-8", "replace") == raw


def checked(files: dict[str, bytes], folder: str, stem: str) -> str:
    raw = files[folder + stem + ".bin"]
    units = files[folder + stem + ".code-units.bin"]
    need(utf16_equivalent(raw, units), f"UTF-8/UTF-16 channel differs: {folder}{stem}")
    return raw.decode("utf-8")


def read_fixture(kind: str, fixture: Path) -> tuple[dict[str, bytes], dict]:
    expected = EXPECTED[kind]
    need(fixture.is_dir() and not fixture.is_symlink(), "fixture root missing or linked")
    children = list(fixture.iterdir())
    need({p.name for p in children} == ROOT_NAMES
         and all(p.is_file() and not p.is_symlink() for p in children),
         "fixture root file set differs")
    root_hashes = {p.name: sha(p.read_bytes()) for p in children if p.name != "manifest.json"}
    need(json.loads((fixture / "manifest.json").read_bytes()) ==
         {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_hashes},
         "fixture root manifest differs")
    need(root_hashes["oracle-evidence.tar.gz"] == expected["archive"], "pinned archive hash differs")
    archive_id = json.loads((fixture / "archive-identity.json").read_bytes())
    need(archive_id == {
        "schema": 1, "baseline": "Pkl 0.32.0", "kind": kind,
        "uri": expected["uri"], "rows": expected["rows"], "statuses": expected["statuses"],
        "archive_sha256": expected["archive"], "fileset_sha256": expected["fileset"],
        "members": expected["members"], "prefixes": expected["prefixes"],
        "root_inputs": {name: root_hashes[name] for name in ("cases.json", "module.pkl", "queries.tsv")},
        "candidate_compatibility_passes_claimed": 0,
    }, "archive identity differs")
    member_manifest = json.loads((fixture / "member-manifest.json").read_bytes())
    need(member_manifest["schema"] == 1 and len(member_manifest["members"]) == expected["members"],
         "member manifest shape differs")
    files: dict[str, bytes] = {}
    actual = {}
    fileset = hashlib.sha256()
    with tarfile.open(fixture / "oracle-evidence.tar.gz", "r:gz") as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and not Path(name).is_absolute()
                 and ".." not in Path(name).parts and name not in files
                 and any(name.startswith(prefix + "/") for prefix in expected["prefixes"]),
                 "unsafe, duplicate or undeclared archive member")
            stream = archive.extractfile(member)
            need(stream is not None, "unreadable archive member")
            raw = stream.read()
            files[name] = raw
            actual[name] = ident(raw, member.mode)
            fileset.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(actual == member_manifest["members"] and len(files) == expected["members"]
         and fileset.hexdigest() == expected["fileset"], "whole archive/member identity differs")
    prefix = expected["capture_prefix"]
    for name in ("cases.json", "module.pkl", "queries.tsv"):
        need(files[prefix + "/" + name] == (fixture / name).read_bytes(),
             "authored fixture input differs")
    return files, archive_id


def verify_emptiness(files: dict[str, bytes]) -> None:
    prefix = "capture-02"
    provenance = json.loads(files[prefix + "/provenance.json"])
    need(len(provenance["source_before"]) == 58, "pinned source closure differs")
    validate_capture(files, prefix, mode="string", rows_count=47,
                     expected_statuses={"success": 34, "error": 13}, expected_raw_files=989,
                     module_uri=EXPECTED["collection-emptiness"]["uri"],
                     source_names=tuple(provenance["source_before"]))
    launch = json.loads(files["launch-02/process.json"])
    need(launch["exit_code"] == 0 and launch["source_stable"], "outer launch differs")
    review = json.loads(files["review-01/report.json"])
    need(review["complete"] and review["approved"], "independent emptiness review differs")


def verify_every(files: dict[str, bytes]) -> None:
    base = "capture-03/"
    capture = json.loads(files[base + "report.json"])
    cases = json.loads(files[base + "cases.json"])
    rows = cases["rows"]
    need(cases["schema"] == 1 and cases["baseline"] == "Pkl 0.32.0"
         and cases["uri"] == EXPECTED["collection-every"]["uri"]
         and cases["repetitions"] == 2 and len(rows) == 64
         and len({r["id"] for r in rows}) == 64
         and all(set(r) == {"id", "expression"}
                 and re.fullmatch(r"[a-z0-9-]+", r["id"])
                 and "\t" not in r["expression"] and "\n" not in r["expression"]
                 for r in rows), "every authored case set differs")
    need(files[base + "queries.tsv"] == "".join(
        f"string\t{r['id']}\t{r['expression']}\n" for r in rows).encode(),
        "every ordered queries differ")
    need(capture["schema"] == 1 and capture["complete"] and capture["source_stable"]
         and capture["input_stable"] and capture["raw_equal"]
         and capture["rows"] == 64 and capture["uri"] == cases["uri"]
         and capture["candidate_compatibility_passes_claimed"] == 0
         and capture["jar_sha256"] == JAR_SHA,
         "every capture completion/authority differs")
    need(capture["inputs_before"] == capture["inputs_after"]
         and set(capture["inputs_before"]) ==
         {"run.py", "CollectionEveryOracle.java", "cases.json", "module.pkl", "queries.tsv", "README.md"}
         and all(sha(files[base + name]) == digest
                 for name, digest in capture["inputs_before"].items()),
         "every copied inputs differ")
    need(capture["sources_before"] == capture["sources_after"] == capture["source_copies_after"]
         and len(capture["sources_before"]) == 80
         and all(sha(files[base + "sources/" + name]) == digest
                 for name, digest in capture["sources_before"].items()),
         "every pinned source closure differs")
    lock = json.loads(files[base + "sources/oracle.lock.json"])
    need(lock["version"] == "0.32.0"
         and lock["source"]["revision"] == "e51fb2f91774265666f94980ec630db794a4db2d"
         and lock["artifact"]["sha256"] == JAR_SHA
         and sha(files[base + "sources/oracle.lock.json"]) == capture["oracle_lock_sha256"]
         and sha(files[base + "sources/references.lock"]) == capture["references_lock_sha256"],
         "every pinned locks differ")
    jdk = capture["jdk"]
    raw_jdk = files[base + "java-identity.stderr.bin"]
    props = dict(re.findall(r"^\s+([\w.]+) = (.*)$", raw_jdk.decode(), re.M))
    need(files[base + "java-identity.stdout.bin"] == b"" and props == jdk["properties"]
         and b'openjdk version "21.0.12.1"' in raw_jdk
         and (jdk["executable_sha256"], jdk["modules_sha256"], jdk["release_sha256"]) == JDK_SHA
         and all(props.get(k) == v for k, v in lock["jdk"]["properties"].items()),
         "every raw JDK identity differs")
    sessions = capture["sessions"]
    need(len(sessions) == 2 and [s["repeat"] for s in sessions] == [0, 1]
         and len({s["pid"] for s in sessions}) == 2
         and sessions[0]["finished_epoch"] <= sessions[1]["started_epoch"]
         and all(s["exit_code"] == 0 and s["rows"] == 64 and not s["timed_out"]
                 for s in sessions), "every two fresh sessions differ")
    row_sets = []
    statuses = Counter()
    for session in sessions:
        repeat = session["repeat"]
        session_prefix = base + f"repeat-{repeat}/"
        stdout = files[session_prefix + "process.stdout.bin"]
        stderr = files[session_prefix + "process.stderr.bin"]
        need(sha(stdout) == session["stdout_sha256"]
             and sha(stderr) == session["stderr_sha256"] and stderr == b"",
             "every process raw channel differs")
        expected_argv = [jdk["executable"], *lock["jvm_options"],
            f"-Duser.home={capture['environment']['HOME']}",
            f"-Djava.io.tmpdir={capture['environment']['TMPDIR']}", "-cp",
            session["argv"][-5], session["argv"][-4], session["argv"][-3],
            session["argv"][-2], cases["uri"]]
        need(session["argv"] == expected_argv
             and session["argv"][-5].endswith(lock["artifact"]["filename"])
             and session["argv"][-4] == session["cwd"] + "/CollectionEveryOracle.java"
             and session["argv"][-3] == session["cwd"] + f"/repeat-{repeat}"
             and session["argv"][-2] == session["cwd"],
             "every JVM invocation differs")
        protocol = ["session-open mode=string rows=64"]
        row_set = {}
        for index, row in enumerate(rows):
            folder = session_prefix + f"{index:03d}-{row['id']}/"
            actual_names = {name[len(folder):] for name in files if name.startswith(folder)}
            count = int(files[folder + "logs.count.bin"].decode().strip())
            need(count >= 0, "negative logger event count")
            expected_names = set(ROW_CHANNELS) | {"logs.count.bin"}
            for log in range(count):
                for field in LOG_FIELDS:
                    stem = f"log-{log:03d}.{field}"
                    expected_names.update((stem + ".bin", stem + ".code-units.bin"))
            need(actual_names == expected_names, f"every row/logger channel set differs: {row['id']}")
            need(checked(files, folder, "mode") == "string"
                 and checked(files, folder, "expression") == row["expression"],
                 "every row input differs")
            for field in ("result.stdout", "result.stderr", "result.error-class",
                          "result.exception-to-string", "result.cause-class",
                          "result.cause-message", "result.cause-to-string", "result.java-type"):
                checked(files, folder, field)
            for log in range(count):
                for field in LOG_FIELDS:
                    checked(files, folder, f"log-{log:03d}.{field}")
            status = files[folder + "result.status.bin"].decode().strip()
            error_class = files[folder + "result.error-class.bin"].decode()
            need(status in ("success", "error")
                 and (status != "error" or error_class == "org.pkl.core.PklException")
                 and (status != "success" or error_class == ""),
                 "every row status/error class differs")
            if repeat == 0:
                statuses[status] += 1
                receipt = capture["row_statuses"][index]
                need(receipt == {"id": row["id"], "expression": row["expression"],
                     "status": status,
                     "stdout_sha256": sha(files[folder + "result.stdout.bin"]),
                     "stderr_sha256": sha(files[folder + "result.stderr.bin"]),
                     "logs": count}, "every row receipt differs")
            call = f"string-{index}-{row['id']}"
            protocol += [f"call-enter={call}", f"call-exit={call} status={status}"]
            for name in actual_names:
                row_set[f"{index:03d}-{row['id']}/{name}"] = sha(files[folder + name])
        protocol.append(f"session-closed mode=string rows=64 public-calls=64 success=36 errors=28")
        need(stdout == ("\n".join(protocol) + "\n").encode(), "every complete process protocol differs")
        session_files = {name[len(session_prefix):]: sha(raw) for name, raw in files.items()
                         if name.startswith(session_prefix)}
        need(len(session_files) == 1800 and len(row_set) == 1798,
             "every session/row raw file count differs")
        row_sets.append(session_files)
    need(statuses == EXPECTED["collection-every"]["statuses"]
         and row_sets[0] == row_sets[1], "every statuses or raw repetitions differ")
    launch = json.loads(files["launch-03/process.json"])
    need(launch["exit_code"] == 0 and launch["source_stable"]
         and launch["stdout"] == {"bytes": len(files["launch-03/stdout.bin"]),
                                  "sha256": sha(files["launch-03/stdout.bin"])}
         and launch["stderr"] == {"bytes": len(files["launch-03/stderr.bin"]),
                                  "sha256": sha(files["launch-03/stderr.bin"])},
         "every outer launch differs")
    audit = json.loads(files["audit-02/report.json"])
    need(audit["complete"] and audit["rows"] == 64
         and audit["status_per_repetition"] == EXPECTED["collection-every"]["statuses"]
         and audit["raw_files_per_repetition"] == 1798
         and audit["pinned_source_copies"] == 80,
         "every independent audit differs")


def verify(kind: str, fixture: Path | None = None) -> dict:
    need(kind in EXPECTED, "unknown predicate profile")
    if fixture is None:
        fixture = default_fixture(kind)
    fixture = Path(fixture)
    files, identity = read_fixture(kind, fixture)
    if kind == "collection-emptiness":
        verify_emptiness(files)
    else:
        verify_every(files)
    return {"kind": kind, "rows": identity["rows"], "statuses": identity["statuses"],
            "members": identity["members"], "archive_sha256": identity["archive_sha256"],
            "raw_repetitions_equal": True, "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument("kind", choices=EXPECTED)
    parser.add_argument("--fixture", type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.kind, args.fixture), sort_keys=True))
