//! Raw primitive probe transport shared with the pinned Java experiment.
use std::io::{self, BufRead, Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let stdin = io::stdin();
    let mut stdout = io::BufWriter::new(io::stdout().lock());
    for line in stdin.lock().lines() {
        let line = line?;
        let (family, input) = line.split_once('\t').ok_or("missing probe family")?;
        let output = match family {
            "numeric" => pkl_runtime::numeric::probe(input),
            "regex" => pkl_runtime::regex::probe(input),
            _ => return Err(format!("unknown probe family: {family}").into()),
        }?;
        for byte in output.as_bytes() {
            write!(stdout, "{byte:02x}")?;
        }
        writeln!(stdout)?;
    }
    stdout.flush()?;
    Ok(())
}
