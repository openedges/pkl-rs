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
    completeDefaultModule(...args) { calls.push(['completeDefaultModule', ...args]); return options.defaultModule ? options.defaultModule(...args) : false; }
  }
  for (const method of ['startPath','startOutputBytes','completeModuleKey','completeModuleCachePolicy','completeModuleResolution','completeModuleSource',
    'rejectReadIo','reject','rejectModuleResolution','rejectModuleSource','reenterPath','closeFromCallback','respondString','respondConfigured','acknowledgeLog'])
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

test('configured key selection and cache-policy queries preserve identities before source resolution', () => {
  const keyId = '18446744073709551615', uri = 'file:///child%2520.pkl';
  const descriptor = {keyId, keyKind: 'host', keyHandle: `fixture-key:${keyId}`};
  const select = {kind: 'module-request', token: 'select', module: {id: '10', kind: 'select-key',
    originalUri: uri, keyId, selectionPhase: 'runtime-import'}};
  const policy = {kind: 'module-request', token: 'policy', module: {id: '11', kind: 'query-cache-policy',
    originalUri: uri, ...descriptor}};
  const resolve = resolver(); Object.assign(resolve.module, descriptor, {resolvedKeyId: '9223372036854775808'});
  const source = loader(); Object.assign(source.module, descriptor,
    {resolvedKeyId: '9223372036854775808', resolvedKeyHandle: 'resolved-fixture'});
  const {api, calls} = setup([select, policy, resolve, source, {kind: 'value', value: Uint16Array.of(55)}],
    {defaultModule: () => assert.fail('configured factory and loader must take precedence')});
  const row = runModuleLoaderCase(api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(row.requests.map(request => request.kind), ['select-key', 'query-cache-policy', 'resolve', 'load-source']);
  assert.deepEqual(calls.filter(call => call[0].startsWith('complete')).slice(0, 2), [
    ['completeModuleKey', 'select', keyId, uri, 'host', `fixture-key:${keyId}`, 0],
    ['completeModuleCachePolicy', 'policy', keyId, true, 0],
  ]);
  assert.equal(row.requests[3].resolvedKeyHandle, 'resolved-fixture');
});

test('malformed selected-key identity fails before fixture resolution consumes the token', () => {
  const malformed = resolver(); malformed.module.keyId = '7';
  const {api, calls} = setup([malformed]);
  const row = runModuleLoaderCase(api, fixture());
  assert.match(row.acquisitionError.message, /module request context/);
  assert.equal(calls.some(call => call[0] === 'completeModuleResolution'), false);
});

test('hierarchical file factory selection is independent of source existence and main-module resolution entries', () => {
  for (const uri of ['file:///missing.pkl', 'FILE:/missing.pkl', 'file:///main.pkl']) {
    const event = {kind: 'module-request', token: 'select', module: {id: '1', kind: 'select-key',
      originalUri: uri, keyId: '9', selectionPhase: 'clause-preparation'}};
    const {api, calls} = setup([event, {kind: 'value', value: Uint16Array.of(55)}],
      {defaultModule: () => assert.fail('file key selection must precede default fallback')});
    const row = runModuleLoaderCase(api, fixture({resolutions: [], sources: []}));
    assert.equal(row.acquisitionError, null);
    assert.deepEqual(calls.filter(call => call[0].startsWith('complete')), [
      ['completeModuleKey', 'select', '9', uri, 'host', 'fixture-key:9', 0],
    ]);
    assert.deepEqual(row.results[0].valueUnits, [55]);
  }
});

test('unconfigured non-file and opaque file keys stay explicit adapter failures', () => {
  for (const uri of ['unknown:/missing', 'file:opaque']) {
    const event = {kind: 'module-request', token: 'select', module: {id: '1', kind: 'select-key',
      originalUri: uri, keyId: '9', selectionPhase: 'runtime-import'}};
    const {api, calls} = setup([event]);
    const row = runModuleLoaderCase(api, fixture({resolutions: [], sources: []}));
    assert.match(row.acquisitionError.message, /not configured/);
    assert.equal(calls.some(call => call[0] === 'completeModuleKey'), false);
  }
});

test('missing bundle resolution rejects the correct phase and retains UTF16 diagnostics', () => {
  const { api, calls } = setup([resolver('life:/missing'), {kind:'error',error:Uint16Array.of(0xd800,0,120)}]);
  const row = runModuleLoaderCase(api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.deepEqual(calls.find(c => c[0] === 'rejectModuleResolution'), ['rejectModuleResolution','r','not-found','',0]);
  assert.deepEqual(row.results[0].errorUnits,[0xd800,0,120]);
});

test('shared default admission handles import checks and bundled resolution without JavaScript URI policy', () => {
  const edge = resolver('pkl:json'); edge.module.kind = 'check-import'; edge.token = 'edge';
  edge.module.importerResolvedUri = 'file:///resolved.pkl';
  const {api, calls} = setup([edge, resolver('pkl:json'), {kind:'value',value:Uint16Array.of(55)}],
    {defaultModule: () => true});
  const row = runModuleLoaderCase(api, fixture());
  assert.equal(row.acquisitionError, null);
  assert.equal(row.requests[0].importerResolvedUri, 'file:///resolved.pkl');
  assert.deepEqual(calls.filter(call => call[0] === 'completeDefaultModule'),
    [['completeDefaultModule', 'edge', 0], ['completeDefaultModule', 'r', 0]]);
  assert.equal(calls.some(call => call[0] === 'rejectModuleResolution'), false);
});

test('explicit resolution denial precedes default resolution and helper errors cannot become NotFound', () => {
  const f = fixture(); f.resolutions[0].failure = {kind: 'denied', message: 'custom denial'};
  const denied = setup([resolver(), {kind:'error',error:Uint16Array.of(120)}],
    {defaultModule: () => assert.fail('configured denial must win')});
  assert.equal(runModuleLoaderCase(denied.api, f).acquisitionError, null);
  assert.deepEqual(denied.calls.find(call => call[0] === 'rejectModuleResolution'),
    ['rejectModuleResolution', 'r', 'denied', 'custom denial', 0]);
  const stale = setup([resolver('pkl:json')], {defaultModule: () => { throw Error('stale token'); }});
  const result = runModuleLoaderCase(stale.api, fixture());
  assert.equal(result.acquisitionError, null);
  assert.equal(result.results[0].kind, 'thrown');
  assert.equal(result.results[0].error.message, 'stale token');
  assert.equal(stale.calls.some(call => call[0] === 'rejectModuleResolution'), false);
});

test('a malformed default completion result cannot trigger the external NotFound fallback', () => {
  for (const result of [undefined, null, 0, 'unsupported']) {
    const {api, calls} = setup([resolver('pkl:json')], {defaultModule: () => result});
    const row = runModuleLoaderCase(api, fixture());
    assert.match(row.acquisitionError.message, /invalid default module completion result/);
    assert.equal(calls.some(call => call[0] === 'rejectModuleResolution'), false);
  }
});

test('duplicate identities and malformed request contexts are acquisition errors', () => {
  const a = setup([]), f = fixture(); f.sources.push({...f.sources[0]});
  assert.match(runModuleLoaderCase(a.api,f).acquisitionError.message,/duplicate/);
  assert.equal(a.calls.length,0);
  const malformed = resolver(); delete malformed.module.importerOriginalUri;
  const b = setup([malformed]);
  const row = runModuleLoaderCase(b.api,fixture());
  assert.equal(row.acquisitionError.message, 'invalid default-policy module request context');
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

test('resolve retry plan preserves null IO, empty message and prepared identities', () => {
  for (const message of [null, '', '\ud83d|\udc00\0']) {
    const {api,calls} = setup([resolver(),{kind:'error',error:Uint16Array.of(65)},resolver(),loader(),{kind:'value',value:Uint16Array.of(57)}]);
    const f = fixture({actions:[{kind:'path',index:0},{kind:'path',index:0}]});
    f.resolutions=[{importUri:'file:///child%2520.pkl',resolves:[{failure:{kind:'io',message}},
      {resolvedUri:'file:///canonical.pkl',sourceUri:'file:///alias.pkl'}]}];
    const row=runModuleLoaderCase(api,f);
    assert.equal(row.acquisitionError,null);
    assert.deepEqual(calls.filter(c=>c[0]==='rejectModuleResolution'),[['rejectModuleResolution','r','io',message,0]]);
    assert.deepEqual(calls.find(c=>c[0]==='completeModuleResolution'),
      ['completeModuleResolution','r','file:///child%2520.pkl','file:///canonical.pkl','file:///alias.pkl',0]);
    assert.deepEqual(row.results.map(r=>r.kind),['error','value']);
  }
});

test('read plans preserve IO-vs-raw distinction and explicit property completions', () => {
  const message='\ud83d|\udc00\0';
  const events=[{kind:'read',token:'p',uri:'prop:pkl.outputFormat'},
    {kind:'read',token:'r1',uri:'lossless:/resource'},{kind:'error',error:Uint16Array.of(0xd83d)},
    {kind:'read',token:'r2',uri:'lossless:/resource'},{kind:'value',value:Uint16Array.of(57)}];
  const {api,calls}=setup(events);
  const f=fixture({actions:[{kind:'path',index:0},{kind:'path',index:0}],resources:[
    {uri:'prop:pkl.outputFormat',valueUnits:[0xd83d,0xdc00]},
    {uri:'lossless:/resource',reads:[{failure:{kind:'io',message:null}},{valueUnits:[57]}]},
  ]});
  const row=runModuleLoaderCase(api,f);
  assert.equal(row.acquisitionError,null);
  assert.deepEqual(calls.find(c=>c[0]==='respondString'),['respondString','p',Uint16Array.of(0xd83d,0xdc00),0]);
  assert.deepEqual(calls.find(c=>c[0]==='rejectReadIo'),['rejectReadIo','r1',null,0]);
  assert.equal(calls.some(c=>c[0]==='respondConfigured'),false);
  assert.deepEqual(row.reads.map(r=>r.uri),['prop:pkl.outputFormat','lossless:/resource','lossless:/resource']);
  const raw=setup([{kind:'read',token:'raw',uri:'lossless:/resource'},{kind:'error',error:Uint16Array.of(0xdc00)}]);
  assert.equal(runModuleLoaderCase(raw.api,fixture({resources:[{uri:'lossless:/resource',failure:{kind:'diagnostic',message}}]})).acquisitionError,null);
  assert.deepEqual(raw.calls.find(c=>c[0]==='reject'),['reject','raw',message,0]);
});

test('omitted nullable IO input and exhausted plans are acquisition failures', () => {
  const first=setup([]);
  const invalid=fixture({resolutions:[{importUri:'file:///child%2520.pkl',failure:{kind:'io'}}]});
  assert.match(runModuleLoaderCase(first.api,invalid).acquisitionError.message,/invalid module Host failure/);
  assert.equal(first.calls.length,0);
  const {api}=setup([resolver(),{kind:'error',error:Uint16Array.of(65)},resolver()]);
  const f=fixture({resolutions:[{importUri:'file:///child%2520.pkl',resolves:[{failure:{kind:'io',message:''}}]}],
    actions:[{kind:'path',index:0},{kind:'path',index:0}]});
  const row=runModuleLoaderCase(api,f);
  assert.match(row.acquisitionError.message,/plan exhausted/);
  assert.equal(row.results.length,1);
});
