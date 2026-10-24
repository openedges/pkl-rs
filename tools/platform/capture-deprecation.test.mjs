import test from 'node:test';
import assert from 'node:assert/strict';
import { requirePairs } from './capture-deprecation.mjs';

const cases = [{ id: 'first' }, { id: 'second' }];
const complete = cases.flatMap(row => [0, 1].map(repeat => ({ id: row.id, repeat })));

test('missing, duplicate, foreign, and invalid repetitions cannot satisfy acquisition coverage', () => {
  assert.doesNotThrow(() => requirePairs(cases, complete));
  assert.throws(() => requirePairs(cases, complete.slice(1)), /missing/);
  assert.throws(() => requirePairs(cases, [...complete.slice(1), complete[1]]), /duplicate/);
  assert.throws(() => requirePairs(cases, [...complete.slice(1), { id: 'foreign', repeat: 0 }]), /unexpected/);
  assert.throws(() => requirePairs(cases, [...complete.slice(1), { id: 'first', repeat: 2 }]), /unexpected/);
});
