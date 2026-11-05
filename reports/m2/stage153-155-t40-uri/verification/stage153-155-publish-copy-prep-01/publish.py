"""Finite post-integrity publication copies; no extraction or source mutation."""
import argparse,hashlib,json,os,re,stat,subprocess,time
from pathlib import Path
ROOT=Path('/Users/cody.ij.hwang/Projects/pkl-rs.stage57');BASE=ROOT/'target/m2'
PUB=ROOT/'reports/m2/stage153-155-t40-uri';HEAD='6d5d72f66a4a7e9343781d570600c8782f460d9c'
ARCHIVE=BASE/'stage153-155-publication-01/evidence.tar.gz'
ROSTER=BASE/'stage153-155-publication-selection-02/payload-roster.json'
PACK=BASE/'stage153-155-publication-01/report.json'
INTEGRITY=BASE/'stage153-155-publication-readback-01/report.json'
AUDIT=BASE/'stage153-155-source-verification-01/source-verification.json'
FLOOR=1275068416;RESERVE=134217728;KEYS={'path','bytes','sha256','mode'}

def safe(value,archive=False):
 assert isinstance(value,str)and value and chr(0)not in value and chr(92)not in value
 p=Path(value);assert p.is_absolute()and p.is_relative_to(ROOT)and str(p)==value
 assert all(x not in('','.','..')for x in value.split('/')[1:])
 assert not any('stage86'in x.lower()or'private8'in x.lower()or x=='stage65-b-native-regressions-evidence.tar.gz.partial'for x in p.parts)
 if p.name.lower().endswith(('.tar','.tar.gz','.tgz','.tar.zst','.zip','.partial','.gz')):
  assert archive and p in(ARCHIVE,PUB/'evidence.tar.gz'),'only exact new archive bodies allowed'
 return p

def shape(row):
 assert isinstance(row,dict)and set(row)==KEYS;safe(row['path'],archive=True)
 assert type(row['bytes'])is int and row['bytes']>=0 and type(row['mode'])is int and 0<=row['mode']<=0o7777
 assert isinstance(row['sha256'],str)and re.fullmatch('[0-9a-f]{64}',row['sha256']);return row

def chain(p,missing=False):
 safe(str(p),archive=True);q=Path(p.anchor)
 for part in p.parts[1:]:
  q/=part
  if missing and not os.path.lexists(q):continue
  s=q.lstat();assert not stat.S_ISLNK(s.st_mode)
  if q!=p:assert stat.S_ISDIR(s.st_mode)
 return p

def ident(s):return(s.st_dev,s.st_ino,s.st_size,s.st_mtime_ns,s.st_ctime_ns,stat.S_IMODE(s.st_mode))
def pin(value):
 p=chain(safe(str(value),archive=True));fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW)
 with os.fdopen(fd,'rb')as f:
  s=os.fstat(f.fileno());assert stat.S_ISREG(s.st_mode);h=hashlib.file_digest(f,'sha256').hexdigest();assert ident(s)==ident(os.fstat(f.fileno()))
 assert ident(s)==ident(p.lstat());return dict(path=str(p),bytes=s.st_size,sha256=h,mode=stat.S_IMODE(s.st_mode))
def checked(row):assert pin(shape(row)['path'])==row;return row
def document(row):checked(row);return json.loads(Path(row['path']).read_bytes())
def same(a,b):return all(a[k]==b[k]for k in('bytes','sha256','mode'))
def free():s=os.statvfs(ROOT);return s.f_bavail*s.f_frsize
def head():assert subprocess.check_output(['git','-c','gc.auto=0','-c','maintenance.auto=false','rev-parse','HEAD'],cwd=ROOT,text=True).strip()==HEAD

def copy(row):
 source=checked(row['source']);dest=shape(row['destination']);assert same(source,dest)
 p=Path(source['path']);q=Path(dest['path']);assert p!=q and q.is_relative_to(PUB)
 chain(q,missing=True)
 if os.path.lexists(q):
  assert row['allow_existing']is True and q==PUB/'README.md';assert checked(dest)==dest
  return dict(source=source,published=dest,copy_exit_code=0,preexisting_exact=True)
 assert row['allow_existing']is False or q==PUB/'README.md'
 q.parent.mkdir(parents=True,exist_ok=True);chain(q.parent)
 before=p.lstat();fd=os.open(q,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o600)
 with p.open('rb')as src,os.fdopen(fd,'wb')as out:
  for chunk in iter(lambda:src.read(1024*1024),b''):out.write(chunk)
  out.flush();os.fchmod(out.fileno(),dest['mode']);os.fsync(out.fileno())
 assert ident(before)==ident(p.lstat())and checked(source)==source and checked(dest)==dest
 return dict(source=source,published=dest,copy_exit_code=0,preexisting_exact=False)

