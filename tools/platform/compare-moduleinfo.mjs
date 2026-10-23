import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,readdir} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve,dirname,sep,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {runModuleInfoCase} from './moduleinfo-session.mjs';
const startedAt=new Date().toISOString(),script=fileURLToPath(import.meta.url),helper=resolve(dirname(script),'moduleinfo-session.mjs'),policy=resolve(dirname(script),'module-policy-session.mjs');
const [outputArg,wasmArg,addonArg,fixturesArg,packageArg]=process.argv.slice(2);
if(!packageArg)throw Error('usage: compare-moduleinfo.mjs OUTPUT WASM_DIR ADDON FIXTURES_DIR PINNED_PACKAGE_JSON');
const [output,wasm,addon,root,packagePath]=[outputArg,wasmArg,addonArg,fixturesArg,packageArg].map(p=>resolve(p));
await mkdir(output,{recursive:false});
const json=value=>JSON.stringify(value,null,2)+'\n',hash=value=>createHash('sha256').update(value).digest('hex'),hashes={};
async function read(path) {const data=await readFile(path),digest=hash(data);if(hashes[path]!=null&&hashes[path]!==digest)throw Error('input changed during acquisition '+path);hashes[path]=digest;return data;}
const verifier=resolve(dirname(script),'../moduleinfo_fixtures.py'),preflight=spawnSync('python3',[verifier,'--fixtures',root],{encoding:null});
await writeFile(resolve(output,'preflight.stdout.bin'),preflight.stdout??Buffer.alloc(0));await writeFile(resolve(output,'preflight.stderr.bin'),preflight.stderr??Buffer.alloc(0));if(preflight.status!==0)throw Error('ModuleInfo fixture verification failed');
const manifest=JSON.parse(await read(resolve(root,'manifest.json')));
for(const[name,row]of Object.entries(manifest.files)) {const data=await read(resolve(root,name));if(data.length!==row.bytes||hash(data)!==row.sha256)throw Error('fixture bytes '+name);}
const selection=JSON.parse(await read(resolve(root,'selection.json'))),oracle=JSON.parse(await read(resolve(root,'oracle/report.json')));
const require=createRequire(packagePath),{chromium}=require('playwright'),pin=JSON.parse(await read(packagePath)).devDependencies.playwright;
const lockPath=resolve(dirname(packagePath),'package-lock.json'),lock=JSON.parse(await read(lockPath));
if(require('playwright/package.json').version!==pin||require('playwright-core/package.json').version!==pin||lock.packages['node_modules/playwright'].version!==pin||lock.packages['node_modules/playwright-core'].version!==pin)throw Error('unpinned browser tooling');
for(const path of [script,helper,policy,verifier,addon])await read(path);
for(const name of await readdir(wasm))await read(resolve(wasm,name));
const fixtures=[];
for(const row of selection.cases)for(const repeat of [0,1]) {const observation=oracle.observations.find(o=>o.repeat===repeat);fixtures.push({...row,repeat,source:(await read(resolve(root,row.sourceFile))).toString('utf8'),expected:observation.calls.filter(c=>c.case===row.id&&c.prefix.startsWith('outer-')).map(c=>({prefix:c.prefix,kind:c.status==='success\n'?'value':'error',errorClass:c.errorClass,unitHex:c.utf16BEHex,utf8Hex:c.utf8Hex}))});}
await writeFile(resolve(output,'fixture-data.json'),json(fixtures));await writeFile(resolve(output,'moduleinfo-session.mjs'),await read(helper));await writeFile(resolve(output,'module-policy-session.mjs'),await read(policy));
function unitBytes(value) {const out=Buffer.alloc(value.length*2);value.forEach((unit,index)=>out.writeUInt16BE(unit,index*2));return out;}
function javaUtf8(value) {let text='';for(let index=0;index<value.length;index++) {const c=value[index],next=value[index+1];if(c>=0xd800&&c<=0xdbff) {if(next>=0xdc00&&next<=0xdfff) {text+=String.fromCharCode(c,next);index++;}else text+='?';}else text+=c>=0xdc00&&c<=0xdfff?'?':String.fromCharCode(c);}return Buffer.from(text,'utf8');}
const report={schema:1,startedAt,scope:'Actual Node/Chromium ModuleInfo transport:156ordinary raw status/UTF16/JavaUTF8 calls per target,12explicit Unsupported controls for preserved Java internal errors. All56Java same-evaluator root-switch followups remain unexecuted native API boundaries. No full annotation/regex/API/M2 claim.',host:{platform:os.platform(),release:os.release(),architecture:os.arch(),hostname:os.hostname()},node:process.version,playwrightPin:pin,argv:process.argv,cwd:process.cwd(),preflight:{command:['python3',verifier,'--fixtures',root],exitCode:preflight.status},artifactSourceBinding:'Supplied artifacts; bind separately to recorded immutable platform build inputs.',artifactAndInputHashes:hashes,targets:{},infrastructureErrors:[],browserErrors:[]};
async function save(target,rows) {
  await writeFile(resolve(output,target+'.raw.json'),json(rows));
  if(rows.length!==56||rows.some((r,index)=>r.id!==fixtures[index].id||r.repeat!==fixtures[index].repeat))throw Error('missing/reordered ModuleInfo sessions');
  const results=[];
  for(let index=0;index<rows.length;index++) {
    const actual=rows[index],fixture=fixtures[index],directory=resolve(output,target,actual.id,String(actual.repeat));await mkdir(directory,{recursive:true});await writeFile(resolve(directory,'actual.json'),json(actual));
    const architecture=actual.constructorError===null&&actual.acquisitionError===null&&actual.architecture.terminalSlots===0&&actual.architecture.ownedResultsStable;
    if(actual.calls.length!==3)results.push({id:actual.id,repeat:actual.repeat,status:'missing-calls',expected:3,actual:actual.calls.length,architecture});
    for(let callIndex=0;callIndex<actual.calls.length;callIndex++) {
      const call=actual.calls[callIndex],expected=fixture.expected[callIndex],unitData=unitBytes(call.units),utf8=javaUtf8(call.units),checks={architecture,prefix:call.prefix===expected?.prefix};
      await writeFile(resolve(directory,call.prefix+'.actual.utf16be.bin'),unitData);await writeFile(resolve(directory,call.prefix+'.actual.utf8.bin'),utf8);
      const internal=fixture.comparisonScope==='java-internal-parser-boundary';
      let status;
      if(internal) {checks.javaInternal=expected?.errorClass==='org.pkl.core.PklBugException';checks.nativeExplicitUnsupported=call.kind==='error'&&call.disposition==='unsupported';status=Object.values(checks).every(Boolean)?'java-internal-unsupported-boundary':'mismatch';}
      else {checks.status=call.kind===expected?.kind;checks.units=unitData.equals(Buffer.from(expected?.unitHex??'','hex'));checks.javaUtf8=utf8.equals(Buffer.from(expected?.utf8Hex??'','hex'));checks.disposition=call.kind!=='error'||call.disposition==='language-error';status=Object.values(checks).every(Boolean)?'pass':'mismatch';}
      const comparison={id:actual.id,repeat:actual.repeat,prefix:call.prefix,scope:fixture.comparisonScope,status,checks};await writeFile(resolve(directory,call.prefix+'.comparison.json'),json(comparison));results.push(comparison);
    }
  }
  return {complete:true,sessions:rows.length,counts:results.reduce((all,row)=>({...all,[row.status]:(all[row.status]??0)+1}),{}),unexecutedNativeSharedEvaluatorFollowups:56,results};
}
try {report.targets.node=await save('node',fixtures.map(f=>runModuleInfoCase(require(addon),f)));}catch(error) {report.infrastructureErrors.push({target:'node',message:error.message,stack:error.stack});}
const server=createServer(async(req,res)=>{try {const url=new URL(req.url,'http://127.0.0.1');if(url.pathname==='/') {res.end('<!doctype html><title>ModuleInfo verification</title>');return;}const isHelper=url.pathname==='/moduleinfo-session.mjs',isPolicy=url.pathname==='/module-policy-session.mjs',path=isHelper?helper:isPolicy?policy:resolve(wasm,'.'+decodeURIComponent(url.pathname));if(!isHelper&&!isPolicy&&!path.startsWith(wasm+sep))throw Error('invalid path');const assetBytes = await read(path); res.writeHead(200,{'Content-Type':extname(path)==='.wasm'?'application/wasm':'text/javascript'});res.end(assetBytes);}catch {if (!res.headersSent) { res.writeHead(404); res.end(); } else res.destroy();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));let browser;
try {report.browserExecutable=chromium.executablePath();report.browserExecutableSha256=hash(await read(report.browserExecutable));report.browserLaunchOptions={headless:true,executablePath:report.browserExecutable};browser=await chromium.launch(report.browserLaunchOptions);report.browser=browser.version();const page=await browser.newPage();page.on('pageerror',error=>report.browserErrors.push({message:error.message,stack:error.stack}));await page.goto(`http://127.0.0.1:${server.address().port}`);const rows=await page.evaluate(async fixtures=>{const api=await import('/pkl_wasm.js');await api.default();const{runModuleInfoCase}=await import('/moduleinfo-session.mjs');return fixtures.map(f=>runModuleInfoCase(api,f));},fixtures);report.targets.browser=await save('browser',rows);}catch(error) {report.infrastructureErrors.push({target:'browser',message:error.message,stack:error.stack});}finally {if(browser)await browser.close();await new Promise(done=>server.close(done));}
report.changedInputs=[];for(const[path,digest]of Object.entries(hashes))if(hash(await readFile(path))!==digest)report.changedInputs.push(path);
report.transportContractPassed=['node','browser'].every(target=>report.targets[target]?.complete&&report.targets[target].counts.pass===156&&report.targets[target].counts['java-internal-unsupported-boundary']===12&&!report.targets[target].counts.mismatch&&!report.targets[target].counts['missing-calls'])&&!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length;
report.strictJavaCompatibilityPassed=false;report.remainingStrictGateReasons=['12preserved Java-internal parser outcomes per target have Unsupported native boundary.','56same-evaluator independent-root followups per target remain unexecuted by this SourceSession adapter.'];report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'execution.json'),json(report));console.log(json({targets:Object.fromEntries(Object.entries(report.targets).map(([target,data])=>[target,data.counts])),transportContractPassed:report.transportContractPassed,strictJavaCompatibilityPassed:report.strictJavaCompatibilityPassed}));process.exitCode=report.transportContractPassed?0:1;
