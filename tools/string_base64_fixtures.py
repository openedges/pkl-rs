"""Offline verifier for pinned Pkl 0.32.0 public String Base64 calls."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import tarfile

from collection_conversions_fixtures import validate_capture
from string_properties_oracle import SOURCES as STRING_PROPERTY_SOURCES

def project_root():
    for parent in Path(__file__).resolve().parents:
        if (parent / 'oracle.lock.json').is_file():
            return parent
    raise ValueError('project root missing')


ROOT = project_root()
ARCHIVE_PREFIX = 'capture-02'
DEFAULT = (ROOT / 'tests/fixtures/m2-string-base64'
           if Path(__file__).resolve().parent == ROOT / 'tools'
           else ROOT / 'target/m2/stage68-fixture-draft-01/fixture')
ARCHIVE_SHA256 = '8b0a7d2f4a7b1e3b29218b3900ec7b0a4913799ee1224766391b80c400c832f0'
FILESET_SHA256 = '8ee2a4988a818177ba18ae354567acba343c1b673e62e18fc3c137368642f257'
MEMBERS = 3833
ROWS = 89
STATUSES = {'success': 67, 'error': 22}
RAW_FILES = 1871
MODULE_URI = 'file:///stage68/string-base64-exploration.pkl'
PRODUCER_SHA256 = 'cba17c0f83775f9a6f23904eed2f2effb5b074094a652578a64653c0f0d5a2a9'
JDK_SOURCE_SHA256 = '173282dd3f4c4004548e6a09e6687acff25120967b1a8375a5df4c677d7f4e4e'
JDK_ENTRIES = {'java.base/java/util/Base64.java', 'java.base/java/lang/String.java',
               'java.base/java/lang/StringCoding.java'}
SOURCES = (*STRING_PROPERTY_SOURCES,
           'external/pkl/pkl-core/src/main/java/org/pkl/core/util/ByteArrayUtils.java')
CAPTURES = {
    'capture-02': 'target/m2/stage68-base64-oracle-02',
    'successful-launch-02': 'target/m2/stage68-base64-launch-02',
    'successful-prep-02': 'target/m2/stage68-base64-prep-02',
    'failed-launch-01': 'target/m2/stage68-base64-launch-01',
    'failed-prep-01': 'target/m2/stage68-base64-prep-01',
}
ROOT_FILES = {'README.md', 'archive-identity.json', 'cases.json', 'module.pkl',
              'queries.tsv', 'oracle-evidence.tar.gz', 'manifest.json'}

def need(ok, message):
    if not ok:
        raise ValueError(message)

def sha(data): return hashlib.sha256(data).hexdigest()

def identity(data, *, mode=None):
    result = {'bytes':len(data), 'sha256':sha(data)}
    if mode is not None: result['mode'] = mode
    return result

def verify(root=DEFAULT, *, expected_archive=ARCHIVE_SHA256,
           expected_fileset=FILESET_SHA256):
    root=Path(root)
    need(root.is_dir() and not root.is_symlink(), 'fixture root missing or linked')
    entries=list(root.iterdir())
    need({p.name for p in entries} == ROOT_FILES and
         all(p.is_file() and not p.is_symlink() for p in entries), 'fixture root differs')
    hashes={p.name:sha(p.read_bytes()) for p in entries if p.name!='manifest.json'}
    manifest=json.loads((root/'manifest.json').read_bytes())
    need(manifest == {'schema':1,'baseline':'Pkl 0.32.0','files':hashes},
         'fixture root manifest differs')
    need(hashes['oracle-evidence.tar.gz']==expected_archive, 'archive identity differs')
    archive_id=json.loads((root/'archive-identity.json').read_bytes())
    need(archive_id['schema']==1 and archive_id['baseline']=='Pkl 0.32.0' and
         archive_id['archive_sha256']==expected_archive and
         archive_id['fileset_sha256']==expected_fileset and archive_id['members']==MEMBERS and
         archive_id['captures']==CAPTURES and
         archive_id['candidate_compatibility_passes_claimed']==0,
         'declared archive identity differs')
    for name in ('cases.json','module.pkl','queries.tsv'):
        data=(root/name).read_bytes()
        record={'bytes':len(data),'sha256':sha(data),'mode':(root/name).stat().st_mode & 0o777}
        need(archive_id['root_inputs'][name]==record, 'root input identity differs: '+name)
    members={}; fileset=hashlib.sha256(); by_prefix={prefix:0 for prefix in CAPTURES}
    with tarfile.open(root/'oracle-evidence.tar.gz','r:gz') as source:
        for member in source:
            name=member.name
            parts=Path(name).parts
            need(member.isfile() and member.mode==0o644 and not Path(name).is_absolute()
                 and '..' not in parts and len(parts)>1 and parts[0] in CAPTURES
                 and name not in members, 'unsafe, duplicate or unapproved archive member')
            data=source.extractfile(member).read()
            need(len(data)==member.size, 'archive member size differs')
            members[name]=data;by_prefix[parts[0]]+=1
            fileset.update(name.encode()+b'\0'+sha(data).encode()+b'\n')
    need(len(members)==MEMBERS and fileset.hexdigest()==expected_fileset,
         'full archive inventory differs')
    need(by_prefix=={'capture-02':3817,'successful-launch-02':4,
                     'successful-prep-02':4,'failed-launch-01':4,'failed-prep-01':4},
         'archive history member counts differ')
    for name in ('cases.json','module.pkl','queries.tsv'):
        need(members['capture-02/'+name]==members['successful-prep-02/'+name]
             ==members['failed-prep-01/'+name]==(root/name).read_bytes(),
             'authored input changed: '+name)
    captured_run=members['capture-02/run.py']
    need(sha(captured_run)==PRODUCER_SHA256
         and captured_run==members['successful-prep-02/run.py']
         and archive_id['captured_producer']==identity(captured_run,mode=0o644),
         'successful producer identity differs')
    need(archive_id['java_helper']==
         identity(members['capture-02/CollectionConversionsOracle.java'],mode=0o644),
         'Java helper identity differs')
    need(len(SOURCES)==63 and len(set(SOURCES))==63, 'pinned source list differs')
    statuses=validate_capture(members,'capture-02',mode='string',rows_count=ROWS,
                              expected_statuses=STATUSES,expected_raw_files=RAW_FILES,
                              module_uri=MODULE_URI,source_names=SOURCES)
    provenance=json.loads(members['capture-02/provenance.json'])
    need(provenance['jdk']['source_archive_sha256']==JDK_SOURCE_SHA256
         and provenance['jdk_source_after_sha256']==JDK_SOURCE_SHA256
         and set(provenance['jdk_sources_before'])==JDK_ENTRIES
         and provenance['jdk_sources_before']==provenance['jdk_sources_after'],
         'pinned JDK source closure differs')
    for name, expected in provenance['jdk_sources_before'].items():
        need(sha(members['capture-02/jdk-sources/'+name])==expected,
             'JDK source bytes differ: '+name)
    for attempt,exit_code in (('successful-launch-02',0),('failed-launch-01',1)):
        report=json.loads(members[attempt+'/report.json'])
        need(report['complete'] is True and report['input_stable'] is True
             and report['exit_code']==exit_code
             and report['candidate_compatibility_passes_claimed']==0
             and report['inputs_before']==report['inputs_after'],
             'outer process status/source differs: '+attempt)
        for channel in ('stdout','stderr'):
            need(report[channel]==identity(members[attempt+'/'+channel+'.bin']),
                 'outer raw process channel differs: '+attempt+'/'+channel)
        prep='successful-prep-02' if exit_code==0 else 'failed-prep-01'
        for name in ('run.py','cases.json','module.pkl','queries.tsv'):
            key=CAPTURES[prep]+'/'+name
            need(report['inputs_before'][key]==identity(members[prep+'/'+name]),
                 'outer input source differs: '+key)
        key=CAPTURES[attempt]+'/run.py'
        need(report['inputs_before'][key]==identity(members[attempt+'/run.py']),
             'outer launcher source differs: '+key)
        if exit_code==0:
            need(report['audit']==identity(members['capture-02/audit.json'])
                 and report['provenance']==identity(members['capture-02/provenance.json'])
                 and members[attempt+'/stderr.bin']==b'',
                 'successful outer receipt differs')
        else:
            need(report['audit'] is None and report['provenance'] is None
                 and members[attempt+'/stdout.bin']==b''
                 and b'ValueError: authored row set differs' in members[attempt+'/stderr.bin'],
                 'first failed attempt classification differs')
    need(archive_id['first_attempt']=={'launch_exit_code':1,'java_sessions_started':0,
                                      'classification':'stale authored-case hash rejected before Java'}
         and archive_id['canonical_capture']=={'rows':89,'successes':67,
                                               'ordinary_errors':22,'jvm_sessions':2},
         'public boundary/status metadata differs')
    return {'baseline':'Pkl 0.32.0','module_uri':MODULE_URI,'rows':ROWS,
            'statuses':statuses,'members':MEMBERS,'archive_sha256':expected_archive,
            'fileset_sha256':expected_fileset,'raw_repetitions_equal':True,
            'first_attempt_preserved_as_failure':True,
            'candidate_compatibility_passes_claimed':0}

if __name__=='__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--fixture', type=Path)
    args = parser.parse_args()
    print(json.dumps(verify(args.fixture), indent=2, sort_keys=True))
