import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import moduleinfo_fixtures as fixtures
SOURCE = TOOLS.parent / 'tests/fixtures/m2-moduleinfo'

def refresh(root):
    files = {}
    for path in root.rglob('*'):
        if path.is_file() and path != root / 'manifest.json':
            raw = path.read_bytes()
            files[str(path.relative_to(root))] = {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
    (root / 'manifest.json').write_text(json.dumps({'schema': 1, 'files': files}))

class ModuleInfoFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name) / 'fixtures'
        shutil.copytree(SOURCE, self.root)
    def test_complete_accounting_never_promotes_internal_or_followup_calls(self):
        result = fixtures.verify(self.root)
        self.assertEqual(result['sources'], 28)
        self.assertEqual(result['ordinaryRawComparisons'], 156)
        self.assertEqual(result['javaInternalBoundaries'], 12)
        self.assertEqual(result['unexecutedNativeSharedEvaluatorFollowups'], 56)
    def test_internal_boundary_promotion_rejected_after_rehash(self):
        selection = json.loads((self.root / 'selection.json').read_text())
        next(r for r in selection['cases'] if r['id'] == 'annotation-arabic-digits')['comparisonScope'] = 'ordinary-full-diagnostic'
        (self.root / 'selection.json').write_text(json.dumps(selection)); refresh(self.root)
        with self.assertRaises(ValueError): fixtures.verify(self.root)
    def test_dropped_same_evaluator_followup_rejected_after_rehash(self):
        (self.root / 'oracle/repeats/1/calls/annotation-future/followup-0.stdout.bin').unlink(); refresh(self.root)
        with self.assertRaises((ValueError, FileNotFoundError)): fixtures.verify(self.root)
    def test_utf16_tampering_is_rejected_with_utf8_unchanged(self):
        path = self.root / 'oracle/repeats/0/calls/construct-arabic-digits/outer-0.stdout.code-units.bin'
        raw = path.read_bytes(); path.write_bytes(b'\x00\x30' + raw[2:]); refresh(self.root)
        with self.assertRaises(ValueError): fixtures.verify(self.root)
    def test_self_consistent_uri_and_binding_forgery_rejected(self):
        selection = json.loads((self.root / 'selection.json').read_text())
        row = selection['cases'][0]; row['moduleUri'] = 'file:///forged-moduleinfo.pkl'
        (self.root / row['sourceFile']).with_name('uri.txt').write_text(row['moduleUri'])
        (self.root / 'selection.json').write_text(json.dumps(selection))
        provenance = json.loads((self.root / 'provenance.json').read_text()); provenance['bindings'].pop()
        (self.root / 'provenance.json').write_text(json.dumps(provenance)); refresh(self.root)
        with self.assertRaises(ValueError): fixtures.verify(self.root)
    def test_unlisted_directory_symlink_rejected(self):
        (self.root / 'extra-directory').symlink_to(self.root / 'oracle', target_is_directory=True)
        with self.assertRaisesRegex(ValueError, 'directory symlink'): fixtures.verify(self.root)
    def test_unknown_membership_rejected_even_after_rehash(self):
        (self.root / 'extra.bin').write_bytes(b'extra'); refresh(self.root)
        with self.assertRaises(ValueError): fixtures.verify(self.root)

if __name__ == '__main__': unittest.main()
