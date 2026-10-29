"""Independent Stage114 readback: manual tar records and zlib gzip streaming.

No producer imports, tarfile reader, extraction, source-tree hashing or status promotion.
"""
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import time
import traceback
import zlib

ROOT = Path(__file__).absolute().parents[3]
OUT = Path(__file__).absolute().parent
AUX = 'STAGE114-FULL-SELECTION.json'


def require(ok, message):
    if not ok:
        raise ValueError(message)


def name_ok(name):
    require(isinstance(name, str) and name and '\x00' not in name and '\\' not in name, 'bad path')
    p = PurePosixPath(name)
    require(not p.is_absolute() and str(p) == name and not any(c in ('.', '..') for c in p.parts), 'noncanonical path')
    require(not any(c.endswith('.partial') or c.startswith(('stage86', 'stage116', 'stage117', 'stage118', 'stage119')) or 'private8' in c
                    or c in ('.git', '.cache', 'external', 'node_modules', '__pycache__') for c in p.parts), 'protected/excluded path')
    return p


def opened(name):
    parts = name_ok(name).parts
    fd = os.open(ROOT, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW)
    try:
        for c in parts[:-1]:
            child = os.open(c, os.O_RDONLY | os.O_DIRECTORY | os.O_NOFOLLOW, dir_fd=fd)
            os.close(fd)
            fd = child
        leaf = os.open(parts[-1], os.O_RDONLY | os.O_NOFOLLOW, dir_fd=fd)
        try:
            require(stat.S_ISREG(os.fstat(leaf).st_mode), 'nonregular input')
        except BaseException:
            os.close(leaf)
            raise
        return os.fdopen(leaf, 'rb')
    finally:
        os.close(fd)


