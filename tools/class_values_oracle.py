#!/usr/bin/env python3
"""Capture and independently audit actual pinned Java Class-value observations."""
import argparse
from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import sys

import frame_transformer_oracle as wire

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-class-values'
REVISION = 'e51fb2f91774265666f94980ec630db794a4db2d'
JAR_SHA256 = 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059'
KEY_CONFIG = b'hierarchical=true\nglobbable=false\ncached=true\nlocal=false\nresolved-uri=original-uri\nsecurity=checkResolveModule\n'
STATUSES = {b'success\n', b'error\n', b'oracle-error\n', b'api-exception\n'}
require = wire.require
sha = wire.sha
digest = wire.digest
put = wire.put
inventory = wire.inventory


def same_json(a, b):
    return json.dumps(a, sort_keys=True, separators=(',', ':')) == json.dumps(b, sort_keys=True, separators=(',', ':'))


def load_cases(path, fixture=FIXTURE):
    manifest = json.loads(path.read_bytes())
    require(type(manifest.get('schema')) is int and manifest['schema'] == 1
            and manifest['baseline'] == 'Pkl 0.32.0' and type(manifest['repetitions']) is int
            and manifest['repetitions'] == 2, 'unsupported Class oracle manifest')
    seen = set()
    for case in manifest['cases']:
        require(re.fullmatch(r'[a-z0-9-]+', case['id']) and case['id'] not in seen, 'invalid/duplicate Class case')
        seen.add(case['id'])
        all_sources = [case, *case['support_modules']]
        uris = set()
        for source in all_sources:
            name = source['source']
            require(name.startswith('inputs/') and '..' not in Path(name).parts
                    and sha(fixture / name) == source['source_sha256'], 'authored Class source differs')
            require(isinstance(source['uri'], str) and re.match(r'^[a-z][a-z0-9+.-]*:', source['uri'])
                    and '\n' not in source['uri'] and '\t' not in source['uri']
                    and source['uri'] not in uris, 'invalid/duplicate module source URI')
            uris.add(source['uri'])
        require(case['expressions'] and all(isinstance(value, str) and value and '\n' not in value
                    and '\r' not in value for value in case['expressions']), 'invalid exact Class expressions')
        settings = case['logger']
        require(settings['mode'] in {'capture', 'noop'} and settings['action'] in {'none', 'throw-once', 'close'}
                and isinstance(settings['callback'], str) and '\n' not in settings['callback']
                and '\r' not in settings['callback'] and type(settings['callback_after']) is int
                and settings['callback_after'] >= 0, 'invalid Class logger profile')
        if settings['mode'] == 'noop':
            require(settings['action'] == 'none' and settings['callback'] == '', 'noop invented logger action')
    require(seen, 'empty Class selection')
    return manifest


def argument_tail(case):
    logger = case['logger']
    return [case['uri'], logger['mode'], logger['action'], logger['callback'], str(logger['callback_after'])]


def input_files(case, fixture=FIXTURE):
    files = {'input.pkl': (fixture / case['source']).read_bytes(),
             'expressions.txt': ('\n'.join(case['expressions']) + '\n').encode()}
    mappings = []
    for index, source in enumerate(case['support_modules']):
        name = f'support-{index}.pkl'
        files[name] = (fixture / source['source']).read_bytes()
        mappings.append(source['uri'] + '\t' + name)
    files['modules.tsv'] = (''.join(row + '\n' for row in mappings)).encode()
    return files


def payloads(directory):
    return {path.name: path.read_bytes() for path in sorted(directory.glob('*.bin'))}


def audit_provider(directory, case):
    root = directory / 'provider'
    require(root.is_dir() and not root.is_symlink(), 'missing actual provider observations')
    raw = payloads(root)
    require(raw['key-configuration.bin'] == KEY_CONFIG, 'support key behavior differs')
    indices = sorted(int(m[1]) for name in raw if (m := re.fullmatch(r'event-(\d+)\.kind\.bin', name)))
    require(indices == list(range(len(indices))), 'provider callback coverage differs')
    expected = {'key-configuration.bin', 'protocol.bin'}
    protocol, observations = [], []
    known = {row['uri'] for row in case['support_modules']}
    creates = set()
    for index in indices:
        prefix = f'event-{index}'
        wire.check_string(raw, prefix + '.uri')
        kind, matched = raw[prefix + '.kind.bin'], raw[prefix + '.matched.bin']
        require(kind in {b'create', b'resolve'} and matched in {b'true', b'false'}, 'invalid provider callback')
        uri = raw[prefix + '.uri.bin'].decode()
        if kind == b'create':
            require((matched == b'true') == (uri in known), 'provider match differs from authored sources')
            if matched == b'true':
                creates.add(uri)
        else:
            require(uri in known and uri in creates and matched == b'true', 'resolve without matched provider creation')
        protocol.append(f'provider={index} kind={kind.decode()} matched={matched.decode()}\n')
        expected.update({prefix + suffix for suffix in ['.kind.bin', '.matched.bin', '.uri.bin', '.uri.code-units.bin']})
        observations.append(dict(kind=kind.decode(), uri=uri, matched=matched == b'true'))
    require(raw['protocol.bin'] == ''.join(protocol).encode(), 'actual provider protocol differs')
    require(set(raw) == expected and set(p.name for p in root.iterdir()) == expected, 'extra/missing provider channel')
    return dict(callbacks=observations, payloads={name: dict(sha256=digest(data), bytes=len(data)) for name, data in raw.items()},
                boundary='Actual Java public ModuleKeyFactory.create/ModuleKey.resolve observations. Candidate provider API parity is unexecuted unless separately mapped and compared; no inferred cache-hit events.')


