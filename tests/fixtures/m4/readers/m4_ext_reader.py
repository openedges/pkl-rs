#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Deterministic pure-Python stdlib external module & resource reader fixture for M4.

Implements Pkl 0.32.0 MessagePack wire codes 0x26-0x32 with key-omission nullability.
Python 3.9+ compatible so it works under minimal PATH (/usr/bin:/bin) via env python3.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import sys
import time
from urllib.parse import urlsplit

sys.path.insert(0, str(Path(__file__).resolve().parent))
import msgpack_ipc as mp

MODULE_SPECS = {
    "extmod": {
        "scheme": "extmod",
        "hasHierarchicalUris": True,
        "isLocal": True,
        "isGlobbable": True,
    },
    "extmodremote": {
        "scheme": "extmodremote",
        "hasHierarchicalUris": True,
        "isLocal": False,
        "isGlobbable": False,
    },
    "extmodopaque": {
        "scheme": "extmodopaque",
        "hasHierarchicalUris": False,
        "isLocal": True,
        "isGlobbable": True,
    },
}

RESOURCE_SPECS = {
    "extres": {
        "scheme": "extres",
        "hasHierarchicalUris": True,
        "isGlobbable": True,
    },
    "extresopaque": {
        "scheme": "extresopaque",
        "hasHierarchicalUris": False,
        "isGlobbable": True,
    },
    "extresnoglob": {
        "scheme": "extresnoglob",
        "hasHierarchicalUris": True,
        "isGlobbable": False,
    },
}

# Indexed by (scheme, path) for hierarchical URIs so both `scheme:///a` and `scheme:/a` resolve identically.
VIRTUAL_HIER_MODULES = {
    ("extmod", "/root.pkl"): 'source = "extmod-root"\nval = 42\n',
    ("extmod", "/dir/base.pkl"): 'kind = "base-from-parent"\n',
    ("extmod", "/dir/sub/child.pkl"): 'import ".../base.pkl"\nimport "../sibling.pkl"\nfromBase = base.kind\nfromSibling = sibling.label\n',
    ("extmod", "/dir/sibling.pkl"): 'label = "sibling-ok"\n',
    ("extmod", "/glob/a.pkl"): 'name = "alpha"\n',
    ("extmod", "/glob/b.pkl"): 'name = "beta"\n',
    ("extmod", "/glob/nested/c.pkl"): 'name = "gamma"\n',
    ("extmodremote", "/remote.pkl"): 'origin = "remote-nonlocal"\n',
}

VIRTUAL_OPAQUE_MODULES = {
    "extmodopaque:alpha": 'opaqueName = "alpha"\n',
    "extmodopaque:beta": 'opaqueName = "beta"\n',
}

VIRTUAL_HIER_MODULE_DIRS = {
    ("extmod", "/"): [
        {"name": "dir", "isDirectory": True},
        {"name": "glob", "isDirectory": True},
        {"name": "root.pkl", "isDirectory": False},
    ],
    ("extmod", "/glob"): [
        {"name": "b.pkl", "isDirectory": False},
        {"name": "a.pkl", "isDirectory": False},
        {"name": "nested", "isDirectory": True},
    ],
    ("extmod", "/glob/nested"): [
        {"name": "c.pkl", "isDirectory": False},
    ],
}

VIRTUAL_HIER_RESOURCES = {
    ("extres", "/hello.txt"): b"hello from extres\n",
    ("extres", "/items/one.txt"): b"item-1",
    ("extres", "/items/two.txt"): b"item-2",
    ("extres", "/binary.bin"): b"\x00\x01\x02\xff",
    ("extresnoglob", "/single.txt"): b"noglob-ok",
}

VIRTUAL_OPAQUE_RESOURCES = {
    "extresopaque:alpha": b"opaque-alpha",
    "extresopaque:beta": b"opaque-beta",
}

