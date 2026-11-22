#!/usr/bin/env python3
# SPDX-License-Identifier: Apache-2.0
"""M7 JS/Wasm Cross-Browser Verification, Deep Performance/RSS Profiling & Analysis Tool.

Two modes:
  1. `--smoke-browsers chromium,firefox,webkit`:
     Runs end-to-end functional verification of `@pkl-rs/wasm` inside real headless
     browsers via pinned Playwright (`tools/platform/package.json`) without relaxing any
     raw compatibility contract.
  2. `--profile quick|full --browsers chromium,firefox,webkit`:
     Measures head-to-head execution time AND decomposed OS/runtime memory across:
       - Empty Node.js baseline (`os.wait4` peak RSS)
       - `@pkl-rs/napi` (`eval-text`, `typed-conversion`)
       - `@pkl-rs/wasm` on Node.js (`eval-text`, `typed-conversion`, Wasm linear memory, `.wasm` size)
       - `@pkl-rs/wasm` on real headless browsers (`chromium`, `firefox`, `webkit`, plus optional
         `chromium-no-gpu`) with per-process RSS breakdown (`browser_main`, `renderer`, `gpu`,
         `utility_network`, `other`), in-page Wasm linear memory, and JS heap
       - Live pinned Java 0.32.0 (`Temurin 21.0.13.1+2-LTS`) reference baselines
     Outputs sealed `report.json` AND a human-readable `analysis.md` report for evidence-based
     review of JS/Wasm performance and memory targets (never mutating strict M7 contracts).
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
from typing import Any

import m7_benchmark

ROOT = Path(__file__).resolve().parents[1]
VALID_BROWSERS = ("chromium", "firefox", "webkit")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def write_json(path: Path, data: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def fmt_ns(ns: float | None) -> str:
    if ns is None:
        return "—"
    if ns < 1_000:
        return f"{ns:.0f} ns"
    if ns < 1_000_000:
        return f"{ns / 1_000:.2f} us"
    if ns < 1_000_000_000:
        return f"{ns / 1_000_000:.2f} ms"
    return f"{ns / 1_000_000_000:.3f} s"


def fmt_mib(byte_count: float | int | None) -> str:
    if byte_count is None:
        return "—"
    return f"{float(byte_count) / (1024 * 1024):.1f} MiB"


def fmt_ratio(num: float | None, den: float | None) -> str:
    if num is None or den is None or den <= 0:
        return "—"
    return f"{num / den:.3f}x"


SMOKE_DRIVER_JS = r"""
import { createRequire } from "node:module";
import { createServer } from "node:http";
import { readFile, writeFile } from "node:fs/promises";
import { resolve, extname, sep } from "node:path";
import { createHash } from "node:crypto";

const [planPathArg, resultPathArg] = process.argv.slice(2);
const plan = JSON.parse(await readFile(resolve(planPathArg), "utf8"));
const repoRoot = resolve(plan.repo_root);
const platformDir = resolve(repoRoot, "tools/platform");
const requireFromPlatform = createRequire(resolve(platformDir, "package.json"));

const pkgManifest = JSON.parse(await readFile(resolve(platformDir, "package.json"), "utf8"));
const pkgLock = JSON.parse(await readFile(resolve(platformDir, "package-lock.json"), "utf8"));
const pin = pkgManifest.devDependencies.playwright;
const playwrightVersion = requireFromPlatform("playwright/package.json").version;
const playwrightCoreVersion = requireFromPlatform("playwright-core/package.json").version;
if (
  !/^\d+\.\d+\.\d+$/.test(pin) ||
  playwrightVersion !== pin ||
  playwrightCoreVersion !== pin ||
  pkgLock.packages[""].devDependencies.playwright !== pin
) {
  throw new Error(`Playwright pin mismatch: expected ${pin}, got ${playwrightVersion}/${playwrightCoreVersion}`);
}

