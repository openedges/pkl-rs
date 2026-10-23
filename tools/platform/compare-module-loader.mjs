// Actual shared-session module Host comparison. Expected payloads are verified
// Java acquisitions; bundle lookups do not parse/evaluate Pkl or rewrite URIs.
import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir, readdir } from 'node:fs/promises';
import { resolve, dirname, sep, extname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import { runModuleLoaderCase } from './module-loader-session.mjs';
const script=fileURLToPath(import.meta.url), root=resolve(dirname(script),'../..');
const hash=bytes=>createHash('sha256').update(bytes).digest('hex');
const json=value=>JSON.stringify(value,null,2)+'\n';
const equal=(a,b)=>Array.isArray(a)&&Array.isArray(b)?a.length===b.length&&a.every((v,i)=>equal(v,b[i])):a===b;
const hex=value=>{
  if(typeof value!=='string'||! /^(?:[0-9a-f]{2})*$/.test(value))throw Error('invalid Java byte payload');
  return Buffer.from(value,'hex');
};
export function javaUtf8(units) {
  if(!Array.isArray(units)||units.some(unit=>!Number.isInteger(unit)||unit<0||unit>65535))throw Error('invalid UTF16 payload');
  let text='';
  for(let i=0;i<units.length;i++){
    const unit=units[i];
    if(unit>=0xd800&&unit<=0xdbff){
      const low=units[i+1];
      if(low>=0xdc00&&low<=0xdfff){text+=String.fromCharCode(unit,low);i++;}else text+='?';
    }else text+=unit>=0xdc00&&unit<=0xdfff?'?':String.fromCharCode(unit);
  }
  return Buffer.from(text,'utf8');
}
export function compareLoaderResult(expected,actual){
  const expectedBytes=hex(expected.kind==='bytes'?expected.bytesHex:expected.javaUtf8Hex);
  const actualUnits=actual?.kind==='value'?actual.valueUnits:actual?.errorUnits;
  const actualBytes=actual?.kind==='bytes'?Buffer.from(actual.bytes):actual?.errorBytes!=null?Buffer.from(actual.errorBytes)
    :actualUnits!=null?javaUtf8(actualUnits):null;
  const expectedUnits=expected.kind==='value'?expected.javaValueUnits:expected.javaErrorUnits;
  const channels={kind:actual?.kind===expected.kind,payload:actualBytes!==null&&actualBytes.equals(expectedBytes),
    codeUnits:expectedUnits==null?null:equal(actualUnits,expectedUnits),
    exitCode:expected.javaExitCode==null?null:actual?.exitCode===expected.javaExitCode};
  const rawMatch=Object.values(channels).every(value=>value===null||value===true);
  const oracleValid=['success','error'].includes(expected.javaStatus);
  const status=!oracleValid?'oracle-error':!actual?'acquisition-failed':actual.kind==='thrown'?'candidate-thrown'
    :actual.kind==='error'&&!actual.disposition?'candidate-classification-missing'
    :actual.disposition&&actual.disposition!=='language-error'?'candidate-'+actual.disposition:rawMatch?'pass':'mismatch';
  return {status,rawMatch,channels,expectedBytes,actualBytes,actualUnits:actualUnits??null,
    stringBoundary:expected.kind==='bytes'?'exact-output-bytes':expectedUnits==null?'java-utf8-serialization-only':'exact-java-utf16-and-java-utf8'};
}
export function compareScopedResourceReads(expected, reads) {
  if (expected.resourceReadCounts == null) return null;
  const prefix = expected.resourceReadScopePrefix;
  if (typeof prefix !== 'string' || !prefix.length || !Array.isArray(reads)
      || reads.some(row => typeof row?.uri !== 'string')) return false;
  const counts = {};
  for (const {uri} of reads) if (uri.startsWith(prefix)) counts[uri] = (counts[uri] ?? 0) + 1;
  return Object.entries(expected.resourceReadCounts).every(([uri,count]) =>
    Number.isSafeInteger(count) && count >= 0 && (counts[uri] ?? 0) === count)
    && Object.entries(counts).every(([uri,count]) => expected.resourceReadCounts[uri] === count);
}
const unitBytes=units=>{const bytes=Buffer.alloc(units.length*2);units.forEach((unit,index)=>bytes.writeUInt16BE(unit,index*2));return bytes;};
export function selectModuleFixtureVerifier(args){
  if(args.length!==4&&args.length!==5)throw Error('usage: compare-module-loader.mjs OUTPUT WASM_DIR NAPI_BINARY FIXTURE_DIR [VERIFIER]');
  const allowed=['module_loading_fixtures.py','module_inheritance_fixtures.py','string_subscript_fixtures.py','lossless_host_fixtures.py'].map(name=>resolve(root,'tools',name));
  if(args.length===4)return allowed[0];
  if(typeof args[4]!=='string'||args[4].length===0||!allowed.includes(resolve(args[4])))throw Error('unregistered module fixture verifier');
  return resolve(args[4]);
}
export async function compareModuleLoader(args){
  const verifier=selectModuleFixtureVerifier(args);
  const [output,wasm,addon,fixtureDir]=args.slice(0,4).map(path=>resolve(path));
  await mkdir(output,{recursive:false});
  const command=['python3',verifier,'--fixtures',fixtureDir];
  const checked=spawnSync(command[0],command.slice(1),{cwd:root,encoding:null});
  await writeFile(resolve(output,'preflight.stdout.bin'),checked.stdout??Buffer.alloc(0));
  await writeFile(resolve(output,'preflight.stderr.bin'),checked.stderr??Buffer.alloc(0));
  await writeFile(resolve(output,'preflight.process.json'),json({command,cwd:root,exitCode:checked.status,signal:checked.signal,error:checked.error?.message??null}));
  if(checked.status!==0)throw Error('fixture/oracle verification failed; raw preflight retained');
  const casesPath=resolve(fixtureDir,'cases.json'), data=JSON.parse(await readFile(casesPath));
  // The registered string verifier exports a lossless graph of the original
  // ordered API calls. Other graph fixture contracts keep their existing shape.
  const stringSelection=verifier===resolve(root,'tools/string_subscript_fixtures.py')
    ?JSON.parse(checked.stdout.toString('utf8')):null;
  const hostSelection=verifier===resolve(root,'tools/lossless_host_fixtures.py')
    ?JSON.parse(checked.stdout.toString('utf8')):null;
  const fixtures=hostSelection?.graphFixtures??stringSelection?.graphFixtures??data.cases;
  if(!Array.isArray(fixtures)||!fixtures.length||fixtures.some(f=>! /^[a-z0-9-]+$/.test(f.id))||new Set(fixtures.map(f=>f.id)).size!==fixtures.length)throw Error('invalid fixture order/identities');
  const require=createRequire(resolve(root,'tools/platform/package.json'));
  const {chromium}=require('playwright');
  const packagePath=resolve(root,'tools/platform/package.json'), lockPath=resolve(root,'tools/platform/package-lock.json');
  const manifest=JSON.parse(await readFile(packagePath)),lock=JSON.parse(await readFile(lockPath));
  const pin=manifest.devDependencies.playwright,playwright=require('playwright/package.json').version,playwrightCore=require('playwright-core/package.json').version;
  if(!/^\d+\.\d+\.\d+$/.test(pin)||playwright!==pin||playwrightCore!==pin||lock.packages[''].devDependencies.playwright!==pin
    ||lock.packages['node_modules/playwright'].version!==pin||lock.packages['node_modules/playwright-core'].version!==pin)throw Error('Playwright pin mismatch');
  const hashes={};const record=async path=>{hashes[path]=hash(await readFile(path));};
  async function tree(path){for(const entry of await readdir(path,{withFileTypes:true})){
    const child=resolve(path,entry.name);if(entry.isDirectory())await tree(child);else if(entry.isFile())await record(child);else throw Error('unexpected artifact entry: '+child);
  }}
  for(const path of [script,resolve(dirname(script),'module-loader-session.mjs'),verifier,packagePath,lockPath,addon])await record(path);
  await tree(wasm);await tree(fixtureDir);
  await writeFile(resolve(output,'fixtures.json'),json(fixtures));
  for(const name of ['compare-module-loader.mjs','module-loader-session.mjs'])await writeFile(resolve(output,name),await readFile(resolve(dirname(script),name)));
  const report={schema:1,scope:'Two fresh shared SourceSessions per exact Java-backed module graph on actual Node/Chromium. Raw output/diagnostics, UTF16 when captured, callbacks, logs and typed request counts are separate comparisons. API observations are not native CLI process/files parity.',
    argv:process.argv,cwd:process.cwd(),cases:fixtures.length,repetitions:2,unexecutedBoundaryControls:data.unexecutedBoundaryControls,
    ...(stringSelection?{orderedApiCallsPerSession:stringSelection.caseCount,
      oracleApiEvaluations:stringSelection.actualJavaEvaluations,
      selectionBoundary:stringSelection.scope}:{}),
    ...(hostSelection?{orderedCallsPerRepetition:hostSelection.orderedCallsPerRepetition,
      oracleApiEvaluations:hostSelection.actualJavaEvaluations,selectionBoundary:hostSelection.scope}:{}),
    excludedProducerMetadata:{javaErrorClass:'JVM class names are retained in expected/origin records; raw diagnostic strings and API status kinds are compared. Shared phase/disposition are implementation metadata, not Java producer-phase equality.'},
    node:process.version,nodeExecutable:process.execPath,nodeExecutableSha256:hash(await readFile(process.execPath)),
    playwright,playwrightCore,playwrightPin:pin,artifactAndInputHashes:hashes,targets:{},infrastructureErrors:[],browserErrors:[]};
  const execute=(api,fixtures)=>fixtures.flatMap(fixture=>[0,1].map(repeat=>({...runModuleLoaderCase(api,fixture),repeat})));
  async function save(target,rows){
    await writeFile(resolve(output,target+'.raw.json'),json(rows));
    const pairs=fixtures.flatMap(f=>[0,1].map(repeat=>f.id+':'+repeat));
    if(rows.length!==pairs.length||rows.some((row,index)=>row.id+':'+row.repeat!==pairs[index]))throw Error('missing/reordered/duplicate fixture executions');
    const results=[];
    for(const row of rows){
      const f=fixtures.find(f=>f.id===row.id),directory=resolve(output,target,row.id,String(row.repeat));
      if(!directory.startsWith(output+sep))throw Error('unsafe fixture id');
      await mkdir(directory,{recursive:true});await writeFile(resolve(directory,'actual.json'),json(row));await writeFile(resolve(directory,'expected.json'),json(f.expected));
      const comparisons=[];
      for(const [group,expectedRows,actualRows] of [['results',f.expected.results,row.results],['callbacks',f.expected.callbacks??[],row.callbacks.map(c=>({...c.result,path:c.path}))]]){
        const count=Math.max(expectedRows.length,actualRows.length);
        for(let i=0;i<count;i++){
          const expected=expectedRows[i],actual=actualRows[i];
          let comparison;
          if(!expected)comparison={status:'extra-result',rawMatch:false};
          else{
            comparison=compareLoaderResult(expected,actual);
            await writeFile(resolve(directory,`${group}-${i}.java.bin`),comparison.expectedBytes);
            if(comparison.actualBytes!==null)await writeFile(resolve(directory,`${group}-${i}.actual.bin`),comparison.actualBytes);
            if(comparison.actualUnits!==null)await writeFile(resolve(directory,`${group}-${i}.actual.utf16be.bin`),unitBytes(comparison.actualUnits));
            if(expected.javaValueUnits??expected.javaErrorUnits)await writeFile(resolve(directory,`${group}-${i}.java.utf16be.bin`),unitBytes(expected.javaValueUnits??expected.javaErrorUnits));
            delete comparison.expectedBytes;delete comparison.actualBytes;delete comparison.actualUnits;
            const indexMatches=group==='results'?actual?.actionIndex===expected.actionIndex:actual?.path===expected.path;
            comparison.indexMatches=indexMatches;if(!indexMatches&&comparison.status==='pass')comparison.status='mismatch';
          }
          comparisons.push({group,index:i,...comparison});
        }
      }
      const logs=row.logs.map(({token,...log})=>log);
      const logFields=['level','message','moduleUri','memberName','sourceLines','startLine','startColumn','endLine','endColumn'];
      const logsMatch=f.expected.logs==null?null:logs.length===f.expected.logs.length
        &&logs.every((log,index)=>logFields.every(field=>equal(log[field],f.expected.logs[index][field])));
      const counts=row.requests.reduce((counts,request)=>({...counts,[request.kind]:(counts[request.kind]??0)+1}),{});
      const requestCountsMatch=f.expected.requestCounts==null?null:Object.entries(f.expected.requestCounts).every(([kind,count])=>(counts[kind]??0)===count)
        &&Object.entries(counts).every(([kind,count])=>count===(f.expected.requestCounts?.[kind]??0));
      const resourceReadCountsMatch=compareScopedResourceReads(f.expected,row.reads);
      const architectureMatch=row.architecture.terminalSlots===0&&row.architecture.ownedResultsStable;
      const oracleError=f.oracleError||comparisons.some(c=>c.status==='oracle-error');
      const rawMatch=comparisons.every(c=>c.rawMatch&&c.indexMatches)&&logsMatch!==false&&requestCountsMatch!==false&&resourceReadCountsMatch!==false;
      const candidateFailure=comparisons.find(c=>c.status.startsWith('candidate-'));
      const status=oracleError?'oracle-error':row.acquisitionError?'acquisition-failed':candidateFailure?candidateFailure.status:!rawMatch?'mismatch':!architectureMatch?'architecture-failed':'pass';
      const result={id:row.id,repeat:row.repeat,status,rawMatch,logsMatch,requestCountsMatch,resourceReadCountsMatch,resourceReads:row.reads??null,requestCounts:counts,architectureMatch,
        architecture:row.architecture,acquisitionError:row.acquisitionError,comparisons};
      await writeFile(resolve(directory,'result.json'),json(result));results.push(result);
    }
    return {complete:true,executions:results.length,results,statusCounts:results.reduce((c,r)=>({...c,[r.status]:(c[r.status]??0)+1}),{})};
  }
  try{report.targets.node=await save('node',execute(require(addon),fixtures));}catch(error){report.infrastructureErrors.push({target:'node',message:error.message,stack:error.stack});}
  const server=createServer(async(request,response)=>{try{
    const url=new URL(request.url,'http://127.0.0.1');if(url.pathname==='/'){response.end('<!doctype html><title>Module Host verification</title>');return;}
    const helper=url.pathname==='/module-loader-session.mjs';
    const path=helper?resolve(dirname(script),'module-loader-session.mjs'):resolve(wasm,'.'+decodeURIComponent(url.pathname));
    if(!helper&&!path.startsWith(wasm+sep))throw Error('invalid Wasm path');
    response.writeHead(200,{'Content-Type':extname(path)==='.wasm'?'application/wasm':'text/javascript'});response.end(await readFile(path));
  }catch{response.writeHead(404);response.end();}});
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));let browser;
  try{
    const executable=chromium.executablePath();report.browserExecutable=executable;report.browserExecutableSha256=hash(await readFile(executable));
    browser=await chromium.launch({headless:true});report.browser=browser.version();const page=await browser.newPage();
    page.on('pageerror',error=>report.browserErrors.push({message:error.message,stack:error.stack}));await page.goto(`http://127.0.0.1:${server.address().port}`);
    const rows=await page.evaluate(async fixtures=>{const api=await import('/pkl_wasm.js');await api.default();const {runModuleLoaderCase}=await import('/module-loader-session.mjs');
      return fixtures.flatMap(fixture=>[0,1].map(repeat=>({...runModuleLoaderCase(api,fixture),repeat})));},fixtures);
    report.targets.browser=await save('browser',rows);
  }catch(error){report.infrastructureErrors.push({target:'browser',message:error.message,stack:error.stack});}
  finally{if(browser)await browser.close();await new Promise(resolve=>server.close(resolve));}
  report.changedInputs=[];for(const [path,digest]of Object.entries(hashes))if(hash(await readFile(path))!==digest)report.changedInputs.push(path);
  report.selectedCompatibilityComplete=['node','browser'].every(target=>report.targets[target]?.complete&&report.targets[target].results.every(row=>row.status==='pass'))
    &&!report.infrastructureErrors.length&&!report.browserErrors.length&&!report.changedInputs.length;
  await writeFile(resolve(output,'execution.json'),json(report));console.log(json({targets:Object.fromEntries(Object.entries(report.targets).map(([k,v])=>[k,v.statusCounts])),selectedCompatibilityComplete:report.selectedCompatibilityComplete}));return report;
}
if(process.argv[1]&&resolve(process.argv[1])===script){const report=await compareModuleLoader(process.argv.slice(2));process.exitCode=report.selectedCompatibilityComplete?0:1;}
