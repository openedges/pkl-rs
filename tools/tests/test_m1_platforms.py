import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_platforms


class PlatformEvidenceTests(unittest.TestCase):
    def test_platform_scenario_omission_duplication_and_errors_fail(self):
        names = sorted(m1_platforms.REQUIRED_SCENARIOS)
        pin = '1.64.1'
        valid = {'nodeChecks': names, 'browserChecks': names, 'browserErrors': [],
                 'playwright': pin, 'playwrightCore': pin, 'playwrightPin': pin}
        m1_platforms.check_scenarios(valid)
        for target in ['nodeChecks', 'browserChecks']:
            for broken in [[], names[:-1], names[:-1]+names[:1], names+['unexpected']]:
                with self.assertRaises(ValueError):
                    m1_platforms.check_scenarios({**valid, target: broken})
        with self.assertRaises(ValueError):
            m1_platforms.check_scenarios({**valid, 'browserErrors': ['uncaught error']})
        for key in ['playwright', 'playwrightCore', 'playwrightPin']:
            for value in [None, '1.64.0', '^1.64.1']:
                with self.assertRaises(ValueError):
                    m1_platforms.check_scenarios({**valid, key: value})

    def test_obsolete_busy_and_manual_cancellation_scenarios_do_not_satisfy_contract(self):
        legacy = set(m1_platforms.REQUIRED_SCENARIOS) - {
            'same-evaluator-nested-read-resume',
            'live-host-cancel-abort-release-settle',
            'live-host-close-abort-release-settle',
            'live-host-timeout-abort-release-settle',
            'nested-terminal-aborts-all-live-host-operations',
        }
        legacy.update({'async-same-stack-resume-reentry-duplicate',
                       'cancel-close-late-callback', 'real-timer-waiting-timeout'})
        with self.assertRaises(ValueError):
            m1_platforms.check_scenarios({'nodeChecks': sorted(legacy), 'browserChecks': sorted(legacy),
                                         'browserErrors': [], 'playwright': '1.64.1',
                                         'playwrightCore': '1.64.1', 'playwrightPin': '1.64.1'})

    def test_language_scenarios_are_required_on_each_target(self):
        collections = {
            'm2-ordered-collection-members-and-amendments',
            'm2-collection-generators-and-spread',
            'm2-collection-diagnostics-and-power-assertions',
            'm2-lazy-generic-collection-values',
            'm2-strict-list-nested-wrappers',
            'm2-generic-collection-diagnostics',
            'm2-member-predicate-values',
            'm2-member-predicate-owner-cache',
            'm2-member-predicate-diagnostics',
            'm2-structural-equality',
            'm2-structural-keys',
            'm2-structural-equality-diagnostics',
            'm2-typed-default-equality-and-keys',
            'm2-comparison-cache-and-class-order',
            'm2-deep-force-before-key-hash',
            'm2-local-cache-comparison',
            'm2-hashed-key-buckets',
            'm2-cache-fifth-publication',
            'm2-let-diagnostic-scopes',
            'm2-reference-paths-and-rendering',
            'm2-reference-equality-and-type-checks',
            'm2-reference-stdlib-diagnostics',
            'm2-fixed-inheritance-and-virtual-receivers',
            'm2-fixed-reference-declaration-metadata',
            'm2-fixed-property-diagnostics',
            'm2-class-publication-sequences',
            'm2-type-alias-values',
            'm2-function-type-metadata',
            'm2-generic-alias-diagnostics',
            'm2-type-resolution-error-order',
            'm2-type-parameter-shadowing',
            'm2-unfinished-alias-arguments',
            'm2-local-resolution-inline-sequences',
            'm2-local-resolution-call-targets-sequences',
            'm2-local-resolution-recursive-sequences',
            'm2-local-resolution-owners-sequences',
            'm2-local-resolution-wrappers-sequences',
            'm2-nullable-default-listing-sequences',
            'm2-nullable-default-mapping-sequences',
            'm2-nullable-default-nested-sequences',
            'm2-generic-constructors-sequences',
            'm2-generic-constructors-closure-sequences',
            'm2-strict-map-sequences',
            'm2-strict-map-host-sequences',
            'm2-strict-set-sequences',
            'm2-strict-set-host-sequences',
            'm2-collection-type-sequences',
            'm2-collection-type-host-sequences',
            'm2-frontend-depth-sequences',
            'm2-frontend-depth-host-sequences',
            'm2-lossless-string-representation',
        }
        names = sorted(m1_platforms.REQUIRED_SCENARIOS)
        valid = {'nodeChecks': names, 'browserChecks': names, 'browserErrors': [],
                 'playwright': '1.64.1', 'playwrightCore': '1.64.1',
                 'playwrightPin': '1.64.1'}
        for target in ['nodeChecks', 'browserChecks']:
            for missing in collections:
                with self.subTest(target=target, missing=missing), self.assertRaises(ValueError):
                    m1_platforms.check_scenarios({**valid, target: [name for name in names if name != missing]})

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
