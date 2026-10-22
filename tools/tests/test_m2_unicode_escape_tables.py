"""Reject corrupted or ambiguous Java Unicode escape mapping evidence."""
import copy
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_unicode_escape_tables as tables


class UnicodeEscapeTableTests(unittest.TestCase):
    def setUp(self):
        self.letters = b'letter_or_digit\t30\t39\nletter_or_digit\t41\t5a\nletter_or_digit\t660\t669\n'
        self.digits = b'hex_digit\t30\t39\t0\nhex_digit\t41\t46\ta\nhex_digit\t660\t669\t0\n'
        self.payload = self.letters + self.digits

    def test_non_ascii_digits_and_nonhex_letters_remain_distinct(self):
        result = tables.intervals(self.payload)
        self.assertEqual(result['letter_or_digit'], [(48, 57), (65, 90), (1632, 1641)])
        self.assertEqual(result['hex_digit'], [(48, 57, 0), (65, 70, 10), (1632, 1641, 0)])

    def test_transport_rejects_missing_ambiguous_and_incomplete_rows(self):
        cases = [b'', self.payload[:-1], self.letters, self.digits,
                 self.digits + self.letters,
                 self.payload.replace(b'hex_digit\t30\t39\t0', b'hex_digit\t30\t39'),
                 self.payload.replace(b'letter_or_digit\t30\t39', b'letter_or_digit\t30\t39\t0'),
                 self.payload.replace(b'\t660\t669', b'\tGG\t669'),
                 self.payload.replace(b'\t660\t669', b'\t669\t660'),
                 self.payload.replace(b'\t660\t669', b'\t10000\t10009'),
                 self.payload.replace(b'\t660\t669', b'\td800\td809')]
        for payload in cases:
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                tables.intervals(payload)

    def test_rejects_overlaps_nonmaximal_ranges_and_invalid_digit_mapping(self):
        cases = [self.payload.replace(b'letter_or_digit\t41\t5a', b'letter_or_digit\t39\t5a'),
                 self.payload.replace(b'letter_or_digit\t41\t5a', b'letter_or_digit\t3a\t5a'),
                 self.payload.replace(b'hex_digit\t30\t39\t0', b'hex_digit\t30\t34\t0\nhex_digit\t35\t39\t5'),
                 self.payload.replace(b'hex_digit\t41\t46\ta', b'hex_digit\t39\t3e\ta'),
                 self.payload.replace(b'hex_digit\t41\t46\ta', b'hex_digit\t41\t47\ta'),
                 self.payload.replace(b'hex_digit\t41\t46\ta', b'hex_digit\t41\t46\t10'),
                 self.payload.replace(b'hex_digit\t41\t46\ta', b'hex_digit\t50\t5f\t0')]
        for payload in cases:
            with self.subTest(payload=payload), self.assertRaises(ValueError):
                tables.intervals(payload)
        # Adjacent ranges with a reset digit value must not be merged.
        reset = b'letter_or_digit\t30\t43\nhex_digit\t30\t39\t0\nhex_digit\t3a\t43\t0\n'
        self.assertEqual(len(tables.intervals(reset)['hex_digit']), 2)

    def test_generated_bytes_pin_semantics_across_hosts(self):
        metadata = {'jdk': {'properties': {'java.vendor': 'Eclipse Adoptium',
                    'java.runtime.version': '21.0.12.1+1-LTS', 'os.name': 'Mac OS X'}}}
        generated = tables.render(self.payload, metadata, 'a' * 64)
        other = copy.deepcopy(metadata)
        other['jdk']['properties']['os.name'] = 'Linux'
        self.assertEqual(generated, tables.render(self.payload, other, 'a' * 64))
        self.assertIn(b'pub(super) const HEX_DIGIT_RANGES: &[(u32, u32, u32)]', generated)
        self.assertIn(b'(0x660, 0x669, 0x0)', generated)
        self.assertNotEqual(generated, tables.render(self.payload, metadata, 'b' * 64))


if __name__ == '__main__':
    unittest.main()
