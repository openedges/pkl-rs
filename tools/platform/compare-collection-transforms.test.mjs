// Driver admission/gate/HTTP tests; no evaluator or real browser execution.
import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,writeFile,readFile,rm,mkdir,symlink} from 'node:fs/promises';
import {createServer} from 'node:http';
import {tmpdir} from 'node:os';
import {resolve} from 'node:path';
import {parseArguments,validateBuildProof,validateBundle,auditObservations,aggregateComparisons,
  trackedReader,regularWithin,serveAsset,inventoryOutput,hash,main} from './compare-collection-transforms.mjs';
import {compareCollectionTransformCase} from './collection-transform-session.mjs';

const args=['out','wasm','addon','fixtures','package.json','/chrome','1'.repeat(64),'proof.json','2'.repeat(64),'/python'];
const options=()=>parseArguments(args);
function proof(){const o=options();return {schema:1,status:'CLOSED',sourceStable:true,snapshotRoot:'/snapshot',
  sources:Object.fromEntries(['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml',
    'crates/pkl-runtime/src/vm/collection_transforms.rs','crates/pkl-runtime/src/vm/null.rs',
    'crates/pkl-runtime/examples/stage51_collection_probe.rs'].map(n=>[n,'3'.repeat(64)])),
  artifacts:{native:{path:'/native/stage51_collection_probe',sha256:'4'.repeat(64)},
    napi:{path:o.addon,sha256:'5'.repeat(64)},wasmJs:{path:resolve(o.wasm,'pkl_wasm.js'),sha256:'6'.repeat(64)},wasm:{path:resolve(o.wasm,'pkl_wasm_bg.wasm'),sha256:'7'.repeat(64)}}};}
const e=(index,expression,scope='ordinary')=>({index,expression,scope,status:'success',statusRawHex:'737563636573730a',stdoutUnits:[55],stdoutHex:'37',stderrUnits:[],stderrHex:'',readsBefore:0,readsAfter:0,logsBefore:0,logsAfter:0});
function fixture(){return {id:'java/source/r0/s0',source:'x=7\n',sourceUtf8Hex:Buffer.from('x=7\n').toString('hex'),uri:'memo:/source',moduleName:'source',
  operations:[{kind:'evaluate',value:'x'},{kind:'gc',value:''},{kind:'evaluate',value:'x'},{kind:'close'}],
  expected:[e(0,'x'),e(1,'x','intent-not-reached')],expectedEffects:[]};}
function bundle(){return {schema:1,fixtures:[fixture()],javaApiCalls:2,scopeLedger:{ordinary:1,'intent-not-reached':1,'public-api-boundary':0},fullJavaAdministrativeLifecyclePassed:false};}
function actual(f=fixture()){
  const a={id:f.id,calls:f.expected.map(x=>({...structuredClone(x),disposition:null,phase:null})),effects:[],acquisitionError:null,cleanupErrors:[],
    architecture:{terminalSlots:0,ownedResultsStable:true,providerEntriesReleased:true}};
  a.observations=[{kind:'constructed'},{kind:'evaluate-call',index:0,expression:'x'},{kind:'evaluate-return',index:0,status:'success'},
    {kind:'explicit-collect'},{kind:'evaluate-call',index:1,expression:'x'},{kind:'evaluate-return',index:1,status:'success'},
    {kind:'close-call'},{kind:'close-return'},{kind:'final-close-call'},{kind:'final-close-return'}];return a;
}
const combine=(f,a)=>[{comparison:compareCollectionTransformCase(f,a),observation:auditObservations(f,a)}];
async function temporary(t){const root=await mkdtemp(resolve(tmpdir(),'pkl-collection-driver-'));t.after(()=>rm(root,{recursive:true,force:true}));return root;}