def audit_session(directory, case):
    raw = payloads(directory)
    protocol = (directory / 'process/stdout.bin').read_bytes()
    require((directory / 'protocol.bin').read_bytes() == protocol, 'common protocol differs from actual Java stdout')
    require((directory / 'process/stderr.bin').read_bytes() == b'', 'Java process emitted extra stderr')
    statuses = {name.removesuffix('.status.bin'): data for name, data in raw.items() if name.endswith('.status.bin')}
    require({f'outer-{i}' for i in range(len(case['expressions']))}.issubset(statuses), 'missing public Class query')
    expected = {'protocol.bin'}
    for prefix, status in statuses.items():
        require(re.fullmatch(r'(?:outer|log-callback)-\d+', prefix) and status in STATUSES, 'invalid public Class call')
        for channel in ['stdout', 'stderr']:
            wire.check_string(raw, prefix + '.' + channel)
        require(prefix + '.error-class.bin' in raw, 'missing actual Java exception provenance')
        if status == b'success\n':
            require(raw[prefix + '.stderr.bin'] == b'' and raw[prefix + '.error-class.bin'] == b'', 'successful Class query carries error')
        else:
            require(raw[prefix + '.stdout.bin'] == b'' and raw[prefix + '.error-class.bin'], 'failed Class query lacks exception provenance')
        expected.update({prefix + suffix for suffix in ['.stdout.bin', '.stdout.code-units.bin', '.stderr.bin', '.stderr.code-units.bin', '.status.bin', '.error-class.bin']})
    indices = sorted(int(m[1]) for name in raw if (m := re.fullmatch(r'log-(\d+)\.level\.bin', name)))
    require(indices == list(range(len(indices))), 'logger callback coverage differs')
    levels = []
    for index in indices:
        prefix = f'log-{index}'
        level = raw[prefix + '.level.bin']
        require(level in {b'trace', b'warn'}, 'invalid logger level')
        levels.append(level)
        wire.check_string(raw, prefix + '.message')
        require(raw[prefix + '.message.length.bin'].isdigit()
                and int(raw[prefix + '.message.length.bin']) * 2 == len(raw[prefix + '.message.code-units.bin']), 'logger UTF16 length differs')
        wire.audit_frame(raw, prefix)
        expected.update({prefix + suffix for suffix in ['.level.bin', '.message.bin', '.message.code-units.bin', '.message.length.bin',
            '.uri.bin', '.uri.code-units.bin', '.member.bin', '.member.code-units.bin', '.member.null.bin', '.position.bin', '.source-count.bin']})
        for line in range(int(raw[prefix + '.source-count.bin'])):
            expected.update({prefix + f'.source-{line}.bin', prefix + f'.source-{line}.code-units.bin'})
    require(set(raw) == expected, 'extra/missing common Class channel')
    if case['logger']['mode'] == 'noop':
        require(not indices, 'noop invented logger callbacks')
    stack, calls, log_entries = [], set(), set()
    lines = protocol.decode().splitlines()
    for line in lines[:-1]:
        if line.startswith('call-enter='):
            prefix = line.removeprefix('call-enter=')
            require(prefix in statuses and prefix not in calls, 'unknown/repeated Class call entry')
            calls.add(prefix)
            stack.append(('call', prefix))
        elif (match := re.fullmatch(r'log-enter=(\d+) level=(trace|warn) depth=(\d+)', line)):
            index = int(match[1])
            require(index in indices and index not in log_entries and levels[index].decode() == match[2]
                    and int(match[3]) == sum(kind == 'log' for kind, _ in stack), 'logger entry order/depth differs')
            log_entries.add(index)
            stack.append(('log', index))
        elif (match := re.fullmatch(r'log-(?:exit|throw)=(\d+)', line)):
            require(stack and stack.pop() == ('log', int(match[1])), 'logger completion order differs')
        elif (match := re.fullmatch(r'logger-close=(\d+)', line)):
            require(stack and stack[-1] == ('log', int(match[1])) and case['logger']['action'] == 'close', 'unexpected evaluator close')
        elif (match := re.fullmatch(r'call-exit=([a-z0-9-]+) status=([a-z-]+)', line)):
            require(stack and stack.pop() == ('call', match[1]) and statuses[match[1]] == (match[2] + '\n').encode(), 'Class call completion/status differs')
        else:
            require(False, 'unknown common protocol entry')
    require(not stack and calls == set(statuses) and log_entries == set(indices), 'incomplete Class protocol')
    require(lines[-1] == f'closed logs={len(indices)} warnings={levels.count(b"warn")}', 'terminal Class protocol differs')
    return dict(logger_callbacks=len(indices), warning_callbacks=levels.count(b'warn'),
        call_statuses={name: status.decode().removesuffix('\n') for name, status in statuses.items()},
        payloads={name: dict(sha256=digest(data), bytes=len(data)) for name, data in raw.items()},
        provider=audit_provider(directory, case))


