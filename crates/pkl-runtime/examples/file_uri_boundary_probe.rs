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
    let peer_hex = arguments.next();
    assert!(
        arguments.next().is_none(),
        "one input and optional equality peer"
    );
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
            println!("path_scalar_present\t{}", path.to_str().is_some());
            #[cfg(unix)]
            {
                use std::os::unix::ffi::OsStrExt;
                // Display decoding mirrors UnixPath.toString for the pinned
                // UTF-8/default-normalization profile; the PathBuf keeps bytes.
                let display = pkl_runtime::resources::decode_utf8(path.as_os_str().as_bytes());
                text("path", display.as_str().expect("decoder emits scalars"));
            }
            #[cfg(not(unix))]
            text("path", path.to_str().expect("non-Unix scalar boundary"));
            if let Some(peer_hex) = peer_hex {
                assert!(peer_hex.is_ascii() && peer_hex.len().is_multiple_of(4));
                let peer = PklString::from_utf16(
                    (0..peer_hex.len())
                        .step_by(4)
                        .map(|i| {
                            u16::from_str_radix(&peer_hex[i..i + 4], 16).expect("peer hex unit")
                        })
                        .collect::<Vec<_>>(),
                );
                match module_host::file_path(peer) {
                    Ok(peer) => println!("peer_equal\t{}", path.as_os_str() == peer.as_os_str()),
                    Err(error) => text("peer_error_debug", &format!("{error:?}")),
                }
            }
            println!("status\tvalue");
        }
        Err(error) => {
            println!("status\terror");
            text("error_debug", &format!("{error:?}"));
        }
    }
}
