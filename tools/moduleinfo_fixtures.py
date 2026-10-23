"""Verify persistent ModuleInfo raw Java cases without promoting internal/followup boundaries."""
import argparse
import hashlib
import json
import os
from pathlib import Path

PINS = {
    'report.json': '2aa0b16454b54f5d34c364d29003eb9c10a230189b50841a68b3f6eba3d7e09d',
    'closure.json': '0228ed3e39db1382112ba4e6d9ba38060de326c95dd92dec8197f8bd51b9a307',
    'cases.json': 'b1b962fdb1c9c7cb67ec417c820d791b24a02a59074ed93924524f2d69f6f756',
    'inventory.json': 'c18154d8877eaa61a5cbccd33bd6295e7ece9fb91fc2ff8cd992c3b6769bc0fd',
}
TOP = {'cases.json', 'report.json', 'profile.json', 'ModuleInfoProbe.java', 'run.py',
       'interpretation.json', 'source-evidence-addendum.json', 'inputs-before.json', 'inputs-after.json',
       'toolchain-identity.json', 'terminal.json', 'exclusions.json', 'closure-exclusions.json'}


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def need(condition, message):
    if not condition:
        raise ValueError(message)


def selected(name):
    return (name in TOP or name.startswith(('upstream/', 'workspaces/'))
            or (name.startswith('repeats/') and ('/calls/' in name or name.endswith(
                ('/process.json', '/result.json', '/stdout.bin', '/stderr.bin', '.process.json')))))


