"""Preserve the exact original CLI case using the existing general module capture."""
from pathlib import Path
import json
import shutil
import sys
ROOT=Path(__file__).resolve().parents[3]
sys.path.insert(0,str(ROOT/'tools'))
import compat
import m2_module_eval
import oracle
OUT=Path(__file__).resolve().parent/'original-cli'
OUT.mkdir()
meta=json.loads((OUT.parent/'oracle.json').read_text())
original=next(c for c in json.loads((ROOT/'compatibility/corpus.json').read_text())['cases']if c['id'].endswith('parser/lambdaTrailingCommas.pkl'))
relative='external/pkl/'+original['source']['path']
workspace=OUT/'workspace'
source=workspace/relative
source.parent.mkdir(parents=True)
shutil.copyfile(ROOT/relative,source)
case=dict(id=original['id'],input=relative,input_sha256=original['source']['sha256'],upstream_case=original)
compat.write_json(OUT/'case.json',case)
observations=[]
for repeat in range(2):
 process,result=m2_module_eval.capture_process(ROOT,workspace,case,meta['command'],meta['environment'],OUT/('repeat-'+str(repeat)))
 observations.append(process)
compat.write_json(OUT/'report.json',dict(scope='Exact original corpus CLI arguments/cwd/environment overrides; minimal all-local input graph. Java-only, empty output establishes no executed Mixin behavior.',observations=observations,raw_repeat_equal=compat.load_capture(OUT/'repeat-0')['stdout']==compat.load_capture(OUT/'repeat-1')['stdout'] and compat.load_capture(OUT/'repeat-0')['stderr']==compat.load_capture(OUT/'repeat-1')['stderr'],input_sha256=oracle.sha256(source),original_case_sha256=compat.digest(compat.encoded(original)),oracle_sha256=oracle.sha256(OUT.parent/'oracle.json')))
print(json.dumps(observations,indent=2))
