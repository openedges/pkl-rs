"""Boundaries for future 40-module N-API/Chromium Java byte replay."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import union_marker_portable_compare as compare


class UnionMarkerPortableTests(unittest.TestCase):
    def test_canonical_separate_module_java_authority(self):
        programs, java, provenance, verdict = compare.canonical(compare.FIXTURE)
        self.assertEqual(len(programs), 40)
        self.assertEqual(len(provenance['sessions']), 80)
        self.assertEqual(verdict['statuses'], {'success': 9, 'error': 31})
        self.assertEqual(java['programs/00-nonunion-int/repeat-0/000-nonunion-int/result.status.bin'], b'error\n')
        self.assertEqual(java['programs/11-union-first/repeat-0/000-union-first/result.stdout.bin'], b'1')
        self.assertEqual(compare.java_execution(provenance)['locale'], 'en-US')
        self.assertIn(b'\r\n', (compare.FIXTURE / programs[39]['module_file']).read_bytes())

    def test_js_and_python_transitive_capture_source_lists_match(self):
        script = ("import { CAPTURE_TRACKED_SOURCES } from './tools/platform/capture-union-marker-modules.mjs';"
                  "console.log(JSON.stringify(CAPTURE_TRACKED_SOURCES));")
        child = subprocess.run(['node', '--input-type=module', '-e', script], cwd=compare.ROOT,
                               capture_output=True, check=True)
        self.assertEqual(set(json.loads(child.stdout)), compare.CAPTURE_SOURCES)
        self.assertEqual(len(json.loads(child.stdout)), len(compare.CAPTURE_SOURCES))

    @staticmethod
    def requests(programs, alternative=False):
        base = ['/', '/favicon.ico', '/pkl_wasm.js', '/pkl_wasm_bg.wasm',
                '/session.mjs', '/module-policy-session.mjs', '/resource-session.mjs']
        if alternative:
            base = ['/', '/pkl_wasm.js', '/pkl_wasm_bg.wasm', '/session.mjs',
                    '/resource-session.mjs', '/module-policy-session.mjs', '/favicon.ico']
        statuses = {'/favicon.ico': 204}
        return [{'session': f"{index}:{row['id']}:{repeat}", 'path': path,
                 'status': statuses.get(path, 200)}
                for index, row in enumerate(programs) for repeat in (0, 1) for path in base]

    def test_http_two_valid_dependency_orders_each_keep_own_session(self):
        programs = compare.canonical(compare.FIXTURE)[0]
        compare.verify_http_requests(self.requests(programs), programs)
        compare.verify_http_requests(self.requests(programs, alternative=True), programs)

    def test_http_rejects_missing_duplicate_extra_status_order_and_cross_session(self):
        programs = compare.canonical(compare.FIXTURE)[0]
        for kind in ('missing', 'duplicate', 'extra', 'status', 'order', 'session'):
            rows = self.requests(programs)
            if kind == 'missing': rows.pop(3)
            elif kind == 'duplicate': rows[3] = dict(rows[2])
            elif kind == 'extra': rows.append(dict(rows[0]))
            elif kind == 'status': rows[2]['status'] = 404
            elif kind == 'order': rows[2], rows[3] = rows[3], rows[2]
            else: rows[7]['session'] = rows[0]['session']
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                compare.verify_http_requests(rows, programs)

    def test_seven_java_common_raw_channels_are_all_required(self):
        _, java, _, _ = compare.canonical(compare.FIXTURE)
        prefix = 'programs/00-nonunion-int/repeat-0/000-nonunion-int/'
        reference = {name: java[prefix + name] for name in compare.COMMON}
        with tempfile.TemporaryDirectory() as temp:
            folder = Path(temp)
            for name, raw in reference.items(): (folder / name).write_bytes(raw)
            for name in compare.EVENT_META:
                (folder / name).write_bytes(b'language-error' if name == 'candidate.disposition.bin' else b'')
            verdict = compare.shared_collection.compare_row(reference, folder)
            self.assertEqual(verdict, {'status': 'pass', 'channels': [], 'issues': []})
            (folder / 'result.stderr.code-units.bin').write_bytes(b'')
            verdict = compare.shared_collection.compare_row(reference, folder)
            self.assertEqual(verdict['status'], 'mismatch')
            self.assertEqual(verdict['channels'], ['result.stderr.code-units.bin'])

    def test_strict_gate_keeps_failed_full_api_and_real_string_mismatch(self):
        result = {'strict_string_passed': False}
        self.assertEqual(compare.gate_exit(result, 'integrity'), 0)
        self.assertEqual(compare.gate_exit(result, 'string'), 1)
        self.assertEqual(compare.gate_exit(result, 'full-api'), 1)
        result['strict_string_passed'] = True
        self.assertEqual(compare.gate_exit(result, 'string'), 0)
        self.assertEqual(compare.gate_exit(result, 'full-api'), 1)

if __name__ == '__main__': unittest.main()
