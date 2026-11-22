//! Development-only timing probe mirroring `tools/java/Benchmark.java` for Milestone M7.
//!
//! Measures `initialize`, `first-request`, converged `warmup` windows, and `measurement`
//! windows for both `persistent` and `fresh-evaluator` modes using the public `pkl` crate,
//! verifying 100% raw-byte output/error parity against the Java preflight outside the timed
//! region on every single iteration.

use pkl::{Evaluator, EvaluatorBuilder, ModuleSource};
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::Instant;

struct EvalOutput {
    bytes: Vec<u8>,
    error: bool,
}

fn abs_normalize(path: &Path) -> PathBuf {
    let abs = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    };
    let mut out = PathBuf::new();
    for comp in abs.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

fn build_evaluator() -> Result<Evaluator, String> {
    EvaluatorBuilder::preconfigured()
        .with_module_cache_dir(None)
        .with_color(false)
        .with_environment_variables(Vec::new())
        .with_external_properties(Vec::new())
        .build()
        .map_err(|err| format!("failed to build evaluator: {err}"))
}

fn call_eval(evaluator: &mut Evaluator, source: &ModuleSource, expr: &str) -> EvalOutput {
    let result = if expr == "output.text" {
        evaluator.evaluate_output_text(source)
    } else {
        evaluator.evaluate_expression_string(source, expr)
    };
    match result {
        Ok(text) => EvalOutput {
            bytes: text.into_bytes(),
            error: false,
        },
        Err(err) => EvalOutput {
            bytes: err.message().as_bytes().to_vec(),
            error: true,
        },
    }
}

fn invoke(
    mode: &str,
    persistent: Option<&mut Evaluator>,
    source: &ModuleSource,
    expr: &str,
) -> Result<EvalOutput, String> {
    if mode == "fresh-evaluator" {
        let mut fresh = build_evaluator()?;
        let out = call_eval(&mut fresh, source, expr);
        fresh.close();
        Ok(out)
    } else {
        let ev = persistent.ok_or_else(|| "missing persistent evaluator".to_string())?;
        Ok(call_eval(ev, source, expr))
    }
}

