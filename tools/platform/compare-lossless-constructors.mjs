import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { runLosslessConstructorCase, compareConstructorResult } from './lossless-constructor-session.mjs';
import { verifyLosslessSelection } from './lossless-selection.mjs';
const script=fileURLToPath(import.meta.url), helper=resolve(dirname(script),'lossless-constructor-session.mjs');
const [outputArg,wasmArg,addonArg,fixturesArg,packageArg]=process.argv.slice(2);
if(!packageArg)throw Error('usage: compare-lossless-constructors.mjs OUTPUT WASM_DIR ADDON FIXTURES_JSON PINNED_PACKAGE_JSON');
const [output,wasm,addon,fixturePath,packagePath]=[outputArg,wasmArg,addonArg,fixturesArg,packageArg].map(p=>resolve(p));
const hash=b=>createHash('sha256').update(b).digest('hex'), json=v=>JSON.stringify(v,null,2)+'\n';
await mkdir(output,{recursive:false});
const data=JSON.parse(await readFile(fixturePath)),fixtures=data.cases,factories=['constructor','expressions','output'];
if(!Array.isArray(fixtures)||fixtures.length!==4||new Set(fixtures.map(c=>c.id)).size!==4)throw Error('fixture inventory mismatch');
const verifiedSelection=await verifyLosslessSelection(data,'constructors');
await writeFile(resolve(output,'preflight.json'),json(verifiedSelection));
const require=createRequire(packagePath),{chromium}=require('playwright'),pin=JSON.parse(await readFile(packagePath)).devDependencies.playwright;
const lockPath=resolve(dirname(packagePath),'package-lock.json'),lock=JSON.parse(await readFile(lockPath));
if(require('playwright/package.json').version!==pin||require('playwright-core/package.json').version!==pin||lock.packages['node_modules/playwright'].version!==pin)throw Error('unpinned browser dependencies');
const hashes={};async function record(path){hashes[path]=hash(await readFile(path));}
for(const path of [script,resolve(dirname(script),'lossless-selection.mjs'),data.inventoryPath,helper,fixturePath,addon,packagePath,lockPath])await record(path);
for(const name of await readdir(wasm))await record(resolve(wasm,name));
for(const binding of data.bindings??[]){await record(binding.path);if(hashes[binding.path]!==binding.sha256)throw Error('original input binding mismatch');}
await writeFile(resolve(output,'fixtures.json'),json(data));
await writeFile(resolve(output,'lossless-constructor-session.mjs'),await readFile(helper));
const report={schema:1,scope:'Actual constructor rejection message transport on Node/Chromium. Original Java setup diagnostic completeUTF16+JavaUTF8 is an explicit message boundary, not constructor/setup timing equivalence. Input UTF16 rejection controls are binding policy, not Java compatibility.',argv:process.argv,cwd:process.cwd(),node:process.version,playwrightPin:pin,artifactAndInputHashes:hashes,targets:{},infrastructureErrors:[],browserErrors:[]};
const execute=api=>fixtures.flatMap(f=>[0,1].flatMap(repeat=>factories.map(factory=>({...runLosslessConstructorCase(api,f,factory),repeat}))));
function javaUtf8(units){let s='';for(let i=0;i<units.length;i++){const c=units[i],n=units[i+1];if(c>=0xd800&&c<=0xdbff){if(n>=0xdc00&&n<=0xdfff){s+=String.fromCharCode(c,n);i++;}else s+='?';}else s+=c>=0xdc00&&c<=0xdfff?'?':String.fromCharCode(c);}return Buffer.from(s,'utf8');}
const unitBytes=units=>{const b=Buffer.alloc(units.length*2);units.forEach((u,i)=>b.writeUInt16BE(u,i*2));return b;};
async function save(target,rows){await writeFile(resolve(output,target+'.raw.json'),json(rows));const expectedOrder=fixtures.flatMap(f=>[0,1].flatMap(repeat=>factories.map(factory=>`${f.id}:${repeat}:${factory}`)));if(rows.length!==expectedOrder.length||rows.some((r,i)=>`${r.id}:${r.repeat}:${r.factory}`!==expectedOrder[i]))throw Error('missing/reordered constructor results');const results=[];
 for(const actual of rows){const f=fixtures.find(f=>f.id===actual.id),d=resolve(output,target,actual.id,String(actual.repeat),actual.factory);await mkdir(d,{recursive:true});await writeFile(resolve(d,'actual.json'),json(actual));await writeFile(resolve(d,'expected.utf16be.bin'),unitBytes(f.expectedMessageUnits));if(actual.messageUnits)await writeFile(resolve(d,'actual.utf16be.bin'),unitBytes(actual.messageUnits));let c=compareConstructorResult(f,actual);if(f.expectedJavaUtf8Hex!=null){await writeFile(resolve(d,'java.utf8.bin'),Buffer.from(f.expectedJavaUtf8Hex,'hex'));c.javaUtf8Match=actual.messageUnits!=null&&javaUtf8(actual.messageUnits).equals(Buffer.from(f.expectedJavaUtf8Hex,'hex'));if(!c.javaUtf8Match)c.status='mismatch';}const result={id:actual.id,repeat:actual.repeat,factory:actual.factory,boundary:f.boundary,...c};await writeFile(resolve(d,'comparison.json'),json(result));results.push(result);}
 return{complete:true,executions:results.length,statusCounts:results.reduce((a,r)=>({...a,[r.status]:(a[r.status]??0)+1}),{}),results};}
