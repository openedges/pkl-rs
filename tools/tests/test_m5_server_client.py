#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit tests for tools/m5_server_client.py (MessagePack codec + mock server sessions)."""
from __future__ import annotations

import io
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

import m5_server_client as sc

TOOLS_DIR = Path(__file__).resolve().parents[1]


class M5ServerClientTests(unittest.TestCase):
    def test_all_19_message_codes_present(self) -> None:
        self.assertEqual(sc.ALL_MESSAGE_CODES, set(range(0x20, 0x33)))

    def test_msgpack_roundtrip_all_types(self) -> None:
        samples = [
            0, 1, 127, -1, -32, -33, -128, -32768, -2147483648, -9223372036854775800,
            128, 255, 65535, 70000, 1 << 40,
            True, False, None, 3.14159,
            "", "hello", "x" * 300,
            b"", b"\x00\x01\xff", b"B" * 300,
            [], [1, "two", b"\xab", {"k": "v"}],
        ]
        for val in samples:
            packed = sc.pack_value(val, omit_none=False)
            unpacked = sc._unpack_next(io.BytesIO(packed))
            self.assertEqual(unpacked, val)

    def test_omit_none_in_maps(self) -> None:
        body = {"requestId": 1, "error": None, "result": b"\x01"}
        packed = sc.pack_value(body, omit_none=True)
        decoded = sc._unpack_next(io.BytesIO(packed))
        self.assertEqual(decoded, {"requestId": 1, "result": b"\x01"})
        self.assertNotIn("error", decoded)

    def test_mock_server_session_callbacks_and_interleaving(self) -> None:
        mock_code = f"""
import sys, threading
sys.path.insert(0, {str(TOOLS_DIR)!r})
import m5_server_client as sc

in_s = sys.stdin.buffer
out_s = sys.stdout.buffer
lock = threading.Lock()
pending_cb = {{}}

def send(code, body):
    with lock:
        sc.write_message(out_s, code, body)

def run_eval(msg):
    rid = msg["requestId"]
    eid = msg["evaluatorId"]
    if msg.get("moduleUri") == "cb://gated":
        ev = threading.Event()
        holder = []
        pending_cb[9001] = (ev, holder)
        send(sc.READ_RESOURCE_REQUEST, {{"requestId": 9001, "evaluatorId": eid, "uri": "cbres:/gate.txt"}})
        ev.wait(timeout=10)
        cb_resp = holder[0]
        send(sc.EVALUATE_RESPONSE, {{"requestId": rid, "evaluatorId": eid, "result": cb_resp["contents"]}})
    else:
        send(sc.EVALUATE_RESPONSE, {{"requestId": rid, "evaluatorId": eid, "result": b"FAST"}})

next_eid = 777001
threads = []
while True:
    item = sc.read_message(in_s)
    if item is None:
        break
    code, body = item
    if code == sc.CREATE_EVALUATOR_REQUEST:
        eid = next_eid
        next_eid += 1
        send(sc.CREATE_EVALUATOR_RESPONSE, {{"requestId": body["requestId"], "evaluatorId": eid}})
    elif code == sc.EVALUATE_REQUEST:
        t = threading.Thread(target=run_eval, args=(body,))
        t.start()
        threads.append(t)
    elif code == sc.READ_RESOURCE_RESPONSE:
        ev, holder = pending_cb.pop(body["requestId"])
        holder.append(body)
        ev.set()
    elif code == sc.CLOSE_EVALUATOR:
        pass
for t in threads:
    t.join(timeout=5)
"""
        scenario = {
            "callbacks": {"read_resource": {"cbres:/gate.txt": {"contents_utf8": "UNBLOCKED"}}},
            "steps": [
                {"op": "create_evaluator", "slot": "$eval:0", "body": {"requestId": 1}},
                {
                    "op": "evaluate_interleaved_create",
                    "gate_uri": "cbres:/gate.txt",
                    "first": {"requestId": 10, "evaluatorId": "$eval:0", "moduleUri": "cb://gated"},
                    "create_slot": "$eval:1",
                    "create_while_blocked": {"requestId": 2},
                },
                {"op": "evaluate", "body": {"requestId": 11, "evaluatorId": "$eval:1", "moduleUri": "cb://fast"}},
                {"op": "close_evaluator", "body": {"evaluatorId": "$eval:0"}},
                {"op": "close_evaluator", "body": {"evaluatorId": "$eval:1"}},
            ],
        }
        with tempfile.TemporaryDirectory() as td:
            res = sc.run_server_session(
                [sys.executable, "-c", mock_code],
                Path(td),
                dict(os.environ),
                scenario,
                {},
                timeout=15.0,
            )
            self.assertEqual(res["exit_code"], 0)
            self.assertEqual(res["protocol_errors"], [])
            transcript = json.loads(res["stdout"])
            steps = [e["step"] for e in transcript["events"]]
            self.assertEqual(steps, [
                "create_evaluator",
                "interleaved_gate_cb",
                "interleaved_create_resp",
                "interleaved_a",
                "evaluate",
            ])
            # Verify opaque evaluatorId & callback requestId canonicalized and raw bytes preserved as hex
            self.assertEqual(transcript["events"][1]["msg"]["body"]["requestId"], "$cb_req:0")
            self.assertEqual(transcript["events"][2]["msg"]["body"]["evaluatorId"], "$eval:1")
            self.assertEqual(
                transcript["events"][3]["msg"]["body"]["result"],
                {"__bytes_hex__": b"UNBLOCKED".hex()},
            )


if __name__ == "__main__":
    unittest.main()
