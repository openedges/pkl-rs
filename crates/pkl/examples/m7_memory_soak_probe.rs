//! Long-running memory reclamation soak probe for Milestone M7 (`pkl` public Rust API + `SourceSession` GC audit).
//!
//! Exercises all 6 M7 memory reclamation scenarios using 100% safe Rust (`unsafe_code = "deny"`):
//!   1. `repeated-success`: thousands of evaluations on one persistent `Evaluator`
//!   2. `repeated-failure`: thousands of failing evaluations (type/constraint/runtime/syntax/lazy/caught)
//!   3. `unique-sources-churn`: hundreds of distinct unique module URIs/sources via uncached reader + fresh evaluator churn
//!   4. `cyclic-graphs`: cyclic objects, class rings, cyclic closures & collections + `SourceSession` tracing GC `HeapStats` audit
//!   5. `lifecycle-cancel-timeout-close`: repeated create -> eval -> timeout / `cancel()` -> idempotent `close()` (+ optional external reader)
//!   6. `cache-churn`: repeated module/package/modulepath cache loads, invalidations, and reloads
//!
//! Speaks a newline-delimited JSON checkpoint protocol on stdout, optionally pausing for a
//! parent newline ACK (`--sync-stdin`) so the orchestrator can capture exact kernel RSS/FD/PID telemetry.

#![forbid(unsafe_code)]

use pkl::{
    EvaluatorBuilder, ExternalReaderSpec, ModuleKey, ModuleKeyFactories, ModuleSource,
    PklErrorKind, ResolvedModuleKey, SecurityManager,
};
use pkl_host_native::{NativeHost, NativeHostConfig};
use pkl_runtime::evaluation::frontend::DiagnosticProfile;
use pkl_runtime::heap::HeapStats;
use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;
use std::time::{Duration, Instant};

struct Config {
    scenario: String,
    iterations: usize,
    warmup: usize,
    sample_interval: usize,
    work_dir: PathBuf,
    pkg_pool_dir: Option<PathBuf>,
    ext_reader_python: Option<String>,
    ext_reader_script: Option<PathBuf>,
    sync_stdin: bool,
}

fn parse_args() -> Result<Config, String> {
    let args: Vec<String> = std::env::args().collect();
    let mut scenario = String::new();
    let mut iterations = 1000usize;
    let mut warmup = 100usize;
    let mut sample_interval = 100usize;
    let mut work_dir = PathBuf::from(".");
    let mut pkg_pool_dir: Option<PathBuf> = None;
    let mut ext_reader_python: Option<String> = None;
    let mut ext_reader_script: Option<PathBuf> = None;
    let mut sync_stdin = false;

    let mut i = 1usize;
    while i < args.len() {
        let arg = args[i].as_str();
        match arg {
            "--scenario" => {
                i += 1;
                scenario = args
                    .get(i)
                    .ok_or_else(|| "missing --scenario value".to_string())?
                    .clone();
            }
            "--iterations" => {
                i += 1;
                iterations = args
                    .get(i)
                    .ok_or_else(|| "missing --iterations value".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --iterations: {e}"))?;
            }
            "--warmup" => {
                i += 1;
                warmup = args
                    .get(i)
                    .ok_or_else(|| "missing --warmup value".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --warmup: {e}"))?;
            }
            "--sample-interval" => {
                i += 1;
                sample_interval = args
                    .get(i)
                    .ok_or_else(|| "missing --sample-interval value".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("invalid --sample-interval: {e}"))?;
            }
            "--work-dir" => {
                i += 1;
                work_dir = PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| "missing --work-dir value".to_string())?,
                );
            }
            "--pkg-pool-dir" => {
                i += 1;
                pkg_pool_dir = Some(PathBuf::from(
                    args.get(i)
                        .ok_or_else(|| "missing --pkg-pool-dir value".to_string())?,
                ));
            }
            "--ext-reader-python" => {
                i += 1;
                ext_reader_python = Some(
                    args.get(i)
                        .ok_or_else(|| "missing --ext-reader-python value".to_string())?
                        .clone(),
                );
            }
            "--ext-reader-script" => {
                i += 1;
                ext_reader_script =
                    Some(PathBuf::from(args.get(i).ok_or_else(|| {
                        "missing --ext-reader-script value".to_string()
                    })?));
            }
            "--sync-stdin" => {
                sync_stdin = true;
            }
            other => return Err(format!("unknown argument: {other}")),
        }
        i += 1;
    }

    if scenario.is_empty() {
        return Err("--scenario is required".to_string());
    }
    if iterations == 0 {
        return Err("--iterations must be > 0".to_string());
    }
    if sample_interval == 0 {
        sample_interval = (iterations / 10).max(1);
    }

    Ok(Config {
        scenario,
        iterations,
        warmup,
        sample_interval,
        work_dir,
        pkg_pool_dir,
        ext_reader_python,
        ext_reader_script,
        sync_stdin,
    })
}

fn json_escape(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 2);
    out.push('"');
    for ch in input.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(&mut out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn count_open_fds() -> Option<usize> {
    for dir in ["/dev/fd", "/proc/self/fd"] {
        if let Ok(entries) = std::fs::read_dir(dir) {
            let count = entries.count();
            return Some(count.saturating_sub(1));
        }
    }
    None
}

fn read_linux_self_rss_bytes() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let resident_pages = statm.split_whitespace().nth(1)?.parse::<u64>().ok()?;
    Some(resident_pages.saturating_mul(4096))
}

