"""Acquire separate pinned Java 0.32.0 implicit reversal constraint observations."""
import argparse
from collections import Counter
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import time

from reversal_values_oracle import SOURCES as REVERSAL_SOURCES, sha, save, files

ROOT=Path(__file__).resolve().parents[1]
FIXTURE=ROOT/'tests/fixtures/m2-reversal-implicit'
MODULE_URI='file:///stage66/reversal-implicit.pkl'
CASES_SHA256='06b6d407cb7f7049eb83ed9fd8cf2af0801accaf87064915157743694bd95cfc'
MODULE_SHA256='ad7ab1ce7bc50d525b29b10b8db1c5247623ce9bce2da705cd9ee0d01723f8ca'
QUERIES_SHA256='2d05613d0794ca668a54a9ae9d8f9d9938776f6471d08bb24f58a52044755464'
INPUTS=('CollectionConversionsOracle.java','run.py','module.pkl','queries.tsv','cases.json')
SOURCES=tuple(dict.fromkeys((*REVERSAL_SOURCES,
    'tools/reversal_values_oracle.py',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/TypeConstraintNode.java',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/TypeNode.java',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/type/VmTypeMismatchException.java',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/AbstractInvokeMethodLexicalNode.java',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/InvokeObjectMethodNode.java',
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/expression/member/InvokeClassMethodNode.java',
    'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/basic/as2.pkl',
    'external/pkl/pkl-core/src/test/files/LanguageSnippetTests/input/types/typeAliasConstraint3.pkl')))


def require(ok, message):
    if not ok: raise ValueError(message)


