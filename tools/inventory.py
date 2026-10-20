#!/usr/bin/env python3
"""Deterministic source inventory for the pinned Pkl reference (Python stdlib only).

This is a source scanner, not a Pkl/JVM compiler or a compatibility test runner.
See docs/inventory.md for the extraction contract and its limits.
"""

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
IDENT = r"(?:[A-Za-z_$][\w$]*|`[^`]+`)"
TYPE_WORDS = {"class", "interface", "enum", "record", "object"}
STRING_START = re.compile(r'(#*)("""|"|\')')


def sha(data):
    return hashlib.sha256(data).hexdigest()


def mask(text, strings=True):
    """Blank comments/literals, retaining offsets and newlines; nested block comments.

    Pkl raw string delimiters and Kotlin/Java triple quotes are recognized. String
    interpolation is intentionally opaque: declarations inside it are not API.
    """
    result = list(text)
    i = 0
    while i < len(text):
        start = i
        if text.startswith("//", i):
            end = text.find("\n", i)
            i = len(text) if end < 0 else end
        elif text.startswith("/*", i):
            i += 2
            depth = 1
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
        else:
            match = STRING_START.match(text, i)
            if not match:
                i += 1
                continue
            hashes, quote = match.groups()
            i += len(match.group())
            close = quote + hashes
            while i < len(text):
                if text.startswith(close, i):
                    i += len(close)
                    break
                if text.startswith("\\" + hashes, i):
                    i += 2 + len(hashes)
                else:
                    i += 1
            if not strings:
                continue
        for pos in range(start, i):
            if result[pos] != "\n":
                result[pos] = " "
    return "".join(result)


def strip_annotations(header):
    # Annotation arguments may contain nested parentheses; remove their full span.
    pattern = re.compile(r"@(?!interface\b)" + IDENT + r"(?:\." + IDENT + r")*\s*")
    while match := pattern.search(header):
        end = match.end()
        if end < len(header) and header[end] == "(":
            depth = 1
            end += 1
            while end < len(header) and depth:
                depth += (header[end] == "(") - (header[end] == ")")
                end += 1
        header = header[:match.start()] + " " * (end - match.start()) + header[end:]
    return header


def split_components(header):
    """Split source parameter/constant lists at commas outside nested constructs."""
    start = 0
    depth = 0
    for pos, char in enumerate(header):
        if char in "(<[{":
            depth += 1
        elif char in ")>]}":
            depth -= 1
        elif char == "," and depth == 0:
            yield start, header[start:pos]
            start = pos + 1
    yield start, header[start:]


def entry(category, path, line, symbol, kind, signature="", **extra):
    result = {
        "id": f"{category}:{path}:{symbol}" + (":" + sha(signature.encode())[:12] if signature else ""),
        "category": category,
        "source": {"path": path, "line": line},
        "symbol": symbol,
        "kind": kind,
        "implementation": {"state": "unimplemented", "links": []},
        "tests": {"state": "unmapped", "case_ids": []},
    }
    if signature:
        result["signature"] = " ".join(signature.split())
    result.update(extra)
    return result