fn emit_sample(
    phase: &str,
    iter: usize,
    start: Instant,
    sync_stdin: bool,
    heap_stats: Option<HeapStats>,
) {
    let elapsed_ms = start.elapsed().as_millis();
    let fds = count_open_fds();
    let rss = read_linux_self_rss_bytes();
    let mut line = String::with_capacity(256);
    let _ = write!(
        &mut line,
        "{{\"type\":\"sample\",\"phase\":{},\"iter\":{iter},\"elapsed_ms\":{elapsed_ms}",
        json_escape(phase)
    );
    match fds {
        Some(n) => {
            let _ = write!(&mut line, ",\"open_fds\":{n}");
        }
        None => line.push_str(",\"open_fds\":null"),
    }
    match rss {
        Some(b) => {
            let _ = write!(&mut line, ",\"self_rss_bytes\":{b}");
        }
        None => line.push_str(",\"self_rss_bytes\":null"),
    }
    if let Some(hs) = heap_stats {
        let _ = write!(
            &mut line,
            ",\"heap_stats\":{{\"live\":{},\"executable_live\":{},\"slots\":{},\"collections\":{},\"reclaimed\":{}}}",
            hs.live, hs.executable_live, hs.slots, hs.collections, hs.reclaimed
        );
    }
    line.push_str("}\n");
    {
        let mut out = std::io::stdout().lock();
        let _ = out.write_all(line.as_bytes());
        let _ = out.flush();
    }
    if sync_stdin {
        let mut ack = String::new();
        let _ = std::io::stdin().lock().read_line(&mut ack);
    }
}

struct HeapProbeResult {
    iterations: usize,
    warmup: usize,
    after_warmup: HeapStats,
    midpoint: HeapStats,
    after_final: HeapStats,
    assertions_ok: bool,
}

fn emit_summary(
    scenario: &str,
    warmup: usize,
    completed: usize,
    assertions_passed: usize,
    assertion_failures: &[String],
    elapsed_ms: u128,
    heap_probe: Option<&HeapProbeResult>,
) {
    let mut line = String::with_capacity(512);
    let _ = write!(
        &mut line,
        "{{\"type\":\"summary\",\"scenario\":{},\"warmup_iterations\":{warmup},\"completed_iterations\":{completed},\"assertions_passed\":{assertions_passed},\"elapsed_ms\":{elapsed_ms},\"assertion_failures\":[",
        json_escape(scenario)
    );
    for (idx, msg) in assertion_failures.iter().enumerate() {
        if idx > 0 {
            line.push(',');
        }
        line.push_str(&json_escape(msg));
    }
    line.push(']');
    if let Some(hp) = heap_probe {
        let fmt_hs = |hs: HeapStats| {
            format!(
                "{{\"live\":{},\"executable_live\":{},\"slots\":{},\"collections\":{},\"reclaimed\":{}}}",
                hs.live, hs.executable_live, hs.slots, hs.collections, hs.reclaimed
            )
        };
        let _ = write!(
            &mut line,
            ",\"heap_probe\":{{\"iterations\":{},\"warmup\":{},\"after_warmup\":{},\"midpoint\":{},\"after_final\":{},\"assertions_ok\":{}}}",
            hp.iterations,
            hp.warmup,
            fmt_hs(hp.after_warmup),
            fmt_hs(hp.midpoint),
            fmt_hs(hp.after_final),
            hp.assertions_ok
        );
    }
    line.push_str("}\n");
    let mut out = std::io::stdout().lock();
    let _ = out.write_all(line.as_bytes());
    let _ = out.flush();
}

fn make_base_builder() -> EvaluatorBuilder {
    EvaluatorBuilder::unconfigured()
        .with_allowed_modules(vec![
            "pkl:".to_string(),
            "repl:".to_string(),
            "file:".to_string(),
            "modulepath:".to_string(),
            "package:".to_string(),
            "uncached:".to_string(),
            "churn:".to_string(),
        ])
        .with_allowed_resources(vec![
            "prop:".to_string(),
            "env:".to_string(),
            "file:".to_string(),
            "modulepath:".to_string(),
            "package:".to_string(),
            "extres:".to_string(),
        ])
}

fn make_padding(seed: usize, repeats: usize) -> String {
    let mut out = String::with_capacity(repeats * 32);
    for k in 0..repeats {
        let _ = write!(&mut out, "seg_{seed:06}_{k:02}_abcdefghij");
    }
    out
}

