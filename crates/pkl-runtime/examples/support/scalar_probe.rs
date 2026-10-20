//! Filesystem adapter for the shared development scalar evaluator.
use std::path::Path;

pub fn main() {
    match run() {
        Ok(value) => print!("{value}"),
        Err(error) => {
            eprint!("{error}");
            std::process::exit(1);
        }
    }
}

fn run() -> Result<String, String> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("expected source path and expression".into());
    }
    let path = std::fs::canonicalize(&args[1]).map_err(|error| error.to_string())?;
    let text = std::fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let uri = file_uri(&path)?;
    let name = path
        .file_stem()
        .and_then(|name| name.to_str())
        .ok_or("source path must be UTF-8")?;
    pkl_runtime::evaluation::scalar_expression(&text, &uri, name, &args[2])
}

fn file_uri(path: &Path) -> Result<String, String> {
    use std::fmt::Write;
    let mut uri = String::from("file://");
    for byte in path.to_str().ok_or("source path must be UTF-8")?.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            uri.push(char::from(byte));
        } else {
            write!(uri, "%{byte:02X}").unwrap();
        }
    }
    Ok(uri)
}
