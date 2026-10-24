"""Adversarial offline checks for inherited module receiver authority."""
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
import inferred_new_module_receiver_fixtures as fixture


def archived() -> dict[str, bytes]:
    result = {}
    with tarfile.open(fixture.FIXTURE / 'oracle-evidence.tar.gz', 'r:gz') as stream:
        for member in stream:
            result[member.name] = stream.extractfile(member).read()
    return result


class InferredNewModuleReceiverFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.files = archived()

    def test_complete_canonical_archive(self):
        result = fixture.verify()
        self.assertEqual(result['sessions'], 4)
        self.assertEqual(result['statuses_per_repetition'], {'success': 16, 'error': 6})

    def test_missing_utf16_channel_rejected(self):
        files = self.files.copy()
        name = next(name for name in files if name.endswith('/result.stdout.code-units.bin'))
        del files[name]
        with self.assertRaisesRegex(ValueError, '21-channel row set'):
            fixture.verify_capture(files)

    def test_mutated_source_copy_rejected(self):
        files = self.files.copy()
        name = 'capture-01/sources/external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/UnresolvedTypeNode.java'
        files[name] += b'\n'
        with self.assertRaisesRegex(ValueError, 'source copy differs'):
            fixture.verify_capture(files)

    def test_mutated_sibling_module_rejected(self):
        files = self.files.copy()
        files['capture-01/alpha-first/base.pkl'] += b'\n'
        with self.assertRaisesRegex(ValueError, 'sibling module source differs'):
            fixture.verify_capture(files)

    def test_swapped_order_specific_error_rejected(self):
        files = self.files.copy()
        name = 'capture-01/beta-first/repeat-0/001-alpha-module-is-alpha/result.stderr.bin'
        files[name] = files[name].replace(b'`beta`', b'`alpha`')
        with self.assertRaises(ValueError):
            fixture.verify_capture(files)

    def test_session_order_rejected(self):
        files = self.files.copy()
        name = 'capture-01/report.json'
        report = json.loads(files[name])
        report['sessions'][0], report['sessions'][1] = report['sessions'][1], report['sessions'][0]
        files[name] = (json.dumps(report, sort_keys=True) + '\n').encode()
        with self.assertRaisesRegex(ValueError, 'four fresh ordered JVMs'):
            fixture.verify_capture(files)

    def test_process_protocol_rejected(self):
        files = self.files.copy()
        name = 'capture-01/alpha-first/repeat-0/process.stdout.bin'
        files[name] = files[name].replace(b'session-open mode=string rows=11', b'session-open mode=string rows=10')
        with self.assertRaisesRegex(ValueError, 'raw JVM process stream differs'):
            fixture.verify_capture(files)

    def test_undeclared_fixture_root_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / 'fixture'
            shutil.copytree(fixture.FIXTURE, root)
            (root / 'extra.bin').write_bytes(b'undeclared')
            with self.assertRaisesRegex(ValueError, 'fixture root file set differs'):
                fixture.verify(root)


if __name__ == '__main__':
    unittest.main()