fn run_repeated_success(cfg: &Config, start: Instant) -> (usize, usize, Vec<String>) {
    let mut failures = Vec::new();
    let mut assertions_passed = 0usize;

    let file_mod_path = cfg.work_dir.join("repeated_success_base.pkl");
    let file_source = r#"
class Entry {
  idx: Int
  label: String
  score: Int
}
items = List(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
entries: Listing<Entry> = new {
  for (n in items) {
    new Entry {
      idx = n
      label = "item-\(n)"
      score = n * n + 3
    }
  }
}
total = entries.toList().map((e) -> e.score).fold(0, (a, b) -> a + b)
"#;
    if let Err(err) = std::fs::write(&file_mod_path, file_source) {
        failures.push(format!("write base file failed: {err}"));
        return (0, 0, failures);
    }

    let mut evaluator = match make_base_builder().build() {
        Ok(ev) => ev,
        Err(err) => {
            failures.push(format!("build evaluator failed: {err}"));
            return (0, 0, failures);
        }
    };

    let total_steps = cfg.warmup + cfg.iterations;
    let mut completed = 0usize;
    let file_src = ModuleSource::path(&file_mod_path);

    for step in 0..total_steps {
        let iter_id = step;
        let expected_sum = (iter_id % 100) * 16 + 136;
        if step % 3 == 0 {
            let expr = format!("total + {}", iter_id % 100);
            match evaluator.evaluate_expression_string(&file_src, &expr) {
                Ok(out) => {
                    let expected = format!("{}", 1544 + (iter_id % 100));
                    if out == expected {
                        assertions_passed += 1;
                    } else if failures.len() < 16 {
                        failures.push(format!(
                            "iter {iter_id}: file expr mismatch: expected {expected}, got {out}"
                        ));
                    }
                }
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!("iter {iter_id}: file expr error: {err}"));
                    }
                }
            }
        } else {
            let pad = make_padding(iter_id, 24);
            let src_text = format!(
                r#"// padding: {pad}
local base = {base}
local nums = List(1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)
local f = (x) -> x + base
mapped = nums.map(f)
sum = mapped.fold(0, (a, b) -> a + b)
meta {{
  tag = "iter-{iter_id:06}"
  payload = "{short_pad}"
  entries = new Mapping {{
    for (k in List(1, 2, 3, 4)) {{
      ["k-\(k)"] = base + k
    }}
  }}
}}
"#,
                base = iter_id % 100,
                short_pad = &pad[..pad.len().min(256)],
            );
            let src = ModuleSource::text(src_text);
            let res = if step % 3 == 1 {
                evaluator.evaluate_output_text(&src)
            } else {
                evaluator
                    .evaluate_output_bytes(&src)
                    .map(|b| String::from_utf8_lossy(&b).into_owned())
            };
            match res {
                Ok(out) => {
                    let expected_needle = format!("sum = {expected_sum}\n");
                    if out.contains(&expected_needle) {
                        assertions_passed += 1;
                    } else if failures.len() < 16 {
                        failures.push(format!(
                            "iter {iter_id}: inline output missing `{expected_needle}`"
                        ));
                    }
                }
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!("iter {iter_id}: inline eval error: {err}"));
                    }
                }
            }
        }

        if step + 1 == cfg.warmup {
            emit_sample("warmup-end", 0, start, cfg.sync_stdin, None);
        } else if step >= cfg.warmup {
            completed += 1;
            if completed == cfg.iterations {
                emit_sample("final", completed, start, cfg.sync_stdin, None);
            } else if completed.is_multiple_of(cfg.sample_interval) {
                emit_sample("sample", completed, start, cfg.sync_stdin, None);
            }
        }
    }

    evaluator.close();
    (completed, assertions_passed, failures)
}

fn run_repeated_failure(cfg: &Config, start: Instant) -> (usize, usize, Vec<String>) {
    let mut failures = Vec::new();
    let mut assertions_passed = 0usize;

    let mut evaluator = match make_base_builder().build() {
        Ok(ev) => ev,
        Err(err) => {
            failures.push(format!("build evaluator failed: {err}"));
            return (0, 0, failures);
        }
    };

    let total_steps = cfg.warmup + cfg.iterations;
    let mut completed = 0usize;

    for step in 0..total_steps {
        let pad = make_padding(step, 32);
        let mode = step % 7;
        let (src_text, expect_err) = match mode {
            0 => (format!("// {pad}\nx: Int = \"not-an-int-{step}\"\n"), true),
            1 => (
                format!(
                    "// {pad}\nx: Int(this > 1000 && this < 2000) = {}\n",
                    step % 500
                ),
                true,
            ),
            2 => (
                format!(
                    "// {pad}\nfunction dive(n) = if (n <= 0) throw(\"boom-{step}\") else dive(n - 1)\nx = dive(6)\n"
                ),
                true,
            ),
            3 => (
                format!(
                    "// {pad}\nxs = List(1, 2, 3)\nx = xs[{}]\n",
                    10 + (step % 5)
                ),
                true,
            ),
            4 => (format!("// {pad}\nx = (1 + 2 + \n"), true),
            5 => (
                format!(
                    "// {pad}\nlocal lazyBad = throw(\"lazy-fail-{step}\")\nsafe = 1 + {step}\nboom = lazyBad\n"
                ),
                true,
            ),
            _ => (
                format!(
                    "// {pad}\nlocal missing = read?(\"prop:soakMissing_{step}\") ?? \"{step}\"\nlocal nonMatch = \"bad\" is Int(this > 0)\nok = if (!nonMatch && missing == \"{step}\") {next} else -1\n",
                    next = step + 1
                ),
                false,
            ),
        };

        let res = evaluator.evaluate_output_text(&ModuleSource::text(src_text));
        if expect_err {
            match res {
                Err(err) if err.kind() == PklErrorKind::Evaluation => {
                    assertions_passed += 1;
                }
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!(
                            "step {step} mode {mode}: unexpected err kind {:?}",
                            err.kind()
                        ));
                    }
                }
                Ok(out) => {
                    if failures.len() < 16 {
                        failures.push(format!(
                            "step {step} mode {mode}: expected error, got ok ({} B)",
                            out.len()
                        ));
                    }
                }
            }
        } else {
            let expected = format!("ok = {}\n", step + 1);
            match res {
                Ok(out) if out == expected => assertions_passed += 1,
                Ok(out) => {
                    if failures.len() < 16 {
                        failures.push(format!(
                            "step {step}: caught mismatch: expected {expected:?}, got {out:?}"
                        ));
                    }
                }
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!("step {step}: caught failed: {err}"));
                    }
                }
            }
        }

        if step + 1 == cfg.warmup {
            emit_sample("warmup-end", 0, start, cfg.sync_stdin, None);
        } else if step >= cfg.warmup {
            completed += 1;
            if completed == cfg.iterations {
                emit_sample("final", completed, start, cfg.sync_stdin, None);
            } else if completed.is_multiple_of(cfg.sample_interval) {
                emit_sample("sample", completed, start, cfg.sync_stdin, None);
            }
        }
    }

    evaluator.close();
    (completed, assertions_passed, failures)
}

