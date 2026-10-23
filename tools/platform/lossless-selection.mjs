import { readFile } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { resolve } from 'node:path';
const hash=b=>createHash('sha256').update(b).digest('hex');
const same=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
const pins={constructors:{report:'0e79ec5710167168c1c4d08067c99521230aab759c475ca7f62fb22a47417b11',cases:'777cef72e4eb53a651654dafb937484c75e28229977d8512f47a1b9fc318f78d'},scalars:{report:'dbaa38d4e6faa150ad4bd4469fcc836bde664b0781b699a09816f9cc8d559986',cases:'1ccb7c47b561624ad12872d2d1d70d17c7aff5d0c4bc0607620fcc2863adddc8'}};
const fail=m=>{throw Error('lossless selection: '+m);};
export async function verifyLosslessSelection(data,kind){
 if(!pins[kind])fail('unknown selection');
 if(!Array.isArray(data.bindings)||!data.bindings.length||typeof data.inventoryPath!=='string'||typeof data.inventorySha256!=='string')fail('mandatory bindings/inventory missing');
 const rawInventory=await readFile(data.inventoryPath);if(hash(rawInventory)!==data.inventorySha256)fail('inventory hash');const inv=JSON.parse(rawInventory);
 if(inv.schema!==1||inv.kind!==kind||!same(inv.bindings,data.bindings))fail('selection inventory/binding mismatch');
 const bound=new Map();for(const b of data.bindings){if(typeof b.path!=='string'||bound.has(b.path)||! /^[0-9a-f]{64}$/.test(b.sha256))fail('invalid/duplicate binding');const raw=await readFile(b.path);if(hash(raw)!==b.sha256)fail('changed original bound bytes');bound.set(b.path,raw);}
 const required=path=>{if(typeof path!=='string'||!bound.has(path))fail('mandatory original binding missing: '+path);return bound.get(path);};
 const reportRaw=required(inv.reportPath),casesRaw=required(inv.casesPath);if(hash(reportRaw)!==pins[kind].report||hash(casesRaw)!==pins[kind].cases)fail('pinned original oracle identity');
 const report=JSON.parse(reportRaw),originalCases=JSON.parse(casesRaw),source=required(inv.sourcePath).toString('utf8');
 const units=raw=>{if(raw.length%2)fail('odd UTF16 bytes');return Array.from({length:raw.length/2},(_,i)=>raw.readUInt16BE(i*2));};
 const base=inv.baseDirectory;if(typeof base!=='string'||!Array.isArray(data.cases))fail('missing base/cases');
 if(kind==='constructors'){
  const c=originalCases.find(c=>c.id==='lambda-property-header');if(!c||source!==c.source||hash(Buffer.from(source))!==c.sourceSha256)fail('original constructor source');
  if(data.cases.length!==4)fail('constructor selection count');const actual=data.cases[0];
  const args=['eval','--no-cache','--no-project','--settings','pkl:settings','main.pkl'];
  for(const [key,value]of Object.entries({id:c.id,source,uri:c.moduleUri,moduleName:'main',paths:c.expressions,arguments:args,powerAssertions:false,comparisonBoundary:'java-setup-message'}))if(!same(actual[key],value))fail('constructor source/context '+key);
  for(const repeat of [0,1]){
   const process=JSON.parse(required(resolve(base,String(repeat),'process.json'))),record=report.records.find(r=>r.id===c.id&&r.repeat===repeat);if(!same(process,record)||process.moduleUri!==actual.uri||process.sourceSha256!==hash(Buffer.from(actual.source)))fail('pinned process context');
   const call=report.observations.find(r=>r.id===c.id&&r.repeat===repeat)?.calls[0];if(!call||call.status!=='error\n')fail('original setup error status');
   const prefix=resolve(base,String(repeat),'calls','outer-0');if(required(prefix+'.status.bin').toString()!==call.status||required(prefix+'.expression.bin').toString()!==call.expression)fail('original expression/status');const utf8=required(prefix+'.stderr.bin'),utf16=required(prefix+'.stderr.code-units.bin');if(hash(utf8)!==call.utf8Sha256||hash(utf16)!==call.utf16BESha256||actual.expectedJavaUtf8Hex!==utf8.toString('hex')||!same(actual.expectedMessageUnits,units(utf16)))fail('constructor original payload');
  }
  const policy={source:'result = 1\n',uri:'file:///lossless-binding-input-policy.pkl',moduleName:'main',paths:['result'],arguments:args,powerAssertions:false,comparisonBoundary:'binding-policy',expectedMessageUnits:Array.from('unpaired UTF-16 surrogate is unsupported',c=>c.charCodeAt(0))};
  for(const [i,key]of ['source','uri','moduleName'].entries()){
   const row=data.cases[i+1],expected={...policy,id:'unpaired-input-'+key.toLowerCase(),[key]:'\ud800'};for(const[k,v]of Object.entries(expected))if(!same(row[k],v))fail('binding policy control '+k);if(row.expectedJavaUtf8Hex!=null)fail('binding policy mislabeled as Java');
  }
 }else{
  if(data.cases.length!==6||data.powerAssertions!==true||report.powerAssertions!==true)fail('scalar selection/power');
  const first=originalCases[0];for(const[key,value]of Object.entries({source:first.source,uri:first.moduleUri,moduleName:first.moduleName}))if(data[key]!==value)fail('scalar source/context '+key);if(source!==data.source||hash(Buffer.from(source))!==first.sourceSha256)fail('original scalar source');
  for(const repeat of [0,1]){
   const record=report.records.find(r=>r.repeat===repeat);if(!record||record.moduleUri!==data.uri||record.sourceSha256!==first.sourceSha256)fail('pinned scalar process context');
   for(let i=0;i<6;i++){
    const c=originalCases[i],actual=data.cases[i],call=report.observations.find(r=>r.repeat===repeat)?.calls[i];if(!call||call.oracleError||c.expression!==call.expression||actual.expression!==c.expression||actual.id!==`scalar-prefix-${i}`||c.moduleUri!==data.uri||c.source!==data.source)fail('original scalar expression/source');
    const success=call.status==='success\n',channel=success?'stdout':'stderr',prefix=resolve(base,`repeats/${repeat}/calls/outer-${i}`);const raw=required(prefix+'.'+channel+'.bin'),utf16=required(prefix+'.'+channel+'.code-units.bin');if(required(prefix+'.status.bin').toString()!==call.status||required(prefix+'.expression.bin').toString()!==call.expression||raw.toString('hex')!==call.utf8Hex||utf16.toString('hex')!==call.utf16BEHex)fail('original scalar raw payload');if(actual.kind!==(success?'value':'error')||actual.javaUtf8Hex!==call.utf8Hex||!same(actual.javaUnits,units(utf16)))fail('forged scalar expectation');
   }
  }
 }
 return{passed:true,kind,originalBindings:bound.size,inventoryPath:data.inventoryPath,oracleReportSha256:pins[kind].report};
}
