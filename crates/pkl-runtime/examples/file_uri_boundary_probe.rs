//! Development-only URI-to-Path capture; no guest VM or filesystem request.
use pkl_runtime::value::PklString;
use std::fmt::Write;

#[path = "support/module_host.rs"]
mod module_host;

fn bytes_hex(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(output, "{byte:02x}").unwrap();
    }
    output
}
fn text(key: &str, value: &str) {
    println!("{key}_utf8\t{}", bytes_hex(value.as_bytes()));
    let mut units = String::new();
    for unit in value.encode_utf16() {
        write!(units, "{unit:04x}").unwrap();
    }
    println!("{key}_utf16\t{units}");
}
fn main() {
    // Retain the existing development Host module without a dead-code warning;
    // the capture does not call complete() or perform any filesystem operation.
    let _complete: fn(
        &pkl_runtime::modules::ModuleRequest,
    ) -> pkl_runtime::modules::ModuleCompletion = module_host::complete;
    let mut arguments = std::env::args().skip(1);
    let hex = arguments.next().expect("one UTF16 hex input required");
    assert!(arguments.next().is_none(), "one input only");
    assert!(
        hex.is_ascii() && hex.len().is_multiple_of(4),
        "UTF16 hex unit length"
    );
    let units: Vec<u16> = (0..hex.len())
        .step_by(4)
        .map(|i| u16::from_str_radix(&hex[i..i + 4], 16).expect("hex input unit"))
        .collect();
    let input = PklString::from_utf16(units);
    // The finite corpus contains only scalar strings. Any future unpaired-unit
    // control needs its own raw UTF8 encoding policy, not lossy normalization.
    let scalar = input
        .as_str()
        .expect("this finite corpus uses scalar inputs");
    text("input", scalar);
    match module_host::file_path(input.clone()) {
        Ok(path) => {
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                println!("path_os_bytes\t{}", bytes_hex(path.as_os_str().as_bytes()));
            }
            text(
                "path",
                path.to_str()
                    .expect("modeled development Host returns UTF8"),
            );
            println!("status\tvalue");
        }
        Err(error) => {
            println!("status\terror");
            text("error_debug", &format!("{error:?}"));
        }
    }
}