struct UncachedModuleKey {
    uri: String,
    source_text: String,
}

impl ModuleKey for UncachedModuleKey {
    fn uri(&self) -> &str {
        &self.uri
    }

    fn has_hierarchical_uris(&self) -> bool {
        true
    }

    fn is_cached(&self) -> bool {
        false
    }

    fn resolve(&self, _security_manager: &dyn SecurityManager) -> pkl::Result<ResolvedModuleKey> {
        Ok(ResolvedModuleKey::virtual_module(
            self.uri.clone(),
            self.source_text.clone(),
        ))
    }
}

fn run_unique_sources_churn(cfg: &Config, start: Instant) -> (usize, usize, Vec<String>) {
    let mut failures = Vec::new();
    let mut assertions_passed = 0usize;

    let uncached_factory = ModuleKeyFactories::from_fn(|uri: &str| {
        let rest = uri.strip_prefix("uncached:///mod-")?;
        let id_str = rest.strip_suffix(".pkl")?;
        let id = id_str.parse::<usize>().ok()?;
        let pad = make_padding(id, 80);
        let src = format!(
            r#"// unique uncached module {id}: {pad}
class Box_{id} {{
  id: Int = {id}
  tag: String = "u-{id}"
  vals: List<Int> = List({id}, {p1}, {p2})
}}
inst = new Box_{id} {{}}
checksum = inst.id * 2 + inst.vals.length
"#,
            p1 = id + 1,
            p2 = id + 2,
        );
        Some(Box::new(UncachedModuleKey {
            uri: uri.to_string(),
            source_text: src,
        }))
    });

    let mut persistent_ev = match make_base_builder()
        .add_module_key_factory(uncached_factory)
        .build()
    {
        Ok(ev) => ev,
        Err(err) => {
            failures.push(format!("build persistent evaluator failed: {err}"));
            return (0, 0, failures);
        }
    };

    let unique_dir = cfg.work_dir.join("unique_sources");
    if let Err(err) = std::fs::create_dir_all(&unique_dir) {
        failures.push(format!("mkdir unique_sources failed: {err}"));
        return (0, 0, failures);
    }

    let total_steps = cfg.warmup + cfg.iterations;
    let mut completed = 0usize;

    for step in 0..total_steps {
        if step % 2 == 0 {
            // Sub-pattern A: unique uncached URI on persistent evaluator
            let uri = format!("uncached:///mod-{step}.pkl");
            match persistent_ev.evaluate_expression_string(&ModuleSource::uri(&uri), "checksum") {
                Ok(out) => {
                    let expected = format!("{}", step * 2 + 3);
                    if out == expected {
                        assertions_passed += 1;
                    } else if failures.len() < 16 {
                        failures.push(format!(
                            "step {step} uncached mismatch: expected {expected}, got {out}"
                        ));
                    }
                }
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!("step {step} uncached error: {err}"));
                    }
                }
            }
        } else {
            // Sub-pattern B: fresh evaluator loading 3 unique file:// modules with cross-imports -> close()
            let p_a = unique_dir.join(format!("u_{step}_a.pkl"));
            let p_b = unique_dir.join(format!("u_{step}_b.pkl"));
            let p_c = unique_dir.join(format!("u_{step}_c.pkl"));
            let pad = make_padding(step, 64);
            let src_a = format!("// {pad}\nval = {step}\n");
            let src_b = format!("// {pad}\nimport \"u_{step}_a.pkl\" as a\ndoubled = a.val * 2\n");
            let src_c =
                format!("// {pad}\nimport \"u_{step}_b.pkl\" as b\nresult = b.doubled + 7\n");
            if std::fs::write(&p_a, src_a).is_ok()
                && std::fs::write(&p_b, src_b).is_ok()
                && std::fs::write(&p_c, src_c).is_ok()
            {
                match make_base_builder().build() {
                    Ok(mut ev) => {
                        let expected = format!("result = {}\n", step * 2 + 7);
                        match ev.evaluate_output_text(&ModuleSource::path(&p_c)) {
                            Ok(out) if out == expected => assertions_passed += 1,
                            Ok(out) => {
                                if failures.len() < 16 {
                                    failures.push(format!(
                                        "step {step} fresh file mismatch: expected {expected:?}, got {out:?}"
                                    ));
                                }
                            }
                            Err(err) => {
                                if failures.len() < 16 {
                                    failures.push(format!("step {step} fresh file error: {err}"));
                                }
                            }
                        }
                        ev.close();
                    }
                    Err(err) => {
                        if failures.len() < 16 {
                            failures.push(format!("step {step} fresh builder error: {err}"));
                        }
                    }
                }
            }
            let _ = std::fs::remove_file(&p_a);
            let _ = std::fs::remove_file(&p_b);
            let _ = std::fs::remove_file(&p_c);
        }

        if step + 1 == cfg.warmup {
            emit_sample("warmup-end", 0, start, cfg.sync_stdin, None);
        } else if step >= cfg.warmup {
            completed += 1;
            if completed == cfg.iterations {
                emit_sample("final", completed, start, cfg.sync_stdin, None);
            } else if completed.is_multiple_of(cfg.sample_interval) {
                emit_sample("sample", completed, start, cfg.sync_stdin, None);
            }
        }
    }

    persistent_ev.close();
    (completed, assertions_passed, failures)
}

