#!/usr/bin/env python3
"""Acquire and independently audit source-driven pinned Java Logger evidence."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys

import benchmark
import oracle

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-deprecation'
JAR_SHA256 = 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
REVISION = 'e51fb2f91774265666f94980ec630db794a4db2d'
STATUSES = {b'success\n', b'error\n', b'oracle-error\n', b'api-exception\n'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inventory(root):
    require(not root.is_symlink(), 'evidence root is a symlink')
    found = {}
    for folder, directories, names in os.walk(root, followlinks=False):
        for name in directories + names:
            require(not (Path(folder) / name).is_symlink(), 'evidence contains a symlink')
        for name in names:
            path = Path(folder) / name
            require(path.is_file(), 'nonregular evidence file')
            if path != root / 'report.json':
                found[path.relative_to(root).as_posix()] = dict(sha256=sha(path), bytes=path.stat().st_size)
    return dict(sorted(found.items()))


def load_cases(path, fixture=FIXTURE):
    manifest = json.loads(path.read_bytes())
    require(manifest['schema'] == 1 and manifest['baseline'] == 'Pkl 0.32.0'
            and manifest['repetitions'] == 2, 'unsupported Java control manifest')
    seen = set()
    for case in manifest['cases']:
        require(re.fullmatch('[a-z0-9-]+', case['id']) and case['id'] not in seen, 'duplicate/invalid case')
        seen.add(case['id'])
        require(case['action'] in {'capture', 'noop', 'throw', 'throw-once', 'close', 'close-reenter'},
                'unsupported logger action')
        require(isinstance(case['callback'], str) and '\n' not in case['callback'], 'invalid callback')
        require(type(case.get('callback_after', 0)) is int and case.get('callback_after', 0) >= 0,
                'invalid callback warning threshold')
        require(type(case.get('evaluator_cycles', 1)) is int and 1 <= case.get('evaluator_cycles', 1) <= 16,
                'invalid evaluator cycle count')
        require(case['expressions'] and all(isinstance(x, str) and x and '\n' not in x and '\r' not in x
                                           for x in case['expressions']), 'invalid ordered calls')
        source = fixture / case['source']
        require(case['source'].startswith('inputs/') and '..' not in Path(case['source']).parts
                and not source.is_symlink() and sha(source) == case['source_sha256'], 'control source changed')
    require(seen, 'empty controls')
    return manifest


def payloads(directory):
    return {path.name: path.read_bytes() for path in sorted(directory.glob('*.bin'))}


def check_string(raw, name):
    require(name + '.bin' in raw and name + '.code-units.bin' in raw,
            'missing Java UTF-8/UTF-16 string')
    units = raw[name + '.code-units.bin']
    require(len(units) % 2 == 0, 'odd Java UTF-16 byte count')
    # StandardCharsets.UTF_8 replaces each unmatched Java surrogate with ASCII '?'.
    expected = units.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace')
    require(raw[name + '.bin'] == expected, 'Java UTF-8 differs from captured Java UTF-16')


def audit_session(directory, case):
    raw = payloads(directory)
    levels = []
    for i in range(sum(name.endswith('.level.bin') for name in raw)):
        prefix = f'log-{i}'
        level = raw[prefix + '.level.bin']
        require(level in {b'trace', b'warn'}, 'unsupported logger level')
        levels.append(level.decode())
        for field in ['message', 'uri', 'member']:
            check_string(raw, prefix + '.' + field)
        require(raw[prefix + '.member.null.bin'] in {b'true', b'false'}, 'missing nullable frame identity')
        require(re.fullmatch(rb'\d+:\d+:\d+:\d+', raw[prefix + '.position.bin']), 'missing full frame position')
        count = int(raw[prefix + '.source-count.bin'])
        require(count > 0, 'empty source frame')
        require(int(raw[prefix + '.message.length.bin']) * 2
                == len(raw[prefix + '.message.code-units.bin']), 'Java message unit length differs')
        for line in range(count):
            check_string(raw, prefix + f'.source-{line}')
    statuses = {}
    required = ([f'cycle-{cycle}-outer-{call}' for cycle in range(case['evaluator_cycles'])
                 for call in range(len(case['expressions']))] if 'evaluator_cycles' in case
                else [f'outer-{i}' for i in range(len(case['expressions']))])
    require(all(prefix + '.status.bin' in raw for prefix in required), 'missing outer call')
    for name, data in raw.items():
        if name.endswith('.status.bin'):
            require(data in STATUSES, 'invalid public call status')
            prefix = name.removesuffix('.status.bin')
            statuses[prefix] = data.decode().removesuffix('\n')
            for channel in ['stdout', 'stderr']:
                check_string(raw, prefix + '.' + channel)
            require(prefix + '.error-class.bin' in raw, 'missing Java exception provenance')
    protocol = (directory / 'process/stdout.bin').read_bytes()
    if 'evaluator_cycles' in case:
        for cycle in range(case['evaluator_cycles']):
            for event in ['enter', 'exit']:
                require(protocol.count(f'cycle-{event}={cycle}\n'.encode()) == 1,
                        'missing sequential evaluator lifecycle event')
    for prefix in required:
        require(protocol.count(('call-enter=' + prefix + '\n').encode()) == 1
                and protocol.count(('call-exit=' + prefix + ' status=').encode()) == 1,
                'ordered call protocol differs')
    for index, level in enumerate(levels):
        require(protocol.count(f'log-enter={index} level={level} depth='.encode()) == 1,
                'ordered callback entry protocol differs')
    require(protocol.endswith(f'closed logs={len(levels)} warnings={levels.count("warn")}\n'.encode()),
            'final callback counts differ')
    return dict(levels=levels, warnings=levels.count('warn'), traces=levels.count('trace'), statuses=statuses,
                oracle_errors=[key for key, value in statuses.items() if value == 'oracle-error'],
                payloads={name: dict(sha256=hashlib.sha256(data).hexdigest(), bytes=len(data))
                          for name, data in raw.items()})


def verify_report(output):
    report = json.loads((output / 'report.json').read_bytes())
    require(report['schema'] == 1 and report['files'] == inventory(output), 'Java evidence inventory differs')
    meta = json.loads((output / 'oracle.json').read_bytes())
    require(meta == report['oracle'] and meta['version'] == '0.32.0'
            and meta['source']['revision'] == REVISION and meta['artifact_sha256'] == JAR_SHA256,
            'pinned Java authority differs')
    captured = output / 'sources/tests/fixtures/m2-deprecation'
    cycles = report.get('kind', 'main') == 'cycles'
    require(report.get('kind', 'main') in {'main', 'cycles'}, 'unsupported Java observation kind')
    manifest = load_cases(captured / ('cycles.json' if cycles else 'cases.json'), captured)
    require(report['cases'] == manifest['cases'] and report['repetitions'] == manifest['repetitions'],
            'Java execution selection differs')
    require(report['sources'] == report['sources_after'], 'acquisition sources changed')
    for name, digest in report['sources'].items():
        require(sha(output / 'sources' / name) == digest, 'source snapshot differs')
    require(sha(output / 'sources/oracle.lock.json') == meta['lock_sha256'], 'oracle lock differs')
    expected = [(case['id'], repeat) for case in manifest['cases'] for repeat in range(manifest['repetitions'])]
    require([(row['id'], row['repeat']) for row in report['results']] == expected, 'missing/reordered Java session')
    for row in report['results']:
        case = next(case for case in manifest['cases'] if case['id'] == row['id'])
        directory = output / 'cases' / row['id'] / str(row['repeat'])
        process = json.loads((directory / 'process/result.json').read_bytes())
        require(process == row['process'] and process['status'] == 'executed' and process['exit_code'] == 0,
                'Java session was not executed successfully')
        index = meta['command'].index('-cp')
        command = process['command']
        tail = [case['uri'], str(case['evaluator_cycles'])] if cycles else [case['uri'], case['action'], case['callback']]
        if not cycles and 'callback_after' in case:
            tail.append(str(case['callback_after']))
        require(command[:index] == meta['command'][:index]
                and command[index:index + 1] == ['-cp']
                and command[index + 1].endswith(os.pathsep + meta['artifact'])
                and command[index + 2] == ('M2DeprecationCyclesProbe' if cycles else 'M2DeprecationProbe')
                and command[-len(tail):] == tail,
                'Java callback driver arguments differ')
        require(sha(directory / 'input.pkl') == case['source_sha256'], 'executed source differs')
        require((directory / 'expressions.txt').read_bytes() == ('\n'.join(case['expressions']) + '\n').encode(),
                'executed unwrapped queries differ')
        require(row['observation'] == audit_session(directory, case), 'Java raw observation differs')
        for channel in ['stdout', 'stderr']:
            path = directory / 'process' / (channel + '.bin')
            require(sha(path) == process[channel]['sha256']
                    and path.stat().st_size == process[channel]['bytes'], 'raw process capture differs')
    require(report['replay_equal'] == replay_equal(output, manifest), 'fresh Java repetition comparison differs')
    require(report['compile'] == json.loads((output / 'compile/result.json').read_bytes())
            and report['compile']['status'] == 'executed' and report['compile']['exit_code'] == 0,
            'Java driver compilation provenance differs')
    require(report['results'] == json.loads((output / 'partial-results.json').read_bytes()),
            'Java progress records differ')
    return report


def replay_equal(output, manifest):
    result = {}
    for case in manifest['cases']:
        base = output / 'cases' / case['id']
        left, right = payloads(base / '0'), payloads(base / '1')
        for channel in ['stdout', 'stderr']:
            left['process.' + channel] = (base / '0/process' / (channel + '.bin')).read_bytes()
            right['process.' + channel] = (base / '1/process' / (channel + '.bin')).read_bytes()
        result[case['id']] = left == right
    return result


def run(output, cycles=False):
    manifest_path = FIXTURE / ('cycles.json' if cycles else 'cases.json')
    manifest = load_cases(manifest_path)
    output.mkdir(parents=True, exist_ok=False)
    meta = oracle.ensure_oracle(ROOT, fetch=False)
    benchmark.write_json(output / 'oracle.json', meta)
    paths = [ROOT / name for name in ['oracle.lock.json', 'references.lock', 'tools/m2_deprecation.py',
             'tools/java/M2DeprecationProbe.java', 'tools/oracle.py', 'tools/benchmark.py']]
    paths += list(FIXTURE.glob('inputs/*.pkl')) + [manifest_path]
    if cycles:
        paths.append(ROOT / 'tools/java/M2DeprecationCyclesProbe.java')
    source_root = ROOT / 'external/pkl/pkl-core/src/main/java/org/pkl/core'
    paths += [source_root / name for name in ['Logger.java', 'StackFrame.java', 'Evaluator.java',
        'EvaluatorImpl.java', 'runtime/VmUtils.java', 'ast/internal/ToStringNode.java',
        'ast/member/ClassMethod.java', 'ast/member/UnresolvedMethodNode.java',
        'ast/expression/member/AbstractInvokeMethodLexicalNode.java',
        'ast/expression/member/InvokeMethodVirtualNode.java', 'ast/expression/member/InvokeSuperMethodNode.java',
        'ast/expression/member/InvokeMethodDirectNode.java', 'ast/expression/member/InvokeObjectMethodNode.java']]
    paths += [ROOT / 'external/pkl/stdlib' / name for name in ['base.pkl', 'ref.pkl']]
    sources = {}
    for source in paths:
        name = str(source.relative_to(ROOT))
        destination = output / 'sources' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source, destination)
        sources[name] = sha(destination)
    classes = output / 'classes'
    classes.mkdir()
    java_sources = [str(ROOT / 'tools/java/M2DeprecationProbe.java')]
    if cycles:
        java_sources.append(str(ROOT / 'tools/java/M2DeprecationCyclesProbe.java'))
    compile_process = benchmark.capture([str(Path(meta['java']).with_name('javac')), '-cp', meta['artifact'],
        '-d', str(classes), *java_sources], ROOT, meta['environment'],
        output / 'compile', timeout=60)
    require(compile_process['status'] == 'executed' and compile_process['exit_code'] == 0,
            'Java driver compilation failed; raw attempt retained')
    index = meta['command'].index('-cp')
    prefix = [*meta['command'][:index], '-cp', str(classes) + os.pathsep + meta['artifact'],
              'M2DeprecationCyclesProbe' if cycles else 'M2DeprecationProbe']
    rows = []
    for case in manifest['cases']:
        for repeat in range(manifest['repetitions']):
            directory = output / 'cases' / case['id'] / str(repeat)
            directory.mkdir(parents=True)
            shutil.copyfile(FIXTURE / case['source'], directory / 'input.pkl')
            (directory / 'expressions.txt').write_bytes(('\n'.join(case['expressions']) + '\n').encode())
            arguments = ([*prefix, str(directory), case['uri'], str(case['evaluator_cycles'])] if cycles
                         else [*prefix, str(directory), case['uri'], case['action'], case['callback']])
            if not cycles and 'callback_after' in case:
                arguments.append(str(case['callback_after']))
            process = benchmark.capture(arguments,
                ROOT, meta['environment'], directory / 'process', timeout=30)
            require(process['status'] == 'executed' and process['exit_code'] == 0,
                    'Java driver acquisition failed; raw attempt retained')
            observation = audit_session(directory, case)
            rows.append(dict(id=case['id'], repeat=repeat, process=process, observation=observation))
            benchmark.write_json(output / 'partial-results.json', rows)
            print(case['id'], repeat, 'warnings=' + str(observation['warnings']),
                  dict(Counter(observation['statuses'].values())), flush=True)
    report = dict(schema=1, kind='cycles' if cycles else 'main', scope=manifest['scope'], oracle=meta, cases=manifest['cases'],
        repetitions=manifest['repetitions'], sources=sources,
        sources_after={name: sha(ROOT / name) for name in sources}, compile=compile_process, results=rows,
        replay_equal=replay_equal(output, manifest), compatibility_passes_claimed=0)
    report['files'] = inventory(output)
    benchmark.write_json(output / 'report.json', report)
    verify_report(output)
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--output', type=Path)
    modes.add_argument('--cycles-output', type=Path)
    modes.add_argument('--verify-report', type=Path)
    args = parser.parse_args()
    report = (verify_report(args.verify_report.resolve()) if args.verify_report
              else run((args.cycles_output or args.output).resolve(), cycles=bool(args.cycles_output)))
    print(json.dumps(dict(sessions=len(report['cases']), java_processes=len(report['results']),
                         repeated_raw_equal=sum(report['replay_equal'].values()), compatibility_passes_claimed=0)))
    return 0


if __name__ == '__main__':
    sys.exit(main())
