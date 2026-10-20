"""Failure-sensitive tests of M1 primitive evidence transport and inventory."""
from pathlib import Path
import sys
import tempfile
import unittest
from unittest import mock

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import m1_semantics as semantics


class SemanticsEvidenceTests(unittest.TestCase):
    def test_bytes_are_preserved(self):
        self.assertEqual(semantics.decode_capture(b'20610d0a0a\n\n', 2), [b' a\r\n\n', b''])

    def test_missing_and_extra_results_are_rejected(self):
        for payload, count in [(b'61\n', 2), (b'61\n62\n', 1), (b'', 0), (b'61', 1)]:
            with self.subTest(payload=payload, count=count), self.assertRaises(ValueError):
                semantics.decode_capture(payload, count)

    def test_noncanonical_hex_is_rejected(self):
        for payload in [b'6A\n', b'6a \n', b'6\n', b'6x\n', b'6a\r\n']:
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                semantics.decode_capture(payload, 1)

    def test_inventory_unique_and_matches_committed_bytes(self):
        import json
        manifest = semantics.generate_cases()
        self.assertEqual(len(manifest['cases']), 6078)
        self.assertEqual(len({c['id'] for c in manifest['cases']}), 6078)
        self.assertEqual(semantics.FIXTURE.read_text(), json.dumps(manifest, ensure_ascii=False, indent=2)+'\n')

    def test_existing_evidence_directory_is_never_reused(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory)
            preserved = output / 'original.stdout.bin'
            preserved.write_bytes(b'original')
            with mock.patch.object(semantics, 'ensure_oracle', return_value={}):
                with self.assertRaises(FileExistsError):
                    semantics.run(output, Path('/unused'))
            self.assertEqual(preserved.read_bytes(), b'original')


if __name__ == '__main__':
    unittest.main()
