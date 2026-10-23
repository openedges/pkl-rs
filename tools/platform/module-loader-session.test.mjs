import test from 'node:test';
import assert from 'node:assert/strict';
import { runModuleLoaderCase } from './module-loader-session.mjs';

const resolver = (importUri = 'file:///child%2520.pkl') => ({ kind: 'module-request', token: 'r', module: {
  id: '18446744073709551615', kind: 'resolve', importUri, originalUri: importUri,
  importerOriginalUri: 'file:///main.pkl', importerResolvedUri: 'file:///main.pkl',
} });
const loader = () => ({ kind: 'module-request', token: 's', module: { id: '2', kind: 'load-source',
  originalUri: 'file:///child%2520.pkl', resolvedUri: 'file:///canonical.pkl', sourceUri: 'file:///alias.pkl' } });
function setup(events, options = {}) {
  const calls = [];
  class SourceSession {
    static forExpressions(...args) { calls.push(['forExpressions', ...args]); return new SourceSession(); }
    static forOutput(...args) { calls.push(['forOutput', ...args]); return new SourceSession(); }
    constructor(...args) { calls.push(['constructor', ...args]); }
    poll() { if (!events.length) throw Error('unexpected extra poll'); const event=events.shift(); return event.kind==='error' ? {phase:'module-output',disposition:'language-error',...event} : event; }
    collect() { return 0; }
    heapSlots() { return 0; }
    close() { calls.push(['close']); }
    free() { options.free?.(); }
    drainCancelledModules() { return options.cancelledModules ?? []; }
    drainCancelledReads() { return []; }
  }
  for (const method of ['startPath','startOutputBytes','completeModuleResolution','completeModuleSource',
    'rejectModuleResolution','rejectModuleSource','reenterPath','closeFromCallback','respondString','respondConfigured','acknowledgeLog'])
    SourceSession.prototype[method] = (...args) => calls.push([method, ...args]);
  return { api: { SourceSession }, calls };
}
function fixture(extra = {}) {
  return { id: 'fixture', source: 'result=7', uri: 'file:///main.pkl', moduleName: 'main', paths: ['result','stable'],
    resolutions: [{ importUri: 'file:///child%2520.pkl', resolvedUri: 'file:///canonical.pkl', sourceUri: 'file:///alias.pkl' }],
    sources: [{ resolvedUri: 'file:///canonical.pkl', sourceUri: 'file:///alias.pkl', text: 'value=7', inferredName: 'child' }],
    actions: [{ kind: 'path', index: 0 }], ...extra };
}