def stable(a, b):
    require(all(getattr(a, k) == getattr(b, k) for k in
                ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_nlink', 'st_mtime_ns', 'st_ctime_ns')), 'input changed during read')


def load_pin(pin):
    with opened(pin['path']) as f:
        before = os.fstat(f.fileno())
        raw = f.read()
        stable(before, os.fstat(f.fileno()))
    actual = dict(path=pin['path'], bytes=len(raw), mode=stat.S_IMODE(before.st_mode), sha256=hashlib.sha256(raw).hexdigest())
    require(all(actual[k] == pin[k] for k in pin), 'input pin mismatch: ' + pin['path'])
    return raw, actual


class Inflated:
    """One exact gzip member, bounded decompression, compressed identity bookends."""
    def __init__(self, f):
        self.f, self.z = f, zlib.decompressobj(31)
        self.pending, self.buffer = b'', bytearray()
        self.sha, self.count, self.done = hashlib.sha256(), 0, False

    def read(self, size):
        while len(self.buffer) < size and not self.done:
            data = self.pending
            if not data:
                data = self.f.read(65536)
                if not data:
                    raise ValueError('truncated gzip stream')
                self.sha.update(data)
                self.count += len(data)
            block = self.z.decompress(data, 1024 * 1024)
            self.buffer.extend(block)
            self.pending = self.z.unconsumed_tail
            if self.z.eof:
                require(not self.z.unused_data and not self.pending, 'extra gzip member/trailing compressed bytes')
                require(not self.f.read(1), 'trailing compressed bytes')
                self.done = True
        n = min(size, len(self.buffer))
        answer = bytes(self.buffer[:n])
        del self.buffer[:n]
        return answer

    def exact(self, size):
        b = self.read(size)
        require(len(b) == size, 'truncated tar record')
        return b


def octal(raw):
    value = raw.rstrip(b'\0 ').lstrip(b' ')
    require(not value or all(c in b'01234567' for c in value), 'non-octal tar integer')
    return int(value or b'0', 8)


def pax_path(raw):
    fields = {}
    pos = 0
    while pos < len(raw):
        space = raw.find(b' ', pos)
        require(space > pos and raw[pos:space].isdigit(), 'malformed PAX length')
        n = int(raw[pos:space])
        require(n > space - pos + 1 and pos + n <= len(raw), 'PAX extent')
        row = raw[space + 1:pos + n]
        require(row.endswith(b'\n') and b'=' in row, 'malformed PAX row')
        key, value = row[:-1].split(b'=', 1)
        require(key not in fields, 'duplicate PAX field')
        fields[key] = value
        pos += n
    require(set(fields) == {b'path'}, 'unexpected PAX fields')
    return fields[b'path'].decode('utf-8')


def read_part(stream, expected, seen, archive_row, selection_pin, number, part_count):
    actual_names = []
    manifest = None
    next_path = None
    while True:
        header = stream.exact(512)
        if header == bytes(512):
            require(next_path is None and stream.exact(512) == bytes(512), 'bad tar end markers')
            while True:
                tail = stream.read(1024 * 1024)
                if not tail:
                    break
                require(not any(tail), 'nonzero bytes after tar EOF')
            break
        require(octal(header[148:156]) == sum(header[:148]) + 8 * 32 + sum(header[156:]), 'tar checksum mismatch')
        require(header[257:263] == b'ustar\0' and header[263:265] == b'00', 'unexpected tar format')
        raw_name = header[:100].split(b'\0', 1)[0]
        prefix = header[345:500].split(b'\0', 1)[0]
        name = ((prefix + b'/') if prefix else b'') + raw_name
        name = name.decode('utf-8')
        mode, uid, gid, size, mtime = [octal(header[a:b]) for a, b in [(100,108),(108,116),(116,124),(124,136),(136,148)]]
        kind = header[156:157]
        require(not uid and not gid and not mtime and not header[265:329].strip(b'\0'), 'unexpected tar owner/time')
        require(not header[157:257].strip(b'\0'), 'tar link target')
        if kind == b'x':
            require(next_path is None and size <= 1024 * 1024, 'invalid PAX record')
            next_path = pax_path(stream.exact(size))
            name_ok(next_path)
        else:
            require(kind in (b'0', b'\0'), 'nonregular tar payload')
            if next_path is not None:
                name = next_path
                next_path = None
            name_ok(name)
            if not actual_names:
                require(name == 'MANIFEST.json' and size <= 10 * 1024 * 1024 and mode == 0o644, 'part manifest missing')
                raw = stream.exact(size)
                require((len(raw), hashlib.sha256(raw).hexdigest()) == (archive_row['manifest']['bytes'], archive_row['manifest']['sha256']), 'manifest pin mismatch')
                manifest = json.loads(raw)
                require(manifest['index'] == number and manifest['parts'] == part_count and manifest['all_parts_required'] is True,
                        'manifest part metadata')
                require(manifest['selection'] == selection_pin and manifest['standalone_repository'] is False, 'manifest selection/scope')
                require(len(manifest['members']) == len(set(manifest['members'])), 'duplicate manifest member')
            else:
                require(name != 'MANIFEST.json' and name in expected and name not in seen, 'extra/duplicate payload: ' + name)
                want = expected[name]
                require((size, mode) == (want[0], want[1]), 'member size/mode mismatch: ' + name)
                sha, remaining = hashlib.sha256(), size
                while remaining:
                    chunk = stream.exact(min(remaining, 1024 * 1024))
                    sha.update(chunk)
                    remaining -= len(chunk)
                require(sha.hexdigest() == want[2], 'member raw hash mismatch: ' + name)
                seen.add(name)
            actual_names.append(name)
        padding = (-size) % 512
        require(not any(stream.exact(padding)), 'nonzero tar record padding')
    require(manifest is not None and actual_names == ['MANIFEST.json', *manifest['members']], 'part manifest member order/set mismatch')
    require(len(actual_names) == archive_row['logical_members_including_manifest'], 'part count mismatch')
    return len(actual_names)


def write_receipt(report):
    p = OUT / 'report.json'
    with p.open('x') as f:
        json.dump(report, f, ensure_ascii=True, indent=2, sort_keys=True)
        f.write('\n'); f.flush(); os.fsync(f.fileno())
    fd = os.open(OUT, os.O_RDONLY | os.O_DIRECTORY)
    try:
        os.fsync(fd)
    finally:
        os.close(fd)


def main():
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--binding', required=True)
    parser.add_argument('--binding-sha256', required=True)
    args = parser.parse_args()
    report = dict(kind='Stage114 independent manual-tar/zlib streaming archive readback',
                  complete=False, success=False, started_epoch=time.time(), errors=[], parts=[],
                  guest_compiler_Java_canonical_public_Git_writes=False)
    try:
        _, helper_pin = load_pin(dict(path=str(Path(__file__).relative_to(ROOT))))
        raw, binding_pin = load_pin(dict(path=args.binding, sha256=args.binding_sha256))
        binding = json.loads(raw)
        raw, producer_pin = load_pin(binding['producer'])
        producer = json.loads(raw)
        require(producer['complete'] and producer['success'] and not producer['errors']
                and not producer['floor_breaches'], 'producer not successful')
        raw, preflight_pin = load_pin(producer['preflight'])
        require(preflight_pin['sha256'] == binding['preflight_sha256'], 'preflight binding')
        preflight = json.loads(raw)
        require(preflight['complete'] and preflight['success'] and preflight['identity_errors'] == 0,
                'preflight failed')
        raw, selection_pin = load_pin(producer['selection'])
        require(selection_pin['sha256'] == binding['selection_sha256'], 'selection binding')
        selection = json.loads(raw)
        require(selection['complete'] and selection['standalone_repository'] is False, 'selection status')
        raw, config_pin = load_pin(selection['config'])
        require(config_pin['sha256'] == binding['config_sha256'], 'config binding')
        config = json.loads(raw)
        require(binding['full_m2_complete'] is False and config['standalone_repository'] is False, 'scope binding changed')
        require(config_pin['sha256'] == '71e11603506f148cb84311e6d9320b0979e51fd8a4ebc412c87cb005ac3e2781', 'Stage114 sealed config pin')
        require(producer['controller']['sha256'] == '9eb3eb5e37cba484dfe19c3154c10903a4d06fac0335f853f161d4528d4692b5', 'Stage114 sealed producer pin')
        require(config['mandatory_closure_review'] == 'target/m2/stage114-closeout-independent-review-01/report.json', 'mandatory review path')
        expected = {}
        for name, size, mode, sha in selection['members']:
            name_ok(name)
            require(name not in expected and type(size) is int and size >= 0 and type(mode) is int
                    and 0 <= mode <= 0o777 and len(sha) == 64
                    and all(c in '0123456789abcdef' for c in sha), 'invalid selection member')
            expected[name] = (size, mode, sha)
        require(len(expected) == producer['selected_payloads'] == preflight['selected_payloads']
                == binding['selected_payloads'], 'selected count')
        logical = sum(row[0] for row in expected.values())
        require(logical == preflight['logical_payload_bytes'] == binding['logical_payload_bytes'],
                'logical payload size')
        aliases, transformations = {}, []
        for dest, payload, size, mode, sha in selection['aliases']:
            name_ok(dest); name_ok(payload)
            require(dest not in expected and dest not in aliases and payload in expected,
                    'alias collision/chain/missing payload')
            require((size, mode, sha) == expected[payload] and type(mode) is int
                    and 0 <= mode <= 0o777, 'alias bytes/mode')
            aliases[dest] = (size, mode, sha)
            if mode != expected[payload][1]:
                transformations.append(dict(destination=dest, payload=payload, destination_mode=mode,
                                            payload_mode=expected[payload][1]))
        require(len(aliases) == producer['direct_restoration_aliases'] == binding['direct_restoration_aliases'] == 196064,
                'alias count')
        require(not transformations and binding['alias_mode_transformations'] == [], 'Stage114 aliases must preserve mode')
        scoped = config['acquisition_scoped_aliases']
        require(len(scoped) == 25 and all(aliases.get(dest) ==
                (row['identity']['bytes'], row['identity']['mode'], row['identity']['sha256'])
                and expected.get(row['payload']) == aliases[dest] for dest, row in scoped.items()),
                '25 retained acquisition alias identities')
        require(transformations == binding['alias_mode_transformations'], 'alias mode transformations')
        for catalog in selection['catalogs']:
            require(expected.get(catalog['path']) == (catalog['bytes'], catalog['mode'], catalog['sha256']),
                    'catalog not preserved exactly')
        require(any(p['path'] == 'target/m2/stage114-archive-prep-01/members.final.json' and p['sha256'] == 'b1457351afb10d71c0db05252a0eca828fccf3ff9fb8463acf168a5913fa2d51' for p in selection['catalogs']), 'sealed member catalog')
        require(any(p['path'] == 'target/m2/stage114-archive-prep-01/aliases.final.json' and p['sha256'] == '1e6f280aee49f8cd3212040844eb64c88e328837af600fd677c2c1deb674732f' for p in selection['catalogs']), 'sealed alias catalog')
        require(any(p['path'] == config['mandatory_closure_review'] for p in selection['catalogs']), 'independent review receipt not selected')
        for pin in binding.get('mandatory_payloads', []):
            actual = expected.get(pin['path'], aliases.get(pin['path']))
            require(actual == (pin['bytes'], pin['mode'], pin['sha256']), 'mandatory payload/alias missing')
        fixture_count = 0
        raw, fixture_plan_pin = load_pin(binding['fixture_plan'])
        require(expected.get(fixture_plan_pin['path'], aliases.get(fixture_plan_pin['path'])) ==
                (fixture_plan_pin['bytes'], fixture_plan_pin['mode'], fixture_plan_pin['sha256']),
                'fixture plan not archived')
        for row in json.loads(raw):
            identity = row['identity']
            require(expected.get(row['destination'], aliases.get(row['destination'])) ==
                    (identity['bytes'], identity['mode'], identity['sha256']), 'canonical fixture not recoverable')
            fixture_count += 1
        require(fixture_count == binding['canonical_fixture_count'] == 14609, 'fixture scope changed')
        expected[AUX] = (selection_pin['bytes'], selection_pin['mode'], selection_pin['sha256'])
        seen, compressed = set(), 0
        count_parts = len(producer['archives'])
        require(count_parts == binding['archive_parts'], 'part count')
        for number, archive in enumerate(producer['archives'], 1):
            pin = archive['archive']
            require(archive['index'] == number and pin['bytes'] <= 85 * 1024**2, 'archive order/cap')
            with opened(pin['path']) as stream:
                before = os.fstat(stream.fileno())
                require((before.st_size, stat.S_IMODE(before.st_mode)) == (pin['bytes'], pin['mode']),
                        'archive live size/mode')
                inflated = Inflated(stream)
                count = read_part(inflated, expected, seen, archive, selection_pin, number, count_parts)
                stable(before, os.fstat(stream.fileno()))
                require(inflated.done and (inflated.count, inflated.sha.hexdigest()) ==
                        (pin['bytes'], pin['sha256']), 'compressed archive identity')
                compressed += inflated.count
            report['parts'].append(dict(index=number, archive=pin, logical_members_including_manifest=count,
                                        raw_hash_size_mode_order=True, gzip_EOF_CRC_single_member_no_trailing=True))
            print(json.dumps(dict(part=number, parts=count_parts, verified_payloads=len(seen))), flush=True)
        require(seen == set(expected), 'global payload union differs')
        for pin in [binding_pin, producer_pin, preflight_pin, selection_pin, config_pin, helper_pin, fixture_plan_pin]:
            load_pin(pin)
        report.update(complete=True, success=True, binding=binding_pin, producer=producer_pin,
                      preflight=preflight_pin, selection=selection_pin, config=config_pin, controller=helper_pin,
                      selected_payloads=len(expected)-1, auxiliary_payloads=1, part_manifest_members=count_parts,
                      total_logical_regular_members=len(expected)+count_parts, logical_payload_bytes=logical,
                      compressed_bytes=compressed, exact_global_disjoint_union=True,
                      all_member_raw_bytes_size_mode_verified=True, all_archive_hashes_verified=True,
                      direct_restoration_aliases=len(aliases), alias_mode_transformations=transformations,
                      mandatory_payload_or_alias_count=len(binding.get('mandatory_payloads', [])), canonical_fixtures_verified=fixture_count,
                      alias_policy='Direct regular-file recovery with exact byte, size and mode identity; 25 retained acquisition aliases bound to their original payloads. No live alias rehash in this independent readback.',
                      independence='Manual tar/PAX and zlib gzip streaming. No producer import/functions or tarfile reader; preserved parser functions from the reviewed Stage104 verifier.',
                      known_limits=['No repository/toolchain or full workspace executable recovery claim.',
                                    'Prior published archive dependencies retain their earlier independent provenance and are not decompressed here.',
                                    'Unrecovered historical context and external prerequisites remain explicitly classified in the bound catalog.'],
                      full_M2_complete=False, full_API_complete=False, performance_targets_met=False,
                      hosted_CI_result_claimed=False)
    except BaseException as error:
        report['errors'].append(dict(error=repr(error), traceback=traceback.format_exc()))
    report['finished_epoch'] = time.time()
    write_receipt(report)
    print(json.dumps(dict(complete=report['complete'], success=report['success'], errors=report['errors'])), flush=True)
    return 0 if report['success'] else 1


if __name__ == '__main__':
    sys.exit(main())
