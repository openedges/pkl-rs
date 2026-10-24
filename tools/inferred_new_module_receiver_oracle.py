"""Capture order-dependent inherited `new module` Java 0.32.0 behavior."""
from __future__ import annotations

from collections import Counter
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import time
import argparse

from inferred_new_oracle import SOURCES as PARENT_SOURCES

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-inferred-new-module-receiver'
REVISION = 'e51fb2f91774265666f94980ec630db794a4db2d'
EXTRA_SOURCES = (
    'tools/inferred_new_module_receiver_oracle.py',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/UnresolvedTypeNode.java',
    *('external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/types/' + name + '.pkl'
      for name in ('currentModuleType1', 'currentModuleType2', 'moduleType2',
                   'moduleType3', 'moduleType4', 'moduleType5')),
)
SOURCES = tuple(sorted(set(PARENT_SOURCES) | set(EXTRA_SOURCES)))
INPUT_HASHES = {
    'alpha-first.queries.tsv': '438a20cdc23b46e7b756b919767db4b85909825a9e55c8b8e96edc0b39608f4d',
    'alpha.pkl': 'b96b50766b7201465022ca3347ddd9ff4b7d1984e640f69722d1f21193a8376c',
    'base.pkl': '2a351692adc9cbea16e7e8d27248223aa1bd088823f7d807d08a03b9862ff680',
    'beta-first.queries.tsv': '41f60ce23b6046251f20e6cde5a5adaad71c5b6d6701007f6969f5ab2d4c0c32',
    'beta.pkl': 'a64a0c145bbbc955ec2ba3a892dc5a6a7217f5b993f03acd5229567f2aed03cc',
    'cases.json': 'b099d35a56a0fb40ef613f3464db9059cd35f5edbc843d42cad14fc717e8f780',
    'module.pkl': '2f63ea3af3b838456b1a55e6c97e33655aefd9fa4b670401e382894b91852c5b',
}


def sha(path: Path) -> str:
    h = hashlib.sha256()
    with path.open('rb') as stream:
        for part in iter(lambda: stream.read(1 << 20), b''):
            h.update(part)
    return h.hexdigest()


def file_map(folder: Path) -> dict:
    return {p.relative_to(folder).as_posix(): sha(p)
            for p in sorted(folder.rglob('*')) if p.is_file()}


