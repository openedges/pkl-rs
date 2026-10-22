// Synthetic driver logic checks; these are not Java compatibility observations.
import assert from 'node:assert/strict';
import test from 'node:test';
import { compareLanguagePayload, evaluateLanguageFixtures } from './compare-language-cases.mjs';

test('explicit power flags override context in both directions and absent flags inherit', () => {
  for (const inherited of [false, true]) {
    const observed = [];
    const api = { evaluateLanguage(_source, _uri, _module, expression, power) {
      observed.push({ expression, power });
      return '7';
    } };
    const fixtures = [{ id: 'inherit', source: '', expression: 'inherit' },
      { id: 'false', source: '', expression: 'false', powerAssertions: false },
      { id: 'true', source: '', expression: 'true', powerAssertions: true }];
    const results = evaluateLanguageFixtures(api, fixtures, { uri: 'file:///synthetic.pkl',
      moduleName: 'synthetic', powerAssertions: inherited }, 1);
    assert.deepEqual(observed, [{ expression: 'inherit', power: inherited },
      { expression: 'false', power: false }, { expression: 'true', power: true }]);
    assert.equal(results.length, 3);
  }
});

test('UTF16 comparison distinguishes lone units despite identical UTF8 replacement', () => {
  const observed = compareLanguagePayload({ expectedValue: '?', expectedCodeUnitsHex: 'd800' }, 'value', '\udc00');
  assert.equal(observed.nodeUtf8Match, true);
  assert.equal(observed.originalJavaUtf8Match, false);
  assert.equal(observed.codeUnitsMatch, false);
  assert.equal(observed.status, 'mismatch');
  assert.equal(observed.actualCodeUnits.toString('hex'), 'dc00');
});

test('Java UTF8 question-mark boundary does not conceal an expected lone surrogate', () => {
  const observed = compareLanguagePayload({ expectedError: '?', expectedCodeUnitsHex: 'd800' }, 'error', '?');
  assert.equal(observed.originalJavaUtf8Match, true);
  assert.equal(observed.nodeUtf8Match, false);
  assert.equal(observed.codeUnitsMatch, false);
  assert.equal(observed.status, 'mismatch');
});

test('matching internal-error payload is always oracle-error and never pass', () => {
  const observed = compareLanguagePayload({ expectedError: 'synthetic internal error', oracleError: true,
    expectedCodeUnitsHex: Buffer.from('synthetic internal error', 'utf16le').swap16().toString('hex') },
  'error', 'synthetic internal error');
  assert.equal(observed.rawMatch, true);
  assert.equal(observed.status, 'oracle-error');
  assert.equal([observed].filter(row => row.status === 'pass').length, 0);
});

test('unmarked legacy fixtures retain UTF8 pass/mismatch classification', () => {
  assert.equal(compareLanguagePayload({ expectedValue: '7' }, 'value', '7').status, 'pass');
  assert.equal(compareLanguagePayload({ expectedError: 'error' }, 'error', 'different').status, 'mismatch');
  assert.equal(compareLanguagePayload({ expectedError: 'error' }, 'value', 'error').status, 'mismatch');
});


test('exact lone surrogate passes JS string boundary while preserving Java serialization divergence', () => {
  for (const [unit, text] of [['d800', '\ud800'], ['dc00', '\udc00']]) {
    const observed = compareLanguagePayload({ expectedValue: '?', expectedCodeUnitsHex: unit }, 'value', text);
    assert.equal(observed.status, 'pass');
    assert.equal(observed.rawMatch, true);
    assert.equal(observed.codeUnitsMatch, true);
    assert.equal(observed.nodeUtf8Match, true);
    assert.equal(observed.originalJavaUtf8Match, false);
    assert.equal(observed.javaVsNodeSerializationDivergence, true);
    assert.equal(observed.comparisonBoundary, 'js-string-api');
    assert.equal(observed.expected.toString('hex'), '3f');
    assert.equal(observed.expectedNodeUtf8.toString('hex'), 'efbfbd');
    assert.equal(observed.actualCodeUnits.toString('hex'), unit);
  }
});

test('supplementary paired units and lone-unit text preserve exact UTF16 ordering', () => {
  const paired = compareLanguagePayload({ expectedValue: '😀', expectedCodeUnitsHex: 'd83dde00' }, 'value', '😀');
  assert.equal(paired.status, 'pass');
  assert.equal(paired.javaVsNodeSerializationDivergence, false);
  assert.equal(paired.actualCodeUnits.toString('hex'), 'd83dde00');
  assert.equal(compareLanguagePayload({ expectedValue: '😀', expectedCodeUnitsHex: 'd83dde00' }, 'value', '\ude00\ud83d').status, 'mismatch');
  const mixed = compareLanguagePayload({ expectedValue: '?A', expectedCodeUnitsHex: 'd8000041' }, 'value', '\ud800A');
  assert.equal(mixed.status, 'pass');
  assert.equal(mixed.javaVsNodeSerializationDivergence, true);
  assert.equal(mixed.actualCodeUnits.toString('hex'), 'd8000041');
});