fn cyclic_program(iter_id: usize) -> String {
    let pad = make_padding(iter_id, 12);
    format!(
        r#"class Node {{
  id: Int
  payload: String
  next: Node
}}
local a {{
  tag = "a-{iter_id:06}"
  payload = "{pad}"
  peer = b
}}
local b {{
  tag = "b-{iter_id:06}"
  payload = "{pad}"
  peer = a
}}
local n0: Node = new Node {{ id = {i0}; payload = "{pad}"; next = n1 }}
local n1: Node = new Node {{ id = {i1}; payload = "{pad}"; next = n2 }}
local n2: Node = new Node {{ id = {i2}; payload = "{pad}"; next = n3 }}
local n3: Node = new Node {{ id = {i3}; payload = "{pad}"; next = n4 }}
local n4: Node = new Node {{ id = {i4}; payload = "{pad}"; next = n5 }}
local n5: Node = new Node {{ id = {i5}; payload = "{pad}"; next = n6 }}
local n6: Node = new Node {{ id = {i6}; payload = "{pad}"; next = n7 }}
local n7: Node = new Node {{ id = {i7}; payload = "{pad}"; next = n0 }}
local f = (k) -> if (k <= 0) n0.id else g.apply(k - 1) + 1
local g = (k) -> if (k <= 0) n1.id else f.apply(k - 1) + 2
local bag = new Mapping {{
  ["a"] = a
  ["b"] = b
  ["ring"] = new Listing {{ n0; n1; n2; n3 }}
}}
summary = "\(a.peer.peer.tag)|\(b.peer.tag)|\(n0.next.next.next.next.next.next.next.next.id)|\(f.apply(6))|\(bag["a"].tag)"
"#,
        i0 = iter_id,
        i1 = iter_id + 1,
        i2 = iter_id + 2,
        i3 = iter_id + 3,
        i4 = iter_id + 4,
        i5 = iter_id + 5,
        i6 = iter_id + 6,
        i7 = iter_id + 7,
    )
}

fn run_cyclic_heap_probe() -> HeapProbeResult {
    let warmup = 20usize;
    let iters = 160usize;
    let config = NativeHostConfig {
        allowed_modules: Some(vec!["pkl:".to_string(), "repl:".to_string()]),
        allowed_resources: Some(vec!["prop:".to_string()]),
        ..NativeHostConfig::default()
    };
    let host = match NativeHost::new(config) {
        Ok(h) => h,
        Err(_) => {
            return HeapProbeResult {
                iterations: 0,
                warmup,
                after_warmup: HeapStats::default(),
                midpoint: HeapStats::default(),
                after_final: HeapStats::default(),
                assertions_ok: false,
            };
        }
    };
    let mut session = host.new_public_session(DiagnosticProfile::ApiIdentity);
    let mut stderr = Vec::new();
    let mut after_warmup = HeapStats::default();
    let mut midpoint = HeapStats::default();
    let mut assertions_ok = true;

    for step in 0..(warmup + iters) {
        let src = cyclic_program(step);
        let spec = pkl_runtime::modules::ModuleSourceSpec {
            uri: "repl:text".into(),
            contents: Some(src.as_str().into()),
        };
        let now = host.clock_ms();
        let eval_res = session
            .start_evaluation(spec, "summary", now, None)
            .and_then(|()| host.drive_public_session(&mut session, &mut stderr))
            .and_then(pkl_runtime::evaluation::session::SourceSession::result_string);
        match eval_res {
            Ok(val) => {
                let expected = format!("a-{step:06}|a-{step:06}|{step}|{}|a-{step:06}", step + 9);
                if val.to_java_utf8() != expected {
                    assertions_ok = false;
                }
            }
            Err(_) => {
                assertions_ok = false;
            }
        }
        if step + 1 == warmup {
            after_warmup = session.collect(&[]);
        } else if step + 1 == warmup + (iters / 2) {
            midpoint = session.collect(&[]);
        }
    }
    let after_final = session.collect(&[]);
    session.close();
    host.close();
    HeapProbeResult {
        iterations: iters,
        warmup,
        after_warmup,
        midpoint,
        after_final,
        assertions_ok,
    }
}

