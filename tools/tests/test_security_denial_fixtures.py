import hashlib
import json
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
TOOLS=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(TOOLS))
import security_denial_fixtures as fixture
SOURCE=TOOLS.parent/'tests/fixtures/m2-security-denial'
def refresh(root):
    files={str(p.relative_to(root)):{'bytes':p.stat().st_size,'sha256':hashlib.sha256(p.read_bytes()).hexdigest()}for p in root.rglob('*')if p.is_file()and p!=root/'manifest.json'}
    (root/'manifest.json').write_text(json.dumps({'schema':1,'files':files}))
class SecurityDenialFixtureTests(unittest.TestCase):
    def setUp(self):
        self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup);self.root=Path(self.temp.name)/'fixtures';shutil.copytree(SOURCE,self.root)
    def test_actual_policy_typed_provenance_and_all_calls(self):
        result=fixture.verify(self.root);self.assertEqual(result['apiCalls'],10);self.assertEqual(result['actualSecurityManagerExceptions'],6);self.assertEqual(result['completionKind'],'security-manager-exception')
    def test_raw_denied_kind_cannot_be_substituted(self):
        context=json.loads((self.root/'context.json').read_text());context['completionKind']='denied';(self.root/'context.json').write_text(json.dumps(context));refresh(self.root)
        with self.assertRaises(ValueError):fixture.verify(self.root)
    def test_source_stage_cannot_be_promoted_to_checked_java_contract(self):
        context=json.loads((self.root/'context.json').read_text());context['completionStage']='load-source';(self.root/'context.json').write_text(json.dumps(context));refresh(self.root)
        with self.assertRaises(ValueError):fixture.verify(self.root)
    def test_raw_cause_class_forgery_rejected(self):
        (self.root/'oracle/repeats/0/calls/denial-0.error-class.bin').write_bytes(b'generic-host');refresh(self.root)
        with self.assertRaises(ValueError):fixture.verify(self.root)
    def test_missing_cached_followup_rejected(self):
        (self.root/'oracle/repeats/1/calls/outer-4.stdout.code-units.bin').unlink();refresh(self.root)
        with self.assertRaises((ValueError,FileNotFoundError)):fixture.verify(self.root)
    def test_uri_and_ordinary_error_forgery_rejected(self):
        context=json.loads((self.root/'context.json').read_text());context['deniedUri']='file:///forged.pkl';(self.root/'context.json').write_text(json.dumps(context));(self.root/'denied-uri.txt').write_text(context['deniedUri']);refresh(self.root)
        with self.assertRaises(ValueError):fixture.verify(self.root)
    def test_unlisted_directory_symlink_rejected(self):
        (self.root/'unlisted').symlink_to(self.root/'oracle',target_is_directory=True)
        with self.assertRaisesRegex(ValueError,'directory symlink'):fixture.verify(self.root)
if __name__=='__main__':unittest.main()