def pkl_declarations(path, text):
    """Line declarations at module/class indentation, retaining local visibility.

    Pinned stdlib uses top-level classes and two-space class member indentation.
    Brace depth prevents extracting members of object literals/function bodies.
    """
    clean = mask(text)
    module = re.search(r"(?m)^\s*(?:open\s+|abstract\s+)?module\s+([\w.]+)", clean)
    module = module.group(1) if module else "pkl." + Path(path).stem
    result = [entry("stdlib", path, 1, module, "module")]
    depth = 0
    owner = None
    class_depth = None
    pending_class = None
    pattern = re.compile(
        r"^(?P<indent> *)(?P<mods>(?:(?:abstract|open|external|local|hidden|fixed|const)\s+)*)"
        r"(?:(?P<kind>class|typealias|function)\s+)?(?P<name>" + IDENT + r")(?P<rest>.*)$"
    )
    for line_no, line in enumerate(clean.splitlines(), 1):
        match = pattern.match(line)
        if match:
            indent, mods, kind, name, rest = match.groups()
            at_module = depth == 0 and not indent
            at_class = owner and depth == class_depth and len(indent) == 2
            declaration = kind or (rest.lstrip().startswith((":", "=", "{")) and "property")
            if (at_module or at_class) and declaration:
                if kind != "class":
                    pending_class = None
                symbol = module + "#" + ((owner + ".") if at_class else "") + name.strip("`")
                result.append(entry("stdlib", path, line_no, symbol, declaration,
                                    line.strip(), external="external" in mods.split(),
                                    hidden="hidden" in mods.split(),
                                    visibility="local" if "local" in mods.split() else "public"))
                if kind == "class":
                    pending_class = name.strip("`")
        opens, closes = line.count("{"), line.count("}")
        if pending_class and opens:
            owner, class_depth = pending_class, depth + 1
            pending_class = None
        elif pending_class and line.strip() and match is None:
            # A class without a body has no following member scope.
            pending_class = None
        depth += opens - closes
        if owner and depth < class_depth:
            owner, class_depth = None, None
        if pending_class and match and "{" not in line:
            # A following declaration replaces this bodyless class; continuation
            # lines cannot start a public member at top-level indentation.
            next_kind = match.group("kind")
            if next_kind != "class":
                pending_class = None
    return result


def java_declarations(path, text, category="java_api"):
    """Extract explicit public/protected and implicit interface source members.

    Tracks type/body scope and parent visibility; overload identities include the
    complete source header. Inherited and compiler-generated members are not here.
    """
    clean = mask(text)
    package = re.search(r"\bpackage\s+([\w.]+)\s*;", clean)
    package = package.group(1) + "." if package else ""
    result = []
    stack = []
    start = 0
    parens = 0
    for pos, char in enumerate(clean):
        if char == "(":
            parens += 1
        elif char == ")":
            parens -= 1
        if char not in "{};" or parens:
            continue
        raw = clean[start:pos]
        header = strip_annotations(raw).strip()
        parent = stack[-1] if stack else None
        in_type = parent and parent["kind"] in TYPE_WORDS
        reachable = not stack or (in_type and parent["visible"])
        implicit = in_type and parent["kind"] == "interface"
        visible = reachable and not re.search(r"\bprivate\b", header) and (
            implicit or bool(re.search(r"\b(public|protected)\b", header)))
        type_match = re.search(r"\b(class|interface|enum|record)\s+(" + IDENT + r")", header)
        name = None
        kind = "body"
        if type_match and "=" not in header[:type_match.start()]:
            kind, name = type_match.groups()
        elif in_type and header and visible:
            method = re.search(r"(" + IDENT + r")\s*\(", header)
            if method and "=" not in header[:method.start()]:
                name = method.group(1)
                kind = "constructor" if name == parent["name"] else "method"
            elif char == ";":
                left = header.split("=", 1)[0].strip()
                names = re.findall(IDENT, left)
                if names:
                    name, kind = names[-1], "field"
        owners = [scope["name"] for scope in stack if scope["kind"] in TYPE_WORDS]
        if name and visible:
            symbol = package + ".".join(owners + [name])
            line = clean.count("\n", 0, start + len(raw) - len(raw.lstrip())) + 1
            result.append(entry(category, path, line, symbol, kind, header))
            if kind == "record":
                components = header.split("(", 1)[1].rsplit(")", 1)[0] if "(" in header else ""
                for _, component in split_components(components):
                    names = re.findall(IDENT, strip_annotations(component))
                    if names:
                        result.append(entry(category, path, line, symbol + "." + names[-1],
                                            "record-component", component.strip(),
                                            extraction="source-declared-generated-accessor"))
            elif kind == "enum":
                end, nested = pos + 1, 0
                while end < len(clean):
                    current = clean[end]
                    if (current == ";" or current == "}") and nested == 0:
                        break
                    nested += (current in "({[") - (current in ")}]" )
                    end += 1
                constants = clean[pos + 1:end]
                for offset, constant in split_components(constants):
                    found = re.match(r"\s*(" + IDENT + r")", strip_annotations(constant))
                    if found:
                        const_line = clean.count("\n", 0, pos + 1 + offset + found.start(1)) + 1
                        result.append(entry(category, path, const_line, symbol + "." + found.group(1),
                                            "enum-constant", constant.strip()))
        if char == "{":
            stack.append({"kind": kind if type_match else "body", "name": name,
                          "visible": visible if type_match else False})
        elif char == "}" and stack:
            stack.pop()
        start = pos + 1
    return result


