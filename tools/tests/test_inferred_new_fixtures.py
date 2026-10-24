"""Adversarial offline checks for both Stage65 inferred-new Java profiles."""
from __future__ import annotations

import pathlib
import shutil
import sys
import tarfile
import tempfile
import unittest

TOOLS = pathlib.Path(__file__).resolve().parents[1]
sys.path.insert(0, str(TOOLS))
import inferred_new_fixtures as fixture


def archived(profile):
    files = {}
    with tarfile.open(fixture.FIXTURES[profile] / 'oracle-evidence.tar.gz', 'r:gz') as stream:
        for member in stream:
            files[member.name] = stream.extractfile(member).read()
    return files


class InferredNewFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.values = archived('values')
        cls.amends = archived('amends')

    def test_canonical_values_full_archive(self):
        result = fixture.verify('values')
        self.assertEqual(result['statuses'], {'success': 49, 'error': 23})

    def test_canonical_amends_full_archive(self):
        result = fixture.verify('amends')
        self.assertEqual(result['statuses'], {'success': 7})

    def test_missing_utf16_channel_rejected(self):
        files = self.values.copy()
        del files['capture-01/repeat-0/000-module-self-type/result.stdout.code-units.bin']
        with self.assertRaisesRegex(ValueError, '21-channel row set'):
            fixture.validate_capture(files, 'values')

    def test_mutated_source_copy_rejected(self):
        files = self.values.copy()
        name = 'capture-01/sources/external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/GetParentForTypeNode.java'
        files[name] += b'\n'
        with self.assertRaisesRegex(ValueError, 'source copy differs'):
            fixture.validate_capture(files, 'values')

    def test_amends_sibling_base_rejected(self):
        files = self.amends.copy()
        files['capture-01/base.pkl'] += b'\n'
        with self.assertRaisesRegex(ValueError, 'captured input differs'):
            fixture.validate_capture(files, 'amends')

    def test_amends_session_uri_rejected(self):
        files = self.amends.copy()
        import json
        name = 'capture-01/provenance.json'
        row = json.loads(files[name]); row['module_uri'] = 'file:///wrong/module.pkl'
        files[name] = (json.dumps(row, indent=2, sort_keys=True) + '\n').encode()
        with self.assertRaisesRegex(ValueError, 'recorded module URI differs'):
            fixture.validate_capture(files, 'amends')

    def test_process_protocol_rejected(self):
        files = self.values.copy()
        name = 'capture-01/repeat-0/process.stdout.bin'
        files[name] = files[name].replace(b'call-exit=string-0-module-self-type status=success',
                                          b'call-exit=string-0-module-self-type status=error')
        with self.assertRaisesRegex(ValueError, 'raw process stream differs'):
            fixture.validate_capture(files, 'values')

    def test_undeclared_fixture_root_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            dst = pathlib.Path(temp) / 'fixture'
            shutil.copytree(fixture.FIXTURES['values'], dst)
            (dst / 'extra.bin').write_bytes(b'not declared')
            with self.assertRaisesRegex(ValueError, 'fixture root file set differs'):
                fixture.verify('values', dst)


if __name__ == '__main__':
    unittest.main()
