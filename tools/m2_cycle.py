#!/usr/bin/env python3
"""Separate cycle oracle class-initialization investigation; not compatibility."""
import argparse
import json
import os
from pathlib import Path
import subprocess

import m2_manifest as contract
import m2_language
import oracle

ROOT = contract.ROOT


def execute(output):
    metadata = oracle.ensure_oracle(ROOT)
    output.mkdir(parents=True, exist_ok=False)
    sources = ['tools/m2_cycle.py', 'tools/java/M2CycleProbe.java', 'tools/java/M2LanguageProbe.java',
               'tools/m2_language.py', 'tools/m2_manifest.py', 'tools/m2_generate.py', 'tools/oracle.py', 'oracle.lock.json',
               'references.lock', 'tests/fixtures/m2/error-cycle.pkl',
               'external/pkl/pkl-core/src/main/java/org/pkl/core/runtime/VmStackOverflowException.java',
               'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/PklRootNode.java']
    hashes = {path: oracle.sha256(ROOT / path) for path in sources}
    for path in hashes:
        destination = output / 'sources' / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes((ROOT / path).read_bytes())
    classes = output / 'classes'
    classes.mkdir()
    (output / 'oracle.json').write_bytes(contract.encoded(metadata))
    javac = Path(metadata['java']).with_name('javac')
    compiled = m2_language.capture([str(javac), '-cp', metadata['artifact'], '-d', str(classes),
            str(ROOT / 'tools/java/M2LanguageProbe.java'), str(ROOT / 'tools/java/M2CycleProbe.java')],
            output, 'javac', metadata['environment'])
    if compiled['exit_code'] != 0:
        raise ValueError('diagnostic helper compile failed')
    base = list(metadata['command'][:-1])
    base[-1] += os.pathsep + str(classes)
    results = []
    for variant, main in [('cold', 'M2LanguageProbe'), ('preinitialize', 'M2CycleProbe')]:
        for repeat in range(3):
            directory = output / f'{variant}-{repeat}'
            directory.mkdir()
            process = m2_language.capture([*base, main,
                    str(ROOT / 'tests/fixtures/m2/error-cycle.pkl'), 'result'], directory, 'java',
                    metadata['environment'])
            stderr = (directory / 'java.stderr.bin').read_bytes()
            classification = ('controlled-pkl-diagnostic' if stderr.startswith('–– Pkl Error ––\n'.encode())
                    else 'uncaught-jvm-failure')
            results.append(dict(variant=variant, repeat=repeat, classification=classification,
                                process=process, stderr_bytes=len(stderr)))
            print(f'{variant}-{repeat}: {classification}; stderr {len(stderr)} bytes', flush=True)
    report = dict(scope='Java-only cycle oracle investigation. No Rust comparison or compatibility pass.',
            execution_conditions=dict(cold='Unchanged pinned JVM options, normal M2LanguageProbe boundary.',
                    preinitialize='Same pinned JAR/JVM/environment; diagnostic wrapper calls Class.forName(VmStackOverflowException) before M2LanguageProbe.main. This explicitly changes class initialization and call stack.'),
            oracle=metadata, sources=hashes, results=results,
            source_stability=all(oracle.sha256(ROOT / path) == value for path, value in hashes.items()),
            within_variant_raw_stability={variant: len({row['process']['stderr_sha256'] for row in results
                    if row['variant'] == variant}) == 1 for variant in ('cold', 'preinitialize')})
    report['evidence_sha256'] = {str(path.relative_to(output)): oracle.sha256(path)
            for path in sorted(output.rglob('*')) if path.is_file()}
    (output / 'report.json').write_bytes(contract.encoded(report))
    return report


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    report = execute(args.output.resolve())
    print(json.dumps(report['within_variant_raw_stability'], sort_keys=True))


if __name__ == '__main__':
    main()
