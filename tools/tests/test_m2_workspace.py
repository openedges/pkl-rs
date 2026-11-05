"""Explicit workspace boundaries and complete retained-inventory integrity."""
import copy
import os
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import compat
import m2_workspace as workspace


class WorkspaceProfileTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.destination = self.root / 'target/workspace'
        self.source_files = {'external/pkl/input.pkl': b'value = 42\n',
                             'external/pkl/fixture.bin': b'\x00raw\xff',
                             'external/pkl-pantry/other.pkl': b'value = 0\n'}
        for relative, data in self.source_files.items():
            path = self.root / relative
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        fixtures = self.root / 'tests/fixtures/unrelated.pkl'
        fixtures.parent.mkdir(parents=True)
        fixtures.write_bytes(b'unrelated = true\n')
        self.corpus = dict(sources=[], cases=[])
        for repository, root in workspace.SOURCE_ROOTS.items():
            resources = [dict(path=relative, kind='file', sha256=compat.digest(data), bytes=len(data))
                         for relative, data in self.source_files.items() if relative.startswith(root + '/')]
            self.corpus['sources'].append(dict(repository=repository, path=root, revision='pinned-' + repository,
                resources=resources, resource_set_sha256=compat.digest(compat.encoded(resources))))
        original = dict(id='original:first', suite='language-snippet',
                        source=dict(repository='pkl', path='input.pkl',
                                    sha256=compat.digest(self.source_files['external/pkl/input.pkl'])),
                        execution=dict(status='ready', reason=None), cwd='.',
                        resources=[dict(path='external/pkl/fixture.bin',
                                        sha256=compat.digest(self.source_files['external/pkl/fixture.bin']))])
        self.corpus['cases'] = [original]
        self.cases = [dict(id=original['id'], adapter='language-module-eval', upstream_case=original,
                           input='external/pkl/input.pkl', input_sha256=original['source']['sha256'],
                           execution=dict(state='ready', reason=None))]

    def record(self, profile=workspace.ORIGINAL_PROFILE):
        return workspace.profile_record(profile, self.corpus, self.cases)

    def tracked(self, repository):
        return [relative.removeprefix(repository.relative_to(self.root).as_posix() + '/')
                for relative in self.source_files
                if relative.startswith(repository.relative_to(self.root).as_posix() + '/')]

    def prepare(self, record):
        with patch.object(compat, 'tracked_paths', side_effect=self.tracked):
            workspace.prepare(self.root, self.destination, record)

    def test_explicit_profile_retains_both_complete_source_trees_and_excludes_project_fixtures(self):
        record = self.record()
        self.prepare(record)
        self.assertFalse((self.destination / 'tests').exists())
        self.assertEqual(compat.read_json(self.destination / compat.MARKER),
                         workspace.workspace_identity(self.root, self.destination, record))
        self.assertEqual({row['path'] for row in record['retained_sources']}, set(workspace.SOURCE_ROOTS.values()))
        workspace.validate_baseline(compat.snapshot(self.destination), self.corpus, record)
        for relative, data in self.source_files.items():
            self.assertEqual((self.destination / relative).read_bytes(), data)

    def test_filesystem_record_does_not_promote_a_blocked_case_to_module_readiness(self):
        expected = self.record()
        self.corpus['cases'][0]['execution']['status'] = 'blocked'
        self.assertEqual(workspace.original_source_profile(self.corpus), expected)
        with self.assertRaisesRegex(ValueError, 'unchanged original-ready'):
            workspace.profile_record(workspace.ORIGINAL_PROFILE, self.corpus, self.cases)

    def test_default_compat_staging_and_legacy_helper_preserve_fixture_copy_and_marker(self):
        with patch.object(compat, 'tracked_paths', side_effect=self.tracked):
            compat.prepare_workspace(self.root, self.destination)
        before = compat.snapshot(self.destination)
        marker = compat.read_json(self.destination / compat.MARKER)
        self.assertNotIn('workspace_profile', marker)
        self.assertIn('tests/fixtures/unrelated.pkl', before)
        self.prepare(self.record(workspace.DEFAULT_PROFILE))
        self.assertEqual(compat.snapshot(self.destination), before)
        self.assertEqual(compat.read_json(self.destination / compat.MARKER), marker)
        workspace.validate_baseline(before, self.corpus, self.record(workspace.DEFAULT_PROFILE))

    def test_profile_reuse_mismatch_is_rejected_before_any_workspace_mutation(self):
        self.prepare(self.record())
        marker = (self.destination / compat.MARKER).read_bytes()
        sentinel = self.destination / 'sentinel.bin'
        sentinel.write_bytes(b'keep me')
        for record in (self.record(workspace.DEFAULT_PROFILE),
                       dict(self.record(), retained_sources=[])):
            with self.assertRaises(ValueError):
                self.prepare(record)
            self.assertEqual(sentinel.read_bytes(), b'keep me')
            self.assertEqual((self.destination / compat.MARKER).read_bytes(), marker)

    def test_missing_sources_resources_hashes_and_outside_roots_cannot_claim_closure(self):
        for mutation in ('source-root', 'inventory-digest', 'missing-input', 'missing-resource',
                         'resource-hash', 'outside-root', 'cwd', 'blocked', 'non-original'):
            corpus, cases = copy.deepcopy((self.corpus, self.cases))
            original = corpus['cases'][0]
            cases[0]['upstream_case'] = original
            if mutation == 'source-root': corpus['sources'].pop()
            elif mutation == 'inventory-digest': corpus['sources'][0]['resource_set_sha256'] = '0' * 64
            elif mutation.startswith('missing-'):
                source = corpus['sources'][0]
                name = 'input.pkl' if mutation == 'missing-input' else 'fixture.bin'
                source['resources'] = [row for row in source['resources'] if not row['path'].endswith('/' + name)]
                source['resource_set_sha256'] = compat.digest(compat.encoded(source['resources']))
            elif mutation == 'resource-hash': original['resources'][0]['sha256'] = '0' * 64
            elif mutation == 'outside-root': original['resources'][0]['path'] = 'tests/fixtures/unrelated.pkl'
            elif mutation == 'cwd': original['cwd'] = 'tests/fixtures'
            elif mutation == 'blocked': original['execution']['status'] = 'blocked'
            else: cases[0]['adapter'] = 'language-api-string'
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                workspace.profile_record(workspace.ORIGINAL_PROFILE, corpus, cases)
        for profile in ('unsupported', None, True):
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                workspace.profile_record(profile, self.corpus, self.cases)

    def test_exact_baseline_rejects_missing_changed_added_paths_and_parent_kind_changes(self):
        record = self.record()
        self.prepare(record)
        baseline = compat.snapshot(self.destination)
        for mutation in ('missing-file', 'missing-directory', 'file-hash', 'symlink-kind', 'parent-symlink',
                         'extra-fixture', 'extra-empty-directory', 'record-digest'):
            changed = copy.deepcopy(baseline)
            modified_record = copy.deepcopy(record)
            if mutation == 'missing-file': changed.pop('external/pkl/fixture.bin')
            elif mutation == 'missing-directory': changed.pop('external/pkl')
            elif mutation == 'file-hash': changed['external/pkl/input.pkl']['sha256'] = '0' * 64
            elif mutation == 'symlink-kind': changed['external/pkl/input.pkl']['kind'] = 'symlink'
            elif mutation == 'parent-symlink': changed['external/pkl']['kind'] = 'symlink'
            elif mutation == 'extra-fixture': changed['tests/fixtures/hidden.pkl'] = dict(kind='file', sha256='0' * 64)
            elif mutation == 'extra-empty-directory': changed['unrelated'] = dict(kind='directory', sha256=None)
            else: modified_record['retained_sources'][0]['resource_set_sha256'] = '0' * 64
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                workspace.validate_baseline(changed, self.corpus, modified_record)

    def test_pinned_symlink_text_and_kind_are_part_of_exact_baseline(self):
        relative = 'external/pkl/linked.pkl'
        link = self.root / relative
        link.symlink_to('input.pkl')
        source = self.corpus['sources'][0]
        source['resources'].append(dict(path=relative, kind='symlink', bytes=len('input.pkl'),
                                        sha256=compat.digest(os.readlink(link).encode())))
        source['resource_set_sha256'] = compat.digest(compat.encoded(source['resources']))
        self.source_files[relative] = b'input.pkl'
        record = self.record()
        self.prepare(record)
        self.assertTrue((self.destination / relative).is_symlink())
        workspace.validate_baseline(compat.snapshot(self.destination), self.corpus, record)
        changed = compat.snapshot(self.destination)
        changed[relative]['sha256'] = compat.digest(b'fixture.bin')
        with self.assertRaises(ValueError):
            workspace.validate_baseline(changed, self.corpus, record)


if __name__ == '__main__':
    unittest.main()
