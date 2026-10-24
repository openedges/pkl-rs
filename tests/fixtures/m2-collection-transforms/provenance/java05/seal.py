from pathlib import Path
import hashlib,json,os,stat
O=Path(__file__).resolve().parent;sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
report=json.loads((O/'report.json').read_bytes());audit=json.loads((O/'readback-report.json').read_bytes())
assert audit['checks']['rawApiRows']==56 and not report['sourceInputChanges']
# Bind error class and nullable-message flags directly as well as independent encoding pairs.
for row in report['processes']:
 for c in row['calls']:
  p=O/'cases'/row['case']/str(row['repeat'])/'raw'/('session-'+str(c['session']));prefix='call-'+str(c['index'])
  assert c['errorClass']==(p/(prefix+'.error-class.bin')).read_text()
  assert c['messageNull']==(p/(prefix+'.message-null.bin')).read_text()
prunes=[]
for base in sorted((O/'runtime-private').iterdir()):
 p=base/'home/Library/Caches/org.graalvm.polyglot'
 if p.is_dir():prunes.append(str(p))
(O/'cache-prunes.json').write_text(json.dumps({'exactPaths':prunes,'reason':'Private JVM polyglot native-library extraction/dependency caches, derived from pinned official JAR; stat/direct ancestor names only, excluded before descending into these exact directories.','cacheContentsNotWalkedOrCounted':True,'noGenericCacheNameExclusion':True},indent=2)+'\n')
(O/'actual-terminal.json').write_text(json.dumps({'actualJavaAcquisitionToolSession':27285,'actualWorkerPid':92653,'exitCode':0,'captureTerminalSha256':sha(O/'outer-terminal.json'),'actualReadbackToolSession':None,'actualReadbackImmediateChunk':'49b4ce','readbackExitCode':0,'liveEvaluatorHandles':[]},indent=2)+'\n')
h={'status':'CLOSED-bounded-Java-first-collection-capture','reportSha256':sha(O/'report.json'),'readbackReportSha256':sha(O/'readback-report.json'),'profileSha256':sha(O/'profile.json'),'originalWholeSourceUnchanged':True,'originalAdapterRawMatches':0,'sequentialApiCalls':56,'scopeLedger':audit['scopeLedger'],'orderedActualEvents':audit['orderedActualEvents'],'all2JvmProcessesExit0':True,'postCloseBoundaryQualification':'No post-close calls in05; original01 post-close12 remains a separate actual API boundary.','misreachabilityQualification':'All56 parenthesized Null amendment controls reached.04 parser36 remains preserved. Function value/property/arity distinctions are actual ordinary observations.','sourceInputChanges':[],'newRustOrPortableComparison':False,'cachePrunesSha256':sha(O/'cache-prunes.json'),'liveHandles':[]}
(O/'handoff.json').write_text(json.dumps(h,indent=2)+'\n')
paths=[];todo=[O];prunes=set(prunes)
while todo:
 p=todo.pop()
 if str(p)in prunes:continue
 if p.is_symlink():raise AssertionError('unexpected producer link '+str(p))
 if p.is_dir():todo.extend(sorted(p.iterdir(),reverse=True))
 elif p.name not in ['inventory.json','closure.json','seal.stdout.bin','seal.stderr.bin']:paths.append(p)
rows={p.relative_to(O).as_posix():{'sha256':sha(p),'bytes':p.stat().st_size,'mode':stat.S_IMODE(p.stat().st_mode)}for p in sorted(paths)}
(O/'inventory.json').write_text(json.dumps({'schema':1,'files':rows,'exactPrewalkPrune':sorted(prunes)},indent=2)+'\n')
(O/'closure.json').write_text(json.dumps({'sealed':True,'handoffSha256':sha(O/'handoff.json'),'inventorySha256':sha(O/'inventory.json'),'liveHandles':[]},indent=2)+'\n')
for rel,row in rows.items():assert sha(O/rel)==row['sha256']
print(json.dumps({'payloadFiles':len(rows),'payloadBytes':sum(x['bytes']for x in rows.values()),'handoffSha256':sha(O/'handoff.json'),'closureSha256':sha(O/'closure.json'),'exactCachesPruned':len(prunes)}))
