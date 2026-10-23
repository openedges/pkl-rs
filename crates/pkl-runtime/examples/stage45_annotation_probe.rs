//! Development raw bootstrap/ModuleInfo capture. All semantics remain in SourceProbe.
use pkl_runtime::{
    probe::SourceProbe,
    value::{PklString, Value},
};
use std::{fs, path::Path};

fn write(out: &Path, name: &str, value: impl AsRef<[u8]>) {
    fs::write(out.join(name), value).expect("write annotation capture");
}
fn save_text(out: &Path, prefix: &str, channel: &str, units: &[u16]) {
    write(
        out,
        &format!("{prefix}.{channel}.code-units.bin"),
        units
            .iter()
            .flat_map(|unit| unit.to_be_bytes())
            .collect::<Vec<_>>(),
    );
    write(
        out,
        &format!("{prefix}.{channel}.bin"),
        PklString::from_utf16(units.to_vec()).to_java_utf8(),
    );
}
fn capture(out: &Path, source: &str, uri: &str, name: &str, expressions: &[String], format: &str) {
    fs::create_dir_all(out).expect("create annotation session output");
    let mut probe = match SourceProbe::new_expressions_text(
        source,
        uri,
        name,
        expressions,
        "compact",
        false,
        false,
        true,
    ) {
        Ok(probe) => probe,
        Err(error) => {
            save_text(
                out,
                "constructor",
                "stderr",
                &error.encode_utf16().collect::<Vec<_>>(),
            );
            write(out, "constructor.status.bin", "constructor-error\n");
            return;
        }
    };
    let mut reads = 0usize;
    let mut collections = 0usize;
    for (index, expression) in expressions.iter().enumerate() {
        let prefix = format!("outer-{index}");
        write(out, &format!("{prefix}.expression.bin"), expression);
        write(
            out,
            &format!("{prefix}.reads-before.bin"),
            reads.to_string(),
        );
        save_text(out, &prefix, "stdout", &[]);
        save_text(out, &prefix, "stderr", &[]);
        if let Err(error) = probe.start_path(index as u32, 0, None) {
            write(out, &format!("{prefix}.status.bin"), "admission-error\n");
            write(
                out,
                &format!("{prefix}.error-kind.bin"),
                format!("{:?}", error.kind),
            );
            save_text(
                out,
                &prefix,
                "stderr",
                &error.message.encode_utf16().collect::<Vec<_>>(),
            );
        } else {
            let mut settled = false;
            for _ in 0..1_000_000 {
                let event = match probe.poll(0, 1) {
                    Ok(event) => event,
                    Err(error) => {
                        write(out, &format!("{prefix}.status.bin"), "poll-error\n");
                        write(
                            out,
                            &format!("{prefix}.error-kind.bin"),
                            format!("{:?}", error.kind),
                        );
                        save_text(
                            out,
                            &prefix,
                            "stderr",
                            &error.message.encode_utf16().collect::<Vec<_>>(),
                        );
                        settled = true;
                        break;
                    }
                };
                probe.collect();
                collections += 1;
                match event.kind {
                    "yield" => {}
                    "read" => {
                        reads += 1;
                        let token = event.token.expect("read token");
                        let requested = event.uri.expect("read URI");
                        write(out, &format!("read-{}.uri.bin", reads - 1), &requested);
                        if requested != "prop:pkl.outputFormat" {
                            write(out, &format!("{prefix}.status.bin"), "transport-error\n");
                            write(
                                out,
                                &format!("{prefix}.transport-error.bin"),
                                "unexpected resource URI",
                            );
                            settled = true;
                            break;
                        }
                        let value = if format == "absent" {
                            Value::Null
                        } else {
                            Value::String(format.into())
                        };
                        if let Err(error) = probe.respond(&token, value, 0) {
                            write(out, &format!("{prefix}.status.bin"), "transport-error\n");
                            save_text(
                                out,
                                &prefix,
                                "stderr",
                                &error.message.encode_utf16().collect::<Vec<_>>(),
                            );
                            settled = true;
                            break;
                        }
                    }
                    "value" | "error" => {
                        let (status, channel, units) = if event.kind == "value" {
                            ("success\n", "stdout", event.value.expect("value units"))
                        } else {
                            ("error\n", "stderr", event.error.expect("error units"))
                        };
                        write(out, &format!("{prefix}.status.bin"), status);
                        save_text(out, &prefix, channel, &units);
                        write(
                            out,
                            &format!("{prefix}.phase.bin"),
                            event.phase.unwrap_or(""),
                        );
                        write(
                            out,
                            &format!("{prefix}.disposition.bin"),
                            event.disposition.unwrap_or(""),
                        );
                        write(
                            out,
                            &format!("{prefix}.reason.bin"),
                            event.reason.unwrap_or_default(),
                        );
                        settled = true;
                        break;
                    }
                    other => {
                        write(out, &format!("{prefix}.status.bin"), "transport-error\n");
                        write(out, &format!("{prefix}.transport-error.bin"), other);
                        settled = true;
                        break;
                    }
                }
            }
            if !settled {
                write(out, &format!("{prefix}.status.bin"), "poll-bound\n");
            }
        }
        write(out, &format!("{prefix}.reads-after.bin"), reads.to_string());
    }
    probe.close();
    write(out, "terminal-slots.bin", probe.collect().slots.to_string());
    write(out, "collections.bin", collections.to_string());
    write(out, "reads.bin", reads.to_string());
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert!(
        args.len() >= 7,
        "usage: stage45_annotation_probe SOURCE URI MODULE_NAME EXPRESSIONS OUTPUT FORMAT..."
    );
    let source = fs::read_to_string(&args[1]).expect("source");
    let expressions = fs::read_to_string(&args[4])
        .expect("expressions")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for (index, format) in args[6..].iter().enumerate() {
        capture(
            &Path::new(&args[5]).join(format!("session-{index}")),
            &source,
            &args[2],
            &args[3],
            &expressions,
            format,
        );
    }
}
