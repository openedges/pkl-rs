"""Reject incomplete or ambiguous generated identifier interval evidence."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_identifier_tables as tables


class IdentifierTableTests(unittest.TestCase):
    def setUp(self):
        self.payload = b'start\t24\t24\nstart\t41\t5a\npart\t24\t24\npart\t30\t39\npart\t41\t5a\n'

    def test_complete_canonical_intervals_parse(self):
        result = tables.intervals(self.payload)
        self.assertEqual(result['start'], [(36, 36), (65, 90)])
        self.assertEqual(result['part'], [(36, 36), (48, 57), (65, 90)])

    def test_missing_duplicate_overlapping_and_nonmaximal_ranges_fail(self):
        cases = [b'', self.payload[:-1], b'start\t41\t5a\n',
                 b'start\t41\t5a\nstart\t5b\t60\npart\t41\t5a\n',
                 b'start\t41\t5a\nstart\t50\t60\npart\t41\t5a\n',
                 b'start\t41\t110000\npart\t41\t5a\n',
                 b'start\t10000\t10001\npart\t41\t5a\n',
                 b'start\t5a\t41\npart\t41\t5a\n',
                 b'start\tGG\tZZ\npart\t41\t5a\n']
        for payload in cases:
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                tables.intervals(payload)

    def test_generated_table_pins_semantics_without_platform_dependent_hashes(self):
        metadata = dict(jdk=dict(properties={'java.runtime.version': '21.0.12.1+1-LTS',
                'java.vendor': 'Eclipse Adoptium', 'os.name': 'Mac OS X'}, executable_sha256='1' * 64))
        generated = tables.render(self.payload, metadata, 'a' * 64)
        other = copy.deepcopy(metadata)
        other['jdk']['properties']['os.name'] = 'Linux'
        other['jdk']['executable_sha256'] = '2' * 64
        self.assertEqual(generated, tables.render(self.payload, other, 'a' * 64))
        self.assertIn(b'pub(super) const START_RANGES', generated)
        self.assertIn(b'pub(super) const PART_RANGES', generated)
        self.assertIn(b'(0x41, 0x5a)', generated)
        self.assertNotEqual(generated, tables.render(self.payload, metadata, 'b' * 64))


if __name__ == '__main__':
    unittest.main()
