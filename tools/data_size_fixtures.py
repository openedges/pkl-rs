"""Offline, pinned DataSize Java authority with independent raw capture checks."""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import tarfile
import tempfile

import data_size_oracle as oracle

DEFAULT = Path(__file__).resolve().parents[1] / 'tests/fixtures/m2-data-size'
ARCHIVE_SHA256 = '51e14a2c80836141c61a9bbb18aaa708e6a963c238132854e55e7bf6493c6b8e'
FILESET_SHA256 = '001ef10a6ee71f746179757c008dc46ce04d378b0d6d93814124016c06c1f2a6'
PROBE_SHA256 = 'a4389bb16f2cc784288dd79859afb2991ac574ac84f63319ad56c583f91cabbb'
INPUT_SHA256 = {'cases.json':'0e6f86ac0c9da84e7dce1b622bd99e2577b24cae4cf921ece7b6ef02a6a18a39',
    'module.pkl':'fba4e81343683bbe433660e168905537fbd7d6b0dd44699e2b61b321b0a23982',
    'queries.tsv':'2946ae533b25cb6ca76ab8694cd866cfb1781dfc9ec29b0c9584a28229ac4728'}
CAPTURE_SHA256 = {
    '01': {'cases.json': 'd9cb52cc21c273f67c0190d0b4f2733b13d6d7d75837bf04543cddfbe7a3c36a'},
    '02': {'cases.json': '486ee0aac4968922a17606c66a109811c4d136c287c6608507e62c6fce5f1ebb'},
    '03': {'provenance.json':'360a0247d75a1291751d88792b4e05b5542db30604e8201d4b2df862be4aa19b',
           'audit.json':'33c438b628015f6fcba9be3837b8cf126ec276c9b14807c9f1e265d5c6949e64',
           'inventory.tsv':'bd2e3abaff17150016071fc373b436978cea0ff6ea64f01212d53142b43b8b38'}
}
MEMBER_COUNTS = {'01':15853,'02':18454,'03':22556}

