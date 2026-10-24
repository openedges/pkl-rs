import test from 'node:test';
import assert from 'node:assert/strict';
import { admitPairs } from './compare-parameterized-objects.mjs';

test('all ordinary cases require two distinct fresh execution pairs', () => {
  const cases = [{ id: 'a' }, { id: 'b' }];
  const rows = cases.flatMap(row => [0, 1].map(repeat => ({ id: row.id, repeat })));
  assert.doesNotThrow(() => admitPairs(cases, rows));
  assert.throws(() => admitPairs(cases, rows.slice(1)), /missing/);
  assert.throws(() => admitPairs(cases, [...rows, rows[0]]), /duplicate/);
  assert.throws(() => admitPairs(cases, [...rows.slice(1), { id: 'a', repeat: 2 }]), /unexpected/);
});
