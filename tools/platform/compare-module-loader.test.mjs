import test from 'node:test';
import assert from 'node:assert/strict';
import { compareLoaderResult, javaUtf8 } from './compare-module-loader.mjs';

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
