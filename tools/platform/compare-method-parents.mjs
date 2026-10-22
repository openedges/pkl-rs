// Preserve the Stage 34 command while sharing the complete portable comparison.
import { fileURLToPath } from 'node:url';
import { compareLanguageCases } from './compare-language-cases.mjs';

await compareLanguageCases([
  ...process.argv.slice(2),
  fileURLToPath(new URL('./method-parent-cases.mjs', import.meta.url)),
]);