def replay_equal(output, manifest):
    result = {}
    for case in manifest['cases']:
        values = []
        for repeat in range(2):
            root = output / 'cases' / case['id'] / str(repeat)
            values.append({**payloads(root), **{'provider/' + name: data for name, data in payloads(root / 'provider').items()},
                           'process.stderr': (root / 'process/stderr.bin').read_bytes()})
        result[case['id']] = values[0] == values[1]
    return result


def source_names(manifest):
    names = ['oracle.lock.json', 'references.lock', 'tools/class_values_oracle.py', 'tools/frame_transformer_oracle.py',
             'tools/java/M2ClassValuesProbe.java', 'tools/benchmark.py', 'tools/oracle.py',
             'tests/fixtures/m2-class-values/cases.json', 'external/pkl/stdlib/base.pkl']
    names.extend('tests/fixtures/m2-class-values/' + source['source'] for case in manifest['cases'] for source in [case, *case['support_modules']])
    base = 'external/pkl/pkl-core/src/main/java/org/pkl/core/'
    names.extend(base + name for name in ['PClassInfo.java', 'Evaluator.java', 'EvaluatorBuilder.java', 'EvaluatorImpl.java',
        'runtime/VmClass.java', 'runtime/VmFunction.java', 'runtime/BaseModule.java', 'runtime/ModuleCache.java',
        'runtime/VmUtils.java', 'runtime/VmTyped.java', 'ast/member/ClassNode.java', 'ast/builder/AstBuilder.java',
        'ast/type/TypeNode.java', 'ast/type/UnresolvedTypeNode.java', 'stdlib/PklConverter.java',
        'stdlib/base/ClassNodes.java', 'stdlib/base/AnyNodes.java', 'module/ModuleKey.java', 'module/ModuleKeyFactory.java',
        'module/ModuleKeys.java', 'module/ResolvedModuleKeys.java'])
    return sorted(set(names))


