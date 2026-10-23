import { stringUnits } from './lossless-constructor-session.mjs';
export function runLosslessScalarSingles(api, fixture) {
  return fixture.cases.map(c => {
    let value;
    try {
      value = api.evaluateLanguage(fixture.source, fixture.uri, fixture.moduleName,
        c.expression, fixture.powerAssertions);
    } catch (error) {
      return { id: c.id, kind: 'error', isError: error instanceof Error,
        units: typeof error?.message === 'string' ? stringUnits(error.message) : null };
    }
    return typeof value === 'string' ? { id: c.id, kind: 'value', units: stringUnits(value) }
      : { id: c.id, kind: 'invalid-result', valueType: typeof value };
  });
}
export function runLosslessScalarSequence(api, fixture) {
  try {
    const values = api.evaluateLanguageSequence(fixture.source, fixture.uri,
      fixture.moduleName, fixture.cases.map(c => c.expression), fixture.powerAssertions);
    if (!Array.isArray(values)) return { kind: 'invalid-result', valueType: typeof values };
    return { kind: 'sequence', rows: values.map(value => stringUnits(value)) };
  } catch (error) {
    return { kind: 'thrown', isError: error instanceof Error,
      units: typeof error?.message === 'string' ? stringUnits(error.message) : null };
  }
}
export function compareLosslessScalarRows(fixture, singles, sequence) {
  const exact = (a,b) => Array.isArray(a) && a.length === b.length && a.every((u,i) => u === b[i]);
  const singleResults = fixture.cases.map((c,i) => ({ id: c.id,
    kindMatch: singles[i]?.kind === c.kind,
    unitsMatch: exact(singles[i]?.units, c.javaUnits),
    errorObjectMatch: c.kind !== 'error' || singles[i]?.isError === true }));
  const sequenceResults = fixture.cases.map((c,i) => ({ id: c.id,
    unitsMatch: sequence.kind === 'sequence' && exact(sequence.rows[i],
      [...stringUnits(c.kind === 'value' ? 'value\t' : 'error\t'), ...c.javaUnits]) }));
  const passed = singles.length === fixture.cases.length && sequence.kind === 'sequence'
    && sequence.rows.length === fixture.cases.length
    && singleResults.every(r => r.kindMatch && r.unitsMatch && r.errorObjectMatch)
    && sequenceResults.every(r => r.unitsMatch);
  return { status: passed ? 'pass' : 'mismatch', singleResults, sequenceResults };
}
// Explicit binding architecture controls, without Java compatibility expectations.
export function runLosslessBindingArchitecture(api) {
  const uri = 'file:///lossless-binding-architecture.pkl';
  const value = source => {
    try { return { kind: 'value', units: stringUnits(api.evaluateLanguage(source, uri, 'main', 'result', false)) }; }
    catch (error) { return { kind: 'error', isError: error instanceof Error,
      name: error?.name ?? null, units: typeof error?.message === 'string' ? stringUnits(error.message) : null }; }
  };
  const longValue = value('result = "\\u{d800}".repeat(300000)\n');
  const splitPair = value('result = "a".repeat(1023) + "\\u{d800}\\u{dc00}"\n');
  const name = 'a'.repeat(140000);
  let longConstructor;
  try {
    const session = api.SourceSession.forExpressions(`${name} = 1\n${name} = 2\n`, uri, 'main', ['1'], 'compact', false, false, false);
    session.close(); session.collect(); if (typeof session.free === 'function') session.free();
    longConstructor = { kind: 'constructed' };
  } catch (error) {
    longConstructor = { kind: 'error', isError: error instanceof Error,
      name: error?.name ?? null, units: typeof error?.message === 'string' ? stringUnits(error.message) : null };
  }
  return { longValue, splitPair, longConstructor };
}
export function compareBindingArchitecture(actual) {
  return {
    longValue: actual.longValue.kind === 'value' && actual.longValue.units?.length === 300000
      && actual.longValue.units.every(unit => unit === 0xd800),
    splitPair: actual.splitPair.kind === 'value' && actual.splitPair.units?.length === 1025
      && actual.splitPair.units.slice(0, 1023).every(unit => unit === 97)
      && actual.splitPair.units[1023] === 0xd800 && actual.splitPair.units[1024] === 0xdc00,
    longConstructor: actual.longConstructor.kind === 'error' && actual.longConstructor.isError
      && actual.longConstructor.name === 'Error' && actual.longConstructor.units?.length > 140000,
  };
}
