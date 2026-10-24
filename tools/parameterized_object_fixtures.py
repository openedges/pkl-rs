"""Verify the independently pinned Java parameterized object-body observations."""
from pathlib import Path, PurePosixPath
import argparse
import hashlib
import json
import os
import sys
from urllib.parse import quote


ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'tests/fixtures/m2-parameterized-object-bodies'
# Reviewed independent Java authority; updating fixture metadata cannot replace it.
AUTHORITY = {
    "historical-observations.json": "14d6e4adceffba05aa88c90fd8f4402fd6a6fa623c02bfa567b8f900dc6be3b9",
    "original-cli/case.json": "ac6581063413fee29b4e17fcebcc4fdd2909e3df261b603b761abb24a672483e",
    "original-cli/report.json": "0071e39e6e2710b5c89302fcc0d47244b2b108128776743f215b84cd88171241",
    "provenance/M2LanguageProbe.java": "50be55d31f3efac3ec3627774ca7b6a02c0226fb40c499f04013acb838ed65c7",
    "provenance/artifacts/java/M2LanguageProbe.class": "a08710944539e8b1663cf9e6ecddbfec515f911ea153e718e35deeeb19d3f751",
    "provenance/cohorts.json": "a29ad6f287c815a17848a0114e051b0ffaf9328832d97c54048fa8adb24c67bc",
    "provenance/oracle.json": "1aa7870de1acd14941c076e111996889fb75006eec5b475703f7f77dfc67b2d7",
    "provenance/oracle.lock.json": "7b77395ff5b64f7a220db74975e4c3858a36b19c5c0f5bca92681e3b1bc6504b",
    "provenance/original-cases/03.json": "9e3dac5356683871d18ab1c35bc3bab0952ed5371ccfa9f1e8ae77a62703c6e1",
    "provenance/original-cases/04.json": "1e6112dd5c56ca6729ec6153ea97597de7e8a3e3068945684b594d1fb507c3a6",
    "provenance/original-cases/05.json": "9c686b16616509b93d40a234e02a7debc7d5cc831dbb5cbc869406b80401188c",
    "provenance/original-cases/06.json": "b81daf28e86e0ff67b9180d3f52a1b19b6ae46534d85ef1f58a96f313498338a",
    "provenance/original-cases/07.json": "f03ccb8b69abaab6ca84c9afb33a9c75a24e3cfead33db670c2c976f9815221d",
    "provenance/original-cases/08.json": "e2eb657819d159ac1e344d6dc69bd0facd0c45199207d8e91a4c6c7e4dd127af",
    "provenance/original-reports/03.json": "8cb571d3e768abfe9c0f5c0c8943997e0c3052fead74f50c788414b4ff4c2501",
    "provenance/original-reports/04.json": "bbccc7db11ad9e6b3f948b7f295de10a16f25caa49615c0cf8bc4b6f7d0607a4",
    "provenance/original-reports/05.json": "6ea2eefa9b4c6d9fc78adba988cc09360f883a256d90702374787c42e93e482c",
    "provenance/original-reports/06.json": "3d1f5f1767422f257984efb307ae91e4b1de0ecdbd17845e07368ee415564381",
    "provenance/original-reports/07.json": "129700a085561095f7c8a8ce0857bb98258dd9354261ab3d626393f084179eb6",
    "provenance/original-reports/08.json": "1650c2d85f2dd5f176e62fe1be54d3d8e5c72573cf623648396aebf38bbe124a",
    "provenance/references.lock": "278f48fd7697459281aa78a706c9ca57222f36a8c0785d838a6449fbaf26d30b",
    "provenance/original-reports/09.json": "e3e9a074d767928c718dab686098f2b4b62fdb0cfbada40be1503dab5299c538",
    "provenance/original-cases/09.json": "50bf6804271d4659bff03a9bd14ffebc0c0106fbfb68cd6a832c0fde9fd0e0be",
    "provenance/original-reports/10.json": "72623962fbf7c978bcdc46a3f6a884e44c7cdc3c6a61f2483c5c9d6d2959d0e9",
    "provenance/original-cases/10.json": "9df5754c5d778991e04cd2c2d187d65c29f417806bef1b6d232d076ba497dc55",
    "provenance/original-reports/11.json": "5fed43be1a9b81d93eca80bda152dd903576609981d62e743ff4b32956d6e703",
    "provenance/original-cases/11.json": "6d6d79aeaddb5580cf2a796b931b08f349129c4c895908a8a32fdd13919713ca"
}
FILE_SET_SHA256 = '0b2b04e4ea6f17521a88ff38dd2f1cc4617bd68cb24e4700864fe90597b4cf3a'
COHORTS = ('03', '04', '05', '06', '07', '08', '09', '10', '11')


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(condition, message):
    if not condition:
        raise ValueError(message)


