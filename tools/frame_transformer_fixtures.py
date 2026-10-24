"""Offline audit of independently pinned Java full-frame transformer observations."""
import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile

import frame_transformer_oracle as oracle

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'tests/fixtures/m2-frame-transformers'
# Independent reviewed Java authority, not replaceable by refreshing fixture metadata.
AUTHORITY_SHA256 = '53efa9f779095d54fb2fa3345bab0d01e4bc4ddfa707f45c2b3223c63b6606dc'
FILE_SET_SHA256 = 'ae10b5c3e23c3809d8e6245aa0c2d193e08570edb501ee444414db94f2dc0e4a'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def summary(report):
    observations = [row['observation'] for row in report['results']]
    statuses = Counter(status for row in observations for status in row['call_statuses'].values())
    return dict(sessions=len(report['cases']), java_processes=len(report['results']),
        repeated_raw_equal=sum(report['replay_equal'].values()), public_calls=sum(statuses.values()),
        transformer_callbacks=sum(row['transformer_callbacks'] for row in observations),
        logger_callbacks=sum(row['logger_callbacks'] for row in observations),
        warning_callbacks=sum(row['warning_callbacks'] for row in observations),
        call_status_counts=dict(statuses), oracle_error_calls=statuses.get('oracle-error', 0),
        api_exception_calls=statuses.get('api-exception', 0),
        nested_oracle_error_results=sum(row['nested_oracle_error_results'] for row in observations),
        compatibility_passes_claimed=0)


def verify(root=DEFAULT):
    root = Path(root)
    require(not (root / 'report.json').exists(), 'fixture contains unexpected root report')
    actual = oracle.inventory(root)
    actual.pop('manifest.json', None)
    manifest = json.loads((root / 'manifest.json').read_bytes())
    require(manifest == dict(schema=1, baseline='Pkl 0.32.0', files=actual), 'fixture file bytes differ')
    require(sha('\n'.join(sorted(actual)).encode()) == FILE_SET_SHA256, 'fixture file-set identity differs')
    archive = root / 'oracle-evidence.tar.gz'
    require(sha(archive.read_bytes()) == AUTHORITY_SHA256, 'independent Java frame authority differs')
    with tempfile.TemporaryDirectory(prefix='pkl-frame-authority-') as temporary:
        destination = Path(temporary)
        with tarfile.open(archive, 'r:gz') as stream:
            entries = stream.getmembers()
            require(len(entries) == len({entry.name for entry in entries}), 'duplicate authority archive member')
            for entry in entries:
                path = Path(entry.name)
                require(not path.is_absolute() and '..' not in path.parts and (entry.isfile() or entry.isdir()),
                        'unsafe authority archive member')
            stream.extractall(destination, filter='data')
        main = oracle.verify_report(destination / 'main')
        buffer = oracle.verify_report(destination / 'buffer')
        captured = destination / 'main/sources/tests/fixtures/m2-frame-transformers'
        captured_buffer = destination / 'buffer/sources/tests/fixtures/m2-frame-transformers'
        require((root / 'cases.json').read_bytes() == (captured / 'cases.json').read_bytes(),
                'main frame selection differs from Java authority')
        require((root / 'buffer-cases.json').read_bytes() == (captured_buffer / 'buffer-cases.json').read_bytes(),
                'buffer supplement differs from Java authority')
        for path in (root / 'inputs').glob('*.pkl'):
            require(path.read_bytes() == (captured / 'inputs' / path.name).read_bytes(),
                    'frame source differs from Java authority')
        result = summary(main)
        result['buffer_supplement'] = summary(buffer)
        result['buffer_supplement']['scope'] = 'Java-only public TestResults.logs() observations; candidate test-tool API parity is unexecuted.'
        return result


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))
