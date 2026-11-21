#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""Unit tests for tools/m5_repl_driver.py and tools/m5_pkldoc_tree.py."""
from __future__ import annotations

import os
from pathlib import Path
import sys
import tempfile
import unittest

import m5_pkldoc_tree
import m5_repl_driver

ROOT = Path(__file__).resolve().parents[2]


class M5ReplAndPkldocTests(unittest.TestCase):
    def test_repl_pipe_and_pty_raw_capture(self) -> None:
        fake_repl = """
import sys
sys.stdout.write("Welcome!\\n")
sys.stdout.flush()
idx = 0
while True:
    sys.stdout.write(f"pkl{idx}> ")
    sys.stdout.flush()
    line = sys.stdin.readline()
    if not line:
        break
    s = line.strip()
    if s == ":quit":
        break
    sys.stdout.write(f"got:{s}\\n")
    sys.stdout.flush()
    idx += 1
"""
        with tempfile.TemporaryDirectory() as td:
            p_res = m5_repl_driver.run_repl_pipe(
                [sys.executable, "-u", "-c", fake_repl],
                Path(td),
                {"PATH": os.defpath, "LC_ALL": "C"},
                ["hello\n", ":quit\n"],
                {},
                timeout=10.0,
            )
            self.assertEqual(p_res["exit_code"], 0)
            self.assertEqual(p_res["stderr"], b"")
            self.assertEqual(p_res["stdout"], b"Welcome!\npkl0> got:hello\npkl1> ")

            pty_res = m5_repl_driver.run_repl_pty(
                [sys.executable, "-u", "-c", fake_repl],
                Path(td),
                {"PATH": os.defpath, "LC_ALL": "C"},
                [
                    {"expect": "pkl0> ", "send": "hi\r"},
                    {"expect": "pkl1> ", "send": "\x04"},
                ],
                {},
                timeout=10.0,
            )
            self.assertEqual(pty_res["exit_code"], 0)
            self.assertEqual(pty_res["stderr"], b"")
            self.assertIn(b"Welcome!\r\n", pty_res["stdout"])
            self.assertIn(b"got:hi\r\n", pty_res["stdout"])

    def test_pkldoc_tree_captures_files_and_symlinks_without_following(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            out = Path(td) / "site"
            (out / "pkg/1.0.0").mkdir(parents=True)
            (out / "pkg/1.0.0/index.html").write_bytes(b"<h1>pkg</h1>")
            (out / "pkg/current").symlink_to("1.0.0")
            (out / "robots.txt").write_bytes(b"User-agent: *\n")

            tree = m5_pkldoc_tree.collect_pkldoc_tree(out)
            self.assertEqual(tree, {
                "generated/pkg/1.0.0/index.html": b"<h1>pkg</h1>",
                "generated/robots.txt": b"User-agent: *\n",
                "symlink/pkg/current": b"1.0.0",
            })
            # Symlink target dir must NOT be recursed into as generated/pkg/current/index.html!
            self.assertNotIn("generated/pkg/current/index.html", tree)

            # Root symlink must be rejected
            root_link = Path(td) / "link_root"
            root_link.symlink_to(out)
            with self.assertRaisesRegex(ValueError, "must not be a symlink"):
                m5_pkldoc_tree.collect_pkldoc_tree(root_link)

    def test_seed_pkldoc_package_cache_is_deterministic(self) -> None:
        with tempfile.TemporaryDirectory() as td:
            c1 = Path(td) / "c1"
            c2 = Path(td) / "c2"
            r1 = m5_pkldoc_tree.seed_pkldoc_package_cache(ROOT, c1)
            r2 = m5_pkldoc_tree.seed_pkldoc_package_cache(ROOT, c2)
            self.assertEqual(r1, r2)
            self.assertEqual(len(r1), 7)
            for pkg in m5_pkldoc_tree.PKLDOC_TEST_PACKAGE_ORDER:
                self.assertTrue((c1 / "package-2/localhost(3a)0" / pkg / f"{pkg}.zip").is_file())
                self.assertTrue((c1 / "package-2/localhost(3a)0" / pkg / f"{pkg}.json").is_file())


if __name__ == "__main__":
    unittest.main()
