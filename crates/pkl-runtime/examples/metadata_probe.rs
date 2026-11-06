//! Development-only metadata API comparison boundary. No HTTP or ZIP I/O.
use pkl_runtime::{
    packages::{
        PackageUri, PackageVersion,
        metadata::{MetadataFailure, accept_metadata},
    },
    value::PklString,
};
use std::{fs, io, path::Path};

fn write_text(output: &Path, name: &str, value: &PklString) -> io::Result<()> {
    fs::write(
        output.join(format!("{name}.utf8")),
        value.to_java_utf8().as_bytes(),
    )?;
    fs::write(
        output.join(format!("{name}.utf16be")),
        value
            .encode_utf16()
            .flat_map(u16::to_be_bytes)
            .collect::<Vec<_>>(),
    )
}

fn version_text(version: &PackageVersion) -> PklString {
    let mut value: PklString =
        format!("{}.{}.{}", version.major, version.minor, version.patch).into();
    if let Some(pre) = &version.pre_release {
        value = value.concat(&"-".into()).concat(pre);
    }
    if let Some(build) = &version.build {
        value = value.concat(&"+".into()).concat(build);
    }
    value
}

fn run(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() != 6 {
        return Err(
            "expected PACKAGE_URI REQUEST_URI BODY_PATH EXPECTED_OR_DASH TEST_MODE OUTPUT_DIR"
                .into(),
        );
    }
    let package = PackageUri::parse(args[0].as_str())
        .map_err(|error| format!("invalid package: {:?}", error.failure))?;
    let request: PklString = args[1].as_str().into();
    let bytes = fs::read(&args[2])?;
    let expected = (args[3] != "-").then(|| PklString::from(args[3].as_str()));
    let test_mode = match args[4].as_str() {
        "true" => true,
        "false" => false,
        _ => return Err("TEST_MODE must be true or false".into()),
    };
    let output = Path::new(&args[5]);
    fs::create_dir(output)?;
    match accept_metadata(&package, &request, &bytes, expected.as_ref(), test_mode) {
        Ok(metadata) => {
            fs::write(output.join("status.txt"), b"ok\n")?;
            for (name, value) in [
                ("name", metadata.name),
                ("version", version_text(&metadata.version)),
                ("zip-uri", metadata.package_zip_url),
                ("zip-sha256", metadata.package_zip_sha256),
            ] {
                write_text(output, name, &value)?;
            }
            if !metadata.dependencies.is_empty() {
                fs::create_dir(output.join("dependencies"))?;
            }
            for dependency in metadata.dependencies {
                let key = dependency
                    .name
                    .encode_utf16()
                    .map(|unit| format!("{unit:04x}"))
                    .collect::<String>();
                let key = if key.is_empty() {
                    "empty".to_owned()
                } else {
                    key
                };
                let directory = output.join("dependencies").join(key);
                fs::create_dir(&directory)?;
                for (name, value) in [
                    ("name", dependency.name),
                    ("uri", dependency.package_uri.uri),
                    ("sha256", dependency.sha256),
                ] {
                    write_text(&directory, name, &value)?;
                }
            }
        }
        Err(error) => {
            fs::write(output.join("status.txt"), b"error\n")?;
            let class = if matches!(error, MetadataFailure::Unsupported { .. }) {
                "pkl_rs.Unsupported"
            } else {
                "org.pkl.core.packages.PackageLoadError"
            };
            write_text(output, "error-class", &class.into())?;
            write_text(output, "message", &error.message())?;
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run(&std::env::args().skip(1).collect::<Vec<_>>()) {
        eprintln!("metadata probe: {error}");
        std::process::exit(2);
    }
}
