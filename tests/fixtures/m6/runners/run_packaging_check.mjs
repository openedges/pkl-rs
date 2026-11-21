// SPDX-License-Identifier: Apache-2.0
// Clean-directory install & live execution check for packed @pkl-rs/wasm and @pkl-rs/napi tarballs.
// Note: when executed for wasm-node or napi-esm-cjs, this script is staged inside the clean
// temp install root (`install_base/<role>/run_packaging_check.mjs`) so bare ESM `import()` and
// CJS `require()` genuinely resolve through `install_base/<role>/node_modules/@pkl-rs/*`.
import { createRequire } from "node:module";
import { access, readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";

const [mode, pkgDirArg, samplePklArg, resultPathArg] = process.argv.slice(2);
if (!resultPathArg) {
  throw new Error("usage: run_packaging_check.mjs <wasm-node|napi-esm-cjs|matrix> PKG_DIR SAMPLE_PKL RESULT_JSON");
}

async function fileExists(path) {
  try {
    await access(path);
    return true;
  } catch {
    return false;
  }
}

const pkgDir = resolve(pkgDirArg);
const sampleText = await readFile(resolve(samplePklArg), "utf8");
const assertions = [];
let stdout = "";

if (mode === "wasm-node") {
  const pkgJson = JSON.parse(await readFile(resolve(pkgDir, "package.json"), "utf8"));
  if (pkgJson.name !== "@pkl-rs/wasm") assertions.push(`unexpected_name:${pkgJson.name}`);
  if (pkgJson.version !== "0.32.0") assertions.push(`unexpected_version:${pkgJson.version}`);
  if (!pkgJson.exports || !pkgJson.exports["."]) assertions.push("missing_exports");
  const typesRel = pkgJson.exports?.["."]?.types || pkgJson.types;
  if (!typesRel || !(await fileExists(resolve(pkgDir, typesRel)))) {
    assertions.push(`missing_types_file:${typesRel || "none"}`);
  }
  const mod = await import("@pkl-rs/wasm");
  if (typeof mod.init === "function") await mod.init();
  const ev = await mod.createEvaluator();
  try {
    stdout = await ev.evaluateOutputText(mod.ModuleSource.text(sampleText));
  } finally {
    await ev.close();
  }
} else if (mode === "napi-esm-cjs") {
  const pkgJson = JSON.parse(await readFile(resolve(pkgDir, "package.json"), "utf8"));
  if (pkgJson.name !== "@pkl-rs/napi") assertions.push(`unexpected_name:${pkgJson.name}`);
  if (pkgJson.version !== "0.32.0") assertions.push(`unexpected_version:${pkgJson.version}`);
  if (!pkgJson.exports?.["."]?.import || !pkgJson.exports?.["."]?.require) {
    assertions.push("missing_dual_esm_cjs_exports");
  }
  const typesRel = pkgJson.exports?.["."]?.types || pkgJson.types;
  if (!typesRel || !(await fileExists(resolve(pkgDir, typesRel)))) {
    assertions.push(`missing_types_file:${typesRel || "none"}`);
  }
  const esmMod = await import("@pkl-rs/napi");
  const require = createRequire(import.meta.url);
  const cjsMod = require("@pkl-rs/napi");

  const evEsm = await esmMod.createEvaluator();
  let esmOut = "";
  try {
    esmOut = await evEsm.evaluateOutputText(esmMod.ModuleSource.text(sampleText));
  } finally {
    await evEsm.close();
  }
  const evCjs = await cjsMod.createEvaluator();
  let cjsOut = "";
  try {
    cjsOut = await evCjs.evaluateOutputText(cjsMod.ModuleSource.text(sampleText));
  } finally {
    await evCjs.close();
  }
  if (esmOut !== cjsOut) assertions.push("esm_cjs_output_mismatch");
  stdout = esmOut;
} else if (mode === "matrix") {
  const wasmDir = resolve(pkgDir, "packages/pkl-wasm");
  const napiDir = resolve(pkgDir, "packages/pkl-napi");
  const wasmPkg = JSON.parse(await readFile(resolve(wasmDir, "package.json"), "utf8"));
  const napiPkg = JSON.parse(await readFile(resolve(napiDir, "package.json"), "utf8"));
  if (wasmPkg.name !== "@pkl-rs/wasm" || wasmPkg.version !== "0.32.0") assertions.push("wasm_pkg_metadata_invalid");
  if (napiPkg.name !== "@pkl-rs/napi" || napiPkg.version !== "0.32.0") assertions.push("napi_pkg_metadata_invalid");
  const wasmTypes = wasmPkg.exports?.["."]?.types || wasmPkg.types;
  const napiTypes = napiPkg.exports?.["."]?.types || napiPkg.types;
  if (!wasmTypes || !(await fileExists(resolve(wasmDir, wasmTypes)))) assertions.push("wasm_missing_types_file");
  if (!napiTypes || !(await fileExists(resolve(napiDir, napiTypes)))) assertions.push("napi_missing_types_file");
  const hasPlatformMatrix =
    (Array.isArray(napiPkg.os) && napiPkg.os.length > 0 && Array.isArray(napiPkg.cpu) && napiPkg.cpu.length > 0) ||
    (napiPkg.pklPlatformSupport && typeof napiPkg.pklPlatformSupport === "object");
  if (!hasPlatformMatrix) assertions.push("napi_missing_platform_support_metadata");
  if (!Array.isArray(wasmPkg.files) || wasmPkg.files.length === 0) assertions.push("wasm_missing_files_whitelist");
  if (!Array.isArray(napiPkg.files) || napiPkg.files.length === 0) assertions.push("napi_missing_files_whitelist");
  stdout = "0.32.0\n";
} else {
  throw new Error(`Unknown packaging check mode: ${mode}`);
}

await writeFile(
  resolve(resultPathArg),
  JSON.stringify(
    {
      mode,
      stdout_b64: Buffer.from(stdout, "utf8").toString("base64"),
      assertions,
    },
    null,
    2
  ) + "\n",
  "utf8"
);
