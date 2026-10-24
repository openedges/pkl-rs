// Executed from an immutable native acquisition directory, beside its snapshots.
import fs from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {compareCollectionTransformCase} from './helpers/collection-transform-session.mjs';
const root = path.dirname(fileURLToPath(import.meta.url));
const fixtures = JSON.parse(fs.readFileSync(path.join(root, 'fixtures.json')));
const actual = JSON.parse(fs.readFileSync(path.join(root, 'actual.json')));
if (!fixtures.length || fixtures.length !== actual.length) throw Error('session count mismatch');
const sessions = fixtures.map((fixture, index) => compareCollectionTransformCase(fixture, actual[index]));
const summary = {sessions: sessions.length, calls: actual.reduce((sum, row) => sum + row.calls.length, 0),
  rawMatches: sessions.reduce((sum, row) => sum + row.rawByteMatches, 0),
  ordinaryCount: sessions.reduce((sum, row) => sum + row.ordinaryCount, 0),
  ordinaryCompatible: sessions.reduce((sum, row) => sum + row.ordinaryCompatible, 0),
  acquisitionFailures: actual.filter((row, index) => !sessions[index].acquisitionIntegrityPassed)
    .map(row => ({id: row.id, error: row.acquisitionError})),
  orderedEffectsFailures: sessions.filter(row => !row.orderedEffectsPassed).map(row => row.id),
  fullJavaEventsCountersPassed: false, fullCompatibilityPassed: false, results: sessions};
summary.ordinaryContractPassed = !summary.acquisitionFailures.length && !summary.orderedEffectsFailures.length
  && summary.ordinaryCount === summary.ordinaryCompatible;
fs.writeFileSync(path.join(root, 'comparison.json'), JSON.stringify(summary, null, 2) + '\n');
