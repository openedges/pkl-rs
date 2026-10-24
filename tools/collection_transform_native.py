#!/usr/bin/env python3
"""Acquire native collection observations from an authenticated frozen build.

Java fixtures remain the sole expected-result authority. Acquisition integrity,
ordinary compatibility and full Java lifecycle compatibility are separate gates.
"""
import argparse
import collections
import datetime
import hashlib
import json
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=True, indent=2) + "\n")


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def field(folder, name):
    raw = (folder / (name + ".bin")).read_bytes()
    binary = (folder / (name + ".utf16be.bin")).read_bytes()
    need(len(binary) % 2 == 0, "odd UTF16 payload: " + name)
    text = binary.decode("utf-16-be", "surrogatepass")
    need(text.encode("utf-8", "replace") == raw, "raw UTF8/UTF16 differs: " + name)
    return text, [int.from_bytes(binary[i:i + 2], "big") for i in range(0, len(binary), 2)]


def number(folder, name):
    text = field(folder, name)[0]
    need(text.isascii() and text.isdigit() and str(int(text)) == text,
         "noncanonical counter: " + name)
    return int(text)


def actual_session(fixture, folder):
    entries = list(folder.iterdir())
    need(all(p.is_file() and not p.is_symlink() for p in entries), "nonregular raw entry")
    actual_files = {p.name for p in entries}
    consumed = set()

    def read(name):
        consumed.update([name + ".bin", name + ".utf16be.bin"])
        return field(folder, name)

    def count(name):
        text = read(name)[0]
        need(text.isascii() and text.isdigit() and str(int(text)) == text,
             "noncanonical counter: " + name)
        return int(text)

    def opaque(name):
        consumed.add(name)
        return (folder / name).read_bytes().hex()

    calls, effects, requests, resource_policy_requests = [], [], [], []
    retained = 0
    for expected in fixture["expected"]:
        index = expected["index"]
        prefix = f"call-{index}"
        status = read(prefix + ".status")[0]
        statuses = {"success\n": "success", "error\n": "error", "api-error\n": "api-error"}
        need(status in statuses, "unknown raw native status")
        call = {"index": index, "expression": read(prefix + ".expression")[0],
                "status": statuses[status], "disposition": read(prefix + ".disposition")[0] or None,
                "phase": read(prefix + ".phase")[0] or None,
                "stdoutUnits": read(prefix + ".stdout")[1],
                "stderrUnits": read(prefix + ".stderr")[1]}
        for key, suffix in [("readsBefore", "reads-before"), ("readsAfter", "reads-after"),
                            ("logsBefore", "logs-before"), ("logsAfter", "logs-after")]:
            call[key] = count(prefix + "." + suffix)
        if call["status"] != "api-error":
            channel = "stdout" if call["status"] == "success" else "stderr"
            need(read(f"retained-{retained}")[1] == call[channel + "Units"],
                 "owned payload changed after evaluator close")
            retained += 1
        else:
            # The capture adapter emits this Debug classification as opaque
            # bytes, separately from the lossless public error message.
            call["apiErrorKindRawHex"] = opaque(prefix + ".api-error-kind.bin")
        calls.append(call)
    need(count("calls") == len(calls), "native call count differs")
    need(count("retained-output-buffers") == retained, "owned payload count differs")
    for index in range(count("effects")):
        prefix = f"effect-{index}"
        kind = read(prefix + ".kind")[0]
        effect = {"callIndex": count(prefix + ".call-index"), "kind": kind}
        need(effect["callIndex"] < len(calls), "effect references absent API call")
        need(not effects or effects[-1]["callIndex"] <= effect["callIndex"],
             "effect API call order differs")
        if kind == "read":
            result = read(prefix + ".result")[0]
            need(result in ["text", "io"], "unknown reader result")
            effect.update(uri=read(prefix + ".uri")[0],
                          attempt=count(prefix + ".attempt"))
            effect["result"] = ({"kind": result, "units": read(prefix + ".value")[1]}
                                if result == "text" else
                                {"kind": result, "message": read(prefix + ".message")[0]})
        else:
            need(kind == "log", "unknown effect")
            nullable = read(prefix + ".member-null")[0]
            need(nullable in ["true", "false"], "invalid member null flag")
            position = read(prefix + ".position")[0].split(":")
            need(len(position) == 4 and all(v.isascii() and v.isdigit() and str(int(v)) == v
                                            for v in position), "invalid logger position")
            member = read(prefix + ".member")[1]
            need(nullable != "true" or member == [], "null member has data")
            frame = {"moduleUri": read(prefix + ".module-uri")[1],
                     "memberName": None if nullable == "true" else member,
                     "sourceLines": [read(prefix + f".source-{i}")[1]
                                     for i in range(count(prefix + ".source-lines"))]}
            frame.update(zip(["startLine", "startColumn", "endLine", "endColumn"], map(int, position)))
            effect.update(level=read(prefix + ".level")[0],
                          message=read(prefix + ".message")[1], frame=frame)
        effects.append(effect)
    reads, logs = 0, 0
    for call in calls:
        need(call["readsBefore"] == reads and call["logsBefore"] == logs, "counter continuity differs")
        reads += sum(e["kind"] == "read" and e["callIndex"] == call["index"] for e in effects)
        logs += sum(e["kind"] == "log" and e["callIndex"] == call["index"] for e in effects)
        need(call["readsAfter"] == reads and call["logsAfter"] == logs, "counter/effect join differs")
    need(count("reads") == reads and count("logs") == logs, "terminal effect counters differ")
    resource_tokens = set()
    for index in range(count("resource-policy-requests")):
        prefix = f"resource-policy-{index}"
        call_index = count(prefix + ".call-index")
        need(call_index < len(calls), "resource policy references absent API call")
        need(not resource_policy_requests or resource_policy_requests[-1]["callIndex"] <= call_index,
             "resource policy API call order differs")
        uri = read(prefix + ".uri")[0]
        allowed = read(prefix + ".allowed")[0]
        message = read(prefix + ".message")[0]
        token = read(prefix + ".token")[0]
        need(allowed in ["true", "false"], "invalid resource policy decision")
        allowed = allowed == "true"
        # The pinned Java helper configures Pattern.compile("ct:.*"). Its
        # SecurityManager uses lookingAt(), not whole-string matching.
        need(allowed == uri.startswith("ct:"), "resource policy differs from configured allowlist")
        need(bool(token) and (message == "" if allowed else bool(message)),
             "invalid resource policy payload")
        need(token not in resource_tokens, "duplicate resource policy token")
        resource_tokens.add(token)
        resource_policy_requests.append({"callIndex": call_index, "uri": uri,
                                         "allowed": allowed, "message": message, "token": token})
    need([(p["callIndex"], p["uri"]) for p in resource_policy_requests if p["allowed"]]
         == [(e["callIndex"], e["uri"]) for e in effects if e["kind"] == "read"],
         "resource policy/read effect join differs")
    for index in range(count("module-requests")):
        requests.append({"index": index,
                         "requestRawHex": opaque(f"module-{index}.request.bin"),
                         "tokenRawHex": opaque(f"module-{index}.token.bin")})
    need(len(list(folder.glob("module-*.request.bin"))) == len(requests)
         and len(list(folder.glob("module-*.token.bin"))) == len(requests), "module request ledger count differs")
    result = {"id": fixture["id"], "calls": calls, "effects": effects, "moduleRequests": requests,
            "resourcePolicyRequests": resource_policy_requests,
            "resourcePolicyScope": "Actual Host decisions for the pinned ct:.* allowlist; not Java administrative event equality.",
            "moduleRequestScope": "Actual Rust Debug request bytes; not Java administrative event equality.",
            "acquisitionError": None, "cleanupErrors": [],
            "architecture": {"collections": count("collections"),
                             "terminalSlots": count("terminal-slots"),
                             "ownedResultsStable": True,
                             "providerEntriesReleased": count("provider-entries") == 0}}

    need(actual_files == consumed, "unconsumed raw records: " + repr(sorted(actual_files - consumed)))
    return result


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--fixtures", type=Path, required=True)
    parser.add_argument("--build-proof", type=Path, required=True)
    parser.add_argument("--build-proof-sha256", required=True)
    parser.add_argument("--node", type=Path, required=True)
    args = parser.parse_args()
    out, fixture_root = args.output.resolve(), args.fixtures.resolve()
    out.mkdir(parents=True, exist_ok=False)
    inputs = {}

    def pin(path):
        path = path.absolute()
        need(path.is_file() and not path.is_symlink(), "regular input required: " + str(path))
        digest = sha(path)
        need(str(path) not in inputs or inputs[str(path)] == digest, "input drift: " + str(path))
        inputs[str(path)] = digest
        return path

    def execute(name, argv):
        row = {"name": name, "argv": [str(v) for v in argv], "startedUtc": now()}
        with (out / (name + ".stdout.bin")).open("wb") as stdout, (out / (name + ".stderr.bin")).open("wb") as stderr:
            child = subprocess.Popen(row["argv"], cwd=ROOT, stdout=stdout, stderr=stderr)
            row["pid"], row["exitCode"] = child.pid, child.wait()
        row["finishedUtc"] = now()
        report["processes"].append(row)
        save(out / "report.json", report)
        return row

    report = {"schema": 1, "scope": "Native frozen-artifact collection acquisition. Ordinary, diagnostic-only and post-close API scopes remain separate.",
              "startedUtc": now(), "cwd": str(ROOT), "host": platform.platform(), "argv": sys.argv,
              "processes": [], "complete": False, "fullJavaEventsCountersPassed": False,
              "fullCompatibilityPassed": False}
    save(out / "report.json", report)
    try:
        pin(Path(__file__).resolve())
        python, node = pin(Path(sys.executable).resolve()), pin(args.node.resolve())
        report["pythonExecutable"], report["nodeExecutable"] = str(python), str(node)
        version = execute("node-version", [node, "--version"])
        need(version["exitCode"] == 0, "Node version command failed")
        need(sha(pin(args.build_proof)) == args.build_proof_sha256, "build proof pin differs")
        proof = json.loads(args.build_proof.read_bytes())
        need(type(proof["schema"]) is int and proof["schema"] == 1 and proof["status"] == "CLOSED"
             and proof["sourceStable"] is True and proof["sources"],
             "build proof is not closed/stable")
        for rel, digest in proof["sources"].items():
            need(not Path(rel).is_absolute() and ".." not in Path(rel).parts, "unsafe source snapshot path")
            need(sha(pin(Path(proof["snapshotRoot"]) / rel)) == digest, "source snapshot differs")
        native = proof["artifacts"]["native"]
        artifact = pin(Path(native["path"]))
        need(sha(artifact) == native["sha256"], "native artifact differs")
        verifier = pin(ROOT / "tools/collection_transform_fixtures.py")
        producer = out / "producer"
        producer.mkdir()
        for source in [Path(__file__).resolve(), verifier, args.build_proof.absolute()]:
            shutil.copy2(source, producer / source.name)
            pin(producer / source.name)
        preflight = execute("preflight", [python, "-B", verifier, "--fixtures", fixture_root])
        need(preflight["exitCode"] == 0, "Java fixture verification failed")
        manifest = json.loads(pin(fixture_root / "manifest.json").read_bytes())
        for rel, row in manifest["files"].items():
            need(not Path(rel).is_absolute() and ".." not in Path(rel).parts, "unsafe fixture path")
            path = pin(fixture_root / rel)
            need(path.stat().st_size == row["bytes"] and sha(path) == row["sha256"], "fixture payload differs")
        fixtures = json.loads((fixture_root / "fixtures.json").read_bytes())["fixtures"]
        need(fixtures and len({f["id"] for f in fixtures}) == len(fixtures), "empty/duplicate session identities")
        helpers = out / "helpers"
        helpers.mkdir()
        for name in ["collection-transform-session.mjs", "resource-session.mjs", "module-policy-session.mjs"]:
            shutil.copy2(pin(ROOT / "tools/platform" / name), helpers / name)
            pin(helpers / name)
        shutil.copy2(pin(ROOT / "tools/platform/compare-collection-transform-native.mjs"), out / "compare.mjs")
        pin(out / "compare.mjs")
        save(out / "fixtures.json", fixtures)
        pin(out / "fixtures.json")
        save(out / "inputs-before.json", inputs)
        jobs = collections.defaultdict(list)
        for fixture in fixtures:
            jobs[(fixture["oracle"], fixture["caseId"], fixture["repeat"])].append(fixture)
        actual = {}
        for index, (key, members) in enumerate(jobs.items()):
            need({f["session"] for f in members} == {0, 1}, "expected two native evaluator sessions")
            first = members[0]
            for member in members[1:]:
                need(all(member[k] == first[k] for k in ["source", "sourceSha256", "uri", "moduleName", "operations", "readerRules"]),
                     "sessions do not share original process inputs")
            source = fixture_root / first["sourceProvenance"]["sourcePath"]
            case = out / "cases" / str(index)
            case.mkdir(parents=True)
            row = execute(f"native-{index}", [artifact, source, first["uri"], first["moduleName"],
                          source.parent / "operations.tsv", source.parent / "reader-plan.tsv", case / "raw"])
            row["oracleCaseRepeat"] = key
            for fixture in members:
                try:
                    need(row["exitCode"] == 0, "native child failed")
                    actual[fixture["id"]] = actual_session(fixture, case / "raw" / f"session-{fixture['session']}")
                except Exception as error:
                    actual[fixture["id"]] = {"id": fixture["id"], "calls": [], "effects": [],
                        "acquisitionError": repr(error), "cleanupErrors": [], "architecture": {
                            "terminalSlots": None, "ownedResultsStable": False, "providerEntriesReleased": False}}
            print("NATIVE", *key, row["exitCode"], flush=True)
        save(out / "actual.json", [actual[f["id"]] for f in fixtures])
        pin(out / "actual.json")
        compared = execute("comparison", [node, out / "compare.mjs"])
        need(compared["exitCode"] == 0, "comparison did not finish")
        report["comparison"] = json.loads((out / "comparison.json").read_bytes())
        report["changedInputs"] = [name for name, digest in inputs.items() if not Path(name).is_file() or sha(Path(name)) != digest]
        report["complete"] = True
        report["acquisitionIntegrityPassed"] = not report["changedInputs"] and not report["comparison"]["acquisitionFailures"]
        report["ordinaryContractPassed"] = report["acquisitionIntegrityPassed"] and report["comparison"]["ordinaryContractPassed"]
    except Exception as error:
        report["infrastructureError"] = repr(error)
        report["acquisitionIntegrityPassed"] = False
    finally:
        save(out / "inputs-final.json", inputs)
        # Bind actual raw files as well as failed attempts; never derive golden bytes here.
        payloads = {str(p.relative_to(out)): {"sha256": sha(p), "bytes": p.stat().st_size}
                    for p in sorted(out.rglob("*")) if p.is_file() and p not in [out / "report.json", out / "payloads.json"]}
        save(out / "payloads.json", {"schema": 1, "files": payloads})
        report["payloadManifestSha256"] = sha(out / "payloads.json")
        report["finishedUtc"] = now()
        save(out / "report.json", report)
    print(json.dumps({k: report.get(k) for k in ["complete", "acquisitionIntegrityPassed", "ordinaryContractPassed", "infrastructureError"]}))
    return 0 if report.get("acquisitionIntegrityPassed") else 1


if __name__ == "__main__":
    raise SystemExit(main())
