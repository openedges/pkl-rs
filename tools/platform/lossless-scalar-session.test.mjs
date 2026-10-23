import test from 'node:test';
import assert from 'node:assert/strict';
import { runLosslessScalarSingles, runLosslessScalarSequence, compareLosslessScalarRows } from './lossless-scalar-session.mjs';
const fixture={source:'result=1',uri:'file:///main.pkl',moduleName:'main',powerAssertions:true,
  cases:[{id:'value',expression:'a',kind:'value',javaUnits:[0xd800]},{id:'error',expression:'b',kind:'error',javaUnits:[0xdc00]}]};
const api={evaluateLanguage(s,u,n,p){if(p==='a')return '\ud800';throw Error('\udc00');},
  evaluateLanguageSequence(){return ['value\t\ud800','error\t\udc00'];}};
test('scalar values, thrown messages and full sequence prefixes remain exact units',()=>{
  assert.equal(compareLosslessScalarRows(fixture,runLosslessScalarSingles(api,fixture),runLosslessScalarSequence(api,fixture)).status,'pass');
});
test('UTF8 replacement and missing sequence result cannot pass',()=>{
  const wrong={...api,evaluateLanguageSequence(){return ['value\t?'];}};
  assert.equal(compareLosslessScalarRows(fixture,runLosslessScalarSingles(wrong,fixture),runLosslessScalarSequence(wrong,fixture)).status,'mismatch');
});

test('invalid scalar result types cannot masquerade as thrown language errors',()=>{
  const wrong={...api,evaluateLanguage(){return {};}};
  const rows=runLosslessScalarSingles(wrong,fixture);
  assert.equal(rows[1].kind,'invalid-result');
  assert.equal(compareLosslessScalarRows(fixture,rows,runLosslessScalarSequence(wrong,fixture)).status,'mismatch');
});

test('actual Java producer units reject surrogate substitutions despite equal Java UTF8', async () => {
  const {readFile} = await import('node:fs/promises');
  const raw = await readFile(new URL('../../tests/fixtures/m2-lossless-diagnostics/producers/repeats/0/calls/outer-0.stdout.code-units.bin', import.meta.url));
  const original = Array.from({length: raw.length / 2}, (_, i) => raw.readUInt16BE(i * 2));
  const captured = {cases: [{id: 'original-Java', kind: 'value', javaUnits: original}]};
  const prefix = Array.from('value\t', c => c.charCodeAt(0));
  assert.equal(compareLosslessScalarRows(captured, [{kind: 'value', units: original}],
    {kind: 'sequence', rows: [[...prefix, ...original]]}).status, 'pass');
  for (const replacement of [0xdc00, 0x003f]) {
    const wrong = [...original]; wrong[0] = replacement;
    assert.equal(compareLosslessScalarRows(captured, [{kind: 'value', units: wrong}],
      {kind: 'sequence', rows: [[...prefix, ...wrong]]}).status, 'mismatch');
  }
});