def kotlin_declarations(path, text, category="kotlin_api"):
    """Conservative line/indentation source declarations (not JVM ABI)."""
    clean = mask(text)
    package = re.search(r"(?m)^package\s+([\w.]+)", clean)
    package = package.group(1) + "." if package else ""
    result = []
    owners = []
    offset = 0
    constructor_items = []
    # Keep constructor properties separately, even when declared private: protocol
    # payloads use private requestId and still expose the wire field.
    pattern = re.compile(
        r"^( *)(?P<mods>(?:(?:public|private|protected|internal|open|abstract|final|sealed|data|"
        r"enum|annotation|override|inline|tailrec|operator|infix|suspend|const|lateinit|external|actual|expect)\s+)*)"
        r"(?P<kind>class|interface|object|fun|val|var|typealias)\s+(?P<tail>.+)$"
    )
    for line_no, line in enumerate(clean.splitlines(keepends=True), 1):
        line_offset = offset
        offset += len(line)
        match = pattern.match(line)
        if not match:
            continue
        indent = len(match.group(1))
        mods, kind, tail = match.group("mods", "kind", "tail")
        while owners and indent <= owners[-1][0]:
            owners.pop()
        top_or_member = indent == 0 or (owners and indent == owners[-1][0] + 2)
        public = not set(mods.split()) & {"private", "protected", "internal"}
        public = public and all(owner[2] for owner in owners)
        if kind == "fun":
            # Keep receiver and type parameter text in overload signature.
            before = tail.split("(", 1)[0]
            names = re.findall(IDENT, before)
            name = names[-1] if names else None
        else:
            found = re.match(IDENT, tail)
            name = found.group() if found else None
        if name and public and top_or_member:
            symbol = package + ".".join([owner[1] for owner in owners] + [name.strip("`")])
            result.append(entry(category, path, line_no, symbol, kind, line.strip(),
                                extraction="source-candidate"))
        if kind in TYPE_WORDS and name and top_or_member:
            owners.append((indent, name, public))
            constructor = re.match(IDENT + r"(?:\s*<[^>]*>)?\s*\(", tail)
            if kind == "class" and public and constructor:
                start = line_offset + match.start("tail") + constructor.end()
                end, depth = start, 1
                while end < len(clean) and depth:
                    depth += (clean[end] == "(") - (clean[end] == ")")
                    end += 1
                for part_offset, component in split_components(clean[start:end - 1]):
                    property_match = re.search(r"\b(val|var)\s+(" + IDENT + r")", component)
                    if property_match and not re.search(r"\b(private|protected|internal)\b", component[:property_match.start()]):
                        component_line = clean.count("\n", 0, start + part_offset + property_match.start()) + 1
                        symbol = package + ".".join([owner[1] for owner in owners] + [property_match.group(2)])
                        constructor_items.append(entry(category, path, component_line, symbol,
                                                       property_match.group(1), component.strip(),
                                                       extraction="constructor-property"))
    existing = {item["symbol"] for item in result}
    result.extend(item for item in constructor_items if item["symbol"] not in existing)
    return result


