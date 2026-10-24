"""Offline verifier for independently pinned Java method-deprecation observations."""
import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile

import m2_deprecation

ROOT = Path(__file__).resolve().parents[1]
DEFAULT = ROOT / 'tests/fixtures/m2-deprecation'
# Independent reviewed Java authority; refreshing a local manifest cannot replace it.
AUTHORITY_SHA256 = 'e0e78f3f55c680ae4a37620dd3355f9efd6158d4f0ba141852c8825070de2fae'
FILE_SET_SHA256 = '4c9228366833de3571ff9c89dc0b360bffec000682dadf83ac11a15c1842d4e6'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def verify(root=DEFAULT):
    root = Path(root)
    require(not (root / 'report.json').exists(), 'fixture file coverage differs')
    files = m2_deprecation.inventory(root)
    manifest = json.loads((root / 'manifest.json').read_bytes())
    # The inventory helper excludes report.json only; this fixture never has one at its root.
    expected = {name: value for name, value in files.items() if name != 'manifest.json'}
    require(manifest == dict(schema=1, baseline='Pkl 0.32.0', files=expected), 'fixture bytes differ')
    require(digest('\n'.join(sorted(expected)).encode()) == FILE_SET_SHA256, 'fixture file coverage differs')
    archive = root / 'oracle-evidence.tar.gz'
    require(digest(archive.read_bytes()) == AUTHORITY_SHA256, 'independent Java authority differs')
    with tempfile.TemporaryDirectory(prefix='pkl-deprecation-verify-') as temporary:
        destination = Path(temporary)
        with tarfile.open(archive, 'r:gz') as stream:
            members = stream.getmembers()
            names = [entry.name for entry in members]
            require(len(names) == len(set(names)), 'duplicate archive member')
            for entry in members:
                path = Path(entry.name)
                require(not path.is_absolute() and '..' not in path.parts
                        and (entry.isfile() or entry.isdir()), 'unsafe archive member')
            stream.extractall(destination, filter='data')
        report = m2_deprecation.verify_report(destination / 'main')
        cycles = m2_deprecation.verify_report(destination / 'cycles')
        captured = destination / 'main/sources/tests/fixtures/m2-deprecation'
        cycle_sources = destination / 'cycles/sources/tests/fixtures/m2-deprecation'
        require((root / 'cases.json').read_bytes() == (captured / 'cases.json').read_bytes(),
                'control selection differs from Java authority')
        require((root / 'cycles.json').read_bytes() == (cycle_sources / 'cycles.json').read_bytes(),
                'evaluator cycle selection differs from Java authority')
        for source in (root / 'inputs').glob('*.pkl'):
            require(source.read_bytes() == (captured / 'inputs' / source.name).read_bytes(),
                    'control source differs from Java authority')
        results = report['results']
        return dict(sessions=len(report['cases']), java_processes=len(results),
                    repeated_raw_equal=sum(report['replay_equal'].values()),
                    public_calls=sum(len(row['observation']['statuses']) for row in results),
                    warning_callbacks=sum(row['observation']['warnings'] for row in results),
                    trace_callbacks=sum(row['observation']['traces'] for row in results),
                    oracle_error_calls=sum(len(row['observation']['oracle_errors']) for row in results),
                    supplemental_cycles=dict(sessions=len(cycles['cases']), java_processes=len(cycles['results']),
                        evaluator_instances=sum(case['evaluator_cycles'] for case in cycles['cases']) * cycles['repetitions'],
                        repeated_raw_equal=sum(cycles['replay_equal'].values()),
                        public_calls=sum(len(row['observation']['statuses']) for row in cycles['results']),
                        warning_callbacks=sum(row['observation']['warnings'] for row in cycles['results']),
                        compatibility_passes_claimed=0),
                    compatibility_passes_claimed=0)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--fixture', type=Path, default=DEFAULT)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), sort_keys=True))


if __name__ == '__main__':
    main()
