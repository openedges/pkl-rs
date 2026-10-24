"""Acquire separate Java 0.32.0 class/object inferred-method cache controls.

The historical object-method alias Java bug is archived separately; it is not
run as a canonical success/error profile by this producer.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

import inferred_new_module_receiver_oracle as engine

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = ROOT / 'tests/fixtures/m2-inferred-new-method-cache'
INPUT_HASHES = {
    'method': {
        'alpha-first.queries.tsv': '376d7a853b74cb7e0ff6c011b0f8cc0993f0f65017dcf01a1e0f1dc81a1d9f20',
        'alpha.pkl': 'b96b50766b7201465022ca3347ddd9ff4b7d1984e640f69722d1f21193a8376c',
        'base.pkl': 'ae0d4d94e3bdb349270e6911b15d155bb8f3ff5f960245f50add9a73173e234c',
        'beta-first.queries.tsv': '16a36770308654f8b80e06ebdff6a3d01ec1d30a783f70fb05557c3b416630fe',
        'beta.pkl': 'a64a0c145bbbc955ec2ba3a892dc5a6a7217f5b993f03acd5229567f2aed03cc',
        'cases.json': 'aeac3c770e21e1f7c83b1d504ff7c0af73fad120ad429e758cd78d1b9b4bdadb',
        'module.pkl': '2f63ea3af3b838456b1a55e6c97e33655aefd9fa4b670401e382894b91852c5b',
    },
    'object-method': {
        'alpha-first.queries.tsv': '2c53f97419a50841029fa2151ea5b2120bb12cf5c31f25da5281ba017a93791b',
        'alpha.pkl': 'b96b50766b7201465022ca3347ddd9ff4b7d1984e640f69722d1f21193a8376c',
        'base.pkl': '3460a611c1d986698c95c92bf886189ba55e9a169cacead63baaaf7014102b60',
        'beta-first.queries.tsv': '1d2bb1ef18e4a937563ad0a1081b45e629ce71e0bd4e237ef0775cba07db3f7e',
        'beta.pkl': 'a64a0c145bbbc955ec2ba3a892dc5a6a7217f5b993f03acd5229567f2aed03cc',
        'cases.json': '99ec0dd2be44618b09be46c087e92bb08d8477bca012c97b1e870b335650edb1',
        'module.pkl': '2f63ea3af3b838456b1a55e6c97e33655aefd9fa4b670401e382894b91852c5b',
    },
}


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def acquire(profile: str, output: Path) -> None:
    if profile not in INPUT_HASHES:
        raise ValueError('unknown inferred-method profile')
    inputs = FIXTURE / profile
    for name, expected in INPUT_HASHES[profile].items():
        if sha(inputs / name) != expected:
            raise ValueError(f'authored {profile} input differs: {name}')
    engine.FIXTURE = inputs
    engine.INPUT_HASHES = INPUT_HASHES[profile]
    engine.SOURCES = tuple(sorted(set(engine.SOURCES) | {'tools/inferred_new_method_cache_oracle.py'}))
    engine.acquire(output)
    report = json.loads((output / 'report.json').read_bytes())
    expected = {'method': {'success': 18, 'error': 4},
                'object-method': {'success': 22}}[profile]
    if not report['complete'] or report['statuses_per_repetition'] != expected:
        raise ValueError(f'{profile} ordinary Java outcomes differ; raw capture retained')
    print(json.dumps({'profile': profile, 'complete': True,
                      'statuses_per_repetition': expected}, sort_keys=True))


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile', choices=sorted(INPUT_HASHES), required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    acquire(args.profile, args.output)
