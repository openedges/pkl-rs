import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runLosslessScalarSingles, runLosslessScalarSequence, compareLosslessScalarRows, runLosslessBindingArchitecture, compareBindingArchitecture } from './lossless-scalar-session.mjs';
import { verifyLosslessSelection } from './lossless-selection.mjs';
const script=fileURLToPath(import.meta.url), helper=resolve(dirname(script),'lossless-scalar-session.mjs'), constructorHelper=resolve(dirname(script),'lossless-constructor-session.mjs');
const [outputArg,wasmArg,addonArg,fixturesArg,packageArg]=process.argv.slice(2);
if(!packageArg)throw Error('usage: compare-lossless-scalars.mjs OUTPUT WASM_DIR ADDON FIXTURES_JSON PINNED_PACKAGE_JSON');
const [output,wasm,addon,fixturePath,packagePath]=[outputArg,wasmArg,addonArg,fixturesArg,packageArg].map(p=>resolve(p));
const hash=b=>createHash('sha256').update(b).digest('hex'), json=v=>JSON.stringify(v,null,2)+'\n';
await mkdir(output,{recursive:false});
const data=JSON.parse(await readFile(fixturePath));
if(!Array.isArray(data.cases)||data.cases.length!==6)throw Error('scalar prefix inventory mismatch');
const verifiedSelection=await verifyLosslessSelection(data,'scalars');
await writeFile(resolve(output,'preflight.json'),json(verifiedSelection));
const require=createRequire(packagePath),{chromium}=require('playwright'),pin=JSON.parse(await readFile(packagePath)).devDependencies.playwright;
const lockPath=resolve(dirname(packagePath),'package-lock.json'),lock=JSON.parse(await readFile(lockPath));
if(require('playwright/package.json').version!==pin||require('playwright-core/package.json').version!==pin||lock.packages['node_modules/playwright'].version!==pin)throw Error('unpinned browser dependencies');
const hashes={};async function record(path){hashes[path]=hash(await readFile(path));}
for(const path of [script,resolve(dirname(script),'lossless-selection.mjs'),data.inventoryPath,helper,constructorHelper,fixturePath,addon,packagePath,lockPath])await record(path);
for(const name of await readdir(wasm))await record(resolve(wasm,name));
for(const binding of data.bindings??[]){await record(binding.path);if(hashes[binding.path]!==binding.sha256)throw Error('original input binding mismatch');}
await writeFile(resolve(output,'fixtures.json'),json(data));
await writeFile(resolve(output,'lossless-scalar-session.mjs'),await readFile(helper));
await writeFile(resolve(output,'lossless-constructor-session.mjs'),await readFile(constructorHelper));
const report={schema:1,nodeExecutable:process.execPath,nodeExecutableSha256:hash(await readFile(process.execPath)),scope:'Actual scalar String/JS Error and sequence UTF16 transport on Node/Chromium. Original Java producer six-call scalar prefix, fullUTF16 values/error messages and JavaUTF8 channels. Singles are freshcalls; sequences retain oneVM. Long300k unpaired string, split-pair and longconstructor are separate binding architecture controls without Java compatibility expectations.',argv:process.argv,cwd:process.cwd(),node:process.version,playwrightPin:pin,artifactAndInputHashes:hashes,targets:{},infrastructureErrors:[],browserErrors:[]};
const execute=api=>({sessions:[0,1].map(repeat=>({repeat,singles:runLosslessScalarSingles(api,data),sequence:runLosslessScalarSequence(api,data)})),architecture:runLosslessBindingArchitecture(api)});
function javaUtf8(units){let s='';for(let i=0;i<units.length;i++){const c=units[i],n=units[i+1];if(c>=0xd800&&c<=0xdbff){if(n>=0xdc00&&n<=0xdfff){s+=String.fromCharCode(c,n);i++;}else s+='?';}else s+=c>=0xdc00&&c<=0xdfff?'?':String.fromCharCode(c);}return Buffer.from(s,'utf8');}
const unitBytes=units=>{const b=Buffer.alloc(units.length*2);units.forEach((u,i)=>b.writeUInt16BE(u,i*2));return b;};
const rawByTarget={};
async function save(target,raw){
 rawByTarget[target]=raw;
 await writeFile(resolve(output,target+'.raw.json'),json(raw));
 if(!Array.isArray(raw.sessions)||raw.sessions.length!==2||raw.sessions.some((s,i)=>s.repeat!==i))throw Error('missing scalar sessions');
 const results=[];
 for(const session of raw.sessions){
  const comparison=compareLosslessScalarRows(data,session.singles,session.sequence);
  for(let i=0;i<data.cases.length;i++){
   const expected=data.cases[i],actual=session.singles[i],d=resolve(output,target,String(session.repeat),expected.id);await mkdir(d,{recursive:true});
   await writeFile(resolve(d,'java.utf16be.bin'),unitBytes(expected.javaUnits));await writeFile(resolve(d,'java.utf8.bin'),Buffer.from(expected.javaUtf8Hex,'hex'));
   if(actual?.units){await writeFile(resolve(d,'actual.utf16be.bin'),unitBytes(actual.units));await writeFile(resolve(d,'actual.java-utf8.bin'),javaUtf8(actual.units));}
   const sequence=session.sequence.rows?.[i];if(sequence)await writeFile(resolve(d,'sequence.utf16be.bin'),unitBytes(sequence));
   comparison.singleResults[i].javaUtf8Match=actual?.units!=null&&javaUtf8(actual.units).equals(Buffer.from(expected.javaUtf8Hex,'hex'));
   const prefix=Buffer.from(expected.kind==='value'?'value\t':'error\t');
   comparison.sequenceResults[i].javaUtf8Match=sequence!=null&&javaUtf8(sequence).equals(Buffer.concat([prefix,Buffer.from(expected.javaUtf8Hex,'hex')]));
   if(sequence)await writeFile(resolve(d,'sequence.java-utf8.bin'),javaUtf8(sequence));
  }
  if(!comparison.singleResults.every(r=>r.javaUtf8Match)||!comparison.sequenceResults.every(r=>r.javaUtf8Match))comparison.status='mismatch';
  results.push({repeat:session.repeat,...comparison});
 }
 const architecture=compareBindingArchitecture(raw.architecture);return{complete:true,results,statusCounts:results.reduce((a,r)=>({...a,[r.status]:(a[r.status]??0)+1}),{}),architecture};
}
try{report.targets.node=await save('node',execute(require(addon)));}catch(e){report.infrastructureErrors.push({target:'node',message:e.message,stack:e.stack});}
const server=createServer(async(req,res)=>{try{const u=new URL(req.url,'http://127.0.0.1');if(u.pathname==='/'){res.end('<!doctype html><title>Lossless scalar transport</title>');return;}const h=['/lossless-scalar-session.mjs','/lossless-constructor-session.mjs'].includes(u.pathname);const path=h?(u.pathname==='/lossless-scalar-session.mjs'?helper:constructorHelper):resolve(wasm,'.'+decodeURIComponent(u.pathname));if(!h&&!path.startsWith(wasm+sep))throw Error('invalid path');res.writeHead(200,{'Content-Type':extname(path)==='.wasm'?'application/wasm':'text/javascript'});res.end(await readFile(path));}catch{res.writeHead(404);res.end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try{const executable=chromium.executablePath();report.browserExecutable=executable;report.browserExecutableSha256=hash(await readFile(executable));browser=await chromium.launch({headless:true});report.browser=browser.version();const page=await browser.newPage();page.on('pageerror',e=>report.browserErrors.push({message:e.message,stack:e.stack}));await page.goto(`http://127.0.0.1:${server.address().port}`);const rows=await page.evaluate(async data=>{const api=await import('/pkl_wasm.js');await api.default();const {runLosslessScalarSingles,runLosslessScalarSequence,runLosslessBindingArchitecture}=await import('/lossless-scalar-session.mjs');return {sessions:[0,1].map(repeat=>({repeat,singles:runLosslessScalarSingles(api,data),sequence:runLosslessScalarSequence(api,data)})),architecture:runLosslessBindingArchitecture(api)};},data);report.targets.browser=await save('browser',rows);}catch(e){report.infrastructureErrors.push({target:'browser',message:e.message,stack:e.stack});}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
report.crossTargetArchitectureUnitsMatch=Object.fromEntries(['longValue','splitPair','longConstructor'].map(k=>{const x=rawByTarget.node?.architecture?.[k]?.units,y=rawByTarget.browser?.architecture?.[k]?.units;return[k,Array.isArray(x)&&Array.isArray(y)&&x.length===y.length&&x.every((u,i)=>u===y[i])];}));
report.changedInputs=[];for(const[p,h]of Object.entries(hashes))if(hash(await readFile(p))!==h)report.changedInputs.push(p);report.transportContractPassed=Object.values(report.crossTargetArchitectureUnitsMatch).every(Boolean)&&['node','browser'].every(t=>report.targets[t]?.complete&&report.targets[t].statusCounts.pass===2&&Object.values(report.targets[t].architecture).every(Boolean))&&!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length;await writeFile(resolve(output,'execution.json'),json(report));console.log(json({targets:Object.fromEntries(Object.entries(report.targets).map(([t,r])=>[t,r.statusCounts])),transportContractPassed:report.transportContractPassed}));process.exitCode=report.transportContractPassed?0:1;