def builtin_declarations(path, text, texts):
    clean = mask(text, strings=False)
    module = Path(path).parent.name
    package_path = str(Path(path).parent / "package-info.java")
    package_match = re.search(r'@PklName\("([^"]+)"\)', texts.get(package_path, ""))
    module = package_match.group(1) if package_match else module
    outer = re.search(r'(?:@PklName\("([^"]+)"\)\s*)?(?:public\s+)?(?:final\s+)?class\s+(\w+)Nodes\b', clean)
    if not outer:
        return []
    owner = outer.group(1) or outer.group(2)
    result = []
    pattern = re.compile(
        r'(?:@PklName\("([^"]+)"\)\s*)?public\s+(?:abstract\s+static|static\s+abstract)\s+class\s+(\w+)'
        r'\s+extends\s+(\w+)\b[^\{]*\{'
    )
    matches = list(pattern.finditer(clean))
    parents = {match.group(2): match.group(3) for match in matches}
    opaque = mask(text)
    for match in matches:
        annotation, name, node = match.groups()
        while node in parents:
            node = parents[node]
        if not re.fullmatch(r"External(?:Property|Method\d)Node", node):
            continue
        end, depth = match.end(), 1
        while end < len(clean) and depth:
            depth += (opaque[end] == "{") - (opaque[end] == "}")
            end += 1
        if "@Specialization" not in clean[match.end():end]:
            continue  # Shared helper classes do not produce registered factories.
        member = annotation or name
        symbol = "pkl." + module + "#" + ("" if owner == module[:1].upper() + module[1:] else owner + ".") + member
        result.append(entry("builtin", path, clean.count("\n", 0, match.start()) + 1,
                            symbol, node, match.group(), registration="source-derived"))
    return result


def specific_declarations(path, text):
    result = []
    clean = mask(text, strings=False)
    if path.endswith("/parser/Token.java"):
        body = mask(text).split("public enum Token {", 1)[1].split(";", 1)[0]
        for match in re.finditer(r"\b[A-Z][A-Z_]*\b", body):
            offset = mask(text).index("public enum Token {") + len("public enum Token {") + match.start()
            result.append(entry("language_token", path, text.count("\n", 0, offset) + 1,
                                match.group(), "token"))
    if re.search(r"/parser/(?:Generic)?Parser(?:Impl)?\.java$|/parser/Lexer\.java$", path):
        for match in re.finditer(r"(?m)^  (?:(?:public|private|protected|static|final)\s+)*[\w<>?, .\[\]]+\s+((?:parse|lex|next|skip)\w*)\s*\([^)]*\)\s*(?:throws[^\{]+)?\{", mask(text)):
            result.append(entry("language_parser", path, text.count("\n", 0, match.start()) + 1,
                                match.group(1), "production-or-lexer-routine", match.group()))
    if path.startswith("docs/modules/") and path.endswith(".adoc"):
        for match in re.finditer(r"(?m)^(={1,6})\s+(.+)$", text):
            result.append(entry("specification", path, text.count("\n", 0, match.start()) + 1,
                                match.group(2), "heading", level=len(match.group(1))))
    if path.endswith("/messaging/Message.java"):
        for match in re.finditer(r"\b([A-Z][A-Z_]+)\((0x[0-9a-f]+)\)", clean):
            result.append(entry("protocol_tag", path, text.count("\n", 0, match.start()) + 1,
                                match.group(1), "message-code", wire_code=match.group(2)))
    if path.endswith("/OutputFormat.java"):
        for match in re.finditer(r'\b([A-Z][A-Z_]+)\("([^"]+)"\)', clean):
            result.append(entry("renderer", path, text.count("\n", 0, match.start()) + 1,
                                match.group(2), "output-format", enum_name=match.group(1)))
    if path.startswith("stdlib/"):
        for match in re.finditer(r"(?m)^class\s+(\w+)\s+extends\s+(ValueRenderer|BytesRenderer)\b", mask(text)):
            result.append(entry("renderer", path, text.count("\n", 0, match.start()) + 1,
                                "pkl." + Path(path).stem + "#" + match.group(1),
                                "renderer-class", match.group()))
    if "MessagePack" in path and Path(path).suffix in {".java", ".kt"}:
        # Retain every literal field key per codec source. Message-specific field
        # order/optionality remains in the linked encoder/decoder and specification.
        keys = {}
        for match in re.finditer(r'"([a-z][A-Za-z0-9]+)"', clean):
            keys.setdefault(match.group(1), match.start())
        for key, offset in sorted(keys.items()):
            result.append(entry("protocol_field", path, text.count("\n", 0, offset) + 1,
                                key, "codec-key-candidate"))
    if path.endswith(".kt") and ("/commands/" in path or path.endswith("/Main.kt")):
        for match in re.finditer(r"\b[A-Z]\w*Command\s*\(", clean):
            end, depth = match.end(), 1
            opaque = mask(text)
            while end < len(clean) and depth:
                depth += (opaque[end] == "(") - (opaque[end] == ")")
                end += 1
            call = clean[match.start():end]
            name = re.search(r'\bname\s*=\s*"([\w-]+)"', call)
            if name:
                result.append(entry("cli_command", path, text.count("\n", 0, match.start()) + 1,
                                    name.group(1), "command-name", call, extraction="source-candidate"))
        # All literal aliases at option/flag/help/version calls, including negative
        # aliases; scan balanced calls so help strings are not mistaken for options.
    if path.endswith(".kt") and ("/commands/" in path or "/Main.kt" in path):
        for match in re.finditer(r"\b(option|flag|helpOption|versionOption|argument)\s*\(", clean):
            end, depth = match.end(), 1
            opaque = mask(text)
            while end < len(clean) and depth:
                depth += (opaque[end] == "(") - (opaque[end] == ")")
                end += 1
            call = clean[match.start():end]
            if match.group(1) == "argument":
                name = re.search(r'\bname\s*=\s*"([\w-]+)"', call)
                if name:
                    result.append(entry("cli_argument", path, text.count("\n", 0, match.start()) + 1,
                                        name.group(1), "positional-argument", call))
                continue
            config_end = clean.find("\n\n", end)
            configuration = clean[match.start():config_end if config_end >= 0 else end]
            for alias in re.finditer(r'"(-{1,2}[\w-]+)"', call):
                result.append(entry("cli_option", path, text.count("\n", 0, match.start()) + 1,
                                    alias.group(1), match.group(1), call,
                                    configuration=" ".join(configuration.split())))
    return result


