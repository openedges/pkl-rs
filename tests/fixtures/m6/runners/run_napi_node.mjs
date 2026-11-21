// SPDX-License-Identifier: Apache-2.0
// Node.js execution driver importing & requiring clean-installed @pkl-rs/napi for M6 napi-node & cross-target-parity lanes.
import { createRequire } from "node:module";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { executeTargetCase } from "./case_executor.mjs";

const [planPathArg, resultPathArg] = process.argv.slice(2);
if (!resultPathArg) {
  throw new Error("usage: run_napi_node.mjs PLAN_JSON RESULT_JSON");
}

const plan = JSON.parse(await readFile(resolve(planPathArg), "utf8"));
const pkgDir = resolve(plan.installed_napi_pkg_dir);
const esmEntry = resolve(pkgDir, plan.pkg_esm_rel || "dist/index.mjs");
const cjsEntry = resolve(pkgDir, plan.pkg_cjs_rel || "dist/index.cjs");

const esmApi = await import(pathToFileURL(esmEntry).href);
const require = createRequire(import.meta.url);
const cjsApi = require(cjsEntry);

const results = [];
for (const spec of plan.cases) {
  const api = spec.module_format === "cjs" ? cjsApi : esmApi;
  const row = await executeTargetCase(api, spec, {});
  if (spec.check_dual_esm_cjs) {
    const requiredExports = [
      "PKL_VERSION", "capabilities", "createEvaluator", "ModuleSource", "PklError",
      "decodePklBinary", "PklDuration", "PklDataSize", "PklPair", "PklRegex",
      "PklObject", "PklClass", "PklTypeAlias", "PklIntSeq", "PklFunction",
      "format", "createRepl", "analyzeImportGraph", "Project",
    ];
    for (const name of requiredExports) {
      if (!(name in esmApi)) row.assertions.push(`missing_esm_export:${name}`);
      if (!(name in cjsApi)) row.assertions.push(`missing_cjs_export:${name}`);
    }
    const cjsEv = await cjsApi.createEvaluator();
    try {
      const cjsOut = await cjsEv.evaluateOutputText(cjsApi.ModuleSource.text("ok = true\n"));
      if (cjsOut !== "ok = true\n") row.assertions.push(`cjs_eval_mismatch:${cjsOut}`);
    } finally {
      await cjsEv.close();
    }
  }
  results.push(row);
}

await writeFile(
  resolve(resultPathArg),
  JSON.stringify(
    {
      schema_version: 1,
      runtime: "napi-node",
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
