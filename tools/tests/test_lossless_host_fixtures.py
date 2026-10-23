import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location('lossless_host_fixtures', ROOT/'tools/lossless_host_fixtures.py')
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)

class LosslessHostFixturesTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.fixtures = Path(self.temporary.name)/'fixtures'
        shutil.copytree(ROOT/'tests/fixtures/m2-lossless-host', self.fixtures)

    def test_all_original_calls_and_nullable_inputs_are_exported(self):
        data = module.verify(self.fixtures)
        self.assertEqual(data['graphCount'], 11)
        self.assertEqual(data['actualJavaEvaluations'], 80)
        self.assertEqual(sum(len(c['paths']) for c in data['graphFixtures']), 40)
        cases = {case['id']: case for case in data['graphFixtures']}
        null = cases['module-resolve-null']['resolutions'][0]['resolves'][0]['failure']
        empty = cases['module-resolve-empty']['resolutions'][0]['resolves'][0]['failure']
        self.assertIsNone(null['message'])
        self.assertEqual(empty['message'], '')
        units = cases['module-resolve-units']['resolutions'][0]['resolves'][0]['failure']['message']
        self.assertEqual(units.encode('utf-16-be', errors='surrogatepass'),
            (self.fixtures/'cases/module-resolve-units/0/calls/cause.code-units.bin').read_bytes())
        self.assertEqual(cases['unknown-output-format']['paths'], ['output.text', 'output.text'])
        self.assertEqual(cases['unknown-output-format']['resources'][0]['uri'], 'prop:pkl.outputFormat')

    def test_refreshed_input_declarations_cannot_change_source_or_uri(self):
        path = self.fixtures/'cases.json'
        cases = json.loads(path.read_text())
        cases[0]['moduleUri'] = 'file:///forged.pkl'
        cases[0]['source'] = 'result = 7\n'
        cases[0]['sourceSha256'] = hashlib.sha256(cases[0]['source'].encode()).hexdigest()
        path.write_text(json.dumps(cases))
        inventory = json.loads((self.fixtures/'inventory.json').read_text())
        inventory['cases.json']['sha256'] = hashlib.sha256(path.read_bytes()).hexdigest()
        (self.fixtures/'inventory.json').write_text(json.dumps(inventory))
        closure = json.loads((self.fixtures/'closure.json').read_text())
        closure['inventorySha256'] = hashlib.sha256((self.fixtures/'inventory.json').read_bytes()).hexdigest()
        (self.fixtures/'closure.json').write_text(json.dumps(closure))
        with self.assertRaisesRegex(ValueError, 'pinned oracle identity'):
            module.verify(self.fixtures)

    def test_refreshed_raw_inventory_cannot_collapse_null_to_empty(self):
        path = self.fixtures/'cases/resource-null/0/calls/cause-null.bin'
        path.write_bytes(b'false')
        (path.parent/'cause.bin').write_bytes(b'')
        (path.parent/'cause.code-units.bin').write_bytes(b'')
        inventory_path = self.fixtures/'inventory.json'
        inventory = json.loads(inventory_path.read_text())
        for raw in [path, path.parent/'cause.bin', path.parent/'cause.code-units.bin']:
            inventory[str(raw.relative_to(self.fixtures))] = {'sha256': hashlib.sha256(raw.read_bytes()).hexdigest(), 'bytes': raw.stat().st_size}
        inventory_path.write_text(json.dumps(inventory))
        closure_path = self.fixtures/'closure.json'
        closure = json.loads(closure_path.read_text())
        closure['inventorySha256'] = hashlib.sha256(inventory_path.read_bytes()).hexdigest()
        closure_path.write_text(json.dumps(closure))
        with self.assertRaisesRegex(ValueError, 'pinned oracle identity'):
            module.verify(self.fixtures)

    def test_missing_late_call_or_wrong_units_are_rejected(self):
        raw = self.fixtures/'cases/resource-units/1/calls/call-3.stdout.code-units.bin'
        original = raw.read_bytes()
        raw.unlink()
        with self.assertRaises(FileNotFoundError):
            module.verify(self.fixtures)
        raw.write_bytes(original + b'\xd8\x00')
        with self.assertRaisesRegex(ValueError, 'original raw binding'):
            module.verify(self.fixtures)

if __name__ == '__main__':
    unittest.main()
