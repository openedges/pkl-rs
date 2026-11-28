"""Offline synthetic fixture and failure-contract tests; no Java execution."""
import json
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
import zipfile

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_package_fixtures as fixtures


class QualificationTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.addCleanup(patch.stopall)
        patch.object(fixtures, 'ROOT', self.root).start()
        patch.object(fixtures, 'EXPECTED_METADATA', {}).start()
        self.preparation = self.root / 'preparation'
        self.generated = self.root / 'generated'
        sources = self.root / 'external/pkl/pkl-commons-test/src/main/files/packages'
        entries = []
        for index, name in enumerate(fixtures.PACKAGES):
            bodies = {f'package/File{number}.pkl': f'value = {number}\n'.encode()
                      for number in range(3 if index < 4 else 2)}
            bodies[name + '.json'] = (b'this is not json\n' if name.startswith('badMetadataJson')
                                      else b'{"checksum":"$computedChecksum"}\n')
            for relative, body in bodies.items():
                source = sources / name / relative
                staged = self.preparation / 'inputs/packages' / name / relative
                for path in (source, staged):
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(body)
                entries.append(dict(source=fixtures.pin(source), staged=fixtures.pin(staged)))
            directory = self.generated / name
            directory.mkdir(parents=True)
            archive_path = directory / (name + '.zip')
            with zipfile.ZipFile(archive_path, 'w') as archive:
                for relative, body in bodies.items():
                    if relative.startswith('package/'):
                        archive.writestr(relative.removeprefix('package/'), body)
            metadata = bodies[name + '.json'].replace(b'$computedChecksum',
                                                    fixtures.pin(archive_path)['sha256'].encode())
            (directory / (name + '.json')).write_bytes(metadata)
            (directory / (name + '.json.sha256')).write_bytes(compat.digest(metadata).encode())
        recipe = self.preparation / 'build.gradle'
        recipe.write_text('// synthetic offline recipe\n')
        self.manifest = self.preparation / 'manifest.json'
        fixtures.write_json(self.manifest, dict(package_names=list(fixtures.PACKAGES),
            upstream_revision='e51fb2f91774265666f94980ec630db794a4db2d',
            input_count=19, inputs=entries, recipe_files=[fixtures.pin(recipe)],
            upstream_recipe_and_notices=[]))

    def qualify(self, suffix='qualified'):
        return fixtures.qualify_generated(self.generated, self.manifest, self.root / suffix)

    def test_qualified_bytes_revalidated_and_malformed_json_preserved(self):
        path = self.qualify()
        record = fixtures.load_verified_manifest(path)
        self.assertEqual(len(record['files']), 15)
        metadata = self.generated / 'badMetadataJson@1.0.0/badMetadataJson@1.0.0.json'
        self.assertEqual(metadata.read_bytes(), b'this is not json\n')
        metadata.write_bytes(b'this is not json\r\n')
        with self.assertRaisesRegex(ValueError, 'canonical source filtering'):
            fixtures.load_verified_manifest(path)

    def test_missing_extra_and_symlink_files_are_not_qualified(self):
        file = self.generated / 'birds@0.5.0/birds@0.5.0.json.sha256'
        body = file.read_bytes()
        file.unlink()
        with self.assertRaisesRegex(ValueError, 'membership'):
            self.qualify('missing')
        self.assertEqual(json.loads((self.root / 'missing/failure.json').read_bytes())['status'],
                         'qualification-failed')
        file.write_bytes(body)
        extra = self.generated / 'extra'
        extra.write_bytes(b'extra')
        with self.assertRaisesRegex(ValueError, 'membership'):
            self.qualify('extra')
        extra.unlink()
        file.unlink()
        file.symlink_to(self.manifest)
        with self.assertRaisesRegex(ValueError, 'non-regular'):
            self.qualify('symlink')

    def test_sidecar_and_fixed_checksum_failure_cannot_be_relabelled(self):
        file = self.generated / 'birds@0.5.0/birds@0.5.0.json.sha256'
        body = file.read_bytes()
        file.write_bytes(body + b'\n')
        with self.assertRaisesRegex(ValueError, 'sidecar'):
            self.qualify('sidecar')
        file.write_bytes(body)
        with patch.object(fixtures, 'EXPECTED_METADATA', {'birds@0.5.0': '0' * 64}):
            with self.assertRaisesRegex(ValueError, 'pinned metadata'):
                self.qualify('fixed')

    def test_source_mutation_and_manifest_pin_mutation_rejected(self):
        record = fixtures.read_json(self.manifest)
        source = Path(record['inputs'][0]['source']['path'])
        source.write_bytes(b'changed source')
        with self.assertRaisesRegex(ValueError, 'provenance changed'):
            self.qualify('source')

    def test_corrupt_zip_payload_rejected_even_with_consistent_metadata_checksums(self):
        name = 'birds@0.5.0'
        directory = self.generated / name
        archive_path = directory / (name + '.zip')
        with zipfile.ZipFile(archive_path, 'w') as archive:
            for number in range(3):
                archive.writestr(f'File{number}.pkl', b'changed payload\n')
        metadata = b'{"checksum":"' + fixtures.pin(archive_path)['sha256'].encode() + b'"}\n'
        (directory / (name + '.json')).write_bytes(metadata)
        (directory / (name + '.json.sha256')).write_bytes(compat.digest(metadata).encode())
        with self.assertRaisesRegex(ValueError, 'ZIP payload differs'):
            self.qualify('zip-payload')

    def test_existing_output_is_never_reused(self):
        self.qualify()
        with self.assertRaises(FileExistsError):
            self.qualify()