test('CLI requires explicit runtime/proof digests and absolute Python/browser',()=>{
  assert.equal(parseArguments(['--preflight-only',...args]).preflightOnly,true);
  for(const [i,v] of [[5,'chrome'],[6,'bad'],[8,''],[9,'python3']]){const a=[...args];a[i]=v;assert.throws(()=>parseArguments(a));}
  assert.throws(()=>parseArguments(args.slice(1)),/usage/);
});
test('proof admits only closed Stage51 artifacts and four captured Cargo controls',()=>{
  assert.equal(validateBuildProof(proof(),options()).status,'CLOSED');
  for(const mutate of [p=>{p.status='building';},p=>{p.sourceStable=false;},p=>{delete p.sources['.cargo/config.toml'];},
    p=>{p.artifacts.native.path='/old/stage49_modulekey_probe';},p=>{p.artifacts.napi.path='/different/addon';},
    p=>{p.sources['../Cargo.toml']='3'.repeat(64);},p=>{p.artifacts.extra={path:'/extra',sha256:'4'.repeat(64)};}]){
    const p=proof();mutate(p);assert.throws(()=>validateBuildProof(p,options()));
  }
});
test('whole cohort keeps nonordinary calls and validates original scope ledger',()=>{
  const b=bundle();assert.deepEqual(validateBundle(b),{sessions:1,calls:2,scopes:{ordinary:1,'intent-not-reached':1,'public-api-boundary':0}});
  b.scopeLedger={'public-api-boundary':0,'intent-not-reached':1,ordinary:1};validateBundle(b);
  for(const mutate of [x=>{x.fixtures.push(structuredClone(x.fixtures[0]));},x=>{x.javaApiCalls=1;},
    x=>{x.scopeLedger.ordinary=2;},x=>{x.fixtures[0].expected.pop();},x=>{x.fixtures[0].expected[0].scope='hidden';},
    x=>{x.fixtures[0].operations[1].value='ignored';},x=>{x.fullJavaAdministrativeLifecyclePassed=true;},x=>{x.fixtures[0].source='changed';}]){
    const changed=bundle();mutate(changed);assert.throws(()=>validateBundle(changed));
  }
});
test('observations check only actual helper schema, including GC/close order',()=>{
  const f=fixture(),a=actual(f);assert.equal(auditObservations(f,a).passed,true);
  for(const mutate of [x=>{x.observations.splice(3,1);},x=>{x.observations[2].status='error';},
    x=>{x.observations.push({kind:'invented-java-reader-close'});},x=>{x.observations[1].expression='wrong';}]){
    const changed=actual(f);mutate(changed);assert.equal(auditObservations(f,changed).passed,false);
  }
});
test('raw/ordinary/acquisition/full gates remain independent',()=>{
  const f=fixture(),a=actual(f);let s=aggregateComparisons([f],[a],combine(f,a));
  assert.equal(s.rawApiPassed,true);assert.equal(s.ordinaryCompatible,1);assert.equal(s.diagnosticOnlyRawMatches,1);assert.equal(s.fullCompatibilityPassed,false);
  a.cleanupErrors.push('free failed');s=aggregateComparisons([f],[a],combine(f,a));assert.equal(s.rawApiPassed,true);assert.equal(s.acquisitionIntegrityPassed,false);assert.equal(s.ordinaryCompatibilityPassed,true);assert.equal(s.ordinaryContractPassed,false);
  a.cleanupErrors=[];a.observations.pop();s=aggregateComparisons([f],[a],combine(f,a));assert.equal(s.rawApiPassed,true);assert.equal(s.acquisitionIntegrityPassed,false);assert.equal(s.ordinaryCompatibilityPassed,true);assert.equal(s.ordinaryContractPassed,false);
});
test('matching limit bytes and extra calls cannot masquerade as ordinary/complete success',()=>{
  const f=fixture(),a=actual(f);Object.assign(f.expected[0],{status:'error',statusRawHex:'6572726f720a',stdoutUnits:[],stdoutHex:'',stderrUnits:[55],stderrHex:'37'});
  Object.assign(a.calls[0],{status:'error',disposition:'limit-exceeded',stdoutUnits:[],stderrUnits:[55]});a.observations[2].status='error';
  let s=aggregateComparisons([f],[a],combine(f,a));assert.equal(s.rawApiPassed,true);assert.equal(s.ordinaryCompatibilityPassed,false);assert.equal(s.acquisitionIntegrityPassed,true);
  a.calls.push({...a.calls[1],index:2});s=aggregateComparisons([f],[a],combine(f,a));assert.equal(s.rawApiPassed,false);assert.equal(s.acquisitionIntegrityPassed,false);
});
test('read/log effect mismatches fail separately from equal API text',()=>{
  const f=fixture(),a=actual(f);f.expectedEffects=[{kind:'read',callIndex:0,uri:'ct:/one',attempt:1,result:{kind:'text',units:[55]}}];
  a.effects=[{kind:'read',callIndex:0,uri:'ct:/wrong',attempt:1,result:{kind:'text',units:[55]}}];
  const s=aggregateComparisons([f],[a],combine(f,a));assert.equal(s.rawApiPassed,true);assert.equal(s.orderedEffectsPassed,false);assert.equal(s.ordinaryCompatibilityPassed,false);
});
test('tracked copies report changed or missing input bytes',async t=>{
  const dir=await temporary(t),file=resolve(dir,'asset.js'),track=trackedReader();await writeFile(file,'before');await track.read(file);await writeFile(file,'after');
  await assert.rejects(track.read(file),/changed/);assert.deepEqual(await track.changes(),[file]);await rm(file);assert.deepEqual(await track.changes(),[file]);
});
test('tracked input rejects nonregular files and leaf links before data access',async t=>{
  const dir=await temporary(t),file=resolve(dir,'real'),link=resolve(dir,'alias');await writeFile(file,'x');await symlink(file,link);const track=trackedReader();
  await assert.rejects(track.read(dir),/non-regular/);await assert.rejects(track.read(link),/non-regular/);assert.deepEqual(track.hashes,{});
});
test('snapshot reads reject root/parent/leaf symlinks and traversal before reading',async t=>{
  const dir=await temporary(t);await mkdir(resolve(dir,'source'));await writeFile(resolve(dir,'source','a'),'x');assert.equal(await regularWithin(dir,'source/a'),resolve(dir,'source/a'));
  await symlink(resolve(dir,'source'),resolve(dir,'alias'));await assert.rejects(regularWithin(dir,'alias/a'),/symlink/);
  await symlink(resolve(dir,'source','a'),resolve(dir,'source','link'));await assert.rejects(regularWithin(dir,'source/link'),/non-regular/);
  await assert.rejects(regularWithin(resolve(dir,'alias'),'a'),/root/);await assert.rejects(regularWithin(dir,'../a'),/unsafe/);
});
test('HTTP whitelist serves tracked bytes with queries; missing assets stay empty404 and server survives',async t=>{
  const dir=await temporary(t),file=resolve(dir,'asset.js'),missing=resolve(dir,'absent.js');await writeFile(file,'export const n=7;');const track=trackedReader();await track.read(file);
  const assets=new Map([['/asset.js',file],['/absent.js',missing]]),server=createServer((q,s)=>{void serveAsset(q,s,assets,track.read);});await new Promise(done=>server.listen(0,'127.0.0.1',done));t.after(()=>new Promise(done=>server.close(done)));
  const url='http://127.0.0.1:'+server.address().port;
  const ok=await fetch(url+'/asset.js?cache=1');assert.equal(ok.status,200);assert.equal(await ok.text(),'export const n=7;');
  for(const path of ['/not-listed','/absent.js','/%2e%2e/secret']){const response=await fetch(url+path);assert.equal(response.status,404);assert.equal(await response.text(),'');}
  assert.equal((await fetch(url+'/asset.js')).status,200);await writeFile(file,'changed');assert.equal((await fetch(url+'/asset.js')).status,404);assert.deepEqual(await track.changes(),[file]);
});
test('wrong build-proof digest preserves a preflight failure report without loading engines',async t=>{
  const dir=await temporary(t),out=resolve(dir,'out'),p=resolve(dir,'proof.json');await writeFile(p,'{}');const argv=[out,resolve(dir,'wasm'),resolve(dir,'addon'),resolve(dir,'fixtures'),resolve(dir,'package.json'),'/chrome','1'.repeat(64),p,'2'.repeat(64),process.execPath];
  const report=await main(['--preflight-only',...argv]);assert.equal(report.acquisitionIntegrityPassed,false);assert.match(report.infrastructureErrors[0].message,/proof SHA/);assert.deepEqual(report.targets,{});
  const inventory=JSON.parse(await readFile(resolve(out,'output-inventory.json')));assert.ok(inventory.files['build-proof.json']);assert.ok(inventory.files['collection_transform_fixtures.py']);assert.ok(inventory.files['compare-collection-transforms.mjs']);assert.equal(inventory.files['execution.json'],undefined);assert.equal(inventory.files['output-inventory.json'],undefined);
  assert.equal(report.outputInventory.sha256,hash(await readFile(resolve(out,'output-inventory.json'))));
  const saved=JSON.parse(await readFile(resolve(out,'execution.json')));assert.equal(saved.preflightOnly,true);assert.equal(saved.node.executable,process.execPath);assert.equal(saved.node.sha256,hash(await readFile(process.execPath)));
});

test('output inventory binds nested failed/raw payload bytes and rejects links',async t=>{
  const dir=await temporary(t);await mkdir(resolve(dir,'node'));await writeFile(resolve(dir,'node','actual.json'),'raw');await writeFile(resolve(dir,'preflight.stderr.bin'),'failure');await writeFile(resolve(dir,'execution.json'),'not-in-self-inventory');
  const result=await inventoryOutput(dir),data=JSON.parse(await readFile(resolve(dir,'output-inventory.json')));assert.equal(result.payloadFiles,2);assert.equal(data.files['node/actual.json'].sha256,hash(Buffer.from('raw')));assert.equal(data.files['preflight.stderr.bin'].bytes,7);assert.equal(data.files['execution.json'],undefined);
  await symlink(resolve(dir,'node','actual.json'),resolve(dir,'foreign'));await assert.rejects(inventoryOutput(dir),/non-regular output/);
});

test('same-byte symlink substitution cannot pass the final input identity guard',async t=>{
  const dir=await temporary(t),a=resolve(dir,'a'),b=resolve(dir,'b');await writeFile(a,'same');await writeFile(b,'same');const track=trackedReader();await track.read(a);await rm(a);await symlink(b,a);assert.deepEqual(await track.changes(),[a]);
});
