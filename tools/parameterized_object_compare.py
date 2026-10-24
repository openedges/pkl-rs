"""Capture exact Java-backed expressions on a frozen native and/or portable candidate."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

import compat
import m2_source_inputs
import parameterized_object_fixtures as fixtures

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def put(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2) + '\n')


def trace_fields(data):
    if not data.endswith(b'\n'):
        raise ValueError('native trace is empty or truncated')
    fields = {}
    for line in data.decode('utf-8').splitlines():
        if '\t' not in line:
            raise ValueError('invalid native trace row')
        key, value = line.split('\t', 1)
        fields.setdefault(key, []).append(value)
    return fields


def compare_native(row, fixture_root, exit_code, stdout, stderr, trace, timed_out=False):
    expected = row['observations'][0]
    checks = dict(exit_code=exit_code == expected['exit_code'],
                  stdout=stdout == (fixture_root / expected['channels']['stdout']['path']).read_bytes(),
                  stderr=stderr == (fixture_root / expected['channels']['stderr']['path']).read_bytes())
    failure = None
    try:
        fields = trace_fields(trace)

        def one(key):
            values = fields.get(key, [])
            if len(values) != 1:
                raise ValueError('missing/duplicate native trace field: ' + key)
            return values[0]

        checks['profile'] = one('profile') == 'arbitrary-expression-noop-logger-power-off-v1'
        identities = {'source_utf8_hex': (fixture_root / row['source']).read_bytes(),
                      'source_uri_utf8_hex': row['source_uri'].encode(),
                      'module_name_utf8_hex': row['module_name'].encode(),
                      'expression_utf8_hex': row['expression'].encode()}
        checks['input_identity'] = all(bytes.fromhex(one(key)) == value for key, value in identities.items())
        checks['disposition'] = exit_code == 0 and one('disposition') == '' or exit_code == 1 and one('disposition') == 'language-error'
        checks['guest_exit_code'] = one('guest_exit_code') == str(exit_code)
        constructed = one('session_constructed')
        if constructed == 'false':
            checks['constructor_shape'] = exit_code == 1 and one('disposition') == 'language-error' and stdout == b''
            checks['terminal_slots'] = 'terminal_slots' not in fields
            checks['terminal_depth'] = 'terminal_depth' not in fields
            unit_field = 'constructor_error_utf16be_hex'
        elif constructed == 'true':
            checks['terminal_slots'] = one('terminal_slots') == '0'
            checks['terminal_depth'] = one('terminal_depth') == '0'
            unit_field = 'value_utf16be_hex' if exit_code == 0 else 'error_utf16be_hex'
        else:
            raise ValueError('invalid native session construction status')
        units = bytes.fromhex(one(unit_field))
        if len(units) % 2:
            raise ValueError('odd native UTF16 channel')
        checks['owned_raw_channel'] = units.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace') == (stdout if exit_code == 0 else stderr)
    except (ValueError, KeyError) as error:
        failure = str(error)
        checks['trace_admission'] = False
    status = ('candidate-timeout' if timed_out else 'acquisition-failed' if failure or exit_code not in (0, 1)
              else 'non-language-failure' if not checks.get('disposition')
              else 'pass' if all(checks.values()) else 'mismatch')
    return dict(status=status, checks=checks, trace_error=failure)


def native_command(binary, row, fixture_root):
    if row['power_assertions'] is not False:
        raise ValueError('native fixture power assertions must be false')
    command = [str(binary), str(fixture_root / row['source']), row['source_uri'], row['module_name'], row['expression']]
    for resource in row['resources']:
        command.extend([resource['source_uri'], str(fixture_root / resource['source'])])
    return command


def source_paths():
    paths = set(m2_source_inputs.source_paths(ROOT))
    paths.update(['tools/parameterized_object_compare.py', 'tools/parameterized_object_fixtures.py',
                  'tools/platform/compare-parameterized-objects.mjs',
                  'tools/platform/parameterized-object-session.mjs', 'tools/platform/module-policy-session.mjs',
                  'tools/platform/package.json', 'tools/platform/package-lock.json'])
    return sorted(paths)


def portable_process_passed(report):
    process = report.get('portable_process', {})
    return (report.get('portable_report', {}).get('status') == 'passed'
            and process.get('exit_code') == 0 and process.get('timed_out') is False)


def execute(args):
    fixture = args.fixture.resolve()
    verification = fixtures.verify(fixture)
    cases = json.loads((fixture / 'cases.json').read_bytes())
    boundaries = json.loads((fixture / 'oracle-boundaries.json').read_bytes())
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    environment = {key: value for key, value in os.environ.items()
                   if key in ('PATH', 'HOME', 'TMPDIR', 'TMP', 'TEMP', 'LANG', 'LC_ALL', 'TZ', 'TERM', 'PLAYWRIGHT_BROWSERS_PATH')}
    environment['PKL_FIXTURE_PYTHON'] = sys.executable
    environment['PYTHONDONTWRITEBYTECODE'] = '1'
    source_names = source_paths()
    fixture_names = sorted(str(p) for p in fixture.rglob('*') if p.is_file())
    inputs = {str(ROOT / name): sha(ROOT / name) for name in source_names}
    inputs.update({name: sha(Path(name)) for name in fixture_names})
    report = dict(schema=1, scope='All selected exact expression/raw-channel observations on requested frozen candidates. Host/lifecycle traces remain candidate evidence, separate from unobserved Java callback traces.',
                  argv=sys.argv, cwd=str(ROOT), environment=environment, host=platform.platform(),
                  python=dict(executable=sys.executable, version=sys.version, sha256=sha(Path(sys.executable))),
                  fixture_verification=verification, fixture_manifest_sha256=sha(fixture / 'manifest.json'),
                  case_ids=[row['id'] for row in cases], repetitions=2, sources_before=inputs,
                  oracle_boundaries=[dict(id=row['id'], status='oracle-error', candidate_execution='not-requested') for row in boundaries],
                  targets={}, infrastructure_errors=[])
    put(output / 'report.json', report)
    if args.native:
        original_binary = args.native.resolve()
        native = output / 'artifacts/expression_fixture_probe'
        native.parent.mkdir(exist_ok=True)
        original_hash = sha(original_binary)
        shutil.copy2(original_binary, native)
        copied_hash = sha(native)
        if not original_hash == copied_hash == sha(original_binary):
            raise ValueError('native executable changed during frozen-copy admission')
        inputs[str(original_binary)] = original_hash
        inputs[str(native)] = copied_hash
        report['native_artifact'] = dict(original_path=str(original_binary), frozen_path=str(native), sha256=sha(native))
        results = []
        for row in cases:
            for repeat in range(2):
                directory = output / 'native' / row['id'] / ('repeat-' + str(repeat))
                directory.mkdir(parents=True)
                trace_path = directory / 'trace.tsv'
                command = native_command(native, row, fixture)
                process_environment = {**environment, 'PKL_FIXTURE_TRACE': str(trace_path)}
                timed_out = False
                try:
                    captured = subprocess.run(command, cwd=ROOT, env=process_environment, capture_output=True, timeout=args.timeout)
                    exit_code, stdout, stderr = captured.returncode, captured.stdout, captured.stderr
                except subprocess.TimeoutExpired as error:
                    exit_code, stdout, stderr, timed_out = None, error.stdout or b'', error.stderr or b'', True
                except OSError as error:
                    exit_code, stdout, stderr = None, b'', b''
                    put(directory / 'acquisition-error.json', dict(type=type(error).__name__, message=str(error)))
                trace = trace_path.read_bytes() if trace_path.exists() else b''
                for channel, data in [('stdout', stdout), ('stderr', stderr)]:
                    (directory / (channel + '.bin')).write_bytes(data)
                comparison = compare_native(row, fixture, exit_code, stdout, stderr, trace, timed_out)
                result = dict(id=row['id'], repeat=repeat, command=command, cwd=str(ROOT), environment=process_environment,
                              exit_code=exit_code, timed_out=timed_out, **comparison,
                              stdout_sha256=hashlib.sha256(stdout).hexdigest(), stderr_sha256=hashlib.sha256(stderr).hexdigest(),
                              trace_sha256=hashlib.sha256(trace).hexdigest(), directory=str(directory.relative_to(output)))
                put(directory / 'process.json', result)
                results.append(result)
        report['targets']['native'] = dict(complete=len(results) == 2 * len(cases), executions=len(results),
                                          statuses=dict(Counter(row['status'] for row in results)), results=results)
    else:
        report['targets']['native'] = dict(complete=False, status='not-requested', expected_executions=2 * len(cases))
    if args.wasm:
        node = shutil.which(args.node)
        if node is None:
            report['infrastructure_errors'].append(dict(target='portable', reason='Node executable unavailable'))
        else:
            command = [node, str(ROOT / 'tools/platform/compare-parameterized-objects.mjs'),
                       str(output / 'portable'), str(args.wasm.resolve()), str(args.napi.resolve()), str(fixture)]
            private = output / 'portable-private-files'
            private.mkdir()
            try:
                captured = compat.capture(command, ROOT, environment, args.portable_timeout, private)
            except compat.CaptureTimeout as error:
                captured = error.result
            for channel in ('stdout', 'stderr'):
                (output / ('portable-process.' + channel + '.bin')).write_bytes(captured[channel])
            report['portable_process'] = dict(command=command, exit_code=captured['exit_code'], timed_out=captured['timed_out'],
                                               timeout_seconds=args.portable_timeout,
                                               executable_sha256=sha(Path(node)))
            path = output / 'portable/report.json'
            if path.exists():
                portable = json.loads(path.read_bytes())
                report['portable_report'] = dict(path=str(path.relative_to(output)), sha256=sha(path), status=portable['status'])
                report['targets'].update(portable['targets'])
            else:
                report['infrastructure_errors'].append(dict(target='portable', reason='No complete portable report; original process channels retained'))
    else:
        for target in ('napi', 'chromium'):
            report['targets'][target] = dict(complete=False, status='not-requested', expected_executions=2 * len(cases))
    report['changed_inputs'] = []
    if source_paths() != source_names:
        report['changed_inputs'].append('source-catalog-file-set')
    if sorted(str(p) for p in fixture.rglob('*') if p.is_file()) != fixture_names:
        report['changed_inputs'].append('fixture-file-set')
    for path, before in inputs.items():
        try:
            if sha(Path(path)) != before:
                report['changed_inputs'].append(path)
        except OSError:
            report['changed_inputs'].append(path)
    requested = (['native'] if args.native else []) + (['napi', 'chromium'] if args.wasm else [])
    def passed(target):
        row = report['targets'].get(target, {})
        return row.get('complete') and row.get('executions') == 2 * len(cases) and (
            row.get('statuses') == {'pass': 2 * len(cases)} if target == 'native' else row.get('passingExecutions') == 2 * len(cases))
    report['status'] = 'passed' if not report['changed_inputs'] and not report['infrastructure_errors'] and all(passed(target) for target in requested) and (not args.wasm or portable_process_passed(report)) else 'failed'
    put(output / 'report.json', report)
    put(output / 'inventory.json', {str(p.relative_to(output)): dict(sha256=sha(p), bytes=p.stat().st_size)
                                  for p in sorted(output.rglob('*')) if p.is_file() and p.name != 'inventory.json'})
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--native', type=Path)
    parser.add_argument('--wasm', type=Path)
    parser.add_argument('--napi', type=Path)
    parser.add_argument('--fixture', type=Path, default=fixtures.DEFAULT)
    parser.add_argument('--node', default='node')
    parser.add_argument('--timeout', type=float, default=30)
    parser.add_argument('--portable-timeout', type=float, default=600)
    args = parser.parse_args()
    if bool(args.wasm) != bool(args.napi) or not (args.native or args.wasm) or args.timeout <= 0 or args.portable_timeout <= 0:
        parser.error('supply --native and/or paired --wasm/--napi; timeout must be positive')
    try:
        report = execute(args)
        print(json.dumps(dict(status=report['status'], cases=len(report['case_ids']), oracle_boundaries=len(report['oracle_boundaries']), targets={key: {name: value for name, value in row.items() if name != 'results'} for key, row in report['targets'].items()}), indent=2))
        return 0 if report['status'] == 'passed' else 1
    except (OSError, ValueError, KeyError) as error:
        print('Parameterized object comparison failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