fn run_cyclic_graphs(
    cfg: &Config,
    start: Instant,
) -> (usize, usize, Vec<String>, Option<HeapProbeResult>) {
    let mut failures = Vec::new();
    let mut assertions_passed = 0usize;

    let mut evaluator = match make_base_builder().build() {
        Ok(ev) => ev,
        Err(err) => {
            failures.push(format!("build evaluator failed: {err}"));
            return (0, 0, failures, None);
        }
    };

    let total_steps = cfg.warmup + cfg.iterations;
    let mut completed = 0usize;

    for step in 0..total_steps {
        let src = cyclic_program(step);
        let expected = format!("a-{step:06}|a-{step:06}|{step}|{}|a-{step:06}", step + 9);
        match evaluator.evaluate_expression_string(&ModuleSource::text(src), "summary") {
            Ok(out) if out == expected => assertions_passed += 1,
            Ok(out) => {
                if failures.len() < 16 {
                    failures.push(format!(
                        "step {step} cyclic mismatch: expected {expected}, got {out}"
                    ));
                }
            }
            Err(err) => {
                if failures.len() < 16 {
                    failures.push(format!("step {step} cyclic error: {err}"));
                }
            }
        }

        if step + 1 == cfg.warmup {
            emit_sample("warmup-end", 0, start, cfg.sync_stdin, None);
        } else if step >= cfg.warmup {
            completed += 1;
            if completed == cfg.iterations {
                emit_sample("final", completed, start, cfg.sync_stdin, None);
            } else if completed.is_multiple_of(cfg.sample_interval) {
                emit_sample("sample", completed, start, cfg.sync_stdin, None);
            }
        }
    }

    evaluator.close();
    let heap_probe = run_cyclic_heap_probe();
    if !heap_probe.assertions_ok {
        failures.push("SourceSession cyclic heap_probe assertion failed".to_string());
    }
    (completed, assertions_passed, failures, Some(heap_probe))
}

fn run_lifecycle_cancel_timeout_close(cfg: &Config, start: Instant) -> (usize, usize, Vec<String>) {
    let mut failures = Vec::new();
    let mut assertions_passed = 0usize;

    let slow_src = r#"
function fib(n) = if (n <= 1) n else fib(n - 1) + fib(n - 2)
result = fib(40)
"#;
    let total_steps = cfg.warmup + cfg.iterations;
    let mut completed = 0usize;

    for step in 0..total_steps {
        let use_timeout = step % 2 == 0;
        let use_ext_reader =
            step % 10 == 1 && cfg.ext_reader_python.is_some() && cfg.ext_reader_script.is_some();

        let mut builder = make_base_builder();
        if use_timeout {
            builder = builder.with_timeout(Duration::from_millis(60));
        }
        if use_ext_reader
            && let (Some(py), Some(script)) = (&cfg.ext_reader_python, &cfg.ext_reader_script)
        {
            let mut readers = BTreeMap::new();
            readers.insert(
                "extres".to_string(),
                ExternalReaderSpec {
                    executable: py.clone(),
                    arguments: vec![script.to_string_lossy().into_owned()],
                    working_dir: None,
                },
            );
            builder = builder.with_external_resource_readers(readers);
        }

        let mut ev = match builder.build() {
            Ok(ev) => ev,
            Err(err) => {
                if failures.len() < 16 {
                    failures.push(format!("step {step} builder failed: {err}"));
                }
                continue;
            }
        };

        // 1. Quick valid evaluation
        let quick = format!("v = {} + 1\n", step);
        match ev.evaluate_output_text(&ModuleSource::text(quick)) {
            Ok(out) if out == format!("v = {}\n", step + 1) => assertions_passed += 1,
            Ok(out) => {
                if failures.len() < 16 {
                    failures.push(format!("step {step} quick eval mismatch: {out:?}"));
                }
            }
            Err(err) => {
                if failures.len() < 16 {
                    failures.push(format!("step {step} quick eval err: {err}"));
                }
            }
        }

        // 2. Optional external reader IPC check (exercises subprocess spawn + graceful 0x32 teardown!)
        if use_ext_reader {
            match ev.evaluate_expression_string(
                &ModuleSource::text(r#"r = read("extres:/hello.txt").text"#),
                "r",
            ) {
                Ok(out) if !out.is_empty() => assertions_passed += 1,
                Ok(_) => {
                    if failures.len() < 16 {
                        failures.push(format!("step {step} extres returned empty"));
                    }
                }
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!("step {step} extres err: {err}"));
                    }
                }
            }
        }

        // 3. Timeout OR thread-triggered cancel()
        if use_timeout {
            match ev.evaluate_output_text(&ModuleSource::text(slow_src)) {
                Err(err) if err.kind() == PklErrorKind::Timeout => assertions_passed += 1,
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!(
                            "step {step} expected Timeout, got {:?}: {err}",
                            err.kind()
                        ));
                    }
                }
                Ok(_) => {
                    if failures.len() < 16 {
                        failures.push(format!("step {step} slow eval unexpectedly succeeded"));
                    }
                }
            }
        } else {
            let cancel_handle = ev.cancel_handle();
            let t = thread::spawn(move || {
                thread::sleep(Duration::from_millis(8));
                cancel_handle.cancel();
            });
            let res = ev.evaluate_output_text(&ModuleSource::text(slow_src));
            let _ = t.join();
            match res {
                Err(err) if err.kind() == PklErrorKind::Cancelled => assertions_passed += 1,
                Err(err) => {
                    if failures.len() < 16 {
                        failures.push(format!(
                            "step {step} expected Cancelled, got {:?}: {err}",
                            err.kind()
                        ));
                    }
                }
                Ok(_) => {
                    if failures.len() < 16 {
                        failures.push(format!("step {step} cancelled eval unexpectedly succeeded"));
                    }
                }
            }
        }

        // 4. Idempotent close + post-close Closed assertion
        ev.close();
        ev.close();
        if !ev.is_closed() {
            if failures.len() < 16 {
                failures.push(format!("step {step}: evaluator not marked closed"));
            }
        } else {
            match ev.evaluate_output_text(&ModuleSource::text("x = 1\n")) {
                Err(err) if err.kind() == PklErrorKind::Closed => assertions_passed += 1,
                other => {
                    if failures.len() < 16 {
                        failures.push(format!(
                            "step {step}: post-close expected Closed, got {other:?}"
                        ));
                    }
                }
            }
        }
        drop(ev);

        if step + 1 == cfg.warmup {
            emit_sample("warmup-end", 0, start, cfg.sync_stdin, None);
        } else if step >= cfg.warmup {
            completed += 1;
            if completed == cfg.iterations {
                emit_sample("final", completed, start, cfg.sync_stdin, None);
            } else if completed.is_multiple_of(cfg.sample_interval) {
                emit_sample("sample", completed, start, cfg.sync_stdin, None);
            }
        }
    }

    (completed, assertions_passed, failures)
}

