import test from 'node:test';
import assert from 'node:assert/strict';
import { readFile, writeFile, mkdtemp, rm, cp } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { verifyLosslessSelection } from './lossless-selection.mjs';
const root=resolve(dirname(fileURLToPath(import.meta.url)),'../..'),hash=b=>createHash('sha256').update(b).digest('hex');
const units=b=>Array.from({length:b.length/2},(_,i)=>b.readUInt16BE(i*2));
async function selection(kind){
 const temp=await mkdtemp(resolve(tmpdir(),'pkl-lossless-verifier-'));let reportPath,casesPath,baseDirectory,sourcePath,data;
 if(kind==='constructors'){
  baseDirectory=resolve(temp,'lambda');await cp(resolve(root,'tests/fixtures/m2-module-duplicates/lambda-property-header'),baseDirectory,{recursive:true});
  reportPath=resolve(root,'tools/platform/test-fixtures/lossless-constructor-oracle/report.json');casesPath=resolve(root,'tools/platform/test-fixtures/lossless-constructor-oracle/cases.json');sourcePath=resolve(baseDirectory,'input.pkl');
  const originals=JSON.parse(await readFile(casesPath)),c=originals.find(c=>c.id==='lambda-property-header'),raw=resolve(baseDirectory,'0/calls/outer-0'),args=['eval','--no-cache','--no-project','--settings','pkl:settings','main.pkl'];
  const main={id:c.id,source:c.source,uri:c.moduleUri,moduleName:'main',paths:c.expressions,arguments:args,powerAssertions:false,comparisonBoundary:'java-setup-message',expectedMessageUnits:units(await readFile(raw+'.stderr.code-units.bin')),expectedJavaUtf8Hex:(await readFile(raw+'.stderr.bin')).toString('hex')};
  const policy={source:'result = 1\n',uri:'file:///lossless-binding-input-policy.pkl',moduleName:'main',paths:['result'],arguments:args,powerAssertions:false,comparisonBoundary:'binding-policy',expectedMessageUnits:Array.from('unpaired UTF-16 surrogate is unsupported',c=>c.charCodeAt(0))};
  data={cases:[main,...['source','uri','moduleName'].map(key=>({...policy,id:'unpaired-input-'+key.toLowerCase(),[key]:'\ud800'}))]};
 }else{
  baseDirectory=resolve(temp,'producers');await cp(resolve(root,'tests/fixtures/m2-lossless-diagnostics/producers'),baseDirectory,{recursive:true});reportPath=resolve(baseDirectory,'report.json');casesPath=resolve(baseDirectory,'cases.json');sourcePath=resolve(baseDirectory,'workspace/main.pkl');
  const originals=JSON.parse(await readFile(casesPath)),report=JSON.parse(await readFile(reportPath));data={source:originals[0].source,uri:originals[0].moduleUri,moduleName:originals[0].moduleName,powerAssertions:true,cases:originals.slice(0,6).map((c,i)=>{const call=report.observations[0].calls[i];return{id:`scalar-prefix-${i}`,expression:c.expression,kind:call.status==='success\n'?'value':'error',javaUtf8Hex:call.utf8Hex,javaUnits:units(Buffer.from(call.utf16BEHex,'hex'))};})};
 }
 const paths=[reportPath,casesPath,sourcePath];
 for(const repeat of [0,1]){
  if(kind==='constructors')paths.push(resolve(baseDirectory,String(repeat),'process.json'));
  const count=kind==='constructors'?1:6;
  for(let i=0;i<count;i++){
   const prefix=resolve(baseDirectory,kind==='constructors'?`${repeat}/calls/outer-0`:`repeats/${repeat}/calls/outer-${i}`),status=(await readFile(prefix+'.status.bin')).toString(),channel=status==='success\n'?'stdout':'stderr';
   paths.push(prefix+'.status.bin',prefix+'.expression.bin',prefix+'.'+channel+'.bin',prefix+'.'+channel+'.code-units.bin');
  }
 }
 data.bindings=await Promise.all(paths.map(async path=>({path,sha256:hash(await readFile(path))})));
 const inventory={schema:1,kind,bindings:data.bindings,reportPath,casesPath,sourcePath,baseDirectory};data.inventoryPath=resolve(temp,'inventory.json');
 async function refresh(){inventory.bindings=data.bindings;const b=Buffer.from(JSON.stringify(inventory));await writeFile(data.inventoryPath,b);data.inventorySha256=hash(b);}
 await refresh();return{data,inventory,refresh,temp,cleanup:()=>rm(temp,{recursive:true,force:true})};
}
for(const kind of ['constructors','scalars']){
 test(kind+' genuine original channels and context verify',async()=>{const c=await selection(kind);try{assert.equal((await verifyLosslessSelection(c.data,kind)).passed,true);}finally{await c.cleanup();}});
 test(kind+' forged expected units are rejected with original metadata intact',async()=>{const c=await selection(kind);try{const row=c.data.cases[0];(row.javaUnits??row.expectedMessageUnits)[0]=63;await c.refresh();await assert.rejects(verifyLosslessSelection(c.data,kind),/payload|expectation/);}finally{await c.cleanup();}});
 test(kind+' missing source binding rejected after self-consistent inventory rehash',async()=>{const c=await selection(kind);try{c.data.bindings=c.data.bindings.filter(b=>b.path!==c.inventory.sourcePath);await c.refresh();await assert.rejects(verifyLosslessSelection(c.data,kind),/mandatory original binding/);}finally{await c.cleanup();}});
 test(kind+' forged URI rejected after self-consistent inventory rehash',async()=>{const c=await selection(kind);try{if(kind==='constructors')c.data.cases[0].uri='file:///forged.pkl';else c.data.uri='file:///forged.pkl';await c.refresh();await assert.rejects(verifyLosslessSelection(c.data,kind),/context/);}finally{await c.cleanup();}});
 test(kind+' changed raw bytes rejected after refreshing all declared hashes',async()=>{const c=await selection(kind);try{const suffix=kind==='constructors'?'0/calls/outer-0.stderr.bin':'repeats/0/calls/outer-0.stdout.bin',path=resolve(c.inventory.baseDirectory,suffix);await writeFile(path,'forged');c.data.bindings.find(b=>b.path===path).sha256=hash(await readFile(path));await c.refresh();await assert.rejects(verifyLosslessSelection(c.data,kind),/payload/);}finally{await c.cleanup();}});
}
test('omitted inventory and bindings cannot select an unchecked path',async()=>{
 await assert.rejects(verifyLosslessSelection({cases:[]},'scalars'),/mandatory/);
});
for(const kind of ['constructors','scalars']){
 test(kind+' forged declarations plus refreshed binding and inventory hashes fail pinned oracle',async()=>{const c=await selection(kind);try{const path=resolve(c.temp,'forged-cases.json'),originals=JSON.parse(await readFile(c.inventory.casesPath));originals[0].moduleUri='file:///forged.pkl';await writeFile(path,JSON.stringify(originals));const old=c.inventory.casesPath;c.inventory.casesPath=path;c.data.bindings=c.data.bindings.filter(b=>b.path!==old);c.data.bindings.push({path,sha256:hash(await readFile(path))});if(kind==='constructors')c.data.cases[0].uri='file:///forged.pkl';else c.data.uri='file:///forged.pkl';await c.refresh();await assert.rejects(verifyLosslessSelection(c.data,kind),/pinned original oracle identity/);}finally{await c.cleanup();}});
 test(kind+' power and expression controls cannot be rewritten',async()=>{const c=await selection(kind);try{if(kind==='constructors'){c.data.cases[0].powerAssertions=true;await assert.rejects(verifyLosslessSelection(c.data,kind),/context/);c.data.cases[0].powerAssertions=false;c.data.cases[0].paths=['rewritten'];}else{c.data.powerAssertions=false;await assert.rejects(verifyLosslessSelection(c.data,kind),/power/);c.data.powerAssertions=true;c.data.cases[0].expression='rewritten';}await assert.rejects(verifyLosslessSelection(c.data,kind),/context|expression/);}finally{await c.cleanup();}});
}
