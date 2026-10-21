"""Failure detection for persistent class evidence, independent of Pkl semantics."""
import copy
import hashlib
import json
import shutil
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_class_lifecycle as tool


class ClassLifecycleEvidenceTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.calls = ['trigger', 'trigger', 'early']

    def process(self, directory, values=None):
        directory.mkdir(parents=True)
        values = values or [('error', b'', b'full error \n'), ('success', b'true', b''), ('success', b'10', b'')]
        result = {'status': 'executed', 'exit_code': 0}
        protocol = ''.join(f'call={i} completed\n' for i in range(len(values))).encode() + b'closed\n'
        for name, data in [('stdout', protocol), ('stderr', b'')]:
            (directory / (name + '.bin')).write_bytes(data)
            result[name] = {'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()}
        for index, (status, stdout, stderr) in enumerate(values):
            for name, data in [('status', (status + '\n').encode()), ('stdout', stdout), ('stderr', stderr), ('error-class', b'')]:
                (directory / f'{index}.{name}.bin').write_bytes(data)
        return result

    def pair(self):
        row = {language: self.process(self.root / language) for language in ['java', 'rust']}
        return row

    def test_raw_whitespace_difference_is_preserved(self):
        row = self.pair()
        (self.root / 'rust/0.stderr.bin').write_bytes(b'full error\n')
        result = tool.observe_pair(self.root, row, self.calls, ['java', 'rust'])
        self.assertEqual([r['status'] for r in result['comparisons']], ['mismatch', 'pass', 'pass'])

    def test_missing_duplicate_or_reordered_protocol_fails_acquisition(self):
        for protocol in [b'', b'call=0 completed\nclosed\n', b'call=1 completed\ncall=0 completed\ncall=2 completed\nclosed\n']:
            with tempfile.TemporaryDirectory() as temporary:
                directory = Path(temporary)
                result = self.process(directory / 'rust')
                (directory / 'rust/stdout.bin').write_bytes(protocol)
                result['stdout'] = {'bytes': len(protocol), 'sha256': hashlib.sha256(protocol).hexdigest()}
                self.assertEqual(tool.observe_process(directory / 'rust', result, self.calls)['status'], 'acquisition-failed')

    def test_process_failure_does_not_grant_call_passes(self):
        row = self.pair()
        for key, value in [('exit_code', 2), ('status', 'timeout')]:
            changed = copy.deepcopy(row); changed['rust'][key] = value
            result = tool.observe_pair(self.root, changed, self.calls, ['java', 'rust'])
            self.assertTrue(all(r['status'] == 'unexecuted' for r in result['comparisons']))

    def test_missing_extra_or_inconsistent_call_capture_rejected(self):
        row = self.pair()
        extra = self.root / 'rust/3.status.bin'; extra.write_bytes(b'success\n')
        with self.assertRaisesRegex(ValueError, 'extra'):
            tool.observe_pair(self.root, row, self.calls, ['java', 'rust'])
        extra.unlink()
        (self.root / 'rust/0.stderr.bin').write_bytes(b'')
        with self.assertRaisesRegex(ValueError, 'inconsistent'):
            tool.observe_pair(self.root, row, self.calls, ['java', 'rust'])

    def test_oracle_internal_error_never_passes_even_when_equal(self):
        row = self.pair()
        for language in ['java', 'rust']:
            (self.root / language / '0.status.bin').write_bytes(b'oracle-error\n')
        result = tool.observe_pair(self.root, row, self.calls, ['java', 'rust'])
        self.assertTrue(result['comparisons'][0]['raw_equal'])
        self.assertEqual(result['comparisons'][0]['status'], 'mismatch')

    def test_java_only_and_empty_evidence_cannot_pass_contract(self):
        self.assertFalse(tool.contract_passes({'languages': ['java'], 'cases': [{'observations': {'comparisons': []}}]}))
        self.assertFalse(tool.contract_passes({'languages': ['java', 'rust'], 'cases': []}))

    def test_process_channel_tampering_rejected(self):
        row = self.pair()
        (self.root / 'rust/stdout.bin').write_bytes(b'changed')
        with self.assertRaisesRegex(ValueError, 'identity'):
            tool.observe_pair(self.root, row, self.calls, ['java', 'rust'])

    def test_command_provenance_rejects_swapped_source_and_result(self):
        directory = self.root / 'java'
        result = self.process(directory)
        result['command'] = ['probe', 'source.pkl', 'calls.expressions', str(directory)]
        result['cwd'] = '/workspace'
        (directory / 'result.json').write_text(json.dumps(result))
        (self.root / 'report.json').write_text(json.dumps({'workspace': '/workspace', 'output_directory': str(self.root)}))
        # Synthetic paths are made consistent with the subprocess capture contract.
        for name in ['stdout', 'stderr']:
            result[name]['path'] = str(directory / (name + '.bin'))
        (directory / 'result.json').write_text(json.dumps(result))
        tool.checked_command(self.root, Path('java'), result, result['command'])
        with self.assertRaisesRegex(ValueError, 'command'):
            tool.checked_command(self.root, Path('java'), result, ['probe', 'other.pkl', 'calls.expressions', str(directory)])
        altered = copy.deepcopy(result); altered['exit_code'] = 1
        with self.assertRaisesRegex(ValueError, 'retained result'):
            tool.checked_command(self.root, Path('java'), altered, result['command'])

    def test_command_validation_reads_relocated_bytes_but_preserves_original_paths(self):
        import shutil
        original = self.root / 'original'
        directory = original / 'java'
        result = self.process(directory)
        result['command'] = ['probe', str(original / 'source.pkl')]
        result['cwd'] = '/workspace'
        for name in ['stdout', 'stderr']:
            result[name]['path'] = str(directory / (name + '.bin'))
        (directory / 'result.json').write_text(json.dumps(result))
        (original / 'report.json').write_text(json.dumps({'workspace': '/workspace', 'output_directory': str(original)}))
        relocated = self.root / 'relocated'
        shutil.copytree(original, relocated)
        tool.checked_command(relocated, Path('java'), result, result['command'])
        (relocated / 'java/stdout.bin').write_bytes(b'changed at relocated path')
        with self.assertRaisesRegex(ValueError, 'identity'):
            tool.checked_command(relocated, Path('java'), result, result['command'])

    def test_missing_required_tool_source_is_rejected(self):
        with self.assertRaisesRegex(ValueError, 'required lifecycle source'):
            tool.source_paths(self.root, java_only=True)

    def report_fixture(self):
        # Synthetic transport evidence exercises the verifier, never Pkl compatibility.
        sources = []
        for path in tool.source_paths(java_only=True):
            relative = path.relative_to(tool.ROOT)
            destination = self.root / 'sources' / relative
            destination.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(path, destination)
            sources.append({'path': relative.as_posix(), 'snapshot': 'sources/' + relative.as_posix(),
                            'bytes': destination.stat().st_size, 'sha256': tool.oracle.sha256(destination)})
        lock = json.loads((self.root / 'sources/oracle.lock.json').read_bytes())
        metadata = {'java': '/pinned/java', 'artifact': '/pinned/oracle.jar',
                    'command': ['/pinned/java', '-cp', '/pinned/oracle.jar', 'Main'],
                    'lock_sha256': tool.oracle.sha256(self.root / 'sources/oracle.lock.json'),
                    'version': lock['version'], 'source': lock['source'],
                    'artifact_sha256': lock['artifact']['sha256'],
                    'release': {'commitId': lock['artifact']['commit_id']},
                    'jdk': {'properties': lock['jdk']['properties']}}
        cache = Path('/workspace/.cache/oracle')
        metadata['profile'] = 'reference'
        metadata['artifact'] = str(cache / lock['artifact']['filename'])
        metadata['environment'] = {'PATH': tool.os.defpath, 'HOME': str(cache / 'home'), 'TMPDIR': str(cache / 'tmp'),
                                   'TMP': str(cache / 'tmp'), 'TEMP': str(cache / 'tmp'), 'LC_ALL': 'C', 'LANG': 'C',
                                   'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1', 'COLUMNS': '80'}
        metadata['command'] = [metadata['java'], *lock['jvm_options'], '-Duser.home=' + metadata['environment']['HOME'],
                               '-Djava.io.tmpdir=' + metadata['environment']['TMPDIR'], '-cp', metadata['artifact'], 'org.pkl.cli.Main']
        metadata['jdk'].update({key: '0' * 64 for key in ['executable_sha256', 'runtime_modules_sha256', 'release_sha256']})
        (self.root / 'oracle.json').write_text(json.dumps(metadata))
        classes = self.root / 'classes'; classes.mkdir()
        artifact = classes / 'M2ClassLifecycleProbe.class'; artifact.write_bytes(b'synthetic transport artifact')
        javac_dir = self.root / 'javac'; javac_dir.mkdir()
        javac = {'status': 'executed', 'exit_code': 0, 'cwd': '/workspace',
                 'command': ['/pinned/javac', '-cp', metadata['artifact'], '-d', str(classes), str(self.root / 'sources/tools/java/M2ClassLifecycleProbe.java')]}
        for name in ['stdout', 'stderr']:
            path = javac_dir / (name + '.bin'); path.write_bytes(b'')
            javac[name] = {'path': str(path), 'bytes': 0, 'sha256': tool.oracle.sha256(path)}
        (javac_dir / 'result.json').write_text(json.dumps(javac))
        manifest = tool.manifest_at(self.root / 'sources')
        report = {'schema': 1, 'acquisition_error': None, 'languages': ['java'], 'sources': sources,
                  'artifacts': [{'path': artifact.relative_to(self.root).as_posix(), 'bytes': artifact.stat().st_size, 'sha256': tool.oracle.sha256(artifact)}],
                  'source_stability_before_execution': True, 'source_stability_after_execution': True,
                  'repetitions': tool.REPETITIONS, 'oracle': metadata, 'compatibility_complete': False,
                  'comparison_channels': ['status', 'stdout', 'stderr'], 'workspace': '/workspace',
                  'output_directory': str(self.root), 'javac': javac, 'cases': [], 'unexecuted_groups': manifest['unexecuted_groups']}
        cp = metadata['command'].index('-cp')
        prefix = metadata['command'][:cp] + ['-cp', str(classes) + tool.os.pathsep + metadata['artifact'], 'M2ClassLifecycleProbe']
        for spec in manifest['sequences']:
            calls = (self.root / 'sources' / tool.FIXTURES / spec['expressions']).read_text().splitlines()
            for repetition in range(tool.REPETITIONS):
                relative = Path('sequences') / spec['id'] / str(repetition)
                directory = self.root / relative / 'java'
                result = self.process(directory, [('success', b'true', b'') for _ in calls])
                result['cwd'] = '/workspace'
                result['command'] = prefix + [str(self.root / 'sources' / tool.FIXTURES / spec['source']), str(self.root / 'sources' / tool.FIXTURES / spec['expressions']), str(directory)]
                for name in ['stdout', 'stderr']:
                    result[name]['path'] = str(directory / (name + '.bin'))
                (directory / 'result.json').write_text(json.dumps(result))
                row = {'id': spec['id'], 'group': spec['group'], 'repetition': repetition, 'directory': relative.as_posix(), 'java': result}
                row['observations'] = tool.observe_pair(self.root / relative, row, calls, ['java'])
                report['cases'].append(row)
        report['evidence_sha256'] = tool.inventory(self.root)
        (self.root / 'report.json').write_text(json.dumps(report))
        return report

    def test_report_integrity_rejects_omitted_sources_reordered_calls_and_forged_observations(self):
        report = self.report_fixture()
        self.assertEqual(tool.check_report(self.root), report)
        self.assertFalse(tool.contract_passes(report))
        mutations = []
        omitted = copy.deepcopy(report); omitted['sources'].pop(); mutations.append((omitted, 'source identities'))
        reordered = copy.deepcopy(report); reordered['cases'].reverse(); mutations.append((reordered, 'sequence executions'))
        forged = copy.deepcopy(report); forged['cases'][0]['observations']['processes']['java']['calls'][0]['status'] = 'error'; mutations.append((forged, 'observations'))
        for changed, message in mutations:
            (self.root / 'report.json').write_text(json.dumps(changed))
            with self.assertRaisesRegex(ValueError, message):
                tool.check_report(self.root)

    def test_oracle_memory_flags_and_environment_cannot_be_relabeled(self):
        report = self.report_fixture()
        report['oracle']['command'][1] = '-Xms128m'
        (self.root / 'oracle.json').write_text(json.dumps(report['oracle']))
        report['evidence_sha256'] = tool.inventory(self.root)
        (self.root / 'report.json').write_text(json.dumps(report))
        with self.assertRaisesRegex(ValueError, 'execution conditions'):
            tool.check_report(self.root)

    def test_complete_report_verifies_after_relocation(self):
        report = self.report_fixture()
        with tempfile.TemporaryDirectory() as temporary:
            relocated = Path(temporary) / 'artifact'
            shutil.copytree(self.root, relocated)
            self.assertEqual(tool.check_report(relocated), report)

    def test_report_inventory_rejects_raw_capture_tampering(self):
        self.report_fixture()
        (self.root / 'sequences/prefix/0/java/0.stdout.bin').write_bytes(b'true\n')
        with self.assertRaisesRegex(ValueError, 'inventory'):
            tool.check_report(self.root)

    def test_fixture_manifest_has_nonempty_supported_queries(self):
        manifest = tool.manifest_at(tool.ROOT)
        self.assertGreater(len(manifest['sequences']), 0)
        self.assertTrue(manifest['unexecuted_groups'])


if __name__ == '__main__':
    unittest.main()
