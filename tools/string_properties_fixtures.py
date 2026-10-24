"""Offline byte and protocol verifier for the Pkl 0.32.0 String property authority."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile

from collection_conversions_fixtures import (ARTIFACT_SHA256, validate_capture,
                                             validate_rows, validate_row_channels)
from string_properties_oracle import SOURCES, JDK_SOURCE_SHA256

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-string-properties"
ARCHIVE_PREFIX = "capture-02"
DIRECT_PREFIX = "jdk-01"
ARCHIVE_SHA256 = "a0dc5929d76467b30c92bb82fb54867e8a4ba4b4cac6d648eb344d03af596ae5"
FILESET_SHA256 = "79d048ce5125198f9e2ce1024e0942557c32a99c65169655fcf5e7f68edcdb5c"
MEMBERS = 6233
ROWS = 146
STATUSES = {"success": 117, "error": 29}
RAW_FILES = 3068
MODULE_URI = "file:///stage67/string-properties.pkl"
BITSET_SHA256 = "2861bb010a0faac3ae03df880d56379f8ea0890e5d1d7089cd4cd9a804d43e57"
JDK_BITSET_SHA256 = "70a0045776ee862d943e8eb43512ca18bae72c522b49e8a2b29e36af26ef4a95"
WHITE_SPACE = (0x9, 0xA, 0xB, 0xC, 0xD, 0x20, 0x85, 0xA0, 0x1680,
               *range(0x2000, 0x200B), 0x2028, 0x2029, 0x202F, 0x205F, 0x3000)
ROOT_FILES = {"README.md", "archive-identity.json", "oracle-evidence.tar.gz",
              "module.pkl", "queries.tsv", "cases.json", "pkl-whitespace-bmp.bitset.bin",
              "manifest.json"}


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def identity(raw, mode=None):
    result = {"bytes": len(raw), "sha256": sha(raw)}
    if mode is not None:
        result["mode"] = mode
    return result


def bitset(indices):
    data = bytearray(8192)
    for index in indices:
        data[index >> 3] |= 1 << (index & 7)
    return bytes(data)


def direct_capture(files, root_bitset):
    prefix = DIRECT_PREFIX + "/"
    observed = {name[len(prefix):]: raw for name, raw in files.items() if name.startswith(prefix)}
    need(len(observed) == 26, "direct JDK supplement member set differs")
    provenance = json.loads(observed["provenance.json"])
    need(provenance["schema"] == 1 and provenance["complete"] is True
         and provenance["stable"] is True and provenance["raw_equal"] is True
         and provenance["candidate_compatibility_passes_claimed"] == 0,
         "direct JDK supplement status differs")
    need(provenance["jar"] == provenance["jar_after"]
         and provenance["jar"]["sha256"] == ARTIFACT_SHA256
         and provenance["jdk"]["source_archive_identity"]["sha256"] == JDK_SOURCE_SHA256
         and provenance["jdk_source_after"] == provenance["jdk"]["source_archive_identity"],
         "direct JDK artifact identity differs")
    expected_sources = {
        "external/pkl/pkl-core/src/main/java/org/pkl/core/util/StringUtils.java": "StringUtils.java",
        "target/m2/stage67-string-blank-jdk-prep-01/WhitespaceBitset.java": "WhitespaceBitset.java",
        "target/m2/stage67-string-blank-jdk-prep-01/run.py": "sources/producer.py",
        "oracle.lock.json": "sources/oracle.lock.json",
        "references.lock": "sources/references.lock",
    }
    need(provenance["source_before"] == provenance["source_after"]
         and set(provenance["source_before"]) == set(expected_sources),
         "direct JDK source closure differs")
    for source, member in expected_sources.items():
        need(identity(observed[member]) == provenance["source_before"][source],
             "direct JDK source copy differs: " + source)
    need(provenance["oracle_lock"] == identity(observed["sources/oracle.lock.json"])
         and provenance["references_lock"] == identity(observed["sources/references.lock"]),
         "direct JDK lock copy differs")
    need(set(provenance["jdk_source_entries"]) == {
        "java.base/java/lang/Character.java", "java.base/java/lang/CharacterData.java",
        "java.base/java/lang/CharacterDataLatin1.java", "java.base/java/lang/String.java",
        "java.base/java/lang/StringLatin1.java", "java.base/java/lang/StringUTF16.java"},
         "direct JDK source set differs")
    for name, record in provenance["jdk_source_entries"].items():
        need(identity(observed["jdk-sources/" + name]) == record,
             "direct JDK source bytes differ: " + name)
    raw_properties = dict(re.findall(r"^\s+([\w.]+) = (.*)$",
                                     observed["java-identity.stderr.bin"].decode(), re.M))
    need(observed["java-identity.stdout.bin"] == b""
         and raw_properties == provenance["jdk"]["properties"]
         and b'openjdk version "21.0.12.1"' in observed["java-identity.stderr.bin"],
         "direct raw Java identity differs")
    sessions = provenance["sessions"]
    need(len(sessions) == 2 and [s["repeat"] for s in sessions] == [0, 1]
         and sessions[0]["pid"] != sessions[1]["pid"]
         and sessions[0]["finished_epoch"] <= sessions[1]["started_epoch"],
         "direct ordered JVM sessions differ")
    raw_sets = []
    for repeat, session in enumerate(sessions):
        need(session["exit_code"] == 0 and session["command"][0] == provenance["jdk"]["executable"]
             and session["command"][1:11] == json.loads(observed["sources/oracle.lock.json"])["jvm_options"]
             and session["command"][-1].endswith(f"/repeat-{repeat}/bits"),
             "direct JVM command differs")
        base = f"repeat-{repeat}/"
        names = {name[len(base):] for name in observed if name.startswith(base)}
        need(names == set(session["files"]), "direct session file set differs")
        raw_set = {}
        for name, record in session["files"].items():
            raw = observed[base + name]
            need(identity(raw) == record, "direct session raw channel differs")
            raw_set[name] = sha(raw)
        for channel in ("stdout", "stderr"):
            need(identity(observed[base + "process." + channel + ".bin"]) == session[channel],
                 "direct process stream differs")
        need(observed[base + "process.stderr.bin"] == b""
             and observed[base + "process.stdout.bin"] == observed[base + "bits/summary.txt"],
             "direct process content differs")
        raw_sets.append(raw_set)
    need(raw_sets[0] == raw_sets[1], "direct raw JVM repetitions differ")
    pkl = observed["repeat-0/bits/pkl-whitespace-bmp.bitset.bin"]
    jdk = observed["repeat-0/bits/jdk-string-isblank-bmp.bitset.bin"]
    need(len(pkl) == len(jdk) == 8192 and sha(pkl) == BITSET_SHA256
         and sha(jdk) == JDK_BITSET_SHA256
         and pkl == root_bitset == bitset(WHITE_SPACE)
         and observed["repeat-0/bits/jdk-character-whitespace-bmp.bitset.bin"] == jdk,
         "direct BMP bitset differs")
    pkl_only = {i for i in range(65536) if pkl[i >> 3] & 1 << (i & 7)
                and not jdk[i >> 3] & 1 << (i & 7)}
    jdk_only = {i for i in range(65536) if jdk[i >> 3] & 1 << (i & 7)
                and not pkl[i >> 3] & 1 << (i & 7)}
    need(pkl_only == {0x85, 0xA0, 0x2007, 0x202F}
         and jdk_only == {0x1C, 0x1D, 0x1E, 0x1F},
         "direct Pkl/JDK whitespace distinction differs")
    return {"pkl_bitset_sha256": sha(pkl), "jdk_bitset_sha256": sha(jdk),
            "raw_repetitions_equal": True}


def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256,
           expected_fileset=FILESET_SHA256):
    root = Path(root)
    need(root.is_dir() and not root.is_symlink(), "fixture directory missing or linked")
    entries = list(root.iterdir())
    need(all(path.is_file() and not path.is_symlink() for path in entries)
         and {path.name for path in entries} == ROOT_FILES,
         "undeclared or nonregular fixture root path")
    manifest = json.loads((root / "manifest.json").read_bytes())
    root_hashes = {p.name: sha(p.read_bytes()) for p in entries if p.name != "manifest.json"}
    need(manifest == {"schema": 1, "baseline": "Pkl 0.32.0", "files": root_hashes},
         "root manifest differs")
    need(root_hashes["oracle-evidence.tar.gz"] == expected_archive,
         "archive SHA differs")
    archived = {}
    fileset = hashlib.sha256()
    with tarfile.open(root / "oracle-evidence.tar.gz", "r:gz") as source:
        for member in source:
            name = member.name
            need(member.isfile() and not Path(name).is_absolute()
                 and ".." not in Path(name).parts
                 and (name.startswith(ARCHIVE_PREFIX + "/")
                      or name.startswith(DIRECT_PREFIX + "/"))
                 and name not in archived,
                 "unsafe, duplicate or undeclared archive member")
            stream = source.extractfile(member)
            need(stream is not None, "unreadable archive member")
            raw = stream.read()
            need(len(raw) == member.size, "archive member size differs")
            archived[name] = raw
            fileset.update(name.encode() + b"\0" + sha(raw).encode() + b"\n")
    need(len(archived) == MEMBERS and fileset.hexdigest() == expected_fileset,
         "archive file set differs")
    expected_inputs = {name: {"bytes": (root / name).stat().st_size,
                              "sha256": root_hashes[name],
                              "mode": (root / name).stat().st_mode & 0o777}
                       for name in ("module.pkl", "queries.tsv", "cases.json",
                                    "pkl-whitespace-bmp.bitset.bin")}
    archive_id = json.loads((root / "archive-identity.json").read_bytes())
    need(archive_id == {
        "schema": 1, "baseline": "Pkl 0.32.0", "archive_sha256": expected_archive,
        "fileset_sha256": expected_fileset, "members": MEMBERS,
        "captures": {ARCHIVE_PREFIX: "stage67-string-properties-oracle-02",
                     DIRECT_PREFIX: "stage67-string-blank-jdk-01"},
        "root_inputs": expected_inputs,
        "producer": {"bytes": (ROOT / "tools/string_properties_oracle.py").stat().st_size,
                     "sha256": sha((ROOT / "tools/string_properties_oracle.py").read_bytes()),
                     "mode": (ROOT / "tools/string_properties_oracle.py").stat().st_mode & 0o777},
        "java_helper": {"bytes": (ROOT / "tools/java/CollectionConversionsOracle.java").stat().st_size,
                        "sha256": sha((ROOT / "tools/java/CollectionConversionsOracle.java").read_bytes()),
                        "mode": (ROOT / "tools/java/CollectionConversionsOracle.java").stat().st_mode & 0o777},
        "candidate_compatibility_passes_claimed": 0},
        "archive identity differs")
    for name in ("module.pkl", "queries.tsv", "cases.json"):
        need(archived[ARCHIVE_PREFIX + "/" + name] == (root / name).read_bytes(),
             "authored input differs: " + name)
    need(archived[ARCHIVE_PREFIX + "/run.py"] ==
         (ROOT / "tools/string_properties_oracle.py").read_bytes(),
         "archived producer differs from tracked source")
    need(archived[ARCHIVE_PREFIX + "/CollectionConversionsOracle.java"] ==
         (ROOT / "tools/java/CollectionConversionsOracle.java").read_bytes(),
         "archived Java helper differs from tracked source")
    statuses = validate_capture(archived, ARCHIVE_PREFIX, mode="string", rows_count=ROWS,
                                expected_statuses=STATUSES, expected_raw_files=RAW_FILES,
                                module_uri=MODULE_URI, source_names=SOURCES)
    provenance = json.loads(archived[ARCHIVE_PREFIX + "/provenance.json"])
    need(provenance["jdk"]["source_archive_sha256"] == JDK_SOURCE_SHA256
         and provenance["jdk_source_after_sha256"] == JDK_SOURCE_SHA256,
         "public JDK source archive differs")
    direct = direct_capture(archived, (root / "pkl-whitespace-bmp.bitset.bin").read_bytes())
    return {"baseline": "Pkl 0.32.0", "module_uri": MODULE_URI,
            "rows": ROWS, "statuses": statuses, "archive_sha256": expected_archive,
            "fileset_sha256": expected_fileset, "members": MEMBERS,
            "raw_repetitions_equal": True, "direct_bitset": direct,
            "candidate_compatibility_passes_claimed": 0}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixture", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
