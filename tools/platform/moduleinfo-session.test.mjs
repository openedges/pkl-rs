import test from 'node:test';
import assert from 'node:assert/strict';
import {runModuleInfoCase} from './moduleinfo-session.mjs';
const fixture={id:'moduleinfo',repeat:0,comparisonScope:'ordinary-full-diagnostic',source:'source',moduleUri:'file:///original/main.pkl',moduleName:'moduleinfo.control',expressions:['field','class','field']};
class Session {
  startPath(index){this.index=index;}
  poll(){return {kind:'value',value:Uint16Array.from([55])};}
  collect(){}close(){}free(){}heapSlots(){return 0;}
}
const api=Class=>({SourceSession:{forExpressions:()=>new Class()}});
test('forwards exact original URI/module name/source/ordered expressions',()=>{
  let args;
  const result=runModuleInfoCase({SourceSession:{forExpressions(...value){args=value;return new Session();}}},fixture);
  assert.deepEqual(args.slice(0,4),[fixture.source,fixture.moduleUri,fixture.moduleName,fixture.expressions]);
  assert.deepEqual(result.calls.map(c=>c.prefix),['outer-0','outer-1','outer-2']);assert.equal(result.acquisitionError,null);
});
test('constructor diagnostic preserves unmatched surrogate units',()=>{
  const result=runModuleInfoCase({SourceSession:{forExpressions(){throw Error('bad\ud800');}}},fixture);
  assert.deepEqual(result.constructorError.messageUnits,[98,97,100,0xd800]);assert.deepEqual(result.calls,[]);
});
test('array value is rejected at typed transport boundary',()=>{
  class Invalid extends Session {poll(){return {kind:'value',value:[55]};}}
  const result=runModuleInfoCase(api(Invalid),fixture);assert.match(result.acquisitionError.message,/expected Uint16Array/);assert.equal(result.architecture.terminalSlots,0);
});
test('private bootstrap leaking Host request cannot be counted complete',()=>{
  class HostLeak extends Session {poll(){return {kind:'read',token:'read',uri:'prop:pkl.outputFormat'};}}
  const result=runModuleInfoCase(api(HostLeak),fixture);assert.match(result.acquisitionError.message,/unexpected ModuleInfo event read/);assert.deepEqual(result.calls,[]);
});
test('internal Unsupported metadata and actual units remain available',()=>{
  class Internal extends Session {poll(){return {kind:'error',error:Uint16Array.from([0xd800,65]),disposition:'unsupported',phase:'runtime',reason:'version parser'};}}
  const result=runModuleInfoCase(api(Internal),{...fixture,comparisonScope:'java-internal-parser-boundary'});
  assert.equal(result.acquisitionError,null);assert.equal(result.calls[0].disposition,'unsupported');assert.deepEqual(result.calls[0].units,[0xd800,65]);
});
test('retained mutable arrays are detected after session release',()=>{
  class Borrowed extends Session {constructor(){super();this.view=Uint16Array.from([55]);}poll(){return {kind:'value',value:this.view};}free(){this.view[0]=99;}}
  const result=runModuleInfoCase(api(Borrowed),fixture);assert.equal(result.architecture.ownedResultsStable,false);assert.deepEqual(result.calls[0].units,[55]);
});