class SafetyAndProcessTests(unittest.TestCase):
    def test_forbidden_paths_rejected_before_stat(self):
        with patch.object(Path, 'stat', side_effect=AssertionError('must not stat')), \
                patch.object(Path, 'lstat', side_effect=AssertionError('must not lstat')):
            for name in ('stage86/anything', 'Private8/file',
                         'stage65-b-native-regressions-evidence.tar.gz.partial',
                         'stage136-evidence/body.zip'):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    fixtures.pin(name)

    def test_process_failure_retains_complete_raw_channels(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            observed = dict(stdout=b'raw\x00stdout\n', stderr=b'bad\r\n', exit_code=7,
                            elapsed_ns=1, timed_out=False, files={})
            with patch.object(compat, 'capture', return_value=observed):
                with self.assertRaisesRegex(RuntimeError, 'failed'):
                    fixtures.run_process(['not-executed'], root, 'failing', {})
            self.assertEqual((root / 'failing/stdout.bin').read_bytes(), observed['stdout'])
            self.assertEqual((root / 'failing/stderr.bin').read_bytes(), observed['stderr'])
            self.assertEqual(fixtures.read_json(root / 'failing/result.json')['exit_code'], 7)

    def test_startup_failure_and_forced_shutdown_are_not_passes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            with patch.object(fixtures, 'load_verified_manifest', return_value={}), \
                    patch.object(fixtures.FixtureServer, '_start', side_effect=RuntimeError('start failed')):
                with self.assertRaisesRegex(RuntimeError, 'start failed'):
                    with fixtures.FixtureServer(root / 'manifest', {}, root / 'failed'):
                        self.fail('must not enter')
            self.assertEqual(fixtures.read_json(root / 'failed/server-result.json')['startup_error'],
                             'start failed')
            server = fixtures.FixtureServer(root / 'manifest', {}, root / 'forced')
            server.output.mkdir()
            process = Mock(pid=123, returncode=-9)
            process.wait.side_effect = [subprocess.TimeoutExpired('java', 5), -9]
            server.process = process
            with patch.object(fixtures, 'load_verified_manifest', return_value={}), \
                    patch.object(fixtures.os, 'killpg') as kill:
                with self.assertRaisesRegex(RuntimeError, 'shutdown'):
                    server.__exit__(None, None, None)
            kill.assert_called_once_with(123, signal.SIGKILL)
            self.assertTrue(fixtures.read_json(root / 'forced/server-result.json')['forced_shutdown'])


class RouteChecks(unittest.TestCase):
    def test_bad_body_status_and_redirect_are_retained_without_short_circuit(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            for name in fixtures.PACKAGES:
                directory = root / 'packages' / name
                directory.mkdir(parents=True)
                for suffix in ('.json', '.zip'):
                    (directory / (name + suffix)).write_bytes(b'expected\x00\n')
            server = Mock(output=root, port=12345, ca_path=root / 'certificate')
            server.fixture = dict(fixture_root=str(root / 'packages'))
            response = Mock(status=200)
            response.read.return_value = b'wrong\r\n'
            response.getheader.return_value = '/wrong'
            response.getheaders.return_value = [('Location', '/wrong')]
            connection = Mock()
            connection.getresponse.return_value = response
            with patch.object(fixtures.ssl, 'create_default_context'), \
                    patch.object(fixtures.http.client, 'HTTPSConnection', return_value=connection):
                with self.assertRaisesRegex(RuntimeError, 'route checks failed'):
                    fixtures.check_server(server)
            report = fixtures.read_json(root / 'route-checks/report.json')
            self.assertFalse(report['passed'])
            self.assertEqual(len(report['checks']), 14)
            self.assertEqual((root / 'route-checks/0.body.bin').read_bytes(), b'wrong\r\n')
            self.assertTrue(all(not row['passed'] for row in report['checks']))


class PurePythonPackageBuildTests(unittest.TestCase):
    def test_build_upstream_packages_matches_pinned_gradle_checksums_without_gradle(self):
        with tempfile.TemporaryDirectory() as temporary:
            stage = Path(temporary).resolve() / 'staged'
            with patch.object(fixtures.subprocess, 'run', side_effect=AssertionError('must not invoke Gradle')):
                out = fixtures.build_upstream_packages(stage)
            self.assertEqual(out, stage)
            for name in fixtures.ALL_PACKAGES:
                pkg_dir = stage / name
                self.assertTrue((pkg_dir / f'{name}.zip').is_file())
                self.assertTrue((pkg_dir / f'{name}.json').is_file())
                sidecar = (pkg_dir / f'{name}.json.sha256').read_text(encoding='ascii')
                self.assertEqual(sidecar, compat.digest((pkg_dir / f'{name}.json').read_bytes()))
                with zipfile.ZipFile(pkg_dir / f'{name}.zip') as zf:
                    self.assertIsNone(zf.testzip())
            for name, expected in fixtures.EXPECTED_METADATA.items():
                actual = compat.digest((stage / name / f'{name}.json').read_bytes())
                self.assertEqual(actual, expected)

    def test_reproducible_zip_rejects_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            (root / 'ok.pkl').write_bytes(b'x = 1\n')
            (root / 'link.pkl').symlink_to(root / 'ok.pkl')
            with self.assertRaisesRegex(ValueError, 'non-regular'):
                fixtures._build_reproducible_gradle_zip(root)


if __name__ == '__main__':
    unittest.main()