def selected(path):
    """All tracked production source files plus reference specifications."""
    return (("/src/main/" in path or "/src/generator/" in path)
            or path.startswith("stdlib/") and path.endswith(".pkl")
            or path.startswith("docs/modules/") and path.endswith(".adoc")
            and any(f"/{module}/" in path for module in (
                "language-reference", "bindings-specification", "pkl-cli", "java-binding",
                "kotlin-binding", "pkl-doc", "pkl-codegen-java", "pkl-codegen-kotlin")))


def generate(root=ROOT):
    pins = dict(line.split("=", 1) for line in (root / "references.lock").read_text().splitlines()
                if line and not line.startswith("#"))
    upstream = root / pins["pkl.path"]
    revision = subprocess.check_output(["git", "-C", str(upstream), "rev-parse", "HEAD"], text=True).strip()
    if revision != pins["pkl.revision"]:
        raise ValueError("Pkl checkout HEAD does not match references.lock")
    files = subprocess.check_output(["git", "-C", str(upstream), "ls-files", "--cached", "--others", "--exclude-standard", "-z"]).decode().split("\0")
    paths = sorted(path for path in files if path and selected(path))
    if not paths:
        raise ValueError("empty upstream production-source census")
    data = {path: (upstream / path).read_bytes() for path in paths}
    texts = {path: content.decode("utf-8") if Path(path).suffix in {
        ".java", ".kt", ".pkl", ".adoc", ".properties", ".json", ".kts"} else ""
             for path, content in data.items()}
    items = []
    sources = []
    for path, text in texts.items():
        source_items = []
        if path.startswith("stdlib/"):
            source_items.extend(pkl_declarations(path, text))
        if path.endswith(".java"):
            # Runtime/AST internals are evidence in the census, not a promise of
            # stable JVM API. Surface candidates are explicitly labeled by module.
            source_items.extend(java_declarations(path, text))
        elif path.endswith(".kt"):
            source_items.extend(kotlin_declarations(path, text))
        if "/stdlib/" in path and path.endswith("Nodes.java"):
            source_items.extend(builtin_declarations(path, text, texts))
        source_items.extend(specific_declarations(path, text))
        if "/src/main/resources/" in path:
            source_items.append(entry("resource", path, 1, path, "production-resource"))
        for item in source_items:
            item["module"] = path.split("/", 1)[0]
            if item["category"] in {"java_api", "kotlin_api"}:
                item["api_scope"] = ("internal-source-candidate" if any(
                    component in path for component in ("/ast/", "/runtime/", "/stdlib/", "/util/", "/src/generator/"))
                    else "source-candidate")
        items.extend(source_items)
        sources.append({"path": path, "sha256": sha(data[path]),
                        "module": path.split("/", 1)[0],
                        "item_counts": dict(sorted(Counter(item["category"] for item in source_items).items()))})
    # Repeated headings and repeated CLI option declaration calls retain distinct
    # location identities; overload signatures remain stable across line movement.
    counts = Counter(item["id"] for item in items)
    for item in items:
        if counts[item["id"]] > 1:
            item["id"] += ":line" + str(item["source"]["line"])
    if len({item["id"] for item in items}) != len(items):
        raise ValueError("duplicate inventory identity")
    def member_key(item):
        kind = ("property" if item["kind"] == "ExternalPropertyNode" else "function") if item["category"] == "builtin" else item["kind"]
        return item["symbol"] + " [" + kind + "]"

    builtins = {member_key(item): item["id"] for item in items if item["category"] == "builtin"}
    external_members = {member_key(item): item["id"] for item in items
                        if item["category"] == "stdlib" and item.get("external")
                        and item["kind"] in {"property", "function"}}
    for item in items:
        if item["id"] in external_members.values():
            key = member_key(item)
            item["upstream_builtin"] = {"state": "matched" if key in builtins else "unmatched",
                                        "item_id": builtins.get(key)}
        elif item["category"] == "builtin":
            key = member_key(item)
            item["upstream_declaration"] = {"state": "matched" if key in external_members else "unmatched",
                                            "item_id": external_members.get(key)}
    return {
        "schema": 1,
        "baseline": {"version": pins["pkl.version"], "url": pins["pkl.url"], "revision": revision,
                     "checkout": pins["pkl.path"],
                     "source_url_template": pins["pkl.url"].removesuffix(".git") + "/blob/" + revision + "/{path}#L{line}"},
        "contract": "docs/inventory.md",
        "status": "source inventory; no implementation or semantic compatibility coverage",
        "counts": {"sources": len(sources), "items": len(items),
                   "by_category": dict(sorted(Counter(item["category"] for item in items).items())),
                   "sources_by_module": dict(sorted(Counter(source["module"] for source in sources).items())),
                   "stdlib_by_visibility": dict(sorted(Counter(item.get("visibility", "public")
                       for item in items if item["category"] == "stdlib").items())),
                   "api_by_scope": dict(sorted(Counter(item["api_scope"] for item in items
                       if "api_scope" in item).items())),
                   "external_members": len(external_members),
                   "external_members_without_builtin": sorted(set(external_members) - set(builtins)),
                   "builtins_without_external_member": sorted(set(builtins) - set(external_members))},
        "sources": sources,
        "items": sorted(items, key=lambda item: item["id"]),
    }


def encoded(inventory):
    return (json.dumps(inventory, ensure_ascii=False, indent=2) + "\n").encode()


def check_bytes(path, expected):
    if not path.is_file():
        raise ValueError(f"missing inventory: {path}")
    if path.read_bytes() != expected:
        raise ValueError(f"inventory is stale or incomplete: {path}; run python3 tools/inventory.py")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true", help="fail on missing/stale inventory without writing")
    parser.add_argument("--output", type=Path, default=ROOT / "compatibility/inventory.json")
    args = parser.parse_args()
    try:
        inventory = generate()
        data = encoded(inventory)
        if args.check:
            check_bytes(args.output, data)
        else:
            args.output.parent.mkdir(parents=True, exist_ok=True)
            args.output.write_bytes(data)
        print(json.dumps(inventory["counts"], sort_keys=True))
    except (OSError, ValueError, subprocess.CalledProcessError) as error:
        print(f"inventory: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
