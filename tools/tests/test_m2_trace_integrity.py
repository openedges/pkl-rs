"""Failure injection for offline trace evidence verification, without runtime replay."""
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import m2_trace


class TraceEvidenceIntegrityTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)
        self.case = dict(id='one', source='tests/fixtures/m2-trace/one.pkl',
                         expressions=['result'], mode='COMPACT', action='capture',
                         logger_callback='', reader_callback='',
                         origin=dict(input_sha256=self.digest(b'result=1\n')))
        manifest = dict(schema=1, uri='file:///stage37-trace.pkl', repetitions=2,
                        cases=[self.case])
        self.write('sources/compatibility/m2-trace.json', self.encoded(manifest))
        self.write('sources/crates/pkl-runtime/src/lib.rs', b'// source snapshot\n')
        self.write('artifacts/trace_host_probe', b'captured native artifact')
        oracle_identity = dict(version='0.32.0', artifact_sha256=self.digest(b'pinned-java-oracle'))
        self.write('oracle.json', self.encoded(oracle_identity))
        command = self.process('processes/javac', b'compiler output\n')
        commands = [dict(name='javac', environment_profile='pinned-oracle', **command)]
        self.write('commands.json', self.encoded(commands))
        rows = []
        for repeat in range(2):
            base = f'cases/one/{repeat}'
            processes = {}
            for target in ['java', 'rust']:
                directory = f'{base}/{target}'
                self.write(directory + '/input.pkl', b'result=1\n')
                self.write(directory + '/expressions.txt', b'result\n')
                self.write(directory + '/logger-callback.txt', b'')
                self.write(directory + '/reader-callback.txt', b'')
                protocol = b'result=outer-0 status=success\nlogs=0 reads=0\n'
                processes[target] = self.process(directory + '/process', protocol)
                for name, data in [('outer-0.status.bin', b'success\n'),
                                   ('outer-0.stdout.bin', b'1'),
                                   ('outer-0.stderr.bin', b''),
                                   ('protocol.bin', protocol), ('process-stderr.bin', b'')]:
                    self.write(directory + '/' + name, data)
            path = self.output / base
            rows.append(dict(id='one', repeat=repeat, status='pass', processes=processes,
                             missing_calls=dict(java=[], rust=[]),
                             **m2_trace.compare_payloads(path / 'java', path / 'rust')))
        sources = {str(path.relative_to(self.output / 'sources')): m2_trace.sha(path)
                   for path in (self.output / 'sources').rglob('*') if path.is_file()}
        artifact = m2_trace.sha(self.output / 'artifacts/trace_host_probe')
        self.write('partial-results.json', self.encoded(rows))
        self.report = dict(schema=1, oracle=oracle_identity, manifest_path='compatibility/m2-trace.json',
                           manifest_sha256=sources['compatibility/m2-trace.json'],
                           cases=[self.case], repetitions=2, sources=sources,
                           sources_after=copy.deepcopy(sources), source_stability=True,
                           candidate=dict(sha256=artifact, source_binding='supplied-artifact-only'),
                           artifact_after_sha256=artifact, artifact_stability=True,
                           commands=commands, results=rows, status_counts={'pass': 2},
                           complete_selection=True, all_selected_match=True,
                           compatibility_complete=False, excluded_before_traversal=['build'])
        self.save(refresh_inventory=True)

    @staticmethod
    def digest(data):
        import hashlib
        return hashlib.sha256(data).hexdigest()

    @staticmethod
    def encoded(value):
        return (json.dumps(value, indent=2) + '\n').encode()

    def write(self, name, data):
        path = self.output / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
        return path

    def process(self, directory, stdout):
        result = dict(command=['synthetic-harness-process'], cwd='/synthetic',
                      status='executed', exit_code=0)
        for channel, data in [('stdout', stdout), ('stderr', b'')]:
            path = self.write(directory + '/' + channel + '.bin', data)
            result[channel] = dict(path=str(path), sha256=self.digest(data), bytes=len(data))
        self.write(directory + '/result.json', self.encoded(result))
        return result

    def save(self, refresh_inventory=False):
        if refresh_inventory:
            self.report['files'] = m2_trace.inventory(self.output)
        self.write('report.json', self.encoded(self.report))

    def test_valid_supplied_artifact_never_becomes_recorded_build_completion(self):
        report = m2_trace.verify_report(self.output)
        self.assertTrue(report['all_selected_match'])
        self.assertFalse(report['compatibility_complete'])
        self.report['compatibility_complete'] = True
        self.save()
        with self.assertRaisesRegex(ValueError, 'completion claim'):
            m2_trace.verify_report(self.output)

    def test_derived_counts_and_stability_flags_cannot_override_captured_hashes(self):
        self.report['status_counts'] = {'pass': 3}
        self.save()
        with self.assertRaisesRegex(ValueError, 'counts changed'):
            m2_trace.verify_report(self.output)
        self.report['status_counts'] = {'pass': 2}
        self.report['source_stability'] = False
        self.save()
        with self.assertRaisesRegex(ValueError, 'completion claim'):
            m2_trace.verify_report(self.output)

    def test_raw_mismatch_is_valid_evidence_but_cannot_claim_all_selected_match(self):
        self.write('cases/one/0/rust/outer-0.stdout.bin', b'2')
        base = self.output / 'cases/one/0'
        self.report['results'][0].update(m2_trace.compare_payloads(base / 'java', base / 'rust'))
        self.report['results'][0]['status'] = 'mismatch'
        self.report['status_counts'] = {'pass': 1, 'mismatch': 1}
        self.write('partial-results.json', self.encoded(self.report['results']))
        self.save(refresh_inventory=True)
        with self.assertRaisesRegex(ValueError, 'completion claim'):
            m2_trace.verify_report(self.output)
        self.report['all_selected_match'] = False
        self.save()
        verified = m2_trace.verify_report(self.output)
        self.assertEqual(verified['status_counts'], {'pass': 1, 'mismatch': 1})
        self.assertFalse(verified['compatibility_complete'])

    def test_dropped_execution_is_rejected_even_when_counts_are_rewritten(self):
        self.report['results'].pop()
        self.report['status_counts'] = {'pass': 1}
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_missing_java_process_metadata_cannot_be_vacuously_executed(self):
        self.report['results'][0]['processes'].pop('java')
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_empty_process_metadata_cannot_be_vacuously_executed(self):
        self.report['results'][0]['processes'] = {}
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_dropped_compiler_metadata_is_rejected(self):
        self.report['commands'] = []
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_changed_raw_payload_is_rejected_after_refreshing_file_inventory(self):
        self.write('cases/one/0/rust/outer-0.stdout.bin', b'1\n')
        self.save(refresh_inventory=True)
        with self.assertRaisesRegex(ValueError, 'raw trace comparison'):
            m2_trace.verify_report(self.output)

    def test_changed_source_snapshot_is_rejected_after_refreshing_file_inventory(self):
        self.write('sources/crates/pkl-runtime/src/lib.rs', b'// changed snapshot\n')
        self.save(refresh_inventory=True)
        with self.assertRaisesRegex(ValueError, 'source snapshot'):
            m2_trace.verify_report(self.output)

    def test_changed_actual_input_is_rejected_after_refreshing_file_inventory(self):
        self.write('cases/one/0/rust/input.pkl', b'result=2\n')
        self.save(refresh_inventory=True)
        with self.assertRaisesRegex(ValueError, 'source input'):
            m2_trace.verify_report(self.output)

    def test_changed_expression_input_is_rejected_after_refreshing_inventory(self):
        self.write('cases/one/0/rust/expressions.txt', b'other\n')
        self.save(refresh_inventory=True)
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_changed_callback_inputs_are_rejected_after_refreshing_inventory(self):
        for name in ['logger', 'reader']:
            with self.subTest(callback=name):
                path = f'cases/one/0/rust/{name}-callback.txt'
                self.write(path, b'other')
                self.save(refresh_inventory=True)
                with self.assertRaises(ValueError):
                    m2_trace.verify_report(self.output)
                self.write(path, b'')
                self.save(refresh_inventory=True)

    def test_changed_candidate_is_rejected_after_refreshing_file_inventory(self):
        self.write('artifacts/trace_host_probe', b'changed native artifact')
        self.save(refresh_inventory=True)
        with self.assertRaisesRegex(ValueError, 'native artifact'):
            m2_trace.verify_report(self.output)

    def test_dropped_source_identity_is_rejected(self):
        name = 'crates/pkl-runtime/src/lib.rs'
        self.report['sources'].pop(name)
        self.report['sources_after'].pop(name)
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_altered_oracle_identity_is_rejected(self):
        self.report['oracle']['version'] = '0.33.0'
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_removed_selection_and_execution_cannot_hide_captured_calls(self):
        self.report['cases'] = []
        self.report['results'] = []
        self.report['status_counts'] = {}
        self.report['complete_selection'] = False
        self.report['all_selected_match'] = False
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_claimed_build_binding_requires_captured_build_command(self):
        self.report['candidate']['source_binding'] = 'recorded-build'
        self.report['compatibility_complete'] = True
        self.save()
        with self.assertRaises(ValueError):
            m2_trace.verify_report(self.output)

    def test_build_cache_is_pruned_before_any_walk_or_hash(self):
        self.write('build/deep/cargo-cache', b'excluded')
        self.write('kept/build/ordinary-evidence', b'included')
        original_walk = os.walk
        visited, hashed = [], []
        original_sha = m2_trace.sha

        def guarded_walk(top, *args, **kwargs):
            for folder, directories, names in original_walk(top, *args, **kwargs):
                folder = Path(folder)
                visited.append(folder)
                self.assertFalse(folder.is_relative_to(self.output / 'build'),
                                 'Cargo build directory was traversed before pruning')
                yield str(folder), directories, names

        def guarded_sha(path):
            hashed.append(path)
            self.assertFalse(path.is_relative_to(self.output / 'build'))
            return original_sha(path)

        with patch.object(m2_trace.os, 'walk', guarded_walk), patch.object(m2_trace, 'sha', guarded_sha):
            files = m2_trace.inventory(self.output)
        self.assertTrue(visited)
        self.assertTrue(hashed)
        self.assertNotIn('build/deep/cargo-cache', files)
        self.assertIn('kept/build/ordinary-evidence', files)


if __name__ == '__main__':
    unittest.main()
