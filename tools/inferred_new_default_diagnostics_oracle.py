#!/usr/bin/env python3
"""Capture four complete Java 0.32.0 method default diagnostics in fresh JVMs."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time

ROOT=Path(__file__).resolve().parents[1]
INPUTS=ROOT/'tests/fixtures/m2-inferred-new-default-diagnostics'
OUT:Path
sys.path.insert(0,str(ROOT/'tools'))
from inferred_new_module_receiver_oracle import SOURCES as BASE_SOURCES, REVISION
SOURCES=tuple(sorted(set(BASE_SOURCES)|{
    'external/pkl/pkl-core/src/main/java/org/pkl/core/ast/member/DefaultPropertyBodyNode.java',
    'tools/inferred_new_default_diagnostics_oracle.py',
}))


def sha(path:Path)->str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save(path:Path,value:object)->None:
    path.write_text(json.dumps(value,indent=2,sort_keys=True)+'\n')


def file_map(folder:Path)->dict:
    return {p.relative_to(folder).as_posix():sha(p) for p in sorted(folder.rglob('*')) if p.is_file()}


def main(output:Path)->None:
    global OUT
    OUT=output.resolve()
    if OUT.exists():
        raise ValueError('fresh Java output required')
    cases=json.loads((INPUTS/'cases.json').read_bytes())
    if cases['schema']!=1 or cases['baseline']!='Pkl 0.32.0' or cases['cases']!=[
        'literal-unknown','any','alias-unknown','generic-alias-unknown']:
        raise ValueError('authored diagnostic cases differ')
    input_before=file_map(INPUTS)
    source_before={name:sha(ROOT/name) for name in SOURCES}
    lock=json.loads((ROOT/'oracle.lock.json').read_bytes())
    jar=ROOT/'.cache/oracle'/lock['artifact']['filename']
    if lock['version']!='0.32.0' or lock['source']['revision']!=REVISION or sha(jar)!=lock['artifact']['sha256']:
        raise ValueError('pinned Java artifact differs')
    OUT.mkdir()
    shutil.copyfile(Path(__file__),OUT/'run.py')
    shutil.copyfile(ROOT/'tools/java/CollectionConversionsOracle.java',OUT/'CollectionConversionsOracle.java')
    for path in sorted(INPUTS.rglob('*')):
        if path.is_file():
            dest=OUT/'inputs'/path.relative_to(INPUTS)
            dest.parent.mkdir(parents=True,exist_ok=True)
            shutil.copyfile(path,dest)
    for name in SOURCES:
        dest=OUT/'sources'/name
        dest.parent.mkdir(parents=True,exist_ok=True)
        shutil.copyfile(ROOT/name,dest)
    if {name:sha(OUT/'sources'/name) for name in SOURCES}!=source_before:
        raise ValueError('copied source differs')
    java=Path(os.environ.get('PKL_ORACLE_JAVA','/usr/bin/java')).resolve()
    env={'PATH':os.defpath,'HOME':str(ROOT/'.cache/oracle/home'),
         'TMPDIR':str(ROOT/'.cache/oracle/tmp'),'TMP':str(ROOT/'.cache/oracle/tmp'),
         'TEMP':str(ROOT/'.cache/oracle/tmp'),'LC_ALL':'C','LANG':'C','TZ':'UTC',
         'TERM':'dumb','NO_COLOR':'1','COLUMNS':'80'}
    identity=subprocess.run([str(java),'-XshowSettings:properties','-version'],cwd=OUT,
                            env=env,capture_output=True,timeout=60)
    (OUT/'java-identity.stdout.bin').write_bytes(identity.stdout)
    (OUT/'java-identity.stderr.bin').write_bytes(identity.stderr)
    properties=dict(re.findall(r'^\s+([\w.]+) = (.*)$',identity.stderr.decode(),re.M))
    if identity.returncode or any(properties.get(k)!=v for k,v in lock['jdk']['properties'].items()):
        raise ValueError('JDK identity differs; raw streams retained')
    home=Path(properties['java.home'])
    jdk={'executable':str(java),'executable_sha256':sha(java),
         'modules_sha256':sha(home/'lib/modules'),'release_sha256':sha(home/'release'),
         'properties':properties}
    options=[*lock['jvm_options'],f"-Duser.home={env['HOME']}",f"-Djava.io.tmpdir={env['TMPDIR']}"]
    report={'schema':1,'kind':'stage65-Java-method-default-diagnostics',
            'complete':False,'candidate_compatibility_passes_claimed':0,
            'input_before':input_before,'source_before':source_before,
            'artifact_before':sha(jar),'oracle_lock_sha256':sha(ROOT/'oracle.lock.json'),
            'references_lock_sha256':sha(ROOT/'references.lock'),
            'environment':env,'jdk':jdk,'sessions':[],'observations':{}}
    save(OUT/'report.json',report)
    for label in cases['cases']:
        folder=OUT/'inputs'/label
        uri=(folder/'module.pkl').as_uri()
        for repeat in range(2):
            target=folder/f'repeat-{repeat}';target.mkdir()
            argv=[str(java),*options,'-cp',str(jar),str(OUT/'CollectionConversionsOracle.java'),
                  str(target),str(folder),uri]
            start=time.time()
            process=subprocess.Popen(argv,cwd=OUT,env=env,stdout=subprocess.PIPE,
                                     stderr=subprocess.PIPE)
            timeout=False
            try:stdout,stderr=process.communicate(timeout=240)
            except subprocess.TimeoutExpired:
                timeout=True;process.kill();stdout,stderr=process.communicate()
            (target/'process.stdout.bin').write_bytes(stdout)
            (target/'process.stderr.bin').write_bytes(stderr)
            rows=[p for p in target.iterdir() if p.is_dir()]
            report['sessions'].append({'case':label,'repeat':repeat,'pid':process.pid,
                                       'argv':argv,'cwd':str(OUT),'module_uri':uri,
                                       'started_epoch':start,'finished_epoch':time.time(),
                                       'exit_code':process.returncode,'timed_out':timeout,
                                       'stdout_sha256':sha(target/'process.stdout.bin'),
                                       'stderr_sha256':sha(target/'process.stderr.bin'),
                                       'row_directories':len(rows)})
            save(OUT/'report.json',report)
            if process.returncode or timeout or len(rows)!=1:
                raise ValueError('Java process incomplete; raw retained')
        report['observations'][label]=[]
        for repeat in range(2):
            target=folder/f'repeat-{repeat}'/'000-result'
            stderr=(target/'result.stderr.bin').read_text()
            report['observations'][label].append({
                'repeat':repeat,'status':(target/'result.status.bin').read_text().strip(),
                'stdout':(target/'result.stdout.bin').read_text(),
                'stderr_sha256':sha(target/'result.stderr.bin'),
                'first_error_line':stderr.splitlines()[1] if len(stderr.splitlines())>1 else '',
            })
        save(OUT/'report.json',report)
    report['input_after']=file_map(INPUTS)
    report['source_after']={name:sha(ROOT/name) for name in SOURCES}
    report['source_copies_after']={name:sha(OUT/'sources'/name) for name in SOURCES}
    report['artifact_after']=sha(jar)
    report['source_stable']=report['source_before']==report['source_after']==report['source_copies_after']
    report['input_stable']=report['input_before']==report['input_after']
    report['raw_equal']=all(file_map(OUT/'inputs'/label/'repeat-0')==file_map(OUT/'inputs'/label/'repeat-1') for label in cases['cases'])
    report['complete']=report['source_stable'] and report['input_stable'] and report['raw_equal'] and all(
        s['exit_code']==0 and not s['timed_out'] for s in report['sessions'])
    save(OUT/'report.json',report)
    print(json.dumps({'complete':report['complete'],'observations':report['observations']},sort_keys=True))
    if not report['complete']:
        raise SystemExit(1)


if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    main(args.output)
