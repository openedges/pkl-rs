import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_platforms


class PlatformEvidenceTests(unittest.TestCase):
    def test_platform_scenario_omission_duplication_and_errors_fail(self):
        names = sorted(m1_platforms.REQUIRED_SCENARIOS)
        valid = {'nodeChecks': names, 'browserChecks': names, 'browserErrors': []}
        m1_platforms.check_scenarios(valid)
        for target in ['nodeChecks', 'browserChecks']:
            for broken in [[], names[:-1], names[:-1]+names[:1], names+['unexpected']]:
                with self.assertRaises(ValueError):
                    m1_platforms.check_scenarios({**valid, target: broken})
        with self.assertRaises(ValueError):
            m1_platforms.check_scenarios({**valid, 'browserErrors': ['uncaught error']})

    def capture(self, values):
        return b''.join(value.hex().encode()+b'\n' for value in values)

    def test_byte_differences_are_preserved_for_every_target(self):
        captures = {name: self.capture([b'a\r\n']) for name in ['java', 'rust', 'node', 'browser']}
        captures['browser'] = self.capture([b'a\n'])
        rows = m1_platforms.compare([{'id': 'newline'}], captures)
        self.assertFalse(rows[0]['shared_engine_match'])
        self.assertFalse(rows[0]['oracle_matches']['browser'])
        self.assertEqual(rows[0]['mismatched_bytes_hex']['java'], '610d0a')

    def test_missing_empty_duplicate_and_truncated_runs_are_rejected(self):
        captures = {name: b'61\n' for name in ['java', 'rust', 'node', 'browser']}
        for cases in [[], [{'id': 'a'}, {'id': 'a'}]]:
            with self.assertRaises(ValueError): m1_platforms.compare(cases, captures)
        del captures['browser']
        with self.assertRaises(ValueError): m1_platforms.compare([{'id': 'a'}], captures)
        captures['browser'] = b'61'
        with self.assertRaises(ValueError): m1_platforms.compare([{'id': 'a'}], captures)

    def test_known_gaps_are_failures_and_contract_requires_exact_inventory(self):
        cases = [{'id': name} for name in sorted(m1_platforms.KNOWN_GAPS)]
        captures = {'java': self.capture([b'a', b'a']), **{name: self.capture([b'b', b'b']) for name in ['rust', 'node', 'browser']}}
        rows = m1_platforms.compare(cases, captures)
        self.assertTrue(m1_platforms.contract_matches(rows))
        self.assertFalse(any(row['oracle_matches']['rust'] for row in rows))
        self.assertFalse(m1_platforms.contract_matches(rows[:1]))
        rows[0]['shared_engine_match'] = False
        self.assertFalse(m1_platforms.contract_matches(rows))


if __name__ == '__main__':
    unittest.main()
