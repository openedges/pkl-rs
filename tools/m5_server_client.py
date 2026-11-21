#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Pure Python stdlib MessagePack client for M5 Lane 3 (`pkl server`, 0x20-0x32).

Drives both pinned Java 0.32.0 (`org.pkl.cli.Main server`) and Rust (`pkl server`)
through deterministic scripted protocol sessions over stdin/stdout pipes.

Opaque server-minted IDs (`CreateEvaluatorResponse.evaluatorId` and server-initiated
callback `requestId` in 0x26/0x28/0x2a/0x2c) are validated, bound dynamically per
session, and canonicalized ONLY into deterministic slot labels (`$eval:N`, `$cb_req:N`)
after asserting every echo matches the exact minted integer. Every wire code, map
key set (omitted vs present), binary `result` payload (hex-encoded raw bytes), error
string, `LogMessage`, callback URI, stderr byte, and exit code is preserved verbatim
for exact raw comparison.
"""
from __future__ import annotations

import hashlib
import json
import os
from pathlib import Path
import queue
import signal
import struct
import subprocess
import threading
import time
from typing import Any, BinaryIO

# All 19 Pkl 0.32.0 message codes from org.pkl.core.messaging.Message
CREATE_EVALUATOR_REQUEST = 0x20
CREATE_EVALUATOR_RESPONSE = 0x21
CLOSE_EVALUATOR = 0x22
EVALUATE_REQUEST = 0x23
EVALUATE_RESPONSE = 0x24
LOG_MESSAGE = 0x25
READ_RESOURCE_REQUEST = 0x26
READ_RESOURCE_RESPONSE = 0x27
READ_MODULE_REQUEST = 0x28
READ_MODULE_RESPONSE = 0x29
LIST_RESOURCES_REQUEST = 0x2A
LIST_RESOURCES_RESPONSE = 0x2B
LIST_MODULES_REQUEST = 0x2C
LIST_MODULES_RESPONSE = 0x2D
INITIALIZE_MODULE_READER_REQUEST = 0x2E
INITIALIZE_MODULE_READER_RESPONSE = 0x2F
INITIALIZE_RESOURCE_READER_REQUEST = 0x30
INITIALIZE_RESOURCE_READER_RESPONSE = 0x31
CLOSE_EXTERNAL_PROCESS = 0x32

ALL_MESSAGE_CODES = {
    CREATE_EVALUATOR_REQUEST,
    CREATE_EVALUATOR_RESPONSE,
    CLOSE_EVALUATOR,
    EVALUATE_REQUEST,
    EVALUATE_RESPONSE,
    LOG_MESSAGE,
    READ_RESOURCE_REQUEST,
    READ_RESOURCE_RESPONSE,
    READ_MODULE_REQUEST,
    READ_MODULE_RESPONSE,
    LIST_RESOURCES_REQUEST,
    LIST_RESOURCES_RESPONSE,
    LIST_MODULES_REQUEST,
    LIST_MODULES_RESPONSE,
    INITIALIZE_MODULE_READER_REQUEST,
    INITIALIZE_MODULE_READER_RESPONSE,
    INITIALIZE_RESOURCE_READER_REQUEST,
    INITIALIZE_RESOURCE_READER_RESPONSE,
    CLOSE_EXTERNAL_PROCESS,
}

CALLBACK_REQUEST_CODES = {
    READ_RESOURCE_REQUEST,
    READ_MODULE_REQUEST,
    LIST_RESOURCES_REQUEST,
    LIST_MODULES_REQUEST,
}


# ---------------------------------------------------------------------------
# Pure Python stdlib MessagePack codec (with raw wire byte tap)
# ---------------------------------------------------------------------------

class TapReader:
    """Wraps a BinaryIO stream and records every raw byte read."""

    def __init__(self, stream: BinaryIO):
        self.stream = stream
        self.raw = bytearray()

    def read(self, n: int) -> bytes:
        chunk = self.stream.read(n)
        if chunk:
            self.raw.extend(chunk)
        return chunk


def _read_exact(stream: Any, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = stream.read(n - len(buf))
        if not chunk:
            raise EOFError("unexpected EOF in MessagePack stream")
        buf.extend(chunk)
    return bytes(buf)


def _unpack_with_prefix(stream: Any, b: int) -> Any:
    if b <= 0x7F:
        return b
    if b >= 0xE0:
        return b - 0x100
    if 0x80 <= b <= 0x8F:
        return _read_map(stream, b & 0x0F)
    if 0x90 <= b <= 0x9F:
        return [_unpack_next(stream) for _ in range(b & 0x0F)]
    if 0xA0 <= b <= 0xBF:
        return _read_exact(stream, b & 0x1F).decode("utf-8")
    if b == 0xC0:
        return None
    if b == 0xC2:
        return False
    if b == 0xC3:
        return True
    if b == 0xC4:
        return _read_exact(stream, _read_exact(stream, 1)[0])
    if b == 0xC5:
        (n,) = struct.unpack(">H", _read_exact(stream, 2))
        return _read_exact(stream, n)
    if b == 0xC6:
        (n,) = struct.unpack(">I", _read_exact(stream, 4))
        return _read_exact(stream, n)
    if b == 0xCA:
        return struct.unpack(">f", _read_exact(stream, 4))[0]
    if b == 0xCB:
        return struct.unpack(">d", _read_exact(stream, 8))[0]
    if b == 0xCC:
        return _read_exact(stream, 1)[0]
    if b == 0xCD:
        return struct.unpack(">H", _read_exact(stream, 2))[0]
    if b == 0xCE:
        return struct.unpack(">I", _read_exact(stream, 4))[0]
    if b == 0xCF:
        return struct.unpack(">Q", _read_exact(stream, 8))[0]
    if b == 0xD0:
        return struct.unpack(">b", _read_exact(stream, 1))[0]
    if b == 0xD1:
        return struct.unpack(">h", _read_exact(stream, 2))[0]
    if b == 0xD2:
        return struct.unpack(">i", _read_exact(stream, 4))[0]
    if b == 0xD3:
        return struct.unpack(">q", _read_exact(stream, 8))[0]
    if b == 0xD9:
        return _read_exact(stream, _read_exact(stream, 1)[0]).decode("utf-8")
    if b == 0xDA:
        (n,) = struct.unpack(">H", _read_exact(stream, 2))
        return _read_exact(stream, n).decode("utf-8")
    if b == 0xDB:
        (n,) = struct.unpack(">I", _read_exact(stream, 4))
        return _read_exact(stream, n).decode("utf-8")
    if b == 0xDC:
        (n,) = struct.unpack(">H", _read_exact(stream, 2))
        return [_unpack_next(stream) for _ in range(n)]
    if b == 0xDD:
        (n,) = struct.unpack(">I", _read_exact(stream, 4))
        return [_unpack_next(stream) for _ in range(n)]
    if b == 0xDE:
        (n,) = struct.unpack(">H", _read_exact(stream, 2))
        return _read_map(stream, n)
    if b == 0xDF:
        (n,) = struct.unpack(">I", _read_exact(stream, 4))
        return _read_map(stream, n)
    raise ValueError(f"unsupported MessagePack prefix byte: 0x{b:02x}")


def _unpack_next(stream: Any) -> Any:
    return _unpack_with_prefix(stream, _read_exact(stream, 1)[0])


def _read_map(stream: Any, count: int) -> dict[Any, Any]:
    result: dict[Any, Any] = {}
    for _ in range(count):
        k = _unpack_next(stream)
        v = _unpack_next(stream)
        result[k] = v
    return result


def pack_value(value: Any, *, omit_none: bool = True) -> bytes:
    if value is None:
        return b"\xc0"
    if isinstance(value, bool):
        return b"\xc3" if value else b"\xc2"
    if isinstance(value, int):
        if 0 <= value <= 0x7F:
            return bytes([value])
        if -32 <= value < 0:
            return bytes([value & 0xFF])
        if 0 <= value <= 0xFF:
            return b"\xcc" + bytes([value])
        if 0 <= value <= 0xFFFF:
            return b"\xcd" + struct.pack(">H", value)
        if 0 <= value <= 0xFFFFFFFF:
            return b"\xce" + struct.pack(">I", value)
        if 0 <= value <= 0xFFFFFFFFFFFFFFFF:
            return b"\xcf" + struct.pack(">Q", value)
        if -128 <= value < 0:
            return b"\xd0" + struct.pack(">b", value)
        if -32768 <= value < 0:
            return b"\xd1" + struct.pack(">h", value)
        if -2147483648 <= value < 0:
            return b"\xd2" + struct.pack(">i", value)
        return b"\xd3" + struct.pack(">q", value)
    if isinstance(value, float):
        return b"\xcb" + struct.pack(">d", value)
    if isinstance(value, (bytes, bytearray)):
        raw = bytes(value)
        n = len(raw)
        if n <= 0xFF:
            return b"\xc4" + bytes([n]) + raw
        if n <= 0xFFFF:
            return b"\xc5" + struct.pack(">H", n) + raw
        return b"\xc6" + struct.pack(">I", n) + raw
    if isinstance(value, str):
        raw = value.encode("utf-8")
        n = len(raw)
        if n <= 31:
            return bytes([0xA0 | n]) + raw
        if n <= 0xFF:
            return b"\xd9" + bytes([n]) + raw
        if n <= 0xFFFF:
            return b"\xda" + struct.pack(">H", n) + raw
        return b"\xdb" + struct.pack(">I", n) + raw
    if isinstance(value, (list, tuple)):
        n = len(value)
        if n <= 15:
            header = bytes([0x90 | n])
        elif n <= 0xFFFF:
            header = b"\xdc" + struct.pack(">H", n)
        else:
            header = b"\xdd" + struct.pack(">I", n)
        return header + b"".join(pack_value(item, omit_none=omit_none) for item in value)
    if isinstance(value, dict):
        items = [(k, v) for k, v in value.items() if (not omit_none or v is not None)]
        n = len(items)
        if n <= 15:
            header = bytes([0x80 | n])
        elif n <= 0xFFFF:
            header = b"\xde" + struct.pack(">H", n)
        else:
            header = b"\xdf" + struct.pack(">I", n)
        parts = [header]
        for k, v in items:
            parts.append(pack_value(k, omit_none=omit_none))
            parts.append(pack_value(v, omit_none=omit_none))
        return b"".join(parts)
    raise TypeError(f"unsupported MessagePack value type: {type(value).__name__}")


def read_message(stream: Any) -> tuple[int, dict[str, Any]] | None:
    first = stream.read(1)
    if not first:
        return None
    msg = _unpack_with_prefix(stream, first[0])
    if not isinstance(msg, list) or len(msg) != 2 or not isinstance(msg[1], dict):
        raise ValueError(f"expected [code, map] MessagePack array, got {msg!r}")
    return int(msg[0]), msg[1]


def write_message(stream: BinaryIO, code: int, body: dict[str, Any], *, omit_none: bool = True) -> bytes:
    payload = pack_value([code, body], omit_none=omit_none)
    stream.write(payload)
    stream.flush()
    return payload


def _encode_json_value(value: Any) -> Any:
    """Losslessly serialize decoded MessagePack values (tagging raw bytes as hex)."""
    if isinstance(value, (bytes, bytearray)):
        return {"__bytes_hex__": bytes(value).hex()}
    if isinstance(value, list):
        return [_encode_json_value(v) for v in value]
    if isinstance(value, dict):
        return {str(k): _encode_json_value(v) for k, v in sorted(value.items(), key=lambda kv: str(kv[0]))}
    return value


def _materialize_payload(obj: Any, placeholders: dict[str, str], eval_slots: dict[str, int]) -> Any:
    """Expand placeholders, `$eval:slot` handles, and `bytes_utf8`/`bytes_file`/`bytes_hex` descriptors."""
    if isinstance(obj, str):
        if obj in eval_slots:
            return eval_slots[obj]
        out = obj
        for k, v in placeholders.items():
            out = out.replace("{" + k + "}", v)
        return out
    if isinstance(obj, list):
        return [_materialize_payload(x, placeholders, eval_slots) for x in obj]
    if isinstance(obj, dict):
        if set(obj.keys()) == {"__bytes_utf8__"}:
            s = _materialize_payload(obj["__bytes_utf8__"], placeholders, eval_slots)
            return s.encode("utf-8")
        if set(obj.keys()) == {"__bytes_file__"}:
            p = _materialize_payload(obj["__bytes_file__"], placeholders, eval_slots)
            return Path(p).read_bytes()
        if set(obj.keys()) == {"__bytes_hex__"}:
            return bytes.fromhex(obj["__bytes_hex__"])
        return {k: _materialize_payload(v, placeholders, eval_slots) for k, v in obj.items()}
    return obj


def _build_callback_response(
    code: int,
    req: dict[str, Any],
    handlers: dict[str, Any],
    placeholders: dict[str, str],
) -> tuple[int, dict[str, Any]]:
    req_id = req["requestId"]
    eid = req["evaluatorId"]
    uri = req["uri"]

    if code == READ_RESOURCE_REQUEST:
        spec = (handlers.get("read_resource") or {}).get(uri)
        body: dict[str, Any] = {"requestId": req_id, "evaluatorId": eid}
        if spec is None or spec.get("missing"):
            return READ_RESOURCE_RESPONSE, body
        if "error" in spec:
            body["error"] = _materialize_payload(spec["error"], placeholders, {})
        elif "contents_utf8" in spec:
            body["contents"] = _materialize_payload(spec["contents_utf8"], placeholders, {}).encode("utf-8")
        elif "contents_hex" in spec:
            body["contents"] = bytes.fromhex(spec["contents_hex"])
        return READ_RESOURCE_RESPONSE, body

    if code == READ_MODULE_REQUEST:
        spec = (handlers.get("read_module") or {}).get(uri)
        body = {"requestId": req_id, "evaluatorId": eid}
        if spec is None or spec.get("missing"):
            return READ_MODULE_RESPONSE, body
        if "error" in spec:
            body["error"] = _materialize_payload(spec["error"], placeholders, {})
        elif "contents" in spec:
            body["contents"] = _materialize_payload(spec["contents"], placeholders, {})
        return READ_MODULE_RESPONSE, body

    if code == LIST_RESOURCES_REQUEST:
        spec = (handlers.get("list_resources") or {}).get(uri)
        body = {"requestId": req_id, "evaluatorId": eid}
        if spec is None or spec.get("missing"):
            return LIST_RESOURCES_RESPONSE, body
        if "error" in spec:
            body["error"] = _materialize_payload(spec["error"], placeholders, {})
        elif "pathElements" in spec:
            body["pathElements"] = spec["pathElements"]
        return LIST_RESOURCES_RESPONSE, body

    if code == LIST_MODULES_REQUEST:
        spec = (handlers.get("list_modules") or {}).get(uri)
        body = {"requestId": req_id, "evaluatorId": eid}
        if spec is None or spec.get("missing"):
            return LIST_MODULES_RESPONSE, body
        if "error" in spec:
            body["error"] = _materialize_payload(spec["error"], placeholders, {})
        elif "pathElements" in spec:
            body["pathElements"] = spec["pathElements"]
        return LIST_MODULES_RESPONSE, body

    raise ValueError(f"unsupported callback request code: 0x{code:02x}")


def run_server_session(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    scenario: dict[str, Any],
    placeholders: dict[str, str],
    timeout: float = 30.0,
) -> dict[str, Any]:
    """Execute one scripted `pkl server` MessagePack session and capture raw outputs."""
    if os.name != "posix":
        raise RuntimeError("M5 server driver requires POSIX process groups")

    before = time.perf_counter_ns()
    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=env,
        stdin=subprocess.PIPE,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    assert proc.stdin is not None and proc.stdout is not None and proc.stderr is not None

    tap = TapReader(proc.stdout)
    wire_in = bytearray()
    msg_queue: queue.Queue[tuple[str, Any]] = queue.Queue()

    def _reader_thread() -> None:
        try:
            while True:
                item = read_message(tap)
                if item is None:
                    msg_queue.put(("eof", None))
                    return
                msg_queue.put(("msg", item))
        except BaseException as exc:
            msg_queue.put(("err", f"{type(exc).__name__}: {exc}"))

    t_read = threading.Thread(target=_reader_thread, daemon=True)
    t_read.start()

    handlers = scenario.get("callbacks") or {}
    eval_slots: dict[str, int] = {}
    eid_to_slot: dict[int, str] = {}
    cb_req_counter = 0
    events: list[dict[str, Any]] = []
    protocol_errors: list[str] = []
    timed_out = False

    def _canonicalize_msg(code: int, body: dict[str, Any], *, is_callback_req: bool = False) -> dict[str, Any]:
        nonlocal cb_req_counter
        canon: dict[str, Any] = {}
        for k, v in sorted(body.items()):
            if k == "evaluatorId" and isinstance(v, int):
                if v in eid_to_slot:
                    canon[k] = eid_to_slot[v]
                else:
                    canon[k] = f"$unknown_eval:{v}"
            elif is_callback_req and k == "requestId":
                if not isinstance(v, int):
                    protocol_errors.append(f"non-int callback requestId: {v!r}")
                    canon[k] = v
                else:
                    label = f"$cb_req:{cb_req_counter}"
                    cb_req_counter += 1
                    canon[k] = label
            else:
                canon[k] = _encode_json_value(v)
        return {"code": f"0x{code:02x}", "keys": sorted(body.keys()), "body": canon}

    def _next_msg(deadline: float) -> tuple[int, dict[str, Any]] | None:
        nonlocal timed_out
        rem = deadline - time.monotonic()
        if rem <= 0:
            timed_out = True
            return None
        try:
            kind, payload = msg_queue.get(timeout=rem)
        except queue.Empty:
            timed_out = True
            return None
        if kind == "eof":
            return None
        if kind == "err":
            protocol_errors.append(f"reader_error:{payload}")
            return None
        return payload

    def _send(code: int, body: dict[str, Any], *, omit_none: bool = True) -> None:
        raw_out = write_message(proc.stdin, code, body, omit_none=omit_none)
        wire_in.extend(raw_out)

    deadline = time.monotonic() + timeout
    try:
        for step in scenario["steps"]:
            op = step["op"]

            if op == "create_evaluator":
                slot = step.get("slot")
                body = _materialize_payload(step["body"], placeholders, eval_slots)
                _send(CREATE_EVALUATOR_REQUEST, body)
                if step.get("expect_crash"):
                    continue
                msg = _next_msg(deadline)
                if msg is None:
                    protocol_errors.append(f"missing CreateEvaluatorResponse for slot={slot}")
                    break
                code, resp = msg
                if code == CREATE_EVALUATOR_RESPONSE and "evaluatorId" in resp and slot is not None:
                    eid = resp["evaluatorId"]
                    if not isinstance(eid, int) or eid == 0:
                        protocol_errors.append(f"invalid minted evaluatorId for {slot}: {eid!r}")
                    else:
                        eval_slots[slot] = eid
                        eid_to_slot[eid] = slot
                events.append({"dir": "recv", "step": op, "msg": _canonicalize_msg(code, resp)})

            elif op == "evaluate":
                body = _materialize_payload(step["body"], placeholders, eval_slots)
                _send(EVALUATE_REQUEST, body)
                while True:
                    msg = _next_msg(deadline)
                    if msg is None:
                        protocol_errors.append("missing EvaluateResponse before EOF/timeout")
                        break
                    code, resp = msg
                    if code in CALLBACK_REQUEST_CODES:
                        events.append({"dir": "recv", "step": "callback", "msg": _canonicalize_msg(code, resp, is_callback_req=True)})
                        rc, rbody = _build_callback_response(code, resp, handlers, placeholders)
                        _send(rc, rbody)
                        continue
                    if code == LOG_MESSAGE:
                        events.append({"dir": "recv", "step": "log", "msg": _canonicalize_msg(code, resp)})
                        continue
                    events.append({"dir": "recv", "step": op, "msg": _canonicalize_msg(code, resp)})
                    if code == EVALUATE_RESPONSE:
                        break
                if timed_out:
                    break

            elif op == "evaluate_pipelined":
                # Mirror AbstractServerTest.`concurrent evaluations`: send multiple EvaluateRequests
                # back-to-back on the same evaluatorId before servicing callbacks; verify FIFO ordering.
                reqs = [_materialize_payload(r, placeholders, eval_slots) for r in step["requests"]]
                for r in reqs:
                    _send(EVALUATE_REQUEST, r)
                remaining = len(reqs)
                while remaining > 0:
                    msg = _next_msg(deadline)
                    if msg is None:
                        protocol_errors.append(f"pipelined: missing {remaining} EvaluateResponse(s)")
                        break
                    code, resp = msg
                    if code in CALLBACK_REQUEST_CODES:
                        events.append({"dir": "recv", "step": "pipelined_cb", "msg": _canonicalize_msg(code, resp, is_callback_req=True)})
                        rc, rbody = _build_callback_response(code, resp, handlers, placeholders)
                        _send(rc, rbody)
                        continue
                    if code == LOG_MESSAGE:
                        events.append({"dir": "recv", "step": "pipelined_log", "msg": _canonicalize_msg(code, resp)})
                        continue
                    events.append({"dir": "recv", "step": op, "msg": _canonicalize_msg(code, resp)})
                    if code == EVALUATE_RESPONSE:
                        remaining -= 1
                if timed_out:
                    break

            elif op == "evaluate_interleaved_create":
                # Proves transport reader stays responsive while an EvaluateRequest is suspended on a callback:
                # 1. Send EvaluateRequest (`first`), wait for server callback matching `gate_uri` (hold it!).
                # 2. While held, send CreateEvaluatorRequest (`create_while_blocked`) and receive its 0x21 response!
                # 3. Release the held callback response and receive `first`'s 0x24 EvaluateResponse.
                body_a = _materialize_payload(step["first"], placeholders, eval_slots)
                create_body = _materialize_payload(step["create_while_blocked"], placeholders, eval_slots)
                create_slot = step.get("create_slot")
                gate_uri = _materialize_payload(step["gate_uri"], placeholders, eval_slots)
                req_a_id = body_a["requestId"]

                _send(EVALUATE_REQUEST, body_a)
                held_cb: tuple[int, dict[str, Any]] | None = None
                while held_cb is None:
                    msg = _next_msg(deadline)
                    if msg is None:
                        protocol_errors.append("interleaved_create: never received gated callback")
                        break
                    code, resp = msg
                    if code in CALLBACK_REQUEST_CODES and resp.get("uri") == gate_uri:
                        events.append({"dir": "recv", "step": "interleaved_gate_cb", "msg": _canonicalize_msg(code, resp, is_callback_req=True)})
                        held_cb = (code, resp)
                        break
                    if code in CALLBACK_REQUEST_CODES:
                        events.append({"dir": "recv", "step": "callback", "msg": _canonicalize_msg(code, resp, is_callback_req=True)})
                        rc, rbody = _build_callback_response(code, resp, handlers, placeholders)
                        _send(rc, rbody)
                        continue
                    events.append({"dir": "recv", "step": "interleaved_pre", "msg": _canonicalize_msg(code, resp)})
                if held_cb is None:
                    break

                # Fire CreateEvaluatorRequest while eval_a is blocked waiting on the callback!
                _send(CREATE_EVALUATOR_REQUEST, create_body)
                msg = _next_msg(deadline)
                if msg is None:
                    protocol_errors.append("interleaved_create: missing CreateEvaluatorResponse while blocked")
                    break
                code, resp = msg
                if code == CREATE_EVALUATOR_RESPONSE and "evaluatorId" in resp and create_slot is not None:
                    eid = resp["evaluatorId"]
                    if not isinstance(eid, int) or eid == 0:
                        protocol_errors.append(f"invalid minted evaluatorId for {create_slot}: {eid!r}")
                    else:
                        eval_slots[create_slot] = eid
                        eid_to_slot[eid] = create_slot
                events.append({"dir": "recv", "step": "interleaved_create_resp", "msg": _canonicalize_msg(code, resp)})

                # Release held callback and await EvaluateResponse for req_a
                rc, rbody = _build_callback_response(held_cb[0], held_cb[1], handlers, placeholders)
                _send(rc, rbody)
                got_a = False
                while not got_a:
                    msg = _next_msg(deadline)
                    if msg is None:
                        protocol_errors.append("interleaved_create: req_a never completed after callback release")
                        break
                    code, resp = msg
                    if code in CALLBACK_REQUEST_CODES:
                        events.append({"dir": "recv", "step": "callback_a", "msg": _canonicalize_msg(code, resp, is_callback_req=True)})
                        rc2, rbody2 = _build_callback_response(code, resp, handlers, placeholders)
                        _send(rc2, rbody2)
                        continue
                    events.append({"dir": "recv", "step": "interleaved_a", "msg": _canonicalize_msg(code, resp)})
                    if code == EVALUATE_RESPONSE and resp.get("requestId") == req_a_id:
                        got_a = True

            elif op == "close_evaluator":
                body = _materialize_payload(step["body"], placeholders, eval_slots)
                _send(CLOSE_EVALUATOR, body)

            elif op == "send_raw":
                code = int(step["code"])
                body = _materialize_payload(step["body"], placeholders, eval_slots)
                omit_none = bool(step.get("omit_none", True))
                _send(code, body, omit_none=omit_none)

            else:
                raise ValueError(f"unknown server step op: {op}")

        # Close stdin to signal clean EOF to server
        try:
            proc.stdin.close()
        except BrokenPipeError:
            pass

        # Drain any trailing messages or wait for exit
        rem = max(0.5, deadline - time.monotonic())
        try:
            proc.wait(timeout=rem)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            proc.wait(timeout=5)
        t_read.join(timeout=2.0)
        stderr_bytes = proc.stderr.read()
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
        for s in (proc.stdin, proc.stdout, proc.stderr):
            try:
                s.close()
            except OSError:
                pass

    transcript_doc = {
        "schema_version": 1,
        "events": events,
        "protocol_errors": protocol_errors,
    }
    transcript_bytes = (json.dumps(transcript_doc, ensure_ascii=False, sort_keys=True, indent=2) + "\n").encode("utf-8")

    return {
        "stdout": transcript_bytes,
        "stderr": stderr_bytes,
        "exit_code": proc.returncode,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
        "raw_wire_in_sha256": hashlib.sha256(bytes(wire_in)).hexdigest(),
        "raw_wire_out_sha256": hashlib.sha256(bytes(tap.raw)).hexdigest(),
        "raw_wire_out_bytes": len(tap.raw),
        "protocol_errors": protocol_errors,
    }
