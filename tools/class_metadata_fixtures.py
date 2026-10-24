"""Offline independent Java authority for five additional base Class declarations."""
import argparse
from collections import Counter
import json
from pathlib import Path
import tarfile
import tempfile

import class_metadata_oracle as metadata
import class_values_oracle as common

DEFAULT = Path(__file__).resolve().parents[1] / 'tests/fixtures/m2-class-metadata-completion'
AUTHORITY_SHA256 = '8b111f12f87e171106ce431c28a3267d027a2d24c927249d387348fd13f5f652'
FILE_SET_SHA256 = '33ccb2c290a8a908ae6cb78584fad15a0eb15c7be01aca846d527e15e718746b'


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
    common.require(not (root / 'report.json').exists(), 'metadata fixture has unexpected root report')
    actual = common.inventory(root)
    actual.pop('manifest.json', None)
    manifest = json.loads((root / 'manifest.json').read_bytes())
    common.require(common.same_json(manifest, dict(schema=1, kind=metadata.KIND, baseline='Pkl 0.32.0', files=actual)),
                   'metadata fixture file bytes/kind differ')
    common.require(common.digest('\n'.join(sorted(actual)).encode()) == FILE_SET_SHA256, 'metadata fixture file-set identity differs')
    archive = root / 'oracle-evidence.tar.gz'
    common.require(common.sha(archive) == AUTHORITY_SHA256, 'independent Java metadata authority differs')
    with tempfile.TemporaryDirectory(prefix='pkl-class-metadata-authority-') as temporary:
        destination = Path(temporary)
        with tarfile.open(archive, 'r:gz') as stream:
            entries = stream.getmembers()
            common.require(len(entries) == len({entry.name for entry in entries}), 'duplicate Java metadata member')
            for entry in entries:
                path = Path(entry.name)
                common.require(not path.is_absolute() and '..' not in path.parts and (entry.isfile() or entry.isdir()),
                               'unsafe Java metadata authority member')
            stream.extractall(destination, filter='data')
        common.require({path.name for path in destination.iterdir()} == {'main', 'preserved-history-01'}, 'metadata cohort coverage differs')
        main = metadata.verify_report(destination / 'main')
        history = metadata.verify_report(destination / 'preserved-history-01')
        captured = destination / 'main/sources' / metadata.INPUT_ROOT
        common.require((root / 'cases.json').read_bytes() == (captured / 'cases.json').read_bytes(), 'metadata selection differs from Java authority')
        names = {source['source'] for case in main['cases'] for source in [case, *case['support_modules']]}
        actual_inputs = {path.relative_to(root).as_posix() for path in (root / 'inputs').rglob('*') if path.is_file()}
        common.require(actual_inputs == names, 'metadata authored source closure differs')
        for name in sorted(names):
            common.require((root / name).read_bytes() == (captured / name).read_bytes(), 'metadata authored input differs from Java authority')
        result = summary(main)
        result['fixture_kind'] = metadata.KIND
        callbacks = Counter(callback['kind'] for row in main['results'] for callback in row['observation']['provider']['callbacks'])
        result['provider_supplement'] = dict(callback_counts=dict(callbacks),
            scope='Actual Java public module-key create/resolve observations and fixed key properties are retained completely. Candidate provider SPI/API parity is unexecuted/required; common query/logger equality is not whole public API compatibility.')
        result['public_export_supplement'] = dict(status='not-acquired-required', sessions=0, java_processes=0,
            public_calls=0, required=True,
            scope='No public exported-Class identity observations were acquired for these five additional Class inputs. Actual Java and candidate export identity/lifecycle parity remain required. Original Class export observations are a separate cohort, not fabricated supplement coverage.')
        result['preserved_history'] = [summary(history)]
        result['preserved_history_scope'] = 'Complete initial Java acquisition, including actual query-scope import-alias errors; history is not extra independent coverage or candidate compatibility.'
        result['immediate_superclass_boundary'] = 'Pinned base declarations and AstBuilder implicit Typed supply immediate parents. Actual ordinary instance is/ancestry queries are captured; complete reflection is not executed by this supplement.'
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
