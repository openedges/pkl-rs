#!/usr/bin/env python3
"""Verify/export the complete pinned Java lossless Host API observation set."""
from pathlib import Path
import argparse
import hashlib
import json

PINS = {
    'cases.json': '6a63370ceaea7393208bed315b6cc22686479e7e23e0401feca1754b598d4be8',
    'report.json': 'c2cbbb2085d7ed3eb792a9b0db0478ec2637e232bb7caa6caae189f26f1fd855',
    'inventory.json': '471c0ff128cffe250bd72dfa254d1f2873ee7300d7b801f01d132ddd033b3742',
    'closure.json': '7017fae0387878db762a7b7fb4e78eea36a7320596416af3908b9141433d1a84',
    'LosslessHostProbe.java': '8426cef56923b5b334a96b1a7dca97acf8862fefe8f1c4e5f69ca0e10610381f',
}

def require(condition, message):
    if not condition:
        raise ValueError(message)

def digest(raw):
    return hashlib.sha256(raw).hexdigest()

def verify(fixtures):
    root = Path(fixtures).resolve()
    bound = {}
    def pinned(name):
        path = root/name
        raw = path.read_bytes()
        require(digest(raw) == PINS[name], 'pinned oracle identity: '+name)
        bound[str(path)] = digest(raw)
        return raw
    cases = json.loads(pinned('cases.json'))
    report = json.loads(pinned('report.json'))
    inventory = json.loads(pinned('inventory.json'))
    closure = json.loads(pinned('closure.json'))
    pinned('LosslessHostProbe.java')
    require(closure['reportSha256'] == PINS['report.json'] and closure['inventorySha256'] == PINS['inventory.json'], 'closure identity')
    def raw(name):
        path = root/name
        value = path.read_bytes()
        require(name in inventory and digest(value) == inventory[name]['sha256'], 'original raw binding: '+name)
        bound[str(path)] = digest(value)
        return value
    def text(name):
        return raw(name).decode('utf-8')
    def units(name):
        value = raw(name)
        require(len(value) % 2 == 0, 'odd UTF16 channel')
        return [int.from_bytes(value[i:i+2], 'big') for i in range(0, len(value), 2)]
    def message_from_units(value):
        return b''.join(unit.to_bytes(2, 'big') for unit in value).decode('utf-16-be', errors='surrogatepass')
    require(len(cases) == 11 and len({c['id'] for c in cases}) == 11, 'all eleven original cases')
    require(report['caseCount'] == 11 and report['freshJvmCount'] == 22 and report['callCount'] == 80, 'full oracle scope')
    require(report['powerAssertions'] is True and not report['inputChanges'] and not report['repeatDifferences'], 'oracle stability')
    graphs = []
    for case in cases:
        identity = case['id']
        source = text(f'workspaces/{identity}/main.pkl')
        require(source == case['source'] and digest(source.encode()) == case['sourceSha256'], 'source identity')
        require((Path(case['workspace'])/'main.pkl').as_uri() == case['moduleUri'], 'original physical source URI')
        expected = None
        cause = None
        counters = None
        for repeat in [0, 1]:
            record = next(r for r in report['records'] if r['case'] == identity and r['repeat'] == repeat)
            require(record['result']['exit_code'] == 0 and not record['result']['timed_out'], 'oracle process failure')
            process = json.loads(raw(f'cases/{identity}/{repeat}.process.json'))
            require(process == {key: record[key] for key in ['command', 'cwd', 'environment', 'result']}, 'captured process/profile identity')
            require(record['cwd'] == case['workspace'] and record['command'][-4:] == [case['workspace'], str(Path(case['workspace']).parents[1]/'cases'/identity/str(repeat)/'calls'), case['mode'], case['cause']], 'actual builder invocation context')
            require(record['workspaceBefore'] == record['workspaceAfter'] and record['workspaceBefore']['main.pkl']['sha256'] == case['sourceSha256'], 'original workspace source')
            observation = next(o for o in report['observations'] if o['case'] == identity and o['repeat'] == repeat)
            require(len(observation['calls']) == len(case['expressions']), 'ordered call coverage')
            base = f'cases/{identity}/{repeat}/calls'
            is_null = raw(base+'/cause-null.bin')
            require(is_null in [b'true', b'false'], 'explicit cause null marker')
            incoming = None if is_null == b'true' else message_from_units(units(base+'/cause.code-units.bin'))
            if incoming is not None:
                require(incoming.encode('utf-8', errors='replace') == raw(base+'/cause.bin'), 'original cause bytes')
            require((case['cause'] == 'null') == (incoming is None), 'null cause context')
            require(case['cause'] != 'empty' or incoming == '', 'empty cause context')
            calls = []
            for i, expression in enumerate(case['expressions']):
                call = observation['calls'][i]
                prefix = f'{base}/call-{i}'
                require(call['index'] == i and call['expression'] == expression and call['rawPrefix'] == prefix, 'original call expression/order')
                require(text(prefix+'.expression.bin') == expression and text(prefix+'.status.bin') == call['status'], 'captured call inputs/status')
                require(call['status'] in ['success\n', 'error\n'], 'actual oracle status')
                success = call['status'] == 'success\n'
                require(not call['oracleError'] and text(prefix+'.error-class.bin') == call['errorClass'], 'ordinary Java result')
                channel = 'stdout' if success else 'stderr'
                payload = raw(prefix+'.'+channel+'.bin')
                value_units = units(prefix+'.'+channel+'.code-units.bin')
                require(message_from_units(value_units).encode('utf-8', errors='replace') == payload, 'original Java unit/byte channel relation')
                other = 'stderr' if success else 'stdout'
                require(raw(prefix+'.'+other+'.bin') == b'' and units(prefix+'.'+other+'.code-units.bin') == [], 'complete opposite channel')
                calls.append({'actionIndex': i, 'kind': 'value' if success else 'error', 'javaStatus': 'success' if success else 'error', 'javaErrorClass': call['errorClass'], 'javaUtf8Hex': payload.hex(), ('javaValueUnits' if success else 'javaErrorUnits'): value_units})
            counter_raw = text(base+'/counters.bin')
            require(counter_raw == observation['counters'], 'original counter observation')
            parsed = dict(line.split('=') for line in counter_raw.splitlines())
            count = {key: int(parsed[key]) for key in ['resolve', 'source', 'read']}
            if repeat == 0:
                expected, cause, counters = calls, incoming, count
            else:
                require(calls == expected and incoming == cause and count == counters, 'original repeat differences')
        failure = {'kind': 'io', 'message': cause}
        resolution = {'importUri': 'lossless:/child', 'resolvedUri': 'lossless:/child', 'sourceUri': 'lossless:/child'}
        graph = {'id': identity, 'operation': 'expressions', 'source': source, 'uri': case['moduleUri'], 'moduleName': 'main', 'paths': case['expressions'], 'powerAssertions': True, 'actions': [{'kind': 'path', 'index': i} for i in range(len(case['expressions']))], 'expected': {'results': expected, 'callbacks': [], 'logs': [], 'requestCounts': {'resolve': counters['resolve'], 'load-source': counters['source']}, 'resourceReadScopePrefix': 'lossless:', 'resourceReadCounts': {'lossless:/resource': counters['read']}}}
        if case['mode'] == 'module-resolve':
            graph['resolutions'] = [{'importUri': 'lossless:/child', 'resolves': [{'failure': failure}, resolution]}]
            graph['sources'] = [{'resolvedUri': 'lossless:/child', 'text': 'value = 9\n', 'inferredName': 'child'}]
        elif case['mode'] == 'module-source':
            graph['resolutions'] = [resolution]
            graph['sources'] = [{'resolvedUri': 'lossless:/child', 'loads': [{'failure': failure}, {'text': 'value = 9\n', 'inferredName': 'child'}]}]
        elif case['mode'] == 'resource':
            graph['resources'] = [{'uri': 'lossless:/resource', 'reads': [{'failure': failure}, {'valueUnits': [57]}]}]
        elif case['mode'] == 'unknown-format':
            encoded = cause.encode('utf-16-be', errors='surrogatepass')
            graph['resources'] = [{'uri': 'prop:pkl.outputFormat', 'valueUnits': [int.from_bytes(encoded[i:i+2], 'big') for i in range(0, len(encoded), 2)]}]
        else:
            require(case['mode'] == 'converter', 'unknown original mode')
        graph['provenance'] = {'sourceSha256': case['sourceSha256'], 'oracleMode': case['mode'], 'causeKind': case['cause'], 'hostIdentityBoundary': 'ModuleKey/ResolvedModuleKey identities and completion plans are helper-source-backed; no independent identity-table observation claimed.', 'outputFormatBoundary': 'Java setOutputFormat uses exact shared prop:pkl.outputFormat completion. Custom reader counters exclude these property completions.'}
        graphs.append(graph)
    return {'passed': True, 'graphFixtures': graphs, 'graphCount': 11, 'orderedCallsPerRepetition': 40, 'actualJavaEvaluations': 80, 'bindings': [{'path': p, 'sha256': v} for p, v in sorted(bound.items())], 'scope': 'All original eleven fresh Java API cases and repeats, full UTF16 and JavaUTF8. Typed IO null/empty/units and real output.text operation; no URI/source rewriting or CLI equivalence.'}

if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--fixtures', type=Path, default=Path(__file__).resolve().parents[1]/'tests/fixtures/m2-lossless-host')
    args = parser.parse_args()
    print(json.dumps(verify(args.fixtures), ensure_ascii=True))
