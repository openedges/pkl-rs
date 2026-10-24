#!/usr/bin/env python3
"""Acquire/audit source-driven pinned Java frame-transformer observations."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-frame-transformers'
REVISION = 'e51fb2f91774265666f94980ec630db794a4db2d'
JAR_SHA256 = 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
MODES = {'default', 'builtin-identity', 'identity', 'full', 'numbered'}
STATUSES = {b'success\n', b'error\n', b'oracle-error\n', b'api-exception\n'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def sha(path):
    return digest(path.read_bytes())


def put(path, value):
    path.write_bytes((json.dumps(value, indent=2, ensure_ascii=True) + '\n').encode())


def inventory(root):
    require(not root.is_symlink(), 'evidence root is a symlink')
    result = {}
    for folder, directories, names in os.walk(root, followlinks=False):
        for name in directories + names:
            require(not (Path(folder) / name).is_symlink(), 'evidence contains a symlink')
        for name in names:
            path = Path(folder) / name
            require(path.is_file(), 'nonregular evidence file')
            if path != root / 'report.json':
                result[path.relative_to(root).as_posix()] = dict(sha256=sha(path), bytes=path.stat().st_size)
    return dict(sorted(result.items()))


def unit_bytes(value):
    require(isinstance(value, list) and all(type(unit) is int and 0 <= unit <= 65535 for unit in value),
            'expected exact unsigned Java UTF16 code units')
    return b''.join(unit.to_bytes(2, 'big') for unit in value)


def check_frame_config(frame):
    require(isinstance(frame, dict) and set(frame) == {'moduleUri', 'memberName', 'sourceLines',
            'startLine', 'startColumn', 'endLine', 'endColumn'}, 'incomplete replacement frame')
    unit_bytes(frame['moduleUri'])
    if frame['memberName'] is not None:
        unit_bytes(frame['memberName'])
    require(isinstance(frame['sourceLines'], list), 'expected source line array')
    for line in frame['sourceLines']:
        unit_bytes(line)
    require(all(type(frame[key]) is int and 1 <= frame[key] <= 2147483647
                for key in ['startLine', 'startColumn', 'endLine', 'endColumn']), 'invalid frame position')
    require(frame['startLine'] <= frame['endLine'] and (frame['startLine'] != frame['endLine']
            or frame['startColumn'] <= frame['endColumn']), 'reversed replacement frame')


def load_cases(path, fixture=FIXTURE):
    manifest = json.loads(path.read_bytes())
    require(manifest['schema'] == 1 and manifest['baseline'] == 'Pkl 0.32.0'
            and type(manifest['repetitions']) is int and manifest['repetitions'] == 2,
            'unsupported frame oracle manifest')
    seen = set()
    for case in manifest['cases']:
        require(re.fullmatch(r'[a-z0-9-]+', case['id']) and case['id'] not in seen, 'invalid/duplicate case')
        seen.add(case['id'])
        require(case['source'].startswith('inputs/') and '..' not in Path(case['source']).parts
                and sha(fixture / case['source']) == case['source_sha256'], 'frame input source differs')
        require(case['operation'] in {'expression', 'test'}, 'unsupported public operation')
        if case['operation'] == 'expression':
            require('calls' not in case and case['expressions'] and all(isinstance(value, str) and value
                    and '\n' not in value and '\r' not in value for value in case['expressions']), 'invalid exact expressions')
        else:
            require('expressions' not in case and type(case['calls']) is int and 1 <= case['calls'] <= 32,
                    'invalid public test call count')
        for kind in ['transformer', 'logger']:
            settings = case[kind]
            require(settings['action'] in {'none', 'throw-once'} and isinstance(settings['callback'], str)
                    and '\n' not in settings['callback'] and '\r' not in settings['callback']
                    and type(settings['callback_after']) is int and settings['callback_after'] >= 0,
                    'invalid callback profile')
        require(case['logger']['mode'] in {'capture', 'noop'} and case['transformer']['mode'] in MODES,
                'invalid logger/transformer mode')
        if case['transformer']['mode'] in {'default', 'builtin-identity'}:
            require(case['transformer']['action'] == 'none' and case['transformer']['callback'] == '',
                    'built-in policy cannot invent observed custom callbacks')
        if case['logger']['mode'] == 'noop':
            require(case['logger']['action'] == 'none' and case['logger']['callback'] == '', 'noop cannot observe logger actions')
        if case['transformer']['mode'] in {'full', 'numbered'}:
            check_frame_config(case['replacement'])
        else:
            require(case['replacement'] is None, 'unused replacement frame')
    require(seen, 'empty frame oracle selection')
    return manifest


def argument_tail(case):
    return [case['uri'], case['operation'], case['transformer']['mode'], case['transformer']['action'],
            case['transformer']['callback'], str(case['transformer']['callback_after']), case['logger']['mode'],
            case['logger']['action'], case['logger']['callback'], str(case['logger']['callback_after'])]


def input_files(case, source):
    result = {'input.pkl': source}
    if case['operation'] == 'expression':
        result['expressions.txt'] = ('\n'.join(case['expressions']) + '\n').encode()
    else:
        result['calls.txt'] = str(case['calls']).encode()
    frame = case['replacement']
    if frame is not None:
        result.update({'replacement/uri.u16be': unit_bytes(frame['moduleUri']),
            'replacement/member.u16be': unit_bytes(frame['memberName'] or []),
            'replacement/member-null.txt': str(frame['memberName'] is None).lower().encode(),
            'replacement/position.txt': ':'.join(str(frame[name]) for name in ['startLine', 'startColumn', 'endLine', 'endColumn']).encode(),
            'replacement/source-count.txt': str(len(frame['sourceLines'])).encode()})
        result.update({f'replacement/source-{index}.u16be': unit_bytes(line) for index, line in enumerate(frame['sourceLines'])})
    return result


def check_string(raw, name):
    require(name + '.bin' in raw and name + '.code-units.bin' in raw, 'missing Java string channel: ' + name)
    units = raw[name + '.code-units.bin']
    require(len(units) % 2 == 0, 'odd Java UTF16 code-unit channel')
    require(raw[name + '.bin'] == units.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace'),
            'Java UTF8 differs from captured UTF16: ' + name)


def audit_frame(raw, prefix):
    for field in ['uri', 'member']:
        check_string(raw, prefix + '.' + field)
    require(raw[prefix + '.member.null.bin'] in {b'true', b'false'}, 'invalid nullable frame member')
    if raw[prefix + '.member.null.bin'] == b'true':
        require(raw[prefix + '.member.code-units.bin'] == b'', 'null frame member carries string units')
    require(re.fullmatch(rb'-?\d+:-?\d+:-?\d+:-?\d+', raw[prefix + '.position.bin']), 'missing complete frame position')
    count = raw[prefix + '.source-count.bin']
    require(count.isdigit(), 'invalid source line count')
    for line in range(int(count)):
        check_string(raw, prefix + f'.source-{line}')
    actual = {name for name in raw if re.fullmatch(re.escape(prefix) + r'\.source-\d+\.bin', name)}
    require(actual == {prefix + f'.source-{line}.bin' for line in range(int(count))}, 'frame source line coverage differs')


def payloads(directory):
    return {path.name: path.read_bytes() for path in sorted(directory.glob('*.bin'))}


def audit_session(directory, case):
    raw = payloads(directory)
    protocol = (directory / 'process/stdout.bin').read_bytes()
    count = len(case['expressions']) if case['operation'] == 'expression' else case['calls']
    statuses = {name.removesuffix('.status.bin'): data for name, data in raw.items() if name.endswith('.status.bin')}
    require({f'outer-{i}' for i in range(count)}.issubset(statuses), 'missing public outer call')
    for prefix, status in statuses.items():
        require(re.fullmatch(r'(?:outer|transform-callback|log-callback)-\d+', prefix) and status in STATUSES,
                'invalid public call status')
        for channel in ['stdout', 'stderr']:
            check_string(raw, prefix + '.' + channel)
        require(prefix + '.error-class.bin' in raw, 'missing actual Java exception class provenance')
        require(protocol.count(('call-enter=' + prefix + '\n').encode()) == 1
                and protocol.count(('call-exit=' + prefix + ' status=').encode()) == 1, 'public call protocol differs')
    transforms = sorted(int(match[1]) for name in raw if (match := re.fullmatch(r'transform-(\d+)\.input\.uri\.bin', name)))
    require(transforms == list(range(len(transforms))), 'transform callback coverage differs')
    for index in transforms:
        prefix = 'transform-' + str(index)
        audit_frame(raw, prefix + '.input')
        require(protocol.count((f'transform-enter={index} depth=').encode()) == 1, 'transform entry protocol differs')
        if prefix + '.output.uri.bin' in raw:
            audit_frame(raw, prefix + '.output')
            require(protocol.count(f'transform-exit={index}\n'.encode()) == 1
                    and f'transform-throw={index}\n'.encode() not in protocol, 'transform completion protocol differs')
        else:
            require(protocol.count(f'transform-throw={index}\n'.encode()) == 1
                    and f'transform-exit={index}\n'.encode() not in protocol, 'missing failed transform observation')
    levels = []
    indices = sorted(int(match[1]) for name in raw if (match := re.fullmatch(r'log-(\d+)\.level\.bin', name)))
    require(indices == list(range(len(indices))), 'logger callback coverage differs')
    for index in indices:
        prefix = 'log-' + str(index)
        level = raw[prefix + '.level.bin']
        require(level in {b'warn', b'trace'}, 'unsupported logger level')
        levels.append(level.decode())
        check_string(raw, prefix + '.message')
        require(int(raw[prefix + '.message.length.bin']) * 2 == len(raw[prefix + '.message.code-units.bin']), 'logger message length differs')
        audit_frame(raw, prefix)
        require(protocol.count(f'log-enter={index} level={level.decode()} depth='.encode()) == 1, 'logger entry protocol differs')
        require((protocol.count(f'log-exit={index}\n'.encode()), protocol.count(f'log-throw={index}\n'.encode()))
                in {(1, 0), (0, 1)}, 'logger completion protocol differs')
    require(protocol.endswith(f'closed transforms={len(transforms)} logs={len(indices)} warnings={levels.count("warn")}\n'.encode()),
            'terminal callback count protocol differs')
    if case['logger']['mode'] == 'noop':
        require(not indices, 'noop delegate invented logger callbacks')
    if case['transformer']['mode'] in {'default', 'builtin-identity'}:
        require(not transforms, 'built-in policy invented observed custom transformer callbacks')
    for name in raw:
        if name.endswith('.code-units.bin'):
            check_string(raw, name.removesuffix('.code-units.bin'))
    return dict(transformer_callbacks=len(transforms), logger_callbacks=len(indices), warning_callbacks=levels.count('warn'),
        call_statuses={prefix: status.decode().removesuffix('\n') for prefix, status in statuses.items()},
        oracle_error_calls=sum(status == b'oracle-error\n' for status in statuses.values()),
        api_exception_calls=sum(status == b'api-exception\n' for status in statuses.values()),
        nested_oracle_error_results=sum(data == b'org.pkl.core.PklBugException' and '.test.error-' in name
                                       for name, data in raw.items() if name.endswith('.error-class.bin')),
        payloads={name: dict(sha256=digest(data), bytes=len(data)) for name, data in raw.items()})


def replay_equal(output, manifest):
    result = {}
    for case in manifest['cases']:
        base = output / 'cases' / case['id']
        observations = []
        for repeat in range(2):
            raw = payloads(base / str(repeat))
            for channel in ['stdout', 'stderr']:
                raw['process.' + channel] = (base / str(repeat) / 'process' / (channel + '.bin')).read_bytes()
            observations.append(raw)
        result[case['id']] = observations[0] == observations[1]
    return result


def verify_report(output):
    report = json.loads((output / 'report.json').read_bytes())
    require(report['schema'] == 1 and report['files'] == inventory(output), 'frame evidence file inventory differs')
    require(report['kind'] in {'main', 'buffer'}, 'unknown frame oracle cohort')
    fixture = output / 'sources/tests/fixtures/m2-frame-transformers'
    manifest = load_cases(fixture / ('cases.json' if report['kind'] == 'main' else 'buffer-cases.json'), fixture)
    require(report['cases'] == manifest['cases'] and report['repetitions'] == manifest['repetitions'], 'frame oracle selection differs')
    require(report['sources_before'] == report['sources_after'], 'frame oracle sources changed during acquisition')
    for name, value in report['sources_before'].items():
        require(sha(output / 'sources' / name) == value, 'frame oracle source snapshot differs')
    metadata = json.loads((output / 'oracle.json').read_bytes())
    require(metadata == report['oracle'] and metadata['version'] == '0.32.0'
            and metadata['source']['revision'] == REVISION and metadata['artifact_sha256'] == JAR_SHA256
            and sha(output / 'sources/oracle.lock.json') == metadata['lock_sha256'], 'pinned Java authority differs')
    expected = [(case['id'], repeat) for case in manifest['cases'] for repeat in range(2)]
    require([(row['id'], row['repeat']) for row in report['results']] == expected, 'missing/reordered frame oracle process')
    for row in report['results']:
        case = next(case for case in manifest['cases'] if case['id'] == row['id'])
        directory = output / 'cases' / row['id'] / str(row['repeat'])
        process = json.loads((directory / 'process/result.json').read_bytes())
        require(process == row['process'] and process['status'] == 'executed' and process['exit_code'] == 0,
                'frame oracle process was not executed')
        index = metadata['command'].index('-cp')
        require(process['command'][:index] == metadata['command'][:index]
                and process['command'][index] == '-cp'
                and process['command'][index + 1].endswith(os.pathsep + metadata['artifact'])
                and process['command'][index + 2] == 'M2FrameTransformerProbe'
                and process['command'][-10:] == argument_tail(case), 'frame oracle callback arguments differ')
        for name, value in input_files(case, (fixture / case['source']).read_bytes()).items():
            require((directory / name).read_bytes() == value, 'executed frame source/query/replacement input differs')
        require(audit_session(directory, case) == row['observation'], 'frame oracle raw observation differs')
        for channel in ['stdout', 'stderr']:
            path = directory / 'process' / (channel + '.bin')
            require(process[channel]['sha256'] == sha(path) and process[channel]['bytes'] == path.stat().st_size,
                    'frame oracle raw process channel differs')
    require(report['replay_equal'] == replay_equal(output, manifest), 'fresh frame oracle replay comparison differs')
    require(report['results'] == json.loads((output / 'partial-results.json').read_bytes()), 'frame progress records differ')
    require(report['compile'] == json.loads((output / 'compile/result.json').read_bytes())
            and report['compile']['status'] == 'executed' and report['compile']['exit_code'] == 0, 'frame Java compilation provenance differs')
    return report


def run(output, kind):
    import benchmark
    import oracle
    manifest_path = FIXTURE / ('cases.json' if kind == 'main' else 'buffer-cases.json')
    manifest = load_cases(manifest_path)
    output.mkdir(parents=True, exist_ok=False)
    names = ['oracle.lock.json', 'references.lock', 'tools/frame_transformer_oracle.py',
        'tools/java/M2FrameTransformerProbe.java', 'tools/benchmark.py', 'tools/oracle.py']
    names += [str(path.relative_to(ROOT)) for path in FIXTURE.glob('inputs/*.pkl')]
    names.append(str(manifest_path.relative_to(ROOT)))
    base = 'external/pkl/pkl-core/src/main/java/org/pkl/core/'
    names += [base + name for name in ['StackFrame.java', 'StackFrameTransformer.java', 'StackFrameTransformers.java',
        'BufferedLogger.java', 'Evaluator.java', 'EvaluatorBuilder.java', 'EvaluatorImpl.java', 'TestResults.java',
        'Loggers.java', 'runtime/LoggerImpl.java', 'runtime/StackTraceRenderer.java', 'runtime/VmException.java',
        'runtime/VmBugException.java', 'runtime/TestRunner.java']]
    before = {}
    for name in names:
        target = output / 'sources' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
        before[name] = sha(target)
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    put(output / 'oracle.json', metadata)
    classes = output / 'classes'
    classes.mkdir()
    compiled = benchmark.capture([str(Path(metadata['java']).with_name('javac')), '-cp', metadata['artifact'], '-d', str(classes),
        str(ROOT / 'tools/java/M2FrameTransformerProbe.java')], ROOT, metadata['environment'], output / 'compile', timeout=60)
    require(compiled['status'] == 'executed' and compiled['exit_code'] == 0, 'Java frame probe compilation failed; original attempt retained')
    index = metadata['command'].index('-cp')
    java = [*metadata['command'][:index], '-cp', str(classes) + os.pathsep + metadata['artifact'], 'M2FrameTransformerProbe']
    results = []
    for case in manifest['cases']:
        for repeat in range(2):
            directory = output / 'cases' / case['id'] / str(repeat)
            directory.mkdir(parents=True)
            for name, value in input_files(case, (FIXTURE / case['source']).read_bytes()).items():
                path = directory / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(value)
            process = benchmark.capture([*java, str(directory), *argument_tail(case)], ROOT, metadata['environment'], directory / 'process', timeout=30)
            require(process['status'] == 'executed' and process['exit_code'] == 0, 'Java frame probe acquisition failed; original attempt retained')
            observation = audit_session(directory, case)
            results.append(dict(id=case['id'], repeat=repeat, process=process, observation=observation))
            put(output / 'partial-results.json', results)
            print(case['id'], repeat, 'transforms=' + str(observation['transformer_callbacks']),
                  dict(Counter(observation['call_statuses'].values())), flush=True)
    report = dict(schema=1, kind=kind, scope=manifest['scope'], cases=manifest['cases'], repetitions=2,
        oracle=metadata, compile=compiled, sources_before=before,
        sources_after={name: sha(ROOT / name) for name in before}, results=results,
        replay_equal=replay_equal(output, manifest), compatibility_passes_claimed=0)
    report['files'] = inventory(output)
    put(output / 'report.json', report)
    verify_report(output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--output', type=Path)
    modes.add_argument('--verify-report', type=Path)
    parser.add_argument('--kind', choices=['main', 'buffer'], default='main')
    args = parser.parse_args()
    report = verify_report(args.verify_report.resolve()) if args.verify_report else run(args.output.resolve(), args.kind)
    print(json.dumps(dict(sessions=len(report['cases']), java_processes=len(report['results']),
                         repeated_raw_equal=sum(report['replay_equal'].values()), compatibility_passes_claimed=0)))


if __name__ == '__main__':
    main()
