#!/usr/bin/env python3
"""Verify the additive, Java-backed inheritance API fixture package offline."""
from __future__ import annotations

import argparse
import hashlib
import json
import struct
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / "tests/fixtures/m2-module-inheritance-platform"
ORIGINS = {
    "stage42-module-inheritance-oracle-01": (
        "fb385ee19c874cf3bd4757a605e168652eea24df2da2a08196c7b98ac42ce3fa",
        "a948815066e14d93ec37f7247edf8e9e5eaf43891a88c2ec7d1629914a4975d2",
    ),
    "stage42-module-extends-oracle-01": (
        "ee6c6b2a8361037fbcd539373fbe0f5166920e8b927a8940b93993fd4ef853db",
        "dff6fe28b48910cb9c899d276619629000856f201a5dcfe3202c903ca39f88b4",
    ),
}
IDS = ["abstract-parent-amends-first-error", "amends-parent-source-primary-identity",
       "extends-open-parent-values-and-class-identity"]


def require(value, message):
    if not value:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def read(root, name):
    path = Path(name)
    require(not path.is_absolute() and ".." not in path.parts and name, "unsafe fixture path")
    require(not any((root / parent).is_symlink() for parent in [path, *path.parents]),
            "fixture symlink is not allowed")
    return (root / path).read_bytes()


def load(root, name):
    return json.loads(read(root, name))