def verify_report(output):
    report = json.loads((output / 'report.json').read_bytes())
    require(report['schema'] == 1 and same_json(report['files'], inventory(output)), 'Class evidence file inventory differs')
    fixture = output / 'sources/tests/fixtures/m2-class-values'
    manifest = load_cases(fixture / 'cases.json', fixture)
    require(same_json(report['cases'], manifest['cases']) and type(report['repetitions']) is int and report['repetitions'] == 2, 'Class selection differs')
    require(set(report['sources_before']) == set(source_names(manifest)) and same_json(report['sources_before'], report['sources_after']), 'Class experiment source closure/stability differs')
    for name, value in report['sources_before'].items():
        require(sha(output / 'sources' / name) == value, 'Class source snapshot differs')
    require({name.removeprefix('sources/') for name in report['files'] if name.startswith('sources/')} == set(report['sources_before']), 'extra/missing captured Class source')
    metadata = json.loads((output / 'oracle.json').read_bytes())
    require(same_json(metadata, report['oracle']) and metadata['version'] == '0.32.0'
            and metadata['source']['revision'] == REVISION and metadata['artifact_sha256'] == JAR_SHA256
            and sha(output / 'sources/oracle.lock.json') == metadata['lock_sha256'], 'pinned Java Class authority differs')
    expected = [(case['id'], repeat) for case in manifest['cases'] for repeat in range(2)]
    require([(row['id'], row['repeat']) for row in report['results']] == expected
            and all(type(row['repeat']) is int for row in report['results']), 'Class process coverage differs')
    for row in report['results']:
        case = next(case for case in manifest['cases'] if case['id'] == row['id'])
        directory = output / 'cases' / row['id'] / str(row['repeat'])
        process = json.loads((directory / 'process/result.json').read_bytes())
        require(same_json(process, row['process']) and process['status'] == 'executed'
                and type(process['exit_code']) is int and process['exit_code'] == 0, 'Class JVM process did not complete')
        index = metadata['command'].index('-cp')
        require(process['command'][:index] == metadata['command'][:index]
                and process['command'][index] == '-cp' and process['command'][index + 1].endswith(os.pathsep + metadata['artifact'])
                and process['command'][index + 2] == 'M2ClassValuesProbe' and process['command'][-5:] == argument_tail(case), 'Class JVM/driver argument contract differs')
        for name, value in input_files(case, fixture).items():
            require((directory / name).read_bytes() == value, 'executed Class source/query/support input differs')
        require(same_json(audit_session(directory, case), row['observation']), 'actual Class raw observation differs')
        for channel in ['stdout', 'stderr']:
            path = directory / 'process' / (channel + '.bin')
            require(process[channel]['sha256'] == sha(path) and process[channel]['bytes'] == path.stat().st_size, 'Class raw process channel differs')
    require(same_json(report['replay_equal'], replay_equal(output, manifest)), 'Class replay observation differs')
    require(same_json(report['results'], json.loads((output / 'partial-results.json').read_bytes())), 'Class progress records differ')
    require(same_json(report['compile'], json.loads((output / 'compile/result.json').read_bytes()))
            and report['compile']['status'] == 'executed' and type(report['compile']['exit_code']) is int
            and report['compile']['exit_code'] == 0, 'Class Java compilation provenance differs')
    return report


def run(output):
    import benchmark
    import oracle
    manifest = load_cases(FIXTURE / 'cases.json')
    output.mkdir(parents=True, exist_ok=False)
    before = {}
    for name in source_names(manifest):
        path = output / 'sources' / name
        path.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, path)
        before[name] = sha(path)
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    put(output / 'oracle.json', metadata)
    classes = output / 'classes'
    classes.mkdir()
    compiled = benchmark.capture([str(Path(metadata['java']).with_name('javac')), '-cp', metadata['artifact'], '-d', str(classes),
        str(ROOT / 'tools/java/M2ClassValuesProbe.java')], ROOT, metadata['environment'], output / 'compile', timeout=60)
    require(compiled['status'] == 'executed' and compiled['exit_code'] == 0, 'Java Class probe compile failed; attempt preserved')
    index = metadata['command'].index('-cp')
    java = [*metadata['command'][:index], '-cp', str(classes) + os.pathsep + metadata['artifact'], 'M2ClassValuesProbe']
    results = []
    for case in manifest['cases']:
        for repeat in range(2):
            directory = output / 'cases' / case['id'] / str(repeat)
            directory.mkdir(parents=True)
            for name, value in input_files(case).items():
                (directory / name).write_bytes(value)
            process = benchmark.capture([*java, str(directory), *argument_tail(case)], ROOT, metadata['environment'], directory / 'process', timeout=45)
            require(process['status'] == 'executed' and process['exit_code'] == 0, 'Java Class acquisition failed; attempt preserved')
            (directory / 'protocol.bin').write_bytes((directory / 'process/stdout.bin').read_bytes())
            observation = audit_session(directory, case)
            results.append(dict(id=case['id'], repeat=repeat, process=process, observation=observation))
            put(output / 'partial-results.json', results)
            print(case['id'], repeat, dict(Counter(observation['call_statuses'].values())), flush=True)
    report = dict(schema=1, scope=manifest['scope'], cases=manifest['cases'], repetitions=2,
        oracle=metadata, compile=compiled, sources_before=before,
        sources_after={name: sha(ROOT / name) for name in before}, results=results,
        replay_equal=replay_equal(output, manifest), compatibility_passes_claimed=0)
    report['files'] = inventory(output)
    put(output / 'report.json', report)
    return verify_report(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    modes = parser.add_mutually_exclusive_group(required=True)
    modes.add_argument('--output', type=Path)
    modes.add_argument('--verify-report', type=Path)
    args = parser.parse_args()
    report = verify_report(args.verify_report.resolve()) if args.verify_report else run(args.output.resolve())
    print(json.dumps(dict(sessions=len(report['cases']), java_processes=len(report['results']),
                         repeated_raw_equal=sum(report['replay_equal'].values()), compatibility_passes_claimed=0)))


if __name__ == '__main__':
    main()
