// Native binding architecture regression, not a Java compatibility oracle.
import assert from 'node:assert/strict';
import { createRequire } from 'node:module';
import { readFile, mkdir, writeFile, copyFile } from 'node:fs/promises';
import { resolve } from 'node:path';
import { createHash } from 'node:crypto';

const [outputArgument, artifactArgument] = process.argv.slice(2);
if (!outputArgument || !artifactArgument) throw Error('usage: OUTPUT ADDON');
const output = resolve(outputArgument), artifact = resolve(artifactArgument);
await mkdir(output); // Fresh output only; never overwrite a prior observation.
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const artifactBefore = sha(await readFile(artifact));
const runnerBefore = sha(await readFile(new URL(import.meta.url)));
await copyFile(new URL(import.meta.url), `${output}/runner.mjs`);
const api = createRequire(import.meta.url)(artifact);
const units = text => Array.from({length: text.length}, (_, i) => text.charCodeAt(i));
const source = 'result = throw("\\u{d800}x\\u{dc00}")\n';
const constructorSource = 'value = (x) -> x\nlocal value = (y) -> y\n';
const uri = 'file:///error-transport.pkl';
const catchError = call => {
  try { call(); } catch (error) { return error; }
  throw Error('expected native rejection');
};
const scalar = () => api.evaluateLanguage(source, uri, 'error-transport', 'result', false);
const constructor = () => new api.SourceSession(constructorSource, uri, 'error-transport', ['result'],
  'compact', false, false, false);
// Unhooked native messages are references only for this binding architecture test.
const referenceScalar = units(catchError(scalar).message);
const referenceConstructor = units(catchError(constructor).message);
const session = new api.SourceSession('result = 7\n', uri, 'error-transport', ['result'],
  'compact', false, false, false);
const originalToString = Object.getOwnPropertyDescriptor(Error.prototype, 'toString');
const originalCause = Object.getOwnPropertyDescriptor(Error.prototype, 'cause');
const formatted = [], causeReads = [], observations = [], retained = [];
let recovery;
try {
  Object.defineProperty(Error.prototype, 'toString', {configurable: true, writable: true,
    value() { formatted.push(this); return 'unexpected native error coercion'; }});
  Object.defineProperty(Error.prototype, 'cause', {configurable: true,
    get() { causeReads.push(this); return undefined; }});
  for (const [kind, call, expected] of [
    ['scalar', scalar, referenceScalar], ['constructor', constructor, referenceConstructor],
  ]) {
    const error = catchError(call);
    retained.push({error, message: error.message});
    observations.push({kind, messageUnits: units(error.message),
      exactMessage: JSON.stringify(units(error.message)) === JSON.stringify(expected),
      nativeError: error instanceof Error, ownMessage: Object.hasOwn(error, 'message'),
      formattingCallbacks: formatted.filter(value => value === error).length,
      causeCallbacks: causeReads.filter(value => value === error).length});
  }
  session.startPath(0, 0);
  const error = catchError(() => session.startPath(0, 0));
  retained.push({error, message: error.message});
  observations.push({kind: 'session-method', messageUnits: units(error.message),
    exactMessage: error.message === 'module setup has not completed',
    nativeError: error instanceof Error, ownMessage: Object.hasOwn(error, 'message'),
    formattingCallbacks: formatted.filter(value => value === error).length,
    causeCallbacks: causeReads.filter(value => value === error).length});
  for (let i = 0; i < 100; i++) {
    recovery = session.poll(0, 10000);
    if (recovery.kind !== 'yield') break;
  }
  session.collect();
  session.close();
  session.collect();
} finally {
  Object.defineProperty(Error.prototype, 'toString', originalToString);
  if (originalCause) Object.defineProperty(Error.prototype, 'cause', originalCause);
  else delete Error.prototype.cause;
  session.close();
}
const gcAvailable = typeof global.gc === 'function';
if (gcAvailable) global.gc();
const checks = {
  explicitGcExecuted: gcAvailable,
  exactMessages: observations.every(row => row.exactMessage),
  nativeErrorObjects: observations.every(row => row.nativeError && row.ownMessage),
  noFormattingCallbacks: formatted.length === 0,
  noCauseGetterCallbacks: causeReads.length === 0,
  loneSurrogateMessage: referenceScalar.includes(0xd800) && referenceScalar.includes(0xdc00),
  recovery: recovery?.kind === 'value' && JSON.stringify(Array.from(recovery.value)) === '[55]',
  retainedObjectIdentity: new Set(retained.map(row => row.error)).size === 3,
  retainedMessagesAfterCollectAndClose: retained.every(row => row.error.message === row.message),
  artifactStable: artifactBefore === sha(await readFile(artifact)),
  runnerStable: runnerBefore === sha(await readFile(new URL(import.meta.url))),
};
const report = {schema: 1, comparisonBoundary: 'native binding architecture only; no Java expected result',
  command: process.argv, execArgv: process.execArgv, gcAvailable, gcExecuted: gcAvailable,
  node: process.version, artifact, artifactSha256: artifactBefore,
  runnerSha256: runnerBefore, source, constructorSource, uri, observations,
  formattingCallbacks: formatted.length, causeGetterCallbacks: causeReads.length,
  recovery: {...recovery, value: recovery?.value ? Array.from(recovery.value) : null},
  checks, passed: Object.values(checks).every(Boolean)};
await writeFile(`${output}/report.json`, `${JSON.stringify(report, null, 2)}\n`);
console.log(JSON.stringify({passed: report.passed, checks}));
assert.equal(report.passed, true, 'native error transport architecture regression');