VIRTUAL_HIER_RESOURCE_DIRS = {
    ("extres", "/items"): [
        {"name": "two.txt", "isDirectory": False},
        {"name": "one.txt", "isDirectory": False},
    ],
}


def pkl_str(text: str) -> str:
    escaped = (
        text.replace("\\", "\\\\")
        .replace('"', '\\"')
        .replace("\n", "\\n")
        .replace("\r", "\\r")
        .replace("\t", "\\t")
    )
    return f'"{escaped}"'


def norm_hier_key(uri: str) -> tuple[str, str]:
    parsed = urlsplit(uri)
    path = parsed.path or "/"
    if len(path) > 1 and path.endswith("/"):
        path = path.rstrip("/")
    return parsed.scheme.lower(), path


def append_lifecycle(log_path: str | None, event: dict) -> None:
    if not log_path:
        return
    p = Path(log_path)
    p.parent.mkdir(parents=True, exist_ok=True)
    with p.open("a", encoding="utf-8") as fh:
        fh.write(json.dumps(event, sort_keys=True) + "\n")


def parse_args(argv: list[str]) -> dict:
    cfg: dict = {"lifecycle_log": None, "mode": "normal", "tag": "default", "extra": []}
    i = 0
    while i < len(argv):
        arg = argv[i]
        if arg == "--lifecycle-log" and i + 1 < len(argv):
            cfg["lifecycle_log"] = argv[i + 1]
            i += 2
        elif arg == "--mode" and i + 1 < len(argv):
            cfg["mode"] = argv[i + 1]
            i += 2
        elif arg == "--tag" and i + 1 < len(argv):
            cfg["tag"] = argv[i + 1]
            i += 2
        else:
            cfg["extra"].append(arg)
            i += 1
    return cfg


def handle_read_module(uri: str, cfg: dict) -> dict:
    if cfg["mode"] == "crash-on-read":
        sys.exit(77)
    if uri in VIRTUAL_OPAQUE_MODULES:
        return {"contents": VIRTUAL_OPAQUE_MODULES[uri]}
    key = norm_hier_key(uri)
    if key == ("extmod", "/fail.pkl"):
        return {"error": "simulated external module read failure"}
    if key == ("extmod", "/info.pkl"):
        cwd_name = Path(os.getcwd()).name
        extra_pkl = ", ".join(pkl_str(x) for x in cfg["extra"])
        content = (
            f"tag = {pkl_str(cfg['tag'])}\n"
            f"cwdName = {pkl_str(cwd_name)}\n"
            f"extraArgs = List({extra_pkl})\n"
        )
        return {"contents": content}
    if key == ("extmod", "/abs-cwd.pkl"):
        content = f"cwd = {pkl_str(str(Path(os.getcwd()).resolve()))}\ntag = {pkl_str(cfg['tag'])}\n"
        return {"contents": content}
    if key in VIRTUAL_HIER_MODULES:
        return {"contents": VIRTUAL_HIER_MODULES[key]}
    return {"error": f"module not found: {uri}"}


def handle_read_resource(uri: str, cfg: dict) -> dict:
    if cfg["mode"] == "crash-on-read":
        sys.exit(77)
    if uri in VIRTUAL_OPAQUE_RESOURCES:
        return {"contents": VIRTUAL_OPAQUE_RESOURCES[uri]}
    key = norm_hier_key(uri)
    if key == ("extres", "/missing.txt"):
        # Deliberately omit both `contents` and `error` to exercise Pkl 0.32.0 Missing semantics.
        return {}
    if key == ("extres", "/err.txt"):
        return {"error": "simulated external resource read failure"}
    if key == ("extres", "/info.txt"):
        cwd_name = Path(os.getcwd()).name
        payload = f"tag={cfg['tag']};cwd={cwd_name};extra={','.join(cfg['extra'])}"
        return {"contents": payload.encode("utf-8")}
    if key in VIRTUAL_HIER_RESOURCES:
        return {"contents": VIRTUAL_HIER_RESOURCES[key]}
    return {}