try{report.targets.node=await save('node',execute(require(addon)));}catch(e){report.infrastructureErrors.push({target:'node',message:e.message,stack:e.stack});}
const server=createServer(async(req,res)=>{try{const u=new URL(req.url,'http://127.0.0.1');if(u.pathname==='/'){res.end('<!doctype html><title>Lossless constructor transport</title>');return;}const h=u.pathname==='/lossless-constructor-session.mjs';const path=h?helper:resolve(wasm,'.'+decodeURIComponent(u.pathname));if(!h&&!path.startsWith(wasm+sep))throw Error('invalid path');res.writeHead(200,{'Content-Type':extname(path)==='.wasm'?'application/wasm':'text/javascript'});res.end(await readFile(path));}catch{res.writeHead(404);res.end();}});
await new Promise(r=>server.listen(0,'127.0.0.1',r));let browser;
try{const executable=chromium.executablePath();report.browserExecutable=executable;report.browserExecutableSha256=hash(await readFile(executable));browser=await chromium.launch({headless:true});report.browser=browser.version();const page=await browser.newPage();page.on('pageerror',e=>report.browserErrors.push({message:e.message,stack:e.stack}));await page.goto(`http://127.0.0.1:${server.address().port}`);const rows=await page.evaluate(async fixtures=>{const api=await import('/pkl_wasm.js');await api.default();const{runLosslessConstructorCase}=await import('/lossless-constructor-session.mjs');return fixtures.flatMap(f=>[0,1].flatMap(repeat=>['constructor','expressions','output'].map(factory=>({...runLosslessConstructorCase(api,f,factory),repeat}))));},fixtures);report.targets.browser=await save('browser',rows);}catch(e){report.infrastructureErrors.push({target:'browser',message:e.message,stack:e.stack});}finally{if(browser)await browser.close();await new Promise(r=>server.close(r));}
report.changedInputs=[];for(const[p,h]of Object.entries(hashes))if(hash(await readFile(p))!==h)report.changedInputs.push(p);report.transportContractPassed=['node','browser'].every(t=>report.targets[t]?.complete&&report.targets[t].statusCounts.pass===24)&&!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length;await writeFile(resolve(output,'execution.json'),json(report));console.log(json({targets:Object.fromEntries(Object.entries(report.targets).map(([t,r])=>[t,r.statusCounts])),transportContractPassed:report.transportContractPassed}));process.exitCode=report.transportContractPassed?0:1;
