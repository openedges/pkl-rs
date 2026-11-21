#!/usr/bin/env node
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, mkdirSync, rmSync, writeFileSync, existsSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const pkgDir = resolve(here, "..");
const stage = mkdtempSync(join(tmpdir(), "pkl-napi-pack-install-"));
const npmBin = join(dirname(process.execPath), process.platform === "win32" ? "npm.cmd" : "npm");
const npmCache = join(stage, "npm-cache");

try {
  const packDir = join(stage, "tarballs");
  const consumerDir = join(stage, "consumer");
  mkdirSync(packDir, { recursive: true });
  mkdirSync(consumerDir, { recursive: true });
  mkdirSync(npmCache, { recursive: true });

  const npmEnv = { ...process.env, npm_config_cache: npmCache };

  const packStdout = execFileSync(npmBin, ["pack", "--pack-destination", packDir], {
    cwd: pkgDir,
    env: npmEnv,
    encoding: "utf8",
  }).trim();
  const tarballName = packStdout.split(/\r?\n/).pop().trim();
  const tarballPath = join(packDir, tarballName);
  assert.ok(existsSync(tarballPath), `Tarball not created at ${tarballPath}`);

  writeFileSync(
    join(consumerDir, "package.json"),
    JSON.stringify({ name: "pkl-napi-consumer-check", version: "1.0.0", private: true }, null, 2),
  );

  execFileSync(
    npmBin,
    ["install", "--offline", "--no-audit", "--no-fund", tarballPath],
    { cwd: consumerDir, env: npmEnv, stdio: "inherit" },
  );

  // Verify TypeScript declarations & native addon are packed in installed package
  const installedRoot = join(consumerDir, "node_modules", "@pkl-rs", "napi");
  assert.ok(existsSync(join(installedRoot, "dist", "index.d.ts")), "dist/index.d.ts missing");
  assert.ok(existsSync(join(installedRoot, "dist", "index.mjs")), "dist/index.mjs missing");
  assert.ok(existsSync(join(installedRoot, "dist", "index.cjs")), "dist/index.cjs missing");
  assert.ok(
    existsSync(join(installedRoot, "native", `pkl_napi.${process.platform}-${process.arch}.node`)),
    "native platform .node addon missing",
  );

  // Clean ESM consumer probe
  writeFileSync(
    join(consumerDir, "check.mjs"),
    `import assert from "node:assert/strict";
import {
  PKL_VERSION,
  capabilities,
  createEvaluator,
  ModuleSource,
  PklError,
  format,
  roundtripInteger,
  roundtripBytes,
} from "@pkl-rs/napi";

assert.equal(PKL_VERSION, "0.32.0");
assert.equal(capabilities().runtime, "napi-node");
assert.equal(capabilities().nativeFs, true);
assert.equal(format("a=1\\n"), "a = 1\\n");
assert.equal(roundtripInteger(9223372036854775807n), 9223372036854775807n);
const b = new Uint8Array([4, 5, 6]);
const b2 = roundtripBytes(b);
b[0] = 99;
assert.equal(b2[0], 4);
const ev = await createEvaluator();
try {
  const out = await ev.evaluateOutputText(ModuleSource.text('msg = "esm-ok"\\nn = 1 + 2'));
  assert.equal(out, 'msg = "esm-ok"\\nn = 3\\n');
} finally {
  await ev.close();
}
console.log("ESM_PACK_INSTALL_OK");
`,
  );

  // Clean CJS consumer probe
  writeFileSync(
    join(consumerDir, "check.cjs"),
    `const assert = require("node:assert/strict");
const {
  PKL_VERSION,
  capabilities,
  createEvaluator,
  createEvaluatorSync,
  ModuleSource,
  PklError,
  format,
} = require("@pkl-rs/napi");

async function main() {
  assert.equal(PKL_VERSION, "0.32.0");
  assert.equal(capabilities().runtime, "napi-node");
  assert.equal(format("x=42\\n"), "x = 42\\n");
  const syncEv = createEvaluatorSync();
  try {
    assert.equal(syncEv.evaluateExpressionSync(ModuleSource.text("x = 40 + 2"), "x"), "42");
  } finally {
    await syncEv.close();
  }
  const ev = await createEvaluator();
  try {
    const out = await ev.evaluateOutputText(ModuleSource.text('msg = "cjs-ok"'));
    assert.equal(out, 'msg = "cjs-ok"\\n');
  } finally {
    await ev.close();
  }
  console.log("CJS_PACK_INSTALL_OK");
}
main().catch((err) => { console.error(err); process.exit(1); });
`,
  );

  const esmOut = execFileSync(process.execPath, [join(consumerDir, "check.mjs")], {
    cwd: consumerDir,
    encoding: "utf8",
  }).trim();
  assert.equal(esmOut, "ESM_PACK_INSTALL_OK");

  const cjsOut = execFileSync(process.execPath, [join(consumerDir, "check.cjs")], {
    cwd: consumerDir,
    encoding: "utf8",
  }).trim();
  assert.equal(cjsOut, "CJS_PACK_INSTALL_OK");

  console.log(JSON.stringify({ packInstall: "PASS", tarball: tarballName, esm: esmOut, cjs: cjsOut }));
} finally {
  rmSync(stage, { recursive: true, force: true });
}
