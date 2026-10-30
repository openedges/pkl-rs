"""Create only the ten new Stage121 evidence parts from the sealed finite catalog."""
import gzip
import hashlib
import io
import json
import os
from pathlib import Path
import shutil
import stat
import tarfile
import time

ROOT = Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage57')
CAT = ROOT / 'target/m2/stage121-publication-catalog-prep-01'
OUT = ROOT / 'target/m2/stage121-evidence-archives-01'
EXPECTED = '07c215e4ad02514be6573d48e62bdace77f4a888c461b7dd8853c4ca2c236218'
CAP = 85 * 1024**2


def safe(name):
    p = Path(name)
    if p.is_absolute() or '..' in p.parts:
        raise ValueError('relative catalog path required')
    if any(x == 'stage65-b-native-regressions-evidence.tar.gz.partial'
           or x.lower().startswith('stage86') or 'private8' in x.lower()
           for x in p.parts):
        raise ValueError('excluded name before metadata')
    if p.suffixes[-2:] == ['.tar', '.gz']:
        raise ValueError('old archive body is not an input')
    return ROOT / p


def read(record):
    p = safe(record['path'])
    fd = os.open(p, os.O_RDONLY | os.O_NOFOLLOW)
    with os.fdopen(fd, 'rb') as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise ValueError('regular input required')
        data = stream.read()
        after = os.fstat(stream.fileno())
    if (before.st_ino, before.st_size, before.st_mtime_ns, before.st_mode) != (
            after.st_ino, after.st_size, after.st_mtime_ns, after.st_mode):
        raise ValueError('input changed during capture')
    if (len(data), hashlib.sha256(data).hexdigest(), stat.S_IMODE(after.st_mode)) != (
            record['bytes'], record['sha256'], record['mode']):
        raise ValueError('catalog identity changed: ' + record['path'])
    return data


def pin(path):
    data = path.read_bytes()
    return dict(path=str(path.relative_to(ROOT)), bytes=len(data),
                mode=stat.S_IMODE(path.stat().st_mode),
                sha256=hashlib.sha256(data).hexdigest())


def write_json(name, value):
    with (OUT / name).open('x') as stream:
        json.dump(value, stream, indent=2)
        stream.write('\n')


def add(archive, name, data, mode):
    if Path(name).is_absolute() or '..' in Path(name).parts:
        raise ValueError('unsafe archive member')
    item = tarfile.TarInfo(name)
    item.size = len(data)
    item.mode = mode
    item.uid = item.gid = item.mtime = 0
    archive.addfile(item, io.BytesIO(data))


def main():
    seal = pin(CAT / 'readiness.json')
    assert seal['sha256'] == EXPECTED
    readiness = json.loads(read(seal))
    metadata = [seal, *readiness['files'], *readiness['supplemental_files']]
    for row in metadata:
        read(row)
    parts = json.loads((CAT / 'parts.json').read_bytes())
    aliases = {}
    with (CAT / 'paths.jsonl').open() as stream:
        for line in stream:
            row = json.loads(line)
            safe(row['path'])
            assert row['path'] not in aliases
            aliases[row['path']] = row
    objects = {r['sha256']: r for part in parts for r in part['members']}
    assert len(parts) == 10 and len(objects) == 13514 and len(aliases) == 222571
    assert sum(len(p['members']) for p in parts) == len(objects)
    for row in aliases.values():
        assert row['sha256'] in objects
        assert row['bytes'] == objects[row['sha256']]['bytes']
    if shutil.disk_usage(ROOT).free < 3 * 1024**3:
        raise ValueError('3 GiB admission floor')
    OUT.mkdir()
    producer = pin(Path(__file__))
    write_json('admission.json', dict(catalog=seal, metadata=metadata,
                                    aliases=len(aliases), objects=len(objects),
                                    producer=producer))
    started = time.monotonic()
    result = dict(complete=False, parts=[], logical_aliases=len(aliases),
                  unique_objects=len(objects), metadata_members=len(metadata),
                  old_archive_bodies_read=False, source_alias_modes_preserved=True,
                  aliases_freshly_rehashed=False, final_documents_archived=False)
    try:
        for part in parts:
            if shutil.disk_usage(ROOT).free < 2 * 1024**3:
                raise ValueError('2 GiB archive floor')
            number = part['number']
            output = OUT / f'stage121-pair-supertype-evidence-{number:02d}.tar.gz'
            with output.open('xb') as raw:
                with gzip.GzipFile(filename='', fileobj=raw, mode='wb',
                                   compresslevel=6, mtime=0) as compressed:
                    with tarfile.open(fileobj=compressed, mode='w|',
                                      format=tarfile.PAX_FORMAT) as archive:
                        if number == 1:
                            for row in metadata:
                                add(archive, row['path'], read(row), row['mode'])
                        for row in part['members']:
                            source = aliases[row['source']]
                            assert source['bytes'] == row['bytes']
                            assert source['sha256'] == row['sha256']
                            add(archive, row['member'], read(source), source['mode'])
                raw.flush()
                os.fsync(raw.fileno())
            identity = pin(output)
            if identity['bytes'] > CAP:
                raise ValueError('actual compressed part exceeds 85 MiB')
            result['parts'].append(identity)
            print(json.dumps(dict(part=number, bytes=identity['bytes'])), flush=True)
        for row in metadata:
            read(row)
        assert pin(Path(__file__)) == producer
        result.update(complete=True, compressed_bytes=sum(p['bytes'] for p in result['parts']),
                      catalog=seal, producer=producer)
    except BaseException as error:
        result['error'] = repr(error)
        raise
    finally:
        result['elapsed_seconds'] = time.monotonic() - started
        write_json('report.json', result)


if __name__ == '__main__':
    main()
