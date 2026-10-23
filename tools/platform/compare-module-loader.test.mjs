import { fileURLToPath } from 'node:url';
import { runModuleLoaderCase } from './module-loader-session.mjs';
import test from 'node:test';
import assert from 'node:assert/strict';
import { compareLoaderResult, javaUtf8, selectModuleFixtureVerifier } from './compare-module-loader.mjs';

test('identical diagnostic bytes cannot make Unsupported expression completion pass',()=>{
  const expected={kind:'error',javaStatus:'error',javaUtf8Hex:'626164'};
  const actual={kind:'error',errorUnits:[98,97,100],disposition:'unsupported'};
  const result=compareLoaderResult(expected,actual);
  assert.equal(result.rawMatch,true);
  assert.equal(result.status,'candidate-unsupported');
  assert.equal(compareLoaderResult(expected,{...actual,disposition:'language-error'}).status,'pass');
  assert.equal(compareLoaderResult(expected,{kind:'error',errorUnits:[98,97,100]}).status,'candidate-classification-missing');
});

test('Java UTF8 serialization retains its distinct boundary while exact captured units reject substitutions',()=>{
  assert.deepEqual([...javaUtf8([0xd800,0,0xd83d,0xde00])],[63,0,240,159,152,128]);
  const expected={kind:'value',javaStatus:'success',javaUtf8Hex:'3f',javaValueUnits:[0xd800]};
  assert.equal(compareLoaderResult(expected,{kind:'value',valueUnits:[0xd800]}).status,'pass');
  const changed=compareLoaderResult(expected,{kind:'value',valueUnits:[63]});
  assert.equal(changed.channels.payload,true);
  assert.equal(changed.channels.codeUnits,false);
  assert.equal(changed.status,'mismatch');
});

test('output bytes and development exit are compared without decoding or trimming',()=>{
  const expected={kind:'bytes',javaStatus:'success',javaExitCode:0,bytesHex:'00c3a90d0a'};
  assert.equal(compareLoaderResult(expected,{kind:'bytes',bytes:[0,195,169,13,10],exitCode:0}).status,'pass');
  assert.equal(compareLoaderResult(expected,{kind:'bytes',bytes:[0,195,169,10],exitCode:0}).status,'mismatch');
  assert.equal(compareLoaderResult(expected,{kind:'bytes',bytes:[0,195,169,13,10],exitCode:2}).status,'mismatch');
});

test('oracle errors remain failures even when full payload matches',()=>{
  assert.equal(compareLoaderResult({kind:'error',javaStatus:'oracle-error',javaUtf8Hex:'62'},
    {kind:'error',errorUnits:[98],disposition:'language-error'}).status,'oracle-error');
});

test('fixture verifier selection preserves the original default and registers inheritance explicitly',()=>{
  const args=['output','wasm','addon','fixtures'];
  const original=fileURLToPath(new URL('../module_loading_fixtures.py',import.meta.url));
  const inheritance=fileURLToPath(new URL('../module_inheritance_fixtures.py',import.meta.url));
  assert.equal(selectModuleFixtureVerifier(args),original);
  assert.equal(selectModuleFixtureVerifier([...args,inheritance]),inheritance);
  assert.throws(()=>selectModuleFixtureVerifier([...args,'/tmp/module_inheritance_fixtures.py']),/unregistered/);
  assert.throws(()=>selectModuleFixtureVerifier([...args,'']),/unregistered/);
  assert.throws(()=>selectModuleFixtureVerifier(args.slice(0,3)),/usage/);
  assert.throws(()=>selectModuleFixtureVerifier([...args,inheritance,'extra']),/usage/);
});

test('a constructor rejection cannot replace two Java per-call setup error events',()=>{
  const message='Cannot instantiate abstract class parent';
  const api={SourceSession:{forExpressions(){throw Error(message);}}};
  const fixture={id:'setup-error',operation:'expressions',source:'amends "life:/parent"',
    uri:'file:///main.pkl',moduleName:'main',paths:['existing','existing'],
    actions:[{kind:'path',index:0},{kind:'path',index:1}],resolutions:[],sources:[]};
  const row=runModuleLoaderCase(api,fixture);
  assert.equal(row.results.length,0);
  assert.equal(row.acquisitionError.message,message);
  const expected={kind:'error',javaStatus:'error',javaUtf8Hex:Buffer.from(message).toString('hex')};
  assert.equal(compareLoaderResult(expected,row.results[0]).rawMatch,false);
  assert.equal(compareLoaderResult(expected,row.results[0]).status,'acquisition-failed');
});

test('registered ordered string API fixture verifier retains exact UTF16 errors', () => {
  assert.match(selectModuleFixtureVerifier(['out','wasm','addon','fixtures','tools/string_subscript_fixtures.py']), /string_subscript_fixtures\.py$/);
  const expected={kind:'error',javaStatus:'error',javaUtf8Hex:'3f',javaErrorUnits:[0xd83d]};
  const replaced=compareLoaderResult(expected,{kind:'error',errorUnits:[0x3f],disposition:'language-error'});
  assert.equal(replaced.channels.payload,true);
  assert.equal(replaced.channels.codeUnits,false);
  assert.equal(replaced.status,'mismatch');
  const exact=compareLoaderResult(expected,{kind:'error',errorUnits:[0xd83d],disposition:'language-error'});
  assert.equal(exact.status,'pass');
});
