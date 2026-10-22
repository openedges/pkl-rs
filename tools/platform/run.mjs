import { createRequire } from 'node:module';
import { createServer } from 'node:http';
import { readFile, writeFile, mkdir } from 'node:fs/promises';
import { resolve, extname, sep } from 'node:path';
import { createHash } from 'node:crypto';
import { chromium } from 'playwright';
import { scenarios } from './scenarios.mjs';

const [outputArg, wasmArg, napiArg, casesArg] = process.argv.slice(2);
if (!casesArg) throw new Error('usage: run.mjs OUTPUT WASM_DIR NAPI_BINARY CASES_JSON');
const require = createRequire(import.meta.url);
const packageManifest = JSON.parse(await readFile(new URL('./package.json', import.meta.url)));
const packageLock = JSON.parse(await readFile(new URL('./package-lock.json', import.meta.url)));
const playwrightVersion = require('playwright/package.json').version;
const playwrightCoreVersion = require('playwright-core/package.json').version;
const pin = packageManifest.devDependencies.playwright;
if (!/^\d+\.\d+\.\d+$/.test(pin) || playwrightVersion !== pin || playwrightCoreVersion !== pin
    || packageLock.packages[''].devDependencies.playwright !== pin
    || packageLock.packages['node_modules/playwright'].version !== pin
    || packageLock.packages['node_modules/playwright-core'].version !== pin) {
  throw new Error(`installed Playwright and playwright-core must match manifest/lock pin ${pin}`);
}
const output = resolve(outputArg), wasm = resolve(wasmArg);
await mkdir(output, { recursive: false });
const manifestBytes = await readFile(resolve(casesArg));
const cases = JSON.parse(manifestBytes).cases;
if (!cases.length || new Set(cases.map(c => c.id)).size !== cases.length) throw new Error('empty or duplicate corpus');
const encode = value => Buffer.from(value).toString('hex');
const native = createRequire(import.meta.url)(resolve(napiArg));
const nodeChecks = await scenarios(native);
const nodeOutputs = cases.map(c => encode(native.primitive(c.family, c.input)));
await writeFile(resolve(output, 'node.stdout.bin'), nodeOutputs.join('\n') + '\n');
await writeFile(resolve(output, 'node-checks.json'), JSON.stringify(nodeChecks, null, 2) + '\n');

const browserErrors = [];
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, 'http://127.0.0.1');
    if (url.pathname === '/') { response.writeHead(200, { 'Content-Type': 'text/html' }); response.end('<!doctype html><title>M1 Wasm verification</title>'); return; }
    let path;
    if (['/scenarios.mjs', '/host-adapter.mjs', '/class-publication-cases.mjs', '/type-metadata-cases.mjs', '/type-resolution-cases.mjs', '/recursive-types-cases.mjs', '/constraint-slots-cases.mjs', '/local-resolution-cases.mjs', '/generic-types-cases.mjs', '/generic-metadata-cases.mjs', '/generic-defaults-cases.mjs', '/generic-constructors-cases.mjs', '/strict-map-cases.mjs', '/strict-set-cases.mjs', '/collection-type-cases.mjs', '/frontend-depth-cases.mjs', '/string-frontend-cases.mjs'].includes(url.pathname)) path = new URL('.' + url.pathname, import.meta.url);
    else {
      path = resolve(wasm, '.' + decodeURIComponent(url.pathname));
      if (!path.startsWith(wasm + sep)) throw new Error('invalid path');
    }
    const bytes = await readFile(path);
    const extension = extname(String(path));
    response.writeHead(200, { 'Content-Type': extension === '.wasm' ? 'application/wasm' : 'text/javascript' });
    response.end(bytes);
  } catch { response.writeHead(404); response.end(); }
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
try {
  browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  page.on('pageerror', e => browserErrors.push(e.stack));
  await page.goto(`http://127.0.0.1:${server.address().port}`);
  const result = await page.evaluate(async cases => {
    const api = await import('/pkl_wasm.js');
    await api.default();
    const { scenarios } = await import('/scenarios.mjs');
    const checks = await scenarios(api);
    const encode = value => [...new TextEncoder().encode(value)].map(b => b.toString(16).padStart(2, '0')).join('');
    return { checks, outputs: cases.map(c => encode(api.primitive(c.family, c.input))) };
  }, cases);
  await writeFile(resolve(output, 'browser.stdout.bin'), result.outputs.join('\n') + '\n');
  await writeFile(resolve(output, 'browser-checks.json'), JSON.stringify(result.checks, null, 2) + '\n');
  const report = { schema: 1, node: process.version, nodeVersions: process.versions, browser: browser.version(), browserExecutable: chromium.executablePath(), playwright: playwrightVersion, playwrightCore: playwrightCoreVersion, playwrightPin: pin, fixtureSha256: createHash('sha256').update(manifestBytes).digest('hex'), total: cases.length, nodeChecks, browserChecks: result.checks, browserErrors };
  await writeFile(resolve(output, 'execution.json'), JSON.stringify(report, null, 2) + '\n');
  if (browserErrors.length) throw new Error('browser reported uncaught errors');
  console.log(JSON.stringify({ total: cases.length, nodeChecks: nodeChecks.length, browserChecks: result.checks.length, browser: browser.version() }));
} finally {
  if (browser) await browser.close();
  await new Promise(resolve => server.close(resolve));
}
