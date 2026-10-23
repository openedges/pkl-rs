import test from 'node:test';
import assert from 'node:assert/strict';
import {runSecurityDenialCase} from './security-denial-session.mjs';
const fixture={id:'denial',repeat:0,source:'source',moduleUri:'file:///original/main.pkl',deniedUri:'file:///original/child.pkl',expressions:['caught','nullable','ordinary','stable','caught'],causes:Array.from({length:3},()=>({uri:'file:///original/child.pkl',errorClass:'org.pkl.core.SecurityManagerException',units:[65,0xd800]}))};
class Session{
 startPath(index){this.index=index;this.pending=index<3;}
 poll(){if(this.pending){this.pending=false;return {kind:'module-request',token:'request-'+this.index,module:{kind:'resolve',originalUri:fixture.deniedUri}};}return {kind:this.index===2?'error':'value',value:Uint16Array.from([55]),error:Uint16Array.from([69]),disposition:this.index===2?'language-error':null};}
 rejectModuleResolution(token,kind,message){assert.equal(token,'request-'+this.index);assert.equal(kind,'security-manager-exception');assert.deepEqual(Array.from({length:message.length},(_,i)=>message.charCodeAt(i)),[65,0xd800]);}
 close(){}collect(){}free(){}heapSlots(){return 0;}
}
const api=Class=>({SourceSession:{forExpressions:()=>new Class()}});
test('typed exception cause preserves full units and denied/cached sequence',()=>{const result=runSecurityDenialCase(api(Session),fixture);assert.equal(result.acquisitionError,null);assert.deepEqual(result.calls.map(c=>c.denialsAfter),[1,2,3,3,3]);assert.equal(result.denials.length,3);assert.equal(result.architecture.terminalSlots,0);});
test('generic raw cause cannot acquire typed provenance',()=>{const result=runSecurityDenialCase(api(Session),{...fixture,causes:[{...fixture.causes[0],errorClass:'generic-host'}]});assert.match(result.acquisitionError.message,/typed security resolve\/cause contract/);assert.equal(result.denials.length,0);});
test('source-stage request cannot use resolve-only exception',()=>{class Source extends Session{poll(){return {kind:'module-request',token:'source',module:{kind:'load-source',originalUri:fixture.deniedUri}};}rejectModuleResolution(){throw Error('must not consume source token');}}const result=runSecurityDenialCase(api(Source),fixture);assert.match(result.acquisitionError.message,/typed security resolve\/cause contract/);assert.equal(result.denials.length,0);});
test('wrong original URI is a transport failure',()=>{class Wrong extends Session{poll(){return {kind:'module-request',token:'other',module:{kind:'resolve',originalUri:'file:///wrong.pkl'}};}}const result=runSecurityDenialCase(api(Wrong),fixture);assert.match(result.acquisitionError.message,/typed security resolve\/cause contract/);});
test('constructor error preserves unmatched surrogate units',()=>{const result=runSecurityDenialCase({SourceSession:{forExpressions(){throw Error('bad\ud800');}}},fixture);assert.deepEqual(result.constructorError.messageUnits,[98,97,100,0xd800]);});
test('untyped completion arrays are rejected',()=>{class Wrong extends Session{poll(){return {kind:'value',value:[55]};}}const result=runSecurityDenialCase(api(Wrong),fixture);assert.match(result.acquisitionError.message,/expected Uint16Array/);});
