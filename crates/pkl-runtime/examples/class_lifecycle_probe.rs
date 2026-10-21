//! Development-only ordered calls against one retained VM and module.
use pkl_runtime::evaluation::{EvaluationOptions, scalar_expressions_with_options};
use std::{fs, path::Path};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 4, "source expressions output");
    let source = fs::read_to_string(&args[1]).expect("read source");
    let expressions: Vec<_> = fs::read_to_string(&args[2])
        .expect("read expressions")
        .lines()
        .map(str::to_owned)
        .collect();
    let out = Path::new(&args[3]);
    let results = match scalar_expressions_with_options(
        &source,
        "file:///m2-platform.pkl",
        "m2-platform",
        &expressions,
        EvaluationOptions::default(),
    ) {
        Ok(results) => results,
        Err(error) => {
            fs::write(out.join("setup.stderr.bin"), error).expect("write setup error");
            std::process::exit(2);
        }
    };
    for (index, result) in results.into_iter().enumerate() {
        let (status, stdout, stderr) = match result {
            Ok(value) => ("success\n", value, String::new()),
            Err(error) => ("error\n", String::new(), error),
        };
        for (channel, bytes) in [
            ("status", status.as_bytes()),
            ("stdout", stdout.as_bytes()),
            ("stderr", stderr.as_bytes()),
        ] {
            fs::write(out.join(format!("{index}.{channel}.bin")), bytes)
                .expect("write call capture");
        }
        fs::write(out.join(format!("{index}.error-class.bin")), b"").expect("write class capture");
        println!("call={index} completed");
    }
    println!("closed");
}
