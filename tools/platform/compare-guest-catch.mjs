import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile,writeFile,mkdir,readdir} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve,dirname,sep,extname} from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {runGuestCatchCase} from './guest-catch-session.mjs';
const startedAt=new Date().toISOString(),script=fileURLToPath(import.meta.url),helper=resolve(dirname(script),'guest-catch-session.mjs');
const [outputArg,wasmArg,addonArg,fixturesArg,packageArg]=process.argv.slice(2);
if(!packageArg)throw Error('usage: compare-guest-catch.mjs OUTPUT WASM_DIR ADDON FIXTURES_DIR PINNED_PACKAGE_JSON');
const [output,wasm,addon,root,packagePath]=[outputArg,wasmArg,addonArg,fixturesArg,packageArg].map(p=>resolve(p));
await mkdir(output,{recursive:false});
const json=v=>JSON.stringify(v,null,2)+'\n',hash=v=>createHash('sha256').update(v).digest('hex');
const hashes={};
async function read(path){const data=await readFile(path);const digest=hash(data);if(hashes[path]!=null&&hashes[path]!==digest)throw Error('input changed during acquisition '+path);hashes[path]=digest;return data;}
const verifier=resolve(dirname(script),'../guest_catch_fixtures.py');
const preflight=spawnSync('python3',[verifier,'--fixtures',root],{encoding:null});
await writeFile(resolve(output,'preflight.stdout.bin'),preflight.stdout??Buffer.alloc(0));await writeFile(resolve(output,'preflight.stderr.bin'),preflight.stderr??Buffer.alloc(0));
if(preflight.status!==0)throw Error('fixture verification failed');
const manifest=JSON.parse(await read(resolve(root,'manifest.json')));
for(const [name,row]of Object.entries(manifest.files)) {const data=await read(resolve(root,name));if(data.length!==row.bytes||hash(data)!==row.sha256)throw Error('fixture hash mismatch '+name);}
const selection=JSON.parse(await read(resolve(root,'selection.json')));
const require=createRequire(packagePath),{chromium}=require('playwright'),pin=JSON.parse(await read(packagePath)).devDependencies.playwright;
const lockPath=resolve(dirname(packagePath),'package-lock.json'),lock=JSON.parse(await read(lockPath));
if(require('playwright/package.json').version!==pin||require('playwright-core/package.json').version!==pin||lock.packages['node_modules/playwright'].version!==pin||lock.packages['node_modules/playwright-core'].version!==pin)throw Error('unpinned browser tooling');
for(const path of [script,helper,verifier,addon])await read(path);
for(const name of await readdir(wasm))await read(resolve(wasm,name));
const bytes=async p=>Array.from(await read(p));
const units=async p=>{const b=await read(p);if(b.length%2)throw Error('odd UTF16 bytes');return Array.from({length:b.length/2},(_,i)=>b.readUInt16BE(i*2));};
const txt=async p=>(await read(p)).toString('utf8');
const fixtures=[];
for(const row of selection.cases)for(const repeat of [0,1]) {
  const workspace=resolve(root,row.group,'workspaces',row.id),calls=resolve(root,row.group,'cases',row.id,String(repeat),'calls');
  const original=JSON.parse(await read(resolve(root,row.group,'report.json'))).observations.find(o=>o.case===row.id&&o.repeat===repeat);
  const expectedCalls=[];
  for(const call of original.calls) {const channel=call.status==='success\n'?'stdout':'stderr';expectedCalls.push({prefix:call.prefix,kind:channel==='stdout'?'value':'error',units:await units(resolve(root,row.group,call.rawPrefix+'.'+channel+'.code-units.bin')),utf8:await bytes(resolve(root,row.group,call.rawPrefix+'.'+channel+'.bin'))});}
  const counterText=await txt(resolve(calls,'counters.bin'));if(!counterText.endsWith('\n'))throw Error('counter final newline missing');
  const counters=Object.fromEntries(counterText.slice(0,-1).split('\n').map(line=>{const[k,v]=line.split('=');return[k,Number(v)];}));
  const events=[];for(let i=0;i<counters.event;i++)events.push(await txt(resolve(calls,`event-${i}.bin`)));
  const logs=[];for(let i=0;i<counters.log;i++) {const stem=resolve(calls,`log-${i}`),position=(await txt(stem+'.position.bin')).split(':').map(Number),sourceLines=[];for(let n=0;n<Number(await txt(stem+'.source-count.bin'));n++)sourceLines.push(await units(stem+`.source-${n}.code-units.bin`));logs.push({level:await txt(stem+'.level.bin'),message:await units(stem+'.message.code-units.bin'),moduleUri:await units(stem+'.uri.code-units.bin'),memberName:(await txt(stem+'.member-null.bin'))==='true'?null:await units(stem+'.member.code-units.bin'),sourceLines,startLine:position[0],startColumn:position[1],endLine:position[2],endColumn:position[3]});}
  const payloadUnits={};for(const name of ['payload-units','payload-white','payload-blank'])payloadUnits[name]=await units(resolve(calls,name+'.code-units.bin'));
  fixtures.push({...row,repeat,source:await txt(resolve(workspace,'main.pkl')),payloadUnits,expected:{calls:events.filter(e=>/^(outer|inner)-[0-9]+ (success|error )/.test(e)).map(e=>{const prefix=e.split(' ')[0],call=expectedCalls.find(c=>c.prefix===prefix);if(!call)throw Error('event missing original call');return call;}),counters,events,logs}});
}
await writeFile(resolve(output,'fixture-data.json'),json(fixtures));await writeFile(resolve(output,'guest-catch-session.mjs'),await readFile(helper));
const report={schema:1,startedAt,host:{platform:os.platform(),release:os.release(),architecture:os.arch(),hostname:os.hostname()},artifactSourceBinding:'Supplied artifacts only; bind to immutable candidate build-source records externally.',preflight:{command:['python3',verifier,'--fixtures',root],exitCode:preflight.status},scope:'Actual Node/Chromium shared-engine guest catch comparison. Full ordinary strings/status, UTF16/JavaUTF8, events/logs/counters; invalid constructor text and partial terminal lifecycle explicitly separate. No JVM exception-class or later terminal API parity claim; not full Stage45/M2.',argv:process.argv,cwd:process.cwd(),node:process.version,playwrightPin:pin,artifactAndInputHashes:hashes,targets:{},infrastructureErrors:[],browserErrors:[]};
function javaUtf8(value) {let text='';for(let i=0;i<value.length;i++) {const c=value[i],n=value[i+1];if(c>=0xd800&&c<=0xdbff) {if(n>=0xdc00&&n<=0xdfff) {text+=String.fromCharCode(c,n);i++;}else text+='?';}else text+=c>=0xdc00&&c<=0xdfff?'?':String.fromCharCode(c);}return Buffer.from(text,'utf8');}
const unitBytes=value=>{const out=Buffer.alloc(value.length*2);value.forEach((u,i)=>out.writeUInt16BE(u,i*2));return out;};
const equal=(a,b)=>JSON.stringify(a)===JSON.stringify(b);
async function save(target,rows) {
  await writeFile(resolve(output,target+'.raw.json'),json(rows));
  if(rows.length!==fixtures.length||rows.some((r,i)=>r.id!==fixtures[i].id||r.group!==fixtures[i].group||r.repeat!==fixtures[i].repeat))throw Error('missing/reordered portable results');
  const results=[];
  for(let i=0;i<rows.length;i++) {
    const actual=rows[i],fixture=fixtures[i],directory=resolve(output,target,actual.group,actual.id,String(actual.repeat));await mkdir(directory,{recursive:true});await writeFile(resolve(directory,'actual.json'),json(actual));
    const checks={acquisition:actual.acquisitionError===null};
    if(fixture.comparisonScope==='frontend-constructor-only') {
      checks.constructorRejected=actual.constructorError?.isError===true;
      checks.constructorText=fixture.expected.calls.every(c=>equal(actual.constructorError?.messageUnits,c.units)&&javaUtf8(actual.constructorError.messageUnits).equals(Buffer.from(c.utf8)));
    } else {
      checks.constructed=actual.constructorError===null;checks.cleanup=actual.architecture.terminalSlots===0&&actual.architecture.ownedResultsStable;
      const expected=fixture.expected.calls.filter(c=>fixture.comparisonScope==='full-ordered-api'||c.prefix==='outer-0'||(fixture.mode.endsWith('-close')&&c.prefix==='outer-1'));
      checks.callCoverage=actual.calls.length===expected.length;
      for(let n=0;n<actual.calls.length;n++) {const call=actual.calls[n],reference=expected[n];await writeFile(resolve(directory,call.prefix+'.actual.utf16be.bin'),unitBytes(call.units));await writeFile(resolve(directory,call.prefix+'.actual.utf8.bin'),javaUtf8(call.units));checks[call.prefix]=reference!=null&&call.prefix===reference.prefix&&call.kind===reference.kind&&equal(call.units,reference.units)&&javaUtf8(call.units).equals(Buffer.from(reference.utf8))&&(call.kind!=='error'||call.disposition==='language-error');}
      if(fixture.comparisonScope==='full-ordered-api') {checks.counters=equal(actual.counts,fixture.expected.counters);checks.events=equal(actual.events,fixture.expected.events);checks.logs=equal(actual.logs,fixture.expected.logs);}
      else {checks.followupRejected=equal(actual.followupRejections,[true,true]);if(fixture.mode.endsWith('-timeout')) {checks.timeoutEscaped=actual.terminalFailure?.kind==='error'&&actual.terminalFailure.disposition==='language-error';const aborted=fixture.id.startsWith('read-')?actual.architecture.cancelledReads:actual.architecture.cancelledModules;checks.cancelledToken=equal(aborted,[actual.pendingToken]);}}
    }
    const passed=Object.values(checks).every(Boolean),status=passed?(fixture.comparisonScope==='full-ordered-api'?'pass':fixture.comparisonScope==='frontend-constructor-only'?'constructor-boundary-pass':'terminal-partial-pass'):'mismatch';
    const comparison={group:actual.group,id:actual.id,repeat:actual.repeat,scope:fixture.comparisonScope,status,checks};await writeFile(resolve(directory,'comparison.json'),json(comparison));results.push(comparison);
  }
  return {complete:true,executions:results.length,statusCounts:results.reduce((a,r)=>({...a,[r.status]:(a[r.status]??0)+1}),{}),results};
}
const execute=api=>fixtures.map(f=>runGuestCatchCase(api,f));
try {report.targets.node=await save('node',execute(require(addon)));}catch(error) {report.infrastructureErrors.push({target:'node',message:error.message,stack:error.stack});}
const server=createServer(async(req,res)=>{try {const u=new URL(req.url,'http://127.0.0.1');if(u.pathname==='/') {res.end('<!doctype html><title>Guest catch verification</title>');return;}const h=u.pathname==='/guest-catch-session.mjs',path=h?helper:resolve(wasm,'.'+decodeURIComponent(u.pathname));if(!h&&!path.startsWith(wasm+sep))throw Error('invalid path');res.writeHead(200,{'Content-Type':extname(path)==='.wasm'?'application/wasm':'text/javascript'});res.end(await readFile(path));}catch {res.writeHead(404);res.end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try {const executable=chromium.executablePath();report.browserExecutable=executable;report.browserExecutableSha256=hash(await readFile(executable));browser=await chromium.launch({headless:true});report.browser=browser.version();const page=await browser.newPage();page.on('pageerror',e=>report.browserErrors.push({message:e.message,stack:e.stack}));await page.goto(`http://127.0.0.1:${server.address().port}`);const rows=await page.evaluate(async fixtures=>{const api=await import('/pkl_wasm.js');await api.default();const{runGuestCatchCase}=await import('/guest-catch-session.mjs');return fixtures.map(f=>runGuestCatchCase(api,f));},fixtures);report.targets.browser=await save('browser',rows);}catch(error) {report.infrastructureErrors.push({target:'browser',message:error.message,stack:error.stack});}finally {if(browser)await browser.close();await new Promise(r=>server.close(r));}
report.changedInputs=[];for(const[path,digest]of Object.entries(hashes))if(hash(await readFile(path))!==digest)report.changedInputs.push(path);
report.transportContractPassed=['node','browser'].every(t=>report.targets[t]?.complete&&report.targets[t].statusCounts.pass===24&&report.targets[t].statusCounts['constructor-boundary-pass']===10&&report.targets[t].statusCounts['terminal-partial-pass']===8)&&!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length;
report.finishedAt=new Date().toISOString();await writeFile(resolve(output,'execution.json'),json(report));console.log(json({targets:Object.fromEntries(Object.entries(report.targets).map(([t,r])=>[t,r.statusCounts])),transportContractPassed:report.transportContractPassed}));process.exitCode=report.transportContractPassed?0:1;
