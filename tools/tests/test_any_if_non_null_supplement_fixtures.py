"""Additive Any.ifNonNull authority integrity regressions."""
import hashlib
import json
from pathlib import Path
import shutil
import tempfile
import unittest

from tools.any_if_non_null_supplement_fixtures import DEFAULT, verify


def resign_manifest(root):
    files = {path.relative_to(root).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
             for path in root.rglob("*") if path.is_file() and path.name != "manifest.json"}
    (root / "manifest.json").write_text(json.dumps({"schema": 1, "baseline": "Pkl 0.32.0",
                                                     "files": files}, indent=2, sort_keys=True) + "\n")


class AnyIfNonNullSupplementFixtureTests(unittest.TestCase):
    def test_sealed_supplement(self):
        result = verify()
        self.assertEqual(result["rows"], 13)
        self.assertEqual(result["statuses"], {"success": 12, "error": 1})
        self.assertEqual(result["candidate_compatibility_passes_claimed"], 0)

    def test_authored_module_tamper_rejected_after_manifest_resigned(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp) / "fixture"
            shutil.copytree(DEFAULT, root)
            module = root / "module.pkl"
            module.write_bytes(module.read_bytes().replace(b'"bad"', b'"good"'))
            resign_manifest(root)
            with self.assertRaisesRegex(ValueError, "authored input differs"):
                verify(root)


if __name__ == "__main__":
    unittest.main()
