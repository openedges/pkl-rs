"""Export Java-only typed resource portable inputs, raw expectations and provenance."""
from pathlib import Path
import argparse,hashlib,json,re,shutil,struct
PINS={'resource':('stage46-resource-oracle-01','a7b35392e82804c581a00ccc19689c376917c4552fb46639248e06b1033e60a2'),'authority':('stage46-uri-authority-oracle-01','6847d1a4e94dbae770da0e4bcb8f2ed9bd40ba8725cf5448f5966018dc5ffc2f'),'base64':('stage46-base64-oracle-01','a297a1277f58a675a0780f8b5f0fd0893a48400b7bcc7c8c96f394e63e1474b0'),'extra':('stage46-resource-extra-oracle-01','5c4531248ead92e0ee27a48b5a2fef912b4e27318084824e32e65cf23216be31')}
IDS={'cache-text-resource-missing-retry','resource-bytes-utf8','authority-key-pairs','base64-decoder','missing-and-io-nullable-first'}
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def units(data):
 if len(data)%2:raise ValueError('odd UTF16 length')
 return list(struct.unpack('>'+'H'*(len(data)//2),data))
def eligible(case,call):return case in IDS and call['source']=='main.pkl'and not(case=='base64-decoder'and call['expression']in {'propValue','propNullable'})
def java_utf8(words):
 result=bytearray();index=0
 while index<len(words):
  word=words[index]
  if type(word)is not int or not 0<=word<=65535:raise ValueError('invalid UTF16 word')
  if 0xD800<=word<=0xDBFF and index+1<len(words)and type(words[index+1])is int and 0xDC00<=words[index+1]<=0xDFFF:
   code=0x10000+((word-0xD800)<<10)+(words[index+1]-0xDC00);result.extend(chr(code).encode());index+=2
  else:
   result.extend(b'?'if 0xD800<=word<=0xDFFF else chr(word).encode());index+=1
 return bytes(result)
def parse_status(raw):
 if raw==b'success\n':return 'success'
 if raw==b'error\n':return 'error'
 raise ValueError('unknown exact Java API status')

def emit(base,out):
 out.mkdir(parents=True,exist_ok=False);bindings={};rules=[];provenance={};fixtures=[];unexecuted=[];api_total=0;session_sources=[]
 def copy(src,relative,kind):
  dst=out/relative;dst.parent.mkdir(parents=True,exist_ok=True)
  if not dst.exists():shutil.copyfile(src,dst)
  if src.read_bytes()!=dst.read_bytes():raise ValueError('copied input differs')
  bindings[str(src)]={'path':relative,'sha256':sha(src),'bytes':src.stat().st_size,'kind':kind};return relative
 for group,(name,pin)in PINS.items():
  oracle=base/name;report_path=oracle/'report.json'
  if sha(report_path)!=pin:raise ValueError('Java report differs')
  report=json.loads(report_path.read_bytes());api_total+=report['apiCalls'];cases=json.loads((oracle/'cases.json').read_bytes());byid={c['id']:c for c in cases}
  for n in ['report.json','cases.json','inventory.json','closure.json','profile.json','interpretation.json']:
   p=oracle/n
   if p.is_file():copy(p,'provenance/'+group+'/'+n,'provenance')
  helper={'resource':'ResourceProbe.java','authority':'AuthorityProbe.java','base64':'Base64Probe.java','extra':'ExtraProbe.java'}[group];helper_path=oracle/helper;copy(helper_path,'provenance/'+group+'/'+helper,'Java helper')
  provenance[group]={'originalRoot':str(oracle),'reportSha256':pin,'helperSha256':sha(helper_path),'javaApiCalls':report['apiCalls']}
  for observation in report['observations']:
   case=byid[observation['case']]
   for session in [0,1]:
    full=[c for c in observation['calls']if c['session']==session];chosen=[c for c in full if eligible(case['id'],c)]
    for c in full:
     if not eligible(case['id'],c):unexecuted.append({'group':group,'id':case['id'],'repeat':observation['repeat'],'session':session,'prefix':c['prefix'],'source':c['source'],'expression':c['expression'],'rawJavaPrefix':str(oracle/c['rawPrefix']),'reason':'same-evaluator independent ModuleSource API'if c['source']!='main.pkl'else'configured-property API'if group=='base64'else'URI/policy consumer not selected'if group=='resource'else'initial capability customfactory setup failure; intendedcapabilitynotreached'})
    if not chosen:continue
    work=Path(case['workspace']);source=work/'main.pkl';source_relative=copy(source,'sources/'+group+'/'+case['id']+'/main.pkl','Pkl source');copy(work/'operations.tsv','sources/'+group+'/'+case['id']+'/operations.tsv','Java expression sequence')
    expected=[];selected_raw=[];ranges=set()
    for c in chosen:
     prefix=oracle/c['rawPrefix'];raw=lambda suffix:prefix.with_name(prefix.name+'.'+suffix).read_bytes();channels={k:raw(k+'.bin')for k in ['stdout','stderr','stdout.code-units','stderr.code-units']};status=raw('status.bin');before=int(raw('reads-before.bin'));after=int(raw('reads-after.bin'));ranges.update(range(before+1,after+1))
     if status!=c['status'].encode():raise ValueError('Java status differs')
     target='stdout'if status==b'success\n'else'stderr'
     if channels[target].hex()!=c['utf8Hex']or channels[target+'.code-units'].hex()!=c['utf16BEHex']:raise ValueError('Java output differs')
     expected.append({'status':parse_status(status),'stdoutUnits':units(channels['stdout.code-units']),'stderrUnits':units(channels['stderr.code-units']),'stdoutHex':channels['stdout'].hex(),'stderrHex':channels['stderr'].hex(),'readsBefore':before,'readsAfter':after})
     for suffix in ['status.bin','stdout.bin','stderr.bin','stdout.code-units.bin','stderr.code-units.bin','reads-before.bin','reads-after.bin','expression.bin','error-class.bin','counters-before.bin','counters-after.bin']:
      p=prefix.with_name(prefix.name+'.'+suffix)
      if p.is_file():copy(p,'raw/'+group+'/'+str(p.relative_to(oracle)), 'Java API raw')
     selected_raw.append({'prefix':c['prefix'],'rawOriginalPrefix':str(prefix),'rawLocalPrefix':'raw/'+group+'/'+c['rawPrefix']})
    directory=(oracle/chosen[0]['rawPrefix']).parent;counter=dict(x.split('=',1)for x in(directory/'counters.bin').read_text().splitlines());reader_plan=[];traces=[]
    for index in range(int(counter['event'])):
     p=directory/f'event-{index}.bin';value=p.read_bytes();m=re.fullmatch(rb'reader (.*?) attempt=(\d+) total=(\d+)',value)
     if not m:continue
     uri=m.group(1).decode('utf-8');attempt=int(m.group(2));total=int(m.group(3));copy(p,'raw/'+group+'/'+str(p.relative_to(oracle)),'actual Java reader event');traces.append({'uri':uri,'attempt':attempt,'total':total,'rawPath':'raw/'+group+'/'+str(p.relative_to(oracle))})
     if group=='authority':result={'kind':'resource','resourceUri':uri,'bytes':list(('payload-'+str(total)).encode('utf-8'))};rule='AuthorityProbe.read(mode authority): new Resource(actual URI, ASCII payload-N)'
     elif uri=='res:/missing':result={'kind':'missing'};rule='ResourceReader Optional.empty for path ending /missing'
     elif uri=='res:/retry'and attempt==1:result={'kind':'io','message':'reader-first'};rule='ResourceReader throws actual IOException(reader-first) on first retry URI attempt'
     elif uri=='res:/resource':result={'kind':'resource','resourceUri':'res:/different-returned-uri','bytes':[65,13,10,0,66]};rule='ResourceProbe reader explicit Resource payload and different returned URI'
     elif re.fullmatch(r'res:/vector\d+',uri):
      n=uri[len('res:/vector'):];p=work/'payloads'/('vector'+n+'.bin');copy(p,'sources/'+group+'/'+case['id']+'/payloads/'+p.name,'raw resource bytes');result={'kind':'resource','resourceUri':'res:/returned/vector'+n,'bytes':list(p.read_bytes())};rule='ResourceProbe reader uses exact byte file and returned vector URI'
     elif re.fullmatch(r'res:/b64-\d+',uri):
      n=uri[len('res:/b64-'):];p=work/'payloads'/('b64-'+n+'.units.bin');copy(p,'sources/'+group+'/'+case['id']+'/payloads/'+p.name,'raw UTF16 String reader input');result={'kind':'text','units':units(p.read_bytes())};rule='Base64Probe reader preserves each DataInputStream.readChar UTF16 code unit'
     elif uri in {'res:/key','res:/retry'}:result={'kind':'text','units':units(('payload-'+str(total)+' '+uri).encode('utf-16-be'))};rule='ResourceProbe default String payload-total + space + actual URI'
     else:raise ValueError('unrecognized captured reader rule '+uri)
     reader_plan.append({'uri':uri,'result':result});rules.append({'group':group,'id':case['id'],'repeat':observation['repeat'],'session':session,'readerIndex':len(reader_plan)-1,'rule':rule,'returnedPayloadRecordedDirectlyInTrace':False,'provenance':'Returned input derived only from actual immutable Java helper rule and captured reader URI/attempt/total, never runtime/Rust result.'})
    if ranges!=set(range(1,len(reader_plan)+1))or [x['total']for x in traces]!=list(range(1,len(reader_plan)+1)):raise ValueError('omitted calls would alter reader plan coverage')
    fixture={'id':group+'/'+case['id'],'repeat':observation['repeat'],'session':session,'source':source.read_text(),'uri':case['sourceUri'],'moduleName':'main','expressions':[c['expression']for c in chosen],'readerPlan':reader_plan,'expected':expected};fixtures.append(fixture);session_sources.append({'id':fixture['id'],'repeat':fixture['repeat'],'session':session,'sourcePath':source_relative,'sourceOriginalPath':str(source),'sourceSha256':sha(source),'readerTrace':traces,'selectedRawJava':selected_raw,'javaGcRequest':'Original operations may contain System.gc request;excludedfromAPIselectors andnotclaimedasportableGCobservation.'})
 put=lambda p,v:p.write_text(json.dumps(v,indent=2,ensure_ascii=True)+'\n')
 put(out/'fixtures.json',{'schema':1,'fixtures':fixtures,'javaApiCalls':api_total,'unexecutedApiBoundaries':unexecuted});put(out/'provenance.json',{'groups':provenance,'originalPaths':bindings,'sessions':session_sources,'readerReturnRules':rules,'ruleQualification':'Actual reader trace records URI/attempt/count, not return payload. Every result is explicitly derived from pinned Java helper input rules;no Rust-derived expectations.','encodingQualification':'stdoutHex/stderrHex retain original Java getBytes(UTF8);UTF16 words are separate. Unpaired surrogate JavaUTF8 question-mark replacement differs from JS TextEncoder;compareunits andJavaencoding independently.'})
 put(out/'manifest.json',{'schema':1,'files':{p.relative_to(out).as_posix():{'sha256':sha(p),'bytes':p.stat().st_size}for p in sorted(out.rglob('*'))if p.is_file()and p.name!='manifest.json'},'javaSourcesAndRawProvenance':'provenance.json','javaApiCalls':api_total,'selectedApiCalls':sum(len(f['expected'])for f in fixtures),'unexecutedApiCalls':len(unexecuted),'sessions':len(fixtures),'source':'Only existing pinned Java captures;no new evaluator execution.'})
 return {'sessions':len(fixtures),'selected':sum(len(f['expected'])for f in fixtures),'javaApiCalls':api_total,'unexecuted':len(unexecuted),'manifestSha256':sha(out/'manifest.json')}
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--base',required=True,type=Path);p.add_argument('--output',required=True,type=Path);a=p.parse_args();print(json.dumps(emit(a.base,a.output),indent=2))
if __name__=='__main__':main()