def verify(fixtures):
    root = Path(fixtures).resolve()
    manifest = json.loads((root / 'manifest.json').read_bytes())
    need(manifest['schema'] == 1, 'manifest schema')
    names = set()
    for directory, dirs, files in os.walk(root, topdown=True, followlinks=False):
        for name in dirs:
            need(not (Path(directory) / name).is_symlink(), 'fixture directory symlink')
        for name in files:
            path = Path(directory) / name
            need(not path.is_symlink(), 'fixture file symlink')
            if path != root / 'manifest.json':
                names.add(str(path.relative_to(root)))
    need(names == set(manifest['files']), 'fixture membership')
    for name, row in manifest['files'].items():
        raw = (root / name).read_bytes()
        need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'fixture bytes ' + name)
    base = root / 'oracle'
    for name, digest in PINS.items():
        need(sha((base / name).read_bytes()) == digest, 'pinned original ' + name)
    closure = json.loads((base / 'closure.json').read_bytes())
    need(closure['closed'] and not closure['liveHandles'], 'original closure')
    need(closure['reportSha256'] == PINS['report.json'] and closure['inventorySha256'] == PINS['inventory.json'], 'original closure pins')
    original_inventory = {r['path']: r for r in json.loads((base / 'inventory.json').read_bytes())['files']}
    required = {name for name in original_inventory if selected(name)} | {'inventory.json', 'closure.json'}
    report = json.loads((base / 'report.json').read_bytes())
    cases = json.loads((base / 'cases.json').read_bytes())
    selection = json.loads((root / 'selection.json').read_bytes())
    provenance = json.loads((root / 'provenance.json').read_bytes())
    need(selection['schema'] == 1 and len(cases) == len(selection['cases']) == 28, 'all28 sources')
    original_root = Path(cases[0]['workspace']).parents[1]
    need(provenance['originalRoot'] == str(original_root) and provenance['pins'] == PINS, 'original provenance')
    bindings = []
    expected_names = {'selection.json', 'provenance.json'}
    for name in sorted(required):
        path = base / name
        raw = path.read_bytes()
        if name in original_inventory:
            row = original_inventory[name]
            need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'copied original ' + name)
        relative = 'oracle/' + name
        expected_names.add(relative)
        bindings.append({'fixturePath': relative, 'originalPath': str(original_root / name), 'bytes': len(raw), 'sha256': sha(raw)})
    need(provenance['bindings'] == bindings, 'all mandatory original bindings')
    need(sha((base / 'profile.json').read_bytes()) == report['profileSha256'], 'original profile identity')
    ordinary = internal = followups = 0
    for case, actual in zip(cases, selection['cases']):
        identity = case['id']
        source = base / 'workspaces' / identity / 'main.pkl'
        folder = source.parent
        name = next(line[len('module '):] for line in case['source'].splitlines() if line.startswith('module '))
        outer = [c for o in report['observations'] for c in o['calls'] if c['case'] == identity and c['prefix'].startswith('outer-')]
        boundary = all(c['errorClass'] == 'org.pkl.core.PklBugException' for c in outer)
        expected = {'id': identity, 'sourceFile': str(source.relative_to(root)), 'sourceSha256': case['sourceSha256'],
                    'moduleUri': case['moduleUri'], 'moduleName': name, 'expressions': case['expressions'],
                    'comparisonScope': 'java-internal-parser-boundary' if boundary else 'ordinary-full-diagnostic',
                    'sharedEvaluatorFollowupScope': 'preserved-unexecuted-native-api'}
        need(actual == expected, 'source/URI/order/scope boundary ' + identity)
        need(source.read_text() == case['source'] and sha(source.read_bytes()) == case['sourceSha256'], 'original source ' + identity)
        need(case['moduleUri'] == (Path(case['workspace']) / 'main.pkl').as_uri(), 'original physical URI')
        need((folder / 'uri.txt').read_text() == case['moduleUri'] and (folder / 'module-name.txt').read_text() == name, 'derived context')
        expected_names.update({str((folder / f).relative_to(root)) for f in ['uri.txt', 'module-name.txt']})
        for repeat in [0, 1]:
            observation = next(o for o in report['observations'] if o['repeat'] == repeat)
            calls = [c for c in observation['calls'] if c['case'] == identity]
            need([c['prefix'] for c in calls] == ['outer-0', 'outer-1', 'outer-2', 'followup-0'], 'all ordered calls ' + identity)
            for call in calls:
                path = base / call['rawPrefix']
                prefix = path.name
                directory = path.parent
                expression = 'stable' if prefix == 'followup-0' else case['expressions'][int(prefix[-1])]
                need(call['expression'] == expression and (directory / (prefix + '.expression.bin')).read_text() == expression, 'call expression')
                need((directory / (prefix + '.status.bin')).read_text() == call['status'] and call['status'] in ['success\n', 'error\n'], 'call status')
                need((directory / (prefix + '.error-class.bin')).read_text() == call['errorClass'], 'full Java error class')
                channel = 'stdout' if call['status'] == 'success\n' else 'stderr'
                opposite = 'stderr' if channel == 'stdout' else 'stdout'
                raw = (directory / (prefix + '.' + channel + '.bin')).read_bytes()
                units = (directory / (prefix + '.' + channel + '.code-units.bin')).read_bytes()
                need(sha(raw) == call['utf8Sha256'] and raw.hex() == call['utf8Hex'], 'full Java bytes')
                need(len(units) % 2 == 0 and sha(units) == call['utf16BESha256'] and units.hex() == call['utf16BEHex'], 'full Java units')
                need(units.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace') == raw, 'Java serialization boundary')
                need(not (directory / (prefix + '.' + opposite + '.bin')).read_bytes() and not (directory / (prefix + '.' + opposite + '.code-units.bin')).read_bytes(), 'empty opposite channels')
                if prefix == 'followup-0':
                    need(call['status'] == 'success\n' and raw == b'7', 'original shared evaluator followup')
                    followups += 1
                elif boundary:
                    need(call['errorClass'] == 'org.pkl.core.PklBugException', 'internal boundary')
                    internal += 1
                else:
                    need(call['errorClass'] in ['', 'org.pkl.core.PklException'], 'ordinary error class')
                    ordinary += 1
    need(names == expected_names, 'mandatory exact fixture membership')
    need((ordinary, internal, followups) == (156, 12, 56), 'complete separate call accounting')
    return {'passed': True, 'sources': 28, 'originalCalls': 224, 'ordinaryRawComparisons': ordinary,
            'javaInternalBoundaries': internal, 'unexecutedNativeSharedEvaluatorFollowups': followups,
            'copiedOriginalBindings': len(bindings), 'scope': 'Fixture integrity only. Internal/constructor/API timing boundaries never become ordinary compatibility passes.'}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--fixtures', type=Path, default=Path(__file__).resolve().parents[1] / 'tests/fixtures/m2-moduleinfo')
    print(json.dumps(verify(parser.parse_args().fixtures), indent=2))