fn check_output(result: &EvalOutput, expected: &[u8], expected_error: bool) -> Result<(), String> {
    if result.error != expected_error || result.bytes != expected {
        return Err(format!(
            "output bytes/outcome differ from the verified Java preflight (error={} expected_error={}, bytes={} expected_bytes={})",
            result.error,
            expected_error,
            result.bytes.len(),
            expected.len()
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn run_window(
    mode: &str,
    mut persistent: Option<&mut Evaluator>,
    source: &ModuleSource,
    expr: &str,
    expected: &[u8],
    expected_error: bool,
    minimum_nanos: u128,
    phase: &str,
    index: usize,
) -> Result<f64, String> {
    let mut elapsed: u128 = 0;
    let mut iterations: u64 = 0;
    // Correctness checks occur outside the timed operation. Each window requires
    // at least the configured amount of measured operation time, not check time.
    while elapsed < minimum_nanos {
        let before = Instant::now();
        let result = invoke(mode, persistent.as_deref_mut(), source, expr)?;
        elapsed += before.elapsed().as_nanos();
        check_output(&result, expected, expected_error)?;
        iterations += 1;
    }
    let per_operation = (elapsed as f64) / (iterations as f64);
    println!(
        "{{\"phase\":\"{phase}\",\"index\":{index},\"elapsed_ns\":{elapsed},\"iterations\":{iterations},\"ns_per_op\":{per_operation}}}"
    );
    Ok(per_operation)
}

fn stable(values: &[f64], windows: usize, tolerance: f64) -> bool {
    if values.len() < windows {
        return false;
    }
    let mut recent: Vec<f64> = values[values.len() - windows..].to_vec();
    recent.sort_by(|a, b| a.total_cmp(b));
    let median = (recent[windows / 2 - 1] + recent[windows / 2]) / 2.0;
    median > 0.0 && (recent[windows - 1] - recent[0]) / median <= tolerance
}

fn parse_bool(raw: &str, name: &str) -> Result<bool, String> {
    match raw {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid boolean for {name}: {raw}")),
    }
}

fn run(args: &[String]) -> Result<(), String> {
    if args.len() != 13 && args.len() != 14 {
        return Err(
            "expected 12 or 13 harness arguments: <mode> <module> <expected> <expected_error> <min_warmup_s> <max_warmup_s> <samples> <sample_s> <convergence_windows> <max_rel_spread> <first_output> <write_expected> [expr]"
                .to_string(),
        );
    }
    let mode = args[1].as_str();
    if !matches!(mode, "verify" | "persistent" | "fresh-evaluator") {
        return Err(format!("unknown benchmark mode: {mode}"));
    }
    let module_path = abs_normalize(Path::new(&args[2]));
    let source = ModuleSource::path(&module_path);
    let expected_path = PathBuf::from(&args[3]);
    let expected_error = parse_bool(&args[4], "expected_error")?;
    let min_warmup_secs: f64 = args[5]
        .parse()
        .map_err(|_| format!("invalid min_warmup_s: {}", args[5]))?;
    let max_warmup_secs: f64 = args[6]
        .parse()
        .map_err(|_| format!("invalid max_warmup_s: {}", args[6]))?;
    let samples: usize = args[7]
        .parse()
        .map_err(|_| format!("invalid samples: {}", args[7]))?;
    let sample_secs: f64 = args[8]
        .parse()
        .map_err(|_| format!("invalid sample_s: {}", args[8]))?;
    let convergence_windows: usize = args[9]
        .parse()
        .map_err(|_| format!("invalid convergence_windows: {}", args[9]))?;
    let tolerance: f64 = args[10]
        .parse()
        .map_err(|_| format!("invalid max_rel_spread: {}", args[10]))?;
    let first_output = PathBuf::from(&args[11]);
    let write_expected = parse_bool(&args[12], "write_expected")?;
    let expr = if args.len() == 14 {
        args[13].as_str()
    } else {
        "run(0)"
    };

    if !min_warmup_secs.is_finite()
        || !max_warmup_secs.is_finite()
        || !sample_secs.is_finite()
        || !tolerance.is_finite()
    {
        return Err("non-finite sampling parameter".to_string());
    }
    let minimum_warmup = (min_warmup_secs * 1e9) as u128;
    let maximum_warmup = (max_warmup_secs * 1e9) as u128;
    let sample_time = (sample_secs * 1e9) as u128;

    if mode != "verify"
        && (minimum_warmup < 250_000_000
            || maximum_warmup < minimum_warmup
            || samples < 3
            || sample_time < 50_000_000
            || convergence_windows < 4
            || !convergence_windows.is_multiple_of(2)
            || tolerance <= 0.0
            || tolerance > 0.5)
    {
        return Err("insufficient warmup/sampling contract".to_string());
    }

    let before_init = Instant::now();
    let mut owned: Option<Evaluator> = None;
    if mode == "fresh-evaluator" {
        let mut initial = build_evaluator()?;
        initial.close();
    } else {
        owned = Some(build_evaluator()?);
    }
    let init_ns = before_init.elapsed().as_nanos();
    println!("{{\"phase\":\"initialize\",\"elapsed_ns\":{init_ns}}}");

    let before_first = Instant::now();
    let first = invoke(mode, owned.as_mut(), &source, expr)?;
    let first_ns = before_first.elapsed().as_nanos();
    if let Some(parent) = first_output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .map_err(|err| format!("failed to create first_output dir: {err}"))?;
    }
    fs::write(&first_output, &first.bytes)
        .map_err(|err| format!("failed to write first_output: {err}"))?;
    if first.error != expected_error {
        return Err(format!(
            "unexpected preflight outcome: error={} expected_error={}",
            first.error, expected_error
        ));
    }
    if write_expected {
        if let Some(parent) = expected_path.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create expected_path dir: {err}"))?;
        }
        fs::write(&expected_path, &first.bytes)
            .map_err(|err| format!("failed to write expected_path: {err}"))?;
    }
    let expected = fs::read(&expected_path).map_err(|err| {
        format!(
            "failed to read expected_path {}: {err}",
            expected_path.display()
        )
    })?;
    check_output(&first, &expected, expected_error)?;
    println!(
        "{{\"phase\":\"first-request\",\"elapsed_ns\":{first_ns},\"error\":{},\"bytes\":{}}}",
        first.error,
        first.bytes.len()
    );

    if mode == "verify" {
        if let Some(mut ev) = owned {
            ev.close();
        }
        let _ = std::io::stdout().flush();
        return Ok(());
    }

    let mut warmup: Vec<f64> = Vec::new();
    let warmup_started = Instant::now();
    let mut elapsed: u128;
    let mut converged: bool;
    loop {
        let ns_per_op = run_window(
            mode,
            owned.as_mut(),
            &source,
            expr,
            &expected,
            expected_error,
            sample_time,
            "warmup",
            warmup.len(),
        )?;
        warmup.push(ns_per_op);
        elapsed = warmup_started.elapsed().as_nanos();
        converged = elapsed >= minimum_warmup && stable(&warmup, convergence_windows, tolerance);
        if converged || elapsed >= maximum_warmup {
            break;
        }
    }
    println!(
        "{{\"phase\":\"warmup-result\",\"elapsed_ns\":{elapsed},\"converged\":{converged},\"windows\":{}}}",
        warmup.len()
    );

    for index in 0..samples {
        run_window(
            mode,
            owned.as_mut(),
            &source,
            expr,
            &expected,
            expected_error,
            sample_time,
            "measurement",
            index,
        )?;
    }

    if let Some(mut ev) = owned {
        ev.close();
    }
    let _ = std::io::stdout().flush();
    Ok(())
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    match run(&args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(msg) => {
            let mut err = std::io::stderr().lock();
            let _ = writeln!(err, "m7_bench_probe: {msg}");
            ExitCode::FAILURE
        }
    }
}
