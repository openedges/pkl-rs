import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,readdir} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve,dirname,sep,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {runSecurityDenialCase} from './security-denial-session.mjs';
const startedAt=new Date().toISOString(),script=fileURLToPath(import.meta.url),helper=resolve(dirname(script),'security-denial-session.mjs');
const [outputArg,wasmArg,addonArg,fixturesArg,packageArg]=process.argv.slice(2);
if(!packageArg)throw Error('usage: compare-security-denial.mjs OUTPUT WASM_DIR ADDON FIXTURES_DIR PINNED_PACKAGE_JSON');
const [output,wasm,addon,root,packagePath]=[outputArg,wasmArg,addonArg,fixturesArg,packageArg].map(p=>resolve(p));
await mkdir(output,{recursive:false});
const json=value=>JSON.stringify(value,null,2)+'\n',hash=value=>createHash('sha256').update(value).digest('hex'),hashes={};
async function read(path) {const data=await readFile(path),digest=hash(data);if(hashes[path]!=null&&hashes[path]!==digest)throw Error('input changed during acquisition '+path);hashes[path]=digest;return data;}
const verifier=resolve(dirname(script),'../security_denial_fixtures.py'),preflight=spawnSync('python3',[verifier,'--fixtures',root],{encoding:null});
await writeFile(resolve(output,'preflight.stdout.bin'),preflight.stdout??Buffer.alloc(0));await writeFile(resolve(output,'preflight.stderr.bin'),preflight.stderr??Buffer.alloc(0));if(preflight.status!==0)throw Error('Security denial fixture verification failed');
const manifest=JSON.parse(await read(resolve(root,'manifest.json')));
for(const[name,row]of Object.entries(manifest.files)) {const data=await read(resolve(root,name));if(data.length!==row.bytes||hash(data)!==row.sha256)throw Error('fixture bytes '+name);}
const context=JSON.parse(await read(resolve(root,'context.json'))),oracle=JSON.parse(await read(resolve(root,'oracle/report.json')));
const require=createRequire(packagePath),{chromium}=require('playwright'),pin=JSON.parse(await read(packagePath)).devDependencies.playwright;
const lockPath=resolve(dirname(packagePath),'package-lock.json'),lock=JSON.parse(await read(lockPath));
if(require('playwright/package.json').version!==pin||require('playwright-core/package.json').version!==pin||lock.packages['node_modules/playwright'].version!==pin||lock.packages['node_modules/playwright-core'].version!==pin)throw Error('unpinned browser tooling');
for(const path of [script,helper,verifier,addon])await read(path);
for(const name of await readdir(wasm))await read(resolve(wasm,name));
const fixtures=[];
for(const repeat of [0,1]) {
 const observation=oracle.observations.find(o=>o.repeat===repeat),causes=[];
 for(let index=0;index<3;index++) {const prefix=resolve(root,'oracle/repeats',String(repeat),'calls',`denial-${index}`),data=await read(prefix+'.message.code-units.bin');if(data.length%2)throw Error('odd cause units');causes.push({errorClass:(await read(prefix+'.error-class.bin')).toString('utf8'),uri:(await read(prefix+'.uri.bin')).toString('utf8'),units:Array.from({length:data.length/2},(_,i)=>data.readUInt16BE(i*2))});}
 fixtures.push({...context,id:'actual-standard-policy-resolve-denied',repeat,source:(await read(resolve(root,context.sourceFile))).toString('utf8'),causes,expected:observation.calls.map(c=>({prefix:c.prefix,kind:c.status==='success\n'?'value':'error',errorClass:c.errorClass,unitHex:c.utf16BEHex,utf8Hex:c.utf8Hex,denialsAfter:Number(c.countersAfter.split('\n').find(line=>line.startsWith('denied=')).slice(7))}))});
}
await writeFile(resolve(output,'fixture-data.json'),json(fixtures));await writeFile(resolve(output,'security-denial-session.mjs'),await readFile(helper));
function unitBytes(value) {const out=Buffer.alloc(value.length*2);value.forEach((unit,index)=>out.writeUInt16BE(unit,index*2));return out;}
function javaUtf8(value) {let text='';for(let index=0;index<value.length;index++) {const c=value[index],next=value[index+1];if(c>=0xd800&&c<=0xdbff) {if(next>=0xdc00&&next<=0xdfff) {text+=String.fromCharCode(c,next);index++;}else text+='?';}else text+=c>=0xdc00&&c<=0xdfff?'?':String.fromCharCode(c);}return Buffer.from(text,'utf8');}
const report={schema:1,startedAt,scope:'Actual Node/Chromium typed SecurityManagerException resolve completion:10ordinary raw status/UTF16/JavaUTF8 calls per target and3denied childrequests per repeat. Java actual Standard policy cause is preserved;NativeHost bridge does not implement or claim full Standard security policy/source-load parity.',host:{platform:os.platform(),release:os.release(),architecture:os.arch(),hostname:os.hostname()},node:process.version,playwrightPin:pin,argv:process.argv,cwd:process.cwd(),preflight:{command:['python3',verifier,'--fixtures',root],exitCode:preflight.status},artifactSourceBinding:'Supplied artifacts; bind separately to recorded immutable platform build inputs.',artifactAndInputHashes:hashes,targets:{},infrastructureErrors:[],browserErrors:[]};
async function save(target,rows) {
  await writeFile(resolve(output,target+'.raw.json'),json(rows));
  if(rows.length!==2||rows.some((r,index)=>r.id!==fixtures[index].id||r.repeat!==fixtures[index].repeat))throw Error('missing/reordered Security denial sessions');
  const results=[];
  for(let index=0;index<rows.length;index++) {
    const actual=rows[index],fixture=fixtures[index],directory=resolve(output,target,actual.id,String(actual.repeat));await mkdir(directory,{recursive:true});await writeFile(resolve(directory,'actual.json'),json(actual));
    const architecture=actual.constructorError===null&&actual.acquisitionError===null&&actual.architecture.terminalSlots===0&&actual.architecture.ownedResultsStable;
    if(actual.calls.length!==5)results.push({id:actual.id,repeat:actual.repeat,status:'missing-calls',expected:5,actual:actual.calls.length,architecture});
    for(let callIndex=0;callIndex<actual.calls.length;callIndex++) {
      const call=actual.calls[callIndex],expected=fixture.expected[callIndex],unitData=unitBytes(call.units),utf8=javaUtf8(call.units),checks={architecture,prefix:call.prefix===expected?.prefix};
      await writeFile(resolve(directory,call.prefix+'.actual.utf16be.bin'),unitData);await writeFile(resolve(directory,call.prefix+'.actual.utf8.bin'),utf8);
      checks.status=call.kind===expected?.kind;checks.units=unitData.equals(Buffer.from(expected?.unitHex??'','hex'));checks.javaUtf8=utf8.equals(Buffer.from(expected?.utf8Hex??'','hex'));checks.disposition=call.kind!=='error'||call.disposition==='language-error';checks.deniedCounter=call.denialsAfter===expected?.denialsAfter;
      const status=Object.values(checks).every(Boolean)?'pass':'mismatch';
      const comparison={id:actual.id,repeat:actual.repeat,prefix:call.prefix,scope:'typed-security-manager-exception-resolve',status,checks};await writeFile(resolve(directory,call.prefix+'.comparison.json'),json(comparison));results.push(comparison);
    }
  }
  return {complete:true,sessions:rows.length,counts:results.reduce((all,row)=>({...all,[row.status]:(all[row.status]??0)+1}),{}),results};
}
try {report.targets.node=await save('node',fixtures.map(f=>runSecurityDenialCase(require(addon),f)));}catch(error) {report.infrastructureErrors.push({target:'node',message:error.message,stack:error.stack});}
const server=createServer(async(req,res)=>{try {const url=new URL(req.url,'http://127.0.0.1');if(url.pathname==='/') {res.end('<!doctype html><title>Security denial verification</title>');return;}const isHelper=url.pathname==='/security-denial-session.mjs',path=isHelper?helper:resolve(wasm,'.'+decodeURIComponent(url.pathname));if(!isHelper&&!path.startsWith(wasm+sep))throw Error('invalid path');res.writeHead(200,{'Content-Type':extname(path)==='.wasm'?'application/wasm':'text/javascript'});res.end(await readFile(path));}catch {res.writeHead(404);res.end();}});
await new Promise(done=>server.listen(0,'127.0.0.1',done));let browser;
try {report.browserExecutable=chromium.executablePath();report.browserExecutableSha256=hash(await readFile(report.browserExecutable));browser=await chromium.launch({headless:true});report.browser=browser.version();const page=await browser.newPage();page.on('pageerror',error=>report.browserErrors.push({message:error.message,stack:error.stack}));await page.goto(`http://127.0.0.1:${server.address().port}`);const rows=await page.evaluate(async fixtures=>{const api=await import('/pkl_wasm.js');await api.default();const{runSecurityDenialCase}=await import('/security-denial-session.mjs');return fixtures.map(f=>runSecurityDenialCase(api,f));},fixtures);report.targets.browser=await save('browser',rows);}catch(error) {report.infrastructureErrors.push({target:'browser',message:error.message,stack:error.stack});}finally {if(browser)await browser.close();await new Promise(done=>server.close(done));}
report.changedInputs=[];for(const[path,digest]of Object.entries(hashes))if(hash(await readFile(path))!==digest)report.changedInputs.push(path);
report.transportContractPassed=['node','browser'].every(target=>report.targets[target]?.complete&&report.targets[target].counts.pass===10&&!report.targets[target].counts.mismatch&&!report.targets[target].counts['missing-calls'])&&!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length;
report.strictJavaCompatibilityPassed=false;report.remainingStrictGateReasons=['Typed reference exception completion proves selected VM/transport behavior;full security policy and source-load/API compatibility remain separate obligations.'];report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'execution.json'),json(report));console.log(json({targets:Object.fromEntries(Object.entries(report.targets).map(([target,data])=>[target,data.counts])),transportContractPassed:report.transportContractPassed,strictJavaCompatibilityPassed:report.strictJavaCompatibilityPassed}));process.exitCode=report.transportContractPassed?0:1;
