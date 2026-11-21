// SPDX-License-Identifier: Apache-2.0
// Node.js execution driver importing clean-installed @pkl-rs/wasm for M6 wasm-node & cross-target-parity lanes.
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { executeTargetCase } from "./case_executor.mjs";

const [planPathArg, resultPathArg] = process.argv.slice(2);
if (!resultPathArg) {
  throw new Error("usage: run_wasm_node.mjs PLAN_JSON RESULT_JSON");
}

const plan = JSON.parse(await readFile(resolve(planPathArg), "utf8"));
const entryPath = resolve(plan.installed_wasm_pkg_dir, plan.pkg_entry_rel || "dist/index.mjs");
const api = await import(pathToFileURL(entryPath).href);
if (typeof api.init === "function") {
  await api.init();
}

const m4HttpPort = plan.m4_http_port || 0;
const rewriteToLocalHttp = (url) => {
  const parsed = new URL(url);
  if (parsed.hostname === "localhost" && parsed.port === "0" && m4HttpPort) {
    return `http://127.0.0.1:${m4HttpPort}${parsed.pathname}${parsed.search}`;
  }
  return url;
};

const ctx = {
  async httpFetch(url, init) {
    const resp = await fetch(rewriteToLocalHttp(url), {
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
    const resp = await fetch(rewriteToLocalHttp(url), { signal: init?.signal });
    if (!resp.ok) throw new Error(`HTTP ${resp.status} for ${url}`);
    return new Uint8Array(await resp.arrayBuffer());
  },
};

const results = [];
for (const spec of plan.cases) {
  results.push(await executeTargetCase(api, spec, ctx));
}

await writeFile(
  resolve(resultPathArg),
  JSON.stringify(
    {
      schema_version: 1,
      runtime: "wasm-node",
      node: process.version,
      platform: process.platform,
      arch: process.arch,
      results,
    },
    null,
    2
  ) + "\n",
  "utf8"
);
