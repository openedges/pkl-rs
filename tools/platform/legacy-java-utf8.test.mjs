import test from 'node:test';
import assert from 'node:assert/strict';
import {legacyJavaUtf8Observation} from './legacy-java-utf8.mjs';
test('historical Java byte boundary retains units and distinguishes byte changes', () => {
  for (const text of ['\ud800', '\udc00', '?']) {
    const row = legacyJavaUtf8Observation(text, '?');
    assert.equal(row.matches, true);
    assert.equal(row.actualUnits[0], text.charCodeAt(0));
  }
  assert.equal(legacyJavaUtf8Observation('🙂\0\r\n', '🙂\0\r\n').matches, true);
  assert.equal(legacyJavaUtf8Observation('�', '?').matches, false);
  assert.equal(legacyJavaUtf8Observation('a\r\n', 'a\n').matches, false);
});
