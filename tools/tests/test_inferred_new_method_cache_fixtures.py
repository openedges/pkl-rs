"""Adversarial offline checks for cached method-parent Java evidence."""
from __future__ import annotations

import json
from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest

TOOLS = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import inferred_new_method_cache_fixtures as fixture


def archived() -> dict[str, bytes]:
    result = {}
    with tarfile.open(fixture.FIXTURE / 'oracle-evidence.tar.gz', 'r:gz') as stream:
        for member in stream:
            result[member.name] = stream.extractfile(member).read()
    return result


class InferredNewMethodCacheFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.files = archived()

    def test_complete_archive_and_oracle_error_history(self):
        result = fixture.verify()
        self.assertEqual(result['captures']['method-capture-01']['statuses_per_repetition'],
                         {'success':18,'error':4})
        self.assertEqual(result['captures']['object-method-capture-01']['statuses_per_repetition'],
                         {'success':22})
        self.assertEqual(result['captures']['object-method-alias-bug-history-01']['statuses_per_repetition'],
                         {'success':14,'oracle-error':8})

    def test_missing_raw_utf16_channel_rejected(self):
        files = self.files.copy()
        name = next(name for name in files if name.startswith('method-capture-01/')
                    and name.endswith('/result.stdout.code-units.bin'))
        del files[name]
        with self.assertRaisesRegex(ValueError, '21-channel row set'):
            fixture.verify_capture(files,'method-capture-01')

    def test_oracle_error_cannot_be_relabelled_ordinary(self):
        files = self.files.copy()
        name = 'object-method-alias-bug-history-01/alpha-first/repeat-0/004-alias-alpha/result.status.bin'
        files[name] = b'error\n'
        with self.assertRaisesRegex(ValueError, 'order-specific raw observation'):
            fixture.verify_capture(files,'object-method-alias-bug-history-01')

    def test_oracle_error_cause_metadata_rejected(self):
        files = self.files.copy()
        name = 'object-method-alias-bug-history-01/alpha-first/repeat-0/004-alias-alpha/result.cause-class.bin'
        files[name] = b'org.pkl.core.PklException'
        with self.assertRaises(ValueError):
            fixture.verify_capture(files,'object-method-alias-bug-history-01')

    def test_mutated_pinned_type_source_rejected(self):
        files = self.files.copy()
        name = 'method-capture-01/sources/external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/UnresolvedTypeNode.java'
        files[name] += b'\n'
        with self.assertRaisesRegex(ValueError, 'source copy differs'):
            fixture.verify_capture(files,'method-capture-01')

    def test_mutated_sibling_module_rejected(self):
        files = self.files.copy()
        files['object-method-capture-01/alpha-first/base.pkl'] += b'\n'
        with self.assertRaisesRegex(ValueError, 'sibling module copy differs'):
            fixture.verify_capture(files,'object-method-capture-01')

    def test_session_order_rejected(self):
        files = self.files.copy()
        name = 'method-capture-01/report.json'
        report = json.loads(files[name])
        report['sessions'][0],report['sessions'][1] = report['sessions'][1],report['sessions'][0]
        files[name] = (json.dumps(report,sort_keys=True)+'\n').encode()
        with self.assertRaisesRegex(ValueError, 'captured report identity differs'):
            fixture.verify_capture(files,'method-capture-01')

    def test_complete_process_protocol_rejected(self):
        files = self.files.copy()
        name = 'object-method-capture-01/alpha-first/repeat-0/process.stdout.bin'
        files[name] = files[name].replace(b'session-open mode=string rows=11',
                                          b'session-open mode=string rows=10')
        with self.assertRaisesRegex(ValueError, 'raw process streams differ'):
            fixture.verify_capture(files,'object-method-capture-01')

    def test_undeclared_fixture_root_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'fixture'
            shutil.copytree(fixture.FIXTURE,root)
            (root/'extra.bin').write_bytes(b'undeclared')
            with self.assertRaisesRegex(ValueError, 'fixture root file set differs'):
                fixture.verify(root)


if __name__ == '__main__':
    unittest.main()