def main() -> int:
    cfg = parse_args(sys.argv[1:])
    stdin = sys.stdin.buffer
    stdout = sys.stdout.buffer
    append_lifecycle(
        cfg["lifecycle_log"],
        {"event": "start", "pid": os.getpid(), "cwd": str(Path(os.getcwd()).resolve()), "tag": cfg["tag"]},
    )

    while True:
        msg = mp.read_message(stdin)
        if msg is None:
            append_lifecycle(cfg["lifecycle_log"], {"event": "eof", "pid": os.getpid()})
            return 0
        code, body = msg
        if code == mp.CLOSE_EXTERNAL_PROCESS:
            append_lifecycle(cfg["lifecycle_log"], {"event": "close", "pid": os.getpid()})
            if cfg["mode"] == "hang-close":
                time.sleep(30)
            return 0
        if code == mp.INITIALIZE_MODULE_READER_REQUEST:
            scheme = body["scheme"]
            resp = {"requestId": body["requestId"]}
            if scheme in MODULE_SPECS:
                resp["spec"] = MODULE_SPECS[scheme]
            mp.write_message(stdout, mp.INITIALIZE_MODULE_READER_RESPONSE, resp)
        elif code == mp.INITIALIZE_RESOURCE_READER_REQUEST:
            scheme = body["scheme"]
            resp = {"requestId": body["requestId"]}
            if scheme in RESOURCE_SPECS:
                resp["spec"] = RESOURCE_SPECS[scheme]
            mp.write_message(stdout, mp.INITIALIZE_RESOURCE_READER_RESPONSE, resp)
        elif code == mp.READ_MODULE_REQUEST:
            res = handle_read_module(body["uri"], cfg)
            resp = {"requestId": body["requestId"], "evaluatorId": body["evaluatorId"]}
            resp.update(res)
            mp.write_message(stdout, mp.READ_MODULE_RESPONSE, resp)
        elif code == mp.READ_RESOURCE_REQUEST:
            res = handle_read_resource(body["uri"], cfg)
            resp = {"requestId": body["requestId"], "evaluatorId": body["evaluatorId"]}
            resp.update(res)
            mp.write_message(stdout, mp.READ_RESOURCE_RESPONSE, resp)
        elif code == mp.LIST_MODULES_REQUEST:
            uri = body["uri"]
            scheme = urlsplit(uri).scheme.lower()
            resp = {"requestId": body["requestId"], "evaluatorId": body["evaluatorId"]}
            if scheme == "extmodopaque":
                resp["pathElements"] = [
                    {"name": "beta", "isDirectory": False},
                    {"name": "alpha", "isDirectory": False},
                ]
            else:
                key = norm_hier_key(uri)
                resp["pathElements"] = VIRTUAL_HIER_MODULE_DIRS.get(key, [])
            mp.write_message(stdout, mp.LIST_MODULES_RESPONSE, resp)
        elif code == mp.LIST_RESOURCES_REQUEST:
            uri = body["uri"]
            scheme = urlsplit(uri).scheme.lower()
            resp = {"requestId": body["requestId"], "evaluatorId": body["evaluatorId"]}
            if scheme == "extresopaque":
                resp["pathElements"] = [
                    {"name": "beta", "isDirectory": False},
                    {"name": "alpha", "isDirectory": False},
                ]
            else:
                key = norm_hier_key(uri)
                resp["pathElements"] = VIRTUAL_HIER_RESOURCE_DIRS.get(key, [])
            mp.write_message(stdout, mp.LIST_RESOURCES_RESPONSE, resp)
        else:
            append_lifecycle(cfg["lifecycle_log"], {"event": "unknown-code", "code": code, "pid": os.getpid()})
            return 2


if __name__ == "__main__":
    raise SystemExit(main())