def save(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def acquire(output: Path) -> None:
    output = output.resolve()
    if output.exists():
        raise ValueError('fresh module-receiver Java output required')
    for name, expected in INPUT_HASHES.items():
        if sha(FIXTURE / name) != expected:
            raise ValueError(f'authored module-receiver input differs: {name}')
    cases = json.loads((FIXTURE / 'cases.json').read_bytes())
    if (cases['schema'] != 1 or cases['baseline'] != 'Pkl 0.32.0'
            or cases['boundary'] != 'public-evaluateExpressionString'
            or cases['repetitions'] != 2
            or cases['module_uri_policy'] != 'captured-absolute-file-uri-with-three-sibling-modules'
            or set(cases['scenarios']) != {'alpha-first', 'beta-first'}):
        raise ValueError('authored scenario boundary differs')
    for scenario, rows in cases['scenarios'].items():
        if (len(rows) != 11 or len({r['id'] for r in rows}) != 11
                or any(r['mode'] != 'string' for r in rows)
                or (FIXTURE / (scenario + '.queries.tsv')).read_text() != ''.join(
                    f"string\t{r['id']}\t{r['expression']}\n" for r in rows)):
            raise ValueError(f'authored scenario rows differ: {scenario}')
    lock = json.loads((ROOT / 'oracle.lock.json').read_bytes())
    jar = ROOT / '.cache/oracle' / lock['artifact']['filename']
    if (lock['version'] != '0.32.0' or lock['source']['revision'] != REVISION
            or sha(jar) != lock['artifact']['sha256']):
        raise ValueError('pinned Java/JAR differs')
    source_before = {name: sha(ROOT / name) for name in SOURCES}
    output.mkdir(parents=True)
    shutil.copyfile(Path(__file__), output / 'run.py')
    shutil.copyfile(ROOT / 'tools/java/CollectionConversionsOracle.java',
                    output / 'CollectionConversionsOracle.java')
    for name in INPUT_HASHES:
        shutil.copyfile(FIXTURE / name, output / name)
    for name in SOURCES:
        target = output / 'sources' / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(ROOT / name, target)
    if {name: sha(output / 'sources' / name) for name in SOURCES} != source_before:
        raise ValueError('source copy differs')
    java = Path(os.environ.get('PKL_ORACLE_JAVA', '/usr/bin/java')).resolve()
    env = {'PATH': os.defpath, 'HOME': str(ROOT / '.cache/oracle/home'),
           'TMPDIR': str(ROOT / '.cache/oracle/tmp'), 'TMP': str(ROOT / '.cache/oracle/tmp'),
           'TEMP': str(ROOT / '.cache/oracle/tmp'), 'LC_ALL': 'C', 'LANG': 'C',
           'TZ': 'UTC', 'TERM': 'dumb', 'NO_COLOR': '1', 'COLUMNS': '80'}
    ident = subprocess.run([str(java), '-XshowSettings:properties', '-version'],
                           cwd=output, env=env, capture_output=True, timeout=60)
    (output / 'java-identity.stdout.bin').write_bytes(ident.stdout)
    (output / 'java-identity.stderr.bin').write_bytes(ident.stderr)
    properties = dict(re.findall(r'^\s+([\w.]+) = (.*)$', ident.stderr.decode(), re.M))
    if ident.returncode or any(properties.get(k) != v for k,v in lock['jdk']['properties'].items()):
        raise ValueError('pinned JDK identity differs; raw streams retained')
    home = Path(properties['java.home'])
    jdk = {'executable': str(java), 'executable_sha256': sha(java),
           'modules_sha256': sha(home / 'lib/modules'),
           'release_sha256': sha(home / 'release'), 'properties': properties}
    options = [*lock['jvm_options'], f"-Duser.home={env['HOME']}",
               f"-Djava.io.tmpdir={env['TMPDIR']}"]
    report = {'schema': 1, 'kind': 'stage65-Java-inherited-module-receiver',
              'scope': 'Public string authority for two order-specific imported module scenarios',
              'candidate_compatibility_passes_claimed': 0,
              'host': platform.platform(),
              'input_before': {name: sha(FIXTURE / name) for name in INPUT_HASHES},
              'source_before': source_before, 'artifact_before': sha(jar),
              'oracle_lock_sha256': sha(ROOT / 'oracle.lock.json'),
              'references_lock_sha256': sha(ROOT / 'references.lock'),
              'jdk': jdk, 'environment': env, 'sessions': [], 'complete': False}
    save(output / 'report.json', report)
    statuses = Counter()
    observations = {}
    for scenario in ('alpha-first', 'beta-first'):
        source = output / scenario
        source.mkdir()
        for name in ('base.pkl', 'alpha.pkl', 'beta.pkl', 'module.pkl'):
            shutil.copyfile(output / name, source / name)
        shutil.copyfile(output / (scenario + '.queries.tsv'), source / 'queries.tsv')
        uri = (source / 'module.pkl').as_uri()
        rows = cases['scenarios'][scenario]
        for repeat in range(2):
            folder = source / f'repeat-{repeat}'
            folder.mkdir()
            argv = [str(java), *options, '-cp', str(jar),
                    str(output / 'CollectionConversionsOracle.java'),
                    str(folder), str(source), uri]
            started = time.time()
            process = subprocess.Popen(argv, cwd=output, env=env,
                                       stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            timed_out = False
            try:
                stdout, stderr = process.communicate(timeout=240)
            except subprocess.TimeoutExpired:
                timed_out = True
                process.kill()
                stdout, stderr = process.communicate()
            (folder / 'process.stdout.bin').write_bytes(stdout)
            (folder / 'process.stderr.bin').write_bytes(stderr)
            session = {'scenario': scenario, 'repeat': repeat, 'pid': process.pid,
                       'argv': argv, 'cwd': str(output), 'module_uri': uri,
                       'started_epoch': started, 'finished_epoch': time.time(),
                       'exit_code': process.returncode, 'timed_out': timed_out,
                       'stdout_sha256': sha(folder / 'process.stdout.bin'),
                       'stderr_sha256': sha(folder / 'process.stderr.bin'),
                       'stdout_bytes': len(stdout), 'stderr_bytes': len(stderr),
                       'row_directories': sum(p.is_dir() for p in folder.iterdir())}
            report['sessions'].append(session)
            save(output / 'report.json', report)
            print(json.dumps({'scenario': scenario, 'repeat': repeat, 'pid': process.pid,
                              'exit_code': process.returncode, 'rows': session['row_directories']}),
                  flush=True)
            if timed_out or process.returncode or session['row_directories'] != len(rows):
                raise ValueError('Java session incomplete; raw attempt retained')
        report.setdefault('raw_equal', {})[scenario] = (
            file_map(source / 'repeat-0') == file_map(source / 'repeat-1'))
        for index, row in enumerate(rows):
            item = source / 'repeat-0' / f"{index:03d}-{row['id']}"
            status = (item / 'result.status.bin').read_text().strip()
            stderr = (item / 'result.stderr.bin').read_text()
            statuses[status] += 1
            observations.setdefault(scenario, []).append({
                'id': row['id'], 'expression': row['expression'], 'status': status,
                'stdout': (item / 'result.stdout.bin').read_text(),
                'first_error_line': stderr.splitlines()[1] if len(stderr.splitlines()) > 1 else '',
                'stdout_sha256': sha(item / 'result.stdout.bin'),
                'stderr_sha256': sha(item / 'result.stderr.bin')})
        save(output / 'report.json', report)
    report['statuses_per_repetition'] = dict(statuses)
    report['observations'] = observations
    report['input_after'] = {name: sha(FIXTURE / name) for name in INPUT_HASHES}
    report['source_after'] = {name: sha(ROOT / name) for name in SOURCES}
    report['source_copies_after'] = {name: sha(output / 'sources' / name) for name in SOURCES}
    report['artifact_after'] = sha(jar)
    report['source_stable'] = (report['source_before'] == report['source_after']
                               == report['source_copies_after'])
    report['input_stable'] = report['input_before'] == report['input_after']
    report['complete'] = (report['source_stable'] and report['input_stable']
                          and report['artifact_before'] == report['artifact_after']
                          and len(report['sessions']) == 4
                          and len({s['pid'] for s in report['sessions']}) == 4
                          and all(report['raw_equal'].values())
                          and not statuses['oracle-error'] and not statuses['api-exception'])
    save(output / 'report.json', report)
    if not report['complete']:
        raise ValueError('Java capture incomplete; raw attempt retained')
    print(json.dumps({'complete': True, 'statuses_per_repetition': dict(statuses)},
                     sort_keys=True))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    acquire(args.output)
