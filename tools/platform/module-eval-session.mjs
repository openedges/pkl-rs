// Shared portable transport for the module frontend prefix. No CLI process or
// filesystem execution is implied by these byte-vector API observations.
export function evaluateModuleFixtures(api, fixtures, repetitions = 2) {
  if (typeof api?.evaluateModuleFrontend !== 'function') throw Error('evaluateModuleFrontend API is unavailable');
  const rows = [];
  for (const fixture of fixtures) {
    for (let repeat = 0; repeat < repetitions; repeat++) {
      const row = { id: fixture.id, repeat };
      try {
        const result = api.evaluateModuleFrontend(fixture.sourceText, fixture.uri, fixture.moduleName, fixture.arguments);
        if (!result || typeof result !== 'object' || typeof result.phase !== 'string'
            || typeof result.disposition !== 'string' || !Number.isInteger(result.exitCode)
            || !(result.stdout instanceof Uint8Array) || !(result.stderr instanceof Uint8Array)
            || (result.reason != null && typeof result.reason !== 'string'))
          throw Error('invalid structured module frontend result');
        rows.push({ ...row, result: { phase: result.phase, disposition: result.disposition,
          reason: result.reason ?? null, reasonPresent: Object.hasOwn(result, 'reason'),
          reasonKind: !Object.hasOwn(result, 'reason') ? 'absent' : result.reason === null ? 'null' : typeof result.reason,
          stdout: Array.from(result.stdout), stderr: Array.from(result.stderr), exitCode: result.exitCode }, acquisitionError: null });
      } catch (error) {
        rows.push({ ...row, result: null, acquisitionError: { message: error instanceof Error ? error.message : String(error),
          stack: error instanceof Error ? error.stack : null } });
      }
    }
  }
  return rows;
}
