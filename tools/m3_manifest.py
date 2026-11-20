#!/usr/bin/env python3
"""Mechanical M3 stdlib, external-builtin, and renderer coverage manifest and gate.

Reads all 1,781 M3 items directly from compatibility/inventory.json (1,222 stdlib +
542 builtin + 17 renderer), generates a deterministic probe plan for
`crates/pkl-runtime/examples/m3_coverage_probe.rs`, and joins 1:1 against the live
runtime census and reviewed compatibility/m3-mappings.json with zero silent omissions.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
INVENTORY_PATH = Path("compatibility/inventory.json")
MAPPINGS_PATH = Path("compatibility/m3-mappings.json")
M3_CASES_PATH = Path("tests/fixtures/m3/cases.json")
M2_MANIFEST_PATH = Path("compatibility/m2-language.json")
OUTPUT_PATH = Path("compatibility/m3-stdlib-output.json")
PKL_BINARY_FORMAT_ID = "renderer:stdlib/base.pkl:pkl-binary"


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def encoded(value: object) -> bytes:
    return (json.dumps(value, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")


def read_properties(text: str) -> dict[str, str]:
    result = {}
    for line in text.splitlines():
        if not line or line.startswith("#"):
            continue
        key, value = line.split("=", 1)
        result[key] = value
    return result


def encode_plan_field(text: str) -> str:
    return (
        text.replace("\\", "\\\\")
        .replace("\t", "\\t")
        .replace("\r", "\\r")
        .replace("\n", "\\n")
    )


def load_json(path: Path) -> dict:
    return json.loads(path.read_bytes())


def extract_m3_inventory(inventory: dict) -> dict[str, list[dict]]:
    stdlib = [item for item in inventory["items"] if item["category"] == "stdlib"]
    builtins = [item for item in inventory["items"] if item["category"] == "builtin"]
    renderers = [item for item in inventory["items"] if item["category"] == "renderer"]
    if len(stdlib) != 1222 or len(builtins) != 542 or len(renderers) != 17:
        raise ValueError(
            f"unexpected M3 inventory counts: stdlib={len(stdlib)}, builtin={len(builtins)}, renderer={len(renderers)}"
        )
    return {"stdlib": stdlib, "builtin": builtins, "renderer": renderers}


def validate_baseline(root: Path, inventory: dict, mappings: dict, cases: dict) -> None:
    refs = read_properties((root / "references.lock").read_text(encoding="utf-8"))
    baseline = inventory.get("baseline", {})
    if (
        baseline.get("version") != refs.get("baseline.pkl")
        or baseline.get("revision") != refs.get("pkl.revision")
    ):
        raise ValueError("inventory baseline disagrees with references.lock")
    expected_base = {"version": baseline["version"], "revision": baseline["revision"]}
    if mappings.get("baseline") != expected_base:
        raise ValueError("m3-mappings.json baseline disagrees with inventory.json")
    if cases.get("baseline") != baseline["version"]:
        raise ValueError("tests/fixtures/m3/cases.json baseline disagrees with inventory.json")


def validate_cases_manifest(root: Path, cases_doc: dict) -> dict[str, dict]:
    if cases_doc.get("schema_version") != 1 or not isinstance(cases_doc.get("cases"), list):
        raise ValueError("invalid tests/fixtures/m3/cases.json schema")
    by_id: dict[str, dict] = {}
    for case in cases_doc["cases"]:
        cid = case["id"]
        if cid in by_id:
            raise ValueError(f"duplicate M3 case id: {cid}")
        if case.get("lane") not in {"formats-renderers", "output-files", "pklbinary-decode"}:
            raise ValueError(f"unknown M3 lane for {cid}: {case.get('lane')}")
        for inp in case.get("inputs", []):
            rel = inp["path"]
            path = Path(rel)
            if path.is_absolute() or ".." in path.parts:
                raise ValueError(f"invalid M3 fixture input path: {rel}")
            full = root / path
            if not full.is_file():
                raise ValueError(f"missing M3 fixture input: {rel}")
            if digest(full.read_bytes()) != inp["sha256"]:
                raise ValueError(f"M3 fixture input sha256 mismatch: {rel}")
        by_id[cid] = case
    return by_id


def validate_mappings(
    root: Path,
    inventory: dict,
    mappings: dict,
    m3_cases: dict[str, dict],
    m2_case_ids: set[str],
) -> dict:
    if mappings.get("schema_version") != 1:
        raise ValueError("invalid compatibility/m3-mappings.json schema_version")
    modules_txt = (root / "crates/pkl-runtime/src/stdlib/modules.txt").read_text(encoding="utf-8")
    expected_uris = sorted(line.strip() for line in modules_txt.splitlines() if line.strip())
    module_links = mappings.get("module_links", {})
    if sorted(module_links.keys()) != expected_uris:
        missing = sorted(set(expected_uris) - set(module_links))
        extra = sorted(set(module_links) - set(expected_uris))
        raise ValueError(f"m3-mappings module_links mismatch: missing={missing}, extra={extra}")

    valid_case_ids = set(m3_cases) | m2_case_ids
    validated_links = {}
    for uri, row in sorted(module_links.items()):
        resolved_impl = []
        for link in row.get("implementation_links", []):
            rel, sym = link["path"], link["symbol"]
            p = Path(rel)
            if p.is_absolute() or ".." in p.parts or p.parts[:1] != ("crates",) or p.suffix != ".rs":
                raise ValueError(f"invalid implementation link path for {uri}: {rel}")
            src = (root / p).read_bytes()
            if not re.search(r"\b" + re.escape(sym) + r"\b", src.decode("utf-8")):
                raise ValueError(f"implementation link symbol missing for {uri}: {rel}::{sym}")
            resolved_impl.append({"path": rel, "symbol": sym, "sha256": digest(src)})
        if not resolved_impl:
            raise ValueError(f"empty implementation_links for {uri}")
        cids = row.get("case_ids", [])
        if not cids or not set(cids) <= valid_case_ids:
            unknown = sorted(set(cids) - valid_case_ids)
            raise ValueError(f"unknown case_ids for {uri}: {unknown}")
        validated_links[uri] = {"implementation_links": resolved_impl, "case_ids": list(cids)}

    for group_name in ("renderer_links", "output_surface_links"):
        group = mappings.get(group_name, {})
        for key, cids in group.items():
            if not cids or not set(cids) <= valid_case_ids:
                unknown = sorted(set(cids) - valid_case_ids)
                raise ValueError(f"unknown {group_name} case_ids for {key}: {unknown}")

    deferred_paths = {}
    for entry in mappings.get("deferred_sources", []):
        path = entry["path"]
        milestone = entry["milestone"]
        reason = entry["reason"]
        if not isinstance(path, str) or not isinstance(milestone, str) or not reason.strip():
            raise ValueError(f"invalid deferred_sources entry: {entry}")
        deferred_paths[path] = {"milestone": milestone, "reason": reason}

    return {"module_links": validated_links, "deferred_paths": deferred_paths}


# ---------------------------------------------------------------------------
# Probe plan synthesis for all 542 builtins + 17 renderers (+ pkl-binary)
# ---------------------------------------------------------------------------

def _split_top_level(text: str, sep: str = ",") -> list[str]:
    parts = []
    depth = 0
    start = 0
    for idx, ch in enumerate(text):
        if ch in "(<[{":
            depth += 1
        elif ch in ")>]}":
            depth = max(0, depth - 1)
        elif ch == sep and depth == 0:
            part = text[start:idx].strip()
            if part:
                parts.append(part)
            start = idx + 1
    tail = text[start:].strip()
    if tail:
        parts.append(tail)
    return parts


def _dummy_arg_for_type(type_expr: str) -> str:
    t = type_expr.strip()
    if t.endswith("?"):
        t = t[:-1].strip()
    # Function type `(A, B) -> R`
    if t.startswith("(") and "->" in t:
        close = t.rfind(")")
        arrow = t.find("->", close)
        if close > 0 and arrow > close:
            params = _split_top_level(t[1:close])
            ret = t[arrow + 2 :].strip()
            names = [f"p{i}" for i in range(len(params))]
            body = "true" if ret.startswith("Boolean") else ("1" if ret.startswith(("Int", "Number")) else ("\"a\"" if ret.startswith("String") else ("List(1)" if ret.startswith("List") else ("Pair(1, 2)" if ret.startswith("Pair") else (names[0] if names else "1")))))
            return f"({', '.join(names)}) -> {body}"
    # Union: pick first branch when a top-level union separator exists
    branches = _split_top_level(t, "|")
    if len(branches) > 1:
        return _dummy_arg_for_type(branches[0])
    # Strip constraints / generics
    base = re.split(r"[<(]", t, maxsplit=1)[0].strip()
    mapping = {
        "Int": "1",
        "UInt": "1",
        "UInt8": "1",
        "UInt16": "1",
        "UInt32": "1",
        "Int8": "1",
        "Int16": "1",
        "Int32": "1",
        "Float": "1.0",
        "Number": "1",
        "Boolean": "true",
        "String": '"a"',
        "Char": '"a"',
        "Charset": '"UTF-8"',
        "DurationUnit": '"s"',
        "DataSizeUnit": '"b"',
        "Bytes": "Bytes(1, 2)",
        "Duration": "1.s",
        "DataSize": "1.b",
        "IntSeq": "IntSeq(1, 3)",
        "Pair": "Pair(1, 2)",
        "Regex": 'Regex("a")',
        "List": "List(1)",
        "Collection": "List(1)",
        "Set": "Set(1)",
        "Map": 'Map("a", 1)',
        "Listing": "new Listing { 1 }",
        "Mapping": 'new Mapping { ["a"] = 1 }',
        "Dynamic": "new Dynamic { a = 1 }",
        "Class": "Int",
        "Module": 'import("pkl:math")',
        "TypeAlias": "Int8",
        "Resource": '"a"',
        "Any": "1",
        "NonNull": "1",
        "Comparable": "1",
        "unknown": "1",
    }
    return mapping.get(base, "1")


def _args_from_signature(signature: str, arity: int) -> list[str]:
    if arity == 0:
        return []
    m = re.search(r"\((.*)\)", signature)
    if not m:
        return ["1"] * arity
    params = _split_top_level(m.group(1))
    result = []
    for raw in params[:arity]:
        if ":" in raw:
            _, ty = raw.split(":", 1)
            result.append(_dummy_arg_for_type(ty))
        else:
            result.append("1")
    while len(result) < arity:
        result.append("1")
    return result


# Concrete receiver definitions for all 54 builtin owners in compatibility/inventory.json.
OWNER_RECEIVERS: dict[str, tuple[str, str]] = {
    "pkl.base": ('import "pkl:base"', "base"),
    "pkl.base#Any": ("", "1"),
    "pkl.base#Boolean": ("", "true"),
    "pkl.base#Bytes": ("", 'Bytes(65, 66, 67)'),
    "pkl.base#Class": ("", "Int"),
    "pkl.base#DataSize": ("", "10.kb"),
    "pkl.base#Duration": ("", "5.s"),
    "pkl.base#Dynamic": ("", 'new Dynamic { a = 1; ["k"] = 2; 3 }'),
    "pkl.base#Float": ("", "1.5"),
    "pkl.base#Function": ("", "((a, b) -> a + b)"),
    "pkl.base#Function0": ("", "(() -> 1)"),
    "pkl.base#Function1": ("", "((a) -> a)"),
    "pkl.base#Function2": ("", "((a, b) -> a)"),
    "pkl.base#Function3": ("", "((a, b, c) -> a)"),
    "pkl.base#Function4": ("", "((a, b, c, d) -> a)"),
    "pkl.base#Function5": ("", "((a, b, c, d, e) -> a)"),
    "pkl.base#Int": ("", "42"),
    "pkl.base#IntSeq": ("", "IntSeq(1, 5)"),
    "pkl.base#JsonRenderer": ("", "new JsonRenderer {}"),
    "pkl.base#List": ("", "List(1, 2, 3)"),
    "pkl.base#Listing": ("", "new Listing { 1; 2; 3 }"),
    "pkl.base#Map": ("", 'Map("a", 1, "b", 2)'),
    "pkl.base#Mapping": ("", 'new Mapping { ["a"] = 1; ["b"] = 2 }'),
    "pkl.base#Module": ("", 'import("pkl:math")'),
    "pkl.base#PListRenderer": ("", "new PListRenderer {}"),
    "pkl.base#Pair": ("", 'Pair("a", 1)'),
    "pkl.base#PcfRenderer": ("", "new PcfRenderer {}"),
    "pkl.base#PropertiesRenderer": ("", "new PropertiesRenderer {}"),
    "pkl.base#Regex": ("", 'Regex("a+")'),
    "pkl.base#Set": ("", "Set(1, 2, 3)"),
    "pkl.base#String": ("", '"hello"'),
    "pkl.base#Typed": ("class T { x: Int = 1 }", "new T {}"),
    "pkl.base#YamlRenderer": ("", "new YamlRenderer {}"),
    "pkl.math": ('import "pkl:math"', "math"),
    "pkl.json#Parser": ('import "pkl:json"', "new json.Parser {}"),
    "pkl.jsonnet#Renderer": ('import "pkl:jsonnet"', "new jsonnet.Renderer {}"),
    "pkl.pklbinary#Renderer": ('import "pkl:pklbinary"', "new pklbinary.Renderer {}"),
    "pkl.platform": ('import "pkl:platform"', "platform"),
    "pkl.protobuf#Renderer": ('import "pkl:protobuf"', "new protobuf.Renderer {}"),
    "pkl.ref": ('import "pkl:ref"', "ref"),
    "pkl.ref#Reference": (
        'import "pkl:ref"\nclass D extends ref.Domain { function renderReference(r) = "R" }\nlocal r0 = ref.Reference(new D {}, String, "x")',
        "r0",
    ),
    "pkl.reflect": ('import "pkl:reflect"', "reflect"),
    "pkl.reflect#Class": ('import "pkl:reflect"', "reflect.Class(Int)"),
    "pkl.reflect#DeclaredType": ('import "pkl:reflect"', "reflect.listType"),
    "pkl.reflect#Type": ('import "pkl:reflect"', "reflect.intType"),
    "pkl.release": ('import "pkl:release"', "release"),
    "pkl.test": ('import "pkl:test"', "test"),
    "pkl.xml#Renderer": ('import "pkl:xml"', "new xml.Renderer {}"),
    "pkl.yaml#Parser": ('import "pkl:yaml"', "new yaml.Parser {}"),
    "pkl.analyze": ('import "pkl:analyze"', "analyze"),
    "pkl.EvaluatorSettings": ('import "pkl:EvaluatorSettings" as ES', "ES"),
    "pkl.Benchmark#Microbenchmark": (
        'import "pkl:Benchmark"\nlocal b = new Benchmark.Microbenchmark { iterations = 1; iterationTime = 1.ns; isVerbose = false; expression = () -> 1 + 1 }',
        "b",
    ),
    "pkl.Benchmark#OutputBenchmark": (
        'import "pkl:Benchmark"\nlocal b = new Benchmark.OutputBenchmark { iterations = 1; iterationTime = 1.ns; isVerbose = false; sourceModule = import("pkl:math") }',
        "b",
    ),
    "pkl.Benchmark#ParserBenchmark": (
        'import "pkl:Benchmark"\nlocal b = new Benchmark.ParserBenchmark { iterations = 1; iterationTime = 1.ns; isVerbose = false; sourceText = "x = 1" }',
        "b",
    ),
}

# Explicit high-signal probe overrides for tricky or local builtins
BUILTIN_OVERRIDES: dict[tuple[str, str], tuple[str, str]] = {
    (
        "pkl.EvaluatorSettings#resolvePath",
        "ExternalMethod3Node",
    ): (
        'import "pkl:EvaluatorSettings" as ES\nlocal s = (ES) { rootDir = "sub" }\nres = s.resolve("file:///tmp/PklProject", "macOS").rootDir',
        "res",
    ),
    (
        "pkl.analyze#importGraph",
        "ExternalMethod1Node",
    ): (
        'import "pkl:analyze"\nres = analyze.importGraph(Set("file:///m3-coverage-probe/leaf.pkl")).imports.length',
        "res",
    ),
    (
        "pkl.base#Typed.getProperty",
        "ExternalMethod1Node",
    ): ('class T { x: Int = 1 }\nres = new T {}.getProperty("x")', "res"),
    (
        "pkl.base#Function.applyToList",
        "ExternalMethod1Node",
    ): ("local f = (a, b) -> a + b\nres = f.applyToList(List(1, 2))", "res"),
    (
        "pkl.base#Module.relativePathTo",
        "ExternalMethod1Node",
    ): ('local m = import("pkl:math")\nres = m.relativePathTo(m)', "res"),
    (
        "pkl.base#Null",
        "ExternalMethod1Node",
    ): ('import "pkl:base"\nres = base.Null(new Dynamic { a = 1 })', "res"),
    (
        "pkl.base#PropertiesRenderer.renderDocument",
        "ExternalMethod1Node",
    ): ('res = new PropertiesRenderer {}.renderDocument(new Dynamic { a = 1 })', "res"),
    (
        "pkl.base#PropertiesRenderer.renderValue",
        "ExternalMethod1Node",
    ): ('res = new PropertiesRenderer {}.renderValue("ok")', "res"),
    (
        "pkl.json#Parser.parse",
        "ExternalMethod1Node",
    ): ('import "pkl:json"\nres = new json.Parser {}.parse("{\\"a\\": 1}")', "res"),
    (
        "pkl.yaml#Parser.parse",
        "ExternalMethod1Node",
    ): ('import "pkl:yaml"\nres = new yaml.Parser {}.parse("a: 1\\n")', "res"),
    (
        "pkl.yaml#Parser.parseAll",
        "ExternalMethod1Node",
    ): ('import "pkl:yaml"\nres = new yaml.Parser {}.parseAll("a: 1\\n---\\nb: 2\\n")', "res"),
    (
        "pkl.protobuf#Renderer.renderDocument",
        "ExternalMethod1Node",
    ): ('import "pkl:protobuf"\nclass M { x: Int = 1 }\nres = new protobuf.Renderer {}.renderDocument(new M {})', "res"),
    (
        "pkl.protobuf#Renderer.renderValue",
        "ExternalMethod1Node",
    ): ('import "pkl:protobuf"\nclass M { x: Int = 1 }\nres = new protobuf.Renderer {}.renderValue(new M {})', "res"),
    (
        "pkl.protobuf#Renderer.renderType",
        "ExternalMethod1Node",
    ): ('import "pkl:protobuf"\nimport "pkl:reflect"\nclass M { x: Int = 1 }\nres = new protobuf.Renderer {}.renderType(reflect.DeclaredType(reflect.Class(M)))', "res"),
    (
        "pkl.ref#Reference",
        "ExternalMethod3Node",
    ): ('import "pkl:ref"\nclass D extends ref.Domain { function renderReference(r) = "R" }\nres = ref.Reference(new D {}, String, "x").toString()', "res"),
    (
        "pkl.reflect#Class",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.Class(Int).name', "res"),
    (
        "pkl.reflect#Module",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.Module(import("pkl:math")).name', "res"),
    (
        "pkl.reflect#moduleOf",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.moduleOf(import("pkl:math")).name', "res"),
    (
        "pkl.reflect#TypeAlias",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.TypeAlias(Int8).name', "res"),
    (
        "pkl.reflect#DeclaredType",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.DeclaredType(reflect.Class(Int)).referent.name', "res"),
    (
        "pkl.reflect#FunctionType",
        "ExternalMethod2Node",
    ): ('import "pkl:reflect"\nres = reflect.FunctionType(List(reflect.intType), reflect.stringType).parameterTypes.length', "res"),
    (
        "pkl.reflect#StringLiteralType",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.StringLiteralType("ok").value', "res"),
    (
        "pkl.reflect#UnionType",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.UnionType(List(reflect.intType, reflect.stringType)).members.length', "res"),
    (
        "pkl.reflect#TypeVariable",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.TypeVariable(reflect.Class(List).typeParameters.first).referent.name', "res"),
    (
        "pkl.reflect#Class.isSubclassOf",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.Class(Int).isSubclassOf(reflect.Class(Number))', "res"),
    (
        "pkl.reflect#DeclaredType.withTypeArgument",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.listType.withTypeArgument(reflect.intType).typeArguments.length', "res"),
    (
        "pkl.reflect#DeclaredType.withTypeArguments",
        "ExternalMethod1Node",
    ): ('import "pkl:reflect"\nres = reflect.listType.withTypeArguments(List(reflect.intType)).typeArguments.length', "res"),
    (
        "pkl.test#catch",
        "ExternalMethod1Node",
    ): ('import "pkl:test"\nres = test.catch(() -> throw("boom"))', "res"),
    (
        "pkl.test#catchOrNull",
        "ExternalMethod1Node",
    ): ('import "pkl:test"\nres = test.catchOrNull(() -> 1)', "res"),
    (
        "pkl.base#String.base64Decoded",
        "ExternalPropertyNode",
    ): ('res = "aGVsbG8=".base64Decoded', "res"),
    (
        "pkl.base#String.base64DecodedBytes",
        "ExternalPropertyNode",
    ): ('res = "aGVsbG8=".base64DecodedBytes', "res"),
    (
        "pkl.base#String.toInt",
        "ExternalMethod0Node",
    ): ('res = "42".toInt()', "res"),
    (
        "pkl.base#String.toFloat",
        "ExternalMethod0Node",
    ): ('res = "3.5".toFloat()', "res"),
    (
        "pkl.base#String.toBoolean",
        "ExternalMethod0Node",
    ): ('res = "true".toBoolean()', "res"),
    (
        "pkl.base#String.toIntOrNull",
        "ExternalMethod0Node",
    ): ('res = "42".toIntOrNull()', "res"),
    (
        "pkl.base#String.toFloatOrNull",
        "ExternalMethod0Node",
    ): ('res = "3.5".toFloatOrNull()', "res"),
    (
        "pkl.base#String.toBooleanOrNull",
        "ExternalMethod0Node",
    ): ('res = "true".toBooleanOrNull()', "res"),
    (
        "pkl.base#Int.toRadixString",
        "ExternalMethod1Node",
    ): ("res = 42.toRadixString(16)", "res"),
    (
        "pkl.base#List.toMap",
        "ExternalMethod1Node",
    ): ("res = List(1, 2).toMap((x) -> x)", "res"),
    (
        "pkl.base#Set.toMap",
        "ExternalMethod1Node",
    ): ("res = Set(1, 2).toMap((x) -> x)", "res"),
    (
        "pkl.base#List.flatMap",
        "ExternalMethod1Node",
    ): ("res = List(1, 2).flatMap((x) -> List(x))", "res"),
    (
        "pkl.base#Set.flatMap",
        "ExternalMethod1Node",
    ): ("res = Set(1, 2).flatMap((x) -> Set(x))", "res"),
    (
        "pkl.base#Map.flatMap",
        "ExternalMethod1Node",
    ): ('res = Map("a", 1).flatMap((k, v) -> Map(k, v))', "res"),
    (
        "pkl.base#List.zip",
        "ExternalMethod1Node",
    ): ("res = List(1, 2).zip(List(3, 4))", "res"),
    (
        "pkl.base#Set.zip",
        "ExternalMethod1Node",
    ): ("res = Set(1, 2).zip(Set(3, 4))", "res"),
    (
        "pkl.base#List.toTyped",
        "ExternalMethod1Node",
    ): ("class T { x: Int = 1 }\nres = List().toTyped(T)", "res"),
    (
        "pkl.base#Listing.toTyped",
        "ExternalMethod1Node",
    ): ("class T { x: Int = 1 }\nres = new Listing {}.toTyped(T)", "res"),
    (
        "pkl.base#Mapping.toTyped",
        "ExternalMethod1Node",
    ): ("class T { x: Int = 1 }\nres = new Mapping {}.toTyped(T)", "res"),
    (
        "pkl.base#Dynamic.toTyped",
        "ExternalMethod1Node",
    ): ("class T { x: Int = 1 }\nres = new Dynamic { x = 2 }.toTyped(T)", "res"),
}


def build_builtin_probe(item: dict, stdlib_by_id: dict[str, dict]) -> tuple[str, str]:
    symbol = item["symbol"]
    kind = item["kind"]
    if (symbol, kind) in BUILTIN_OVERRIDES:
        return BUILTIN_OVERRIDES[(symbol, kind)]

    mod, rest = symbol.split("#", 1)
    if "." in rest:
        cls, member = rest.split(".", 1)
        owner = f"{mod}#{cls}"
    else:
        owner = mod
        member = rest

    if owner not in OWNER_RECEIVERS:
        raise ValueError(f"missing OWNER_RECEIVERS entry for {owner} ({symbol})")
    preamble, recv = OWNER_RECEIVERS[owner]

    decl_id = item.get("upstream_declaration", {}).get("item_id")
    decl_sig = stdlib_by_id[decl_id].get("signature", "") if decl_id in stdlib_by_id else ""

    if kind == "ExternalPropertyNode":
        expr = f"{recv}.{member}"
    else:
        m = re.fullmatch(r"ExternalMethod(\d)Node", kind)
        if not m:
            raise ValueError(f"unsupported builtin kind {kind} for {symbol}")
        arity = int(m.group(1))
        args = _args_from_signature(decl_sig, arity)
        expr = f"{recv}.{member}({', '.join(args)})"

    source = f"{preamble}\nres = {expr}\n" if preamble else f"res = {expr}\n"
    return source, "res"


def build_renderer_probe(item: dict) -> tuple[str, str, str]:
    symbol = item["symbol"]
    kind = item["kind"]
    if kind == "output-format":
        if symbol == "textproto":
            src = "class T { x: Int = 1 }\noutput { value = new T {} }\n"
        else:
            src = "x = 1\n"
        expr = '(output.renderer.getClass().simpleName) + ":" + (output.bytes.length.toString())'
        return src, expr, symbol
    if kind == "renderer-class":
        mod, cls = symbol.split("#", 1)
        if mod == "pkl.base":
            if cls == "PropertiesRenderer":
                src = (
                    f'local r = new {cls} {{}}\n'
                    'res = r.renderDocument(new Dynamic { a = 1 }) + "|" + r.renderValue("ok")\n'
                )
            else:
                src = (
                    f'local r = new {cls} {{}}\n'
                    'res = r.renderDocument(new Dynamic { a = 1 }) + "|" + r.renderValue(1)\n'
                )
        elif mod == "pkl.protobuf":
            src = (
                'import "pkl:protobuf"\n'
                'import "pkl:reflect"\n'
                'class M { x: Int = 1 }\n'
                'local r = new protobuf.Renderer {}\n'
                'res = r.renderDocument(new M {}) + "|" + r.renderValue(new M {}) + "|" + r.renderType(reflect.DeclaredType(reflect.Class(M)))\n'
            )
        elif mod == "pkl.pklbinary":
            src = (
                'import "pkl:pklbinary"\n'
                'local r = new pklbinary.Renderer {}\n'
                'res = r.renderDocument(new Dynamic { a = 1 }).length.toString() + ":" + r.renderValue(1).length.toString()\n'
            )
        else:
            short = mod.removeprefix("pkl.")
            src = (
                f'import "pkl:{short}"\n'
                f'local r = new {short}.{cls} {{}}\n'
                'res = r.renderDocument(new Dynamic { a = 1 }) + "|" + r.renderValue(1)\n'
            )
        return src, "res", ""
    raise ValueError(f"unknown renderer kind: {kind}")


def build_probe_plan(inventory: dict) -> list[str]:
    m3 = extract_m3_inventory(inventory)
    stdlib_by_id = {item["id"]: item for item in m3["stdlib"]}
    lines = ["# category\tid\tsymbol\tkind\tsource_code\texpression\toutput_format"]
    for item in m3["builtin"]:
        src, expr = build_builtin_probe(item, stdlib_by_id)
        lines.append(
            "\t".join(
                [
                    "builtin",
                    item["id"],
                    item["symbol"],
                    item["kind"],
                    encode_plan_field(src),
                    encode_plan_field(expr),
                    "",
                ]
            )
        )
    for item in m3["renderer"]:
        src, expr, fmt = build_renderer_probe(item)
        lines.append(
            "\t".join(
                [
                    "renderer",
                    item["id"],
                    item["symbol"],
                    item["kind"],
                    encode_plan_field(src),
                    encode_plan_field(expr),
                    encode_plan_field(fmt),
                ]
            )
        )
    # Synthetic check for pkl-binary outputFormat from stdlib/base.pkl:123
    pb_src, pb_expr, pb_fmt = build_renderer_probe(
        {"id": PKL_BINARY_FORMAT_ID, "symbol": "pkl-binary", "kind": "output-format"}
    )
    lines.append(
        "\t".join(
            [
                "renderer",
                PKL_BINARY_FORMAT_ID,
                "pkl-binary",
                "output-format",
                encode_plan_field(pb_src),
                encode_plan_field(pb_expr),
                encode_plan_field(pb_fmt),
            ]
        )
    )
    return lines


# ---------------------------------------------------------------------------
# 1:1 Mechanical Coverage Join against m3_coverage_probe census
# ---------------------------------------------------------------------------

def _module_uri_from_stdlib_path(path: str) -> str:
    return "pkl:" + Path(path).stem


def evaluate_census(
    inventory: dict,
    census: dict,
    mappings: dict,
    root: Path = ROOT,
) -> dict:
    if census.get("schema_version") != 1:
        raise ValueError("unsupported census schema_version")
    m3 = extract_m3_inventory(inventory)
    deferred_paths = {d["path"]: d for d in mappings.get("deferred_sources", [])}
    inv_source_sha = {src["path"]: src["sha256"] for src in inventory["sources"]}

    failures: list[str] = []

    # 1. Vendored stdlib files byte-for-byte parity
    vendored_by_file = {row["filename"]: row for row in census.get("vendored_stdlib_files", [])}
    vendored_summary = Counter()
    upstream_stdlib_files = sorted(
        src["path"] for src in inventory["sources"] if src["path"].startswith("stdlib/")
    )
    for src_path in upstream_stdlib_files:
        fname = Path(src_path).name
        row = vendored_by_file.get(fname)
        is_deferred = src_path in deferred_paths
        if row is None:
            vendored_summary["omitted"] += 1
            failures.append(f"vendored stdlib file omitted from census: {src_path}")
            continue
        if row.get("upstream_sha256") != inv_source_sha[src_path]:
            vendored_summary["upstream_hash_drift"] += 1
            failures.append(f"upstream stdlib sha256 disagrees with inventory.json: {src_path}")
            continue
        if is_deferred:
            vendored_summary["deferred"] += 1
            continue
        if row.get("status") != "exact_match" or row.get("vendored_sha256") != inv_source_sha[src_path]:
            vendored_summary[row.get("status", "unknown")] += 1
            failures.append(f"vendored stdlib file mismatch/missing ({row.get('status')}): {src_path}")
        else:
            vendored_summary["exact_match"] += 1

    # 2. All 542 external builtins
    census_builtins = census.get("builtins", {})
    builtin_summary = Counter()
    builtin_status_by_id: dict[str, str] = {}
    for item in m3["builtin"]:
        iid = item["id"]
        row = census_builtins.get(iid)
        if row is None:
            builtin_summary["omitted"] += 1
            builtin_status_by_id[iid] = "omitted"
            failures.append(f"builtin omitted from census: {item['symbol']} ({iid})")
            continue
        st = row.get("status", "unknown")
        builtin_summary[st] += 1
        builtin_status_by_id[iid] = st
        if st != "implemented":
            failures.append(
                f"builtin not implemented ({st}/{row.get('outcome')}): {item['symbol']} ({row.get('detail')})"
            )

    # 3. All 17 renderer inventory items + pkl-binary output-format
    census_renderers = census.get("renderers", {})
    renderer_summary = Counter()
    for item in [*m3["renderer"], {"id": PKL_BINARY_FORMAT_ID, "symbol": "pkl-binary", "kind": "output-format"}]:
        iid = item["id"]
        row = census_renderers.get(iid)
        if row is None:
            renderer_summary["omitted"] += 1
            failures.append(f"renderer omitted from census: {item['symbol']} ({iid})")
            continue
        st = row.get("status", "unknown")
        renderer_summary[st] += 1
        if st != "implemented":
            failures.append(
                f"renderer not implemented ({st}): {item['symbol']} [{item['kind']}] ({row.get('detail')})"
            )

    # 4. All 1,222 stdlib declarations (public via live reflection, local via vendored byte-parity + external probe)
    census_modules = census.get("modules", {})
    stdlib_summary = Counter()
    for item in m3["stdlib"]:
        src_path = item["source"]["path"]
        if src_path in deferred_paths:
            stdlib_summary["deferred"] += 1
            continue
        uri = _module_uri_from_stdlib_path(src_path)
        mod_row = census_modules.get(uri)
        if mod_row is None:
            stdlib_summary["omitted"] += 1
            failures.append(f"stdlib module omitted from census: {uri} ({item['symbol']})")
            continue
        if mod_row.get("status") != "loaded" or not isinstance(mod_row.get("reflected"), dict):
            stdlib_summary["module_unavailable"] += 1
            failures.append(
                f"stdlib module failed to load/reflect ({mod_row.get('status')}): {uri} ({item['symbol']})"
            )
            continue

        vis = item.get("visibility", "public")
        if vis == "local":
            # Verified via vendored byte-parity + live external builtin probe when external
            if item.get("external") and item["kind"] in {"property", "function"}:
                bid = item.get("upstream_builtin", {}).get("item_id")
                if builtin_status_by_id.get(bid) == "implemented":
                    stdlib_summary["implemented"] += 1
                else:
                    stdlib_summary["missing_or_unsupported"] += 1
                    failures.append(f"local external stdlib member not implemented: {item['symbol']}")
            else:
                fname = Path(src_path).name
                vrow = vendored_by_file.get(fname, {})
                if vrow.get("status") == "exact_match":
                    stdlib_summary["implemented"] += 1
                else:
                    stdlib_summary["missing_or_unsupported"] += 1
                    failures.append(f"local stdlib member unvendored: {item['symbol']}")
            continue

        reflected = mod_row["reflected"]
        kind = item["kind"]
        sym = item["symbol"]
        found = False
        if kind == "module":
            found = reflected.get("module_name") == sym and reflected.get("uri") == uri
        else:
            _, rest = sym.split("#", 1)
            if "." in rest:
                cls_name, mem_name = rest.split(".", 1)
                cls_obj = reflected.get("classes", {}).get(cls_name)
                if isinstance(cls_obj, dict):
                    bucket = "properties" if kind == "property" else "methods"
                    found = mem_name in cls_obj.get(bucket, [])
            else:
                if kind == "class":
                    found = rest in reflected.get("classes", {})
                elif kind == "typealias":
                    found = rest in reflected.get("type_aliases", [])
                elif kind == "property":
                    found = rest in reflected.get("properties", [])
                elif kind == "function":
                    found = rest in reflected.get("methods", [])

        if not found:
            stdlib_summary["missing_from_reflection"] += 1
            failures.append(f"public stdlib member missing from live reflection: {sym} [{kind}]")
            continue

        if item.get("external") and kind in {"property", "function"}:
            bid = item.get("upstream_builtin", {}).get("item_id")
            if builtin_status_by_id.get(bid) != "implemented":
                stdlib_summary["builtin_not_implemented"] += 1
                failures.append(f"external stdlib member builtin not implemented: {sym} [{kind}]")
                continue

        stdlib_summary["implemented"] += 1

    return {
        "schema_version": 1,
        "passed": len(failures) == 0,
        "counts": {
            "vendored_stdlib_files": dict(sorted(vendored_summary.items())),
            "stdlib": dict(sorted(stdlib_summary.items())),
            "builtin": dict(sorted(builtin_summary.items())),
            "renderer": dict(sorted(renderer_summary.items())),
            "failure_count": len(failures),
        },
        "failures": failures,
    }


def build_manifest(root: Path = ROOT, census: dict | None = None) -> dict:
    inventory = load_json(root / INVENTORY_PATH)
    mappings = load_json(root / MAPPINGS_PATH)
    cases_doc = load_json(root / M3_CASES_PATH)
    m2_doc = load_json(root / M2_MANIFEST_PATH)

    validate_baseline(root, inventory, mappings, cases_doc)
    m3_cases = validate_cases_manifest(root, cases_doc)
    m2_case_ids = {row["id"] for row in m2_doc.get("cases", [])}
    validated = validate_mappings(root, inventory, mappings, m3_cases, m2_case_ids)
    m3 = extract_m3_inventory(inventory)

    deferred_paths = validated["deferred_paths"]
    coverage = evaluate_census(inventory, census, mappings, root) if census is not None else None

    stdlib_m3_count = sum(1 for i in m3["stdlib"] if i["source"]["path"] not in deferred_paths)
    stdlib_deferred_count = sum(1 for i in m3["stdlib"] if i["source"]["path"] in deferred_paths)

    return {
        "schema_version": 1,
        "baseline": inventory["baseline"],
        "scope": (
            "Complete M3 standard-library, external-builtin, renderer, parser, binary, and "
            "output.files accounting. Source mappings and case registrations do not claim "
            "compatibility passes; live census and Java 0.32.0 differential reports are separate."
        ),
        "sources": {
            INVENTORY_PATH.as_posix(): digest((root / INVENTORY_PATH).read_bytes()),
            MAPPINGS_PATH.as_posix(): digest((root / MAPPINGS_PATH).read_bytes()),
            M3_CASES_PATH.as_posix(): digest((root / M3_CASES_PATH).read_bytes()),
            "crates/pkl-runtime/src/stdlib/modules.txt": digest(
                (root / "crates/pkl-runtime/src/stdlib/modules.txt").read_bytes()
            ),
        },
        "counts": {
            "inventory_total_m3_category_items": len(m3["stdlib"]) + len(m3["builtin"]) + len(m3["renderer"]),
            "stdlib_total": len(m3["stdlib"]),
            "stdlib_m3_required": stdlib_m3_count,
            "stdlib_deferred": stdlib_deferred_count,
            "stdlib_by_visibility": dict(
                sorted(Counter(i.get("visibility", "public") for i in m3["stdlib"]).items())
            ),
            "builtin_total": len(m3["builtin"]),
            "renderer_total": len(m3["renderer"]),
            "probe_plan_rows": len(m3["builtin"]) + len(m3["renderer"]) + 1,
            "m3_differential_cases": len(m3_cases),
            "m3_differential_cases_by_lane": dict(
                sorted(Counter(c["lane"] for c in m3_cases.values()).items())
            ),
        },
        "deferred_sources": mappings.get("deferred_sources", []),
        "module_links": validated["module_links"],
        "renderer_links": mappings.get("renderer_links", {}),
        "output_surface_links": mappings.get("output_surface_links", {}),
        "cases": cases_doc["cases"],
        "coverage_gate": coverage,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, default=None)
    parser.add_argument("--emit-plan", type=Path, help="write deterministic TSV plan for m3_coverage_probe")
    parser.add_argument("--write", action="store_true", help="write compatibility/m3-stdlib-output.json")
    parser.add_argument("--check", action="store_true", help="verify compatibility/m3-stdlib-output.json freshness")
    parser.add_argument("--census", type=Path, help="existing JSON census produced by m3_coverage_probe")
    parser.add_argument("--verify-coverage", type=Path, help="verify 1:1 coverage against a census JSON and fail on gaps")
    parser.add_argument("--run-probe", type=Path, help="execute built m3_coverage_probe binary and verify 1:1 coverage")
    parser.add_argument("--census-out", type=Path, help="optional path to persist raw census JSON with --run-probe")
    args = parser.parse_args()

    root = args.root.resolve()
    out_path = args.output.resolve() if args.output else (root / OUTPUT_PATH)

    try:
        inventory = load_json(root / INVENTORY_PATH)
        if args.emit_plan:
            plan_text = "\n".join(build_probe_plan(inventory)) + "\n"
            args.emit_plan.parent.mkdir(parents=True, exist_ok=True)
            args.emit_plan.write_text(plan_text, encoding="utf-8")
            print(json.dumps({"plan_rows": len(plan_text.splitlines()) - 1}, sort_keys=True))
            return 0

        if args.run_probe:
            with tempfile.TemporaryDirectory() as td:
                td_path = Path(td)
                plan_file = td_path / "m3_probe_plan.tsv"
                plan_file.write_text("\n".join(build_probe_plan(inventory)) + "\n", encoding="utf-8")
                census_file = args.census_out.resolve() if args.census_out else (td_path / "census.json")
                proc = subprocess.run(
                    [str(args.run_probe.resolve()), "--root", str(root), "--plan", str(plan_file), "--output", str(census_file)],
                    capture_output=True,
                    text=True,
                )
                if proc.returncode != 0:
                    print(f"m3_coverage_probe failed (rc={proc.returncode}): {proc.stderr}", file=sys.stderr)
                    return 1
                census = load_json(census_file)
            mappings = load_json(root / MAPPINGS_PATH)
            verdict = evaluate_census(inventory, census, mappings, root)
            print(json.dumps(verdict["counts"], sort_keys=True))
            if not verdict["passed"]:
                for line in verdict["failures"][:40]:
                    print(f"M3 coverage gap: {line}", file=sys.stderr)
                if len(verdict["failures"]) > 40:
                    print(f"... and {len(verdict['failures']) - 40} more gap(s)", file=sys.stderr)
                return 1
            return 0

        if args.verify_coverage:
            census = load_json(args.verify_coverage)
            mappings = load_json(root / MAPPINGS_PATH)
            verdict = evaluate_census(inventory, census, mappings, root)
            print(json.dumps(verdict["counts"], sort_keys=True))
            if not verdict["passed"]:
                for line in verdict["failures"][:40]:
                    print(f"M3 coverage gap: {line}", file=sys.stderr)
                return 1
            return 0

        census = load_json(args.census) if args.census else None
        manifest = build_manifest(root, census=census)
        data = encoded(manifest)
        if args.check:
            if not out_path.is_file() or out_path.read_bytes() != data:
                raise ValueError(f"M3 manifest missing or stale: {out_path}; run tools/m3_manifest.py --write")
        elif args.write:
            out_path.parent.mkdir(parents=True, exist_ok=True)
            out_path.write_bytes(data)
        print(json.dumps(manifest["counts"], sort_keys=True))
        return 0
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print(f"M3 manifest error: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