def acquire(output):
    output=Path(output).resolve()
    require(not output.exists(),'fresh implicit reversal output required')
    lock=json.loads((ROOT/'oracle.lock.json').read_bytes())
    require(lock['version']=='0.32.0' and lock['source']['revision']=='e51fb2f91774265666f94980ec630db794a4db2d',
            'pinned oracle source differs')
    jar=ROOT/'.cache/oracle'/lock['artifact']['filename']
    require(sha(jar)==lock['artifact']['sha256'],'pinned oracle JAR differs')
    cases=json.loads((FIXTURE/'cases.json').read_bytes());rows=cases['rows']
    require(cases['schema']==1 and cases['baseline']=='Pkl 0.32.0' and cases['module_uri']==MODULE_URI
            and cases['boundary']=='string' and cases['repetitions']==2 and len(rows)==8
            and all(row['mode']=='string' for row in rows)
            and sha(FIXTURE/'cases.json')==CASES_SHA256 and sha(FIXTURE/'module.pkl')==MODULE_SHA256
            and sha(FIXTURE/'queries.tsv')==QUERIES_SHA256,'authored implicit input differs')
    require((FIXTURE/'queries.tsv').read_text()==''.join(
        f"string\t{row['id']}\t{row['expression']}\n" for row in rows),'ordered queries differ')
    output.mkdir()
    for name,source in (('CollectionConversionsOracle.java',ROOT/'tools/java/CollectionConversionsOracle.java'),
                        ('run.py',ROOT/'tools/reversal_implicit_oracle.py'),
                        ('module.pkl',FIXTURE/'module.pkl'),('queries.tsv',FIXTURE/'queries.tsv'),
                        ('cases.json',FIXTURE/'cases.json')):
        shutil.copyfile(source,output/name)
    for name in SOURCES:
        dest=output/'sources'/name;dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(ROOT/name,dest)
    java=Path(os.environ.get('PKL_ORACLE_JAVA','/usr/bin/java')).resolve()
    env={'PATH':os.defpath,'HOME':str(ROOT/'.cache/oracle/home'),'TMPDIR':str(ROOT/'.cache/oracle/tmp'),
         'TMP':str(ROOT/'.cache/oracle/tmp'),'TEMP':str(ROOT/'.cache/oracle/tmp'),
         'LC_ALL':'C','LANG':'C','TZ':'UTC','TERM':'dumb','NO_COLOR':'1','COLUMNS':'80'}
    identity=subprocess.run([str(java),'-XshowSettings:properties','-version'],cwd=output,env=env,capture_output=True,timeout=60)
    (output/'java-identity.stdout.bin').write_bytes(identity.stdout)
    (output/'java-identity.stderr.bin').write_bytes(identity.stderr)
    require(identity.returncode==0,'JDK identity failed; raw retained')
    properties=dict(re.findall(r'^\s+([\w.]+) = (.*)$',identity.stderr.decode(),re.M))
    require(all(properties.get(k)==v for k,v in lock['jdk']['properties'].items()),'JDK identity differs')
    home=Path(properties['java.home'])
    jdk={'executable':str(java),'executable_sha256':sha(java),'modules_sha256':sha(home/'lib/modules'),
         'release_sha256':sha(home/'release'),'source_archive_sha256':sha(home/'lib/src.zip'),
         'properties':properties}
    provenance={'schema':1,'scope':'Independent public implicit reversal type-constraint Java string observations; no Rust candidate',
                'host':platform.platform(),'reference_revision':lock['source']['revision'],
                'oracle_lock_sha256':sha(ROOT/'oracle.lock.json'),'references_lock_sha256':sha(ROOT/'references.lock'),
                'artifact':str(jar),'artifact_sha256':sha(jar),'jdk':jdk,'environment':env,
                'input_before':{name:sha(output/name) for name in INPUTS},
                'source_before':{name:sha(ROOT/name) for name in SOURCES},'sessions':[]}
    save(output/'provenance.json',provenance)
    options=[*lock['jvm_options'],f"-Duser.home={env['HOME']}",f"-Djava.io.tmpdir={env['TMPDIR']}"]
    for repeat in range(2):
        folder=output/f'repeat-{repeat}';folder.mkdir()
        command=[str(java),*options,'-cp',str(jar),str(output/'CollectionConversionsOracle.java'),
                 str(folder),str(output),MODULE_URI]
        started=time.time();process=subprocess.Popen(command,cwd=output,env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE)
        try:stdout,stderr=process.communicate(timeout=180)
        except subprocess.TimeoutExpired:
            process.kill();stdout,stderr=process.communicate()
            (folder/'process.stdout.bin').write_bytes(stdout);(folder/'process.stderr.bin').write_bytes(stderr)
            raise ValueError('Java session timed out; raw retained')
        (folder/'process.stdout.bin').write_bytes(stdout);(folder/'process.stderr.bin').write_bytes(stderr)
        session={'repeat':repeat,'pid':process.pid,'command':command,'cwd':str(output),'exit_code':process.returncode,
                 'started_epoch':started,'finished_epoch':time.time(),'stdout_sha256':sha(folder/'process.stdout.bin'),
                 'stderr_sha256':sha(folder/'process.stderr.bin'),'stdout_bytes':len(stdout),'stderr_bytes':len(stderr),
                 'query_directories':sum(path.is_dir() for path in folder.iterdir())}
        provenance['sessions'].append(session);save(output/'provenance.json',provenance)
        require(process.returncode==0 and session['query_directories']==len(rows),'Java session failed; raw retained')
    provenance['input_after']={name:sha(output/name) for name in INPUTS}
    provenance['source_after']={name:sha(ROOT/name) for name in SOURCES}
    provenance['artifact_after_sha256']=sha(jar)
    provenance['jdk_source_after_sha256']=sha(home/'lib/src.zip')
    provenance['stable']=(provenance['input_before']==provenance['input_after'] and
                          provenance['source_before']==provenance['source_after'] and
                          provenance['artifact_sha256']==provenance['artifact_after_sha256'] and
                          jdk['source_archive_sha256']==provenance['jdk_source_after_sha256'])
    first,second=files(output/'repeat-0'),files(output/'repeat-1')
    provenance['raw_equal']=first==second
    provenance['raw_files']=len(first)
    provenance['raw_differences']=[name for name in sorted(first.keys()|second.keys()) if first.get(name)!=second.get(name)]
    provenance['complete']=provenance['stable'] and provenance['raw_equal'] and len(provenance['sessions'])==2
    save(output/'provenance.json',provenance)
    observations=[]
    for index,row in enumerate(rows):
        folder=output/'repeat-0'/f"{index:03d}-{row['id']}"
        status=(folder/'result.status.bin').read_text().strip()
        stderr=(folder/'result.stderr.bin').read_text()
        observations.append({'id':row['id'],'group':row['group'],'mode':row['mode'],'expression':row['expression'],
                             'status':status,'stdout_sha256':sha(folder/'result.stdout.bin'),
                             'stderr_sha256':sha(folder/'result.stderr.bin'),
                             'stdout':(folder/'result.stdout.bin').read_text(),
                             'first_error_line':stderr.splitlines()[1] if len(stderr.splitlines())>1 else '',
                             'error_class':(folder/'result.error-class.bin').read_text()})
    audit={'schema':1,'scope':provenance['scope'],'rows':len(observations),
           'statuses':dict(Counter(row['status'] for row in observations)),
           'observations':observations,'candidate_compatibility_passes_claimed':0,
           'limits':['Authored public calls only; original upstream snippets are source evidence, not replayed modules.',
                     'Rust native, browser and Node raw comparison, typed candidate export, full M2 and performance gates remain unverified.']}
    save(output/'audit.json',audit)
    require(provenance['complete'],'implicit reversal acquisition incomplete; raw retained')
    print(json.dumps({'rows':len(observations),'statuses':audit['statuses'],'raw_equal':True,'stable':True}))

if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    acquire(args.output)
