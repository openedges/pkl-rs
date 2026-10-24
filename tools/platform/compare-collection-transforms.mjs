// Acquire the authenticated Java collection cohort on actual Node/Chromium.
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile, writeFile, mkdir, lstat, readdir, realpath} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import {resolve, dirname, basename, isAbsolute, sep} from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {createHash} from 'node:crypto';
import os from 'node:os';
import {runCollectionTransformCase, compareCollectionTransformCase} from './collection-transform-session.mjs';
import {javaUtf8Bytes} from './resource-session.mjs';

const json = value => JSON.stringify(value, null, 2) + '\n';
const same = (a, b) => JSON.stringify(a) === JSON.stringify(b);
const need = (ok, message) => { if (!ok) throw Error(message); };
export const hash = value => createHash('sha256').update(value).digest('hex');
const digest = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
export function relative(value) {
  need(typeof value === 'string' && value && !isAbsolute(value) && !value.includes('\\')
    && value.split('/').every(p => p && p !== '.' && p !== '..'), 'unsafe declared relative path');
  return value;
}
export function parseArguments(argv) {
  const values = [...argv], preflightOnly = values[0] === '--preflight-only';
  if (preflightOnly) values.shift();
  need(values.length === 10, 'usage: compare-collection-transforms.mjs [--preflight-only] OUTPUT WASM_DIR ADDON FIXTURES_DIR PINNED_PACKAGE_JSON BROWSER_EXECUTABLE BROWSER_SHA256 BUILD_PROOF BUILD_PROOF_SHA256 PYTHON');
  const [output, wasm, addon, fixtureRoot, packagePath, browserExecutable, browserSha256, buildProof, buildProofSha256, python] = values;
  need(digest(browserSha256) && digest(buildProofSha256), 'explicit browser/build SHA256 required');
  need(isAbsolute(python) && isAbsolute(browserExecutable), 'explicit absolute Python/browser executable required');
  return {preflightOnly, browserSha256, buildProofSha256,
    ...Object.fromEntries(Object.entries({output, wasm, addon, fixtureRoot, packagePath, browserExecutable, buildProof, python}).map(([k,v]) => [k, resolve(v)]))};
}
export function validateBuildProof(proof, options) {
  need(proof?.schema === 1 && proof.status === 'CLOSED' && proof.sourceStable === true, 'closed stable build proof required');
  need(typeof proof.snapshotRoot === 'string' && isAbsolute(proof.snapshotRoot)
    && proof.sources && !Array.isArray(proof.sources) && Object.keys(proof.sources).length, 'captured source snapshot required');
  for (const [name, value] of Object.entries(proof.sources)) { relative(name); need(digest(value), 'source SHA256 required'); }
  for (const name of ['.cargo/config.toml','Cargo.toml','Cargo.lock','rust-toolchain.toml',
    'crates/pkl-runtime/src/vm/collection_transforms.rs','crates/pkl-runtime/src/vm/null.rs',
    'crates/pkl-runtime/examples/stage51_collection_probe.rs']) need(Object.hasOwn(proof.sources,name), 'missing Stage51 source/Cargo control: ' + name);
  need(proof.artifacts && Object.keys(proof.artifacts).length === 4, 'exact four artifact identities required');
  for (const name of ['native','napi','wasmJs','wasm']) {
    const row = proof.artifacts?.[name];
    need(row && typeof row.path === 'string' && isAbsolute(row.path) && digest(row.sha256), 'explicit artifact proof: ' + name);
  }
  need(basename(proof.artifacts.native.path) === 'stage51_collection_probe', 'Stage51 native artifact required; no earlier-stage fallback');
  need(resolve(proof.artifacts.napi.path) === options.addon
    && resolve(proof.artifacts.wasmJs.path) === resolve(options.wasm,'pkl_wasm.js')
    && resolve(proof.artifacts.wasm.path) === resolve(options.wasm,'pkl_wasm_bg.wasm'), 'CLI artifacts do not match build proof');
  return proof;
}
export function validateBundle(bundle) {
  need(bundle?.schema === 1 && Array.isArray(bundle.fixtures) && bundle.fixtures.length, 'authenticated collection bundle required');
  const ids = new Set(), scopes = {ordinary:0, 'intent-not-reached':0, 'public-api-boundary':0};
  let calls = 0;
  for (const f of bundle.fixtures) {
    need(typeof f.id === 'string' && !ids.has(f.id), 'unique fixture session identity required'); ids.add(f.id);
    need(typeof f.source === 'string' && typeof f.uri === 'string' && typeof f.moduleName === 'string'
      && Buffer.from(f.source,'utf8').toString('hex') === f.sourceUtf8Hex
      && Array.isArray(f.operations) && Array.isArray(f.expected) && Array.isArray(f.expectedEffects), 'fixture source/operations required');
    const expressions = [];
    for (const op of f.operations) {
      if (op.kind === 'evaluate') { need(typeof op.value === 'string','expression required'); expressions.push(op.value); }
      else need(['gc','close'].includes(op.kind) && (op.value === undefined || op.value === ''), 'zero-argument gc/close required');
    }
    need(expressions.length === f.expected.length, 'all Java API calls must remain selected');
    for (const [i, e] of f.expected.entries()) {
      need(e.index === i && e.expression === expressions[i] && Object.hasOwn(scopes,e.scope), 'Java API identity/scope mismatch');
      scopes[e.scope]++; calls++;
    }
  }
  need(bundle.javaApiCalls === calls && bundle.fullJavaAdministrativeLifecyclePassed === false
    && bundle.scopeLedger && Object.keys(bundle.scopeLedger).length === Object.keys(scopes).length
    && Object.entries(scopes).every(([key,value]) => bundle.scopeLedger[key] === value), 'original cohort/ledger mismatch');
  return {sessions:bundle.fixtures.length,calls,scopes};
}
export function auditObservations(fixture, actual) {
  const expected = [{kind:'constructed'}]; let index = 0;
  for (const operation of fixture.operations) {
    if (operation.kind === 'evaluate') {
      expected.push({kind:'evaluate-call',index,expression:operation.value},
        {kind:'evaluate-return',index,status:actual.calls?.[index]?.status}); index++;
    } else if (operation.kind === 'gc') expected.push({kind:'explicit-collect'});
    else if (operation.kind === 'close') expected.push({kind:'close-call'},{kind:'close-return'});
  }
  expected.push({kind:'final-close-call'},{kind:'final-close-return'});
  return {passed:same(expected,actual.observations), expected,
    qualification:'Actual helper operation sequencing only; not Java administrative/GC event parity.'};
}
export function aggregateComparisons(fixtures, actuals, sessions) {
  const complete = actuals.length === fixtures.length && sessions.length === fixtures.length;
  const selected = sessions.flatMap(row => row.comparison.comparisons);
  return {complete, sessions:sessions.length, calls:actuals.reduce((n,r)=>n+(r.calls?.length??0),0),
    rawByteMatches:selected.filter(r=>r.rawMatch).length,
    ordinaryCompatible:selected.filter(r=>r.compatible).length,
    ordinaryCount:selected.filter(r=>r.scope==='ordinary').length,
    diagnosticOnlyRawMatches:selected.filter(r=>r.scope==='intent-not-reached'&&r.rawMatch).length,
    publicApiBoundaryRawMatches:selected.filter(r=>r.scope==='public-api-boundary'&&r.rawMatch).length,
    rawApiPassed:complete && sessions.every((row,i)=>actuals[i].id===fixtures[i].id
      && actuals[i].calls.length===fixtures[i].expected.length && row.comparison.comparisons.every(c=>c.rawMatch)),
    ordinaryCompatibilityPassed:complete && selected.filter(r=>r.scope==='ordinary').every(r=>r.compatible),
    ordinaryContractPassed:complete && selected.filter(r=>r.scope==='ordinary').every(r=>r.compatible)
      && sessions.every(row=>row.comparison.acquisitionIntegrityPassed && row.observation.passed && row.comparison.orderedEffectsPassed),
    acquisitionIntegrityPassed:complete && sessions.every(row=>row.comparison.acquisitionIntegrityPassed && row.observation.passed),
    orderedEffectsPassed:complete && sessions.every(row=>row.comparison.orderedEffectsPassed),
    fullJavaEventsCountersPassed:false, fullCompatibilityPassed:false};
}
export function trackedReader() {
  const hashes = {}, metadata = {};
  async function read(path) {
    path=resolve(path); const info=await lstat(path);need(info.isFile(),'non-regular tracked input: '+path);
    const mode=info.mode&0o7777;need(!metadata[path]||metadata[path].mode===mode,'input metadata changed during acquisition: '+path);
    const raw=await readFile(path), value=hash(raw);
    need(!Object.hasOwn(hashes,path)||hashes[path]===value,'input changed during acquisition: '+path);
    hashes[path]=value;metadata[path]={kind:'regular',mode};return raw;
  }
  async function changes() {
    const result=[];
    for (const [path,value] of Object.entries(hashes)) {
      try { const info=await lstat(path);if(!info.isFile()||(info.mode&0o7777)!==metadata[path].mode||hash(await readFile(path))!==value)result.push(path); } catch { result.push(path); }
    }
    return result;
  }
  return {read,hashes,metadata,changes};
}
export async function regularWithin(root, name) {
  relative(name); let current=root;
  need((await lstat(current)).isDirectory(),'non-directory snapshot/fixture root');
  for(const part of name.split('/').slice(0,-1)) { current=resolve(current,part);need((await lstat(current)).isDirectory(),'non-directory or symlink parent'); }
  const path=resolve(root,name);need((await lstat(path)).isFile(),'non-regular declared input');return path;
}
export async function serveAsset(request,response,assets,read) {
  try {
    const name=new URL(request.url,'http://127.0.0.1').pathname;
    if(name==='/') { response.writeHead(200,{'Content-Type':'text/html'});response.end('<!doctype html><title>Collection verification</title>');return; }
    const file=assets.get(name);if(!file)throw Error('unconfigured asset');
    const bytes=await read(file);
    response.writeHead(200,{'Content-Type':name.endsWith('.wasm')?'application/wasm':'text/javascript'});response.end(bytes);
  } catch { if(!response.headersSent){response.writeHead(404);response.end();}else response.destroy(); }
}
export async function inventoryOutput(root) {
  const files={};
  async function walk(dir,prefix='') {
    for(const entry of await readdir(dir,{withFileTypes:true})) {
      const name=prefix+entry.name;
      if(name==='execution.json'||name==='output-inventory.json')continue;
      const path=resolve(dir,entry.name);
      if(entry.isDirectory())await walk(path,name+'/');
      else {need(entry.isFile(),'non-regular output payload: '+name);const bytes=await readFile(path);files[name]={sha256:hash(bytes),bytes:bytes.length,mode:(await lstat(path)).mode&0o777};}
    }
  }
  await walk(root);
  const value={schema:1,scope:'Fresh driver output payloads only; execution.json and this inventory excluded to avoid a circular hash.',files};
  const path=resolve(root,'output-inventory.json');await writeFile(path,json(value));
  return {path:'output-inventory.json',sha256:hash(await readFile(path)),payloadFiles:Object.keys(files).length};
}
const unitBytes = units => { const out=Buffer.alloc(units.length*2);units.forEach((u,i)=>out.writeUInt16BE(u,i*2));return out; };

