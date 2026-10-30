import json
import sys
import tempfile
from pathlib import Path
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m1_platforms


class PlatformEvidenceTests(unittest.TestCase):
    def test_cargo_products_respect_target_directory_for_all_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'workspace'
            external = Path(temporary) / 'external cache'
            for setting, target in [(None, root / 'target'),
                                    ('custom/cache', root / 'custom/cache'),
                                    (str(external), external)]:
                with self.subTest(setting=setting), patch.object(m1_platforms, 'ROOT', root), \
                     patch.dict(m1_platforms.os.environ, {}, clear=True), \
                     patch.object(m1_platforms.platform, 'system', return_value='Linux'):
                    if setting is not None:
                        m1_platforms.os.environ['CARGO_TARGET_DIR'] = setting
                    products = m1_platforms.cargo_products()
                    self.assertEqual(products['wasm'], target / 'wasm32-unknown-unknown/release/pkl_wasm.wasm')
                    self.assertEqual(products['napi'], target / 'release/libpkl_napi.so')
                    native = 'semantics_probe.exe' if m1_platforms.os.name == 'nt' else 'semantics_probe'
                    self.assertEqual(products['native'], target / 'debug/examples' / native)

    def test_source_snapshot_prunes_caches_before_traversal_and_hashing(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            fixed = {'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
                     'tools/m1_platforms.py', 'tools/m1_semantics.py', 'tools/vendor_sources.py',
                     'tools/java/M1SemanticsProbe.java', 'tests/fixtures/m1/semantics/cases.json',
                     'crates/pkl-runtime/Cargo.toml',
                     'crates/pkl-runtime/examples/semantics_probe.rs',
                     'crates/pkl-runtime/src/stdlib/modules.txt'}
            selected = fixed | {'vendor/java_regex/src/lib.rs',
                                'vendor/java_regex/src/utf16/name-pool.bin',
                                'vendor/java_regex/src/utf16/property-aliases.tsv',
                                'vendor/java_regex/LICENSE-MIT',
                                'crates/pkl-runtime/src/value.rs',
                                'crates/pkl-wasm/src/lib.rs', 'crates/pkl-napi/src/lib.rs',
                                'crates/pkl-napi/Cargo.toml', 'tools/platform/run.mjs',
                                'tools/platform/package.json', 'tools/platform/example.pkl',
                                'tools/platform/nested/build/ordinary.json',
                                'tools/platform/targeting/ordinary.rs'}
            excluded_names = {'node_modules', 'target', '.cache'}
            ignored = {'tools/platform/ignored.txt', 'tools/platform/ignored.RS'}
            for folder in ['crates/pkl-runtime/src', 'crates/pkl-wasm',
                           'crates/pkl-napi', 'tools/platform', 'vendor/java_regex']:
                ignored.update(f'{folder}/{name}/nested/cache.json' for name in excluded_names)
            for name in selected | ignored:
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b'raw source\r\n')
            original_walk, original_sha = m1_platforms.os.walk, m1_platforms.sha
            visited, hashed = [], []

            def guarded_walk(top, *args, **kwargs):
                self.assertTrue(kwargs.get('topdown'))
                for folder, directories, names in original_walk(top, *args, **kwargs):
                    folder = Path(folder)
                    visited.append(folder)
                    self.assertFalse(excluded_names.intersection(folder.relative_to(root).parts),
                                     'dependency/cache directory traversed before filtering')
                    yield str(folder), directories, names

            def guarded_sha(path):
                hashed.append(path)
                self.assertFalse(excluded_names.intersection(path.relative_to(root).parts))
                return original_sha(path)

            with patch.object(m1_platforms.os, 'walk', guarded_walk), patch.object(m1_platforms, 'sha', guarded_sha):
                paths = m1_platforms.source_paths(root)
                for path in paths:
                    m1_platforms.sha(path)
            self.assertTrue(visited)
            self.assertEqual({path.relative_to(root).as_posix() for path in paths}, selected)
            self.assertEqual(set(hashed), set(paths))
            self.assertEqual(paths, sorted(paths))

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
            'm2-lossless-diagnostic-frame-identity',
            'm2-numeric-value-frame-spans',
            'm2-frontend-annotation-owners',
            'm2-frontend-const-owner-and-parent-order',
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

    def test_resolved_regex_cases_remain_and_contract_requires_zero_mismatches(self):
        former_gaps = {'regex-unicode-15-vs-16', 'regex-zero-width-supplementary'}
        inventory = json.loads((m1_platforms.ROOT / 'tests/fixtures/m1/semantics/cases.json').read_bytes())['cases']
        ids = [case['id'] for case in inventory]
        self.assertEqual(m1_platforms.KNOWN_GAPS, set())
        for case_id in former_gaps:
            self.assertEqual(ids.count(case_id), 1)

        cases = [{'id': case_id} for case_id in sorted(former_gaps)]
        equal = {name: self.capture([b'a', b'a']) for name in ['java', 'rust', 'node', 'browser']}
        self.assertTrue(m1_platforms.contract_matches(m1_platforms.compare(cases, equal)))

        # A future common mismatch cannot become an accepted exception.
        all_wrong = {**equal, **{name: self.capture([b'b', b'b']) for name in ['rust', 'node', 'browser']}}
        wrong_rows = m1_platforms.compare(cases, all_wrong)
        self.assertTrue(all(row['shared_engine_match'] for row in wrong_rows))
        self.assertFalse(m1_platforms.contract_matches(wrong_rows))

        # Platform disagreement remains a contract failure even with complete captures.
        browser_wrong = {**equal, 'browser': self.capture([b'b', b'b'])}
        divergent_rows = m1_platforms.compare(cases, browser_wrong)
        self.assertFalse(all(row['shared_engine_match'] for row in divergent_rows))
        self.assertFalse(m1_platforms.contract_matches(divergent_rows))


if __name__ == '__main__':
    unittest.main()