struct PoolEntry {
    generation: usize,
    meta_sha256: String,
    meta_json_path: PathBuf,
    zip_path: PathBuf,
    mp_jar_path: PathBuf,
}

fn load_pkg_pool(pool_dir: &Path) -> Vec<PoolEntry> {
    let index_path = pool_dir.join("pool.tsv");
    let Ok(text) = std::fs::read_to_string(index_path) else {
        return Vec::new();
    };
    let mut entries = Vec::new();
    for line in text.lines() {
        let parts: Vec<&str> = line.split('\t').collect();
        if parts.len() == 5
            && let Ok(generation) = parts[0].parse::<usize>()
        {
            entries.push(PoolEntry {
                generation,
                meta_sha256: parts[1].to_string(),
                meta_json_path: pool_dir.join(parts[2]),
                zip_path: pool_dir.join(parts[3]),
                mp_jar_path: pool_dir.join(parts[4]),
            });
        }
    }
    entries
}

struct DynamicChurnKey {
    uri: String,
    version: Arc<AtomicUsize>,
    cached: bool,
}

impl ModuleKey for DynamicChurnKey {
    fn uri(&self) -> &str {
        &self.uri
    }

    fn has_hierarchical_uris(&self) -> bool {
        true
    }

    fn is_cached(&self) -> bool {
        self.cached
    }

    fn resolve(&self, _security_manager: &dyn SecurityManager) -> pkl::Result<ResolvedModuleKey> {
        let v = self.version.load(Ordering::SeqCst);
        let pad = make_padding(v, 30);
        let src = format!("// churn gen {v}: {pad}\ngen = {v}\n");
        Ok(ResolvedModuleKey::virtual_module(self.uri.clone(), src))
    }
}

