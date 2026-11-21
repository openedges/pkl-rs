#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Deterministic pipe + POSIX PTY transcript driver for M5 Lane 2 (`pkl repl`).

Runs pinned Java 0.32.0 (`org.pkl.cli.Main repl`) and Rust (`pkl repl`) under
identical pipe or real POSIX PTY conditions (`pty.openpty()`, `TIOCSCTTY`,
80x24 `TIOCSWINSZ`, `TERM=dumb`, separate `stderr=PIPE`) and captures exact raw
bytes with zero normalization or whitespace trimming.
"""
from __future__ import annotations

import fcntl
import os
from pathlib import Path
import pty
import select
import signal
import struct
import subprocess
import termios
import time
from typing import Any


def _set_winsize(fd: int, rows: int = 24, cols: int = 80) -> None:
    packed = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, packed)


def _expand(text: str, placeholders: dict[str, str]) -> str:
    out = text
    for k, v in placeholders.items():
        out = out.replace("{" + k + "}", v)
    return out


def run_repl_pipe(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    lines: list[str],
    placeholders: dict[str, str],
    timeout: float = 30.0,
) -> dict[str, Any]:
    """Run `pkl repl` over non-TTY stdin/stdout/stderr pipes."""
    if os.name != "posix":
        raise RuntimeError("M5 REPL driver requires POSIX process groups")
    stdin_payload = "".join(_expand(line, placeholders) for line in lines).encode("utf-8")
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
    timed_out = False
    try:
        try:
            stdout, stderr = proc.communicate(input=stdin_payload, timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            try:
                stdout, stderr = proc.communicate(timeout=5)
            except subprocess.TimeoutExpired as err:
                stdout = err.output or b""
                stderr = err.stderr or b""
                proc.wait(timeout=5)
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass
    return {
        "stdout": stdout,
        "stderr": stderr,
        "exit_code": proc.returncode,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
    }


def run_repl_pty(
    command: list[str],
    cwd: Path,
    env: dict[str, str],
    interactions: list[dict[str, Any]],
    placeholders: dict[str, str],
    timeout: float = 30.0,
) -> dict[str, Any]:
    """Run `pkl repl` attached to a real POSIX PTY (TERM=dumb, 80x24, stderr=PIPE).

    Each interaction item specifies:
      - `expect`: substring (UTF-8) to wait for in cumulative PTY output since last match
      - `send`: UTF-8 string to write to PTY master (e.g. "1 + 2\\r", ":quit\\r", "\\x04")
    """
    if os.name != "posix":
        raise RuntimeError("M5 PTY REPL driver requires POSIX PTY support")

    before = time.perf_counter_ns()
    master, slave = pty.openpty()
    _set_winsize(slave, rows=24, cols=80)

    def _preexec() -> None:
        os.setsid()
        fcntl.ioctl(slave, termios.TIOCSCTTY, 0)

    pty_env = dict(env)
    pty_env["TERM"] = "dumb"
    pty_env["COLUMNS"] = "80"
    pty_env["LINES"] = "24"

    proc = subprocess.Popen(
        command,
        cwd=cwd,
        env=pty_env,
        stdin=slave,
        stdout=slave,
        stderr=subprocess.PIPE,
        close_fds=True,
        preexec_fn=_preexec,
    )
    os.close(slave)
    assert proc.stderr is not None

    buf = bytearray()
    cursor = 0
    timed_out = False
    deadline = time.monotonic() + timeout

    def _drain_until(needle: bytes) -> bool:
        nonlocal cursor, timed_out
        while time.monotonic() < deadline:
            pos = buf.find(needle, cursor)
            if pos >= 0:
                cursor = pos + len(needle)
                return True
            r, _, _ = select.select([master], [], [], 0.1)
            if r:
                try:
                    chunk = os.read(master, 4096)
                except OSError:
                    return False
                if not chunk:
                    return False
                buf.extend(chunk)
            elif proc.poll() is not None:
                # Drain any remaining buffered PTY bytes after process exit
                while True:
                    r2, _, _ = select.select([master], [], [], 0.05)
                    if not r2:
                        break
                    try:
                        chunk = os.read(master, 4096)
                    except OSError:
                        break
                    if not chunk:
                        break
                    buf.extend(chunk)
                pos = buf.find(needle, cursor)
                if pos >= 0:
                    cursor = pos + len(needle)
                    return True
                return False
        timed_out = True
        return False

    try:
        for step in interactions:
            if "expect" in step:
                needle = _expand(step["expect"], placeholders).encode("utf-8")
                if not _drain_until(needle):
                    break
            if "send" in step:
                payload = _expand(step["send"], placeholders).encode("utf-8")
                os.write(master, payload)

        # Wait for EOF / process exit and drain all remaining PTY bytes
        while time.monotonic() < deadline:
            r, _, _ = select.select([master], [], [], 0.1)
            if r:
                try:
                    chunk = os.read(master, 4096)
                except OSError:
                    break
                if not chunk:
                    break
                buf.extend(chunk)
            elif proc.poll() is not None:
                # One final non-blocking drain pass
                while True:
                    r2, _, _ = select.select([master], [], [], 0.05)
                    if not r2:
                        break
                    try:
                        chunk = os.read(master, 4096)
                    except OSError:
                        break
                    if not chunk:
                        break
                    buf.extend(chunk)
                break
        else:
            timed_out = True

        # Wait gracefully for process exit after PTY slave closes (avoids false timeout race on macOS EIO)
        rem_wait = max(0.5, deadline - time.monotonic())
        try:
            rc = proc.wait(timeout=rem_wait)
        except subprocess.TimeoutExpired:
            timed_out = True
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except (ProcessLookupError, PermissionError):
                pass
            rc = proc.wait(timeout=5)
        stderr_bytes = proc.stderr.read()
    finally:
        try:
            os.close(master)
        except OSError:
            pass
        try:
            proc.stderr.close()
        except OSError:
            pass
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except (ProcessLookupError, PermissionError):
            pass

    return {
        "stdout": bytes(buf),
        "stderr": stderr_bytes,
        "exit_code": rc,
        "files": {},
        "timed_out": timed_out,
        "workspace_changes": {},
        "elapsed_ns": time.perf_counter_ns() - before,
    }
