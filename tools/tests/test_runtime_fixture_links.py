"""Fresh-checkout fixture linkage tests; temporary roots contain no target data."""
from __future__ import annotations

import os
from pathlib import Path
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from runtime_fixture_links import verify


def project_root() -> Path:
    for parent in Path(__file__).resolve().parents:
        if (parent / "oracle.lock.json").is_file():
            return parent
    raise ValueError("project root missing")


SOURCE_ROOT = (Path(os.environ["PKL_RUNTIME_FIXTURE_SOURCE_ROOT"])
               if "PKL_RUNTIME_FIXTURE_SOURCE_ROOT" in os.environ else project_root())
ROOTS = ("m2-string-base64", "m2-collection-emptiness", "m2-collection-emptiness-oracle",
         "m2-collection-every", "m2-collection-every-oracle")
TEST_SOURCE = "crates/pkl-runtime/tests/m2_string_base64.rs"


class RuntimeFixtureLinkTests(unittest.TestCase):
    def installed(self, mutate=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ROOTS:
                shutil.copytree(SOURCE_ROOT / "tests/fixtures" / name,
                                root / "tests/fixtures" / name)
            test = root / TEST_SOURCE
            test.parent.mkdir(parents=True)
            shutil.copy2(SOURCE_ROOT / TEST_SOURCE, test)
            self.assertFalse((root / "target").exists())
            if mutate is None:
                result = verify(root)
                self.assertEqual((result["runtime_files"], result["raw_expected_files"],
                                  result["base64_inline_rows"]), (578, 565, 89))
                self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)
            else:
                mutate(root)
                with self.assertRaises((ValueError, KeyError)):
                    verify(root)

    def test_clean_root_no_target(self):
        self.installed()

    def test_missing_runtime_input(self):
        self.installed(lambda root: (root / "tests/fixtures/m2-collection-every/module.pkl").unlink())

    def test_extra_expected_file(self):
        self.installed(lambda root: (root / "tests/fixtures/m2-collection-emptiness/expected/extra.bin")
                       .write_bytes(b"extra"))

    def test_mutated_utf16_expected(self):
        def edit(root):
            path = next((root / "tests/fixtures/m2-collection-emptiness/expected").iterdir())
            path.write_bytes(path.read_bytes() + b"\0")
        self.installed(edit)

    def test_missing_logger_utf16(self):
        def edit(root):
            path = next((root / "tests/fixtures/m2-collection-every/expected")
                        .glob("*-log-*.code-units.bin"))
            path.unlink()
        self.installed(edit)

    def test_mutated_logger_ordered_byte(self):
        def edit(root):
            path = next((root / "tests/fixtures/m2-collection-every/expected")
                        .glob("*-log-*.message.bin"))
            path.write_bytes(path.read_bytes() + b"changed")
        self.installed(edit)

    def test_mutated_sealed_archive(self):
        def edit(root):
            path = root / "tests/fixtures/m2-collection-every-oracle/oracle-evidence.tar.gz"
            path.write_bytes(path.read_bytes() + b"changed")
        self.installed(edit)

    def test_mutated_inline_base64_expectation(self):
        def edit(root):
            path = root / TEST_SOURCE
            content = path.read_bytes()
            marker = b'"encode-empty"########'
            self.assertIn(marker, content)
            path.write_bytes(content.replace(marker, b'"encode-other"########', 1))
        self.installed(edit)

    def test_missing_base64_input(self):
        self.installed(lambda root: (root / "tests/fixtures/m2-string-base64/queries.tsv").unlink())


if __name__ == "__main__":
    unittest.main()