fn run_cache_churn(cfg: &Config, start: Instant) -> (usize, usize, Vec<String>) {
    let mut failures = Vec::new();
    let mut assertions_passed = 0usize;

    let pool = cfg
        .pkg_pool_dir
        .as_deref()
        .map(load_pkg_pool)
        .unwrap_or_default();

    let churn_dir = cfg.work_dir.join("cache_churn");
    let active_cache_dir = churn_dir.join("active_cache");
    let active_mod_dir = churn_dir.join("mods");
    if let Err(err) = std::fs::create_dir_all(&active_mod_dir) {
        failures.push(format!("mkdir cache_churn failed: {err}"));
        return (0, 0, failures);
    }

    // Persistent evaluator with uncached custom reader that reloads changing content on same URI
    let live_ver = Arc::new(AtomicUsize::new(0));
    let live_ver_factory = Arc::clone(&live_ver);
    let factory = ModuleKeyFactories::from_fn(move |uri: &str| {
        if uri == "churn:///live.pkl" {
            Some(Box::new(DynamicChurnKey {
                uri: uri.to_string(),
                version: Arc::clone(&live_ver_factory),
                cached: false,
            }))
        } else {
            None
        }
    });
    let mut uncached_ev = match make_base_builder().add_module_key_factory(factory).build() {
        Ok(ev) => ev,
        Err(err) => {
            failures.push(format!("build uncached evaluator failed: {err}"));
            return (0, 0, failures);
        }
    };

    let total_steps = cfg.warmup + cfg.iterations;
    let mut completed = 0usize;

    for step in 0..total_steps {
        // 1. Invalidate & reload uncached custom reader on persistent evaluator
        live_ver.store(step, Ordering::SeqCst);
        match uncached_ev.evaluate_expression_string(&ModuleSource::uri("churn:///live.pkl"), "gen")
        {
            Ok(out) if out == format!("{step}") => assertions_passed += 1,
            Ok(out) => {
                if failures.len() < 16 {
                    failures.push(format!(
                        "step {step} uncached reload mismatch: expected {step}, got {out}"
                    ));
                }
            }
            Err(err) => {
                if failures.len() < 16 {
                    failures.push(format!("step {step} uncached reload error: {err}"));
                }
            }
        }

        // 2. Disk module + package-2 cache + modulepath JAR load, cache hit, invalidate & reload
        let _ = std::fs::remove_dir_all(&active_cache_dir);
        let dep_path = active_mod_dir.join("dep.pkl");
        let main_path = active_mod_dir.join("main.pkl");
        let pad = make_padding(step, 24);
        let _ = std::fs::write(&dep_path, format!("// {pad}\nlocalGen = {step}\n"));

        let mut builder = make_base_builder();
        let expected_expr_val: String;

        if !pool.is_empty() {
            let entry = &pool[step % pool.len()];
            let pkg_dir = active_cache_dir
                .join("package-2")
                .join("soak.local")
                .join("soakpkg@1.0.0");
            if let Err(err) = std::fs::create_dir_all(&pkg_dir) {
                if failures.len() < 16 {
                    failures.push(format!("step {step} mkdir pkg cache failed: {err}"));
                }
                continue;
            }
            let _ = std::fs::copy(&entry.meta_json_path, pkg_dir.join("soakpkg@1.0.0.json"));
            let _ = std::fs::copy(&entry.zip_path, pkg_dir.join("soakpkg@1.0.0.zip"));
            let active_jar = churn_dir.join("active_mp.jar");
            let _ = std::fs::copy(&entry.mp_jar_path, &active_jar);

            let pkg_uri = format!(
                "package://soak.local/soakpkg@1.0.0::sha256:{}#/lib/info.pkl",
                entry.meta_sha256
            );
            let main_src = format!(
                "import \"dep.pkl\"\nimport \"modulepath:/mp.pkl\"\nimport \"{pkg_uri}\" as pkg\ncombined = \"\\(dep.localGen):\\(mp.mpGen):\\(pkg.pkgGen)\"\n"
            );
            let _ = std::fs::write(&main_path, main_src);
            builder = builder
                .with_module_cache_dir(Some(active_cache_dir.clone()))
                .with_module_paths(vec![active_jar]);
            expected_expr_val = format!("{step}:{}:{}", entry.generation, entry.generation);
        } else {
            let main_src = "import \"dep.pkl\"\ncombined = \"\\(dep.localGen)\"\n";
            let _ = std::fs::write(&main_path, main_src);
            expected_expr_val = format!("{step}");
        }

        match builder.build() {
            Ok(mut ev) => {
                let main_src = ModuleSource::path(&main_path);
                // Cold cache load + warm cache hit on the same evaluator
                for pass in 0..2 {
                    match ev.evaluate_expression_string(&main_src, "combined") {
                        Ok(out) if out == expected_expr_val => assertions_passed += 1,
                        Ok(out) => {
                            if failures.len() < 16 {
                                failures.push(format!(
                                    "step {step} pass {pass} cache-churn mismatch: expected {expected_expr_val}, got {out}"
                                ));
                            }
                        }
                        Err(err) => {
                            if failures.len() < 16 {
                                failures.push(format!(
                                    "step {step} pass {pass} cache-churn error: {err}"
                                ));
                            }
                        }
                    }
                }
                ev.close();
            }
            Err(err) => {
                if failures.len() < 16 {
                    failures.push(format!("step {step} cache-churn build err: {err}"));
                }
            }
        }

        if step + 1 == cfg.warmup {
            emit_sample("warmup-end", 0, start, cfg.sync_stdin, None);
        } else if step >= cfg.warmup {
            completed += 1;
            if completed == cfg.iterations {
                emit_sample("final", completed, start, cfg.sync_stdin, None);
            } else if completed.is_multiple_of(cfg.sample_interval) {
                emit_sample("sample", completed, start, cfg.sync_stdin, None);
            }
        }
    }

    uncached_ev.close();
    let _ = std::fs::remove_dir_all(&active_cache_dir);
    (completed, assertions_passed, failures)
}

fn main() -> ExitCode {
    let cfg = match parse_args() {
        Ok(c) => c,
        Err(err) => {
            eprintln!("m7_memory_soak_probe: {err}");
            return ExitCode::from(2);
        }
    };
    if let Err(err) = std::fs::create_dir_all(&cfg.work_dir) {
        eprintln!("m7_memory_soak_probe: failed to create work dir: {err}");
        return ExitCode::from(2);
    }

    let start = Instant::now();
    let (completed, assertions_passed, failures, heap_probe) = match cfg.scenario.as_str() {
        "repeated-success" => {
            let (c, a, f) = run_repeated_success(&cfg, start);
            (c, a, f, None)
        }
        "repeated-failure" => {
            let (c, a, f) = run_repeated_failure(&cfg, start);
            (c, a, f, None)
        }
        "unique-sources-churn" => {
            let (c, a, f) = run_unique_sources_churn(&cfg, start);
            (c, a, f, None)
        }
        "cyclic-graphs" => run_cyclic_graphs(&cfg, start),
        "lifecycle-cancel-timeout-close" => {
            let (c, a, f) = run_lifecycle_cancel_timeout_close(&cfg, start);
            (c, a, f, None)
        }
        "cache-churn" => {
            let (c, a, f) = run_cache_churn(&cfg, start);
            (c, a, f, None)
        }
        other => {
            eprintln!("m7_memory_soak_probe: unknown scenario {other:?}");
            return ExitCode::from(2);
        }
    };

    emit_summary(
        &cfg.scenario,
        cfg.warmup,
        completed,
        assertions_passed,
        &failures,
        start.elapsed().as_millis(),
        heap_probe.as_ref(),
    );

    if failures.is_empty() && completed == cfg.iterations {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
