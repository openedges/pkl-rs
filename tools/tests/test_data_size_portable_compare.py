"""Failure detection for the independent portable DataSize admission gate."""
from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import data_size_portable_compare as compare


def browser_http_session():
    return [{'path': path, 'status': 204 if path == '/favicon.ico' else 200}
            for path in ('/', '/favicon.ico', '/pkl_wasm.js', '/pkl_wasm_bg.wasm',
                         '/session.mjs', '/module-policy-session.mjs',
                         '/resource-session.mjs')]


class PortableComparisonTests(unittest.TestCase):
    def setUp(self):
        temp = tempfile.TemporaryDirectory()
        self.addCleanup(temp.cleanup)
        self.root = Path(temp.name)
        self.java = self.root / 'java'
        self.candidate = self.root / 'candidate'
        self.java.mkdir()
        self.candidate.mkdir()
        self.row(self.java, False)
        self.row(self.candidate, True)

    def row(self, path, candidate, status=b'success\n', output=b'1.mb', error=b''):
        channels = {'expression.bin': b'1.mb',
                    'expression.code-units.bin': '1.mb'.encode('utf-16-be'),
                    'result.status.bin': status, 'result.stdout.bin': output,
                    'result.stdout.code-units.bin': output.decode().encode('utf-16-be'),
                    'result.stderr.bin': error,
                    'result.stderr.code-units.bin': error.decode().encode('utf-16-be')}
        channels.update({name: b'' for name in
                         (compare.EVENT_META if candidate else compare.JAVA_ONLY)})
        if candidate and status == b'error\n':
            channels['candidate.disposition.bin'] = b'language-error'
        for name, data in channels.items():
            (path / name).write_bytes(data)

    def verdict(self):
        return compare.classify(self.java, self.candidate)

    def test_exact_common_seven_channels_and_producer_exclusion(self):
        (self.java / 'result.error-class.bin').write_bytes(b'org.pkl.core.PklException')
        self.assertEqual(self.verdict()['status'], 'pass')
        (self.candidate / 'result.error-class.bin').write_bytes(b'fabricated')
        self.assertEqual(self.verdict()['status'], 'mismatch')

    def test_raw_output_status_query_and_utf16_drift_are_not_normalized(self):
        for name, content in [('result.stdout.bin', b'1.mb '),
                              ('result.status.bin', b'error\n'),
                              ('expression.bin', b'1.kb'),
                              ('result.stdout.code-units.bin', b'\x00?')]:
            with self.subTest(name=name):
                path = self.candidate / name
                original = path.read_bytes()
                path.write_bytes(content)
                self.assertEqual(self.verdict()['status'], 'mismatch')
                path.write_bytes(original)

    def test_candidate_error_cannot_be_labeled_ordinary(self):
        self.row(self.java, False, b'error\n', b'', b'error')
        self.row(self.candidate, True, b'error\n', b'', b'error')
        (self.candidate / 'candidate.disposition.bin').write_bytes(b'unsupported')
        self.assertEqual(self.verdict()['status'], 'mismatch')
        self.assertEqual(self.verdict()['channels'], [])
        (self.candidate / 'result.status.bin').write_bytes(b'candidate-error\n')
        self.assertEqual(self.verdict()['status'], 'mismatch')

    def test_missing_and_extra_payloads_fail(self):
        (self.candidate / 'result.stderr.code-units.bin').unlink()
        self.assertEqual(self.verdict()['status'], 'mismatch')
        (self.candidate / 'result.stderr.code-units.bin').write_bytes(b'')
        (self.candidate / 'candidate.expected.bin').write_bytes(b'1.mb')
        self.assertEqual(self.verdict()['status'], 'mismatch')

    def trace(self, status='success'):
        fixture = {'uri': 'file:///stage57/datasize-oracle.pkl', 'source': 'a=1',
                   'rows': [{'id': 'one', 'expression': '1.mb'}]}
        event = {'kind': 'value' if status == 'success' else 'error',
                 'disposition': None if status == 'success' else 'language-error'}
        event['value' if status == 'success' else 'error'] = {
            str(i): ord(char) for i, char in enumerate('1.mb')}
        trace = {'acquisitionError': None,
                 'boundary': 'String query status and complete UTF16/Java UTF8 output only',
                 'architecture': {'sessionCreated': True, 'closed': True, 'polls': 1,
                                  'collections': 2, 'terminalSlots': 0,
                                  'terminalDepth': 0, 'retainedPayloadsStable': True},
                 'trace': [
                     {'kind': 'constructor', 'method': 'moduleEvaluator', 'args': ['compact', False, False, False]},
                     {'kind': 'evaluate-module-source', 'uri': fixture['uri'], 'expression': '1.mb',
                      'index': 0, 'id': 'one'},
                     {'kind': 'api-call', 'method': 'evaluateModuleSource',
                      'args': [fixture['uri'], fixture['source'], '1.mb', 0, None]},
                     {'kind': 'terminal', 'index': 0, 'id': 'one', 'event': event},
                     {'kind': 'api-call', 'method': 'close', 'args': []},
                 ]}
        (self.root / 'candidate-trace.json').write_text(json.dumps(trace))
        (self.root / 'process.stdout.bin').write_bytes(b'session-open mode=string rows=1\n')
        row = self.root / '000-one'
        row.mkdir(exist_ok=True)
        (row / ('result.stdout.code-units.bin' if status == 'success' else
                'result.stderr.code-units.bin')).write_bytes('1.mb'.encode('utf-16-be'))
        return fixture, trace

    def test_trace_binds_ordered_public_calls_terminal_and_cleanup(self):
        fixture, trace = self.trace()
        compare.validate_trace(self.root, ['success'], fixture, 'chromium')
        for mutation in [lambda t: t['trace'][2]['args'].__setitem__(2, '2.mb'),
                         lambda t: t['trace'][3].update(id='wrong'),
                         lambda t: t['architecture'].update(terminalSlots=1),
                         lambda t: t['architecture'].update(retainedPayloadsStable=False),
                         lambda t: t['trace'].insert(2, {'kind': 'api-call', 'method': 'evaluateModuleSource',
                                                          'args': t['trace'][2]['args']})]:
            with self.subTest(mutation=mutation):
                broken = json.loads(json.dumps(trace))
                mutation(broken)
                (self.root / 'candidate-trace.json').write_text(json.dumps(broken))
                with self.assertRaises(ValueError):
                    compare.validate_trace(self.root, ['success'], fixture, 'chromium')
        (self.root / 'candidate-trace.json').write_text(json.dumps(trace))

    def test_api_exception_requires_separate_failure_trace(self):
        fixture, trace = self.trace()
        trace['trace'][3] = {'kind': 'api-error', 'index': 0, 'id': 'one',
                             'error': {'name': 'Error', 'message': 'failed'}}
        (self.root / 'candidate-trace.json').write_text(json.dumps(trace))
        (self.root / '000-one/result.stderr.code-units.bin').write_bytes('failed'.encode('utf-16-be'))
        compare.validate_trace(self.root, ['api-exception'], fixture, 'napi')
        with self.assertRaises(ValueError):
            compare.validate_trace(self.root, ['success'], fixture, 'napi')

    def test_host_request_trace_requires_token_and_primary_key_ownership(self):
        fixture, trace = self.trace()
        request = {'kind': 'module-request', 'event': {'token': 'token-1',
                   'module': {'id': '1', 'kind': 'select-key', 'selectionPhase': 'primary',
                              'originalUri': fixture['uri'], 'keyId': '42'}}}
        completion = {'kind': 'api-call', 'method': 'completeModuleKey',
                      'args': ['token-1', '42', fixture['uri'], 'host', 'fixture-key:42', 0]}
        trace['trace'][3:3] = [request, completion]
        (self.root / 'candidate-trace.json').write_text(json.dumps(trace))
        compare.validate_trace(self.root, ['success'], fixture, 'napi')
        for wrong in [dict(completion, args=['wrong', *completion['args'][1:]]),
                      dict(completion, args=[*completion['args'][:4], 'foreign-key', 0])]:
            with self.subTest(wrong=wrong):
                broken = json.loads(json.dumps(trace))
                broken['trace'][4] = wrong
                (self.root / 'candidate-trace.json').write_text(json.dumps(broken))
                with self.assertRaises(ValueError):
                    compare.validate_trace(self.root, ['success'], fixture, 'napi')

    def test_fixed_build_source_contract_includes_production_literals_and_tools(self):
        self.assertEqual(len(compare.EMBEDDED), 5)
        self.assertIn('crates/pkl-runtime/src/stdlib/base.pkl', compare.EMBEDDED)
        self.assertIn('tools/data_size_portable_compare.py', compare.SOURCE_EXTRAS)
        self.assertIn('tools/platform/data-size-session.mjs', compare.CAPTURE_SOURCES)
        self.assertTrue(compare.SOURCE_EXTRAS <= set(compare.source_names()))

    def test_repetition_requires_exact_integer_zero_and_one(self):
        good = {'napi': [{'repeat': 0}, {'repeat': 1}],
                'chromium': [{'repeat': 0}, {'repeat': 1}]}
        compare.verify_target_selection(good)
        for bad in [[{'repeat': False}, {'repeat': True}],
                    [{'repeat': 0}, {'repeat': 0}],
                    [{'repeat': 1}, {'repeat': 0}],
                    [{'repeat': 0}],
                    [{'repeat': 0}, {'repeat': 1}, {'repeat': 2}]]:
            with self.subTest(bad=bad), self.assertRaisesRegex(ValueError, 'repetition'):
                compare.verify_target_selection({**good, 'chromium': bad})

    def test_integrity_string_and_full_api_gates_remain_distinct(self):
        result = {'integrity': 'passed', 'strict_string_passed': True,
                  'full_public_api_comparison_passed': False}
        self.assertEqual(compare.gate_exit(result, 'integrity'), 0)
        self.assertEqual(compare.gate_exit(result, 'string'), 0)
        self.assertEqual(compare.gate_exit(result, 'full-api'), 1)
        result['strict_string_passed'] = False
        self.assertEqual(compare.gate_exit(result, 'integrity'), 0)
        self.assertEqual(compare.gate_exit(result, 'string'), 1)
        self.assertEqual(compare.gate_exit(result, 'full-api'), 1)

    def test_browser_launch_request_binds_the_declared_executable(self):
        browser = self.root / 'Chromium'
        browser.write_bytes(b'binary')
        row = {'browserExecutable': str(browser),
               'browserExecutableRealpath': str(browser.resolve()),
               'launchOptions': {'headless': True, 'executablePath': str(browser)}}
        compare.verify_browser_launch(row)
        for options in [{'headless': True},
                        {'headless': False, 'executablePath': str(browser)},
                        {'headless': True, 'executablePath': str(self.root / 'other')}]:
            with self.subTest(options=options), self.assertRaises(ValueError):
                compare.verify_browser_launch({**row, 'launchOptions': options})

    def test_browser_http_log_preserves_favicon_and_rejects_404_or_extra_request(self):
        one = browser_http_session()
        compare.verify_http_requests(one + one)
        for broken in [one + one[:-1],
                       one + one + [{'path': '/unexpected', 'status': 200}],
                       [*one[:1], {'path': '/favicon.ico', 'status': 404}, *one[2:], *one]]:
            with self.subTest(broken=broken), self.assertRaises(ValueError):
                compare.verify_http_requests(broken)

    def test_browser_http_asynchronous_completions_preserve_the_original_log(self):
        one = browser_http_session()
        # These are the two independently observed browser scheduling changes:
        # favicon after the first JS asset, and reversed static helper imports.
        first = [one[0], one[2], one[1], *one[3:]]
        second = [*one[:5], one[6], one[5]]
        requests = first + second
        original_bytes = json.dumps(requests).encode()
        compare.verify_http_requests(requests)
        self.assertEqual(json.dumps(requests).encode(), original_bytes)

    def test_browser_http_session_boundaries_and_load_dependencies_are_required(self):
        one = browser_http_session()
        for broken in (
                [one[1], one[0], *one[2:]],
                [*one[:2], one[3], one[2], *one[4:]],
                [*one[:3], one[4], one[3], *one[5:]],
                [*one[:4], one[5], one[4], one[6]],
                [*one[:4], one[6], one[5], one[4]]):
            with self.subTest(broken=broken), self.assertRaises(ValueError):
                compare.verify_http_requests(broken + one)
        crossed = [*one[:6], one[0], one[6], *one[1:]]
        with self.assertRaises(ValueError):
            compare.verify_http_requests(crossed)

    def test_browser_http_requires_exact_unique_resource_status_records(self):
        one = browser_http_session()
        for replacement in (
                one[2], {'path': '/unexpected', 'status': 204},
                {'path': '/favicon.ico', 'status': 200},
                {'path': '/favicon.ico', 'status': True},
                {'path': '/favicon.ico', 'status': '204'},
                {'path': '/favicon.ico', 'status': 204, 'extra': 1}):
            with self.subTest(replacement=replacement), self.assertRaises(ValueError):
                compare.verify_http_requests([one[0], replacement, *one[2:], *one])
        with self.assertRaises(ValueError):
            compare.verify_http_requests(tuple(one + one))

    def test_wrapper_rejects_os_channel_and_source_bookend_forgery(self):
        build_wrapper, wrapper = self.root / 'build-wrapper', self.root / 'capture-wrapper'
        build_wrapper.mkdir()
        wrapper.mkdir()
        m1 = self.root / 'm1'
        capture = self.root / 'stage57-data-size-portable-capture-02'
        old_capture = self.root / 'stage57-data-size-portable-capture-01'
        fixture = self.root / 'fixture'
        m1.mkdir()
        capture.mkdir()
        old_capture.mkdir()
        fixture.mkdir()
        (capture / 'report.json').write_bytes(b'{}\n')
        (old_capture / 'report.json').write_text(json.dumps({'complete': False}))
        for folder in (build_wrapper, wrapper):
            (folder / 'acquisition-source-before.json').write_text(json.dumps({'source': {'bytes': 1}}))
            (folder / 'acquisition-source-after.json').write_text(json.dumps({'source': {'bytes': 1}}))
        (build_wrapper / 'acquisition-summary.json').write_text(json.dumps({
            'm1_exit': 0, 'datasize_capture_exit': 1, 'source_stable': True,
            'capture_complete': False, 'capture_report': compare.identity(old_capture / 'report.json')}))
        (wrapper / 'acquisition-summary.json').write_text(json.dumps({
            'm1_exit': None, 'datasize_capture_exit': 0, 'source_stable': True,
            'capture_complete': True, 'capture_report': compare.identity(capture / 'report.json')}))
        for folder, name in [(build_wrapper, 'm1-contract'),
                             (build_wrapper, 'datasize-portable'),
                             (wrapper, 'datasize-portable')]:
            for channel in ['stdout', 'stderr']:
                (folder / f'{name}.{channel}.bin').write_bytes(b'')
        node = '/example/node'
        portable_argv = [node, str(compare.ROOT / 'tools/platform/capture-data-size.mjs'),
                         str(capture), str(m1 / 'wasm'), str(m1 / 'pkl_napi.node'), str(fixture)]
        records = [[], []]
        for index, (folder, name) in enumerate([(build_wrapper, 'm1-contract'),
                                                 (build_wrapper, 'datasize-portable'),
                                                 (wrapper, 'datasize-portable')]):
            row = {'name': name, 'argv':
                   ['python3', 'tools/m1_platforms.py', '--output', str(m1),
                    '--verify-contract'] if index == 0 else portable_argv,
                            'cwd': str(compare.ROOT), 'environment': {'PATH': '/fixed'},
                            'exit_code': 1 if index == 1 else 0, 'signal': None,
                            'started_epoch': 1 + index * 2,
                            'finished_epoch': 2 + index * 2,
                            **{channel + '_log': str(folder / f'{name}.{channel}.bin')
                               for channel in ['stdout', 'stderr']},
                            **{channel: compare.identity(folder / f'{name}.{channel}.bin')
                               for channel in ['stdout', 'stderr']}}
            records[0 if index < 2 else 1].append(row)
        (build_wrapper / 'acquisition-commands.json').write_text(json.dumps(records[0]))
        (wrapper / 'acquisition-commands.json').write_text(json.dumps(records[1]))
        reported = {'argv': portable_argv, 'node': {'executable': node},
                    'cwd': str(compare.ROOT), 'environment': {'PATH': '/fixed'}}
        compare.verify_wrapper(build_wrapper, wrapper, m1, capture, fixture, reported)
        records[1][0]['stdout']['sha256'] = '0' * 64
        (wrapper / 'acquisition-commands.json').write_text(json.dumps(records[1]))
        with self.assertRaisesRegex(ValueError, 'OS channel'):
            compare.verify_wrapper(build_wrapper, wrapper, m1, capture, fixture, reported)
        records[1][0]['stdout'] = compare.identity(wrapper / 'datasize-portable.stdout.bin')
        (wrapper / 'acquisition-commands.json').write_text(json.dumps(records[1]))
        (wrapper / 'datasize-portable.stderr.bin').write_bytes(b'warning')
        records[1][0]['stderr'] = compare.identity(wrapper / 'datasize-portable.stderr.bin')
        (wrapper / 'acquisition-commands.json').write_text(json.dumps(records[1]))
        with self.assertRaisesRegex(ValueError, 'unexpected output'):
            compare.verify_wrapper(build_wrapper, wrapper, m1, capture, fixture, reported)
        (wrapper / 'datasize-portable.stderr.bin').write_bytes(b'')
        records[1][0]['stderr'] = compare.identity(wrapper / 'datasize-portable.stderr.bin')
        (wrapper / 'acquisition-commands.json').write_text(json.dumps(records[1]))
        (wrapper / 'acquisition-source-after.json').write_text(json.dumps({'source': {'bytes': 2}}))
        with self.assertRaisesRegex(ValueError, 'sources changed'):
            compare.verify_wrapper(build_wrapper, wrapper, m1, capture, fixture, reported)

    def test_m1_build_binding_rejects_missing_literal_and_artifact_drift(self):
        m1 = self.root / 'm1'
        build_wrapper, capture_wrapper = self.root / 'build-wrapper', self.root / 'capture-wrapper'
        (m1 / 'sources').mkdir(parents=True)
        (m1 / 'wasm').mkdir()
        build_wrapper.mkdir()
        capture_wrapper.mkdir()
        names = compare.EMBEDDED | compare.CAPTURE_SOURCES
        hashes, bookend, fresh_bookend = {}, {}, {}
        for name in names:
            path = m1 / 'sources' / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes((compare.ROOT / name).read_bytes() if name in compare.CAPTURE_SOURCES
                             and name != compare.TRANSPORT else name.encode())
            hashes[name] = compare.sha(path.read_bytes())
            bookend[name] = compare.identity(path)
            fresh_bookend[name] = (compare.identity(compare.ROOT / name)
                                   if name == compare.TRANSPORT else bookend[name])
        (m1 / 'sources.json').write_text(json.dumps(hashes))
        (build_wrapper / 'acquisition-source-before.json').write_text(json.dumps(bookend))
        (capture_wrapper / 'acquisition-source-before.json').write_text(json.dumps(fresh_bookend))
        artifacts = {'napi': m1 / 'pkl_napi.node', 'wasm_js': m1 / 'wasm/pkl_wasm.js',
                     'wasm_bg': m1 / 'wasm/pkl_wasm_bg.wasm',
                     'native': m1 / 'semantics_probe'}
        for name, path in artifacts.items():
            path.write_bytes(name.encode())
        sealed = {name: compare.identity(path) for name, path in artifacts.items()}
        for folder in (build_wrapper, capture_wrapper):
            for name in ['before', 'after']:
                (folder / f'm1-artifacts-{name}-capture.json').write_text(json.dumps(sealed))
        report = {'source_stability': True, 'source_hashes': hashes,
                  'experiment_contract_status': 'pass', 'compatibility_status': 'fail',
                  'known_gaps': ['regex-unicode-15-vs-16', 'regex-zero-width-supplementary'],
                  'total': 10, 'shared_engine_passed': 10,
                  'oracle_passed': {'rust': 8, 'node': 8, 'browser': 8},
                  'artifacts': {path.relative_to(m1).as_posix(): sealed[name]['sha256']
                                for name, path in artifacts.items()}}
        (m1 / 'report.json').write_text(json.dumps(report))
        (capture_wrapper / 'm1-transport-delta.json').write_text(json.dumps({
            'm1_report': str(m1 / 'report.json'),
            'm1_report_identity': compare.identity(m1 / 'report.json'),
            'unchanged_m1_source_count': len(hashes) - 1,
            'changed_transport_only': {compare.TRANSPORT: {
                'm1_sha256': hashes[compare.TRANSPORT],
                'capture02_sha256': compare.CAPTURE02_TRANSPORT_SHA256}}}))
        commands = [
            {'name': 'napi-build', 'command': ['cargo', 'build', '-p', 'pkl-napi',
                                               '--release', '--locked'], 'exit_code': 0},
            {'name': 'wasm-build', 'command': ['cargo', 'build', '-p', 'pkl-wasm',
                                               '--target', 'wasm32-unknown-unknown',
                                               '--release', '--locked'], 'exit_code': 0},
            {'name': 'wasm-bindgen', 'command': ['/example/wasm-bindgen', '--target',
                                                'web', '--out-dir', str(m1 / 'wasm'),
                                                '/example/pkl_wasm.wasm'], 'exit_code': 0},
        ]
        (m1 / 'commands.json').write_text(json.dumps(commands))
        for row in commands:
            for channel in ['stdout', 'stderr']:
                (m1 / f'{row["name"]}.{channel}.bin').write_bytes(b'')
        capture = {'inputsBefore': {str(path): sealed[name]['sha256']
                                    for name, path in artifacts.items() if name != 'native'}}
        capture['inputsBefore'][str(compare.ROOT / compare.TRANSPORT)] = (
            compare.CAPTURE02_TRANSPORT_SHA256)
        paths = [m1 / 'sources' / name for name in names]
        with patch.object(compare.m1_platforms, 'source_paths', return_value=paths):
            compare.verify_m1(m1, capture, build_wrapper, capture_wrapper)
            (m1 / 'sources/crates/pkl-runtime/src/stdlib/base.pkl').unlink()
            with self.assertRaises(ValueError):
                compare.verify_m1(m1, capture, build_wrapper, capture_wrapper)
            path = m1 / 'sources/crates/pkl-runtime/src/stdlib/base.pkl'
            path.write_bytes(path.relative_to(m1 / 'sources').as_posix().encode())
            artifacts['wasm_bg'].write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'artifact'):
                compare.verify_m1(m1, capture, build_wrapper, capture_wrapper)


if __name__ == '__main__':
    unittest.main()
