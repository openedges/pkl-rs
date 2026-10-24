from pathlib import Path
import datetime,hashlib,json,os,subprocess,sys
r=Path(__file__).resolve().parent;cmd=[sys.executable,'-B',str(r/'run.py')];sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
row={'command':cmd,'cwd':str(r.parents[2]),'parentPid':os.getpid(),'startedUtc':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sourceSha256':sha(r/'run.py')}
with (r/'outer.stdout.bin').open('wb') as out,(r/'outer.stderr.bin').open('wb') as err:
 p=subprocess.Popen(cmd,cwd=r.parents[2],stdout=out,stderr=err);row['pid']=p.pid;(r/'outer-launch.json').write_text(json.dumps(row,indent=2)+'\n');code=p.wait()
row.update(exitCode=code,finishedUtc=datetime.datetime.now(datetime.timezone.utc).isoformat(),sourceAfterSha256=sha(r/'run.py'),stdoutSha256=sha(r/'outer.stdout.bin'),stderrSha256=sha(r/'outer.stderr.bin'))
(r/'outer-terminal.json').write_text(json.dumps(row,indent=2)+'\n');print(json.dumps(row),flush=True);sys.exit(code)
