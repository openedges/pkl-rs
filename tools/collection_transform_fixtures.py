#!/usr/bin/env python3
"""Export and hermetically authenticate actual Pkl 0.32.0 Collection transform observations.

The pinned Java inventories and reports are the authority. This tool never runs an
 evaluator and never obtains expected output from Rust. Container JSON is defined
here; every Java output/error/effect remains an independent raw byte/unit value.
"""
import argparse
import collections
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat

PINS = {'java01': {'inventory.json': 'cf8dba308017f8f7a3502f3f8829de82aef1cef7f6b1bca70234a9586b35d6d8', 'closure.json': 'e4b9302b59bea8c51812cae1b71377c1fadc079c61529de5747aa59fc173b44b', 'report.json': '1ba9bf7e62e69926f89122d8f6c2826b64046b5e05d60716bebbdf56ca770fea', 'cases.json': '791fac0471a844acd459e2e514b0701d0c8c5be27178a48fdf975f4db675f72c', 'profile.json': 'de7618e5d93ab8fff8eaa67925610ecefebe2eaa6fd42f4ec48046194819bd5c', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': 'd158f49d1b85187193c66b2c1237ec431ffdf5de0a87e2bdf1afebf3f0991715'}, 'java02': {'inventory.json': '0df8fbef504d76b4e5be6bfe8397b02ff58ecf6efd2acc17808e6554d013cc34', 'closure.json': '80ee5cea1a5d69a78bee40376416d8bf6b055b03fb6dcf36e062a70b87c862d0', 'report.json': '3af56b4db46658204016e7c0efccc226d72cfd241b938d56706e489899f2e176', 'cases.json': 'b3c87f3dcc960848641eb04dd63c26f5e6d379f05dc24779cae62712f7737ee5', 'profile.json': '3352a1889af7ea27586438be3d4b79117a669e3a47beb378c21d693db33fb24e', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': 'd9e6cd85ec086d21b5c49125a9a179bebc0cdcea73f06a7f41040fbd72903df6'}, 'java03': {'inventory.json': 'b55899624972df3e4fe172372c65bd9bcffed3a1751daef5a234d63ef31a0d3c', 'closure.json': 'd392587941007bc68e31131c666aeb8b427af666ea4dd6e04e3a4318a34ca467', 'report.json': 'eddb1086d957ddf84b196741ca6fccc328aeaa4e6ca602ab2927f41c138d0cd0', 'cases.json': '4d239ea50e90a0a9c10261199640134d57562d06f8aa97a52ca49b6285f0e6cf', 'profile.json': '5fcdc1148fb929eeecaf4d0044a17d6d8f4a33a1f0cb1ee5d386d2668e1df3dd', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': '4c4338dfaf482b77ed7e5ddfd4b6ce04a26694d81b1f70730b4fb87a7943153a'}, 'java04': {'inventory.json': '68f63fadcc2c78b81c60aa6c65f744ecae9372ca4fcc1b0498212d2511148ee3', 'closure.json': 'f05348a3e0a369d59cfcfdbfb6b2d7e9151216465f763494bb43d8252ee12aa0', 'report.json': 'beb91d3116abffd931657ed315336a8809f55645fd8d6d0dd5d7628e32b7d79f', 'cases.json': '6381ee6fe8b53f4c3bb5269dec97d2a150a1235753939da310b3b9fd712fcb9e', 'profile.json': '61a187a785101067e994eae737a29ffdb7194d2ccff54e921dc8ca82baf5ecda', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': '248de0ed04ff3c0022fc8c41f12676990e8bc355d6944fbd70ea83761a96877e'}, 'java05': {'inventory.json': 'de23d2b87f795468387a969837784a5ec0cb640763c8e0e71945b81afe11d6cc', 'closure.json': 'e5116ae08f513913c0bf8ea31da8c33a01eb5aa2e146ad6ec7cea441e9b64d4c', 'report.json': '039e4762fdc7923fd1fda5ebddc09abcfa75e764a1bd6c2f80fd24f8016a808a', 'cases.json': 'ce94ce213bcce016e77166703fe7a4374ba3607891f38b9cef6f87a391044b3a', 'profile.json': '7c03a61c701f9b16784f0dae130a7714dd4488880c70539714d3da6c4b32ccd1', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': 'db5cd9477a6bc7c2e1363596f5cdae1166558c4d0ba428b1fc9d35ebf5770f7a'}, 'java06': {'inventory.json': '25ffa295ab7f2721146600b66124510ca75c49e056004ce4cdcb3c58bdacfb43', 'closure.json': 'a8e793705a91cddd3b4ef4c30c739f14f829a374e9f8a36018f779a03b592aa1', 'report.json': '8c8d796eefbb210b3de52ea0dc7fcfc4647400d7f35831ebbd197a5b56bce44d', 'cases.json': '4494727e250a7708c1d7288f96370eb8eea00757e32f65f15dea0b523b94fdc0', 'profile.json': 'ff30979ebe428b2be0172758dfd1ffafd2535e20074b26877b0e09ea092200f5', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': '2c1991b424423b9141a79edab3e3ec94dcf6f48c1d83d280e9944b28cb4f1167'}, 'java07': {'inventory.json': 'cc97986eb16c363ae90f45a8ab3744b8e6d9f6b410b69865944836e99a739ac2', 'closure.json': '6f91136ae117ada78fead17c4297aee151494861a1d52d63f26c032ecf6d057d', 'report.json': '73d9ff7836a9b5cb97889278a6b2719a34eb470776820ce26f2787ad09ec7176', 'cases.json': '17d365d24a449bb0ccf2ed9e351d66896657c73c3432f2b78600e3f80924ef6e', 'profile.json': '00a7676fe604aee507e6dcb6efd42e47da9ff3134090426db6bb69d4b7ff9954', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': 'a9be6ef3200685c5ee4814289ee3f02470789fbfeae8d2d55d633df74c3cf1ff'}, 'java08': {'inventory.json': 'b84c117ef96eab5b64d2ee0b5a93e97c1926fe55f6facfb2092fadea4578550e', 'closure.json': 'a8657bc797719cca1ff9ad84cd0c1d5ccdd79687233586d2958f4fc4a449cf36', 'report.json': '5a4afe4c9033c7301b6a33e87093704432d03aa698fce43a2605412b62152ade', 'cases.json': 'ec5affcd1858b54ee97612598b471f624e6543ffdbbcb92f6d1a73cc7529bdb9', 'profile.json': 'ffb508a7479ae4ab9e45f8285ee7e95b5d2aabbc2b036b97265c561653d7bfac', 'CollectionProbe.java': 'f5590269aa09fac28b44bac73550de77f91277b20936faf0eaa1d65ea159ce77', 'handoff.json': '0e87e342cc7200d54856ec25d59692eb2ef536027e7cc99ce5cc442a44306012'}}
LICENSE_SHA = "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"
NOTICE_SHA = "d03b8db1a0d36ba65550ba30a51c7897f4b48b0eae7582d3f0576ea41e23d48e"
README = """# Collection transform Java observations

This fixture preserves pinned Pkl 0.32.0 Java API observations and original
source/effect provenance. It is not a Rust or full standard-library compatibility
result. Intent failures and post-close public API boundaries remain separate.
Verify with `python3 -B tools/collection_transform_fixtures.py --fixtures
tests/fixtures/m2-collection-transforms` from the repository root.

The inherited upstream source files retain their original copyright headers.
See LICENSE.txt and NOTICE.txt for the pinned upstream license and notices.
"""
DERIVED = {"fixtures.json", "provenance-map.json", "README.md", "LICENSE.txt", "NOTICE.txt"}


def need(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def json_bytes(value):
    return (json.dumps(value, ensure_ascii=False, indent=2) + "\n").encode("utf-8")


def parse(data):
    def unique(pairs):
        value = {}
        for key, item in pairs:
            need(key not in value, "duplicate JSON key")
            value[key] = item
        return value
    return json.loads(data, object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError("nonfinite JSON")))


def relative(value):
    need(isinstance(value, str) and value and "\\" not in value, "invalid relative path")
    p = PurePosixPath(value)
    need(not p.is_absolute() and ".." not in p.parts and p.as_posix() == value,
         "unsafe or noncanonical relative path")
    return value


class Store:
    def __init__(self, root):
        self.root = Path(root).absolute()
        need(self.root.is_dir() and not self.root.is_symlink(), "fixture root must be a real directory")
        self.cache = {}

    def read(self, name):
        relative(name)
        if name not in self.cache:
            path = self.root
            for part in PurePosixPath(name).parts:
                path = path / part
                need(not path.is_symlink(), "symlink in declared payload path")
            need(stat.S_ISREG(path.stat().st_mode), "declared payload must be regular")
            self.cache[name] = path.read_bytes()
        return self.cache[name]


def units(data):
    need(len(data) % 2 == 0, "odd Java UTF16 payload")
    return [int.from_bytes(data[n:n + 2], "big") for n in range(0, len(data), 2)]


def number(text):
    need(re.fullmatch(r"0|[1-9][0-9]*", text) is not None, "noncanonical unsigned counter")
    return int(text)


def authorities(store):
    required = set(DERIVED)
    inventories = {}
    for group, pins in PINS.items():
        prefix = "provenance/" + group + "/"
        # Authenticate literal authority pins before deriving paths or walking.
        for name, expected in pins.items():
            need(digest(store.read(prefix + name)) == expected, "pinned authority differs: " + name)
        inv = parse(store.read(prefix + "inventory.json"))
        closure = parse(store.read(prefix + "closure.json"))
        need(closure["inventorySha256"] == pins["inventory.json"] and closure["sealed"] is True,
             "producer closure/inventory link differs")
        need(closure["handoffSha256"] == pins["handoff.json"], "producer handoff link differs")
        rows = inv["files"]
        need(isinstance(rows, dict) and rows, "empty producer inventory")
        for rel, row in rows.items():
            relative(rel)
            name = prefix + rel
            data = store.read(name)
            need(len(data) == row["bytes"] and digest(data) == row["sha256"],
                 "pinned raw/source payload differs: " + rel)
            required.add(name)
        required.update(prefix + name for name in pins)
        inventories[group] = rows
    need(digest(store.read("LICENSE.txt")) == LICENSE_SHA, "upstream license differs")
    need(digest(store.read("NOTICE.txt")) == NOTICE_SHA, "upstream notice differs")
    need(store.read("README.md") == README.encode(), "fixture README differs")
    return required, inventories


def scope(group, case, index, status):
    if group == "java07" and case == "duration-type-render" and index == 9:
        return "intent-not-reached"
    if group == "java04" and case == "null-amendment-and-nonnull":
        return "intent-not-reached"
    if group == "java01" and (case == "catch-boundaries" or
            (case == "callback-types-first-error" and index in [1, 2, 3, 4])):
        return "intent-not-reached"
    if group == "java02" and case == "catch-boundaries-import-first" and index in [1, 2, 3, 4]:
        return "intent-not-reached"
    if status == "api-error\n":
        return "public-api-boundary"
    return "ordinary"


def construct(store):
    fixtures, mapping, ledger = [], {}, collections.Counter()
    events_total = 0
    for group, pins in PINS.items():
        prefix = "provenance/" + group + "/"
        report = parse(store.read(prefix + "report.json"))
        cases_list = parse(store.read(prefix + "cases.json"))
        cases = {c["id"]: c for c in cases_list}
        need(len(cases) == len(cases_list), "duplicate Java case")
        profile = parse(store.read(prefix + "profile.json"))
        need(profile["oracleVersion"] == "0.32.0" and profile["powerAssertions"] is False,
             "oracle version/power profile differs")
        need(profile["oracleJarSha256"] == "c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059",
             "oracle jar differs")
        need(report["sourceInputChanges"] == [], "producer inputs changed")
        records = report["processes"]
        need({(r["case"], r["repeat"]) for r in records} ==
             {(case, repeat) for case in cases for repeat in range(2)} and len(records) == len(cases) * 2,
             "missing/duplicate Java process record")
        for record in records:
            need(record["exitCode"] == 0 and record["timedOut"] is False, "unexecuted Java process")
            case = cases[record["case"]]
            workspace = "workspace/" + relative(case["id"]) + "/"
            source = store.read(prefix + workspace + "input.pkl")
            operations = case["operations"]
            ops_bytes = "".join(op["kind"] + "\t" + op.get("value", "").encode().hex() + "\n"
                                for op in operations).encode()
            need(store.read(prefix + workspace + "operations.tsv") == ops_bytes, "Java operations differ")
            rules = case["readerPlan"]
            reader_bytes = "".join(r["uri"] + "\t" + r["kind"] + "\t" + r["value"].encode().hex() + "\n"
                                   for r in rules).encode()
            need(store.read(prefix + workspace + "reader-plan.tsv") == reader_bytes, "Java reader inputs differ")
            need(all(op["kind"] in ["evaluate", "gc", "close"] for op in operations), "unknown Java operation")
            need(all(op.get("value", "") == "" for op in operations if op["kind"] != "evaluate"),
                 "nonempty GC/close operand")
            expression_ops = [op["value"] for op in operations if op["kind"] == "evaluate"]
            need(len({r["uri"] for r in rules}) == len(rules) and
                 all(r["kind"] in ["text", "io-first", "io"] for r in rules), "invalid reader rule table")
            original_root = PurePosixPath(case["workspace"]).parent.parent
            for session in range(2):
                raw = prefix + "cases/" + case["id"] + "/" + str(record["repeat"]) + "/raw/session-" + str(session)

                def read(field):
                    return store.read(raw + "/" + field + ".bin")

                def string(field):
                    java_bytes, utf16 = read(field), read(field + ".utf16be")
                    need(utf16.decode("utf-16-be", "surrogatepass").encode("utf-8", "replace") == java_bytes,
                         "independent Java UTF8/UTF16 differs")
                    return {"units": units(utf16), "javaUtf8Hex": java_bytes.hex()}

                totals = [number(x) for x in read("session-counts").decode().split("\t")]
                need(len(totals) == 4, "session counter shape")
                events, effects, plan, attempts = [], [], [], collections.Counter()
                reader_by_uri = {r["uri"]: r for r in rules}
                for index in range(totals[3]):
                    e = "event-" + str(index)
                    kind = read(e + ".kind").decode()
                    ci = number(read(e + ".call-index").decode())
                    event = {"javaIndex": index, "callIndex": ci, "kind": kind, "rawPrefix": raw + "/" + e}
                    need(kind in ["logger", "reader-call", "reader-close", "evaluator-created", "api-call", "api-result",
                                  "gc-request", "close-call", "close-return", "final-close-call", "final-close-return"],
                         "unknown actual Java event kind")
                    events.append(event)
                    if kind == "logger":
                        null = read(e + ".member-null").decode()
                        need(null in ["true", "false"], "invalid nullable member flag")
                        frame = {"moduleUri": string(e + ".module-uri"), "memberName": string(e + ".member"),
                                 "memberNameNull": null == "true"}
                        pos = [number(x) for x in read(e + ".position").decode().split(":")]
                        need(len(pos) == 4, "trace frame position shape")
                        frame.update(zip(["startLine", "startColumn", "endLine", "endColumn"], pos))
                        frame["sourceLines"] = [string(e + ".source-" + str(n))
                                                for n in range(number(read(e + ".source-lines").decode()))]
                        level = read(e + ".level").decode()
                        need(level in ["trace", "warn"], "unknown Java logger level")
                        effects.append({**event, "kind": "log", "level": level,
                                        "message": string(e + ".message"), "frame": frame})
                    elif kind == "reader-call":
                        uri = read(e + ".uri").decode()
                        need(uri in reader_by_uri, "unconfigured actual reader URI")
                        rule = reader_by_uri[uri]
                        attempts[uri] += 1
                        attempt = number(read(e + ".attempt").decode())
                        need(attempt == attempts[uri], "reader attempt differs")
                        io = rule["kind"] == "io" or (rule["kind"] == "io-first" and attempt == 1)
                        need(read(e + ".result") == (b"io" if io else b"text"), "source-derived reader kind differs")
                        if io:
                            need(read(e + ".message") == rule["value"].encode(), "reader IOException differs")
                            result = {"kind": "io", "message": rule["value"]}
                        else:
                            value = "recovered" if rule["kind"] == "io-first" else rule["value"]
                            need(read(e + ".value") == value.encode(), "source-derived reader value differs")
                            result = {"kind": "text", "units": string(e + ".value")["units"]}
                        effects.append({**event, "kind": "read", "uri": uri, "attempt": attempt, "result": result})
                        plan.append({"uri": uri, "result": result, "javaEventIndex": index})
                events_total += len(events)
                calls = [c for c in record["calls"] if c["session"] == session]
                need(len(calls) == len(expression_ops) == totals[0], "missing/extra API call")
                expected = []
                for index, (call, expression) in enumerate(zip(calls, expression_ops)):
                    c = "call-" + str(index)
                    need(call["index"] == index and call["expression"] == expression and
                         read(c + ".expression") == expression.encode(), "expression/order differs")
                    status = read(c + ".status").decode()
                    statuses = {"success\n": "success", "error\n": "error", "api-error\n": "api-error", "oracle-error\n": "oracle-error"}
                    need(status in statuses and status == call["status"] and read(c + ".status").hex() == call["statusHex"],
                         "raw status differs")
                    out, err = string(c + ".stdout"), string(c + ".stderr")
                    need(out["javaUtf8Hex"] == call["stdoutHex"] and err["javaUtf8Hex"] == call["stderrHex"] and
                         read(c + ".stdout.utf16be").hex() == call["stdoutUTF16BEHex"] and
                         read(c + ".stderr.utf16be").hex() == call["stderrUTF16BEHex"], "report/raw API channels differ")
                    error_class = read(c + ".error-class").decode()
                    null = read(c + ".message-null").decode()
                    need(error_class == call["errorClass"] and null == call["messageNull"] and
                         null in ["true", "false"], "error class/null flag differs")
                    need((status == "success\n" and error_class == "") or
                         (status == "error\n" and error_class == "org.pkl.core.PklException") or
                         (status == "api-error\n" and error_class == "java.util.concurrent.RejectedExecutionException"),
                         "unrecognized Java classification")
                    before, after = call["countsBefore"], call["countsAfter"]
                    need(all(type(n) is int and n >= 0 for n in before + after) and len(before) == len(after) == 3,
                         "counter shape")
                    need(read(c + ".counts-before") == "\t".join(map(str, before)).encode() and
                         read(c + ".counts-after") == "\t".join(map(str, after)).encode(), "report/raw counters differ")
                    for counter, end in [(before, before[2]), (after, after[2])]:
                        need(counter[:2] == [sum(e["kind"] == "reader-call" for e in events[:end]),
                                             sum(e["kind"] == "logger" for e in events[:end])], "actual event counters differ")
                    need(events[before[2]]["kind"] == "api-call" and events[after[2] - 1]["kind"] == "api-result" and
                         all(e["callIndex"] == index for e in events[before[2]:after[2]]), "API effect boundary differs")
                    rel_prefix = str(PurePosixPath(call["rawPrefix"]).relative_to(original_root))
                    need(prefix + rel_prefix == raw + "/" + c, "original report prefix/inventory join differs")
                    disposition_scope = scope(group, case["id"], index, status)
                    ledger[disposition_scope] += 1
                    expected.append({"index": index, "expression": expression, "scope": disposition_scope,
                                     "status": statuses[status], "statusRawHex": call["statusHex"],
                                     "stdoutUnits": out["units"], "stderrUnits": err["units"],
                                     "stdoutHex": out["javaUtf8Hex"], "stderrHex": err["javaUtf8Hex"],
                                     "oracleErrorClass": error_class, "oracleMessageNull": null == "true",
                                     "readsBefore": before[0], "readsAfter": after[0], "logsBefore": before[1], "logsAfter": after[1],
                                     "javaEventsBefore": before[2], "javaEventsAfter": after[2], "rawPrefix": raw + "/" + c})
                fixture_id = group + "/" + case["id"] + "/r" + str(record["repeat"]) + "/s" + str(session)
                fixture = {"id": fixture_id, "oracle": group, "caseId": case["id"], "repeat": record["repeat"], "session": session,
                           "source": source.decode(), "sourceUtf8Hex": source.hex(), "sourceSha256": digest(source),
                           "uri": case["sourceUri"], "moduleName": "input", "operations": operations, "expressions": expression_ops,
                           "readerRules": rules, "readerPlan": plan, "expected": expected, "expectedEffects": effects,
                           "javaEvents": events, "profile": {"entrypoint": "public-module-source-uri-v1",
                            "primaryProvider": "exact virtual cached source registry; no general File SPI parity",
                            "traceMode": "COMPACT", "hostLogger": True, "hostFrameTransformer": False,
                            "powerAssertions": False, "javaTimeoutSeconds": 8},
                           "sourceProvenance": {"sourcePath": prefix + workspace + "input.pkl",
                            "originalSourcePath": case["source"], "report": prefix + "report.json", "reportSha256": pins["report.json"],
                            "helperSha256": pins["CollectionProbe.java"]}}
                fixtures.append(fixture)
                mapping[fixture_id] = {"source": prefix + workspace + "input.pkl", "operations": prefix + workspace + "operations.tsv",
                                       "readerPlan": prefix + workspace + "reader-plan.tsv", "rawSession": raw,
                                       "report": prefix + "report.json", "reportSha256": pins["report.json"]}
        if group == "java01":
            original = store.read(prefix + "workspace/original-source-code/input.pkl")
            binding = parse(store.read(prefix + "original-binding.json"))
            need(digest(original) == binding["sha256"], "unchanged whole SourceCode source differs")
            result = store.read(prefix + "cases/original-source-code/0/raw/session-0/call-0.stdout.bin")
            for repeat in range(2):
                cli = prefix + "original-adapter/" + str(repeat) + "/"
                need(store.read(cli + "outcome.bin") == b"success\n" and store.read(cli + "stdout.bin") == result and
                     store.read(cli + "stderr.bin") == b"", "original adapter boundary differs")
    value = {"schema": 1, "fixtures": fixtures, "javaApiCalls": sum(ledger.values()), "scopeLedger": dict(ledger),
             "separateOriginalAdapterCalls": 2, "orderedJavaEvents": events_total,
             "observableProjectionScope": "Actual Logger trace/warn and configured ResourceReader.read deliveries only; no inferred callbacks/hash count or full Java administrative lifecycle parity.",
             "fullJavaAdministrativeLifecyclePassed": False,
             "unexecutedApiBoundaries": [{"kind": "public-api-boundary", "calls": ledger["public-api-boundary"],
                 "reason": "Post-close RejectedExecutionException object identity messages retained individually; ordinary native/portable equality not established."}]}
    return value, mapping


def exact_paths(store, required):
    allowed = {"."}
    for name in required:
        allowed.update(p.as_posix() for p in PurePosixPath(name).parents)
    found = set()
    todo = [store.root]
    while todo:
        current = todo.pop()
        with os.scandir(current) as entries:
            for entry in entries:
                p = Path(entry.path)
                rel = p.relative_to(store.root).as_posix()
                need(not entry.is_symlink(), "symlink in fixture tree")
                if entry.is_dir(follow_symlinks=False):
                    need(rel in allowed, "undeclared directory before descent: " + rel)
                    todo.append(p)
                else:
                    need(entry.is_file(follow_symlinks=False) and rel in required, "undeclared/nonregular fixture payload")
                    found.add(rel)
    need(found == required, "missing fixture payload coverage")


def verify(root):
    store = Store(root)
    required, _ = authorities(store)
    manifest = parse(store.read("manifest.json"))
    need(type(manifest.get("schema")) is int and manifest["schema"] == 1 and
         set(manifest) == {"schema", "files"} and isinstance(manifest["files"], dict), "manifest schema")
    need(set(manifest["files"]) == required, "exact manifest coverage differs")
    exact_paths(store, required | {"manifest.json"})
    for name, row in manifest["files"].items():
        data = store.read(name)
        need(isinstance(row, dict) and set(row) == {"bytes", "sha256"} and
             type(row["bytes"]) is int and row["bytes"] == len(data) and
             isinstance(row["sha256"], str) and re.fullmatch(r"[0-9a-f]{64}", row["sha256"]) is not None and
             row["sha256"] == digest(data),
             "manifest payload hash/length differs")
    fixture, mapping = construct(store)
    need(store.read("fixtures.json") == json_bytes(fixture), "derived API/effect fixture differs from pinned Java")
    need(store.read("provenance-map.json") == json_bytes(mapping), "provenance join differs")
    return {"schema": 1, "files": len(required) + 1, "sessions": len(fixture["fixtures"]),
            "javaApiCalls": fixture["javaApiCalls"], "scopeLedger": fixture["scopeLedger"],
            "orderedJavaEvents": fixture["orderedJavaEvents"], "manifestSha256": digest(store.read("manifest.json")),
            "fullJavaAdministrativeLifecyclePassed": False}


def export(destination, roots, license_path, notice_path):
    destination = Path(destination).absolute()
    need(not destination.exists(), "export destination must be fresh")
    need(set(roots) == set(PINS), "explicit oracle-root set must match authority cohorts")
    destination.mkdir(parents=True)
    for group, pins in PINS.items():
        source = Store(roots[group])
        for name, expected in pins.items():
            need(digest(source.read(name)) == expected, "export source authority differs")
        rows = parse(source.read("inventory.json"))["files"]
        for rel in set(rows) | set(pins):
            data = source.read(rel)
            if rel in rows:
                need(digest(data) == rows[rel]["sha256"] and len(data) == rows[rel]["bytes"], "export source payload differs")
            p = destination / "provenance" / group / relative(rel)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_bytes(data)
    for path, name, expected in [(license_path, "LICENSE.txt", LICENSE_SHA), (notice_path, "NOTICE.txt", NOTICE_SHA)]:
        data = Path(path).read_bytes()
        need(digest(data) == expected, "pinned upstream license/notice differs")
        (destination / name).write_bytes(data)
    (destination / "README.md").write_text(README)
    store = Store(destination)
    required, _ = authorities(store)
    fixture, mapping = construct(store)
    (destination / "fixtures.json").write_bytes(json_bytes(fixture))
    (destination / "provenance-map.json").write_bytes(json_bytes(mapping))
    rows = {name: {"sha256": digest((destination / name).read_bytes()), "bytes": (destination / name).stat().st_size}
            for name in sorted(required)}
    (destination / "manifest.json").write_bytes(json_bytes({"schema": 1, "files": rows}))
    return verify(destination)


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--fixtures", type=Path, default=Path(__file__).resolve().parents[1] / "tests/fixtures/m2-collection-transforms")
    p.add_argument("--export-to", type=Path)
    p.add_argument("--oracle-root", action="append", default=[])
    p.add_argument("--license", type=Path)
    p.add_argument("--notice", type=Path)
    a = p.parse_args()
    try:
        if a.export_to:
            roots = {}
            for item in a.oracle_root:
                name, path = item.split("=", 1)
                need(name not in roots, "duplicate oracle root")
                roots[name] = path
            need(a.license is not None and a.notice is not None, "export requires explicit license and notice")
            result = export(a.export_to, roots, a.license, a.notice)
        else:
            need(not a.oracle_root and a.license is None and a.notice is None, "verification never reads external source authorities")
            result = verify(a.fixtures)
        print(json.dumps(result, indent=2))
        return 0
    except (ValueError, OSError, KeyError, TypeError, UnicodeError) as error:
        print("Collection fixture verification failed: " + str(error), file=__import__("sys").stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