def safe(root, name):
    path = PurePosixPath(name)
    require(isinstance(name, str) and str(path) == name and not path.is_absolute()
            and '..' not in path.parts, 'unsafe fixture path')
    candidate = root.joinpath(*path.parts)
    require(not candidate.is_symlink() and candidate.resolve().is_relative_to(root.resolve()),
            'unsafe fixture link')
    return candidate


def actual_files(root):
    require(not root.is_symlink(), 'fixture root is a symlink')
    found = set()
    pending = [root]
    while pending:
        directory = pending.pop()
        with os.scandir(directory) as entries:
            for entry in entries:
                require(not entry.is_symlink(), 'fixture contains a symlink')
                path = Path(entry.path)
                if entry.is_dir(follow_symlinks=False):
                    pending.append(path)
                else:
                    require(entry.is_file(follow_symlinks=False), 'nonregular fixture entry')
                    found.add(path.relative_to(root).as_posix())
    return found


def selected(number, identifier):
    if number == '03':
        return not identifier.startswith('lexical-')
    if number == '04':
        return identifier.startswith('eager-scope-') or identifier in ('lexical-00', 'lexical-01', 'lexical-02')
    if number == '05':
        return identifier != 'read-constraint-02'
    return number in ('06', '07', '09', '10', '11')


def boundary(number, identifier):
    return (number == '08' or (number == '03' and identifier in ('dispatch-01', 'dispatch-09'))
            or (number == '04' and identifier in ('lexical-03', 'lexical-04')))