def require(ok, message):
    if not ok: raise ValueError(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def read(path):
    return json.loads(Path(path).read_bytes())

def compare_raw(a,b):
    left={p.relative_to(a).as_posix():sha(p.read_bytes()) for p in a.rglob('*') if p.is_file() and not p.name.startswith('process.')}
    right={p.relative_to(b).as_posix():sha(p.read_bytes()) for p in b.rglob('*') if p.is_file() and not p.name.startswith('process.')}
    require(left==right,'raw Java repetitions differ')
    return len(left)

def verify_history(path, number, source_root):
    p=read(path/'provenance.json'); m=read(path/'cases.json')
    require(p['complete'] is True and p['stable'] is True and p['repetition_raw_equal'] is True and p['repetition_file_set_equal'] is True,'historical process incomplete')
    require(p['artifact_sha256']==oracle.ARTIFACT_SHA256==p['artifact_after_sha256'],'historical JAR changed')
    require(p['reference_revision']=='e51fb2f91774265666f94980ec630db794a4db2d','historical revision changed')
    require(p['oracle_lock_sha256']==sha((source_root/'oracle.lock.json').read_bytes()) and p['references_lock_sha256']==sha((source_root/'references.lock').read_bytes()),'historical lock changed')
    require(p['input_before']==p['input_after'] and p['source_before']==p['source_after'],'historical input drift')
    for name,digest in p['input_before'].items(): require(sha((path/name).read_bytes())==digest,'historical input bytes changed')
    for name,digest in p['source_before'].items(): require(sha((source_root/name).read_bytes())==digest,'historical source bytes changed')
    rows=len(m['rows']);require(rows=={'01':240,'02':255}[number],'historical row count changed')
    require((path/'queries.tsv').read_text()==''.join(x['id']+'\t'+x['expression']+'\n' for x in m['rows']),'historical query order changed')
    require(len(p['repetitions'])==2 and [x['repeat'] for x in p['repetitions']]==[0,1],'historical process count changed')
    for entry in p['repetitions']:
        folder=path/f'repeat-{entry["repeat"]}'
        require(entry['exit_code']==0 and entry['query_directories']==rows,'historical process failed')
        original=f'/Users/cody.ij.hwang/Projects/pkl-rs.stage56/target/m2/stage57-datasize-oracle-{number}'
        require(entry['cwd']==original and entry['command'][-3:]==[f'{original}/repeat-{entry["repeat"]}',original,m['module_uri']], 'historical Java command/module URI changed')
        for channel in ('stdout','stderr'):
            raw=(folder/f'process.{channel}.bin').read_bytes()
            require(sha(raw)==entry[f'{channel}_sha256'] and len(raw)==entry[f'{channel}_bytes'],'historical process raw bytes changed')
    require(compare_raw(path/'repeat-0',path/'repeat-1')==p['repetition_files'],'historical raw file count changed')
    if number=='02':
        for sub, expected_runs in [('supplement',4),('hash-threshold',2)]:
            report=read(path/sub/'report.json')
            require(report['complete'] is True and len(report['runs'])==expected_runs and report['main_provenance_sha256']==sha((path/'provenance.json').read_bytes()),'historical supplement incomplete')
            for entry in report['runs']:
                require(entry['exit_code']==0,'historical supplement process failed')
            comparisons = report['comparison']
            comparisons = [comparisons] if 'differences' in comparisons else list(comparisons.values())
            for comp in comparisons:
                require(comp['differences']==[] and comp.get('raw_equal',True) is True,'historical supplement raw mismatch')
            cohorts=['language-identity','public-symbol'] if sub=='supplement' else ['.']
            for cohort in cohorts:
                base=path/sub/cohort
                left={p.relative_to(base/'repeat-0').as_posix():sha(p.read_bytes()) for p in (base/'repeat-0').rglob('*') if p.is_file()}
                right={p.relative_to(base/'repeat-1').as_posix():sha(p.read_bytes()) for p in (base/'repeat-1').rglob('*') if p.is_file()}
                reported=comparisons[0] if sub=='hash-threshold' else report['comparison'][cohort]
                require(left==right and len(left)==reported['files'],'historical supplement raw files differ')
            for entry in report['runs']:
                cohort=entry.get('kind','.')
                folder=path/sub/cohort/f'repeat-{entry["repeat"]}'
                require(sha((folder/'process.stdout.bin').read_bytes())==entry['stdout_sha256'] and
                        sha((folder/'process.stderr.bin').read_bytes())==entry['stderr_sha256'],
                        'historical supplement process bytes differ')
    return {'rows':rows,'repeated_raw_files':p['repetition_files'],'scope':'historical mixed string/export evidence; not common candidate boundary'}

def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256, expected_fileset=FILESET_SHA256):
    root=Path(root)
    manifest=read(root/'manifest.json')
    actual={p.name:sha(p.read_bytes()) for p in root.iterdir() if p.is_file() and p.name!='manifest.json'}
    require(manifest=={'schema':1,'kind':'pkl-java-data-size-split-v1','baseline':'Pkl 0.32.0','files':actual},'fixture manifest/files differ')
    require(set(actual)=={'README.md','archive-identity.json','cases.json','module.pkl','queries.tsv','oracle-evidence.tar.gz'},'fixture file set differs')
    archive=root/'oracle-evidence.tar.gz'
    require(sha(archive.read_bytes())==expected_archive,'independent archive SHA differs')
    with tempfile.TemporaryDirectory(prefix='pkl-data-size-authority-') as temporary:
        dest=Path(temporary);digest=hashlib.sha256();names=set();counts={n:0 for n in MEMBER_COUNTS}
        with tarfile.open(archive,'r:gz') as stream:
            for member in stream:
                name=member.name; parts=Path(name).parts
                require(member.isfile() and not Path(name).is_absolute() and '..' not in parts and name not in names,'unsafe/duplicate archive member')
                names.add(name);raw=stream.extractfile(member).read()
                digest.update(name.encode()+b'\0'+sha(raw).encode()+b'\n')
                for n in counts:
                    if name.startswith(f'capture-{n}/'): counts[n]+=1
                path=dest/name;path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(raw)
        require(digest.hexdigest()==expected_fileset and counts==MEMBER_COUNTS and len(names)==56870,'independent archive file set differs')
        identity=read(root/'archive-identity.json')
        require(identity['archive_sha256']==expected_archive and identity['fileset_sha256']==expected_fileset and identity['captures']==counts and identity['archive_members']==len(names),'archive identity report differs')
        for name,h in INPUT_SHA256.items():
            require(actual[name]==h==sha((dest/'capture-03'/name).read_bytes()),'canonical input differs')
        require(sha((oracle.ROOT/'tools/java/DataSizeSplitOracle.java').read_bytes())==PROBE_SHA256==sha((dest/'capture-03/DataSizeSplitOracle.java').read_bytes()),'executed probe bytes differ')
        for n,entries in CAPTURE_SHA256.items():
            for name,h in entries.items(): require(sha((dest/f'capture-{n}'/name).read_bytes())==h,f'capture {n} fixed pin differs')
        source_root=dest/'sources'
        for name,h in identity['sources'].items(): require(sha((source_root/name).read_bytes())==h,'snapshot source differs')
        histories={n:verify_history(dest/f'capture-{n}',n,source_root) for n in ('01','02')}
        try: audit=oracle.audit_split(dest/'capture-03',source_root)
        except AssertionError as error: raise ValueError(f'canonical raw Java audit failed: {error}') from error
        require(audit['query_rows']==255 and audit['error_count']==22 and audit['exported_datasize_rows']==134,'canonical scope differs')
        return {'kind':manifest['kind'],'archive_sha256':expected_archive,'fileset_sha256':expected_fileset,
                'canonical':{'string_sessions':2,'export_sessions':2,'query_rows':255,'string_calls_per_session':255,
                             'export_calls_per_session':389,'intentional_error_rows':22,'exported_datasize_rows':134,
                             'common_candidate_boundary':'string status, exact stdout/stderr UTF-8 and UTF-16BE, complete process protocol',
                             'public_export_boundary':'required: exported value type, bits, unit, hash, identity and errors; no candidate match claimed'},
                'history':histories,'compatibility_passes_claimed':0}

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--fixture',type=Path,default=DEFAULT)
    args=parser.parse_args();print(json.dumps(verify(args.fixture),sort_keys=True))