def units(raw):
    require(len(raw) % 2 == 0, "incomplete Java UTF16")
    return list(struct.unpack(">" + "H" * (len(raw) // 2), raw))


def verify_package(root=DEFAULT):
    root = Path(root)
    manifest = load(root, "manifest.json")
    actual = {p.relative_to(root).as_posix() for p in root.rglob("*")
              if p.is_file() and p.name != "manifest.json"}
    require(set(manifest["files"]) == actual, "fixture membership differs")
    for name, expected in manifest["files"].items():
        require(digest(read(root, name)) == expected, "fixture payload hash differs: " + name)
    data, provenance = load(root, "cases.json"), load(root, "origins.json")
    require(data["schema"] == manifest["schema"] == 1
            and manifest["family"] == "module-inheritance-api", "fixture schema differs")
    require([c["id"] for c in data["cases"]] == manifest["orderedIds"] == IDS
            and manifest["fixtureCount"] == len(IDS), "inheritance selection differs")
    require(set(provenance["origins"]) == set(ORIGINS), "oracle origins differ")
    bindings = {b["path"]: b for b in provenance["bindings"]}
    require(len(bindings) == len(provenance["bindings"]), "duplicate origin binding")
    reports, declared = {}, {}
    for key, origin in provenance["origins"].items():
        report_raw, closure_raw = read(root, origin["reportFile"]), read(root, origin["closureFile"])
        require((digest(report_raw), digest(closure_raw)) == ORIGINS[key], "pinned oracle identity differs")
        require(origin["reportSha256"] == ORIGINS[key][0]
                and origin["closureSha256"] == ORIGINS[key][1], "declared oracle identity differs")
        report, closure = json.loads(report_raw), json.loads(closure_raw)
        if origin["indexFile"] is not None:
            raw = read(root, origin["indexFile"])
            require(digest(raw) == closure["inventorySha256"], "closed oracle inventory differs")
            entries = {v["path"]: v["sha256"] for v in json.loads(raw)}
            entries["inventory.json"] = digest(raw)
        else:
            entries = {p: row["sha256"] for p, row in closure["inventory"].items()}
        entries["closure.json"] = digest(closure_raw)
        for binding in provenance["bindings"]:
            if binding["origin"] == key:
                require(entries.get(binding["originPath"]) == binding["sha256"]
                        == digest(read(root, binding["path"])), "closed oracle payload differs")
        reports[key] = report
        declared[key] = {c["id"]: c for c in load(root, origin["declaredCasesFile"])}
        require(not report["repeatDifferences"] and not report["inputChanges"], "oracle was not stable")

    def raw(path):
        require(path in bindings, "unbound raw oracle payload")
        return read(root, path)

    payload_count = 0
    for case in data["cases"]:
        key = case["provenance"]["origin"]
        original = declared[key][case["provenance"]["id"]]
        callback = original.get("callback", "")
        paths = original["expressions"] + ([callback] if callback else [])
        require(case["operation"] == "expressions" and case["source"] == original["source"]
                and case["sourceSha256"] == original["sourceSha256"]
                and case["uri"] == original["moduleUri"] and case["moduleName"] == "main"
                and case["paths"] == paths and not case["powerAssertions"], "source/API context differs")
        require(raw(case["sourceFile"]) == case["source"].encode()
                and digest(raw(case["sourceFile"])) == case["sourceSha256"], "primary source differs")
        require(case["actions"] == [{"kind": "path", "index": i}
                                     for i in range(len(original["expressions"]))], "ordered API calls differ")
        child = original["childUri"]
        require(case["resolutions"] == [{"importUri": child, "originalUri": child,
                                         "resolvedUri": child, "sourceUri": child}], "SPI identity contract differs")
        require(len(case["sources"]) == 1, "source bundle differs")
        source = case["sources"][0]
        require(source["text"] == original["child"] and source["sourceSha256"] == original["childSha256"]
                and raw(source["sourceFile"]) == original["child"].encode()
                and digest(raw(source["sourceFile"])) == original["childSha256"]
                and source["resolvedUri"] == source["sourceUri"] == child
                and source["inferredName"] == child.rsplit("/", 1)[-1], "parent source contract differs")
        require(source.get("callbackPaths", []) == ([len(original["expressions"])] if callback else [])
                and source.get("callbackTimes", 1) == 1, "source callback contract differs")
        require(case["provenance"]["mode"] == original.get("mode", "plain"), "oracle reader mode differs")
        for repeat, name in enumerate(case["provenance"]["processFiles"]):
            process = json.loads(raw(name))
            record = next(r for r in reports[key]["records"]
                          if r["id"] == original["id"] and r["repeat"] == repeat)
            require(process == record and process["moduleUri"] == case["uri"]
                    and process["sourceSha256"] == case["sourceSha256"]
                    and process["result"]["exit_code"] == 0
                    and not process["result"]["timed_out"], "actual Java process/context differs")
        prefix = "origins/" + key + "/cases/" + original["id"] + "/"

        def result(row, call, expression):
            nonlocal payload_count
            require(len(row["payloadOrigin"]) == len(set(row["payloadOrigin"])), "duplicate expected payload")
            expected_paths = []
            for repeat in range(2):
                pre = prefix + str(repeat) + "/calls/" + call
                status = raw(pre + ".status.bin")
                require(status in [b"success\n", b"error\n"], "nonordinary Java API outcome")
                success = status == b"success\n"
                channel = "stdout" if success else "stderr"
                expected_paths.extend(pre + suffix for suffix in [".expression.bin", ".status.bin", ".stdout.bin",
                                      ".stderr.bin", ".error-class.bin", "." + channel + ".code-units.bin"])
                require(raw(pre + ".expression.bin") == expression.encode(), "actual Java expression differs")
                require(row["kind"] == ("value" if success else "error")
                        and row["javaStatus"] == ("success" if success else "error")
                        and row["javaErrorClass"] == raw(pre + ".error-class.bin").decode()
                        and bytes.fromhex(row["javaUtf8Hex"]) == raw(pre + "." + channel + ".bin")
                        and row["javaValueUnits" if success else "javaErrorUnits"]
                        == units(raw(pre + "." + channel + ".code-units.bin")), "Java result channels differ")
            require(set(row["payloadOrigin"]) == set(expected_paths), "missing/replaced Java expected binding")
            payload_count += len(expected_paths)
        expected = case["expected"]
        require(len(expected["results"]) == len(original["expressions"]), "missing expected outer API call")
        for i, (row, expression) in enumerate(zip(expected["results"], original["expressions"])):
            require(row["actionIndex"] == i, "reordered expected API result")
            result(row, "outer-" + str(i), expression)
        require(len(expected["callbacks"]) == bool(callback), "missing actual Java callback")
        if callback:
            require(expected["callbacks"][0]["path"] == len(original["expressions"]), "callback entry differs")
            result(expected["callbacks"][0], "inner-0", callback)
        counters = dict(line.split("=") for line in raw(prefix + "0/calls/counters.bin").decode().splitlines())
        require(expected["requestCounts"] == {"resolve": int(counters["resolve"]), "load-source": int(counters["source"])},
                "actual source/resolve count differs")
        require(not expected["logs"] and not any(path.startswith(prefix) and path.endswith(".level.bin")
                                                for path in bindings), "Java log coverage differs")
        require(case["oracleError"] == any(row["javaErrorClass"] == "org.pkl.core.PklBugException"
                                           for row in expected["results"] + expected["callbacks"]), "oracle error marker differs")
    return {"scope": "Offline actual Java inheritance API fixture integrity; no candidate evaluation.",
            "fixtures": len(data["cases"]), "javaProcesses": sum(r["processCount"] for r in reports.values()),
            "expectedChannelBindings": payload_count, "primaryIdentity": "Exact ModuleSource.path input only; no invented observed key table."}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify_package(args.fixtures), indent=2))


if __name__ == "__main__":
    main()
