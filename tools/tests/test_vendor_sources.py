"""Source provenance must include generated data and exclude generated build output."""
import sys
import tempfile
import unittest
from pathlib import Path
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from vendor_sources import vendor_source_paths


class VendorSourceInputsTests(unittest.TestCase):
    def test_code_generated_tables_licenses_and_build_exclusion(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            paths = ['src/lib.rs', 'src/utf16/name-pool.bin', 'src/utf16/property-aliases.tsv',
                     'LICENSE-MIT', 'SPEC.md', 'Cargo.toml', 'target/debug/generated']
            for name in paths:
                path = root / 'vendor/java_regex' / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'input')
            observed = [p.relative_to(root / 'vendor/java_regex').as_posix()
                        for p in vendor_source_paths(root)]
            self.assertEqual(observed, sorted(paths[:-1]))

    def test_source_symlinks_are_not_silently_followed(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            base = root / 'vendor/java_regex'
            base.mkdir(parents=True)
            outside = root / 'outside'; outside.write_bytes(b'outside')
            (base / 'table.bin').symlink_to(outside)
            with self.assertRaises(ValueError):
                vendor_source_paths(root)

    def test_absent_vendor_remains_valid_for_historical_sources(self):
        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(vendor_source_paths(Path(tmp)), [])

    def test_lexically_present_non_directory_roots_are_rejected(self):
        for kind in ('dangling-symlink', 'ordinary-file'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); base = root / 'vendor/java_regex'
                base.parent.mkdir()
                if kind == 'dangling-symlink':
                    base.symlink_to(root / 'missing')
                else:
                    base.write_bytes(b'not a directory')
                with self.assertRaises(ValueError):
                    vendor_source_paths(root)
