// Shared portable transport for explicit module frontend/output operations. No CLI process or
// filesystem execution is implied by these byte-vector API observations.
export function evaluateModuleFixtures(api, fixtures, repetitions = 2) {
  for (const fixture of fixtures) {
    const operation = fixture.operation ?? 'frontend';
    if (!['frontend', 'output-bytes'].includes(operation)) throw Error('unknown module operation');
    const method = operation === 'output-bytes' ? 'evaluateModuleOutput' : 'evaluateModuleFrontend';
    if (typeof api?.[method] !== 'function') throw Error(method + ' API is unavailable');
  }
  const rows = [];
  for (const fixture of fixtures) {
    for (let repeat = 0; repeat < repetitions; repeat++) {
      const operation = fixture.operation ?? 'frontend';
      const row = { id: fixture.id, repeat, operation };
      try {
        const method = operation === 'output-bytes' ? 'evaluateModuleOutput' : 'evaluateModuleFrontend';
        const result = api[method](fixture.sourceText, fixture.uri, fixture.moduleName, fixture.arguments);
        const validDisposition = operation === 'frontend' ? typeof result?.disposition === 'string'
          : result?.exitCode === 0 ? result.disposition == null && result.phase === 'module-output'
          : result?.disposition === ({ 1: 'language-error', 2: 'unsupported', 3: 'limit-exceeded', 4: 'internal-failure' })[result?.exitCode]
            && typeof result?.disposition === 'string';
        if (!result || typeof result !== 'object' || typeof result.phase !== 'string'
            || !validDisposition || !Number.isInteger(result.exitCode)
            || !(result.stdout instanceof Uint8Array) || !(result.stderr instanceof Uint8Array)
            || (result.reason != null && typeof result.reason !== 'string'))
          throw Error('invalid structured module ' + operation + ' result');
        rows.push({ ...row, result: { phase: result.phase, disposition: result.disposition ?? null,
          dispositionPresent: Object.hasOwn(result, 'disposition'),
          dispositionKind: !Object.hasOwn(result, 'disposition') ? 'absent' : result.disposition === null ? 'null' : typeof result.disposition,
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
