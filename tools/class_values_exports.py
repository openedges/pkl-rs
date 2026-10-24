#!/usr/bin/env python3
"""Java-only public exported Class identity supplement; no candidate API claim."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import shutil

import class_values_oracle as main

ROOT, FIXTURE = main.ROOT, main.FIXTURE
require, put, sha, inventory = main.require, main.put, main.sha, main.inventory
CALLS = ['export-0', 'export-1', 'export-2', 'export-3', 'export-4', 'marker-0', 'post-close-0']
IDENTITIES = ['same-uri', 'same-uri-changed-source', 'different-uri', 'different-evaluator']


def load_cases(fixture=FIXTURE):
    data = json.loads((fixture / 'export-cases.json').read_bytes())
    require(type(data['schema']) is int and data['schema'] == 1 and data['baseline'] == 'Pkl 0.32.0'
            and type(data['repetitions']) is int and data['repetitions'] == 2 and len(data['cases']) == 1,
            'invalid public-export supplement selection')
    case = data['cases'][0]
    require(case['id'] == 'public-export-identity' and case['uri'] != case['other_uri'], 'public-export URI identity differs')
    require(set(case['sources']) == {'export-first', 'export-changed'}, 'export input coverage differs')
    for value in case['sources'].values():
        require(value['source'].startswith('inputs/') and '..' not in Path(value['source']).parts
                and sha(fixture / value['source']) == value['source_sha256'], 'export authored source differs')
    return data


def source_names():
    base = 'external/pkl/pkl-core/src/main/java/org/pkl/core/'
    return sorted(['oracle.lock.json', 'references.lock', 'tools/class_values_exports.py',
        'tools/class_values_oracle.py', 'tools/frame_transformer_oracle.py', 'tools/benchmark.py', 'tools/oracle.py',
        'tools/java/M2ClassExportProbe.java', 'tests/fixtures/m2-class-values/export-cases.json',
        'tests/fixtures/m2-class-values/inputs/export-first.pkl', 'tests/fixtures/m2-class-values/inputs/export-changed.pkl',
        *[base + name for name in ['PClass.java', 'PClassInfo.java', 'Evaluator.java', 'EvaluatorImpl.java',
                                   'runtime/VmClass.java', 'runtime/ModuleCache.java', 'ast/member/ClassNode.java']]])


def audit(directory):
    raw = main.payloads(directory)
    expected = {'protocol.bin'}
    protocol = (directory / 'process/stdout.bin').read_bytes()
    require(raw['protocol.bin'] == protocol and (directory / 'process/stderr.bin').read_bytes() == b'', 'public-export raw process protocol differs')
    statuses = {}
    for prefix in CALLS:
        for channel in ['stdout', 'stderr']:
            main.wire.check_string(raw, prefix + '.' + channel)
        status = raw[prefix + '.status.bin']
        require(status in main.STATUSES, 'invalid public-export status')
        statuses[prefix] = status.decode().removesuffix('\n')
        require(protocol.count(f'call-enter={prefix}\n'.encode()) == 1
                and protocol.count(f'call-exit={prefix} status={statuses[prefix]}\n'.encode()) == 1, 'missing public-export call protocol')
        expected.update(prefix + suffix for suffix in ['.stdout.bin', '.stdout.code-units.bin', '.stderr.bin', '.stderr.code-units.bin', '.status.bin', '.error-class.bin', '.java-value-type.bin'])
        if status == b'success\n':
            require(raw[prefix + '.error-class.bin'] == b'' and raw[prefix + '.stderr.bin'] == b''
                    and raw[prefix + '.java-value-type.bin'], 'export success/error/value metadata differs')
        else:
            require(raw[prefix + '.error-class.bin'] and raw[prefix + '.stdout.bin'] == b''
                    and raw[prefix + '.java-value-type.bin'] == b'', 'failed export provenance differs')
    identities = {}
    for name in IDENTITIES:
        values = {}
        for kind in ['reference', 'equals']:
            prefix = name + '.' + kind
            main.wire.check_string(raw, prefix)
            require(raw[prefix + '.bin'] in {b'true', b'false', b'unavailable'}, 'invalid public exported identity observation')
            values[kind] = raw[prefix + '.bin'].decode()
            expected.update([prefix + '.bin', prefix + '.code-units.bin'])
        require(protocol.count(f'identity={name} reference={values["reference"]} equals={values["equals"]}\n'.encode()) == 1, 'export identity protocol differs')
        identities[name] = values
    require(set(raw) == expected and protocol.endswith(b'closed public-calls=7 identity-comparisons=4\n'), 'export channel/terminal coverage differs')
    return dict(call_statuses=statuses, public_calls=7, identity_comparisons=identities,
                payloads={name: dict(sha256=main.digest(data), bytes=len(data)) for name, data in raw.items()})


def verify_report(output):
    report = json.loads((output / 'report.json').read_bytes())
    require(main.same_json(report['files'], inventory(output)), 'public-export evidence inventory differs')
    fixture = output / 'sources/tests/fixtures/m2-class-values'
    data = load_cases(fixture)
    require(main.same_json(report['cases'], data['cases']) and report['kind'] == 'java-only-public-export', 'public-export authority scope differs')
    require(set(report['sources_before']) == set(source_names()) and main.same_json(report['sources_before'], report['sources_after']), 'public-export source closure/stability differs')
    for name, digest in report['sources_before'].items():
        require(sha(output / 'sources' / name) == digest, 'public-export captured source differs')
    metadata = json.loads((output / 'oracle.json').read_bytes())
    require(main.same_json(metadata, report['oracle']) and metadata['version'] == '0.32.0'
            and metadata['source']['revision'] == main.REVISION and metadata['artifact_sha256'] == main.JAR_SHA256
            and sha(output / 'sources/oracle.lock.json') == metadata['lock_sha256'], 'public-export Java pin differs')
    require([(row['id'], row['repeat']) for row in report['results']] == [('public-export-identity', 0), ('public-export-identity', 1)], 'public-export process coverage differs')
    case = data['cases'][0]
    all_raw = []
    for row in report['results']:
        directory = output / 'cases' / row['id'] / str(row['repeat'])
        process = json.loads((directory / 'process/result.json').read_bytes())
        require(main.same_json(process, row['process']) and process['status'] == 'executed'
                and type(process['exit_code']) is int and process['exit_code'] == 0, 'public-export process did not complete')
        require(process['command'][-2:] == [case['uri'], case['other_uri']] and 'M2ClassExportProbe' in process['command'], 'public-export driver arguments differ')
        for name, key in [('first.pkl', 'export-first'), ('changed.pkl', 'export-changed')]:
            require((directory / name).read_bytes() == (fixture / case['sources'][key]['source']).read_bytes(), 'executed public-export input differs')
        require(main.same_json(row['observation'], audit(directory)), 'public-export raw observation differs')
        for channel in ['stdout', 'stderr']:
            path = directory / 'process' / (channel + '.bin')
            require(process[channel]['sha256'] == sha(path) and process[channel]['bytes'] == path.stat().st_size, 'public-export process channel differs')
        all_raw.append(main.payloads(directory))
    require(type(report['replay_equal']) is bool and report['replay_equal'] == (all_raw[0] == all_raw[1]), 'public-export replay differs')
    require(main.same_json(report['compile'], json.loads((output / 'compile/result.json').read_bytes()))
            and report['compile']['status'] == 'executed' and type(report['compile']['exit_code']) is int
            and report['compile']['exit_code'] == 0, 'public-export compiler provenance differs')
    return report


def run(output):
    import benchmark
    import oracle
    data = load_cases()
    output.mkdir(parents=True, exist_ok=False)
    before = {}
    for name in source_names():
        target = output / 'sources' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
        before[name] = sha(target)
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    put(output / 'oracle.json', metadata)
    classes = output / 'classes'; classes.mkdir()
    compiled = benchmark.capture([str(Path(metadata['java']).with_name('javac')), '-cp', metadata['artifact'], '-d', str(classes),
        str(ROOT / 'tools/java/M2ClassExportProbe.java')], ROOT, metadata['environment'], output / 'compile', timeout=60)
    require(compiled['status'] == 'executed' and compiled['exit_code'] == 0, 'public-export Java compile failed; attempt preserved')
    index = metadata['command'].index('-cp')
    command = [*metadata['command'][:index], '-cp', str(classes) + os.pathsep + metadata['artifact'], 'M2ClassExportProbe']
    results = []
    case = data['cases'][0]
    for repeat in range(2):
        directory = output / 'cases' / case['id'] / str(repeat); directory.mkdir(parents=True)
        for name, key in [('first.pkl', 'export-first'), ('changed.pkl', 'export-changed')]:
            shutil.copyfile(FIXTURE / case['sources'][key]['source'], directory / name)
        process = benchmark.capture([*command, str(directory), case['uri'], case['other_uri']], ROOT,
                                    metadata['environment'], directory / 'process', timeout=45)
        require(process['status'] == 'executed' and process['exit_code'] == 0, 'public-export Java acquisition failed; attempt preserved')
        (directory / 'protocol.bin').write_bytes((directory / 'process/stdout.bin').read_bytes())
        observation = audit(directory)
        results.append(dict(id=case['id'], repeat=repeat, process=process, observation=observation))
        print(repeat, observation['identity_comparisons'], dict(Counter(observation['call_statuses'].values())), flush=True)
    report = dict(schema=1, kind='java-only-public-export', scope=data['scope'], cases=data['cases'], repetitions=2,
                  oracle=metadata, compile=compiled, sources_before=before, sources_after={name: sha(ROOT / name) for name in before},
                  results=results, replay_equal=main.payloads(output / 'cases' / case['id'] / '0') == main.payloads(output / 'cases' / case['id'] / '1'),
                  compatibility_passes_claimed=0)
    report['files'] = inventory(output); put(output / 'report.json', report)
    return verify_report(output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--output', type=Path); group.add_argument('--verify-report', type=Path)
    args = parser.parse_args()
    report = verify_report(args.verify_report.resolve()) if args.verify_report else run(args.output.resolve())
    print(json.dumps(dict(java_processes=len(report['results']), public_calls=14, candidate_export_API='unexecuted/required')))