test('exact prepared URI and decimal request ID reach typed Host completions unchanged', () => {
  const { api, calls } = setup([resolver(), loader(), { kind:'value', value:Uint16Array.of(55) }]);
  const row = runModuleLoaderCase(api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.equal(row.requests[0].id, '18446744073709551615');
  assert.equal(row.requests[0].importUri, 'file:///child%2520.pkl');
  assert.deepEqual(calls.filter(c => c[0].startsWith('complete')), [
    ['completeModuleResolution','r','file:///child%2520.pkl','file:///canonical.pkl','file:///alias.pkl',0],
    ['completeModuleSource','s','value=7','child',0],
  ]);
  assert.deepEqual(row.results[0].valueUnits, [55]);
});

test('source callback reenters before completing the suspended original source token', () => {
  const { api, calls } = setup([resolver(), loader(), {kind:'value',value:Uint16Array.of(57)}, {kind:'value',value:Uint16Array.of(55)}]);
  const f = fixture(); f.sources[0].callbackPaths = [1];
  const row = runModuleLoaderCase(api, f);
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(row.callbacks[0].result.valueUnits, [57]);
  assert.deepEqual(row.results[0].valueUnits, [55]);
  assert.deepEqual(calls.filter(c => ['reenterPath','completeModuleSource'].includes(c[0])), [
    ['reenterPath','s',1,0], ['completeModuleSource','s','value=7','child',0],
  ]);
});

test('missing bundle resolution rejects the correct phase and retains UTF16 diagnostics', () => {
  const { api, calls } = setup([resolver('life:/missing'), {kind:'error',error:Uint16Array.of(0xd800,0,120)}]);
  const row = runModuleLoaderCase(api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(calls.find(c => c[0] === 'rejectModuleResolution'), ['rejectModuleResolution','r','not-found','',0]);
  assert.deepEqual(row.results[0].errorUnits,[0xd800,0,120]);
});

test('duplicate identities and malformed request contexts are acquisition errors', () => {
  const a = setup([]), f = fixture(); f.sources.push({...f.sources[0]});
  assert.match(runModuleLoaderCase(a.api,f).acquisitionError.message,/duplicate/);
  assert.equal(a.calls.length,0);
  const malformed = resolver(); delete malformed.module.importerOriginalUri;
  const b = setup([malformed]);
  const row = runModuleLoaderCase(b.api,fixture());
  assert.match(row.acquisitionError.message,/incomplete resolver context/);
  assert.equal(row.results.length,0);
});

test('output Unsupported remains typed exit2 with unchanged byte vectors', () => {
  const event = {kind:'error',errorBytes:Uint8Array.of(0,195,169,13,10),exitCode:2,phase:'module-output',disposition:'unsupported',reason:'capability'};
  const {api} = setup([event]);
  const row = runModuleLoaderCase(api,fixture({operation:'output-bytes',arguments:['eval','main.pkl'],actions:[{kind:'output-bytes'}]}));
  assert.equal(row.acquisitionError,null);
  assert.equal(row.results[0].exitCode,2);
  assert.equal(row.results[0].disposition,'unsupported');
  assert.deepEqual(row.results[0].errorBytes,[0,195,169,13,10]);
});

test('returned buffers remain checked after free and cancellation tokens stay separate', () => {
  const value = Uint16Array.of(0xd800,120);
  const {api} = setup([{kind:'value',value}],{free:()=>{value[0]=63;},cancelledModules:['module-token']});
  const row = runModuleLoaderCase(api,fixture());
  assert.deepEqual(row.results[0].valueUnits,[0xd800,120]);
  assert.equal(row.architecture.ownedResultsStable,false);
  assert.deepEqual(row.architecture.cancelledModules,['module-token']);
  assert.deepEqual(row.architecture.cancelledReads,[]);
});

test('bundle source mutation changes the next Host completion without evaluating in JavaScript', () => {
  const {api,calls} = setup([resolver(),loader(),{kind:'value',value:Uint16Array.of(55)},resolver(),loader(),{kind:'value',value:Uint16Array.of(56)}]);
  const row = runModuleLoaderCase(api,fixture({actions:[{kind:'path',index:0},
    {kind:'set-source',source:{resolvedUri:'file:///canonical.pkl',text:'value=8',inferredName:'changed'}}, {kind:'path',index:0}]}));
  assert.equal(row.acquisitionError,null);
  assert.deepEqual(calls.filter(c=>c[0]==='completeModuleSource').map(c=>c.slice(2,4)),[['value=7','child'],['value=8','changed']]);
});


test('invalid shared failure exit codes cannot be recorded as compatible call errors', () => {
  for (const exitCode of [0,10]) {
    const {api}=setup([{kind:'error',errorBytes:Uint8Array.of(63),exitCode,phase:'module-output'}]);
    const row=runModuleLoaderCase(api,fixture({operation:'output-bytes',arguments:['eval','main.pkl'],actions:[{kind:'output-bytes'}]}));
    assert.match(row.acquisitionError.message,/invalid shared module completion metadata/);
    assert.equal(row.results.length,0);
  }
});


test('finite source plans allow inner publication before the outer source fails', () => {
  const innerResolve={...resolver(),token:'r2'}, innerLoad={...loader(),token:'s2'};
  const {api,calls}=setup([resolver(),loader(),innerResolve,innerLoad,
    {kind:'value',value:Uint16Array.of(57)},{kind:'error',error:Uint16Array.of(111)}]);
  const f=fixture();f.sources=[{resolvedUri:'file:///canonical.pkl',loads:[
    {callbackPaths:[1],failure:{kind:'io',message:'outer'}}, {text:'value=9',inferredName:'inner'}]}];
  const row=runModuleLoaderCase(api,f);
  assert.equal(row.acquisitionError,null);
  assert.deepEqual(row.callbacks[0].result.valueUnits,[57]);
  assert.deepEqual(calls.filter(c=>['completeModuleSource','rejectModuleSource'].includes(c[0])),[
    ['completeModuleSource','s2','value=9','inner',0],['rejectModuleSource','s','io','outer',0]]);
});

test('logger callback uses its real pending token and preserves raw frame units', () => {
  const log={kind:'log',token:'logger',log:{level:'trace',message:Uint16Array.of(0xd800,120),
    moduleUri:Uint16Array.of(109),memberName:null,sourceLines:[Uint16Array.of(120)],startLine:1,startColumn:1,endLine:1,endColumn:2}};
  const {api,calls}=setup([log,{kind:'value',value:Uint16Array.of(57)},{kind:'value',value:Uint16Array.of(55)}]);
  const row=runModuleLoaderCase(api,fixture({logger:{callbackPaths:[1]}}));
  assert.equal(row.acquisitionError,null);
  assert.deepEqual(row.logs[0].message,[0xd800,120]);
  assert.deepEqual(calls.filter(c=>['reenterPath','acknowledgeLog'].includes(c[0])),[
    ['reenterPath','logger',1,0],['acknowledgeLog','logger',undefined,0]]);
});


test('optional metadata keeps absent, undefined and null shapes distinct', () => {
  for (const [shape,patch] of [['absent',{}],['undefined',{reason:undefined}],['null',{reason:null}]]) {
    const {api}=setup([{kind:'bytes',bytes:Uint8Array.of(55),exitCode:0,phase:'module-output',...patch}]);
    const row=runModuleLoaderCase(api,fixture({operation:'output-bytes',arguments:['eval','main.pkl'],actions:[{kind:'output-bytes'}]}));
    assert.equal(row.acquisitionError,null);
    assert.equal(row.results[0].reasonKind,shape);
    assert.equal(row.results[0].reasonPresent,shape!=='absent');
  }
});


test('nested callback records preserve admission order rather than completion order', () => {
  const nested = [];
  for (const [r,s] of [['r','s'],['r2','s2'],['r3','s3']]) nested.push({...resolver(),token:r},{...loader(),token:s});
  nested.push(...[57,56,55].map(unit=>({kind:'value',value:Uint16Array.of(unit)})));
  const {api}=setup(nested);const f=fixture();f.sources=[{resolvedUri:'file:///canonical.pkl',loads:[
    {callbackPaths:[0],text:'value=7',inferredName:'outer'},
    {callbackPaths:[1],text:'value=8',inferredName:'middle'},
    {text:'value=9',inferredName:'inner'}]}];
  const row=runModuleLoaderCase(api,f);
  assert.equal(row.acquisitionError,null);
  assert.deepEqual(row.callbacks.map(c=>c.path),[0,1]);
  assert.deepEqual(row.callbacks.map(c=>c.result.valueUnits),[[56],[57]]);
});


test('logger member name requires an explicit null or exact UTF16 vector', () => {
  for (const shape of ['omitted', 'undefined', 'wrong-type', 'null', 'units']) {
    const log={level:'trace',message:Uint16Array.of(120),moduleUri:Uint16Array.of(109),
      sourceLines:[Uint16Array.of(120)],startLine:1,startColumn:1,endLine:1,endColumn:2};
    if (shape === 'undefined') log.memberName=undefined;
    if (shape === 'wrong-type') log.memberName=[];
    if (shape === 'null') log.memberName=null;
    if (shape === 'units') log.memberName=Uint16Array.of(0xd800,120);
    const {api,calls}=setup([{kind:'log',token:'logger',log},{kind:'value',value:Uint16Array.of(55)}]);
    const row=runModuleLoaderCase(api,fixture({logger:{}}));
    if (shape === 'null' || shape === 'units') {
      assert.equal(row.acquisitionError,null);
      assert.deepEqual(row.logs[0].memberName,shape === 'null' ? null : [0xd800,120]);
    } else {
      assert.match(row.acquisitionError.message,/invalid logger frame metadata/);
      assert.equal(row.logs.length,0);
      assert.equal(calls.some(call=>call[0]==='acknowledgeLog'),false);
    }
  }
});


test('module expressions use the explicit parser-backed session factory without source rewriting', () => {
  const {api,calls}=setup([{kind:'value',value:Uint16Array.of(116,114,117,101)}]);
  const f=fixture({source:'a=1\nb=1\n',paths:['a == b']});
  const row=runModuleLoaderCase(api,f);
  assert.equal(row.acquisitionError,null);
  const call=calls.find(call=>call[0]==='forExpressions');
  assert.deepEqual(call.slice(1,5),[f.source,f.uri,f.moduleName,['a == b']]);
});
