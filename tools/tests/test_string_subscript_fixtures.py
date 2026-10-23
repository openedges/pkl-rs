"""Origin-linked failure injections for ordered actual-Java string API fixtures."""
import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import string_subscript_fixtures as fixtures

class StringSubscriptFixturesTest(unittest.TestCase):
    def copy(self, name='m2-string-subscripts'):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name) / name
        shutil.copytree(fixtures.ROOT / 'tests/fixtures' / name, root)
        return root

    def edit(self, root, name, mutate):
        path = root / name
        value = json.loads(path.read_text())
        mutate(value)
        path.write_text(json.dumps(value) + '\n')
        manifest = root / 'manifest.json'
        data = json.loads(manifest.read_text())
        data['files'][name] = hashlib.sha256(path.read_bytes()).hexdigest()
        manifest.write_text(json.dumps(data) + '\n')

    def test_both_original_selections_keep_order_and_actual_surrogates(self):
        for name, count in [('m2-string-subscripts', 14), ('m2-string-subscript-error-units', 2)]:
            result = fixtures.verify_package(fixtures.ROOT / 'tests/fixtures' / name)
            graph = result['graphFixtures'][0]
            self.assertEqual(result['caseCount'], count)
            self.assertEqual(len(graph['paths']), count)
            self.assertEqual(graph['logger'], {'callbackPaths': []})
            self.assertEqual(graph['actions'], [{'kind': 'path', 'index': i} for i in range(count)])
            units = [unit for row in graph['expected']['results']
                     for unit in row.get('javaValueUnits', row.get('javaErrorUnits', []))]
            self.assertIn(0xD83D, units)
            self.assertIn(0xDC00, units)

    def test_rehashed_uri_or_expression_cannot_replace_original(self):
        for field in ('moduleUri', 'expression'):
            root = self.copy()
            self.edit(root, 'cases.json', lambda data: data['cases'][0].update({field: 'changed'}))
            with self.assertRaisesRegex(ValueError, 'context'):
                fixtures.verify_package(root)

    def test_self_consistent_forged_context_with_deleted_binding_rejected(self):
        root = self.copy()
        forged = 'file:///forged-string-source.pkl'
        self.edit(root, 'origin/cases.json',
                  lambda rows: [row.update(moduleUri=forged) for row in rows])
        def mutate(data):
            data['moduleUri'] = forged
            for row in data['cases']:
                row['moduleUri'] = forged
        self.edit(root, 'cases.json', mutate)
        self.edit(root, 'provenance.json', lambda data: data.update(
            bindings=[row for row in data['bindings'] if row['path'] != 'origin/cases.json']))
        # All mutable manifests/declarations agree; the closed Java record does not.
        with self.assertRaisesRegex(ValueError, 'mandatory.*binding'):
            fixtures.verify_package(root)

    def test_rehashed_dropped_or_reordered_calls_rejected(self):
        for reorder in (False, True):
            root = self.copy()
            def mutate(data):
                if reorder:
                    data['cases'][0], data['cases'][1] = data['cases'][1], data['cases'][0]
                else:
                    data['cases'].pop()
            self.edit(root, 'cases.json', mutate)
            with self.assertRaisesRegex(ValueError, 'selection'):
                fixtures.verify_package(root)

    def test_rehashed_code_units_are_bound_to_closed_oracle(self):
        root = self.copy('m2-string-subscript-error-units')
        path = 'raw/0/outer-0.stderr.code-units.bin'
        raw = bytearray((root / path).read_bytes())
        offset = raw.index(b'\xd8\x3d')
        raw[offset:offset + 2] = b'\x00?'
        (root / path).write_bytes(raw)
        manifest = json.loads((root / 'manifest.json').read_text())
        manifest['files'][path] = hashlib.sha256(raw).hexdigest()
        (root / 'manifest.json').write_text(json.dumps(manifest))
        with self.assertRaisesRegex(ValueError, 'origin binding'):
            fixtures.verify_package(root)

    def test_forged_oracle_classification_and_rehashed_report_rejected(self):
        root = self.copy()
        self.edit(root, 'cases.json', lambda data: data['cases'][0]['expected'].update(oracleError=True))
        with self.assertRaisesRegex(ValueError, 'classification'):
            fixtures.verify_package(root)
        root = self.copy()
        self.edit(root, 'origin/report.json', lambda data: data.update(workspaceUnchanged=False))
        with self.assertRaisesRegex(ValueError, 'pinned oracle'):
            fixtures.verify_package(root)

if __name__ == '__main__':
    unittest.main()
