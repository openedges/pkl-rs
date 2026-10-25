"""Verify installed runtime fixture bytes against sealed Pkl 0.32.0 Java archives.

This offline verifier reads only tracked fixture roots and the tested Rust source.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re
import stat
import tarfile


PINS = {
    "string-base64": ("8b0a7d2f4a7b1e3b29218b3900ec7b0a4913799ee1224766391b80c400c832f0",
                      "8ee2a4988a818177ba18ae354567acba343c1b673e62e18fc3c137368642f257", 3833, "capture-02", 89),
    "collection-emptiness": ("431e41e78ff9f2ad799549becb30463cd67c7d4883a147add78daaf96fecbd62",
                             "ed94162e8ff012786ed6281e7bb136a7168a4ffb8f74c9027d89e17d2a116b5c", 2057, "capture-02", 47),
    "collection-every": ("42d5d89cd10c35de457f273da34b22d02fd48791210de7a3e15816b0955a979e",
                         "6872aa67dc17118cd6df73d15a971f5aa0f97f79b49c1c073c6765430724089d", 3701, "capture-03", 64),
}
PREDICATE_ROOT = {"README.md", "archive-identity.json", "cases.json", "manifest.json",
                  "member-manifest.json", "module.pkl", "oracle-evidence.tar.gz", "queries.tsv"}
BASE64_ROOT = PREDICATE_ROOT - {"member-manifest.json"}
LOG_FIELDS = ("level", "message", "module-uri", "member", "position")
BASE64_TEST_SHA = "6617f285636743adb48e1b8fd1ef2ea6e0577a79dbf6b13c3d1d5ac8849bd464"


def need(ok: bool, message: str) -> None:
    if not ok:
        raise ValueError(message)


def sha(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def ident(path: Path) -> dict:
    need(path.is_file() and not path.is_symlink() and stat.S_ISREG(path.lstat().st_mode),
         "missing/linked fixture file: " + str(path))
    raw = path.read_bytes()
    return {"bytes": len(raw), "sha256": sha(raw), "mode": stat.S_IMODE(path.stat().st_mode)}


def files(root: Path) -> dict[str, Path]:
    need(root.is_dir() and not root.is_symlink(), "missing/linked fixture root: " + str(root))
    result = {}
    for path in root.rglob("*"):
        need(not path.is_symlink(), "linked fixture member: " + str(path))
        if path.is_dir():
            continue
        need(path.is_file() and stat.S_ISREG(path.lstat().st_mode), "nonregular fixture member")
        result[path.relative_to(root).as_posix()] = path
    return result


def authority(root: Path, kind: str) -> tuple[dict[str, bytes], dict]:
    archive_sha, fileset_sha, count, _prefix, _rows = PINS[kind]
    own = files(root)
    need(set(own) == (BASE64_ROOT if kind == "string-base64" else PREDICATE_ROOT),
         "sealed authority root file set differs: " + kind)
    manifest = json.loads(own["manifest.json"].read_bytes())
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0",
                      "files": {name: sha(path.read_bytes()) for name, path in own.items()
                                if name != "manifest.json"}},
         "sealed authority root manifest differs: " + kind)
    need(sha(own["oracle-evidence.tar.gz"].read_bytes()) == archive_sha,
         "sealed archive SHA differs: " + kind)
    archive_id = json.loads(own["archive-identity.json"].read_bytes())
    need(archive_id["archive_sha256"] == archive_sha
         and archive_id["fileset_sha256"] == fileset_sha
         and archive_id["members"] == count
         and archive_id["candidate_compatibility_passes_claimed"] == 0,
         "sealed archive identity differs: " + kind)
    result = {}
    actual = {}
    fileset = hashlib.sha256()
    with tarfile.open(own["oracle-evidence.tar.gz"], "r:gz") as archive:
        for member in archive:
            name = member.name
            need(member.isfile() and not Path(name).is_absolute()
                 and ".." not in Path(name).parts and name not in result
                 and member.mode == 0o644, "unsafe/duplicate archive member: " + name)
            stream = archive.extractfile(member)
            need(stream is not None, "unreadable archive member: " + name)
            raw = stream.read()
            need(len(raw) == member.size, "archive member size differs: " + name)
            result[name] = raw
            actual[name] = {"bytes": len(raw), "sha256": sha(raw), "mode": member.mode}
            fileset.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(result) == count and fileset.hexdigest() == fileset_sha,
         "whole Java archive inventory differs: " + kind)
    if kind != "string-base64":
        member_manifest = json.loads(own["member-manifest.json"].read_bytes())
        need(member_manifest == {"schema": 1, "members": actual},
             "archive member manifest differs: " + kind)
    return result, {"archive_sha256": archive_sha, "fileset_sha256": fileset_sha,
                    "members": count, "root_files": sorted(own)}


def runtime_members(kind: str, archive: dict[str, bytes]) -> dict[str, str]:
    _archive_sha, _fileset_sha, _count, prefix, rows_count = PINS[kind]
    cases = json.loads(archive[prefix + "/cases.json"])
    rows = cases["rows"]
    need(len(rows) == rows_count and cases["baseline"] == "Pkl 0.32.0",
         "authored row count/baseline differs: " + kind)
    uri = cases.get("uri", cases.get("module_uri"))
    need(uri == {"string-base64": "file:///stage68/string-base64-exploration.pkl",
                 "collection-emptiness": "file:///stage68/collection-emptiness.pkl",
                 "collection-every": "file:///stage68/collection-every.pkl"}[kind],
         "authored module URI differs: " + kind)
    result = {name: prefix + "/" + name for name in
              (("README.md", "archive-identity.json", "cases.json", "manifest.json",
                "module.pkl", "oracle-evidence.tar.gz", "queries.tsv")
               if kind == "string-base64" else
               (("cases.json", "module.pkl") if kind == "collection-emptiness" else
                ("cases.json", "module.pkl", "queries.tsv", "report.json")))}
    if kind == "string-base64":
        # The seven-file Base64 envelope is also the runtime fixture root.
        return {name: member for name, member in result.items()
                if name in ("cases.json", "module.pkl", "queries.tsv")}
    total_logs = 0
    statuses = {"success": 0, "error": 0}
    for index, row in enumerate(rows):
        row_dir = f'{prefix}/repeat-0/{index:03d}-{row["id"]}/'
        repeat_dir = f'{prefix}/repeat-1/{index:03d}-{row["id"]}/'
        status = archive[row_dir + "result.status.bin"]
        need(status in (b"success\n", b"error\n"), "nonordinary Java result status")
        need(archive[repeat_dir + "result.status.bin"] == status,
             "Java result status repetition differs: " + row_dir)
        statuses[status.decode().strip()] += 1
        channel = "stdout" if status == b"success\n" else "stderr"
        stem = f'{index:03d}-{row["id"]}'
        channels = (("result." + channel + ".code-units.bin",) if kind == "collection-emptiness"
                    else ("result." + channel + ".bin",
                          "result." + channel + ".code-units.bin"))
        if kind == "collection-emptiness":
            output = f"expected/{stem}.utf16be.bin"
            result[output] = row_dir + channels[0]
        else:
            for channel_name in channels:
                result[f"expected/{stem}-{channel_name}"] = row_dir + channel_name
            count_raw = archive[row_dir + "logs.count.bin"]
            need(count_raw.rstrip(b"\n").isdigit() and count_raw.endswith(b"\n"),
                 "invalid Java logger count")
            count = int(count_raw)
            total_logs += count
            for log_index in range(count):
                for field in LOG_FIELDS:
                    for suffix in ("bin", "code-units.bin"):
                        channel_name = f"log-{log_index:03d}.{field}.{suffix}"
                        result[f"expected/{stem}-{channel_name}"] = row_dir + channel_name
        for channel_name in channels:
            need(archive[row_dir + channel_name] == archive[repeat_dir + channel_name],
                 "Java raw repetition differs: " + row_dir + channel_name)
        if kind == "collection-every":
            need(archive[row_dir + "logs.count.bin"] == archive[repeat_dir + "logs.count.bin"],
                 "Java logger count repetition differs")
            for log_index in range(count):
                for field in LOG_FIELDS:
                    for suffix in ("bin", "code-units.bin"):
                        name = f"log-{log_index:03d}.{field}.{suffix}"
                        need(archive[row_dir + name] == archive[repeat_dir + name],
                             "Java logger repetition differs: " + name)
    need(statuses == ({"success": 34, "error": 13} if kind == "collection-emptiness"
                      else {"success": 36, "error": 28}),
         "Java status counts differ: " + kind)
    need(len(result) == (49 if kind == "collection-emptiness" else 522)
         and (kind != "collection-every" or total_logs == 39),
         "raw expected file count/logger total differs: " + kind)
    return result


def inline_base64(test_source: Path, archive: dict[str, bytes]) -> None:
    need(ident(test_source)["sha256"] == BASE64_TEST_SHA,
         "tested Base64 Rust source identity differs")
    code = test_source.read_text()
    pattern = re.compile(r'\(\s*r########"(.*?)"########,\s*r########"(.*?)"########,\s*'
                         r'r########"(.*?)"########,\s*r########"(.*?)"########\s*'
                         r'\.encode_utf16\(\)\s*\.collect::<Vec<_>>\(\),\s*\)', re.S)
    inline = [match.groups() for match in pattern.finditer(code)]
    cases = json.loads(archive["capture-02/cases.json"])
    need(len(inline) == len(cases["rows"]) == 89,
         "Base64 inline expected row count differs")
    statuses = {"success": 0, "error": 0}
    for index, (authored, (case_id, expression, kind, expected)) in enumerate(zip(cases["rows"], inline)):
        row_dir = f"capture-02/repeat-0/{index:03d}-{case_id}/"
        repeat_dir = f"capture-02/repeat-1/{index:03d}-{case_id}/"
        status = archive[row_dir + "result.status.bin"]
        need(status in (b"success\n", b"error\n")
             and archive[repeat_dir + "result.status.bin"] == status,
             "Base64 Java status differs")
        status_name = status.decode().strip()
        channel = "stdout" if status_name == "success" else "stderr"
        member = "result." + channel + ".code-units.bin"
        raw = expected.encode("utf-16-be", "surrogatepass")
        need(case_id == authored["id"] and expression == authored["expression"]
             and kind == ("value" if status_name == "success" else "error")
             and raw == archive[row_dir + member]
             and raw == archive[repeat_dir + member],
             "Base64 inline expectation differs from Java: " + str(index))
        statuses[status_name] += 1
    need(statuses == {"success": 67, "error": 22}, "Base64 status totals differ")


def verify(project_root: Path | None = None) -> dict:
    project_root = Path(project_root) if project_root is not None else Path(__file__).resolve().parents[1]
    fixtures = project_root / "tests/fixtures"
    need(fixtures.is_dir() and not fixtures.is_symlink(), "fixture directory missing or linked")
    total_runtime = 0
    raw_expected = 0
    for kind in PINS:
        name = "m2-" + kind
        sealed_name = name if kind == "string-base64" else name + "-oracle"
        archive, _meta = authority(fixtures / sealed_name, kind)
        expected = runtime_members(kind, archive)
        runtime = files(fixtures / name)
        allowed = BASE64_ROOT if kind == "string-base64" else set(expected)
        need(set(runtime) == allowed,
             "installed runtime file set missing/extra: " + kind)
        for relative, path in runtime.items():
            if relative in expected:
                need(path.read_bytes() == archive[expected[relative]],
                     "runtime input/result/logger byte differs from Java: " + kind + "/" + relative)
            elif kind != "string-base64":
                raise ValueError("undeclared runtime file: " + kind + "/" + relative)
            total_runtime += 1
            raw_expected += relative.startswith("expected/")
        if kind == "string-base64":
            inline_base64(project_root / "crates/pkl-runtime/tests/m2_string_base64.rs", archive)
    need(total_runtime == 578 and raw_expected == 565,
         "runtime raw fixture coverage differs")
    return {"profiles": 3, "runtime_files": total_runtime,
            "raw_expected_files": raw_expected, "base64_inline_rows": 89,
            "archive_members_read": sum(pin[2] for pin in PINS.values()),
            "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=None,
                        help="repository root; defaults to parent of installed tools directory")
    options = parser.parse_args()
    print(json.dumps(verify(options.root), sort_keys=True))


