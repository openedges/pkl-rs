// SPDX-License-Identifier: Apache-2.0
// Real Chromium execution driver for M6 wasm-browser & cross-target-parity lanes using pinned Playwright 1.64.1.
import { createRequire } from "node:module";
import { createServer, request as httpRequest } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { createHash } from "node:crypto";

const [planPathArg, resultPathArg] = process.argv.slice(2);
if (!resultPathArg) {
  throw new Error("usage: run_wasm_browser.mjs PLAN_JSON RESULT_JSON");
}

const plan = JSON.parse(await readFile(resolve(planPathArg), "utf8"));
const repoRoot = resolve(plan.repo_root);
const platformDir = resolve(repoRoot, "tools/platform");
const requireFromPlatform = createRequire(resolve(platformDir, "package.json"));

// Enforce exact pinned Playwright version matching tools/platform/package.json & package-lock.json
const pkgManifest = JSON.parse(await readFile(resolve(platformDir, "package.json"), "utf8"));
const pkgLock = JSON.parse(await readFile(resolve(platformDir, "package-lock.json"), "utf8"));
const pin = pkgManifest.devDependencies.playwright;
const playwrightVersion = requireFromPlatform("playwright/package.json").version;
const playwrightCoreVersion = requireFromPlatform("playwright-core/package.json").version;
if (
  !/^\d+\.\d+\.\d+$/.test(pin) ||
  playwrightVersion !== pin ||
  playwrightCoreVersion !== pin ||
  pkgLock.packages[""].devDependencies.playwright !== pin ||
  pkgLock.packages["node_modules/playwright"].version !== pin ||
  pkgLock.packages["node_modules/playwright-core"].version !== pin
) {
  throw new Error(`Playwright pin mismatch: expected ${pin}, got ${playwrightVersion}/${playwrightCoreVersion}`);
}

const { chromium } = requireFromPlatform("playwright");
const pkgDir = resolve(plan.installed_wasm_pkg_dir);
const runnersDir = resolve(repoRoot, "tests/fixtures/m6/runners");
const m4HttpPort = plan.m4_http_port || 0;

function proxyToM4(targetPath, reqHeaders, response) {
  const req = httpRequest(
    {
      hostname: "127.0.0.1",
      port: m4HttpPort,
      path: targetPath,
      method: "GET",
      headers: reqHeaders,
    },
    (upstream) => {
      const headers = {
        "Content-Type": upstream.headers["content-type"] || "application/octet-stream",
        "Access-Control-Allow-Origin": "*",
      };
      if (upstream.headers.location) headers.Location = upstream.headers.location;
      response.writeHead(upstream.statusCode || 502, headers);
      upstream.pipe(response);
    }
  );
  req.on("error", () => {
    response.writeHead(502);
    response.end();
  });
  req.end();
}

const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json; charset=utf-8",
};

const browserErrors = [];
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://127.0.0.1");
    if (url.pathname === "/") {
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      response.end("<!doctype html><title>M6 Wasm Browser Verification</title>");
      return;
    }
    if (url.pathname.startsWith("/__m4_http/")) {
      const rel = "/" + url.pathname.slice("/__m4_http/".length);
      proxyToM4(rel, {}, response);
      return;
    }
    let filePath = null;
    if (url.pathname === "/__runners/canonical_value.mjs" || url.pathname === "/__runners/case_executor.mjs") {
      filePath = resolve(runnersDir, url.pathname.slice("/__runners/".length));
    } else if (url.pathname.startsWith("/pkg/")) {
      const rel = decodeURIComponent(url.pathname.slice("/pkg/".length));
      const candidate = resolve(pkgDir, rel);
      if (candidate.startsWith(pkgDir + sep)) filePath = candidate;
    }
    if (!filePath) {
      response.writeHead(404);
      response.end();
      return;
    }
    const bytes = await readFile(filePath);
    const ct = MIME[extname(filePath)] || "application/octet-stream";
    response.writeHead(200, { "Content-Type": ct });
    response.end(bytes);
  } catch {
    response.writeHead(404);
    response.end();
  }
});

await new Promise((res) => server.listen(0, "127.0.0.1", res));
const uiPort = server.address().port;

let browser;
try {
  const browserExecutable = chromium.executablePath();
  const browserExecutableSha256 = createHash("sha256").update(await readFile(browserExecutable)).digest("hex");
  browser = await chromium.launch({ headless: true, executablePath: browserExecutable });
  const page = await browser.newPage();
  page.on("pageerror", (err) => browserErrors.push(err.stack || String(err)));
  await page.goto(`http://127.0.0.1:${uiPort}/`);

  const results = await page.evaluate(
    async ({ cases, pkgEntryRel }) => {
      const api = await import(`/pkg/${pkgEntryRel}`);
      if (typeof api.init === "function") {
        await api.init();
      }
      const { executeTargetCase } = await import("/__runners/case_executor.mjs");

      const mapUrlToProxy = (url) => {
        const parsed = new URL(url);
        return `/__m4_http${parsed.pathname}${parsed.search}`;
      };
      const ctx = {
        async httpFetch(url, init) {
          const resp = await fetch(mapUrlToProxy(url), {
            method: init?.method || "GET",
            headers: init?.headers,
            signal: init?.signal,
          });
          const buf = await resp.arrayBuffer();
          const headers = {};
          resp.headers.forEach((v, k) => {
            headers[k] = v;
          });
          return { status: resp.status, headers, bytes: new Uint8Array(buf) };
        },
        async packageTransport(url, init) {
          const resp = await fetch(mapUrlToProxy(url), { signal: init?.signal });
          if (!resp.ok) throw new Error(`HTTP ${resp.status} for ${url}`);
          return new Uint8Array(await resp.arrayBuffer());
        },
      };

      const out = [];
      for (const spec of cases) {
        out.push(await executeTargetCase(api, spec, ctx));
      }
      return out;
    },
    { cases: plan.cases, pkgEntryRel: plan.pkg_entry_rel || "dist/index.mjs" }
  );

  const output = {
    schema_version: 1,
    runtime: "wasm-browser",
    node: process.version,
    browser: browser.version(),
    browserExecutable,
    browserExecutableSha256,
    playwright: playwrightVersion,
    playwrightCore: playwrightCoreVersion,
    playwrightPin: pin,
    browserErrors,
    results,
  };
  await writeFile(resolve(resultPathArg), JSON.stringify(output, null, 2) + "\n", "utf8");
  if (browserErrors.length > 0) {
    throw new Error(`Chromium reported uncaught page errors: ${browserErrors.join("; ")}`);
  }
} finally {
  if (browser) await browser.close();
  await new Promise((res) => server.close(res));
}
