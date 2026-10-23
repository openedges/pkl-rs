import test from 'node:test';
import assert from 'node:assert/strict';
import {runGuestCatchCase} from './guest-catch-session.mjs';
const base={group:'test',id:'transport',repeat:0,comparisonScope:'full-ordered-api',source:'source',moduleUri:'file:///original.pkl',mode:'plain',expressions:['outer'],callbacks:[]};
const value=text=>({kind:'value',value:Uint16Array.from(Array.from(text,c=>c.charCodeAt(0)))});
function apiFor(Session){return {SourceSession:{forExpressions:()=>new Session()}};}
class Session {
  close(){}collect(){}heapSlots(){return 0;}free(){}
  startPath(){this.queue=[value('done')];}
  poll(){return this.queue.shift();}
}
test('constructor rejection retains unmatched surrogate units',()=>{
  const api={SourceSession:{forExpressions(){throw new Error('bad\ud83d|\udc00');}}};
  const result=runGuestCatchCase(api,{...base,comparisonScope:'frontend-constructor-only'});
  assert.deepEqual(result.constructorError.messageUnits,[98,97,100,0xd83d,124,0xdc00]);
  assert.equal(result.acquisitionError,null);assert.deepEqual(result.calls,[]);
});
test('completion rejects a JS array at the typed-unit boundary',()=>{
  class Invalid extends Session {poll(){return {kind:'value',value:[1]};}}
  const result=runGuestCatchCase(apiFor(Invalid),base);
  assert.match(result.acquisitionError.message,/expected Uint16Array/);
  assert.equal(result.architecture.terminalSlots,0);
});
test('resource callback finishes before suspended outer completion',()=>{
  class Callback extends Session {
    startPath(){this.queue=[{kind:'read',token:'request',uri:'catch:/value'}];}
    reenterPath(token,index){assert.equal(token,'request');assert.equal(index,1);this.queue.push(value('inner'));}
    rejectReadIo(token,message){assert.equal(token,'request');assert.equal(message,'read-first');this.queue.push(value('outer'));}
  }
  const result=runGuestCatchCase(apiFor(Callback),{...base,mode:'read-reentry',callbacks:['callback']});
  assert.equal(result.acquisitionError,null);
  assert.deepEqual(result.calls.map(c=>c.prefix),['inner-0','outer-0']);
  assert.deepEqual(result.events,['read catch:/value attempt=1','callback-start','inner-0 success','callback-end','outer-0 success']);
  assert.deepEqual(result.counts,{resolve:0,source:0,read:1,log:0,callback:1,event:5});
  assert.equal(result.architecture.ownedResultsStable,true);
});
test('missing nullable logger member remains a transport error',()=>{
  class InvalidLog extends Session {poll(){return {kind:'log',token:'logger',log:{message:Uint16Array.from([1])}};}}
  const result=runGuestCatchCase(apiFor(InvalidLog),base);
  assert.match(result.acquisitionError.message,/missing nullable logger member/);
});
test('terminal timeout preserves one cancelled request and rejects both followups',()=>{
  class Timeout extends Session {
    startPath(index){if(index===0)this.queue=[value('7')];else if(index===1)this.queue=[{kind:'read',token:'pending',uri:'catch:/value'}];else throw Error('closed');}
    poll(now){return now===3001?{kind:'error',error:Uint16Array.from([84]),disposition:'language-error'}:super.poll();}
    drainCancelledReads(){return ['pending'];}drainCancelledModules(){return [];}
  }
  const result=runGuestCatchCase(apiFor(Timeout),{...base,comparisonScope:'terminal-partial-parity',id:'read-timeout-not-ordinary-catch',mode:'read-timeout',expressions:['stable','caught','stable','caught']});
  assert.equal(result.acquisitionError,null);assert.equal(result.terminalFailure.kind,'error');
  assert.deepEqual(result.architecture.cancelledReads,['pending']);assert.deepEqual(result.followupRejections,[true,true]);
  assert.equal(result.calls.length,1);
});
