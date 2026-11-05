//! Development-only PackageUri capture. Java supplies the reference observations.
use pkl_runtime::modules::{
    ModuleCompletion, ModuleKey, ModuleKeyKind, ModuleRequestKind, SelectionPhase,
    default_completion, standard_key_completion,
};
use pkl_runtime::packages::{PackageAssetUri, PackageUri, PackageUriError, PackageUriFailure};
use pkl_runtime::probe::SourceProbe;
use pkl_runtime::value::PklString;
use std::fmt::Write;

fn bytes_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(out, "{byte:02x}").unwrap();
    }
    out
}
fn text(name: &str, value: &PklString) {
    println!(
        "{name}_utf8\t{}",
        bytes_hex(value.to_java_utf8().as_bytes())
    );
    let mut units = String::new();
    for unit in value.encode_utf16() {
        write!(units, "{unit:04x}").unwrap();
    }
    println!("{name}_utf16\t{units}");
}
fn input(hex: &str) -> PklString {
    assert!(
        hex.is_ascii() && hex.len().is_multiple_of(4),
        "UTF16 hex units required"
    );
    PklString::from_utf16(
        (0..hex.len())
            .step_by(4)
            .map(|i| u16::from_str_radix(&hex[i..i + 4], 16).expect("UTF16 hex unit"))
            .collect::<Vec<_>>(),
    )
}
fn optional(name: &str, value: Option<&PklString>) {
    println!("{name}_present\t{}", value.is_some());
    if let Some(value) = value {
        text(name, value);
    }
}
fn package_value(uri: &PackageUri) {
    text("package_uri", &uri.uri);
    text("path_without_version", &uri.path_without_version);
    let v = &uri.version;
    let mut version = PklString::from(format!("{}.{}.{}", v.major, v.minor, v.patch));
    if let Some(pre) = &v.pre_release {
        version = version.concat(&"-".into()).concat(pre);
    }
    if let Some(build) = &v.build {
        version = version.concat(&"+".into()).concat(build);
    }
    text("version", &version);
    optional("prerelease", v.pre_release.as_ref());
    optional("build", v.build.as_ref());
    optional("sha256", uri.sha256.as_ref());
}
fn constructor_error(error: PackageUriError) {
    let status = if matches!(&error.failure, PackageUriFailure::Unsupported(_)) {
        "unsupported"
    } else {
        "error"
    };
    println!("status\t{status}");
    text("native_error_kind", &format!("{:?}", error.failure).into());
    text("reason", &error.reason());
    text("error_input", &error.input);
    // The public typed API does not expose a Java Throwable class/stack or a
    // complete URISyntaxException message/index. Do not manufacture those fields.
}
fn native_event(events: &mut usize, kind: &str, details: impl Into<PklString>) {
    text(&format!("native_event_{}_kind", *events), &kind.into());
    text(
        &format!("native_event_{}_details", *events),
        &details.into(),
    );
    *events += 1;
}
fn api(mode: &str, payload: PklString, primary: PklString) {
    let mut events = 0;
    if matches!(mode, "module-deny" | "read-deny" | "import-deny") {
        println!("status\tunsupported");
        text(
            "native_error_kind",
            &"Configurable security profile is not implemented by this native boundary capture."
                .into(),
        );
        println!("native_event_count\t0");
        println!("transport_boundary_reached\tfalse");
        return;
    }
    assert!(matches!(mode, "module-default" | "read-default"));
    let mut probe = SourceProbe::new_module_evaluator("compact", true, false, false).unwrap();
    probe
        .evaluate_module_source_text(primary.clone(), Some(payload.into()), "res", 0, None)
        .unwrap();
    let mut primary_selected = false;
    let mut terminal = false;
    for _ in 0..2_000_000 {
        let event = probe.poll_text(0, 1024).unwrap();
        probe.collect();
        match event.kind {
            "yield" => {}
            "module-request" => {
                let token = event.token.as_deref().expect("module token");
                let request = probe.module_request(token).unwrap();
                native_event(&mut events, "module-request", format!("{:?}", request.kind));
                let completion = match &request.kind {
                    ModuleRequestKind::SelectKey {
                        id,
                        uri,
                        phase: SelectionPhase::Primary,
                    } if *uri == primary && !primary_selected => {
                        primary_selected = true;
                        Some(ModuleCompletion::KeySelected(Ok(ModuleKey {
                            id: *id,
                            uri: uri.clone(),
                            kind: ModuleKeyKind::Host,
                            handle: "package-boundary-primary".into(),
                        })))
                    }
                    ModuleRequestKind::SelectKey {
                        phase: SelectionPhase::Primary,
                        ..
                    } => None,
                    _ => default_completion(&request).or_else(|| standard_key_completion(&request)),
                };
                let Some(completion) = completion else {
                    println!("status\tunsupported");
                    text(
                        "native_error_kind",
                        &"Unexpected native boundary Host callback.".into(),
                    );
                    terminal = true;
                    break;
                };
                probe.complete_module(token, completion, 0).unwrap();
            }
            "read" => {
                let token = event.token.as_deref().expect("read token");
                let request = probe.read_request(token).unwrap();
                native_event(&mut events, "read", format!("{request:?}"));
                probe.respond_configured(token, 0).unwrap();
            }
            "value" => {
                text(
                    "result",
                    &PklString::from_utf16(event.value.expect("value units")),
                );
                println!("status\tvalue");
                terminal = true;
                break;
            }
            "error" => {
                let status = if event.disposition == Some("unsupported") {
                    "unsupported"
                } else {
                    "error"
                };
                println!("status\t{status}");
                text(
                    "native_disposition",
                    &event.disposition.expect("failure disposition").into(),
                );
                text(
                    "error_message",
                    &PklString::from_utf16(event.error.expect("error units")),
                );
                terminal = true;
                break;
            }
            other => {
                native_event(&mut events, "unexpected-event", other);
                println!("status\tunsupported");
                terminal = true;
                break;
            }
        }
    }
    if !terminal {
        println!("status\tunsupported");
        text(
            "native_error_kind",
            &"Native capture poll bound reached.".into(),
        );
    }
    probe.close();
    println!("native_event_count\t{events}");
    // Native typed Host callbacks are not labeled as Java SecurityManager events.
    // Any external I/O callback is rejected above; no network/file Host is linked.
    println!("transport_boundary_reached\tfalse");
}
fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    assert!(
        (2..=3).contains(&args.len()),
        "mode, UTF16 payload, optional primary URI"
    );
    let mode = &args[0];
    let value = input(&args[1]);
    text("input", &value);
    text("mode", &mode.as_str().into());
    match mode.as_str() {
        "package" => {
            match PackageUri::parse(value) {
                Ok(uri) => {
                    package_value(&uri);
                    println!("status\tvalue");
                }
                Err(error) => constructor_error(error),
            }
            println!("event_count\t0");
            println!("transport_boundary_reached\tfalse");
        }
        "asset" => {
            match PackageAssetUri::parse(value) {
                Ok(uri) => {
                    package_value(&uri.package);
                    text("asset_uri", &uri.uri);
                    text("asset_path", &uri.asset_path);
                    println!("status\tvalue");
                }
                Err(error) => constructor_error(error),
            }
            println!("event_count\t0");
            println!("transport_boundary_reached\tfalse");
        }
        _ => {
            let primary = args
                .get(2)
                .map(|value| input(value))
                .unwrap_or_else(|| "file:///stage155/control.pkl".into());
            api(mode, value, primary);
        }
    }
}