def main():
 ap=argparse.ArgumentParser();ap.add_argument('--mapping',required=True);ap.add_argument('--mapping-sha256',required=True);a=ap.parse_args()
 mp=pin(a.mapping);assert mp['sha256']==a.mapping_sha256;m=document(mp)
 assert m['draft']is False and m['head']==HEAD and m['all_publication_guards_terminal_reaped']is True
 head();authorities=m['authorities'];assert set(authorities)=={'pack','integrity','source_audit','roster','documentation_application'}
 assert authorities['pack']['path']==str(PACK)and authorities['integrity']['path']==str(INTEGRITY)and authorities['source_audit']['path']==str(AUDIT)and authorities['roster']['path']==str(ROSTER)
 pack,integrity,audit,roster,doc=[document(authorities[k])for k in('pack','integrity','source_audit','roster','documentation_application')]
 assert pack['complete']and not pack['errors']and integrity['complete']and not integrity['errors']and audit['complete']and not audit['errors']and doc['complete']
 assert roster['complete']and roster['draft']is False and roster['mutable_scratch_pending_m1']is False and roster['head']==HEAD
 assert pack['roster_path']==str(ROSTER)and pack['roster_sha256']==authorities['roster']['sha256']and roster['source_audit']==authorities['source_audit']
 assert integrity['roster_expected']['path']==str(ROSTER)and integrity['roster_expected']['sha256']==authorities['roster']['sha256']
 archive=m['archive'];shape(archive);assert archive['path']==str(ARCHIVE)
 assert all(pack['archive'][k]==archive[k]for k in('path','bytes','sha256'))and all(integrity['archive'][k]==archive[k]for k in KEYS)
 assert integrity['member_count']==pack['file_count']==roster['file_count']and integrity['logical_payload_bytes']==roster['logical_payload_bytes']==pack['logical_payload_bytes']
 for k in('exact_member_order_and_set','all_members_regular','modes_lengths_and_stream_sha256_match','canonical_forbidden_lexical_checks','raw_archive_checksum_verified_twice','archive_and_roster_bookends_stable'):assert integrity[k]is True
 assert integrity['duplicate_members']is False
 assert {r['label']for r in m['guards']}=={'audit','select','pack','archive_readback'}and len(m['guards'])==4
 guardpins=[]
 for row in m['guards']:
  g=document(row['pin']);assert g['complete']and not g['errors']and g['selected_source_bookends_stable']and g['finished_epoch']
  assert g['commands']and all(c['exit_code']==0 and c['stop_reason']is None and not c['owned']['terminal_live']for c in g['commands']);guardpins.append(row['pin'])
 assert {r['label']for r in m['preserved_failures']}=={'selector01','prep_error'}and len(m['preserved_failures'])==2
 for row in m['preserved_failures']:checked(row['pin']);assert isinstance(row['reason'],str)and row['reason']
 rows=m['copies'];assert rows and len(rows)<=256
 sources={};destinations=set()
 for row in rows:
  assert set(row)=={'source','destination','allow_existing'}and type(row['allow_existing'])is bool
  src=shape(row['source']);dst=shape(row['destination']);assert same(src,dst)
  p=safe(src['path'],archive=True);q=safe(dst['path'],archive=True)
  assert p.is_relative_to(BASE)and p.parts[len(BASE.parts)].startswith('stage153-155-')and q.is_relative_to(PUB)and p!=q
  assert not any(x in p.parts for x in('artifacts','.cache','node_modules'))and not any(x.startswith('stage142-yaml-native-cargo-')for x in p.parts)
  assert q not in destinations and q!=PUB/'publication.json';destinations.add(q)
  assert q.parent==PUB or q.is_relative_to(PUB/'verification')
  assert p==ARCHIVE or p.suffix.lower()in('.json','.py','.md','.patch','.txt','.bin','.tsv','.toml')
  assert src['path']not in sources or sources[src['path']]==src;sources[src['path']]=src
  checked(src);chain(q,missing=True)
 required={PUB/'README.md':None,PUB/'evidence.tar.gz':archive,PUB/'evidence-index.json':authorities['roster'],PUB/'source-verification.json':authorities['source_audit'],PUB/'evidence-integrity.json':authorities['integrity'],PUB/'verification/pack-report.json':authorities['pack']}
 bydest={Path(r['destination']['path']):r for r in rows}
 for dst,src in required.items():assert dst in bydest and(src is None or bydest[dst]['source']==src)
 for src in [*authorities.values(),*guardpins,*[r['pin']for r in m['preserved_failures']],*m['postplan_sources']]:assert shape(src)['path']in sources and sources[src['path']]==src
 # Copy the final mapping automatically; it cannot pin itself as an input row.
 mdest=PUB/'verification/publish-copy-map.json';assert mdest not in destinations
 rows=rows+[dict(source=mp,destination=dict(mp,path=str(mdest)),allow_existing=False)]
 needed=sum(r['source']['bytes']for r in rows if not os.path.lexists(r['destination']['path']))
 assert free()>=FLOOR+RESERVE+needed,'insufficient publication headroom';beforefree=free();copies=[]
 for row in rows:
  assert free()>=FLOOR+RESERVE;copies.append(copy(row))
 for row in rows:checked(row['source']);checked(row['destination'])
 for row in authorities.values():checked(row)
 head();assert free()>=FLOOR+RESERVE
 report=dict(complete=True,epoch=time.time(),source_epoch=HEAD,actual_git_head_checked=True,pack=pack,integrity_report=authorities['integrity'],source_verification=authorities['source_audit'],documentation_application=doc,resource_guards=guardpins,preserved_failures=m['preserved_failures'],mapping=mp,postplan_sources=m['postplan_sources'],copies=copies,free_before=beforefree,free_after=free(),git_closeout_recorded_separately=True,scope='Actual byte/mode-identical finite publication copies after completed new-archive integrity. Post-plan verification records stay outside the frozen archive plan. No full M2/API/platform/performance or hermetic restoration claim.')
 out=PUB/'publication.json';chain(out,missing=True);fd=os.open(out,os.O_WRONLY|os.O_CREAT|os.O_EXCL|os.O_NOFOLLOW,0o644)
 with os.fdopen(fd,'w')as f:json.dump(report,f,indent=2);f.write('\n');f.flush();os.fsync(f.fileno())
 print(json.dumps(pin(out)))
if __name__=='__main__':main()