def verify(root=DEFAULT):
    root = Path(root)

    def raw(name):
        return safe(root, name).read_bytes()

    def read(name):
        return json.loads(raw(name))

    def reference(record):
        require(set(record) == {'path', 'sha256', 'bytes'}, 'invalid raw reference')
        data = raw(record['path'])
        require(digest(data) == record['sha256'] and len(data) == record['bytes'],
                'raw reference identity differs')
        return data

    manifest = read('manifest.json')
    require(manifest['schema'] == 1 and manifest['baseline'] == 'Pkl 0.32.0',
            'fixture baseline differs')
    files = manifest['files']
    actual = actual_files(root)
    require(actual == set(files) | {'manifest.json'}
            and digest('\n'.join(sorted(files)).encode()) == FILE_SET_SHA256,
            'fixture file coverage differs')
    for name, record in files.items():
        data = raw(name)
        require(digest(data) == record['sha256'] and len(data) == record['bytes'],
                'fixture bytes differ: ' + name)
    for name, pin in AUTHORITY.items():
        require(digest(raw(name)) == pin, 'independent Java authority differs: ' + name)

    oracle = read('provenance/oracle.json')
    require(oracle['version'] == '0.32.0'
            and oracle['source']['revision'] == 'e51fb2f91774265666f94980ec630db794a4db2d'
            and oracle['artifact_sha256'] == 'c3b6f6215a08b31453b0fb0edd8c9da0aee31233f65abb8ab07f9c945151e059',
            'oracle baseline provenance differs')
    reports = {number: read('provenance/original-reports/' + number + '.json') for number in COHORTS}
    expected = []
    internal = []
    index = {}
    for number, report in reports.items():
        require(report['baseline'] == 'Pkl 0.32.0' and report['input_files_unchanged']
                and report['input_files_before'] == report['input_files_after']
                and report['environment'] == oracle['environment'], 'Java input/environment drift')
        originals = read('provenance/original-cases/' + number + '.json')
        require([(c['id'], c['source_sha256'], c['expression']) for c in originals]
                == [(c['id'], c['source_sha256'], c['expression']) for c in report['observations']],
                'original Java case coverage differs')
        for original in report['observations']:
            key = (number, original['id'])
            require(key not in index, 'duplicate Java observation')
            index[key] = original
            if boundary(*key):
                internal.append(key)
            elif selected(*key):
                expected.append(key)

    cases = read('cases.json')
    boundaries = read('oracle-boundaries.json')
    require([(c['cohort'], c['original_id']) for c in cases] == expected,
            'ordinary Java selection differs')
    require([(c['cohort'], c['original_id']) for c in boundaries] == internal,
            'Java internal boundary selection differs')
    for row in cases + boundaries:
        key = (row['cohort'], row['original_id'])
        original = index[key]
        is_internal = boundary(*key)
        source = PurePosixPath(original['source'])
        identity = 'stage53-' + row['cohort'] + '-' + original['id']
        require(row['id'] == identity and row['source'] == 'inputs/' + row['cohort'] + '/' + source.name
                and row['source_uri'] == 'file://' + quote(str(source)) and row['module_name'] == source.stem
                and row['expression'] == original['expression'] and row['power_assertions'] is False
                and row['classification'] == ('oracle-error' if is_internal else 'ordinary'),
                'source/expression/classification differs from Java')
        require(digest(raw(row['source'])) == row['source_sha256'] == original['source_sha256'],
                'source bytes differ from Java')
        require(row['java_repeats'] == 2 and row['replay_equal'] == original['replay_equal']
                and (is_internal or row['replay_equal']), 'Java repeat contract differs')
        require(len(row['observations']) == len(original['observations']) == 2,
                'missing fresh Java process')
        for observed, captured in zip(row['observations'], original['observations'], strict=True):
            process_path = 'provenance/processes/' + identity + '/repeat-' + str(captured['repeat']) + '.json'
            reference(observed['process'])
            require(observed['process']['path'] == process_path and read(process_path) == captured['process'],
                    'exact Java process differs')
            process = captured['process']
            require(observed['repeat'] == captured['repeat'] and observed['exit_code'] == process['exit_code']
                    and observed['timed_out'] == process['timed_out']
                    and observed['public_generated_files'] == {} and observed['input_tree_unchanged'],
                    'Java status/files differ')
            prefix = 'raw/' + identity + ('/repeat-' + str(captured['repeat']) if is_internal else '')
            require(set(observed['channels']) == {'stdout', 'stderr', 'outcome'},
                    'raw channel coverage differs')
            for channel in ('stdout', 'stderr', 'outcome'):
                record = observed['channels'][channel]
                require(record['path'] == prefix + '/' + channel + '.bin', 'raw channel path differs')
                payload = reference(record)
                authority_hash = process[channel + '_sha256']
                require(digest(payload) == record['sha256'] == authority_hash
                        and len(payload) == record['bytes'], 'raw Java channel differs')
            outcome = raw(observed['channels']['outcome']['path'])
            require(outcome == captured['outcome'].encode(), 'Java exception outcome differs')
            if not is_internal:
                require(not process['timed_out'] and (
                    (process['exit_code'] == 0 and outcome == b'success\n')
                    or (process['exit_code'] == 1 and outcome == b'pkl-exception\torg.pkl.core.PklException\n')),
                    'internal or unexecuted Java outcome cannot be ordinary')
                other = 'stderr' if process['exit_code'] == 0 else 'stdout'
                require(raw(observed['channels'][other]['path']) == b'', 'unexpected Java output channel')
        needs_host = source.name == 'read-constraint.pkl'
        require(row['requires_host'] == needs_host, 'Host requirement differs')
        resources = []
        if needs_host:
            name = 'inputs/' + row['cohort'] + '/gate.txt'
            pin = reports[row['cohort']]['input_files_before']['gate.txt']['sha256']
            require(digest(raw(name)) == pin, 'Java resource bytes differ')
            resources = [dict(source=name, source_uri='file://' + quote(str(source.parent / 'gate.txt')), sha256=pin)]
        require(row['resources'] == resources, 'Java resource identity differs')

    original = read('original-cli/case.json')
    cli = read('original-cli/report.json')
    require(digest(raw('original-cli/input.pkl')) == original['input_sha256'] == cli['input_sha256'],
            'original upstream input differs')
    require(cli['raw_repeat_equal'] and len(cli['observations']) == 2, 'original CLI repeat differs')
    for repeat, process in enumerate(cli['observations']):
        prefix = 'original-cli/repeat-' + str(repeat)
        require(read(prefix + '/process.json') == process and process['exit_code'] == 0
                and not process['timed_out'] and not process['workspace_changes']
                and process['files_sha256'] == {}, 'original CLI status/files differ')
        for channel in ('stdout', 'stderr'):
            require(raw(prefix + '/' + channel + '.bin') == b''
                    and digest(b'') == process[channel + '_sha256'], 'original CLI raw channel differs')

    summary = read('summary.json')
    cohorts = read('provenance/cohorts.json')
    history = read('historical-observations.json')
    require(summary['ordinary_cases'] == len(cases) == 85
            and summary['ordinary_successes'] == sum(c['observations'][0]['exit_code'] == 0 for c in cases) == 56
            and summary['ordinary_language_errors'] == 29
            and summary['ordinary_java_processes'] == 170 and summary['oracle_error_cases'] == len(boundaries) == 7
            and summary['original_cli_java_processes'] == 2 and summary['compatibility_passes_claimed'] == 0
            and summary['all_historical_expression_java_processes'] == sum(c['java_processes'] for c in cohorts.values()) == 460
            and summary['excluded_historical_rows'] == len(history) == 138
            and summary['unreached_intent_rows'] == sum(not c['intended_semantics_reached'] for c in history) == 114
            and summary['superseded_reached_rows'] == sum(c['intended_semantics_reached'] for c in history) == 24,
            'coverage accounting differs')
    return dict(verified=True, ordinary_cases=85, ordinary_java_processes=170, oracle_error_cases=7,
                original_cli_processes=2, compatibility_passes_claimed=0,
                scope='Fixture integrity and Java provenance only; no candidate compatibility verdict.')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('fixture', nargs='?', type=Path, default=DEFAULT)
    args = parser.parse_args()
    try:
        print(json.dumps(verify(args.fixture), indent=2))
        return 0
    except (OSError, ValueError, KeyError, TypeError) as error:
        print('Parameterized object fixture verification failed: ' + str(error), file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
