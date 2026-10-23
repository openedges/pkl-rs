// Historical fixtures captured Java String.getBytes(UTF_8), not String units.
// Preserve original bytes and the actual JS units as separate observations.
export function legacyJavaUtf8Observation(actual, expected) {
  if (typeof actual !== 'string' || typeof expected !== 'string') throw Error('expected string channels');
  const units = Array.from({length: actual.length}, (_, i) => actual.charCodeAt(i));
  let javaText = '';
  for (let i = 0; i < units.length; i++) {
    const unit = units[i], next = units[i + 1];
    if (unit >= 0xd800 && unit <= 0xdbff && next >= 0xdc00 && next <= 0xdfff) {
      javaText += String.fromCharCode(unit, next); i++;
    } else javaText += unit >= 0xd800 && unit <= 0xdfff ? '?' : String.fromCharCode(unit);
  }
  const actualJavaUtf8 = Array.from(new TextEncoder().encode(javaText));
  const originalJavaUtf8 = Array.from(new TextEncoder().encode(expected));
  return {comparisonBoundary: 'historical Java UTF8 only; no expected UTF16 units',
    actualUnits: units, actualJavaUtf8, originalJavaUtf8,
    matches: actualJavaUtf8.length === originalJavaUtf8.length
      && actualJavaUtf8.every((byte, i) => byte === originalJavaUtf8[i])};
}
