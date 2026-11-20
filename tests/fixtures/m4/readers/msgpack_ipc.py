# SPDX-License-Identifier: Apache-2.0
"""Minimal Python 3.9+ stdlib MessagePack codec for Pkl 0.32.0 external reader IPC."""
from __future__ import annotations

import struct
from typing import Any, BinaryIO, Optional

# Wire codes from org.pkl.core.messaging.Message (Pkl 0.32.0)
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


def _read_exact(stream: BinaryIO, n: int) -> bytes:
    buf = bytearray()
    while len(buf) < n:
        chunk = stream.read(n - len(buf))
        if not chunk:
            raise EOFError("unexpected EOF in MessagePack stream")
        buf.extend(chunk)
    return bytes(buf)


def unpack_value(stream: BinaryIO) -> Any:
    first = stream.read(1)
    if not first:
        raise EOFError("clean EOF")
    return _unpack_with_prefix(stream, first[0])


def _unpack_with_prefix(stream: BinaryIO, b: int) -> Any:
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


def _unpack_next(stream: BinaryIO) -> Any:
    return _unpack_with_prefix(stream, _read_exact(stream, 1)[0])


def _read_map(stream: BinaryIO, count: int) -> dict:
    result = {}
    for _ in range(count):
        k = _unpack_next(stream)
        v = _unpack_next(stream)
        result[k] = v
    return result


def pack_value(value: Any) -> bytes:
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
        return header + b"".join(pack_value(item) for item in value)
    if isinstance(value, dict):
        # Pkl 0.32.0 MessagePack protocol omits optional keys whose value is None.
        items = [(k, v) for k, v in value.items() if v is not None]
        n = len(items)
        if n <= 15:
            header = bytes([0x80 | n])
        elif n <= 0xFFFF:
            header = b"\xde" + struct.pack(">H", n)
        else:
            header = b"\xdf" + struct.pack(">I", n)
        parts = [header]
        for k, v in items:
            parts.append(pack_value(k))
            parts.append(pack_value(v))
        return b"".join(parts)
    raise TypeError(f"unsupported MessagePack value type: {type(value).__name__}")


def read_message(stream: BinaryIO) -> Optional[tuple]:
    first = stream.read(1)
    if not first:
        return None
    msg = _unpack_with_prefix(stream, first[0])
    if not isinstance(msg, list) or len(msg) != 2:
        raise ValueError(f"expected 2-element message array, got {msg!r}")
    return int(msg[0]), msg[1]


def write_message(stream: BinaryIO, code: int, body: dict) -> None:
    stream.write(pack_value([code, body]))
    stream.flush()
