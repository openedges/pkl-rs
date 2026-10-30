//! One finite provider session. All result text channels retain complete UTF-16
//! and Java UTF-8 boundary bytes. Typed Rust effects are retained as Rust traces,
//! never relabeled as the Java oracle's SPI callback stream.
#[path = "support/glob_fixture.rs"]
mod glob_fixture;
#[path = "support/glob_fixture_host.rs"]
mod glob_fixture_host;

use glob_fixture::{Fixture, Provider};
use glob_fixture_host::FixtureHost;
use pkl_runtime::probe::{SourceLog, SourceProbe};
use pkl_runtime::value::PklString;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

type Result<T> = std::result::Result<T, String>;
fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())
}
fn text(directory: &Path, name: &str, value: &PklString) -> Result<()> {
    write(
        &directory.join(format!("{name}.bin")),
        value.to_java_utf8().as_bytes(),
    )?;
    write(
        &directory.join(format!("{name}.code-units.bin")),
        &value
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>(),
    )
}
fn logger(directory: &Path, index: usize, log: &SourceLog) -> Result<()> {
    let prefix = format!("log-{index:03}");
    text(directory, &format!("{prefix}.level"), &log.level.into())?;
    text(
        directory,
        &format!("{prefix}.message"),
        &PklString::from_utf16(log.message.clone()),
    )?;
    text(
        directory,
        &format!("{prefix}.module-uri"),
        &PklString::from_utf16(log.module_uri.clone()),
    )?;
    text(
        directory,
        &format!("{prefix}.member"),
        &log.member_name
            .clone()
            .map(PklString::from_utf16)
            .unwrap_or_else(|| "null".into()),
    )?;
    text(
        directory,
        &format!("{prefix}.position"),
        &format!(
            "{}:{}-{}:{}",
            log.start_line, log.start_column, log.end_line, log.end_column
        )
        .into(),
    )?;
    for (line, value) in log.source_lines.iter().enumerate() {
        text(
            directory,
            &format!("{prefix}.source-{line}"),
            &PklString::from_utf16(value.clone()),
        )?;
    }
    Ok(())
}
fn run(input: &Path, output: &Path, profile: &str) -> Result<()> {
    let fixture = Fixture::read(input)?;
    let queries = fixture.queries.clone();
    let source_uri = fixture
        .config
        .get("sourceUri")
        .cloned()
        .ok_or("missing source URI")?;
    let contents = if fixture
        .config
        .get("primaryMode")
        .is_some_and(|mode| mode == "provider")
    {
        None
    } else {
        Some(fixture.source.clone())
    };
    let mut host = FixtureHost::new(Provider::new(fixture));
    let mut session = SourceProbe::new_module_evaluator("compact", true, false, false)
        .map_err(|error| format!("probe setup: {error:?}"))?;
    match profile {
        "production16384" => {}
        "explicit512" => session
            .set_glob_test_mode(true)
            .map_err(|error| format!("profile: {:?}", error.message))?,
        _ => return Err("explicit retained glob profile required".into()),
    }
    fs::create_dir(output).map_err(|error| error.to_string())?;
    write(&output.join("profile.bin"), profile.as_bytes())?;
    let mut trace = File::options()
        .write(true)
        .create_new(true)
        .open(output.join("rust-typed-effects.debug.bin"))
        .map_err(|error| error.to_string())?;
    let mut failed: Option<String> = None;
    for (index, query) in queries.iter().enumerate() {
        if !query
            .id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            return Err("invalid query fixture identifier".into());
        }
        let row = output.join(format!("{index:03}-{}", query.id));
        fs::create_dir(&row).map_err(|error| error.to_string())?;
        text(&row, "expression", query.expression.original())?;
        if let Some(reason) = &failed {
            write(&row.join("result.status.bin"), b"unexecuted\n")?;
            write(&row.join("unexecuted.reason.bin"), reason.as_bytes())?;
            continue;
        }
        host.provider.phase = format!("query:{}", query.id);
        let mut logs = 0usize;
        let outcome = (|| -> Result<(String, PklString, PklString)> {
            session
                .evaluate_module_source_text(
                    source_uri.clone(),
                    contents.clone(),
                    query.expression.clone(),
                    0,
                    None,
                )
                .map_err(|error| format!("query start: {:?}", error.message))?;
            for _ in 0..1_000_000 {
                let event = session
                    .poll_text(0, 4096)
                    .map_err(|error| format!("poll API failure: {:?}", error.message))?;
                if event.kind == "yield" {
                    continue;
                }
                writeln!(trace, "{} EVENT {event:?}", host.provider.phase)
                    .map_err(|error| error.to_string())?;
                let token = event.token.as_deref();
                match event.kind {
                    "module-request" => {
                        let token = token.ok_or("module effect without token")?;
                        let request = session
                            .module_request(token)
                            .map_err(|error| format!("module request: {:?}", error.message))?;
                        let completion = host.module(&request)?;
                        writeln!(
                            trace,
                            "{} MODULE {request:?} COMPLETE {completion:?}",
                            host.provider.phase
                        )
                        .map_err(|error| error.to_string())?;
                        session
                            .complete_module(token, completion, 0)
                            .map_err(|error| format!("module completion: {:?}", error.message))?;
                    }
                    "glob-request" => {
                        let token = token.ok_or("glob effect without token")?;
                        let request = session
                            .glob_request(token)
                            .map_err(|error| format!("glob request: {:?}", error.message))?;
                        let completion = host.glob(&request)?;
                        writeln!(
                            trace,
                            "{} GLOB {request:?} COMPLETE {completion:?}",
                            host.provider.phase
                        )
                        .map_err(|error| error.to_string())?;
                        session
                            .complete_glob(token, completion, 0)
                            .map_err(|error| format!("glob completion: {:?}", error.message))?;
                    }
                    "read" => {
                        let token = token.ok_or("read effect without token")?;
                        let request = session
                            .read_request(token)
                            .map_err(|error| format!("read request: {:?}", error.message))?;
                        let uri = session
                            .resolved_read_uri_text(token)
                            .map_err(|error| format!("resolved read URI: {:?}", error.message))?;
                        let completion = host.read(&uri)?;
                        writeln!(
                            trace,
                            "{} READ {request:?} RESOLVED {uri:?} COMPLETE {completion:?}",
                            host.provider.phase
                        )
                        .map_err(|error| error.to_string())?;
                        session
                            .complete_read(token, completion, 0)
                            .map_err(|error| format!("read completion: {:?}", error.message))?;
                    }
                    "log" => {
                        logger(
                            &row,
                            logs,
                            event.log.as_ref().ok_or("log effect lacks body")?,
                        )?;
                        logs += 1;
                        session
                            .acknowledge_log(token.ok_or("log effect lacks token")?, None, 0)
                            .map_err(|error| format!("logger completion: {:?}", error.message))?;
                    }
                    "value" => {
                        return Ok((
                            "success\n".into(),
                            PklString::from_utf16(event.value.ok_or("value event lacks value")?),
                            PklString::default(),
                        ));
                    }
                    "error" => {
                        let status = if event.disposition == Some("unsupported") {
                            "unsupported\n"
                        } else {
                            "error\n"
                        };
                        return Ok((
                            status.into(),
                            PklString::default(),
                            PklString::from_utf16(
                                event.error.ok_or("error event lacks diagnostic")?,
                            ),
                        ));
                    }
                    _ => return Err(format!("unexpected source event {}", event.kind)),
                }
            }
            Err("finite driver poll bound exceeded".into())
        })();
        match outcome {
            Ok((status, value, error)) => {
                write(&row.join("result.status.bin"), status.as_bytes())?;
                text(&row, "result.stdout", &value)?;
                text(&row, "result.stderr", &error)?;
            }
            Err(reason) => {
                write(&row.join("result.status.bin"), b"adapter-error\n")?;
                write(&row.join("adapter-error.bin"), reason.as_bytes())?;
                failed = Some(reason);
            }
        }
        write(&row.join("logs.count.bin"), format!("{logs}\n").as_bytes())?;
    }
    session.close();
    writeln!(trace, "close SOURCE_SESSION_CLOSED").map_err(|error| error.to_string())?;
    write(
        &output.join("fixture-decisions.debug.bin"),
        format!("{:#?}\n", host.provider.decisions).as_bytes(),
    )?;
    write(
        &output.join("session.complete.bin"),
        if failed.is_some() {
            b"adapter-failure\n"
        } else {
            b"complete\n"
        },
    )?;
    if let Some(reason) = failed {
        return Err(reason);
    }
    Ok(())
}
fn main() {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    let result = if args.len() == 3 {
        args[2]
            .to_str()
            .ok_or_else(|| "profile must be ASCII".to_owned())
            .and_then(|profile| run(Path::new(&args[0]), Path::new(&args[1]), profile))
    } else {
        Err("usage: glob_fixture_probe INPUT FRESH_OUTPUT production16384|explicit512".into())
    };
    if let Err(error) = result {
        eprintln!("{error}");
        std::process::exit(2);
    }
}
