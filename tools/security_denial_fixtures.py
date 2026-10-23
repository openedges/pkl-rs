"""Verify actual Standard-policy SecurityManagerException fixture and exact typed boundary."""
import argparse
import hashlib
import json
import os
from pathlib import Path
PINS = {'report.json': '5aa3d3a4f48c525e8bdf485d9347ecd800270fb4dbc4932d995df3291c7759ad', 'closure.json': '4961f82718d9c9323f3303781af43a59fe820e8cf6c0e2c05a5658aca5001d25', 'inventory.json': '8a79a007eb225ca1e74d3bec27a57696afeb81c785e0b7b7231efd7b9ee29ede', 'cases.json': '5ebf1093031f303e0ebbcd8d9b86992305ffea69f6a0f73ffc861d82885ead72'}

def sha(raw): return hashlib.sha256(raw).hexdigest()
def need(value, message):
    if not value: raise ValueError(message)
def verify(fixtures):
    root = Path(fixtures).resolve(); base = root / 'oracle'
    manifest = json.loads((root / 'manifest.json').read_bytes()); need(manifest['schema'] == 1, 'manifest schema'); names = set()
    for directory, dirs, files in os.walk(root, topdown=True, followlinks=False):
        for name in dirs: need(not (Path(directory) / name).is_symlink(), 'fixture directory symlink')
        for name in files:
            path = Path(directory) / name; need(not path.is_symlink(), 'fixture file symlink')
            if path != root / 'manifest.json': names.add(str(path.relative_to(root)))
    need(names == set(manifest['files']), 'fixture membership')
    for name, row in manifest['files'].items():
        raw = (root / name).read_bytes(); need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'fixture bytes ' + name)
    for name, digest in PINS.items(): need(sha((base / name).read_bytes()) == digest, 'pinned original ' + name)
    closure = json.loads((base / 'closure.json').read_bytes()); need(closure['closed'] and not closure['liveHandles'], 'original closed')
    need(closure['reportSha256'] == PINS['report.json'] and closure['inventorySha256'] == PINS['inventory.json'], 'closure identity')
    inventory = {r['path']: r for r in json.loads((base / 'inventory.json').read_bytes())['files']}; originals = set(inventory) | {'inventory.json', 'closure.json'}
    report = json.loads((base / 'report.json').read_bytes()); cases = json.loads((base / 'cases.json').read_bytes()); need(len(cases) == 1, 'case count'); case = cases[0]; origin = Path(case['workspace']).parent
    provenance = json.loads((root / 'provenance.json').read_bytes()); need(provenance['originalRoot'] == str(origin) and provenance['pins'] == PINS, 'source provenance')
    bindings = []
    for name in sorted(originals):
        raw = (base / name).read_bytes()
        if name in inventory: need(len(raw) == inventory[name]['bytes'] and sha(raw) == inventory[name]['sha256'], 'copied original ' + name)
        bindings.append({'fixturePath': 'oracle/' + name, 'originalPath': str(origin / name), 'bytes': len(raw), 'sha256': sha(raw)})
    need(provenance['bindings'] == bindings, 'mandatory source bindings')
    need(names == {'oracle/' + name for name in originals} | {'context.json', 'provenance.json', 'uri.txt', 'denied-uri.txt'}, 'exact fixture membership')
    context = json.loads((root / 'context.json').read_bytes()); expected = {'sourceFile': 'oracle/workspace/main.pkl', 'moduleUri': case['moduleUri'], 'deniedUri': case['deniedUri'], 'expressions': case['expressions'], 'completionStage': 'resolve', 'completionKind': 'security-manager-exception', 'nativeCounterScope': 'Only3actualdeniedchildresolutions per repeat;Java fullpolicycallbacks retained separately,not claimed equivalent.'}
    need(context == expected, 'typed exception/URI/stage contract')
    need((root / 'uri.txt').read_text() == case['moduleUri'] and (root / 'denied-uri.txt').read_text() == case['deniedUri'], 'derived original URI context')
    need((base / 'workspace/main.pkl').read_text() == case['source'] and sha((base / 'workspace/main.pkl').read_bytes()) == case['sourceSha256'], 'source bytes')
    need(case['moduleUri'] == (Path(case['workspace']) / 'main.pkl').as_uri() and case['deniedUri'] == (Path(case['workspace']) / 'child.pkl').as_uri(), 'original physical URIs')
    need(sha((base / 'workspace/child.pkl').read_bytes()) == case['deniedExistingFileSha256'], 'actual existing child')
    need(sha((base / 'profile.json').read_bytes()) == report['profileSha256'], 'profile identity')
    calls = denials = 0
    need([o['repeat'] for o in report['observations']] == [0, 1], 'fresh repeat coverage')
    for observation in report['observations']:
        folder = base / 'repeats' / str(observation['repeat']) / 'calls'; need(len(observation['calls']) == 5, 'all5calls')
        for index, call in enumerate(observation['calls']):
            prefix = 'outer-' + str(index); need(call['prefix'] == prefix and call['expression'] == case['expressions'][index], 'ordered calls')
            need((folder / (prefix + '.expression.bin')).read_text() == call['expression'], 'actual expression')
            need((folder / (prefix + '.status.bin')).read_text() == call['status'] and call['status'] == ('error\n' if index == 2 else 'success\n'), 'actual statuses')
            need((folder / (prefix + '.error-class.bin')).read_text() == call['errorClass'] == ('org.pkl.core.PklException' if index == 2 else ''), 'ordinary API class')
            channel = 'stderr' if index == 2 else 'stdout'; opposite = 'stdout' if index == 2 else 'stderr'; raw = (folder / (prefix + '.' + channel + '.bin')).read_bytes(); units = (folder / (prefix + '.' + channel + '.code-units.bin')).read_bytes()
            need(sha(raw) == call['utf8Sha256'] and raw.hex() == call['utf8Hex'], 'full API bytes'); need(sha(units) == call['utf16BESha256'] and units.hex() == call['utf16BEHex'], 'full API units'); need(units.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace') == raw, 'Java serialization boundary')
            need(not (folder / (prefix + '.' + opposite + '.bin')).read_bytes() and not (folder / (prefix + '.' + opposite + '.code-units.bin')).read_bytes(), 'opposite channels')
            for suffix, key in [('before', 'countersBefore'), ('after', 'countersAfter')]: need((folder / (prefix + '.counters-' + suffix + '.bin')).read_text() == call[key], 'full policy counters')
            denied = dict(line.split('=', 1) for line in call['countersAfter'].splitlines())['denied']; need(denied == str([1, 2, 3, 3, 3][index]), 'denied retries and successful caught cache'); calls += 1
        for index in range(3):
            prefix = 'denial-' + str(index); need((folder / (prefix + '.error-class.bin')).read_bytes() == b'org.pkl.core.SecurityManagerException', 'actual exception provenance'); need((folder / (prefix + '.uri.bin')).read_text() == case['deniedUri'], 'typed denied URI'); raw = (folder / (prefix + '.message.bin')).read_bytes(); units = (folder / (prefix + '.message.code-units.bin')).read_bytes(); need(units.decode('utf-16-be', 'surrogatepass').encode('utf-8', 'replace') == raw, 'raw typed cause serialization'); need(raw == (folder / 'outer-0.stdout.bin').read_bytes() == (folder / 'outer-1.stdout.bin').read_bytes() == (folder / 'outer-4.stdout.bin').read_bytes(), 'exact guest caught message'); denials += 1
    need((calls, denials) == (10, 6), 'complete accounting')
    return {'passed': True, 'apiCalls': calls, 'actualSecurityManagerExceptions': denials, 'completionKind': 'security-manager-exception', 'completionStage': 'resolve', 'scope': 'Actual Java policy producer bound to typed native resolve completion;not generic Denied/rawHost,fullsecurity policy or source-load equivalence.'}
if __name__ == '__main__':
    p = argparse.ArgumentParser(); p.add_argument('--fixtures', type=Path, default=Path(__file__).resolve().parents[1] / 'tests/fixtures/m2-security-denial'); print(json.dumps(verify(p.parse_args().fixtures), indent=2))
