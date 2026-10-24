"""Offline, independently pinned Java Class authority; candidate parity is separate."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile

import class_values_oracle as oracle
import class_values_exports as exports

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'tests/fixtures/m2-class-values'
AUTHORITY_SHA256 = 'a39060fbf155947212aaae5b2e950dc2cd04381a78cb2860a7f8b1675b3657fc'
FILE_SET_SHA256 = '55c61ff764188c4b613d94b1667b75a3f68afc7cfc381db7e6a9ff92e3a971a3'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def summary(report):
    observations = [row['observation'] for row in report['results']]
    statuses = Counter(value for row in observations for value in row['call_statuses'].values())
    return dict(sessions=len(report['cases']), java_processes=len(report['results']),
        repeated_raw_equal=sum(report['replay_equal'].values()), public_calls=sum(statuses.values()),
        logger_callbacks=sum(row['logger_callbacks'] for row in observations),
        warning_callbacks=sum(row['warning_callbacks'] for row in observations),
        call_status_counts=dict(statuses), oracle_error_calls=statuses.get('oracle-error', 0),
        api_exception_calls=statuses.get('api-exception', 0), compatibility_passes_claimed=0)


def verify(root=DEFAULT):
    root = Path(root)
    dispatch = json.loads((root / 'manifest.json').read_bytes())
    if 'kind' in dispatch:
        import class_metadata_fixtures
        require(dispatch['kind'] == class_metadata_fixtures.metadata.KIND, 'unknown Class fixture authority kind')
        return class_metadata_fixtures.verify(root)
    require(not (root / 'report.json').exists(), 'fixture has an unexpected root report')
    actual = oracle.inventory(root)
    actual.pop('manifest.json', None)
    manifest = json.loads((root / 'manifest.json').read_bytes())
    require(oracle.same_json(manifest, dict(schema=1, baseline='Pkl 0.32.0', files=actual)), 'fixture file bytes differ')
    require(sha('\n'.join(sorted(actual)).encode()) == FILE_SET_SHA256, 'fixture file-set identity differs')
    archive = root / 'oracle-evidence.tar.gz'
    require(sha(archive.read_bytes()) == AUTHORITY_SHA256, 'independent Java Class authority differs')
    with tempfile.TemporaryDirectory(prefix='pkl-class-authority-') as temporary:
        destination = Path(temporary)
        with tarfile.open(archive, 'r:gz') as stream:
            entries = stream.getmembers()
            require(len(entries) == len({entry.name for entry in entries}), 'duplicate Java Class archive member')
            for entry in entries:
                path = Path(entry.name)
                require(not path.is_absolute() and '..' not in path.parts and (entry.isfile() or entry.isdir()), 'unsafe Java Class authority member')
            stream.extractall(destination, filter='data')
        require({path.name for path in destination.iterdir()} == {'main', 'exports', 'preserved-history-01', 'preserved-history-02', 'preserved-history-03'}, 'Java Class cohort coverage differs')
        main = oracle.verify_report(destination / 'main')
        supplemental = exports.verify_report(destination / 'exports')
        histories = [oracle.verify_report(destination / ('preserved-history-' + index)) for index in ['01', '02', '03']]
        captured = destination / 'main/sources/tests/fixtures/m2-class-values'
        captured_exports = destination / 'exports/sources/tests/fixtures/m2-class-values'
        require((root / 'cases.json').read_bytes() == (captured / 'cases.json').read_bytes(), 'main Class selection differs from Java authority')
        require((root / 'export-cases.json').read_bytes() == (captured_exports / 'export-cases.json').read_bytes(), 'public-export selection differs from Java authority')
        main_sources = {row['source'] for case in main['cases'] for row in [case, *case['support_modules']]}
        export_sources = {row['source'] for case in supplemental['cases'] for row in case['sources'].values()}
        actual_inputs = {path.relative_to(root).as_posix() for path in (root / 'inputs').rglob('*') if path.is_file()}
        require(actual_inputs == main_sources | export_sources, 'fixture authored input closure differs')
        for name in sorted(actual_inputs):
            source_root = captured if name in main_sources else captured_exports
            require((root / name).read_bytes() == (source_root / name).read_bytes(), 'Class authored input differs from Java authority')
        result = summary(main)
        provider_kinds = Counter(callback['kind'] for row in main['results'] for callback in row['observation']['provider']['callbacks'])
        result['provider_supplement'] = dict(callback_counts=dict(provider_kinds),
            scope='Actual Java public module-key create/resolve observations and fixed key properties, retained completely. Candidate SPI/API parity is unexecuted/required unless separately mapped and compared. Common query/logger byte equality alone is not whole API compatibility.')
        statuses = Counter(value for row in supplemental['results'] for value in row['observation']['call_statuses'].values())
        result['public_export_supplement'] = dict(sessions=len(supplemental['cases']), java_processes=len(supplemental['results']),
            repeated_raw_equal=int(supplemental['replay_equal']), public_calls=sum(statuses.values()), call_status_counts=dict(statuses),
            identity_comparisons=sum(len(row['observation']['identity_comparisons']) for row in supplemental['results']),
            observations=supplemental['results'][0]['observation']['identity_comparisons'],
            scope='Actual public evaluateExpression exported Class reference/equals identity and evaluator lifecycle. Candidate public export API parity is unexecuted/required; Java value/exception classes are producer provenance.')
        result['preserved_history'] = [summary(report) for report in histories]
        result['preserved_history_scope'] = 'All earlier complete acquisitions, including actual invalid authored/query scope outcomes. Retained as history, never additional independent feature coverage or candidate compatibility passes.'
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
