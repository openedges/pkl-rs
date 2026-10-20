# Pinned Java oracle

The development oracle is the official `org.pkl-lang:pkl-tools:0.32.0` shaded
JVM artifact from Maven Central. It contains the CLI, core evaluator, server,
configuration API, documentation tool, and Java/Kotlin code generators. It is
used for verification and baseline measurements; it is not a production fallback.
A successful bootstrap or repeated Java result does not establish Rust language
compatibility or performance.

[oracle.lock.json](../oracle.lock.json) pins the download URL, SHA-256, embedded
release version and source commit, source metadata hashes, JDK identity, heap
settings, and execution policy. The JAR's `org/pkl/core/Release.properties`
contains version `0.32.0` and commit `e51fb2f`, corresponding to the full commit
in [references.lock](../references.lock). No reference checkout is modified.

## Bootstrap and verification

Requirements are Python 3.9 or newer, Git, and Eclipse Temurin OpenJDK
`21.0.12.1+1-LTS` (`java.version=21.0.12.1`). Provision that exact JDK separately
from [Adoptium's official releases](https://adoptium.net/temurin/releases/).
The tool does not install system software. Select its executable using
`PKL_ORACLE_JAVA` when the pinned JDK is not the `java` on `PATH`:

```sh
export PKL_ORACLE_JAVA=/absolute/path/to/pinned-jdk/bin/java
python3 tools/oracle.py fetch
python3 tools/oracle.py verify
python3 tools/oracle.py smoke
python3 tools/oracle.py fetch --profile performance
python3 tools/oracle.py verify --profile performance
python3 -m unittest discover -s tools/tests -p 'test_oracle.py'
```

`fetch` downloads the fixed HTTPS URL to a temporary file, verifies SHA-256 and
embedded metadata, and atomically installs `.cache/oracle/pkl-tools-0.32.0.jar`.
It never replaces an existing corrupt artifact. Remove a corrupt cache file
explicitly before fetching again. `verify` and `smoke` never fetch artifacts;
a missing or corrupted artifact, wrong JDK vendor/build, dirty reference
checkout, mismatched source revision/gitlink, or changed source metadata fails.
The SHA-256 is a reviewed project pin calculated from the official publication,
not an independent signature verification. The artifact is not rebuilt locally.

The full `fetch` command is also the clean-cache bootstrap check. A JDK with the
same major version but another patch, vendor, or runtime build is rejected. The
exact executable, runtime `lib/modules`, and JDK `release` hashes are recorded in
provenance for the current OS/architecture; this does not assert that JDK binaries
on different platforms have identical bytes. Changing JDK policy requires an
explicit lock update and new evidence.

The default `reference` profile uses externally provisioned Temurin 21 for the
byte oracle. The `performance` profile downloads the separately pinned official
GraalVM Community 25.0.1 archive into `.cache/oracle` and extracts it there.
It verifies the archive SHA-256 and installed Java launcher, runtime modules,
release metadata, JVM library, Java library, and optimizing compiler library
against locked hashes. Currently only macOS arm64 has a pinned performance
distribution; other platforms fail explicitly until their distribution and
runtime hashes are reviewed and added. System JDK installations are untouched.

## Execution contract and evidence

Each run records `target/m0/oracle/provenance.json`. It includes the lock hash,
artifact URL/hash, source metadata, absolute Java and JAR paths, Java identity and
binary hashes, OS/architecture, complete JVM command, allowlisted environment,
and raw Java/Pkl version output encoded as hexadecimal. No inherited environment
or secrets are logged.
The performance profile writes `performance-provenance.json`, including the
official JDK archive URL/hash and an actual Java source-launch probe of
`Truffle.getRuntime().getName()`. It requires the exact optimizing runtime name
`GraalVM CE`, with no stderr. A runtime probe establishes compiler availability;
actual warmup and compiled workload evidence belong to the benchmark runner.

Execution uses UTC, English/US Java locale, UTF-8, a noninteractive terminal,
fixed terminal width, a 64 MiB initial heap and 512 MiB maximum heap, and isolated
home/temp directories under `.cache/oracle`. The subprocess environment is an
allowlist; inherited `JAVA_TOOL_OPTIONS`, `JDK_JAVA_OPTIONS`, `_JAVA_OPTIONS`,
`CLASSPATH`, credentials, proxy settings, and user settings are not passed through.
Callers must supply an explicit working directory and record their inputs,
resources, CLI arguments, and allowed network fixtures. This bootstrap does not
sandbox arbitrary upstream modules or block all network access.

`smoke` evaluates a successful module and a module with a missing member twice.
It checks the known success bytes and exit status, checks that the error exits
with code 1 on stderr, then compares each replay's complete raw stdout, stderr,
and exit code. The error result is captured from Java, not synthesized or used
as evidence of Rust compatibility. Files are:

- `work/success.pkl` and `work/error.pkl`: exact smoke inputs.
- `success.stdout.bin`, `success.stderr.bin`, `error.stdout.bin`,
  `error.stderr.bin`: untouched bytes from the first execution.
- `smoke.json`: input/output hashes, commands, working paths, exit statuses,
  replay checks, and oracle provenance.

With `--profile performance`, smoke evidence is placed in the `performance/`
subdirectory so it cannot overwrite reference-profile captures.

Source locations contain the actual working path. Move or mount the entire test
workspace consistently for differential comparisons; never rewrite diagnostics
to hide path differences. Version text also contains the actual OS/JDK identity.

Python callers can use the importable interface:

```python
metadata = ensure_oracle(repository_root, fetch=False)
result = subprocess.run(
    [*metadata["command"], "eval", "--no-project", "--color=never", "case.pkl"],
    cwd=case_directory,
    env=metadata["environment"],
    capture_output=True,
)
```

The returned metadata is JSON-compatible. `command` ends with
`org.pkl.cli.Main` and is ready for CLI arguments; `java` and `artifact` are
absolute paths for JVM API probes. Keep subprocess output as bytes.
Use `ensure_oracle(repository_root, fetch=False, profile="performance")` for
measurements against the optimizing GraalVM runtime.

## Upstream build, Truffle, and licensing

The pinned `pkl-tools/pkl-tools.gradle.kts` defines the shaded publication.
Upstream source builds require JDK 25 or newer and target JVM bytecode 17 by
default; this is distinct from this project's selected runtime JDK 21.
`gradle/libs.versions.toml` pins Graal/Truffle dependencies to `25.0.1`.
The default reference runtime is Temurin HotSpot using the artifact's bundled Truffle
runtime, with no external Graal compiler, no additional polyglot engine flags,
and no native-image executable. Upstream suppresses its interpreter-only warning
in `VmUtils`. This interpreter-based reference must not be presented as
measurement against an optimizing GraalVM. Performance measurements use the
separate GraalVM profile, which permits native access and legacy Unsafe memory
access explicitly with `--enable-native-access=ALL-UNNAMED` and
`--sun-misc-unsafe-memory-access=allow`; these match upstream dependency needs
on JDK 25 and avoid warnings changing captured stderr. Neither flag disables
Truffle optimization. Both profiles use the locked 64 MiB initial/512 MiB maximum
heap; this is a fixed experiment condition, not the upstream CLI default or a
claim about total resident memory. Performance reports must identify the
configuration and distinguish cold and warmed measurements. Native-image
measurements remain a separate comparison.

The official artifact is Apache-2.0 with third-party components. Consult the
pinned source [LICENSE.txt](../external/pkl/LICENSE.txt),
[NOTICE.txt](../external/pkl/NOTICE.txt), and
[THIRD-PARTY-NOTICES.txt](../external/pkl/THIRD-PARTY-NOTICES.txt), whose hashes are
in the lock. Preserve these notices when redistributing. JAR license entries can
belong to shaded dependencies and do not replace the complete upstream notices.
Temurin is GPL-2.0 with the Classpath exception; its installed `legal/` directory
contains the runtime's applicable notices. The downloaded GraalVM Community
distribution and its components also carry notices in their extracted `legal/`
directory, including GPL-2.0 with the Classpath exception.
