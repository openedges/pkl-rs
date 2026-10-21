"""Reviewed mappings must retain original case contracts and fail closed."""
import copy
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_manifest


class ReviewedMappingTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.source = self.root / 'crates/runtime/src/reference.rs'
        self.source.parent.mkdir(parents=True)
        self.source.write_text('pub struct Reference;\n')
        self.baseline = dict(version='0.32.0', revision='pinned-source-revision')
        self.requirements = [dict(id='source:reference', required=True,
            implementation=dict(state='unimplemented', links=[]),
            tests=dict(state='partial', case_ids=['old-probe']))]
        self.cases = [
            dict(id='old-probe', required=True, requirements=['source:reference'],
                 input='original.pkl', expression='result', execution=dict(state='ready')),
            dict(id='new-probe', required=True, requirements=[],
                 input='new.pkl', expression='result', execution=dict(state='ready')),
            dict(id='upstream-original', required=True, requirements=[], adapter=None,
                 input='external/pkl/original.pkl', execution=dict(state='unimplemented')),
        ]
        self.mapping = dict(schema_version=1, baseline=self.baseline.copy(), requirements=[
            dict(requirement='source:reference', scope='Synthetic property access for user classes.',
                 remaining=['Complete access/type/diagnostic corpus.'],
                 implementation=dict(state='partial', links=[
                     dict(path='crates/runtime/src/reference.rs', symbol='Reference')]),
                 tests=dict(state='partial', case_ids=['new-probe']))])

    def apply(self, mapping=None):
        m2_manifest.apply_mappings(self.requirements, self.cases,
                                  self.mapping if mapping is None else mapping,
                                  self.baseline, self.root)

    def test_partial_review_is_additive_and_does_not_complete_upstream_execution(self):
        original = copy.deepcopy(self.cases)
        self.apply()
        requirement = self.requirements[0]
        self.assertEqual(requirement['tests']['case_ids'], ['old-probe', 'new-probe'])
        self.assertEqual(self.cases[1]['requirements'], ['source:reference'])
        self.assertEqual(self.cases[0], original[0])
        self.assertEqual(self.cases[2], original[2])
        link = requirement['implementation']['links'][0]
        self.assertEqual(link['sha256'], m2_manifest.digest(self.source.read_bytes()))
        results = [dict(id=row['id'], status='pass') for row in self.cases[:2]]
        failures = m2_manifest.full_gate_failures(
            dict(requirements=self.requirements, cases=self.cases), results)
        self.assertEqual(failures, ['implementation incomplete: source:reference',
                                   'test mapping incomplete: source:reference',
                                   'required case incomplete: upstream-original'])

    def test_completed_review_still_requires_original_case_execution(self):
        row = self.mapping['requirements'][0]
        row['implementation']['state'] = 'implemented'
        row['tests'] = dict(state='mapped', case_ids=['new-probe', 'upstream-original'])
        row['remaining'] = []
        original_contract = copy.deepcopy(self.cases[2])
        self.apply()
        self.assertEqual(self.cases[2], dict(original_contract, requirements=['source:reference']))
        manifest = dict(requirements=self.requirements, cases=self.cases)
        results = [dict(id=row['id'], status='pass') for row in self.cases[:2]]
        self.assertIn('requirement execution incomplete: source:reference',
                      m2_manifest.full_gate_failures(manifest, results))
        results.append(dict(id='upstream-original', status='pass'))
        self.assertEqual(m2_manifest.full_gate_failures(manifest, results),
                         ['required case incomplete: upstream-original'])

    def test_unknown_requirement_case_and_duplicate_reviews_are_rejected(self):
        for mutation in ('requirement', 'case', 'duplicate-review', 'duplicate-case'):
            mapping = copy.deepcopy(self.mapping)
            row = mapping['requirements'][0]
            if mutation == 'requirement':
                row['requirement'] = 'unknown'
            elif mutation == 'case':
                row['tests']['case_ids'] = ['missing-case']
            elif mutation == 'duplicate-review':
                mapping['requirements'].append(copy.deepcopy(row))
            else:
                row['tests']['case_ids'] = ['new-probe', 'new-probe']
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                self.apply(mapping)

    def test_baseline_and_schema_cannot_be_silently_changed(self):
        for field, value in [('schema_version', True), ('schema_version', 2),
                             ('baseline', dict(self.baseline, version='0.33.0')),
                             ('requirements', {}), ('allow_incomplete', True)]:
            mapping = copy.deepcopy(self.mapping)
            mapping[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(ValueError):
                self.apply(mapping)

    def test_scope_cannot_claim_complete_while_retaining_known_work(self):
        for implementation, tests, remaining in [
                ('implemented', 'mapped', ['Unimplemented boundary.']),
                ('partial', 'partial', []), ('complete', 'mapped', []),
                ('implemented', 'complete', [])]:
            mapping = copy.deepcopy(self.mapping)
            row = mapping['requirements'][0]
            row['implementation']['state'] = implementation
            row['tests']['state'] = tests
            row['remaining'] = remaining
            with self.subTest(implementation=implementation, tests=tests), self.assertRaises(ValueError):
                self.apply(mapping)

    def test_missing_or_changed_source_symbols_cannot_support_a_mapping(self):
        for body in ('pub struct AnotherType;\n', 'pub struct ReferenceSuffix;\n'):
            self.source.write_text(body)
            with self.subTest(body=body), self.assertRaises(ValueError):
                self.apply()
        self.source.unlink()
        with self.assertRaises(OSError):
            self.apply()

    def test_source_paths_and_links_are_bounded_and_unique(self):
        for path in ('/tmp/reference.rs', '../reference.rs', 'crates/../reference.rs',
                     'external/pkl/reference.rs', 'crates/runtime/src/../src/reference.rs'):
            mapping = copy.deepcopy(self.mapping)
            mapping['requirements'][0]['implementation']['links'][0]['path'] = path
            with self.subTest(path=path), self.assertRaises(ValueError):
                self.apply(mapping)
        mapping = copy.deepcopy(self.mapping)
        links = mapping['requirements'][0]['implementation']['links']
        links.append(copy.deepcopy(links[0]))
        with self.assertRaises(ValueError):
            self.apply(mapping)

    def test_empty_claims_and_unrecognized_fields_are_rejected(self):
        for field, value in [('scope', ''), ('remaining', ['']),
                             ('implementation', dict(state='partial', links=[])),
                             ('tests', dict(state='partial', case_ids=[])),
                             ('required', False)]:
            mapping = copy.deepcopy(self.mapping)
            mapping['requirements'][0][field] = value
            with self.subTest(field=field), self.assertRaises(ValueError):
                self.apply(mapping)


if __name__ == '__main__':
    unittest.main()