export async function main(argv=process.argv.slice(2)) {
  const o=parseArguments(argv), tracker=trackedReader(), {read}=tracker;
  const script=fileURLToPath(import.meta.url), tools=dirname(script), verifier=resolve(tools,'../collection_transform_fixtures.py');
  const helpers=['collection-transform-session.mjs','resource-session.mjs','module-policy-session.mjs'];
  await mkdir(o.output,{recursive:false});
  const report={schema:1,startedUtc:new Date().toISOString(),scope:'All authenticated Java collection API calls on frozen Stage51 Node/Chromium artifacts. Ordinary, intent-not-reached and public API boundaries remain separate; full Java administrative lifecycle false.',argv:process.argv,cwd:process.cwd(),host:{platform:os.platform(),release:os.release(),architecture:os.arch()},targets:{},infrastructureErrors:[],browserErrors:[],artifactAndInputHashes:tracker.hashes,inputFileMetadata:tracker.metadata,preflightOnly:o.preflightOnly,fullJavaEventsCountersPassed:false,fullCompatibilityPassed:false};
  let browser, server;
  try {
    for(const p of [script,verifier,...helpers.map(n=>resolve(tools,n))]){const bytes=await read(p);const copy=resolve(o.output,basename(p));await writeFile(copy,bytes);await read(copy);}
    report.node={version:process.version,versions:process.versions,executable:process.execPath,sha256:hash(await read(process.execPath))};
    const pythonReal=await realpath(o.python);report.python={requested:o.python,executable:pythonReal,sha256:hash(await read(pythonReal))};
    const proofBytes=await read(o.buildProof);await writeFile(resolve(o.output,'build-proof.json'),proofBytes);await read(resolve(o.output,'build-proof.json'));
    report.buildProofSha256=hash(proofBytes);need(report.buildProofSha256===o.buildProofSha256,'build proof SHA mismatch');
    const proof=validateBuildProof(JSON.parse(proofBytes),o);report.buildProof=proof;
    for(const [name,expected] of Object.entries(proof.sources))need(hash(await read(await regularWithin(proof.snapshotRoot,name)))===expected,'snapshot hash mismatch: '+name);
    for(const [name,row] of Object.entries(proof.artifacts))need(hash(await read(row.path))===row.sha256,'artifact hash mismatch: '+name);
    report.browserExecutable=o.browserExecutable;report.browserExecutableSha256=hash(await read(o.browserExecutable));need(report.browserExecutableSha256===o.browserSha256,'browser SHA mismatch');
    report.browserLaunchOptions={headless:true,executablePath:o.browserExecutable};
    await read(resolve(o.fixtureRoot,'manifest.json'));
    report.preflight={argv:[o.python,'-B',resolve(o.output,basename(verifier)),'--fixtures',o.fixtureRoot],originalVerifier:verifier};
    const check=spawnSync(o.python,report.preflight.argv.slice(1),{env:{...process.env,PYTHONDONTWRITEBYTECODE:'1'},encoding:null});
    report.preflight.exitCode=check.status;report.preflight.signal=check.signal;report.preflight.error=check.error?.message??null;
    for(const channel of ['stdout','stderr'])await writeFile(resolve(o.output,'preflight.'+channel+'.bin'),check[channel]??Buffer.alloc(0));
    need(check.status===0&&!check.error,'pinned Java fixture verifier failed');
    const manifest=JSON.parse(await read(resolve(o.fixtureRoot,'manifest.json')));
    need(manifest.schema===1&&manifest.files&&!Array.isArray(manifest.files),'authenticated fixture manifest required');
    for(const [name,claim] of Object.entries(manifest.files)) {
      const data=await read(await regularWithin(o.fixtureRoot,name));need(digest(claim.sha256)&&hash(data)===claim.sha256&&data.length===claim.bytes,'fixture payload mismatch: '+name);
    }
    const bundle=JSON.parse(await read(await regularWithin(o.fixtureRoot,'fixtures.json')));report.selection=validateBundle(bundle);report.javaApiCalls=bundle.javaApiCalls;report.unexecutedApiBoundaries=bundle.unexecutedApiBoundaries;
    report.boundaryQualification='All acquired calls retained. Intent failures/post-close outcomes are not ordinary compatibility; Java GC/admin trace is not projected from portable collections.';
    await writeFile(resolve(o.output,'fixtures.json'),json(bundle));await read(resolve(o.output,'fixtures.json'));
    const require=createRequire(o.packagePath), packageInfo=JSON.parse(await read(o.packagePath)), lock=JSON.parse(await read(resolve(dirname(o.packagePath),'package-lock.json')));
    const pin=packageInfo.devDependencies?.playwright;need(typeof pin==='string'&&/^\d+\.\d+\.\d+$/.test(pin),'pinned Playwright version required');
    for(const name of ['playwright','playwright-core']) {
      const p=require.resolve(name+'/package.json');need(JSON.parse(await read(p)).version===pin&&lock.packages?.['node_modules/'+name]?.version===pin,'installed Playwright/lock mismatch');
      // Exactly these two pinned tool packages; never enumerate node_modules or a browser cache.
      const packageRoot=await realpath(dirname(p));
      const cacheRoots=new Set(['node_modules','.local-browsers','.cache'].map(n=>resolve(packageRoot,n)));
      async function walk(dir){for(const entry of await readdir(dir,{withFileTypes:true})){const child=resolve(dir,entry.name);need(!cacheRoots.has(child),'unexpected cache/dependency tree in pinned tool package: '+child);if(entry.isDirectory())await walk(child);else{need(entry.isFile(),'non-regular pinned tool package');await read(child);}}}
      await walk(packageRoot);
    }
    need(lock.packages?.['']?.devDependencies?.playwright===pin,'root Playwright lock mismatch');report.playwrightPin=pin;
    const {chromium}=require('playwright');
    const assets=new Map();
    for(const name of helpers){const dest=resolve(o.output,name);await read(dest);assets.set('/'+name,dest);}
    for(const name of ['wasmJs','wasm']){const row=proof.artifacts[name];assets.set('/'+basename(row.path),row.path);}
    await writeFile(resolve(o.output,'inputs-before.json'),json(tracker.hashes));
    if(!o.preflightOnly) {
      async function save(target,rows){
        await writeFile(resolve(o.output,target+'.raw.json'),json(rows));need(rows.length===bundle.fixtures.length,'session count mismatch');
        const sessions=[];
        for(const [i,row] of rows.entries()){
          const fixture=bundle.fixtures[i];need(row.id===fixture.id,'reordered session');const dir=resolve(o.output,target,String(i));await mkdir(dir,{recursive:true});
          await writeFile(resolve(dir,'actual.json'),json(row));await writeFile(resolve(dir,'expected.json'),json(fixture.expected));
          for(const call of row.calls){for(const channel of ['stdout','stderr']){await writeFile(resolve(dir,call.index+'.'+channel+'.utf16be.bin'),unitBytes(call[channel+'Units']));await writeFile(resolve(dir,call.index+'.'+channel+'.java-utf8.bin'),Buffer.from(javaUtf8Bytes(call[channel+'Units'])));}await writeFile(resolve(dir,call.index+'.status.bin'),call.status+'\n');}
          const record={id:fixture.id,comparison:compareCollectionTransformCase(fixture,row),observation:auditObservations(fixture,row)};
          await writeFile(resolve(dir,'comparison.json'),json(record));sessions.push(record);
        }
        return {...aggregateComparisons(bundle.fixtures,rows,sessions),results:sessions};
      }
      try {report.targets.node=await save('node',bundle.fixtures.map(f=>runCollectionTransformCase(require(o.addon),f)));} catch(error){report.infrastructureErrors.push({target:'node',message:error.message,stack:error.stack});}
      server=createServer((request,response)=>{void serveAsset(request,response,assets,read);});await new Promise(done=>server.listen(0,'127.0.0.1',done));
      try {
        const origin='http://127.0.0.1:'+server.address().port;
        const missing=await fetch(origin+'/__pkl_missing_collection_asset__.js');const body=Buffer.from(await missing.arrayBuffer());report.missingAssetProbe={status:missing.status,bodyHex:body.toString('hex')};need(missing.status===404&&body.length===0,'missing asset must be empty404');
        browser=await chromium.launch(report.browserLaunchOptions);report.browserVersion=browser.version();const page=await browser.newPage();page.on('pageerror',e=>report.browserErrors.push({message:e.message,stack:e.stack}));await page.goto(origin);
        const rows=await page.evaluate(async fixtures=>{const api=await import('/pkl_wasm.js');await api.default();const {runCollectionTransformCase}=await import('/collection-transform-session.mjs');return fixtures.map(f=>runCollectionTransformCase(api,f));},bundle.fixtures);
        report.targets.browser=await save('browser',rows);
      }catch(error){report.infrastructureErrors.push({target:'browser',message:error.message,stack:error.stack});}
    }
  } catch(error){report.infrastructureErrors.push({target:'preflight',message:error.message,stack:error.stack});}
  finally {
    if(browser)try{await browser.close();}catch(error){report.infrastructureErrors.push({target:'browser-close',message:error.message});}
    if(server)await new Promise(done=>server.close(done));
    report.changedInputs=await tracker.changes();
    try{report.outputInventory=await inventoryOutput(o.output);}catch(error){report.infrastructureErrors.push({target:'output-inventory',message:error.message});}
    report.acquisitionIntegrityPassed=!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length
      && (o.preflightOnly||['node','browser'].every(t=>report.targets[t]?.acquisitionIntegrityPassed));
    for(const field of ['rawApiPassed','ordinaryCompatibilityPassed','orderedEffectsPassed'])report[field]=!o.preflightOnly&&['node','browser'].every(t=>report.targets[t]?.[field]===true);
    report.ordinaryContractPassed=report.acquisitionIntegrityPassed&&report.orderedEffectsPassed&&report.ordinaryCompatibilityPassed;
    report.selectedContractPassed=report.ordinaryContractPassed&&report.rawApiPassed;
    report.finishedUtc=new Date().toISOString();await writeFile(resolve(o.output,'execution.json'),json(report));
  }
  return report;
}
if(process.argv[1]&&pathToFileURL(resolve(process.argv[1])).href===import.meta.url){
  try{const report=await main();console.log(json({selection:report.selection,acquisitionIntegrityPassed:report.acquisitionIntegrityPassed,rawApiPassed:report.rawApiPassed,ordinaryCompatibilityPassed:report.ordinaryCompatibilityPassed,selectedContractPassed:report.selectedContractPassed,fullJavaEventsCountersPassed:false,fullCompatibilityPassed:false}));process.exitCode=(report.preflightOnly?report.acquisitionIntegrityPassed:report.selectedContractPassed)?0:1;}catch(error){console.error(error.stack??error);process.exitCode=2;}
}