const engines = requireFromPlatform("playwright");
const engineName = plan.browser_engine;
const browserType = engines[engineName];
if (!browserType) throw new Error(`Unknown browser engine: ${engineName}`);

const pkgDir = resolve(plan.wasm_pkg_dir);
const MIME = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".wasm": "application/wasm",
  ".json": "application/json; charset=utf-8",
};

const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://127.0.0.1");
    if (url.pathname === "/") {
      response.writeHead(200, { "Content-Type": "text/html; charset=utf-8" });
      response.end("<!doctype html><title>Pkl Wasm Cross-Browser Smoke</title>");
      return;
    }
    if (url.pathname.startsWith("/pkg/")) {
      const rel = decodeURIComponent(url.pathname.slice("/pkg/".length));
      const candidate = resolve(pkgDir, rel);
      if (candidate.startsWith(pkgDir + sep)) {
        const bytes = await readFile(candidate);
        response.writeHead(200, { "Content-Type": MIME[extname(candidate)] || "application/octet-stream" });
        response.end(bytes);
        return;
      }
    }
    response.writeHead(404);
    response.end();
  } catch {
    response.writeHead(404);
    response.end();
  }
});

await new Promise((res) => server.listen(0, "127.0.0.1", res));
const port = server.address().port;

const browserErrors = [];
let browser;
try {
  const browserExecutable = browserType.executablePath();
  const browserExecutableSha256 = createHash("sha256").update(await readFile(browserExecutable)).digest("hex");
  browser = await browserType.launch({ headless: true, executablePath: browserExecutable });
  const page = await browser.newPage();
  page.on("pageerror", (err) => browserErrors.push(err.stack || String(err)));
  await page.goto(`http://127.0.0.1:${port}/`);

  const summary = await page.evaluate(async () => {
    const t0 = performance.now();
    const api = await import("/pkg/dist/index.mjs");
    const rt = await import("/pkg/dist/runtime.mjs");
    await api.init();
    const initMs = performance.now() - t0;

    const checks = [];
    const runCheck = async (name, fn) => {
      await fn();
      checks.push({ name, status: "pass" });
    };

    const assertEq = (a, b, msg) => {
      if (!Object.is(a, b)) throw new Error(`${msg}: expected ${String(b)}, got ${String(a)}`);
    };

    await runCheck("version-and-capabilities", async () => {
      assertEq(api.PKL_VERSION, "0.32.0", "PKL_VERSION");
      const caps = api.capabilities();
      assertEq(caps.runtime, "wasm-browser", "capabilities.runtime");
      assertEq(caps.vfs, true, "capabilities.vfs");
      assertEq(caps.nativeFs, false, "capabilities.nativeFs");
      assertEq(caps.bigint64, true, "capabilities.bigint64");
    });

    await runCheck("all-9-renderers-and-binary-roundtrip", async () => {
      const src = api.ModuleSource.text('a = 42\nb = "hello-한글-😀"\n');
      for (const fmt of ["pcf", "json", "yaml", "xml", "properties", "plist", "jsonnet", "textproto"]) {
        const ev = await api.createEvaluator({ outputFormat: fmt });
        try {
          const text = await ev.evaluateOutputText(src);
          if (!text || !text.includes("42")) throw new Error(`renderer ${fmt} missing value`);
        } finally {
          ev.close();
        }
      }
      const bev = await api.createEvaluator({ outputFormat: "pkl-binary" });
      try {
        const bytes = await bev.evaluateOutputBytes(src);
        if (!(bytes instanceof Uint8Array) || bytes.length === 0) throw new Error("empty pkl-binary");
        const decoded = api.decodePklBinary(bytes);
        assertEq(decoded.properties.a, 42n, "decoded.a");
        assertEq(decoded.properties.b, "hello-한글-😀", "decoded.b");
      } finally {
        bev.close();
      }
    });

    await runCheck("lossless-bigint-bytes-utf16", async () => {
      const MIN_I64 = -(1n << 63n);
      const MAX_I64 = (1n << 63n) - 1n;
      assertEq(api.roundtripInteger(MIN_I64), MIN_I64, "MIN_I64");
      assertEq(api.roundtripInteger(MAX_I64), MAX_I64, "MAX_I64");
      const tricky = "a\uD800b🚀c\uDC00d";
      assertEq(api.roundtripString(tricky), tricky, "UTF-16 unpaired surrogates");
      const orig = new Uint8Array([0, 127, 255]);
      const rtBytes = api.roundtripBytes(orig);
      orig[0] = 99;
      assertEq(rtBytes[0], 0, "Uint8Array owned copy");
      assertEq(rtBytes[2], 255, "Uint8Array last byte");
    });

    await runCheck("vfs-security-schema-repl-format-test", async () => {
      assertEq(api.format("x=1+2\n"), "x = 1 + 2\n", "format");
      const vfs = new api.VirtualFileSystem(
        {
          "/app/lib.pkl": "v = 21\n",
          "/app/main.pkl": 'import "lib.pkl"\nout = lib.v * 2\n',
          "/outside.pkl": "secret = 1\n",
        },
        { rootDir: "/app" }
      );
      const ev = await api.createEvaluator({ vfs });
      try {
        assertEq(await ev.evaluateExpression(api.ModuleSource.path("/app/main.pkl"), "out"), "42", "vfs eval");
        let denied = false;
        try {
          await ev.evaluateOutputText(api.ModuleSource.text('import "../outside.pkl"\nx = outside.secret\n', "file:///app/e.pkl"));
        } catch (err) {
          denied = err instanceof api.PklError && err.kind === "security";
        }
        assertEq(denied, true, "rootDir escape blocked");
        const schema = await ev.evaluateSchema(api.ModuleSource.text("class C { n: Int }\nc: C\n"));
        assertEq(Boolean(schema.classes.C), true, "schema class");
      } finally {
        ev.close();
      }
      const repl = await api.createRepl({ vfs, workingDir: "/app" });
      try {
        await repl.eval("x = 10");
        const r = await repl.eval("x + 5");
        assertEq(r[0].result, "15", "repl eval");
      } finally {
        repl.close();
      }
    });

    await runCheck("capability-unavailable-and-abort", async () => {
      const bare = await api.createEvaluator();
      try {
        let capCode = null;
        try {
          await bare.evaluateOutputText(api.ModuleSource.uri("file:///missing.pkl"));
        } catch (err) {
          capCode = err?.code === "capability-unavailable" ? err?.capability : null;
        }
        assertEq(capCode, "nativeFs", "nativeFs capability-unavailable");
      } finally {
        bare.close();
      }
      const ac = new AbortController();
      ac.abort();
      const abEv = await api.createEvaluator({ signal: ac.signal });
      try {
        let cancelled = false;
        try {
          await abEv.evaluateOutputText(api.ModuleSource.text("a = 1\n"));
        } catch (err) {
          cancelled = err instanceof api.PklError && err.code === "cancelled";
        }
        assertEq(cancelled, true, "pre-aborted signal");
      } finally {
        abEv.close();
      }
    });

    const wasmMem = rt.getWasmMemory ? rt.getWasmMemory().buffer.byteLength : null;
    return {
      init_ms: Number(initMs.toFixed(2)),
      wasm_linear_memory_bytes: wasmMem,
      checks,
    };
  });

  const output = {
    schema_version: 1,
    browser_engine: engineName,
    browser_version: browser.version(),
    browser_executable: browserExecutable,
    browser_executable_sha256: browserExecutableSha256,
    playwright_pin: pin,
    node_version: process.version,
    browser_errors: browserErrors,
    ...summary,
  };
  await writeFile(resolve(resultPathArg), JSON.stringify(output, null, 2) + "\n", "utf8");
  if (browserErrors.length > 0) {
    throw new Error(`${engineName} uncaught page errors: ${browserErrors.join("; ")}`);
  }
} finally {
  if (browser) await browser.close();
  await new Promise((res) => server.close(res));
}
"""


def run_smoke_browsers(root: Path, browsers: list[str], output_dir: Path) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    node = shutil.which("node")
    if not node:
        raise RuntimeError("node executable not found on PATH")
    wasm_pkg = root / "packages/pkl-wasm"
    if not (wasm_pkg / "dist/wasm/pkl_wasm_bg.wasm").is_file():
        raise RuntimeError("packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm missing; build @pkl-rs/wasm first")

    driver_path = output_dir / "smoke_browser_driver.mjs"
    driver_path.write_text(SMOKE_DRIVER_JS, encoding="utf-8")

    results: dict[str, Any] = {}
    all_ok = True
    for engine in browsers:
        plan_path = output_dir / f"plan-{engine}.json"
        res_path = output_dir / f"result-{engine}.json"
        write_json(
            plan_path,
            {
                "repo_root": str(root),
                "wasm_pkg_dir": str(wasm_pkg),
                "browser_engine": engine,
            },
        )
        proc = subprocess.run(
            [node, str(driver_path), str(plan_path), str(res_path)],
            cwd=root,
            capture_output=True,
            text=True,
            timeout=180,
        )
        if proc.returncode == 0 and res_path.is_file():
            doc = json.loads(res_path.read_bytes())
            doc["status"] = "pass"
            results[engine] = doc
        else:
            all_ok = False
            results[engine] = {
                "status": "fail",
                "exit_code": proc.returncode,
                "stdout": proc.stdout[-2000:],
                "stderr": proc.stderr[-2000:],
            }

    report = {
        "schema_version": 1,
        "mode": "smoke-browsers",
        "host": {"system": platform.system(), "machine": platform.machine(), "release": platform.release()},
        "browsers_requested": browsers,
        "all_passed": all_ok,
        "results": results,
    }
    write_json(output_dir / "report.json", report)
    return report


def run_js_analysis(
    root: Path,
    *,
    profile: str,
    browsers: list[str],
    include_chromium_no_gpu: bool,
    output_dir: Path,
) -> dict[str, Any]:
    output_dir.mkdir(parents=True, exist_ok=True)
    contract = m7_benchmark.load_contract(root)
    sampling = contract["profiles"][profile]
    js_sampling = contract["js_sampling"][profile]
    oracle_meta = m7_benchmark.ensure_pinned_oracle(root)
    node = shutil.which("node")
    if not node:
        raise RuntimeError("node executable not found on PATH")

    wasm_pkg = root / "packages/pkl-wasm"
    napi_pkg = root / "packages/pkl-napi"
    wasm_bg = wasm_pkg / "dist/wasm/pkl_wasm_bg.wasm"
    if not wasm_bg.is_file():
        raise RuntimeError("Missing packages/pkl-wasm/dist/wasm/pkl_wasm_bg.wasm; build @pkl-rs/wasm first")
    wasm_raw = wasm_bg.read_bytes()
    wasm_gzip_bytes = len(gzip.compress(wasm_raw, compresslevel=9))

    # 1. Empty Node.js baseline RSS
    node_base_cap = m7_benchmark.capture_wait4(
        [node, "-e", "process.exit(0)"],
        root,
        dict(os.environ),
        output_dir / "node-baseline",
        timeout=30.0,
    )
    node_baseline_rss = int(node_base_cap["peak_rss_bytes"])

    # 2. Live pinned Java 0.32.0 reference baselines via tested m7_benchmark helpers
    classes_dir, _ = m7_benchmark.compile_benchmark_java(oracle_meta, output_dir / "java-build", root)

    java_refs: dict[str, dict[str, Any]] = {}
    preflights: dict[str, dict[str, Path]] = {}

    for workload_key, mod_rel, needs_binary in (
        ("eval-text", "benchmarks/fixtures/collections.pkl", False),
        ("typed-conversion", "benchmarks/m7_fixtures/js_boundary_payload.pkl", True),
    ):
        mod_path = (root / mod_rel).resolve()
        pf_dir = output_dir / f"preflight-{workload_key}"
        j_text_state = pf_dir / "java-text-state"
        j_text_cmd = m7_benchmark.java_prefix_for_state(oracle_meta["command"], j_text_state) + [
            "eval",
            "--no-cache",
            "--no-project",
            "--settings",
            "pkl:settings",
            "--color=never",
            str(mod_path),
        ]
        j_text_cap = m7_benchmark.capture_wait4(
            j_text_cmd, root, m7_benchmark.build_isolated_env(oracle_meta["environment"], j_text_state), pf_dir / "java-text"
        )
        m7_benchmark.verify_capture_raw(j_text_cap, expected_exit=0, expected_stderr=b"")
        preflights[workload_key] = {"text": Path(j_text_cap["stdout"]["path"])}

        if needs_binary:
            j_bin_state = pf_dir / "java-bin-state"
            j_bin_cmd = m7_benchmark.java_prefix_for_state(oracle_meta["command"], j_bin_state) + [
                "eval",
                "--no-cache",
                "--no-project",
                "--settings",
                "pkl:settings",
                "--color=never",
                "-f",
                "pkl-binary",
                str(mod_path),
            ]
            j_bin_cap = m7_benchmark.capture_wait4(
                j_bin_cmd, root, m7_benchmark.build_isolated_env(oracle_meta["environment"], j_bin_state), pf_dir / "java-bin"
            )
            m7_benchmark.verify_capture_raw(j_bin_cap, expected_exit=0, expected_stderr=b"")
            preflights[workload_key]["binary"] = Path(j_bin_cap["stdout"]["path"])

        if workload_key == "typed-conversion":
            typed_ref = m7_benchmark._run_java_typed_persistent_ref(
                mod_path, output_dir / "java-ref-typed", oracle_meta, classes_dir, root, sampling
            )
            java_refs[workload_key] = {
                "median_ns": float(typed_ref["fork_median_operation_ns"]["median"]),
                "p95_ns": float(typed_ref["fork_median_operation_ns"]["p95"]),
                "peak_rss_bytes": float(typed_ref["fork_peak_rss_bytes"]["median"]),
                "converged": typed_ref["warmup"] == "converged",
                "forks": typed_ref["forks"],
            }
        else:
            ref_dir = output_dir / "java-ref-eval-text"
            expected_api = ref_dir / "expected-api.bin"
            pf_state = ref_dir / "state/verify"
            j_verify_cmd = m7_benchmark.java_benchmark_cmd(
                oracle_meta,
                classes_dir,
                pf_state,
                "verify",
                mod_path,
                expected_api,
                False,
                sampling,
                ref_dir / "first-verify.bin",
                write_expected=True,
            )
            j_verify_cap = m7_benchmark.capture_wait4(
                j_verify_cmd, root, m7_benchmark.build_isolated_env(oracle_meta["environment"], pf_state), ref_dir / "verify", timeout=180.0
            )
            m7_benchmark.verify_capture_raw(j_verify_cap, expected_exit=0, helper=True)
            forks = []
            timeout = max(float(sampling["maximum_warmup_seconds"]) + 180.0, 240.0)
            for fi in range(sampling["independent_api_forks"]):
                fstate = ref_dir / f"state/fork-{fi}"
                first_out = ref_dir / f"first-fork-{fi}.bin"
                cmd = m7_benchmark.java_benchmark_cmd(
                    oracle_meta,
                    classes_dir,
                    fstate,
                    "persistent",
                    mod_path,
                    expected_api,
                    False,
                    sampling,
                    first_out,
                    write_expected=False,
                )
                cap = m7_benchmark.capture_wait4(
                    cmd, root, m7_benchmark.build_isolated_env(oracle_meta["environment"], fstate), ref_dir / f"fork-{fi}", timeout=timeout
                )
                parsed = m7_benchmark.parse_probe_series(cap, sampling, "persistent", "java-eval-text")
                forks.append(parsed)
            java_refs[workload_key] = {
                "median_ns": float(statistics.median(f["operation_ns"]["median"] for f in forks)),
                "p95_ns": float(statistics.median(f["operation_ns"]["p95"] for f in forks)),
                "peak_rss_bytes": float(statistics.median(f["peak_rss_bytes"] for f in forks)),
                "converged": all(f["warmup"] == "converged" for f in forks),
                "forks": forks,
            }

    # 3. Run all JS/Wasm targets
    target_specs: list[dict[str, Any]] = [
        {"id": "napi-node", "kind": "napi"},
        {"id": "wasm-node", "kind": "wasm-node"},
    ]
    for eng in browsers:
        target_specs.append({"id": f"wasm-browser-{eng}", "kind": "wasm-browser", "browser_engine": eng, "launch_args": []})
        if eng == "chromium" and include_chromium_no_gpu:
            target_specs.append(
                {
                    "id": "wasm-browser-chromium-no-gpu",
                    "kind": "wasm-browser",
                    "browser_engine": "chromium",
                    "launch_args": ["--disable-gpu", "--disable-software-rasterizer", "--disable-dev-shm-usage"],
                }
            )

    rows: list[dict[str, Any]] = []
    for spec in target_specs:
        for workload_key, op_mode, mod_rel in (
            ("eval-text", "eval-expression", "benchmarks/fixtures/collections.pkl"),
            ("typed-conversion", "typed-conversion", "benchmarks/m7_fixtures/js_boundary_payload.pkl"),
        ):
            run_id = f"{spec['id']}:{workload_key}"
            run_dir = output_dir / "runs" / run_id.replace(":", "_")
            run_dir.mkdir(parents=True, exist_ok=True)
            plan_path = run_dir / "plan.json"
            res_path = run_dir / "result.json"

            mod_abs = (root / mod_rel).resolve()
            plan: dict[str, Any] = {
                "repo_root": str(root),
                "napi_pkg_dir": str(napi_pkg),
                "wasm_pkg_dir": str(wasm_pkg),
                "module_path": str(mod_abs),
                "module_uri": mod_abs.as_uri(),
                "expected_text_path": str(preflights[workload_key]["text"]),
                "expected_binary_path": str(preflights[workload_key]["binary"]) if "binary" in preflights[workload_key] else None,
                "op_mode": op_mode,
                "expr": "run(0)",
                "sampling": js_sampling,
            }
            if spec["kind"] == "napi":
                driver = root / "benchmarks/m7_fixtures/bench_napi.mjs"
            elif spec["kind"] == "wasm-node":
                driver = root / "benchmarks/m7_fixtures/bench_wasm_node.mjs"
            else:
                driver = root / "benchmarks/m7_fixtures/bench_wasm_browser.mjs"
                plan["browser_engine"] = spec["browser_engine"]
                plan["launch_args"] = spec.get("launch_args", [])

            write_json(plan_path, plan)
            cap = m7_benchmark.capture_wait4(
                [node, str(driver), str(plan_path), str(res_path)],
                root,
                dict(os.environ),
                run_dir / "driver",
                timeout=240.0,
            )
            jref = java_refs[workload_key]
            if cap["status"] != "executed" or cap["exit_code"] != 0 or not res_path.is_file():
                err_tail = Path(cap["stderr"]["path"]).read_text(encoding="utf-8", errors="replace")[-1200:]
                rows.append(
                    {
                        "id": run_id,
                        "target": spec["id"],
                        "kind": spec["kind"],
                        "workload": workload_key,
                        "status": "error",
                        "exit_code": cap["exit_code"],
                        "stderr_tail": err_tail,
                    }
                )
                continue

            doc = json.loads(res_path.read_bytes())
            meas_vals = [float(m["ns_per_op"]) for m in doc["measurements"]]
            op_dist = m7_benchmark.distribution(meas_vals)
            driver_rss = int(cap["peak_rss_bytes"])
            total_rss = int(doc["peak_tree_rss_bytes"]) if spec["kind"] == "wasm-browser" else driver_rss
            renderer_rss = doc.get("peak_renderer_rss_bytes")
            role_rss = doc.get("peak_role_rss_bytes") or {}
            wasm_lin_mem = (
                doc.get("wasm_linear_memory_bytes_peak")
                if spec["kind"] == "wasm-browser"
                else doc.get("wasm_linear_memory_bytes_after_eval")
            )
            node_mem = doc.get("node_memory_usage") or {}
            rows.append(
                {
                    "id": run_id,
                    "target": spec["id"],
                    "kind": spec["kind"],
                    "browser_engine": spec.get("browser_engine"),
                    "browser_version": doc.get("browser"),
                    "launch_args": spec.get("launch_args", []),
                    "workload": workload_key,
                    "status": "ok",
                    "preflight_verified": bool(doc.get("preflight_raw_byte_verified")),
                    "converged": bool(doc.get("warmup", {}).get("converged")),
                    "cross_origin_isolated": doc.get("cross_origin_isolated"),
                    "wasm_init_only_ns": doc.get("wasm_init_only_ns"),
                    "initialize_ns": doc.get("initialize_ns"),
                    "first_request_ns": doc.get("first_request_ns"),
                    "median_ns": op_dist["median"],
                    "p95_ns": op_dist["p95"],
                    "java_median_ns": jref["median_ns"],
                    "time_ratio_vs_java": op_dist["median"] / jref["median_ns"],
                    "driver_wait4_rss_bytes": driver_rss,
                    "total_peak_rss_bytes": total_rss,
                    "total_rss_ratio_vs_java": total_rss / jref["peak_rss_bytes"],
                    "renderer_peak_rss_bytes": renderer_rss,
                    "renderer_rss_ratio_vs_java": (renderer_rss / jref["peak_rss_bytes"]) if renderer_rss else None,
                    "role_rss_bytes": role_rss,
                    "peak_process_breakdown": doc.get("peak_process_breakdown") or [],
                    "wasm_linear_memory_bytes": wasm_lin_mem,
                    "wasm_linear_mem_ratio_vs_java": (wasm_lin_mem / jref["peak_rss_bytes"]) if wasm_lin_mem else None,
                    "node_memory_usage": node_mem,
                    "js_heap_after_eval": doc.get("js_heap_after_eval"),
                    "java_peak_rss_bytes": jref["peak_rss_bytes"],
                }
            )

    report = {
        "schema_version": 1,
        "mode": "js-analysis",
        "profile": profile,
        "host": {"system": platform.system(), "machine": platform.machine(), "release": platform.release()},
        "oracle_jdk": oracle_meta["jdk"]["properties"],
        "node_baseline_wait4_rss_bytes": node_baseline_rss,
        "wasm_binary_bytes": len(wasm_raw),
        "wasm_binary_gzip_bytes": wasm_gzip_bytes,
        "java_references": java_refs,
        "results": rows,
    }
    write_json(output_dir / "report.json", report)

    # Render concise Markdown analysis table
    lines = [
        f"# JS / Wasm Performance & Memory Analysis ({platform.system()} {platform.machine()}, profile `{profile}`)",
        "",
        f"- **Oracle**: Eclipse Adoptium Temurin `{oracle_meta['jdk']['properties']['java.runtime.version']}` (Pkl `0.32.0`)",
        f"- **Empty Node.js baseline `os.wait4` RSS**: `{fmt_mib(node_baseline_rss)}`",
        f"- **`pkl_wasm_bg.wasm` binary size**: `{len(wasm_raw):,} B` (`{len(wasm_raw) / (1024 * 1024):.2f} MiB` raw, `{wasm_gzip_bytes / (1024 * 1024):.2f} MiB` gzip-9)",
        "",
        "## Execution Time & Memory Decomposition vs Live Pinned Java 0.32.0",
        "",
        "| Target | Workload | Init / First | Warm Median | Java Median | Time Ratio | Total Peak RSS | Total RSS Ratio | Renderer RSS (Ratio) | Wasm Linear Mem | Browser Role RSS (`main / rend / gpu / util`) |",
        "| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |",
    ]
    for r in rows:
        if r["status"] != "ok":
            lines.append(f"| `{r['target']}` | `{r['workload']}` | ERROR (`exit={r.get('exit_code')}`) | — | — | — | — | — | — | — | `{r.get('stderr_tail', '').splitlines()[-1:]}` |")
            continue
        roles = r.get("role_rss_bytes") or {}
        role_str = (
            f"{fmt_mib(roles.get('browser_main', 0))} / {fmt_mib(roles.get('renderer', 0))} / "
            f"{fmt_mib(roles.get('gpu', 0))} / {fmt_mib(roles.get('utility_network', 0))}"
            if r["kind"] == "wasm-browser"
            else f"Node heapUsed {fmt_mib((r.get('node_memory_usage') or {}).get('heapUsed'))}"
        )
        rend_str = (
            f"{fmt_mib(r['renderer_peak_rss_bytes'])} ({fmt_ratio(r['renderer_peak_rss_bytes'], r['java_peak_rss_bytes'])})"
            if r.get("renderer_peak_rss_bytes")
            else "—"
        )
        lines.append(
            f"| `{r['target']}` | `{r['workload']}` | {fmt_ns(r.get('initialize_ns'))} / {fmt_ns(r.get('first_request_ns'))} | "
            f"{fmt_ns(r['median_ns'])} | {fmt_ns(r['java_median_ns'])} | **{fmt_ratio(r['median_ns'], r['java_median_ns'])}** | "
            f"{fmt_mib(r['total_peak_rss_bytes'])} | **{fmt_ratio(r['total_peak_rss_bytes'], r['java_peak_rss_bytes'])}** | "
            f"{rend_str} | {fmt_mib(r.get('wasm_linear_memory_bytes'))} | {role_str} |"
        )
    (output_dir / "analysis.md").write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--smoke-browsers", type=str, default=None, help="Comma-separated browser engines for functional smoke (chromium,firefox,webkit)")
    parser.add_argument("--browsers", type=str, default="chromium,firefox,webkit", help="Comma-separated browser engines for analysis mode")
    parser.add_argument("--profile", choices=("quick", "full"), default="quick")
    parser.add_argument("--chromium-no-gpu", action="store_true", help="Also profile Chromium with --disable-gpu for GPU-process RSS attribution")
    args = parser.parse_args()
    root = args.root.resolve()
    out = args.output.resolve()

    try:
        if args.smoke_browsers:
            browsers = [b.strip() for b in args.smoke_browsers.split(",") if b.strip()]
            for b in browsers:
                if b not in VALID_BROWSERS:
                    raise ValueError(f"Unsupported browser: {b}")
            rep = run_smoke_browsers(root, browsers, out)
            print(json.dumps({k: v.get("status") for k, v in rep["results"].items()}, sort_keys=True))
            return 0 if rep["all_passed"] else 1

        browsers = [b.strip() for b in args.browsers.split(",") if b.strip()]
        for b in browsers:
            if b not in VALID_BROWSERS:
                raise ValueError(f"Unsupported browser: {b}")
        rep = run_js_analysis(
            root,
            profile=args.profile,
            browsers=browsers,
            include_chromium_no_gpu=args.chromium_no_gpu,
            output_dir=out,
        )
        print((out / "analysis.md").read_text(encoding="utf-8"))
        return 0 if all(r.get("status") == "ok" for r in rep["results"]) else 1
    except (OSError, ValueError, KeyError, RuntimeError, subprocess.SubprocessError) as err:
        print(f"M7 JS analysis error: {err}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
