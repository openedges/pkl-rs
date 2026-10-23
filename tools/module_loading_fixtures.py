#!/usr/bin/env python3
"""Verify Java-origin module graph fixtures; this does not run an evaluator."""
from __future__ import annotations

import argparse
import base64
import hashlib
import json
import re
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-module-loading"
ORIGINALS = {f"original-import{i}" for i in range(1, 4)}


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def load(path: Path):
    return json.loads(path.read_bytes())


def encoded(value) -> bytes:
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def payload(root: Path, name: str) -> bytes:
    require(isinstance(name, str) and name != "", "empty fixture path")
    path = Path(name)
    require(not path.is_absolute() and ".." not in path.parts, "unsafe fixture path")
    require((root / path).is_file() and not (root / path).is_symlink(), "missing fixture file: " + name)
    return (root / path).read_bytes()


def raw_hex(value: str) -> bytes:
    require(isinstance(value, str) and re.fullmatch(r"(?:[0-9a-f]{2})*", value) is not None,
            "invalid raw byte hex")
    return bytes.fromhex(value)


def code_units(data: bytes) -> list[int]:
    require(len(data) % 2 == 0, "incomplete UTF16 unit")
    return list(struct.unpack(">" + "H" * (len(data) // 2), data))


def verify_package(root: Path = DEFAULT, repository: Path | None = ROOT) -> dict:
    manifest = load(root / "manifest.json")
    files = manifest["files"]
    actual = set()
    for path in root.rglob("*"):
        require(not path.is_symlink(), "fixture symlink is not allowed")
        if path.is_file() and path.name != "manifest.json":
            actual.add(path.relative_to(root).as_posix())
    require(set(files) == actual, "fixture membership differs from manifest")
    for path, expected in files.items():
        require(digest(payload(root, path)) == expected, "fixture file hash differs: " + path)

    data = load(root / "cases.json")
    cases = data["cases"]
    ids = [case["id"] for case in cases]
    require(data["schema"] == manifest["schema"] == 1, "unsupported fixture schema")
    require(len(ids) == len(set(ids)) == manifest["fixtureCount"], "duplicate or missing fixtures")
    require(set(ids) >= ORIGINALS, "original import1/2/3 contract is missing")
    require(set(manifest["originalCases"]) == ORIGINALS, "original fixture coverage differs")

    origins = load(root / "origins.json")
    bindings = {entry["path"]: entry for entry in origins["bindings"]}
    for key, origin in origins["origins"].items():
        report = payload(root, origin["reportFile"])
        closure_bytes = payload(root, origin["closureFile"])
        require(digest(report) == origin["reportSha256"], "origin report differs: " + key)
        require(digest(closure_bytes) == origin["closureSha256"], "origin closure differs: " + key)
        index = load(root / origin["indexFile"])
        if origin["indexFile"].endswith("inventory.json"):
            require(digest(payload(root, origin["indexFile"])) == json.loads(closure_bytes)["inventorySha256"],
                    "origin inventory differs")
            entries = {row["path"]: row["sha256"] for row in index["files"]}
        else:
            entries = index["files"]
        for binding in origins["bindings"]:
            if binding["origin"] == key:
                require(entries.get(binding["originPath"]) == binding["sha256"],
                        "payload does not match original closed inventory")
                require(digest(payload(root, binding["path"])) == binding["sha256"],
                        "original payload bytes differ")

    identity = origins["identityMetadata"]
    require(digest(payload(root, identity["path"])) == identity["sha256"], "identity metadata differs")
    identities = load(root / identity["path"])
    observed_identities = {}
    for name in ["identity.stdout.bin", "identity-extra.stdout.bin"]:
        for line in payload(root, "origins/file-identities/" + name).decode().splitlines():
            parts = line.split("\t")
            if parts[3] == "ERROR":
                observed = {"importUri": base64.b64decode(parts[1]).decode(),
                            "originalUri": base64.b64decode(parts[2]).decode(),
                            "failure": {"class": base64.b64decode(parts[4]).decode(),
                                        "message": base64.b64decode(parts[5]).decode()}}
            else:
                values = [base64.b64decode(value).decode() for value in parts[1:6]]
                observed = dict(zip(["importUri", "originalUri", "resolvedUri", "sourceUri", "inferredName"], values))
                observed["observedCurrentSourceSha256"] = parts[6]
            require(parts[0] not in observed_identities, "duplicate Java identity observation")
            observed_identities[parts[0]] = observed
    require(identities == observed_identities, "identity metadata differs from raw Java observation")

    def text_source(text: str, path: str, expected: str) -> None:
        raw = payload(root, path)
        require(raw == text.encode("utf-8") and digest(raw) == expected, "source bytes differ")

    def expected_result(result: dict, expression: str | None = None) -> None:
        paths = result["payloadOrigin"]
        require(paths and all(path in bindings for path in paths), "unbound Java expected payload")
        values = [(path, payload(root, path)) for path in paths]
        if result["kind"] == "bytes":
            stdout = [raw for path, raw in values if path.endswith("/stdout.bin")]
            stderr = [raw for path, raw in values if path.endswith("/stderr.bin")]
            captures = [json.loads(raw) for path, raw in values if path.endswith("/result.json")]
            require(len(stdout) == len(stderr) == len(captures) == 2, "both Java CLI repeats are required")
            require(all(raw == raw_hex(result["bytesHex"]) for raw in stdout), "expected CLI bytes differ")
            require(all(raw == raw_hex(result["javaStderrHex"]) for raw in stderr), "expected CLI stderr differs")
            require(all(row["exit_code"] == result["javaExitCode"] and row["files"] == result["javaFiles"]
                        and not row["timed_out"] for row in captures), "expected CLI status/files differ")
        else:
            require(result["kind"] in {"value", "error"}, "unknown expected result kind")
            success = result["kind"] == "value"
            channel = "stdout" if success else "stderr"
            selected = [raw for path, raw in values if path.endswith("." + channel + ".bin")]
            status = [raw for path, raw in values if path.endswith(".status.bin")]
            classes = [raw for path, raw in values if path.endswith(".error-class.bin")]
            require(len(selected) == len(status) == len(classes) == 2, "both Java API repeats are required")
            require(all(raw == raw_hex(result["javaUtf8Hex"]) for raw in selected), "expected API payload differs")
            require(all(raw == (result["javaStatus"] + "\n").encode() for raw in status), "expected API status differs")
            require(result["javaStatus"] == ("success" if success else "error"), "API result/status mismatch")
            require(all(raw.decode() == result["javaErrorClass"] for raw in classes), "Java exception provenance differs")
            expressions = [raw for path, raw in values if path.endswith(".expression.bin")]
            require(expression is not None and len(expressions) == 2
                    and all(raw == expression.encode() for raw in expressions), "Java expression differs")
            unit_name = "javaValueUnits" if success else "javaErrorUnits"
            unit_payloads = [raw for path, raw in values if path.endswith("." + channel + ".code-units.bin")]
            if unit_name in result:
                require(len(unit_payloads) == 2 and all(code_units(raw) == result[unit_name] for raw in unit_payloads),
                        "expected Java UTF16 units differ")
            else:
                require(not unit_payloads and result["stringBoundary"] == "java-utf8-serialization-only",
                        "uncaptured Java UTF16 units must not be invented")

    for case in cases:
        text_source(case["source"], case["sourceFile"], case["sourceSha256"])
        primary = identities[case["primaryIdentityRow"]]
        require(case["primaryIdentity"] == {key: primary[key] for key in ["originalUri", "resolvedUri", "sourceUri"]},
                "primary ModuleKey identity differs")
        require(case["uri"] == primary["sourceUri"] and case["moduleName"] == primary["inferredName"],
                "primary URI/name differs from Java metadata")
        for process_path in case["provenance"]["processFiles"]:
            process = load(root / process_path)
            require(process["moduleUri"] == case["uri"] and process["sourceSha256"] == case["sourceSha256"],
                    "source/context differs from Java process")
        require(case["primaryIdentity"]["originalUri"] == case["primaryIdentity"]["resolvedUri"] == case["uri"],
                "current session cannot represent a different primary resolved identity")
        for row in case["resolutions"]:
            if "identityRow" in row:
                observed = identities[row["identityRow"]]
                require(all(row[key] == observed[key] for key in ["importUri", "originalUri", "resolvedUri", "sourceUri"]),
                        "resolution identity differs from observed Java")

        def source_entry(entry: dict) -> None:
            if "loads" in entry:
                require(entry["loads"], "empty source load plan")
                for leaf in entry["loads"]:
                    source_entry(leaf)
            elif "failure" not in entry:
                text_source(entry["text"], entry["sourceFile"], entry["sourceSha256"])

        for entry in case["sources"]:
            source_entry(entry)
        for action in case["actions"]:
            if action["kind"] == "set-source":
                source_entry(action["source"])
        expected_indices = [index for index, action in enumerate(case["actions"])
                            if action["kind"] in {"path", "output-bytes"}]
        require([result["actionIndex"] for result in case["expected"]["results"]] == expected_indices,
                "missing or reordered expected calls")
        for result in case["expected"]["results"]:
            action = case["actions"][result["actionIndex"]]
            expression = case["paths"][action["index"]] if action["kind"] == "path" else None
            expected_result(result, expression)
        for result in case["expected"].get("callbacks", []):
            expected_result(result, case["paths"][result["path"]])
        origin_prefix = "origins/" + case["provenance"]["origin"] + "/cases/" + case["provenance"]["id"] + "/0/calls/"
        callback_files = [path for path in bindings if path.startswith(origin_prefix)
                          and re.fullmatch(r"inner-[0-9]+\.status\.bin", path[len(origin_prefix):])]
        require(len(callback_files) == len(case["expected"].get("callbacks", [])), "missing Java callbacks")
        if "requestCounts" in case["expected"]:
            raw = payload(root, origin_prefix + "counters.bin").decode()
            counts = dict(line.split("=") for line in raw.splitlines())
            require(case["expected"]["requestCounts"] == {"resolve": int(counts["resolve"]),
                                                          "load-source": int(counts["source"])},
                    "request counts differ from Java observation")
        log_files = [path for path in bindings if path.startswith(origin_prefix)
                     and re.fullmatch(r"log-[0-9]+\.level\.bin", path[len(origin_prefix):])]
        require(len(log_files) == len(case["expected"].get("logs", [])), "missing Java logs")
        for index, log in enumerate(case["expected"].get("logs", [])):
            prefix = origin_prefix + f"log-{index}"
            require(log["level"].encode() == payload(root, prefix + ".level.bin"), "log level differs")
            for field, suffix in [("message", "message"), ("moduleUri", "uri")]:
                require(log[field] == code_units(payload(root, prefix + f".{suffix}.code-units.bin")),
                        "log UTF16 field differs")
            member = None if payload(root, prefix + ".member.null.bin") == b"true" else code_units(
                payload(root, prefix + ".member.code-units.bin"))
            require(log["memberName"] == member, "log member null/UTF16 field differs")
            position = [int(value) for value in payload(root, prefix + ".position.bin").decode().split(":")]
            require([log[key] for key in ["startLine", "startColumn", "endLine", "endColumn"]] == position,
                    "log source position differs")
            for line, expected in enumerate(log["sourceLines"]):
                require(expected == code_units(payload(root, prefix + f".source-{line}.code-units.bin")),
                        "log source line differs")
        require(case["oracleError"] == any(row.get("javaErrorClass") == "org.pkl.core.PklBugException"
                                           for row in case["expected"]["results"]), "oracle error marker differs")
        if case["id"] in ORIGINALS:
            original = case["originalContract"]
            require(digest(encoded(original["upstreamCase"])) == original["upstreamCaseSha256"],
                    "original contract digest differs")
            require(case["arguments"] == original["upstreamCase"]["command"]
                    and case["uri"] == original["moduleUri"]
                    and case["sourceSha256"] == original["sourceSha256"], "original invocation/source differs")
            if repository is not None:
                require(digest((repository / original["source"]).read_bytes()) == original["sourceSha256"],
                        "original fixture does not match pinned source")

    return {"scope": "Fixture integrity only; no evaluator execution or compatibility claim.",
            "fixtures": len(cases), "originalCliCases": len(ORIGINALS), "javaPayloadBindings": len(bindings),
            "pendingBoundaryControls": data["unexecutedBoundaryControls"]}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=DEFAULT)
    arguments = parser.parse_args()
    print(json.dumps(verify_package(arguments.fixtures), indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
