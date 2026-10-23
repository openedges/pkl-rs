//! Development-only bounded arithmetic workload. Every timed call recomputes it.
use std::hint::black_box;
use std::sync::Arc;
use std::time::{Duration, Instant};

use pkl_runtime::value::Value;
use pkl_runtime::vm::{Instruction, Program, Vm, VmPoll};

fn call(vm: &mut Vm, seed: i64) -> String {
    vm.start(0, vec![Value::Int(black_box(seed.into()))])
        .unwrap();
    match vm.poll(1024) {
        VmPoll::Ready(Ok(Value::Int(value))) => black_box(value).to_string(),
        other => panic!("unexpected bounded workload result: {other:?}"),
    }
}

fn window(vm: &mut Vm, duration: Duration, phase: &str, index: usize) -> Duration {
    let mut iterations = 0_u64;
    let mut elapsed = Duration::ZERO;
    while elapsed < duration {
        let seed = (iterations % 256) as i64;
        let before = Instant::now();
        let result = call(vm, seed);
        elapsed += before.elapsed();
        // Exact returned UTF-8 bytes, outside the timed evaluation+conversion.
        assert_eq!(result.as_bytes(), (seed + 8256).to_string().as_bytes());
        iterations += 1;
    }
    println!(
        "{{\"phase\":\"{phase}\",\"index\":{index},\"iterations\":{iterations},\"elapsed_ns\":{},\"ns_per_op\":{}}}",
        elapsed.as_nanos(),
        elapsed.as_nanos() as f64 / iterations as f64
    );
    elapsed
}

fn main() {
    let arguments: Vec<_> = std::env::args().collect();
    assert_eq!(
        arguments.len(),
        4,
        "expected warmup seconds, sample count, and dispatch variant"
    );
    let warmup: u64 = arguments[1].parse().unwrap();
    let samples: usize = arguments[2].parse().unwrap();
    let specialized = match arguments[3].as_str() {
        "specialized" => true,
        "generic" => false,
        _ => panic!("unknown dispatch variant"),
    };
    assert!(warmup >= 10 && samples >= 5);
    let mut block = vec![Instruction::Input(0)];
    for value in 1..=128 {
        if specialized {
            block.push(Instruction::AddIntConstant(value));
        } else {
            block.extend([
                Instruction::Constant(Value::Int(value.into())),
                Instruction::Add,
            ]);
        }
    }
    block.push(Instruction::Return);
    let mut vm = Vm::new(Arc::new(Program::new(vec![block])));
    // Validate all varying inputs before collecting any measurements.
    for seed in 0..256 {
        assert_eq!(call(&mut vm, seed), (seed + 8256).to_string());
    }
    println!(
        "{{\"phase\":\"preflight\",\"seeds\":256,\"instructions_per_evaluation\":{},\"value_bytes\":{},\"instruction_bytes\":{}}}",
        if specialized { 130 } else { 258 },
        std::mem::size_of::<Value>(),
        std::mem::size_of::<Instruction>()
    );
    let mut measured_warmup = Duration::ZERO;
    let mut index = 0;
    while measured_warmup < Duration::from_secs(warmup) {
        measured_warmup += window(&mut vm, Duration::from_millis(100), "warmup", index);
        index += 1;
    }
    for index in 0..samples {
        window(&mut vm, Duration::from_millis(250), "measurement", index);
    }
}
