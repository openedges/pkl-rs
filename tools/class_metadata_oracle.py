#!/usr/bin/env python3
"""Actual Java authority for additive pkl:base Class metadata/default controls."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import shutil

import class_values_oracle as common

ROOT = Path(__file__).resolve().parents[1]
KIND = 'pkl-java-class-metadata-completion-v1'
INPUT_ROOT = 'experiment-inputs'


def source_names(manifest):
    names = [name for name in common.source_names(manifest)
             if not name.startswith('tests/fixtures/m2-class-values/')]
    names.extend(['tools/class_metadata_oracle.py', INPUT_ROOT + '/cases.json'])
    names.extend(INPUT_ROOT + '/' + source['source'] for case in manifest['cases']
                 for source in [case, *case['support_modules']])
    return sorted(set(names))


def load_inputs(root):
    manifest = common.load_cases(root / 'cases.json', root)
    common.require(manifest.get('kind') == KIND, 'unknown Class metadata authority kind')
    return manifest


def verify_report(output):
    report = json.loads((output / 'report.json').read_bytes())
    common.require(type(report.get('schema')) is int and report['schema'] == 1
                   and report.get('kind') == KIND, 'unknown Class metadata report')
    common.require(common.same_json(report['files'], common.inventory(output)), 'metadata evidence inventory differs')
    inputs = output / 'sources' / INPUT_ROOT
    manifest = load_inputs(inputs)
    common.require(common.same_json(report['cases'], manifest['cases'])
                   and type(report['repetitions']) is int and report['repetitions'] == 2, 'metadata selection differs')
    common.require(set(report['sources_before']) == set(source_names(manifest))
                   and common.same_json(report['sources_before'], report['sources_after']), 'metadata source closure/stability differs')
    for name, value in report['sources_before'].items():
        common.require(common.sha(output / 'sources' / name) == value, 'metadata source snapshot differs')
    common.require({name.removeprefix('sources/') for name in report['files'] if name.startswith('sources/')}
                   == set(report['sources_before']), 'extra/missing metadata source snapshot')
    metadata = json.loads((output / 'oracle.json').read_bytes())
    common.require(common.same_json(metadata, report['oracle']) and metadata['version'] == '0.32.0'
                   and metadata['source']['revision'] == common.REVISION
                   and metadata['artifact_sha256'] == common.JAR_SHA256
                   and common.sha(output / 'sources/oracle.lock.json') == metadata['lock_sha256'], 'metadata Java pin differs')
    expected = [(case['id'], repeat) for case in manifest['cases'] for repeat in range(2)]
    common.require([(row['id'], row['repeat']) for row in report['results']] == expected
                   and all(type(row['repeat']) is int for row in report['results']), 'metadata process coverage differs')
    for row in report['results']:
        case = next(case for case in manifest['cases'] if case['id'] == row['id'])
        directory = output / 'cases' / row['id'] / str(row['repeat'])
        process = json.loads((directory / 'process/result.json').read_bytes())
        common.require(common.same_json(process, row['process']) and process['status'] == 'executed'
                       and type(process['exit_code']) is int and process['exit_code'] == 0, 'metadata JVM did not complete')
        index = metadata['command'].index('-cp')
        common.require(process['command'][:index] == metadata['command'][:index]
                       and process['command'][index] == '-cp'
                       and process['command'][index + 1].endswith(os.pathsep + metadata['artifact'])
                       and process['command'][index + 2] == 'M2ClassValuesProbe'
                       and process['command'][-5:] == common.argument_tail(case), 'metadata Java driver arguments differ')
        for name, value in common.input_files(case, inputs).items():
            common.require((directory / name).read_bytes() == value, 'metadata executed input differs')
        common.require(common.same_json(common.audit_session(directory, case), row['observation']), 'metadata raw observation differs')
        for channel in ['stdout', 'stderr']:
            path = directory / 'process' / (channel + '.bin')
            common.require(process[channel]['sha256'] == common.sha(path)
                           and process[channel]['bytes'] == path.stat().st_size, 'metadata process payload differs')
    common.require(common.same_json(report['replay_equal'], common.replay_equal(output, manifest)), 'metadata replay differs')
    common.require(common.same_json(report['results'], json.loads((output / 'partial-results.json').read_bytes())), 'metadata progress differs')
    compiled = json.loads((output / 'compile/result.json').read_bytes())
    common.require(common.same_json(report['compile'], compiled) and compiled['status'] == 'executed'
                   and type(compiled['exit_code']) is int and compiled['exit_code'] == 0, 'metadata compilation differs')
    common.require(report['compatibility_passes_claimed'] == 0, 'Java acquisition claims candidate compatibility')
    return report


def run(output, inputs):
    import benchmark
    import oracle
    manifest = load_inputs(inputs)
    output.mkdir(parents=True, exist_ok=False)
    before, originals = {}, {}
    for name in source_names(manifest):
        original = inputs / name.removeprefix(INPUT_ROOT + '/') if name.startswith(INPUT_ROOT + '/') else ROOT / name
        destination = output / 'sources' / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(original, destination)
        before[name], originals[name] = common.sha(destination), original
    metadata = oracle.ensure_oracle(ROOT, fetch=False)
    common.put(output / 'oracle.json', metadata)
    classes = output / 'classes'
    classes.mkdir()
    compiled = benchmark.capture([str(Path(metadata['java']).with_name('javac')), '-cp', metadata['artifact'], '-d', str(classes),
        str(ROOT / 'tools/java/M2ClassValuesProbe.java')], ROOT, metadata['environment'], output / 'compile', timeout=60)
    common.require(compiled['status'] == 'executed' and compiled['exit_code'] == 0, 'metadata compile failed; attempt retained')
    index = metadata['command'].index('-cp')
    java = [*metadata['command'][:index], '-cp', str(classes) + os.pathsep + metadata['artifact'], 'M2ClassValuesProbe']
    results = []
    for case in manifest['cases']:
        for repeat in range(2):
            directory = output / 'cases' / case['id'] / str(repeat)
            directory.mkdir(parents=True)
            for name, value in common.input_files(case, inputs).items():
                (directory / name).write_bytes(value)
            process = benchmark.capture([*java, str(directory), *common.argument_tail(case)], ROOT, metadata['environment'], directory / 'process', timeout=45)
            common.require(process['status'] == 'executed' and process['exit_code'] == 0, 'metadata JVM failed; attempt retained')
            (directory / 'protocol.bin').write_bytes((directory / 'process/stdout.bin').read_bytes())
            observation = common.audit_session(directory, case)
            results.append(dict(id=case['id'], repeat=repeat, process=process, observation=observation))
            common.put(output / 'partial-results.json', results)
            print(case['id'], repeat, dict(Counter(observation['call_statuses'].values())), flush=True)
    report = dict(schema=1, kind=KIND, scope=manifest['scope'], cases=manifest['cases'], repetitions=2,
        oracle=metadata, compile=compiled, sources_before=before,
        sources_after={name: common.sha(original) for name, original in originals.items()}, results=results,
        replay_equal=common.replay_equal(output, manifest), compatibility_passes_claimed=0)
    report['files'] = common.inventory(output)
    common.put(output / 'report.json', report)
    return verify_report(output)


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    group = parser.add_mutually_exclusive_group(required=True)
    group.add_argument('--output', type=Path)
    group.add_argument('--verify-report', type=Path)
    parser.add_argument('--inputs', type=Path)
    args = parser.parse_args()
    if args.output and not args.inputs:
        parser.error('--output requires --inputs')
    report = run(args.output.resolve(), args.inputs.resolve()) if args.output else verify_report(args.verify_report.resolve())
    print(json.dumps(dict(sessions=len(report['cases']), java_processes=len(report['results']),
        repeated_raw_equal=sum(report['replay_equal'].values()), compatibility_passes_claimed=0)))
