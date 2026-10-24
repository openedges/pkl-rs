"""Adversarial offline tests for inferred Listing default diagnostics."""
from __future__ import annotations

from pathlib import Path
import shutil
import sys
import tarfile
import tempfile
import unittest

TOOLS=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(TOOLS))
import inferred_new_default_diagnostics_fixtures as fixture


def archived()->dict[str,bytes]:
    result={}
    with tarfile.open(fixture.FIXTURE/'oracle-evidence.tar.gz','r:gz') as stream:
        for member in stream:result[member.name]=stream.extractfile(member).read()
    return result


class InferredNewDefaultDiagnosticsFixtures(unittest.TestCase):
    @classmethod
    def setUpClass(cls):cls.files=archived()

    def test_full_archive_and_four_separate_programs(self):
        result=fixture.verify()
        self.assertEqual(result['fresh_jvms'],8)
        self.assertEqual(result['statuses_per_repetition'],{'success':2,'error':2})

    def test_missing_utf16_channel_rejected(self):
        files=self.files.copy()
        del files['capture-01/inputs/any/repeat-0/000-result/result.stderr.code-units.bin']
        with self.assertRaisesRegex(ValueError,'21-channel row set'):
            fixture.verify_capture(files)

    def test_ordinary_error_relabelled_success_rejected(self):
        files=self.files.copy()
        files['capture-01/inputs/any/repeat-0/000-result/result.status.bin']=b'success\n'
        with self.assertRaisesRegex(ValueError,'ordinary error/success boundary'):
            fixture.verify_capture(files)

    def test_missing_default_frame_rejected(self):
        files=self.files.copy()
        name='capture-01/inputs/alias-unknown/repeat-0/000-result/result.stderr.bin'
        files[name]=files[name].replace(b'at module#make.default',b'at module#make.changed')
        files[name.replace('.bin','.code-units.bin')]=files[name].decode().encode('utf-16-be')
        with self.assertRaisesRegex(ValueError,'complete undefined-default frames'):
            fixture.verify_capture(files)

    def test_mutated_source_copy_rejected(self):
        files=self.files.copy()
        name='capture-01/sources/external/pkl/pkl-core/src/main/java/org/pkl/core/ast/member/DefaultPropertyBodyNode.java'
        files[name]+=b'\n'
        with self.assertRaisesRegex(ValueError,'pinned source copy differs'):
            fixture.verify_capture(files)

    def test_process_protocol_rejected(self):
        files=self.files.copy()
        name='capture-01/inputs/literal-unknown/repeat-0/process.stdout.bin'
        files[name]=files[name].replace(b'rows=1',b'rows=2')
        with self.assertRaisesRegex(ValueError,'raw Java process stream differs'):
            fixture.verify_capture(files)

    def test_undeclared_root_entry_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root=Path(temporary)/'fixture'
            shutil.copytree(fixture.FIXTURE,root)
            (root/'extra.bin').write_bytes(b'undeclared')
            with self.assertRaisesRegex(ValueError,'fixture root file set differs'):
                fixture.verify(root)


if __name__=='__main__':unittest.main()
