//! Bounded M1 primitive costs; not an evaluator performance benchmark.
use pkl_runtime::numeric;
use std::{fmt::Write, hint::black_box, time::Instant};

const OPERATIONS: [&str; 4] = ["format", "cbrt", "sin", "checked_add"];

fn result(operation: &str, bits: u64) -> String {
    let value = f64::from_bits(bits);
    match operation {
        "format" => numeric::double_to_string(value),
        "checked_add" => match numeric::add(bits as i64, 17) {
            Ok(value) => value.to_string(),
            Err(_) => "integerOverflow".into(),
        },
        _ => {
            let value = numeric::math(operation, value).unwrap();
            let bits = if value.is_nan() {
                0x7ff8000000000000
            } else {
                value.to_bits()
            };
            format!("{bits:016x}")
        }
    }
}

fn batch(operation: &str, values: &[u64], repetitions: usize) -> u64 {
    let mut checksum = 0_u64;
    for _ in 0..repetitions {
        for &bits in values {
            let bits = black_box(bits);
            let contribution = match operation {
                "format" => {
                    let value = numeric::double_to_string(f64::from_bits(bits));
                    let mut hash = 0_u64;
                    for byte in value.bytes() {
                        hash = hash.wrapping_mul(31).wrapping_add(u64::from(byte));
                    }
                    black_box(hash)
                }
                "checked_add" => numeric::add(bits as i64, 17)
                    .map(|value| value as u64)
                    .unwrap_or(0xdeadbeef),
                _ => {
                    let value = numeric::math(operation, f64::from_bits(bits)).unwrap();
                    if value.is_nan() {
                        0x7ff8000000000000
                    } else {
                        value.to_bits()
                    }
                }
            };
            checksum = checksum.rotate_left(7).wrapping_add(contribution);
        }
    }
    black_box(checksum)
}

fn stable(samples: &[u128]) -> bool {
    if samples.len() < 12 {
        return false;
    }
    let values = &samples[samples.len() - 12..];
    fn median(values: &[u128]) -> f64 {
        let mut sorted = values.to_vec();
        sorted.sort_unstable();
        (sorted[sorted.len() / 2 - 1] as f64 + sorted[sorted.len() / 2] as f64) / 2.0
    }
    let first = median(&values[..4]);
    let last = median(&values[8..]);
    let center = median(values);
    let deviations: Vec<u128> = values
        .iter()
        .map(|value| (*value as f64 - center).abs() as u128)
        .collect();
    first > 0.0
        && center > 0.0
        && (last - first).abs() / first <= 0.10
        && median(&deviations) / center <= 0.10
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        6,
        "mode inputs repetitions warmups measurements"
    );
    let values: Vec<u64> = std::fs::read_to_string(&args[2])
        .unwrap()
        .lines()
        .map(|line| u64::from_str_radix(line, 16).unwrap())
        .collect();
    let repetitions = args[3].parse::<usize>().unwrap();
    let warmups = args[4].parse::<usize>().unwrap();
    let measurements = args[5].parse::<usize>().unwrap();
    assert!(!values.is_empty() && repetitions > 0 && warmups > 0 && measurements > 0);
    for operation in OPERATIONS {
        for &bits in &values {
            let mut encoded = String::new();
            for byte in result(operation, bits).bytes() {
                write!(encoded, "{byte:02x}").unwrap();
            }
            println!("result\t{operation}\t{bits:016x}\t{encoded}");
        }
    }
    if args[1] == "check" {
        return;
    }
    assert_eq!(args[1], "bench");
    let start_warmup = Instant::now();
    let mut windows: [Vec<u128>; 4] = std::array::from_fn(|_| Vec::new());
    let mut rounds = 0;
    let converged = loop {
        for (index, operation) in OPERATIONS.iter().enumerate() {
            let start = Instant::now();
            let checksum = batch(operation, &values, repetitions);
            let elapsed = start.elapsed().as_nanos();
            windows[index].push(elapsed);
            println!("sample\t{operation}\twarmup\t{rounds}\t{elapsed}\t{checksum:016x}");
        }
        rounds += 1;
        let elapsed = start_warmup.elapsed().as_secs_f64();
        if rounds >= warmups && elapsed >= 5.0 && windows.iter().all(|values| stable(values)) {
            break true;
        }
        if elapsed >= 20.0 {
            break false;
        }
    };
    let elapsed = start_warmup.elapsed().as_nanos();
    let status = if converged { "stable" } else { "inconclusive" };
    println!("warmup-status\t{status}\t{elapsed}\t{rounds}");
    for operation in OPERATIONS {
        for sample in 0..measurements {
            let start = Instant::now();
            let checksum = batch(operation, &values, repetitions);
            let elapsed = start.elapsed().as_nanos();
            println!("sample\t{operation}\tmeasurement\t{sample}\t{elapsed}\t{checksum:016x}");
        }
    }
}
