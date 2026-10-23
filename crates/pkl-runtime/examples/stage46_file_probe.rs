//! Actual file Host API capture. Shared engine owns URI preparation and caching.
use pkl_runtime::{probe::SourceProbe, value::PklString};
use std::{fs, path::Path};
#[path = "support/module_host.rs"]
mod module_host;
#[path = "stage46_file_probe/observer.rs"]
mod observer;
#[path = "support/resource_host.rs"]
mod resource_host;

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
fn capture(
    out: &Path,
    source: &str,
    uri: &str,
    name: &str,
    expressions: &[String],
    policy: &[String],
) {
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
    let mut observer = observer::Observer::new(
        out,
        &policy[0],
        &policy[1],
        Path::new(&policy[2]),
        Path::new(&policy[3]),
    );
    let mut module_requests = 0usize;
    let mut collections = 0usize;
    for (index, expression) in expressions.iter().enumerate() {
        let prefix = format!("outer-{index}");
        write(out, &format!("{prefix}.expression.bin"), expression);
        write(
            out,
            &format!("{prefix}.reads-before.bin"),
            observer.reads.to_string(),
        );
        write(
            out,
            &format!("{prefix}.host-counters-before.bin"),
            observer.counters(),
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
                    "module-request" => {
                        let token = event.token.expect("module token");
                        let request = probe
                            .module_request(&token)
                            .expect("pending module request");
                        write(
                            out,
                            &format!("module-{module_requests}.request.bin"),
                            format!("{:?}", request.kind),
                        );
                        module_requests += 1;
                        probe
                            .complete_module(&token, observer.module(&request), 0)
                            .expect("typed module completion");
                    }
                    "read" => {
                        let token = event.token.expect("read token");
                        let request = probe.read_request(&token).expect("pending read");
                        if let Some(result) = observer.read(&request) {
                            probe
                                .complete_read(&token, result, 0)
                                .expect("typed file reader completion");
                        } else {
                            probe
                                .respond_configured(&token, 0)
                                .expect("configured reader completion");
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
        write(
            out,
            &format!("{prefix}.reads-after.bin"),
            observer.reads.to_string(),
        );
        write(
            out,
            &format!("{prefix}.host-counters-after.bin"),
            observer.counters(),
        );
        write(
            out,
            &format!("{prefix}.module-requests-after.bin"),
            module_requests.to_string(),
        );
    }
    probe.close();
    write(out, "terminal-slots.bin", probe.collect().slots.to_string());
    write(out, "collections.bin", collections.to_string());
    write(out, "reads.bin", observer.reads.to_string());
    write(out, "host-counters.bin", observer.counters());
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    assert!(
        args.len() == 11,
        "usage: stage46_file_probe SOURCE URI MODULE_NAME EXPRESSIONS OUTPUT SESSIONS MODE DENIED_URI MODULE_CAUSE_UTF16BE RESOURCE_CAUSE_UTF16BE"
    );
    let source = fs::read_to_string(&args[1]).expect("source");
    let expressions = fs::read_to_string(&args[4])
        .expect("expressions")
        .lines()
        .map(str::to_owned)
        .collect::<Vec<_>>();
    for index in 0..args[6].parse::<usize>().expect("session count") {
        capture(
            &Path::new(&args[5]).join(format!("session-{index}")),
            &source,
            &args[2],
            &args[3],
            &expressions,
            &args[7..11],
        );
    }
}
